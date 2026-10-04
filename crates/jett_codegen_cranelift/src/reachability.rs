use jett_common::{SourceOrigin, Span};
use jett_hir::{self as hir, Expression, ExpressionKind, FunctionId};
use jett_mir::{Function, Program, StatementKind, TerminatorKind};

use crate::CodegenError;

/// Find the backend-reachable MIR functions without changing their original
/// `FunctionId` identity. The returned IDs are in original program order so
/// declaration and object-symbol order stay deterministic.
#[cfg(test)]
fn reachable_function_ids(program: &Program) -> Result<Vec<FunctionId>, CodegenError> {
    reachable_functions(program, None)
}

pub(crate) fn reachable_function_ids_with_types(
    program: &Program,
    types: &jett_types::TypeInterner,
) -> Result<Vec<FunctionId>, CodegenError> {
    reachable_functions(program, Some(types))
}

struct References<'a> {
    functions: Vec<(FunctionId, Span)>,
    program: &'a Program,
    types: Option<&'a jett_types::TypeInterner>,
}
impl References<'_> {
    fn push(&mut self, reference: (FunctionId, Span)) {
        self.functions.push(reference);
    }
}

fn reachable_functions(
    program: &Program,
    types: Option<&jett_types::TypeInterner>,
) -> Result<Vec<FunctionId>, CodegenError> {
    let mut reachable = vec![false; program.functions.len()];
    let mut pending = Vec::new();

    for function in &program.functions {
        if matches!(function.identity.declaration.origin, SourceOrigin::Project)
            && function.identity.declaration.kind != hir::DeclarationKind::RefinementPredicate
            && function.debug_kind != hir::FunctionDebugKind::Inline
            && !uninhabited_specialization(function)
        {
            mark_reachable(
                program,
                function.id,
                function.span,
                function,
                &mut reachable,
                &mut pending,
            )?;
        }
    }

    while let Some((function_id, function_span)) = pending.pop() {
        let function = resolve_function(program, function_id)
            .ok_or_else(|| invalid_target(program.functions.first(), function_span, function_id))?;
        let mut references = References {
            functions: Vec::new(),
            program,
            types,
        };
        // Retain its identity for descriptor storage, but no valid call can
        // enter this original body or make its callees executable roots.
        if !crate::verify::descriptor_only_function(function) {
            collect_function_references(function, &mut references);
        }
        for (target, span) in references.functions {
            mark_reachable(
                program,
                target,
                span,
                function,
                &mut reachable,
                &mut pending,
            )?;
        }
    }

    Ok(program
        .functions
        .iter()
        .enumerate()
        .filter_map(|(index, function)| reachable[index].then_some(function.id))
        .collect())
}

// Checking an unreachable empty-collection body can instantiate a project
// generic with a bare `never` parameter or result. Such an eager instantiation
// is not an independent callable root: no value can inhabit that signature.
// A retained call or function reference still reaches it and must pass the
// ordinary verifier. Containers and absent sum arms around `never` are inhabited.
fn uninhabited_specialization(function: &Function) -> bool {
    !function.identity.type_arguments.is_empty()
        && (function.return_type == jett_types::TypeInterner::NEVER
            || function
                .params
                .iter()
                .any(|param| param.ty == jett_types::TypeInterner::NEVER))
}

fn mark_reachable(
    program: &Program,
    target: FunctionId,
    span: Span,
    referring_function: &Function,
    reachable: &mut [bool],
    pending: &mut Vec<(FunctionId, Span)>,
) -> Result<(), CodegenError> {
    let Some(target_function) = resolve_function(program, target) else {
        return Err(invalid_target(Some(referring_function), span, target));
    };
    let index = usize::try_from(target.index())
        .map_err(|_| invalid_target(Some(referring_function), span, target))?;
    if !reachable[index] {
        reachable[index] = true;
        pending.push((target_function.id, target_function.span));
    }
    Ok(())
}

fn resolve_function(program: &Program, id: FunctionId) -> Option<&Function> {
    let index = usize::try_from(id.index()).ok()?;
    program
        .functions
        .get(index)
        .filter(|function| function.id == id)
}

fn invalid_target(
    referring_function: Option<&Function>,
    span: Span,
    target: FunctionId,
) -> CodegenError {
    let function = referring_function
        .map(function_label)
        .unwrap_or_else(|| "<invalid-function>".to_string());
    CodegenError::InvalidMirContract {
        function,
        span,
        message: format!(
            "reached function target {} is absent from the original MIR function table",
            target.index()
        ),
    }
}

