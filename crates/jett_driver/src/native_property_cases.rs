//! Translate deterministic property inputs into typed, compiler-owned HIR.
//! The interpreter chooses cases; only compiled Jett code executes their bodies.

use jett_common::{FileId, Span};
use jett_comptime::value::{ClosureTypeContext, Value};
use jett_comptime::verify::{PROPERTY_DEFAULT_ITERATIONS, PropertyCase};
use jett_hir::{
    Block, DeclarationId, DeclarationKind, Expression, ExpressionKind, Function, FunctionDebugKind,
    FunctionId, FunctionIdentity, HandleKind, IntrinsicId, Local, LocalId, MapEntry, ParamMode,
    StateId, Statement, StatementKind, VariantId,
};
use jett_parser::ast::{Item, Module};
use jett_types::{ReflectionMetadata, ReflectionTypeInfo, Type, TypeId, TypeInterner};
use std::collections::HashSet;

pub(super) fn append_property_suite(
    hir: &mut jett_hir::Program,
    module: &Module,
    entry_file: FileId,
    cases: &[PropertyCase],
    types: &TypeInterner,
    reflection: &ReflectionMetadata,
    method_value_definitions: &HashSet<Span>,
) -> Result<Option<NativePropertyPlan>, Vec<jett_hir::LowerError>> {
    let function_values = function_value_candidates(&hir.functions, method_value_definitions);
    let mut trials = Vec::new();
    let mut statements = Vec::new();
    let mut locals = Vec::new();
    let mut first_identity = None;
    let mut first_span = None;
    for item in &module.items {
        let Item::Property(property) = item else {
            continue;
        };
        if property.span.file != entry_file {
            continue;
        }
        let mut matches = hir.functions.iter().filter(|function| {
            function.span == property.span
                && function.identity.declaration.kind == DeclarationKind::Property
        });
        let Some(function) = matches.next() else {
            return Err(error(
                property.span,
                "checked property has no exact HIR function",
            ));
        };
        if matches.next().is_some() {
            return Err(error(
                property.span,
                "checked property has multiple HIR functions",
            ));
        }
        if first_identity.is_none() {
            first_identity = Some(function.identity.declaration.clone());
            first_span = Some(property.span);
        }
        let mut count = 0;
        for case in cases
            .iter()
            .filter(|case| case.property_span == property.span)
        {
            if case.iteration != count || case.arguments.len() != function.params.len() {
                return Err(error(
                    property.span,
                    "generated property cases disagree with checked parameters or iteration order",
                ));
            }
            let trial = NativePropertyTrial {
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
            append_trial(
                &trial,
                function,
                types,
                reflection,
                &function_values,
                None,
                &mut locals,
                &mut statements,
            )?;
            trials.push(trial);
            count += 1;
        }
        if count != PROPERTY_DEFAULT_ITERATIONS {
            return Err(error(
                property.span,
                format!(
                    "checked property has {count} generated cases; expected {PROPERTY_DEFAULT_ITERATIONS}"
                ),
            ));
        }
    }
    let Some(span) = first_span else {
        return Ok(None);
    };
    let declaration = first_identity.expect("a suite span has a checked declaration");
    let id = FunctionId::new(hir.functions.len() as u32);
    hir.functions.push(Function {
        id,
        debug_kind: jett_hir::FunctionDebugKind::named(
            &declaration.namespace,
            &format!("__native_property_suite:{}", span.start),
        ),
        identity: FunctionIdentity {
            declaration: DeclarationId {
                origin: declaration.origin,
                namespace: declaration.namespace,
                name: format!("__native_property_suite:{}", span.start),
                kind: DeclarationKind::Property,
            },
            scoped_type_bindings: Vec::new(),
            type_arguments: Vec::new(),
            specialization: jett_typecheck::CheckedGenericSpecialization::default(),
        },
        source_definition: None,
        params: Vec::new(),
        capture_count: 0,
        return_type: TypeInterner::NOTHING,
        locals,
        body: Block { statements, span },
        span,
    });
    Ok(Some(NativePropertyPlan {
        entry: id,
        trials,
        function_values,
        checked: None,
    }))
}

/// The opaque sampler and HIR factory own all typed values, locals and calls.
/// The ordinary fallback above keeps its existing public behavior.
pub(super) fn append_checked_property_suite(
    hir: &mut jett_hir::Program,
    checked: &std::sync::Arc<jett_typecheck::CheckedResourceProgram>,
    entry_file: FileId,
    cases: &jett_comptime::verify::CheckedPropertyCases,
) -> Result<Option<NativePropertyPlan>, Vec<jett_hir::LowerError>> {
    let Some(plan) =
        jett_hir::append_checked_native_property_suite(hir, checked, entry_file, cases)?
    else {
        return Ok(None);
    };
    let trials = plan
        .trials()
        .iter()
        .map(|trial| NativePropertyTrial {
            name: trial.name().to_owned(),
            span: trial.span(),
            function: trial.function(),
            given_names: trial.given_names().to_vec(),
            case: trial.case().clone(),
        })
        .collect();
    Ok(Some(NativePropertyPlan {
        entry: plan.entry(),
        trials,
        function_values: Vec::new(),
        checked: Some(plan),
    }))
}

/// Exact checked inputs and identities retained for native replay and shrinking.
/// Its contents are compiler-owned and cannot be supplied by source programs.
#[derive(Debug)]
pub struct NativePropertyPlan {
    pub(crate) entry: FunctionId,
    pub(crate) trials: Vec<NativePropertyTrial>,
    function_values: Vec<FunctionValueCandidate>,
    checked: Option<jett_hir::CheckedNativePropertyPlan>,
}

#[derive(Clone, Debug)]
pub(crate) struct NativePropertyTrial {
    pub(crate) name: String,
    pub(crate) span: Span,
    pub(crate) function: FunctionId,
    pub(crate) given_names: Vec<String>,
    pub(crate) case: PropertyCase,
}

impl NativePropertyPlan {
    pub(crate) fn replay_mir(
        &self,
        lowered: &crate::BackendLoweringResult,
        trials: &[NativePropertyTrial],
        validate_inputs: bool,
    ) -> Result<jett_mir::Program, crate::BackendLoweringError> {
        if let Some(plan) = &self.checked {
            // Public case values select only original trial identities. HIR
            // rebuilds its private Source graph and validates changed inputs.
            let mut cases = Vec::with_capacity(trials.len());
            for trial in trials {
                let mut selected = plan.trials().iter().filter(|original| {
                    original.case().property_span == trial.case.property_span
                        && original.case().iteration == trial.case.iteration
                });
                let original = selected.next().ok_or_else(|| {
                    crate::BackendLoweringError::Hir(error(
                        trial.span,
                        "native Property replay lost its original retained trial tuple",
                    ))
                })?;
                if selected.next().is_some()
                    || trial.name != original.name()
                    || trial.span != original.span()
                    || trial.function != original.function()
                    || trial.given_names != original.given_names()
                {
                    return Err(crate::BackendLoweringError::Hir(error(
                        trial.span,
                        "native Property replay changed its retained name/span/function/given tuple",
                    )));
                }
                cases.push(trial.case.clone());
            }
            let mut hir = jett_hir::rebuild_checked_native_property_replay(
                &lowered.hir,
                plan,
                &cases,
                &lowered.interner,
            )
            .map_err(crate::BackendLoweringError::Hir)?;
            jett_hir::complete_value_conversions(&mut hir, &lowered.interner)
                .map_err(crate::BackendLoweringError::Hir)?;
            let mir = jett_mir::lower(&hir, &lowered.interner)
                .map_err(crate::BackendLoweringError::Mir)?;
            jett_mir::validate(&mir).map_err(crate::BackendLoweringError::MirValidation)?;
            return Ok(mir);
        }
        let mut hir = lowered.hir.clone();
        let mut locals = Vec::new();
        let mut statements = Vec::new();
        for trial in trials {
            let function = hir
                .functions
                .iter()
                .find(|function| function.id == trial.function)
                .ok_or_else(|| {
                    crate::BackendLoweringError::Hir(error(
                        trial.span,
                        "native property replay lost its checked function",
                    ))
                })?;
            append_trial(
                trial,
                function,
                &lowered.interner,
                &lowered.reflection_metadata,
                &self.function_values,
                validate_inputs.then_some(hir.functions.as_slice()),
                &mut locals,
                &mut statements,
            )
            .map_err(crate::BackendLoweringError::Hir)?;
        }
        let suite = hir
            .functions
            .iter_mut()
            .find(|function| function.id == self.entry)
            .expect("checked property plan retains its suite entry");
        suite.locals = locals;
        suite.body.statements = statements;
        jett_hir::complete_value_conversions(&mut hir, &lowered.interner)
            .map_err(crate::BackendLoweringError::Hir)?;
        let mir =
            jett_mir::lower(&hir, &lowered.interner).map_err(crate::BackendLoweringError::Mir)?;
        jett_mir::validate(&mir).map_err(crate::BackendLoweringError::MirValidation)?;
        Ok(mir)
    }
}

fn append_trial(
    trial: &NativePropertyTrial,
    function: &Function,
    types: &TypeInterner,
    reflection: &ReflectionMetadata,
    functions: &[FunctionValueCandidate],
    refinement_functions: Option<&[Function]>,
    locals: &mut Vec<Local>,
    statements: &mut Vec<Statement>,
) -> Result<(), Vec<jett_hir::LowerError>> {
    let span = trial.case.property_span;
    if trial.case.arguments.len() != function.params.len() {
        return Err(error(
            span,
            "native replay arguments disagree with checked parameters",
        ));
    }
    statements.push(property_case_context(
        Some(jett_hir::NativePropertyCase {
            name: trial.name.clone(),
            trial: trial.case.iteration as u64 + 1,
        }),
        span,
    ));
    let mut context = ValueContext {
        actor_seeds: None,
        types,
        reflection,
        functions,
        refinement_functions,
        locals,
        bindings: statements,
    };
    let args = trial
        .case
        .arguments
        .iter()
        .zip(&function.params)
        .map(|(value, param)| value_expression(value, param.ty, span, &mut context))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|message| error(span, message))?;
    let evaluation_order = (0..args.len()).collect::<Vec<_>>();
    let parameter_types = function
        .params
        .iter()
        .map(|parameter| parameter.ty)
        .collect::<Vec<_>>();
    let access = function
        .params
        .iter()
        .map(|parameter| match parameter.mode {
            ParamMode::Owned => jett_typecheck::CheckedCalleeAccess::Owned,
            ParamMode::View => jett_typecheck::CheckedCalleeAccess::View,
        })
        .collect::<Vec<_>>();
    let ownership = jett_hir::CallOwnership::generated(
        jett_hir::GeneratedOperation::NativeSuite {
            function: function.id,
        },
        &args,
        &parameter_types,
        &access,
        TypeInterner::NOTHING,
        &evaluation_order,
        types,
    )
    .map_err(|message| error(span, message))?;
    statements.push(Statement {
        kind: StatementKind::Expression(Expression {
            kind: ExpressionKind::Call {
                function: function.id,
                evaluation_order,
                ownership,
                args,
            },
            ty: TypeInterner::NOTHING,
            span,
        }),
        span,
    });
    statements.push(property_case_context(None, span));
    Ok(())
}

fn property_case_context(case: Option<jett_hir::NativePropertyCase>, span: Span) -> Statement {
    Statement {
        kind: StatementKind::Expression(Expression {
            kind: ExpressionKind::PropertyCaseContext(case),
            ty: TypeInterner::NOTHING,
            span,
        }),
        span,
    }
}

fn error(span: Span, message: impl Into<String>) -> Vec<jett_hir::LowerError> {
    vec![jett_hir::LowerError {
        span,
        message: message.into(),
    }]
}

pub(super) use jett_hir::evaluated_values::{
    FunctionValueCandidate, ValueContext, evaluated_intrinsic_expression,
    function_value_candidates, value_expression,
};
