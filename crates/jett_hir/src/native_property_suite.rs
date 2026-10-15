//! Canonical Property wrappers over an immutable checked case/cache graph.
use crate::evaluated_values::{ValueContext, function_value_candidates, value_expression};
use crate::{
    Block, CallOwnership, DeclarationId, DeclarationKind, Expression, ExpressionKind, Function,
    FunctionDebugKind, FunctionId, FunctionIdentity, GeneratedOperation, Local, LowerError,
    ParamMode, Program, Statement, StatementKind,
};
use jett_common::{FileId, Span};
use jett_comptime::verify::{CheckedPropertyCases, PROPERTY_DEFAULT_ITERATIONS, PropertyCase};
use jett_comptime::{ExplicitComptimeValues, Value};
use jett_parser::ast::Item;
use jett_typecheck::{CheckedCalleeAccess, CheckedResourceProgram};
use jett_types::{Type, TypeId, TypeInterner};
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct CheckedNativePropertyTrial {
    name: String,
    span: Span,
    function: FunctionId,
    given_names: Vec<String>,
    case: PropertyCase,
}
impl CheckedNativePropertyTrial {
    pub fn name(&self) -> &str {
        &self.name
    }
    pub fn span(&self) -> Span {
        self.span
    }
    pub fn function(&self) -> FunctionId {
        self.function
    }
    pub fn given_names(&self) -> &[String] {
        &self.given_names
    }
    pub fn case(&self) -> &PropertyCase {
        &self.case
    }
}

/// No public constructor: only checked sampling and canonical HIR construction
/// produce the original body, capture, input and purpose association.
#[derive(Debug, Clone)]
pub struct CheckedNativePropertyPlan {
    data: Arc<PropertyData>,
}
#[derive(Debug)]
struct PropertyData {
    base: Program,
    cases: CheckedPropertyCases,
    checked: Arc<CheckedResourceProgram>,
    entry_file: FileId,
    entry: FunctionId,
    trials: Vec<CheckedNativePropertyTrial>,
    canonical: Function,
}
impl CheckedNativePropertyPlan {
    pub fn entry(&self) -> FunctionId {
        self.data.entry
    }
    pub fn trials(&self) -> &[CheckedNativePropertyTrial] {
        &self.data.trials
    }
    pub(crate) fn same_plan(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.data, &other.data)
    }
}

pub fn append_checked_native_property_suite(
    program: &mut Program,
    checked: &Arc<CheckedResourceProgram>,
    entry_file: FileId,
    cases: &CheckedPropertyCases,
) -> Result<Option<CheckedNativePropertyPlan>, Vec<LowerError>> {
    input(program, checked)?;
    if !cases.belongs_to(checked)
        || !crate::reflected_callable::baked::same_required_cache(program, cases.required_values())
        || crate::reflected_callable::baked::has_property_suite(program)
    {
        return Err(error(
            fallback_span(program),
            "native Property cases changed their checked program/cache/purpose",
        ));
    }
    let mut trials = Vec::new();
    let mut locals = Vec::new();
    let mut statements = Vec::new();
    let mut first = None;
    for item in &checked.module().items {
        let Item::Property(property) = item else {
            continue;
        };
        if property.span.file != entry_file {
            continue;
        }
        let function = original_target(program, checked, property)?;
        if first.is_none() {
            first = Some(function.clone());
        }
        let selected = cases
            .cases()
            .iter()
            .filter(|case| case.property_span == property.span)
            .collect::<Vec<_>>();
        if selected.len() != PROPERTY_DEFAULT_ITERATIONS {
            return Err(error(
                property.span,
                format!(
                    "checked property has {} generated cases; expected {PROPERTY_DEFAULT_ITERATIONS}",
                    selected.len()
                ),
            ));
        }
        for (iteration, case) in selected.into_iter().enumerate() {
            if case.iteration != iteration || case.arguments.len() != function.params.len() {
                return Err(error(
                    property.span,
                    "native Property sampling changed its exact parameter/iteration tuple",
                ));
            }
            let trial = CheckedNativePropertyTrial {
                name: property.name.name.clone(),
                span: property.name.span,
                function: function.id,
                given_names: property
                    .givens
                    .iter()
                    .map(|given| given.name.name.clone())
                    .collect(),
                case: case.clone(),
            };
            append_trial(program, checked, &trial, None, &mut locals, &mut statements)?;
            trials.push(trial);
        }
    }
    let Some(first) = first else {
        return Ok(None);
    };
    let entry = FunctionId::new(u32::try_from(program.functions.len()).map_err(|_| {
        error(
            first.span,
            "native Property function table exceeds its ID range",
        )
    })?);
    let canonical = suite_function(
        entry,
        &first.identity.declaration,
        first.span,
        locals,
        statements,
    );
    let plan = CheckedNativePropertyPlan {
        data: Arc::new(PropertyData {
            base: program.clone(),
            cases: cases.clone(),
            checked: checked.clone(),
            entry_file,
            entry,
            trials,
            canonical: canonical.clone(),
        }),
    };
    install(program, canonical, &plan)?;
    Ok(Some(plan))
}