fn function_label(function: &Function) -> String {
    let declaration = &function.identity.declaration;
    format!("{}::{}", declaration.namespace, declaration.name)
}

fn collect_function_references(function: &Function, references: &mut References<'_>) {
    for block in &function.blocks {
        for statement in &block.statements {
            match &statement.kind {
                StatementKind::Let { value, .. }
                | StatementKind::BeginCallView { value, .. }
                | StatementKind::CheckRefinement { call: value, .. }
                | StatementKind::Evaluate(value)
                | StatementKind::HandleDefault(value) => {
                    collect_expression_references(value, references);
                }
                StatementKind::Assign { target, value } => {
                    collect_expression_references(target, references);
                    collect_expression_references(value, references);
                }
                StatementKind::Assert { condition, message } => {
                    collect_expression_references(condition, references);
                    if let Some(message) = message {
                        collect_expression_references(message, references);
                    }
                }
                StatementKind::Breakpoint { condition, .. } => {
                    if let Some(condition) = condition {
                        collect_expression_references(condition, references);
                    }
                }
                // Generation operations use current Local operands only;
                // their immutable Source archives are not callable roots.
                StatementKind::OpenCallOwnerGeneration { .. }
                | StatementKind::ReplaceCallOwnerGeneration { .. }
                | StatementKind::CloseCallOwnerGeneration { .. }
                | StatementKind::EndCallView { .. }
                | StatementKind::ReflectedContainerReady { .. }
                | StatementKind::IterationBorrow { .. }
                | StatementKind::SequenceLength { .. }
                | StatementKind::SequenceGet { .. }
                | StatementKind::SumTag { .. }
                | StatementKind::SumTake { .. }
                | StatementKind::Trace(_) => {}
            }
        }
        match &block.terminator.kind {
            TerminatorKind::Return(value) => {
                if let Some(value) = value {
                    collect_expression_references(value, references);
                }
            }
            TerminatorKind::Respond(value) => collect_expression_references(value, references),
            TerminatorKind::Branch { condition, .. } => {
                collect_expression_references(condition, references);
            }
            TerminatorKind::Switch { scrutinee, .. } => {
                collect_expression_references(scrutinee, references);
            }
            TerminatorKind::ForEach { iterable, .. } => {
                collect_expression_references(iterable, references);
            }
            TerminatorKind::ReflectedTypeDispatch { type_info, .. } => {
                collect_expression_references(type_info, references);
            }
            TerminatorKind::Goto(_) | TerminatorKind::Unreachable => {}
        }
    }
}

fn collect_hir_block_references(block: &hir::Block, references: &mut References<'_>) {
    for statement in &block.statements {
        match &statement.kind {
            hir::StatementKind::Let { value, .. }
            | hir::StatementKind::HandleDefault(value)
            | hir::StatementKind::Expression(value)
            | hir::StatementKind::Respond(value) => {
                collect_expression_references(value, references);
            }
            hir::StatementKind::Assign { target, value } => {
                collect_expression_references(target, references);
                collect_expression_references(value, references);
            }
            hir::StatementKind::Return(value) => {
                if let Some(value) = value {
                    collect_expression_references(value, references);
                }
            }
            hir::StatementKind::If {
                condition,
                then_block,
                else_block,
            } => {
                collect_expression_references(condition, references);
                collect_hir_block_references(then_block, references);
                if let Some(else_block) = else_block {
                    collect_hir_block_references(else_block, references);
                }
            }
            hir::StatementKind::While { condition, body } => {
                collect_expression_references(condition, references);
                collect_hir_block_references(body, references);
            }
            hir::StatementKind::For { iterable, body, .. } => {
                collect_expression_references(iterable, references);
                collect_hir_block_references(body, references);
            }
            hir::StatementKind::Match { scrutinee, arms } => {
                collect_expression_references(scrutinee, references);
                for arm in arms {
                    collect_hir_block_references(&arm.body, references);
                }
            }
            hir::StatementKind::Assert { condition, message } => {
                collect_expression_references(condition, references);
                if let Some(message) = message {
                    collect_expression_references(message, references);
                }
            }
            hir::StatementKind::Breakpoint { condition, .. } => {
                if let Some(condition) = condition {
                    collect_expression_references(condition, references);
                }
            }
            hir::StatementKind::Scope(block) => {
                collect_hir_block_references(block, references);
            }
            hir::StatementKind::ReflectedTypeDispatch { type_info, arms } => {
                collect_expression_references(type_info, references);
                for arm in arms {
                    collect_hir_block_references(&arm.body, references);
                }
            }
            hir::StatementKind::Break
            | hir::StatementKind::Continue
            | hir::StatementKind::Trace(_) => {}
        }
    }
}

