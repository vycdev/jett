use crate::checked_types::{CheckedExpressionTypes, CheckedFunctionTypes, CheckedScopedTypes};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use jett_common::{FileId, Span};
use jett_diagnostics::Diagnostic;
use jett_parser::ast::{Block, Expr, Item, Module, Stmt, StringPart};
use jett_types::{ReflectionMetadata, ReflectionTypeInfo};

use crate::resource_execution::{CheckedAttemptKey, PreparedRequiredExpression};
pub use crate::resource_execution::{
    CheckedRequiredOwner, CheckedRequiredResourceHook, CheckedRequiredScope, CheckedRequiredValue,
};
use crate::value::ClosureScopedTypeBinding;
use crate::{DebugEvent, Interpreter, Value};

#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
pub struct ComptimeContext {
    pub type_arguments: Vec<String>,
    pub type_argument_reflections: Vec<ReflectionTypeInfo>,
    pub type_info_kinds: Vec<(usize, String)>,
    pub type_info_primitives: Vec<(usize, Option<String>)>,
    pub type_kind_values: Vec<(usize, String)>,
    pub type_primitive_values: Vec<(usize, String)>,
    pub scoped_type_bindings: Vec<ClosureScopedTypeBinding>,
}

impl ComptimeContext {
    pub fn from_checked(function: Option<&CheckedFunctionTypes>) -> Self {
        let Some(function) = function else {
            return Self::default();
        };
        Self {
            type_arguments: function.type_arguments.clone(),
            type_argument_reflections: function.type_argument_reflections.clone(),
            type_info_kinds: function.type_info_kinds.clone(),
            type_info_primitives: function.type_info_primitives.clone(),
            type_kind_values: function.type_kind_values.clone(),
            type_primitive_values: function.type_primitive_values.clone(),
            scoped_type_bindings: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct ExplicitComptimeValues {
    values: HashMap<(Span, ComptimeContext), Value>,
    constants: HashMap<Span, Value>,
    // Private authority association. Each successful insertion below also
    // mirrors the value into values or constants in that same branch, so the
    // existing public collection views count reusable values once.
    checked_values: HashMap<CheckedAttemptKey, Value>,
}

impl ExplicitComptimeValues {
    pub fn len(&self) -> usize {
        self.values.len() + self.constants.len()
    }

    pub fn is_empty(&self) -> bool {
        self.values.is_empty() && self.constants.is_empty()
    }

    pub fn values(&self) -> impl Iterator<Item = &Value> {
        self.values.values().chain(self.constants.values())
    }

    pub fn get(&self, span: Span, context: &ComptimeContext) -> Option<&Value> {
        self.values.get(&(span, context.clone()))
    }

    pub fn insert(&mut self, span: Span, context: ComptimeContext, value: Value) {
        self.values.insert((span, context), value);
    }

    /// Authenticate successful private required entries for this exact checked
    /// program. Public span/context mirrors are never used to mint proof.
    pub fn checked_required_values(
        &self,
        program: &Arc<jett_typecheck::CheckedResourceProgram>,
    ) -> Result<Vec<CheckedRequiredValue>, String> {
        self.checked_values
            .keys()
            .map(|key| CheckedRequiredValue::from_cache(self, key, program))
            .collect()
    }

    /// Readonly hook projections of the same private successful entries.
    pub fn checked_resource_hook_values(
        &self,
        program: &Arc<jett_typecheck::CheckedResourceProgram>,
    ) -> Result<Vec<CheckedRequiredResourceHook>, String> {
        Ok(self
            .checked_required_values(program)?
            .into_iter()
            .filter_map(CheckedRequiredResourceHook::from_value)
            .collect())
    }
    pub(crate) fn checked_get(&self, key: &CheckedAttemptKey) -> Option<&Value> {
        self.checked_values.get(key)
    }

    fn checked_insert(&mut self, key: CheckedAttemptKey, value: Value) -> Result<(), String> {
        if value.contains_live_resource_or_grant() {
            return Err(
                "required value cannot cache runtime authority or Resource custody".to_string(),
            );
        }
        self.checked_values.insert(key, value);
        Ok(())
    }

    #[cfg(test)]
    pub(crate) fn checked_values_are_mirrored(&self) -> bool {
        self.checked_values.values().all(|checked| {
            self.values()
                .any(|value| match (value.payload(), checked.payload()) {
                    (Value::ResourceHook(left), Value::ResourceHook(right)) => {
                        crate::resource_execution::tests::same_resource_hook_descriptor_identity(
                            left, right,
                        )
                    }
                    _ => value == checked,
                })
        })
    }

    /// A checked namespace constant, independent of any function instantiation.
    pub fn constant(&self, declaration: Span) -> Option<&Value> {
        self.constants.get(&declaration)
    }
}

/// Actual required-evaluation observations, separate from the reusable baked values.
/// Reading a baked value must never replay these events.
#[derive(Debug, Clone, Default)]
pub struct ExplicitComptimeEvaluation {
    pub values: ExplicitComptimeValues,
    pub diagnostics: Vec<Diagnostic>,
    pub debug_events: Vec<DebugEvent>,
}

struct CollectedExpression<'a> {
    namespace: Option<String>,
    aliases: HashMap<String, String>,
    expression: &'a Expr,
    span: Span,
    owner: Option<(Span, Vec<String>)>,
    bindings: Vec<(Span, String)>,
}

#[derive(Clone, Default)]
struct EvaluationContext {
    required: Option<PreparedRequiredExpression>,
    function: Option<Arc<CheckedFunctionTypes>>,
    scope: Option<Arc<CheckedScopedTypes>>,
    bindings: Vec<ClosureScopedTypeBinding>,
}

impl EvaluationContext {
    fn key(&self) -> ComptimeContext {
        let mut key = ComptimeContext::from_checked(self.function.as_deref());
        key.scoped_type_bindings = self.bindings.clone();
        key
    }
}

/// Evaluate namespace constants and every explicit `comptime` expression in a
/// checked module.
///
/// The expressions are evaluated without runtime locals or parameters. Visible
/// `use` aliases are lexical name bindings and are carried into evaluation.
/// An explicit comptime value remains closed and reproducible at build time.
pub fn evaluate_explicit_comptime_expressions(
    module: &Module,
    reflection_metadata: Arc<ReflectionMetadata>,
    checked_expression_types: Arc<CheckedExpressionTypes>,
    breakpoint_exclusions: Arc<HashMap<Span, HashSet<String>>>,
) -> (ExplicitComptimeValues, Vec<Diagnostic>) {
    let captured = evaluate_explicit_comptime_expressions_capture(
        module,
        reflection_metadata,
        checked_expression_types,
        breakpoint_exclusions,
    );
    (captured.values, captured.diagnostics)
}

/// Evaluate required values once and retain ordered debug observations, including
/// events emitted before a failed expression or namespace constant initializer.
/// The worker records silently; callers select whether and how to render events.
pub fn evaluate_explicit_comptime_expressions_capture(
    module: &Module,
    reflection_metadata: Arc<ReflectionMetadata>,
    checked_expression_types: Arc<CheckedExpressionTypes>,
    breakpoint_exclusions: Arc<HashMap<Span, HashSet<String>>>,
) -> ExplicitComptimeEvaluation {
    let mut expressions = Vec::new();
    collect_module_expressions(module, &mut expressions);
    let span = expressions
        .first()
        .map(|expression| expression.span)
        .or_else(|| {
            module.items.iter().find_map(|item| {
                if let Item::VarDecl(decl) = item {
                    Some(decl.name.span)
                } else {
                    None
                }
            })
        });
    let Some(span) = span else {
        return ExplicitComptimeEvaluation::default();
    };
    // Compiler callers may have a smaller stack than reference execution.
    // Keep required comptime evaluation on the same fixed interpreter budget.
    std::thread::scope(|scope| {
        match std::thread::Builder::new()
            .name("jett-comptime".into())
            .stack_size(crate::INTERPRETER_STACK_SIZE)
            .spawn_scoped(scope, move || {
                evaluate_collected_expressions(
                    module,
                    expressions,
                    reflection_metadata,
                    checked_expression_types,
                    breakpoint_exclusions,
                )
            }) {
            Ok(handle) => match handle.join() {
                Ok(result) => result,
                Err(payload) => std::panic::resume_unwind(payload),
            },
            Err(error) => ExplicitComptimeEvaluation {
                diagnostics: vec![Diagnostic::error(
                    9001,
                    format!("cannot create comptime evaluation worker: {error}"),
                    span,
                )],
                ..ExplicitComptimeEvaluation::default()
            },
        }
    })
}

fn evaluate_collected_expressions(
    module: &Module,
    expressions: Vec<CollectedExpression<'_>>,
    reflection_metadata: Arc<ReflectionMetadata>,
    checked_expression_types: Arc<CheckedExpressionTypes>,
    breakpoint_exclusions: Arc<HashMap<Span, HashSet<String>>>,
) -> ExplicitComptimeEvaluation {
    if let Some(program) = &checked_expression_types.resource_program {
        if !std::ptr::eq(module, program.module()) {
            return ExplicitComptimeEvaluation {
                diagnostics: vec![Diagnostic::error(
                    0,
                    "required worker received a foreign checked source module",
                    module.span,
                )],
                ..ExplicitComptimeEvaluation::default()
            };
        }
    }
    let mut interpreter = Interpreter::new();
    if let Some(program) = &checked_expression_types.resource_program {
        if let Err(error) = interpreter
            .install_checked_resource_program(
                program.clone(),
                crate::ExecutionPurpose::ExplicitComptime,
            )
            .and_then(|()| {
                interpreter.authorize_checked_resource_worker(
                    module,
                    crate::ExecutionPurpose::ExplicitComptime,
                )
            })
        {
            return ExplicitComptimeEvaluation {
                diagnostics: vec![Diagnostic::error(0, error, module.span)],
                ..ExplicitComptimeEvaluation::default()
            };
        }
    }
    interpreter.set_reflection_metadata(reflection_metadata);
    interpreter.set_checked_expression_types(checked_expression_types.clone());
    interpreter.set_breakpoint_exclusions(breakpoint_exclusions);
    interpreter.register_module(module);

    let mut values = ExplicitComptimeValues::default();
    let mut diagnostics = Vec::new();
    let previous_purpose =
        interpreter.checked_execution_purpose(crate::ExecutionPurpose::NamespaceConstant);
    let mut attempted_checked = HashSet::new();
    let mut attempted_expressions = evaluate_constants(
        module,
        &checked_expression_types,
        &mut interpreter,
        &mut values,
        &mut diagnostics,
        &mut attempted_checked,
    );
    if let Some(purpose) = previous_purpose {
        interpreter.checked_execution_purpose(purpose);
    }
    for collected in expressions {
        let contexts = match interpreter.checked_explicit_contexts(
            collected.expression,
            collected.span,
            collected.owner.as_ref().map(|(span, _)| *span),
            &collected.bindings,
        ) {
            Ok(Some(contexts)) => contexts
                .into_iter()
                .map(|required| EvaluationContext {
                    function: required.function_projection(),
                    scope: required.scope_projection(),
                    bindings: required.bindings(),
                    required: Some(required),
                })
                .collect(),
            Ok(None) => evaluation_contexts(&collected, &checked_expression_types),
            Err(error) => {
                diagnostics.push(Diagnostic::error(9001, error, collected.span));
                continue;
            }
        };
        let parameters = collected
            .owner
            .as_ref()
            .map_or(&[][..], |(_, names)| names.as_slice());
        for context in contexts {
            let key = context.key();
            let checked_key = context.required.as_ref().map(|required| required.key());
            if let Some(identity) = &checked_key {
                if values.checked_get(identity).is_some()
                    || !attempted_checked.insert(identity.clone())
                {
                    continue;
                }
            } else {
                if values.get(collected.span, &key).is_some() {
                    continue;
                }
                if !attempted_expressions.insert((collected.span, key.clone())) {
                    continue;
                }
            }
            let evaluated = if let Some(required) = &context.required {
                interpreter.eval_checked_required_expression(
                    required,
                    collected.namespace.as_deref(),
                    &collected.aliases,
                    parameters,
                )
            } else {
                interpreter.eval_closed_comptime_expression(
                    collected.namespace.as_deref(),
                    &collected.aliases,
                    collected.expression,
                    parameters,
                    context.function,
                    context.scope,
                    context.bindings,
                )
            };
            match evaluated {
                Ok(value) => {
                    if let Some(identity) = checked_key {
                        if let Err(error) = values.checked_insert(identity, value.clone()) {
                            diagnostics.push(Diagnostic::error(9001, error, collected.span)); continue;
                        }
                    }
                    values.insert(collected.span, key, value);
                }
                Err(error) => diagnostics.push(Diagnostic::error(
                    9001,
                    format!("`comptime` expression must be closed and evaluable during compilation: {error}"),
                    collected.span,
                )),
            }
        }
    }
    ExplicitComptimeEvaluation {
        values,
        diagnostics,
        debug_events: interpreter.take_debug_events(),
    }
}

fn evaluate_constants(
    module: &Module,
    checked: &CheckedExpressionTypes,
    interpreter: &mut Interpreter,
    values: &mut ExplicitComptimeValues,
    diagnostics: &mut Vec<Diagnostic>,
    attempted_checked: &mut HashSet<CheckedAttemptKey>,
) -> HashSet<(Span, ComptimeContext)> {
    let mut attempted = HashSet::new();
    let mut current_file = None;
    let mut namespace = None;
    for item in &module.items {
        let file = item_file(item);
        if current_file.is_some_and(|previous| previous != file) {
            namespace = None;
        }
        current_file = Some(file);
        if let Item::Namespace(declaration) = item {
            namespace = Some(declaration.name.name.clone());
        }
        let Item::VarDecl(declaration) = item else {
            continue;
        };
        let ty = checked.get(&declaration.name.span).map(String::as_str);
        if !matches!(
            ty,
            Some(
                "int8"
                    | "int16"
                    | "int32"
                    | "int64"
                    | "uint8"
                    | "uint16"
                    | "uint32"
                    | "uint64"
                    | "float32"
                    | "float64"
                    | "string"
                    | "bool"
                    | "nothing"
            )
        ) {
            diagnostics.push(Diagnostic::error(
                9001,
                "constant execution currently requires an implicitly copyable primitive type; move-only constant ownership is unresolved",
                declaration.name.span,
            ));
            continue;
        }
        let expression = match &declaration.value {
            Expr::Comptime(inner, _) => inner.as_ref(),
            expression => expression,
        };
        // The same explicit root is also in the collected-expression pass.
        // Record an actual attempt independently of whether it yields a value,
        // so error recovery does not rerun it or repeat its observations.
        if let Expr::Comptime(_, span) = &declaration.value {
            attempted.insert((*span, ComptimeContext::default()));
        }
        let prepared = match interpreter.checked_namespace_initializer(declaration) {
            Ok(prepared) => prepared,
            Err(error) => {
                diagnostics.push(Diagnostic::error(9001, error, declaration.value.span()));
                continue;
            }
        };
        if let Some(prepared) = &prepared {
            attempted_checked.insert(prepared.key());
        }
        let evaluated = if let Some(prepared) = &prepared {
            interpreter.eval_checked_required_expression(
                prepared,
                namespace.as_deref(),
                &HashMap::new(),
                &[],
            )
        } else {
            interpreter.eval_closed_comptime_expression(
                namespace.as_deref(),
                &HashMap::new(),
                expression,
                &[],
                None,
                None,
                Vec::new(),
            )
        };
        match evaluated {
            Ok(value) => {
                if !constant_value_matches_type(&value, ty) {
                    diagnostics.push(Diagnostic::error(
                        9001,
                        "global constant value does not match its declared primitive type; pending tasks and other hidden runtime state cannot be baked as primitive constants",
                        declaration.value.span(),
                    ));
                    continue;
                }
                if let Some(prepared) = &prepared {
                    if let Err(error) = values.checked_insert(prepared.key(), value.clone()) {
                        diagnostics.push(Diagnostic::error(9001, error, declaration.value.span()));
                        continue;
                    }
                }
                interpreter.register_constant_in_namespace(
                    namespace.as_deref(),
                    declaration,
                    value.clone(),
                );
                if let Expr::Comptime(_, span) = declaration.value {
                    values.insert(span, ComptimeContext::default(), value.clone());
                }
                values.constants.insert(declaration.name.span, value);
            }
            Err(error) => diagnostics.push(Diagnostic::error(
                9001,
                format!("global constant must be evaluable during compilation: {error}"),
                declaration.value.span(),
            )),
        }
    }
    attempted
}

fn constant_value_matches_type(value: &Value, ty: Option<&str>) -> bool {
    // Sized primitives retain their checked identity around the scalar carrier.
    // Removing that metadata must not also unwrap a pending task.
    match (value.payload(), ty) {
        (Value::Int64(value), Some("int8")) => i8::try_from(*value).is_ok(),
        (Value::Int64(value), Some("int16")) => i16::try_from(*value).is_ok(),
        (Value::Int64(value), Some("int32")) => i32::try_from(*value).is_ok(),
        (Value::Int64(_), Some("int64")) => true,
        // The interpreter uses its signed carrier for these unsigned widths.
        (Value::Int64(value), Some("uint8")) => u8::try_from(*value).is_ok(),
        (Value::Int64(value), Some("uint16")) => u16::try_from(*value).is_ok(),
        (Value::Int64(value), Some("uint32")) => u32::try_from(*value).is_ok(),
        (Value::Uint64(_), Some("uint64")) => true,
        (Value::Float64(_), Some("float32" | "float64"))
        | (Value::String(_), Some("string"))
        | (Value::Bool(_), Some("bool"))
        | (Value::Nothing, Some("nothing")) => true,
        _ => false,
    }
}

fn evaluation_contexts(
    expression: &CollectedExpression<'_>,
    checked: &CheckedExpressionTypes,
) -> Vec<EvaluationContext> {
    let mut contexts = if let Some((owner, parameters)) = &expression.owner {
        match checked.functions.get(owner) {
            Some(instances) => instances
                .iter()
                .map(|instance| EvaluationContext {
                    function: Some(instance.clone()),
                    ..EvaluationContext::default()
                })
                .collect(),
            None if !parameters.is_empty() => Vec::new(),
            None => vec![EvaluationContext::default()],
        }
    } else {
        vec![EvaluationContext::default()]
    };
    for (span, name) in &expression.bindings {
        contexts = contexts
            .into_iter()
            .flat_map(|context| {
                let bindings = context
                    .scope
                    .as_ref()
                    .map(|scope| &scope.bindings)
                    .or_else(|| context.function.as_ref().map(|function| &function.bindings))
                    .unwrap_or(&checked.bindings);
                bindings
                    .get(span)
                    .into_iter()
                    .flatten()
                    .map(|scope| {
                        let mut next = context.clone();
                        next.scope = Some(scope.clone());
                        next.bindings.push(ClosureScopedTypeBinding {
                            name: name.clone(),
                            canonical_name: scope.bound_type.clone(),
                            reflection: scope.reflection.clone(),
                        });
                        next
                    })
                    .collect::<Vec<_>>()
            })
            .collect();
    }
    contexts.retain(|context| {
        let expressions = context
            .scope
            .as_ref()
            .map(|scope| &scope.expressions)
            .or_else(|| {
                context
                    .function
                    .as_ref()
                    .map(|function| function.expressions.as_ref())
            });
        expressions.is_none_or(|expressions| expressions.contains_key(&expression.span))
    });
    contexts
}

fn collect_module_expressions<'a>(
    module: &'a Module,
    expressions: &mut Vec<CollectedExpression<'a>>,
) {
    let mut current_file = None;
    let mut current_namespace = None;
    for item in &module.items {
        let file = item_file(item);
        if current_file.is_some_and(|current| current != file) {
            current_namespace = None;
        }
        current_file = Some(file);
        if let Item::Namespace(namespace) = item {
            current_namespace = Some(namespace.name.name.clone());
        }
        collect_item(item, current_namespace.as_deref(), expressions);
    }
}

fn item_file(item: &Item) -> FileId {
    match item {
        Item::Namespace(item) => item.span.file,
        Item::Function(item) => item.span.file,
        Item::Mutual(item) => item.span.file,
        Item::Interface(item) => item.span.file,
        Item::Implement(item) => item.span.file,
        Item::Struct(item) => item.span.file,
        Item::Bitfield(item) => item.span.file,
        Item::Enum(item) => item.span.file,
        Item::Machine(item) => item.span.file,
        Item::Actor(item) => item.span.file,
        Item::VarDecl(item) => item.span.file,
        Item::Verify(item) => item.span.file,
        Item::Property(item) => item.span.file,
        Item::Resource(item) => item.span.file,
        Item::TypeAlias(item) => item.span.file,
    }
}

fn collect_item<'a>(
    item: &'a Item,
    namespace: Option<&str>,
    expressions: &mut Vec<CollectedExpression<'a>>,
) {
    let aliases = &HashMap::new();
    match item {
        Item::Function(function) => {
            collect_function(function, &[], namespace, aliases, expressions)
        }
        Item::Implement(implementation) => {
            for method in &implementation.methods {
                collect_function(method, &[], namespace, aliases, expressions);
            }
        }
        Item::Struct(structure) => {
            for method in &structure.methods {
                collect_function(
                    method,
                    &structure.type_params,
                    namespace,
                    aliases,
                    expressions,
                );
            }
        }
        Item::Actor(actor) => {
            for field in &actor.state_fields {
                collect_expr(&field.value, namespace, aliases, expressions);
            }
            for handler in &actor.handlers {
                collect_block(&handler.body, namespace, aliases, expressions);
            }
        }
        Item::VarDecl(declaration) => {
            collect_expr(&declaration.value, namespace, aliases, expressions)
        }
        Item::Verify(verify) => collect_block(&verify.body, namespace, aliases, expressions),
        Item::Property(property) => collect_block(&property.body, namespace, aliases, expressions),
        Item::TypeAlias(alias) => {
            if let Some(constraint) = &alias.constraint {
                collect_expr(constraint, namespace, aliases, expressions);
            }
        }
        Item::Namespace(_)
        | Item::Mutual(_)
        | Item::Interface(_)
        | Item::Bitfield(_)
        | Item::Enum(_)
        | Item::Resource(_)
        | Item::Machine(_) => {}
    }
}