/// Rebuild from the private pre-suite graph; a public edited HIR body never
/// supplies Source or callback authority. Changed shrink inputs are validated.
pub fn rebuild_checked_native_property_replay(
    program: &Program,
    plan: &CheckedNativePropertyPlan,
    cases: &[PropertyCase],
    types: &TypeInterner,
) -> Result<Program, Vec<LowerError>> {
    let checked = &plan.data.checked;
    input(program, checked)?;
    program
        .resource_source
        .validate_types(types)
        .map_err(|message| error(fallback_span(program), message))?;
    if !crate::reflected_callable::baked::owns_property_suite(program, plan)
        || !plan.data.cases.belongs_to(checked)
        || !crate::reflected_callable::baked::same_required_cache(
            program,
            plan.data.cases.required_values(),
        )
    {
        return Err(error(
            fallback_span(program),
            "native Property replay does not own this exact checked suite",
        ));
    }
    let mut candidate = plan.data.base.clone();
    input(&candidate, checked)?;
    let mut previous = None;
    let mut locals = Vec::new();
    let mut statements = Vec::new();
    for case in cases {
        if case.property_span.file != plan.data.entry_file {
            return Err(error(
                case.property_span,
                "native Property replay changed its entry-file purpose",
            ));
        }
        let mut matches = plan.data.trials.iter().enumerate().filter(|(_, trial)| {
            trial.case.property_span == case.property_span && trial.case.iteration == case.iteration
        });
        let Some((ordinal, original)) = matches.next() else {
            return Err(error(
                case.property_span,
                "native Property replay changed its original property/iteration identity",
            ));
        };
        if matches.next().is_some() || previous.is_some_and(|prior| ordinal <= prior) {
            return Err(error(
                case.property_span,
                "native Property replay duplicated or reordered an original trial",
            ));
        }
        previous = Some(ordinal);
        let mut trial = original.clone();
        trial.case = case.clone();
        append_trial(
            &candidate,
            checked,
            &trial,
            Some(&original.case.arguments),
            &mut locals,
            &mut statements,
        )?;
    }
    let original = &plan.data.canonical;
    let canonical = suite_function(
        plan.data.entry,
        &original.identity.declaration,
        original.span,
        locals,
        statements,
    );
    install(&mut candidate, canonical, plan)?;
    Ok(candidate)
}

fn input(program: &Program, checked: &Arc<CheckedResourceProgram>) -> Result<(), Vec<LowerError>> {
    let span = fallback_span(program);
    if !program.resource_source.belongs_to(checked)
        || !program
            .resource_source
            .checked_program()
            .is_some_and(|retained| Arc::ptr_eq(retained, checked))
        || !program.resource_manifest.kinds().is_empty()
    {
        return Err(error(
            span,
            "native Property transition has no exact protected ordinary Source archive",
        ));
    }
    program
        .resource_source
        .validate_current(program, &checked.checked().interner)
        .map_err(|message| error(span, message))?;
    crate::reflected_callable::baked::validate_conversion_input(program)?;
    crate::actor_graph::validate_conversion_input(program)?;
    Ok(())
}