/// Deliberately match every expression variant without a wildcard. Adding a
/// new MIR-carried HIR expression cannot compile until reachability decides how
/// to traverse it.
fn collect_expression_references(expression: &Expression, references: &mut References<'_>) {
    match &expression.kind {
        ExpressionKind::ResourceHookValue { .. } => {}
        ExpressionKind::ResourceInvoke { args, .. } => {
            for argument in args {
                collect_expression_references(argument, references);
            }
        }
        ExpressionKind::FunctionRef(function) => references.push((*function, expression.span)),
        ExpressionKind::FunctionAdapter { value, function } => {
            references.push((*function, expression.span));
            collect_expression_references(value, references);
        }
        ExpressionKind::ClosureRef { function, .. } => {
            references.push((*function, expression.span));
        }
        ExpressionKind::Call { function, args, .. } => {
            references.push((*function, expression.span));
            for argument in args {
                collect_expression_references(argument, references);
            }
        }
        ExpressionKind::Binary { left, op, right } => {
            if matches!(op, hir::BinaryOp::Equal | hir::BinaryOp::NotEqual) {
                if let Some(types) = references.types {
                    if let Some(layout) = crate::emit::debug::equality_layout(
                        types,
                        left.ty,
                        &references.program.equality_methods,
                    ) {
                        for (_, method) in layout.custom {
                            references.push((method, expression.span));
                        }
                    }
                }
            }
            collect_expression_references(left, references);
            collect_expression_references(right, references);
        }
        ExpressionKind::Unary { value, .. }
        | ExpressionKind::ResultOk(value)
        | ExpressionKind::ResultFail(value)
        | ExpressionKind::OptionalSome(value)
        | ExpressionKind::Comptime { value, .. }
        | ExpressionKind::DisplayResult(value)
        | ExpressionKind::EquatableResult(value)
        | ExpressionKind::RuntimeFailureMessage(value)
        | ExpressionKind::Declassify(value)
        | ExpressionKind::Coarsen(value)
        | ExpressionKind::RefinementValidated(value)
        | ExpressionKind::InterfaceType(value)
        | ExpressionKind::Run(value)
        | ExpressionKind::Join(value)
        | ExpressionKind::Cancel(value)
        | ExpressionKind::View(value)
        | ExpressionKind::Clone(value) => collect_expression_references(value, references),
        ExpressionKind::InterfaceCoerce { value, adapters } => {
            collect_expression_references(value, references);
            references.functions.extend(
                adapters
                    .iter()
                    .map(|entry| (entry.function, expression.span)),
            );
        }
        ExpressionKind::Intrinsic {
            intrinsic: _,
            type_arguments: _,
            reflection_arguments: _,
            refinement_predicates: _,
            field_validation: _,
            args,
            evaluation_order: _,
            ..
        } => {
            for argument in args {
                collect_expression_references(argument, references);
            }
        }
        ExpressionKind::ActorSpawn {
            args, constructor, ..
        } => {
            if let Some(constructor) = constructor {
                references.push((*constructor, expression.span));
            }
            for argument in args {
                collect_expression_references(argument, references);
            }
        }
        ExpressionKind::IndirectCall { callee, args, .. } => {
            collect_expression_references(callee, references);
            for argument in args {
                collect_expression_references(argument, references);
            }
        }
        ExpressionKind::StructConstruct { fields, .. }
        | ExpressionKind::BitfieldConstruct { fields, .. } => {
            for field in fields {
                collect_expression_references(field, references);
            }
        }
        ExpressionKind::MachineConstruct { payloads, .. }
        | ExpressionKind::EnumConstruct { payloads, .. } => {
            for payload in payloads {
                collect_expression_references(payload, references);
            }
        }
        ExpressionKind::MachineTransition {
            source, payloads, ..
        } => {
            collect_expression_references(source, references);
            for payload in payloads {
                collect_expression_references(payload, references);
            }
        }
        ExpressionKind::ListConstruct { elements } => {
            for element in elements {
                collect_expression_references(element, references);
            }
        }
        ExpressionKind::MapConstruct { entries } => {
            for entry in entries {
                collect_expression_references(&entry.key, references);
                collect_expression_references(&entry.value, references);
            }
        }
        ExpressionKind::Handle {
            target, failure, ..
        } => {
            collect_expression_references(target, references);
            collect_hir_block_references(failure, references);
        }
        ExpressionKind::StringInterpolation(parts) => {
            for part in parts {
                if let hir::StringSegment::Value(value) = part {
                    collect_expression_references(value, references);
                }
            }
        }
        ExpressionKind::StateIs { value, .. } | ExpressionKind::Field { base: value, .. } => {
            collect_expression_references(value, references);
        }
        ExpressionKind::InlineFunction { body, .. } => {
            collect_hir_block_references(body, references);
        }
        ExpressionKind::ActorMessage {
            actor,
            args,
            handler,
            ..
        } => {
            references.push((*handler, expression.span));
            collect_expression_references(actor, references);
            for argument in args {
                collect_expression_references(argument, references);
            }
        }
        ExpressionKind::Int(_)
        | ExpressionKind::Constant { .. }
        | ExpressionKind::Float(_)
        | ExpressionKind::String(_)
        | ExpressionKind::Bool(_)
        | ExpressionKind::Nothing
        | ExpressionKind::PropertyCaseContext(_)
        | ExpressionKind::RuntimeFailure(_)
        | ExpressionKind::Local(_)
        | ExpressionKind::OptionalNone => {}
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use jett_common::FileId;
    use jett_mir::Program;

    use super::*;

    fn lower_source(source: &str) -> Program {
        lower_source_with_types(source).0
    }

    fn lower_source_with_types(source: &str) -> (Program, jett_types::TypeInterner) {
        let file = FileId::new(0);
        let parsed = jett_parser::parse(source, file);
        assert!(
            parsed.errors.is_empty(),
            "parse errors: {:?}",
            parsed.errors
        );
        let resolved = jett_resolve::resolve(&parsed.module);
        let checked = jett_typecheck::check(&parsed.module, &resolved);
        assert!(
            checked
                .diagnostics
                .iter()
                .all(|diagnostic| diagnostic.severity != jett_diagnostics::Severity::Error),
            "check errors: {:?}",
            checked.diagnostics
        );
        let hir = jett_hir::lower(
            &parsed.module,
            &resolved,
            &checked,
            &HashMap::from([(file, SourceOrigin::Project)]),
        )
        .expect("HIR lowering");
        let mir = jett_mir::lower(&hir, &checked.interner).expect("MIR lowering");
        (mir, checked.interner)
    }

    fn set_origin(program: &mut Program, name: &str, origin: SourceOrigin) {
        program
            .functions
            .iter_mut()
            .find(|function| function.identity.declaration.name == name)
            .unwrap_or_else(|| panic!("missing test function `{name}`"))
            .identity
            .declaration
            .origin = origin;
    }

    fn reachable_names(program: &Program) -> Vec<&str> {
        reachable_function_ids(program)
            .expect("reachable functions")
            .into_iter()
            .map(|id| {
                resolve_function(program, id)
                    .expect("reachable function exists")
                    .identity
                    .declaration
                    .name
                    .as_str()
            })
            .collect()
    }

    #[test]
    fn direct_calls_and_function_refs_reach_stdlib_and_dependency_functions() {
        let mut program = lower_source(
            r#"namespace app
function dependency_leaf(value: int64) returns int64:
    return value
function stdlib_factory() returns function(int64) returns int64:
    return dependency_leaf
function root() returns function(int64) returns int64:
    return stdlib_factory()
"#,
        );
        set_origin(
            &mut program,
            "dependency_leaf",
            SourceOrigin::Dependency("dep.math".to_string()),
        );
        set_origin(&mut program, "stdlib_factory", SourceOrigin::Stdlib);

        assert_eq!(
            reachable_names(&program),
            ["dependency_leaf", "stdlib_factory", "root"]
        );
    }

    #[test]
    fn exhaustive_visitors_follow_calls_inside_inline_hir_bodies() {
        let mut program = lower_source(
            r#"namespace app
function stdlib_leaf() returns int64:
    return 1
function root(seed: int64) returns function(bool) returns int64:
    return function(flag: bool) returns int64:
        if flag:
            return stdlib_leaf() + seed
        return 0
"#,
        );
        set_origin(&mut program, "stdlib_leaf", SourceOrigin::Stdlib);

        // The production visitors use wildcard-free matches for every current
        // MIR statement/terminator and MIR-carried HIR statement/expression.
        // This exercises the extra nested HIR body that a shallow MIR walk
        // would miss; adding an enum variant also fails those matches to build.
        let names = reachable_names(&program);
        assert_eq!(names[0..2], ["stdlib_leaf", "root"]);
        assert!(names[2].starts_with("root$inline"));
    }

    #[test]
    fn capture_free_inline_function_reaches_its_extracted_body() {
        let mut program = lower_source(
            r#"namespace app
function stdlib_leaf() returns int64:
    return 1
function root() returns function(bool) returns int64:
    return function(flag: bool) returns int64:
        if flag:
            return stdlib_leaf()
        return 0
"#,
        );
        set_origin(&mut program, "stdlib_leaf", SourceOrigin::Stdlib);

        let names = reachable_names(&program);
        assert_eq!(&names[..2], ["stdlib_leaf", "root"]);
        assert_eq!(names.len(), 3);
        assert!(names[2].starts_with("root$inline"));
    }

    #[test]
    fn empty_loop_only_uninhabited_specializations_are_not_implicit_roots() {
        let program = lower_source(
            r#"namespace app
function pass[T](value: T) returns T:
    return value
function identity[T](value: T) returns T:
    return pass[T](value)
function main() returns nothing:
    for value in list():
        println(identity(value))
"#,
        );
        let dead = program
            .functions
            .iter()
            .filter(|function| uninhabited_specialization(function))
            .collect::<Vec<_>>();
        assert_eq!(dead.len(), 2, "eager checked specializations remain in MIR");
        for function in dead {
            assert_eq!(
                function.identity.type_arguments,
                [jett_types::TypeInterner::NEVER]
            );
        }
        assert_eq!(reachable_names(&program), ["main"]);
    }

    #[test]
    fn descriptor_only_entry_does_not_make_original_body_callees_callable() {
        let (mut program, types) = lower_source_with_types(
            r#"namespace app
function source_leaf() returns int64:
    return 99
function consume_value[T](value: T) returns int64:
    return 0
function stored[T](values: list[T]) returns list[function(T) returns int64]:
    function(T) returns int64 callback = function(ignored: T) returns int64: return source_leaf()
    return list(callback)
function root() returns int64:
    return consume_value(stored(list()))
"#,
        );
        set_origin(&mut program, "source_leaf", SourceOrigin::Stdlib);
        let descriptor = program
            .functions
            .iter()
            .find(|function| crate::verify::descriptor_only_function(function))
            .expect("Never-input descriptor identity");
        let descriptor_id = descriptor.id;
        let reachable = reachable_function_ids_with_types(&program, &types).unwrap();
        assert!(reachable.contains(&descriptor_id));
        let source_leaf = program
            .functions
            .iter()
            .find(|function| function.identity.declaration.name == "source_leaf")
            .unwrap();
        assert!(!reachable.contains(&source_leaf.id));
        let object = crate::emit_host_object(&program, &types)
            .expect("metadata validates original call without emitting it");
        let leaf_symbol = crate::symbol_name(&source_leaf.identity, &types).unwrap();
        assert!(!object.symbols.contains(&leaf_symbol));
    }

    #[test]
    fn retained_calls_and_references_still_reach_uninhabited_specializations() {
        for (case, source) in [
            r#"namespace app
function identity(value: int64) returns int64:
    return value
function root() returns int64:
    return identity(1)
"#,
            r#"namespace app
function identity(value: int64) returns int64:
    return value
function root() returns function(int64) returns int64:
    return identity
"#,
        ]
        .into_iter()
        .enumerate()
        {
            let (mut program, types) = lower_source_with_types(source);
            let callee = program
                .functions
                .iter_mut()
                .find(|function| function.identity.declaration.name == "identity")
                .unwrap();
            callee.identity.type_arguments = vec![jett_types::TypeInterner::NEVER];
            callee.params[0].ty = jett_types::TypeInterner::NEVER;
            let parameter = callee.params[0].local.index() as usize;
            callee.locals[parameter].ty = jett_types::TypeInterner::NEVER;
            callee.locals[parameter].debug_ty = jett_types::TypeInterner::NEVER;
            assert!(uninhabited_specialization(callee));
            assert_eq!(reachable_names(&program), ["identity", "root"]);
            // Reachability keeps both identities, but an earlier source
            // packet rejection is independent of the native callable gate.
            let result = crate::emit_host_object(&program, &types);
            if case == 0 {
                assert!(matches!(result, Err(CodegenError::InvalidMir(errors))
                    if errors.len() == 1 && errors[0].message
                        == "source function ownership signature differs from the exact HIR declaration"));
            } else {
                // Preserve the original reference-only refusal until its
                // actual validation path is characterized separately.
                assert!(matches!(
                    result,
                    Err(CodegenError::InvalidMirContract { .. })
                ));
            }
            let root = program
                .functions
                .iter()
                .find(|function| function.identity.declaration.name == "root")
                .expect("original checked root");
            let expression = root
                .blocks
                .iter()
                .find_map(|block| match &block.terminator.kind {
                    TerminatorKind::Return(Some(value)) => Some(value),
                    _ => None,
                })
                .expect("retained original call or reference");
            let expected = if case == 0 {
                "callable direct edge requires an uninhabited parameter"
            } else {
                "function value signature does not match target"
            };
            assert!(matches!(
                crate::verify::verify_callable_expression_for_test(&program, &types, root, expression),
                Err(CodegenError::InvalidMirContract { message, .. }) if message == expected
            ));
        }
    }

    #[test]
    fn never_container_specializations_remain_inhabited_project_roots() {
        let program = lower_source(
            r#"namespace app
function pass[T](values: list[T]) returns list[T]:
    return values
function main() returns nothing:
    for value in pass(list()):
        println(value)
"#,
        );
        let callee = program
            .functions
            .iter()
            .find(|function| function.identity.declaration.name == "pass")
            .unwrap();
        assert_eq!(
            callee.identity.type_arguments,
            [jett_types::TypeInterner::NEVER]
        );
        assert!(!uninhabited_specialization(callee));
        assert_eq!(reachable_names(&program), ["main", "pass"]);
        let mut plain = callee.clone();
        plain.identity.type_arguments.clear();
        plain.params[0].ty = jett_types::TypeInterner::NEVER;
        plain.return_type = jett_types::TypeInterner::NEVER;
        assert!(!uninhabited_specialization(&plain));
    }

    #[test]
    fn generated_inline_functions_require_a_retained_descriptor_reference() {
        let (mut program, types) = lower_source_with_types(
            r#"namespace app
function root() returns int64:
    function() returns int64 callback = function() returns int64: return 1
    return 7
"#,
        );
        assert_eq!(reachable_names(&program).len(), 2);
        let root = &mut program.functions[0];
        assert!(matches!(
            root.blocks[0].statements[0].kind,
            StatementKind::Let { .. }
        ));
        // Model the descriptor being removed with an impossible handler arm.
        // The generated function retains its identity and table position.
        root.blocks[0].statements.remove(0);
        assert_eq!(program.functions.len(), 2);
        assert_eq!(
            program.functions[1].debug_kind,
            hir::FunctionDebugKind::Inline
        );
        assert_eq!(reachable_names(&program), ["root"]);
        let object = crate::emit_host_object(&program, &types).unwrap();
        assert_eq!(object.symbols.len(), 1);
    }

    #[test]
    fn retained_inline_capture_with_never_metadata_still_fails_verification() {
        let (mut program, types) = lower_source_with_types(
            r#"namespace app
function root(seed: int64) returns function() returns int64:
    return function() returns int64: return seed
"#,
        );
        let inline = program
            .functions
            .iter_mut()
            .find(|function| function.debug_kind == hir::FunctionDebugKind::Inline)
            .unwrap();
        assert_eq!(inline.capture_count, 1);
        inline.params[0].ty = jett_types::TypeInterner::NEVER;
        let capture = inline.params[0].local.index() as usize;
        inline.locals[capture].ty = jett_types::TypeInterner::NEVER;
        inline.locals[capture].debug_ty = jett_types::TypeInterner::NEVER;
        assert_eq!(reachable_names(&program).len(), 2);
        assert!(matches!(
            crate::emit_host_object(&program, &types),
            Err(CodegenError::InvalidMirContract { message, .. })
                if message == "closure capture type does not match target"
        ));
    }
}
