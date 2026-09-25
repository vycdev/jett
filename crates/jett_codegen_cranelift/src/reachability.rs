use jett_common::{SourceOrigin, Span};
use jett_hir::{self as hir, Expression, ExpressionKind, FunctionId};
use jett_mir::{Function, Program, StatementKind, TerminatorKind};

use crate::CodegenError;

/// Find the backend-reachable MIR functions without changing their original
/// `FunctionId` identity. The returned IDs are in original program order so
/// declaration and object-symbol order stay deterministic.
pub(crate) fn reachable_function_ids(program: &Program) -> Result<Vec<FunctionId>, CodegenError> {
    let mut reachable = vec![false; program.functions.len()];
    let mut pending = Vec::new();

    for function in &program.functions {
        if matches!(function.identity.declaration.origin, SourceOrigin::Project)
            && function.identity.declaration.kind != hir::DeclarationKind::RefinementPredicate
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
        let mut references = Vec::new();
        collect_function_references(function, &mut references);
        for (target, span) in references {
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

fn collect_function_references(function: &Function, references: &mut Vec<(FunctionId, Span)>) {
    for block in &function.blocks {
        for statement in &block.statements {
            match &statement.kind {
                StatementKind::Let { value, .. }
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
                StatementKind::IterationBorrow { .. }
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

fn collect_hir_block_references(block: &hir::Block, references: &mut Vec<(FunctionId, Span)>) {
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
fn collect_expression_references(
    expression: &Expression,
    references: &mut Vec<(FunctionId, Span)>,
) {
    match &expression.kind {
        ExpressionKind::FunctionRef(function) => references.push((*function, expression.span)),
        ExpressionKind::ClosureRef { function, .. } => {
            references.push((*function, expression.span));
        }
        ExpressionKind::Call { function, args, .. } => {
            references.push((*function, expression.span));
            for argument in args {
                collect_expression_references(argument, references);
            }
        }
        ExpressionKind::Binary { left, right, .. } => {
            collect_expression_references(left, references);
            collect_expression_references(right, references);
        }
        ExpressionKind::Unary { value, .. }
        | ExpressionKind::ResultOk(value)
        | ExpressionKind::ResultFail(value)
        | ExpressionKind::OptionalSome(value)
        | ExpressionKind::Comptime(value)
        | ExpressionKind::Declassify(value)
        | ExpressionKind::Coarsen(value)
        | ExpressionKind::RefinementValidated(value)
        | ExpressionKind::Run(value)
        | ExpressionKind::Join(value)
        | ExpressionKind::Cancel(value)
        | ExpressionKind::View(value)
        | ExpressionKind::Clone(value) => collect_expression_references(value, references),
        ExpressionKind::Intrinsic {
            intrinsic: _,
            type_arguments: _,
            reflection_arguments: _,
            args,
            evaluation_order: _,
        }
        | ExpressionKind::ActorSpawn { args, .. } => {
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
        ExpressionKind::ActorMessage { actor, args, .. } => {
            collect_expression_references(actor, references);
            for argument in args {
                collect_expression_references(argument, references);
            }
        }
        ExpressionKind::Int(_)
        | ExpressionKind::Float(_)
        | ExpressionKind::String(_)
        | ExpressionKind::Bool(_)
        | ExpressionKind::Nothing
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
        jett_mir::lower(&hir).expect("MIR lowering")
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
}