fn collect_function<'a>(
    function: &'a jett_parser::ast::FunctionDef,
    outer_parameters: &[jett_parser::ast::Ident],
    namespace: Option<&str>,
    aliases: &HashMap<String, String>,
    expressions: &mut Vec<CollectedExpression<'a>>,
) {
    let start = expressions.len();
    collect_block(&function.body, namespace, aliases, expressions);
    let parameters = outer_parameters
        .iter()
        .chain(&function.type_params)
        .map(|parameter| parameter.name.clone())
        .collect::<Vec<_>>();
    for expression in &mut expressions[start..] {
        expression.owner = Some((function.name.span, parameters.clone()));
    }
}

fn collect_block<'a>(
    block: &'a Block,
    namespace: Option<&str>,
    aliases: &HashMap<String, String>,
    expressions: &mut Vec<CollectedExpression<'a>>,
) {
    let mut visible_aliases = aliases.clone();
    for statement in &block.stmts {
        collect_stmt(statement, namespace, &mut visible_aliases, expressions);
    }
}

fn collect_stmt<'a>(
    statement: &'a Stmt,
    namespace: Option<&str>,
    aliases: &mut HashMap<String, String>,
    expressions: &mut Vec<CollectedExpression<'a>>,
) {
    match statement {
        Stmt::VarDecl(declaration) => {
            collect_expr(&declaration.value, namespace, aliases, expressions)
        }
        Stmt::Assign(assignment) => {
            collect_expr(&assignment.target, namespace, aliases, expressions);
            collect_expr(&assignment.value, namespace, aliases, expressions);
        }
        Stmt::Return(statement) => {
            if let Some(value) = &statement.value {
                collect_expr(value, namespace, aliases, expressions);
            }
        }
        Stmt::Respond(statement) => collect_expr(&statement.value, namespace, aliases, expressions),
        Stmt::ComptimeTypeBind(binding) => {
            collect_expr(&binding.value, namespace, aliases, expressions);
            let start = expressions.len();
            collect_block(&binding.body, namespace, aliases, expressions);
            for expression in &mut expressions[start..] {
                expression
                    .bindings
                    .insert(0, (binding.span, binding.name.name.clone()));
            }
        }
        Stmt::If(statement) => {
            collect_expr(&statement.condition, namespace, aliases, expressions);
            collect_block(&statement.then_block, namespace, aliases, expressions);
            for (condition, block) in &statement.else_ifs {
                collect_expr(condition, namespace, aliases, expressions);
                collect_block(block, namespace, aliases, expressions);
            }
            if let Some(block) = &statement.else_block {
                collect_block(block, namespace, aliases, expressions);
            }
        }
        Stmt::For(statement) => {
            collect_expr(&statement.iterable, namespace, aliases, expressions);
            collect_block(&statement.body, namespace, aliases, expressions);
        }
        Stmt::While(statement) => {
            collect_expr(&statement.condition, namespace, aliases, expressions);
            collect_block(&statement.body, namespace, aliases, expressions);
        }
        Stmt::Match(statement) => {
            collect_expr(&statement.expr, namespace, aliases, expressions);
            for arm in &statement.arms {
                collect_block(&arm.body, namespace, aliases, expressions);
            }
        }
        Stmt::Expr(statement) => collect_expr(&statement.expr, namespace, aliases, expressions),
        Stmt::Assert(statement) => {
            collect_expr(&statement.condition, namespace, aliases, expressions);
            if let Some(message) = &statement.message {
                collect_expr(message, namespace, aliases, expressions);
            }
        }
        Stmt::Breakpoint(statement) => {
            if let Some(condition) = &statement.condition {
                collect_expr(condition, namespace, aliases, expressions);
            }
        }
        Stmt::Use(declaration) => {
            let bound_name =
                Interpreter::use_bound_name(&declaration.path.name, declaration.alias.as_ref());
            aliases.insert(bound_name, declaration.path.name.clone());
        }
        Stmt::Trace(_) | Stmt::Break(_) | Stmt::Continue(_) => {}
    }
}