fn original_target<'a>(
    program: &'a Program,
    checked: &Arc<CheckedResourceProgram>,
    property: &jett_parser::ast::PropertyBlock,
) -> Result<&'a Function, Vec<LowerError>> {
    let span = property.span;
    let mut namespace = "";
    for item in &checked.module().items {
        if let Item::Namespace(item) = item {
            if item.span.file == span.file {
                namespace = &item.name.name;
            }
        }
    }
    let origin = checked
        .source_origins()
        .get(&span.file)
        .ok_or_else(|| error(span, "checked Property has no original Source origin"))?;
    let name = format!("{}:{}", property.name.name, span.start);
    let mut matches = program
        .resource_source
        .functions()
        .iter()
        .filter(|function| {
            function.span == span
                && function.identity.declaration.kind == DeclarationKind::Property
                && function.identity.declaration.namespace == namespace
                && function.identity.declaration.name == name
                && &function.identity.declaration.origin == origin
        });
    let original = matches
        .next()
        .ok_or_else(|| error(span, "checked Property has no exact original HIR body"))?;
    if matches.next().is_some() {
        return Err(error(
            span,
            "checked Property has multiple original HIR bodies",
        ));
    }
    let current = program
        .functions
        .get(original.id.index() as usize)
        .ok_or_else(|| error(span, "checked Property lost its protected current body"))?;
    if current.id != original.id
        || current.identity != original.identity
        || current.debug_kind != original.debug_kind
        || current.source_definition != original.source_definition
        || current.span != original.span
        || current.body.span != original.body.span
        || current.params != original.params
        || current.capture_count != 0
        || current.return_type != TypeInterner::NOTHING
        || current.params.len() != property.givens.len()
        || current
            .params
            .iter()
            .zip(&property.givens)
            .any(|(param, given)| param.name != given.name.name)
        || !current.identity.scoped_type_bindings.is_empty()
        || !current.identity.type_arguments.is_empty()
        || current.identity.specialization
            != jett_typecheck::CheckedGenericSpecialization::default()
    {
        return Err(error(
            span,
            "checked Property changed its original formal/header tuple",
        ));
    }
    Ok(current)
}

fn append_trial(
    program: &Program,
    checked: &Arc<CheckedResourceProgram>,
    trial: &CheckedNativePropertyTrial,
    sampled: Option<&[Value]>,
    locals: &mut Vec<Local>,
    statements: &mut Vec<Statement>,
) -> Result<(), Vec<LowerError>> {
    let span = trial.case.property_span;
    let function = program
        .functions
        .get(trial.function.index() as usize)
        .ok_or_else(|| error(span, "native Property trial lost its checked function"))?;
    if function.id != trial.function
        || function.span != span
        || function.identity.declaration.kind != DeclarationKind::Property
        || trial.case.arguments.len() != function.params.len()
    {
        return Err(error(
            span,
            "native Property trial changed its original callable tuple",
        ));
    }
    let types = &checked.checked().interner;
    let functions = function_value_candidates(
        &program.functions,
        &checked.checked().method_value_definitions,
    );
    statements.push(case_context(
        Some(crate::NativePropertyCase {
            name: trial.name.clone(),
            trial: u64::try_from(trial.case.iteration)
                .map_err(|_| error(span, "native Property trial ordinal exceeds its range"))?
                + 1,
        }),
        span,
    ));
    let mut args = Vec::new();
    for (index, (value, param)) in trial
        .case
        .arguments
        .iter()
        .zip(&function.params)
        .enumerate()
    {
        authenticate_callbacks(program, value, param.ty, checked)
            .map_err(|message| error(span, message))?;
        let unchanged = sampled
            .and_then(|values| values.get(index))
            .is_some_and(|original| original.same_checked_snapshot(value));
        let mut context = ValueContext {
            actor_seeds: None,
            types,
            reflection: &checked.checked().reflection_metadata,
            functions: &functions,
            refinement_functions: sampled
                .filter(|_| !unchanged)
                .map(|_| program.functions.as_slice()),
            locals,
            bindings: statements,
        };
        args.push(
            value_expression(value, param.ty, span, &mut context)
                .map_err(|message| error(span, message))?,
        );
    }
    let evaluation_order = (0..args.len()).collect::<Vec<_>>();
    let parameters = function
        .params
        .iter()
        .map(|param| param.ty)
        .collect::<Vec<_>>();
    let access = function
        .params
        .iter()
        .map(|param| match param.mode {
            ParamMode::Owned => CheckedCalleeAccess::Owned,
            ParamMode::View => CheckedCalleeAccess::View,
        })
        .collect::<Vec<_>>();
    let ownership = CallOwnership::generated(
        GeneratedOperation::NativeSuite {
            function: function.id,
        },
        &args,
        &parameters,
        &access,
        TypeInterner::NOTHING,
        &evaluation_order,
        types,
    )
    .map_err(|message| error(span, message))?;
    let expression = Expression {
        kind: ExpressionKind::Call {
            function: function.id,
            args,
            evaluation_order,
            ownership,
        },
        ty: TypeInterner::NOTHING,
        span,
    };
    let local_facts = locals
        .iter()
        .map(|local| crate::OwnershipLocalInfo {
            ty: local.ty,
            mutable: local.mutable,
            span: local.span,
            view_source: local.view_source,
            is_view_parameter: false,
            view_iteration: None,
            match_binding: None,
        })
        .collect::<Vec<_>>();
    crate::validate_hir_invocation(
        &program.functions,
        &local_facts,
        &expression,
        types,
        jett_typecheck::CheckedOwnershipContext::Property,
    )
    .map_err(|message| error(span, message))?;
    statements.push(Statement {
        kind: StatementKind::Expression(expression),
        span,
    });
    statements.push(case_context(None, span));
    Ok(())
}