fn collect_expr<'a>(
    expression: &'a Expr,
    namespace: Option<&str>,
    aliases: &HashMap<String, String>,
    expressions: &mut Vec<CollectedExpression<'a>>,
) {
    match expression {
        Expr::Comptime(inner, span) => {
            expressions.push(CollectedExpression {
                namespace: namespace.map(str::to_string),
                aliases: aliases.clone(),
                expression: inner,
                span: *span,
                owner: None,
                bindings: Vec::new(),
            });
            // A baked inline function can still execute its body later. Its
            // nested explicit expressions need independent closed values too.
            collect_expr(inner, namespace, aliases, expressions);
        }
        Expr::Binary(left, _, right, _) => {
            collect_expr(left, namespace, aliases, expressions);
            collect_expr(right, namespace, aliases, expressions);
        }
        Expr::Unary(_, inner, _)
        | Expr::FieldAccess(inner, _, _)
        | Expr::Paren(inner, _)
        | Expr::View(inner, _)
        | Expr::Ok(inner, _)
        | Expr::Fail(inner, _)
        | Expr::Some(inner, _)
        | Expr::Default(inner, _)
        | Expr::Declassify(inner, _)
        | Expr::Coarsen(inner, _)
        | Expr::At(inner, _, _)
        | Expr::Spawn(inner, _)
        | Expr::Send(inner, _)
        | Expr::Ask(inner, _)
        | Expr::Clone(inner, _)
        | Expr::Run(inner, _)
        | Expr::Join(inner, _)
        | Expr::Cancel(inner, _) => collect_expr(inner, namespace, aliases, expressions),
        Expr::Call(callee, arguments, _) => {
            collect_expr(callee, namespace, aliases, expressions);
            for argument in arguments {
                collect_expr(&argument.value, namespace, aliases, expressions);
            }
        }
        Expr::GenericCall(callee, _, arguments, _) => {
            collect_expr(callee, namespace, aliases, expressions);
            for argument in arguments {
                collect_expr(&argument.value, namespace, aliases, expressions);
            }
        }
        Expr::ListConstruct(items, _) => {
            for item in items {
                collect_expr(item, namespace, aliases, expressions);
            }
        }
        Expr::MapConstruct(entries, _) => {
            for (key, value) in entries {
                collect_expr(key, namespace, aliases, expressions);
                collect_expr(value, namespace, aliases, expressions);
            }
        }
        Expr::Handle(target, _, block, _) => {
            collect_expr(target, namespace, aliases, expressions);
            collect_block(block, namespace, aliases, expressions);
        }
        Expr::StringInterpolation(parts, _) => {
            for part in parts {
                if let StringPart::Expr(expression) = part {
                    collect_expr(expression, namespace, aliases, expressions);
                }
            }
        }
        Expr::Pipeline(initial, steps, _) => {
            collect_expr(initial, namespace, aliases, expressions);
            for step in steps {
                collect_expr(&step.function, namespace, aliases, expressions);
                for argument in &step.extra_args {
                    collect_expr(&argument.value, namespace, aliases, expressions);
                }
                if let Some(handle) = &step.handle {
                    collect_block(&handle.body, namespace, aliases, expressions);
                }
            }
        }
        Expr::InlineFn(_, _, block, _) => collect_block(block, namespace, aliases, expressions),
        Expr::IntLiteral(_, _)
        | Expr::FloatLiteral(_, _)
        | Expr::StringLiteral(_, _)
        | Expr::BoolLiteral(_, _)
        | Expr::Nothing(_)
        | Expr::Ident(_)
        | Expr::None(_)
        | Expr::EnumVariant(_, _, _)
        | Expr::Error(_) => {}
    }
}

#[cfg(test)]
mod debug_capture_tests {
    use super::*;
    use crate::DebugEventKind;

    fn parse(source: &str) -> Module {
        let parsed = jett_parser::parse(source, FileId::new(0));
        assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
        parsed.module
    }

    fn capture(module: &Module, checked: CheckedExpressionTypes) -> ExplicitComptimeEvaluation {
        evaluate_explicit_comptime_expressions_capture(
            module,
            Arc::new(ReflectionMetadata::new()),
            Arc::new(checked),
            Arc::new(HashMap::new()),
        )
    }

    fn primitive_constants(module: &Module) -> CheckedExpressionTypes {
        let mut checked = CheckedExpressionTypes::default();
        for item in &module.items {
            if let Item::VarDecl(declaration) = item {
                checked
                    .expressions
                    .insert(declaration.name.span, "int64".into());
            }
        }
        checked
    }

    #[test]
    fn required_evaluation_captures_constants_and_expressions_once_without_baked_replay() {
        let module = parse(
            r#"
function observe(label: string) returns int64:
    print(view label)
    println("!", view label)
    return 7
int64 cached = comptime observe("constant")
function main() returns int64:
    int64 first = comptime observe("expression")
    return cached + first
"#,
        );
        let captured = capture(&module, primitive_constants(&module));
        assert!(
            captured.diagnostics.is_empty(),
            "{:?}",
            captured.diagnostics
        );
        assert_eq!(captured.values.len(), 3);
        assert_eq!(
            captured.debug_events,
            vec![
                DebugEvent {
                    kind: DebugEventKind::Print,
                    text: "constant".into()
                },
                DebugEvent {
                    kind: DebugEventKind::Println,
                    text: "! constant\n".into()
                },
                DebugEvent {
                    kind: DebugEventKind::Print,
                    text: "expression".into()
                },
                DebugEvent {
                    kind: DebugEventKind::Println,
                    text: "! expression\n".into()
                },
            ]
        );
        let mut interpreter = Interpreter::new();
        interpreter.enable_stdout_capture();
        interpreter.set_explicit_comptime_values(Arc::new(captured.values));
        interpreter.register_module(&module);
        assert_eq!(
            interpreter.call_function("main", vec![]).unwrap(),
            Value::Int64(14)
        );
        assert!(interpreter.take_debug_events().is_empty());
        assert!(interpreter.take_stdout_output().is_empty());
    }