// Authenticate callback descriptors before the general value materializer can
// select a body by its typed shape. Values are inspected as stored metadata;
// pending callbacks are never joined or invoked here.
fn authenticate_callbacks(
    program: &Program,
    value: &Value,
    ty: TypeId,
    checked: &Arc<CheckedResourceProgram>,
) -> Result<(), String> {
    let types = &checked.checked().interner;
    match (types.resolve(ty), value) {
        (_, Value::Pending(inner) | Value::Typed { value: inner, .. }) => {
            authenticate_callbacks(program, inner, ty, checked)
        }
        (Type::Refinement { base, .. } | Type::Secret(base), _) => {
            authenticate_callbacks(program, value, *base, checked)
        }
        (Type::Function { .. }, _) => {
            value.validate_checked_callable_source(checked, ty)?;
            match value {
                Value::Function { captures, .. } => {
                    let functions = function_value_candidates(
                        &program.functions,
                        &checked.checked().method_value_definitions,
                    );
                    let selected =
                        crate::evaluated_values::selected_function(value, ty, types, &functions)?;
                    for (name, capture_type) in selected.captures() {
                        let captured = captures
                            .get(name)
                            .ok_or("native Property callback lost an original capture")?;
                        authenticate_callbacks(program, captured, *capture_type, checked)?;
                    }
                    Ok(())
                }
                Value::ReflectedCallable(adapter) => authenticate_callbacks(
                    program,
                    adapter.source(),
                    adapter.source_type(),
                    checked,
                ),
                _ => Ok(()),
            }
        }
        (Type::List(element) | Type::Set(element), Value::List(values) | Value::Set(values)) => {
            for value in values {
                authenticate_callbacks(program, value, *element, checked)?;
            }
            Ok(())
        }
        (Type::Map(key, mapped), Value::Map(entries)) => {
            for (k, v) in entries {
                authenticate_callbacks(program, k, *key, checked)?;
                authenticate_callbacks(program, v, *mapped, checked)?;
            }
            Ok(())
        }
        (Type::Optional(inner), Value::OptionalSome(value))
        | (Type::Result(inner, _), Value::ResultOk(value))
        | (Type::Result(_, inner), Value::ResultFail(value)) => {
            authenticate_callbacks(program, value, *inner, checked)
        }
        (Type::Struct(id), Value::Struct { fields, .. }) => {
            for (name, ty) in &types.resolve_struct(*id).fields {
                let value = fields
                    .iter()
                    .find(|(field, _)| field == name)
                    .ok_or("native Property struct field is absent")?;
                authenticate_callbacks(program, &value.1, *ty, checked)?;
            }
            Ok(())
        }
        (
            Type::Enum(id),
            Value::Enum {
                variant, fields, ..
            },
        ) => {
            let variant = types
                .resolve_enum(*id)
                .variants
                .iter()
                .find(|candidate| candidate.name == *variant)
                .ok_or("native Property enum variant is absent")?;
            if fields.len() != variant.fields.len() {
                return Err("native Property enum payload arity changed".into());
            }
            for (value, (_, ty)) in fields.iter().zip(&variant.fields) {
                authenticate_callbacks(program, value, *ty, checked)?;
            }
            Ok(())
        }
        (
            Type::Machine(id) | Type::MachineState { machine: id, .. },
            Value::Machine { state, fields, .. },
        ) => {
            let state = types
                .resolve_machine(*id)
                .states
                .iter()
                .find(|candidate| candidate.name == *state)
                .ok_or("native Property machine state is absent")?;
            if fields.len() != state.fields.len() {
                return Err("native Property machine payload arity changed".into());
            }
            for (value, (_, ty)) in fields.iter().zip(&state.fields) {
                authenticate_callbacks(program, value, *ty, checked)?;
            }
            Ok(())
        }
        (Type::TypeConstruction, Value::TypeConstruction { fields, .. }) => {
            for (_, _, stored, value) in fields {
                let declared = if let Some(ty) = checked
                    .checked()
                    .reflection_metadata
                    .source_type_id_for_name(stored)
                {
                    ty
                } else {
                    let mut candidates = types
                        .type_ids()
                        .filter(|ty| types.type_name(*ty) == *stored);
                    let selected = candidates
                        .next()
                        .ok_or("native Property builder field has no checked type identity")?;
                    if candidates.next().is_some() {
                        return Err(
                            "native Property builder field has ambiguous checked type identity"
                                .into(),
                        );
                    }
                    selected
                };
                // The shared builder materializer separately joins this stored
                // field tuple to the original checked builder layout.
                authenticate_callbacks(program, value, declared, checked)?;
            }
            Ok(())
        }
        (Type::Interface(_), _) => {
            // General reconstruction selects the same concrete identity; an
            // opaque erased callable cannot bypass exact Source authentication.
            if contains_callback(value) {
                return Err(
                    "native Property erased callable has no exact checked concrete Source identity"
                        .into(),
                );
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

fn contains_callback(value: &Value) -> bool {
    match value {
        Value::NamedFunction(_) | Value::Function { .. } | Value::ReflectedCallable(_) => true,
        Value::Typed { value, .. }
        | Value::Pending(value)
        | Value::OptionalSome(value)
        | Value::ResultOk(value)
        | Value::ResultFail(value) => contains_callback(value),
        Value::List(values)
        | Value::Set(values)
        | Value::Enum { fields: values, .. }
        | Value::Machine { fields: values, .. } => values.iter().any(contains_callback),
        Value::Map(entries) => entries
            .iter()
            .any(|(key, value)| contains_callback(key) || contains_callback(value)),
        Value::Struct { fields, .. } => fields.iter().any(|(_, value)| contains_callback(value)),
        Value::TypeConstruction { fields, .. } => fields
            .iter()
            .any(|(_, _, _, value)| contains_callback(value)),
        _ => false,
    }
}

fn suite_function(
    id: FunctionId,
    declaration: &DeclarationId,
    span: Span,
    locals: Vec<Local>,
    statements: Vec<Statement>,
) -> Function {
    let name = format!("__native_property_suite:{}", span.start);
    Function {
        id,
        debug_kind: FunctionDebugKind::named(&declaration.namespace, &name),
        identity: FunctionIdentity {
            declaration: DeclarationId {
                origin: declaration.origin.clone(),
                namespace: declaration.namespace.clone(),
                name,
                kind: DeclarationKind::Property,
            },
            scoped_type_bindings: Vec::new(),
            type_arguments: Vec::new(),
            specialization: jett_typecheck::CheckedGenericSpecialization::default(),
        },
        source_definition: None,
        actor_handler: None,
        actor_state_views: Vec::new(),
        params: Vec::new(),
        capture_count: 0,
        return_type: TypeInterner::NOTHING,
        locals,
        body: Block { statements, span },
        span,
    }
}
fn install(
    program: &mut Program,
    canonical: Function,
    plan: &CheckedNativePropertyPlan,
) -> Result<(), Vec<LowerError>> {
    let before = program.clone();
    input(&before, &plan.data.checked)?;
    if canonical.id != plan.data.entry
        || canonical.id.index() as usize != before.functions.len()
        || canonical.span != plan.data.canonical.span
        || canonical.identity != plan.data.canonical.identity
        || !canonical.params.is_empty()
        || canonical.capture_count != 0
        || canonical.return_type != TypeInterner::NOTHING
    {
        return Err(error(
            canonical.span,
            "private Property constructor changed its canonical purpose/header",
        ));
    }
    let mut candidate = before.clone();
    candidate.functions.push(canonical);
    let mut graph_candidate = candidate.clone();
    crate::actor_graph::complete_conversion(&mut graph_candidate, &before)?;
    crate::reflected_callable::baked::complete_conversion(&mut candidate, &before)?;
    candidate.resource_source.graph_values = graph_candidate.resource_source.graph_values;
    crate::reflected_callable::baked::install_property_suite(&mut candidate, plan.clone())?;
    if before.resource_source.graph_values.is_some() {
        candidate
            .resource_source
            .install_materialized(crate::resource_source::Materialized {
                functions: candidate.functions.clone(),
                values: before.resource_source.required_materializations().to_vec(),
                required_only: before.resource_source.required_only_function_ids().to_vec(),
            });
    }
    input(&candidate, &plan.data.checked)?;
    *program = candidate;
    Ok(())
}
fn case_context(case: Option<crate::NativePropertyCase>, span: Span) -> Statement {
    Statement {
        kind: StatementKind::Expression(Expression {
            kind: ExpressionKind::PropertyCaseContext(case),
            ty: TypeInterner::NOTHING,
            span,
        }),
        span,
    }
}
fn fallback_span(program: &Program) -> Span {
    program
        .functions
        .first()
        .map_or(Span::new(FileId::new(0), 0, 0), |function| function.span)
}
fn error(span: Span, message: impl Into<String>) -> Vec<LowerError> {
    vec![LowerError {
        span,
        message: message.into(),
    }]
}

#[cfg(test)]
#[path = "native_property_suite_tests.rs"]
mod tests;