    #[test]
    fn failed_required_expressions_and_constants_retain_prior_events_in_actual_order() {
        let module = parse(
            r#"
function reject(label: string) returns int64:
    println(view label)
    list[int64] rejected = range(0, 9223372036854775807)
    return 0
function accept() returns int64:
    print("last")
    return 1
int64 failed = comptime reject("constant failure")
function main() returns nothing:
    int64 first = comptime reject("expression failure")
    int64 second = comptime accept()
    return nothing
"#,
        );
        let captured = capture(&module, primitive_constants(&module));
        assert_eq!(captured.diagnostics.len(), 2);
        assert!(
            captured
                .diagnostics
                .iter()
                .all(|d| d.message.contains("range: requested output is too large"))
        );
        assert_eq!(captured.values.len(), 1);
        let failed = module
            .items
            .iter()
            .find_map(|item| match item {
                Item::VarDecl(declaration) if declaration.name.name == "failed" => {
                    Some(declaration)
                }
                _ => None,
            })
            .expect("failed namespace declaration");
        let Expr::Comptime(_, span) = &failed.value else {
            panic!("explicit namespace initializer")
        };
        assert!(captured.values.constant(failed.name.span).is_none());
        assert!(
            captured
                .values
                .get(*span, &ComptimeContext::default())
                .is_none()
        );
        assert_eq!(
            captured.debug_events,
            vec![
                DebugEvent {
                    kind: DebugEventKind::Println,
                    text: "constant failure\n".into()
                },
                DebugEvent {
                    kind: DebugEventKind::Println,
                    text: "expression failure\n".into()
                },
                DebugEvent {
                    kind: DebugEventKind::Print,
                    text: "last".into()
                },
            ]
        );
    }

    #[test]
    fn duplicate_checked_contexts_reuse_only_values_and_do_not_repeat_events() {
        let module = parse(
            r#"
function observe() returns int64:
    println("once")
    return 7
function main() returns int64:
    int64 value = comptime observe()
    return value
"#,
        );
        let Item::Function(function) = &module.items[1] else {
            panic!("main")
        };
        let Stmt::VarDecl(declaration) = &function.body.stmts[0] else {
            panic!("local")
        };
        let Expr::Comptime(_, span) = &declaration.value else {
            panic!("comptime")
        };
        let instance = Arc::new(CheckedFunctionTypes {
            expressions: Arc::new(HashMap::from([(*span, "int64".into())])),
            ..CheckedFunctionTypes::default()
        });
        let mut checked = CheckedExpressionTypes::default();
        checked
            .functions
            .insert(function.name.span, vec![instance.clone(), instance]);
        let captured = capture(&module, checked);
        assert!(
            captured.diagnostics.is_empty(),
            "{:?}",
            captured.diagnostics
        );
        assert_eq!(captured.values.len(), 1);
        assert_eq!(
            captured.debug_events,
            vec![DebugEvent {
                kind: DebugEventKind::Println,
                text: "once\n".into(),
            }]
        );
    }

    #[test]
    fn duplicate_failed_checked_contexts_evaluate_once_without_baked_values() {
        let module = parse(
            r#"
function reject() returns int64:
    println("failed once")
    list[int64] rejected = range(0, 9223372036854775807)
    return 0
function main() returns int64:
    int64 value = comptime reject()
    return value
"#,
        );
        let Item::Function(function) = &module.items[1] else {
            panic!("main")
        };
        let Stmt::VarDecl(declaration) = &function.body.stmts[0] else {
            panic!("local")
        };
        let Expr::Comptime(_, span) = &declaration.value else {
            panic!("comptime")
        };
        let instance = Arc::new(CheckedFunctionTypes {
            expressions: Arc::new(HashMap::from([(*span, "int64".into())])),
            ..CheckedFunctionTypes::default()
        });
        let mut checked = CheckedExpressionTypes::default();
        checked
            .functions
            .insert(function.name.span, vec![instance.clone(), instance]);
        let captured = capture(&module, checked);
        assert_eq!(captured.diagnostics.len(), 1);
        assert_eq!(captured.diagnostics[0].code.code(), 9001);
        assert_eq!(captured.diagnostics[0].span, *span);
        assert_eq!(
            captured.diagnostics[0].message,
            "`comptime` expression must be closed and evaluable during compilation: range: requested output is too large"
        );
        assert!(captured.values.is_empty());
        assert!(
            captured
                .values
                .get(*span, &ComptimeContext::default())
                .is_none()
        );
        assert_eq!(
            captured.debug_events,
            vec![DebugEvent {
                kind: DebugEventKind::Println,
                text: "failed once\n".into(),
            }]
        );
    }
}
