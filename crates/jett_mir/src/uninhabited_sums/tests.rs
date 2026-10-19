use super::*;
use jett_common::{FileId, SourceOrigin};
use std::collections::{HashMap, HashSet};

fn lower_source(source: &str) -> (Program, TypeInterner) {
    let file = FileId::new(0);
    let parsed = jett_parser::parse(source, file);
    assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
    let resolved = jett_resolve::resolve(&parsed.module);
    let checked = jett_typecheck::check(&parsed.module, &resolved);
    assert!(
        checked
            .diagnostics
            .iter()
            .all(|diagnostic| diagnostic.severity != jett_diagnostics::Severity::Error),
        "{:?}",
        checked.diagnostics
    );
    let hir = hir::lower(
        &parsed.module,
        &resolved,
        &checked,
        &HashMap::from([(file, SourceOrigin::Project)]),
    )
    .expect("HIR lowering");
    (
        lower(&hir, &checked.interner).expect("MIR lowering"),
        checked.interner,
    )
}

fn named_function<'a>(program: &'a Program, name: &str) -> &'a Function {
    program
        .functions
        .iter()
        .find(|function| function.identity.declaration.name == name)
        .unwrap()
}

fn foreign_type_id(types: &TypeInterner) -> TypeId {
    let mut foreign = TypeInterner::new();
    for index in 0..types.len() + 8 {
        foreign.intern(Type::Refinement {
            name: format!("ForeignUninhabitedTest{index}"),
            base: TypeInterner::INT64,
        });
    }
    foreign.intern(Type::Refinement {
        name: "ForeignUninhabitedTestFinal".into(),
        base: TypeInterner::INT64,
    })
}

fn assert_dense_reachable(function: &Function) {
    let cfg = ControlFlowGraph::analyze(function).unwrap();
    let mut reachable = HashSet::new();
    let mut pending = vec![function.entry];
    while let Some(block) = pending.pop() {
        if reachable.insert(block) {
            pending.extend_from_slice(cfg.successors(block));
        }
    }
    assert_eq!(reachable.len(), function.blocks.len());
    for (index, block) in function.blocks.iter().enumerate() {
        assert_eq!(block.id.index() as usize, index);
    }
    for (index, local) in function.locals.iter().enumerate() {
        assert_eq!(local.id.index() as usize, index);
        assert_ne!(local.ty, TypeInterner::NEVER);
    }
}

fn tags(function: &Function) -> usize {
    function
        .blocks
        .iter()
        .flat_map(|block| &block.statements)
        .filter(|statement| matches!(statement.kind, StatementKind::SumTag { .. }))
        .count()
}

fn takes(function: &Function) -> Vec<bool> {
    function
        .blocks
        .iter()
        .flat_map(|block| &block.statements)
        .filter_map(|statement| match statement.kind {
            StatementKind::SumTake { success, .. } => Some(success),
            _ => None,
        })
        .collect()
}

const OPTIONAL: &str = r#"namespace app
function inspect[T](value: optional[T]) returns nothing:
    T item = value handle:
        return nothing
    trace item
    return nothing
function main() returns nothing:
    inspect(none)
    return nothing
"#;

#[test]
fn optional_never_retains_checked_tag_and_function_metadata_without_extracting() {
    let (mut program, types) = lower_source(OPTIONAL);
    let original = named_function(&program, "inspect").clone();
    assert_eq!(tags(&original), 1);
    assert_eq!(takes(&original), [true]);
    let original_main = named_function(&program, "main").clone();
    prepare_native_uninhabited_sums(&mut program, &types);
    validate(&program).unwrap();
    let function = named_function(&program, "inspect");
    assert_dense_reachable(function);
    assert_eq!(tags(function), 1, "runtime SumHandleTag guard is retained");
    assert!(takes(function).is_empty());
    assert_eq!(function.id, original.id);
    assert_eq!(function.identity, original.identity);
    assert_eq!(function.debug_kind, original.debug_kind);
    assert_eq!(function.params, original.params);
    assert_eq!(function.capture_count, original.capture_count);
    assert_eq!(function.return_type, original.return_type);
    assert_eq!(function.span, original.span);
    assert_eq!(named_function(&program, "main"), &original_main);
    move_values::MoveValuePlan::analyze(&program, function, &types)
        .expect("absent sum snapshot is cleaned up");
    let prepared = program.clone();
    prepare_native_uninhabited_sums(&mut program, &types);
    assert_eq!(program, prepared, "preparation is idempotent");
}

#[test]
fn result_never_selects_either_inhabited_arm_and_keeps_owned_error_extraction() {
    let (mut program, types) = lower_source(
        r#"namespace app
function keep[T, E](outcome: result[T, E]) returns optional[T]:
    T item = outcome handle error:
        trace error
        return none
    return some(item)
function consume[T](value: optional[T]) returns nothing:
    return nothing
function main() returns nothing:
    consume(keep(fail("failure")))
    consume(keep(ok(7)))
    return nothing
"#,
    );
    prepare_native_uninhabited_sums(&mut program, &types);
    validate(&program).unwrap();
    let mut directions = Vec::new();
    for function in program
        .functions
        .iter()
        .filter(|function| function.identity.declaration.name == "keep")
    {
        assert_dense_reachable(function);
        assert_eq!(tags(function), 1);
        let Type::Result(ok, error) = types.resolve(function.params[0].ty) else {
            panic!("checked result parameter");
        };
        let success = *error == TypeInterner::NEVER;
        assert_eq!(*ok == TypeInterner::NEVER, !success);
        assert_eq!(takes(function), [success]);
        let traces = function
            .blocks
            .iter()
            .flat_map(|block| &block.statements)
            .filter(|statement| matches!(statement.kind, StatementKind::Trace(_)))
            .count();
        assert_eq!(traces, usize::from(!success));
        move_values::MoveValuePlan::analyze(&program, function, &types)
            .expect("selected payload and unselected sum owners are cleaned up");
        directions.push(success);
    }
    directions.sort();
    assert_eq!(directions, [false, true]);
}

#[test]
fn nested_uninhabited_handlers_are_selected_in_one_pass_with_prior_effects() {
    let (mut program, types) = lower_source(
        r#"namespace app
function nested[T, E](outer: result[T, E], inner: optional[T]) returns int64:
    T item = outer handle error:
        trace error
        T other = inner handle:
            return 7
        trace other
        return 8
    trace item
    return 9
function main() returns int64:
    return nested(fail("failure"), none)
"#,
    );
    prepare_native_uninhabited_sums(&mut program, &types);
    validate(&program).unwrap();
    let function = named_function(&program, "nested");
    assert_dense_reachable(function);
    assert_eq!(tags(function), 2);
    assert_eq!(takes(function), [false]);
    assert_eq!(
        function
            .blocks
            .iter()
            .flat_map(|block| &block.statements)
            .filter(|statement| matches!(statement.kind, StatementKind::Trace(_)))
            .count(),
        1
    );
    let returns = function
        .blocks
        .iter()
        .filter_map(|block| match &block.terminator.kind {
            TerminatorKind::Return(Some(Expression {
                kind: ExpressionKind::Int(value),
                ..
            })) => Some(*value),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(returns, [7]);
    move_values::MoveValuePlan::analyze(&program, function, &types)
        .expect("nested failure exits clean both snapshots");
}

#[test]
fn pending_sum_evaluation_is_retained_once_before_the_unchanged_runtime_guard() {
    fn pending_input_shape(function: &Function) -> Vec<(TypeId, Span)> {
        assert_eq!(tags(function), 1);
        let block = function
            .blocks
            .iter()
            .find(|block| {
                matches!(
                    block.statements.last().map(|statement| &statement.kind),
                    Some(StatementKind::SumTag { .. })
                )
            })
            .unwrap();
        let StatementKind::SumTag { source, .. } = block.statements.last().unwrap().kind else {
            unreachable!();
        };
        let definitions = block
            .statements
            .iter()
            .filter_map(|statement| match &statement.kind {
                StatementKind::Let { local, value } if *local == source => Some(value),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(
            definitions.len(),
            1,
            "sum input is evaluated once before its tag"
        );
        let outer = definitions[0];
        let ExpressionKind::Run(inner) = &outer.kind else {
            panic!("sum input must retain its outer pending layer: {outer:?}");
        };
        let ExpressionKind::Run(cloned) = &inner.kind else {
            panic!("sum input must retain its second pending layer: {inner:?}");
        };
        let ExpressionKind::Clone(input) = &cloned.kind else {
            panic!("pending sum must own a separate input snapshot: {cloned:?}");
        };
        assert!(
            matches!(input.kind, ExpressionKind::Local(local) if local == function.params[0].local)
        );
        assert_eq!(
            function
                .blocks
                .iter()
                .flat_map(|block| &block.statements)
                .filter(|statement| matches!(&statement.kind, StatementKind::Let { value, .. } if matches!(value.kind, ExpressionKind::Run(_))))
                .count(),
            1,
            "the depth-two input is not evaluated again elsewhere"
        );
        [outer, inner.as_ref(), cloned.as_ref(), input.as_ref()]
            .map(|expression| (expression.ty, expression.span))
            .to_vec()
    }

    let (mut program, types) = lower_source(
        r#"namespace app
function inspect[T](value: optional[T]) returns nothing:
    T item = (run run clone value) handle:
        return nothing
    trace item
    return nothing
function main() returns nothing:
    inspect(none)
    return nothing
"#,
    );
    let original = named_function(&program, "inspect");
    let input_shape = pending_input_shape(original);
    prepare_native_uninhabited_sums(&mut program, &types);
    validate(&program).unwrap();
    let function = named_function(&program, "inspect");
    assert_dense_reachable(function);
    assert_eq!(pending_input_shape(function), input_shape);
    let guards = function
        .blocks
        .iter()
        .filter_map(|block| {
            let StatementKind::SumTag { .. } = &block.statements.last()?.kind else {
                return None;
            };
            assert!(matches!(block.terminator.kind, TerminatorKind::Goto(_)));
            Some(())
        })
        .count();
    assert_eq!(guards, 1);
    assert!(takes(function).is_empty());
    move_values::MoveValuePlan::analyze(&program, function, &types)
        .expect("pending guard owns a separate snapshot until failure cleanup");
}

#[test]
fn owned_and_borrowed_local_sums_are_snapshotted_before_reuse() {
    for borrowed in [false, true] {
        let mode = if borrowed { "view " } else { "" };
        let source = format!(
            r#"namespace app
function inspect[T]({mode}value: optional[T]) returns nothing:
    T item = value handle:
        trace value
        return nothing
    trace value
    trace item
    return nothing
function failed[T, E]({mode}outcome: result[T, E]) returns optional[T]:
    T item = outcome handle error:
        trace outcome
        trace error
        return none
    trace outcome
    return some(item)
function succeeded[T, E]({mode}outcome: result[T, E], fallback: T) returns T:
    T item = outcome handle error:
        trace outcome
        default fallback
    trace outcome
    return item
function consume[T](value: optional[T]) returns nothing:
    return nothing
function main() returns nothing:
    inspect({mode}none)
    consume(failed({mode}fail("failure")))
    succeeded({mode}ok(7), 0)
    return nothing
"#
        );
        let (mut program, types) = lower_source(&source);
        prepare_native_uninhabited_sums(&mut program, &types);
        validate(&program).unwrap();
        for name in ["inspect", "failed", "succeeded"] {
            let function = named_function(&program, name);
            assert_dense_reachable(function);
            let owner = function.params[0].local;
            assert_eq!(function.params[0].mode == ParamMode::View, borrowed);
            let snapshots = function
                .blocks
                .iter()
                .flat_map(|block| &block.statements)
                .filter(|statement| matches!(&statement.kind, StatementKind::Let { value: Expression { kind: ExpressionKind::Clone(inner), .. }, .. } if matches!(inner.kind, ExpressionKind::Local(local) if local == owner)))
                .count();
            assert_eq!(snapshots, 1, "{name} retains a separate owned snapshot");
            assert!(function.blocks.iter().flat_map(|block| &block.statements).any(|statement| matches!(statement.kind, StatementKind::Trace(local) if local == owner)));
            move_values::MoveValuePlan::analyze(&program, function, &types)
                .expect("source sum remains readable after selected handler path");
        }
    }
}

#[test]
fn source_or_tag_writes_and_mismatched_typed_metadata_are_not_reinterpreted() {
    for mutation in 0..9 {
        let (mut program, mut types) = lower_source(OPTIONAL);
        let invalid_type = foreign_type_id(&types);
        let function_id = named_function(&program, "inspect").id;
        let function = &mut program.functions[function_id.index() as usize];
        let block = function
            .blocks
            .iter_mut()
            .find(|block| {
                matches!(
                    block.statements.last().map(|statement| &statement.kind),
                    Some(StatementKind::SumTag { .. })
                )
            })
            .unwrap();
        let StatementKind::SumTag { source, target } = block.statements.last().unwrap().kind else {
            unreachable!();
        };
        let TerminatorKind::Branch { condition, .. } = &mut block.terminator.kind else {
            unreachable!();
        };
        match mutation {
            0 | 1 => {
                let (local, ty, kind) = if mutation == 0 {
                    (
                        source,
                        function.locals[source.index() as usize].ty,
                        ExpressionKind::OptionalNone,
                    )
                } else {
                    (target, TypeInterner::BOOL, ExpressionKind::Bool(false))
                };
                block.statements.push(Statement {
                    kind: StatementKind::Assign {
                        target: Expression {
                            kind: ExpressionKind::Local(local),
                            ty,
                            span: function.span,
                        },
                        value: Expression {
                            kind,
                            ty,
                            span: function.span,
                        },
                    },
                    span: function.span,
                });
            }
            2 => condition.kind = ExpressionKind::Local(function.params[0].local),
            3 => condition.ty = TypeInterner::INT64,
            4 => function.locals[target.index() as usize].ty = TypeInterner::INT64,
            5 => function.locals[source.index() as usize].ty = invalid_type,
            6 => {
                function.locals[source.index() as usize].ty =
                    types.intern(Type::Result(TypeInterner::NEVER, TypeInterner::NEVER))
            }
            7 => {
                function.locals[source.index() as usize].ty =
                    types.intern(Type::Secret(function.locals[source.index() as usize].ty))
            }
            _ => {
                function.locals[source.index() as usize].ty =
                    types.intern(Type::Result(TypeInterner::NEVER, invalid_type))
            }
        }
        validate(&program).unwrap();
        let original = program.clone();
        prepare_native_uninhabited_sums(&mut program, &types);
        assert_eq!(program, original, "conservative mutation {mutation}");
    }
}

#[test]
fn malformed_live_or_dead_ids_are_reported_before_sum_compaction() {
    for corruption in 0..4 {
        let (mut program, types) = lower_source(OPTIONAL);
        let function_id = named_function(&program, "inspect").id;
        let function = &mut program.functions[function_id.index() as usize];
        let block = function
            .blocks
            .iter_mut()
            .find(|block| {
                matches!(
                    block.statements.last().map(|statement| &statement.kind),
                    Some(StatementKind::SumTag { .. })
                )
            })
            .unwrap();
        let StatementKind::SumTag { source, target } =
            &mut block.statements.last_mut().unwrap().kind
        else {
            unreachable!();
        };
        let TerminatorKind::Branch { then_block, .. } = &mut block.terminator.kind else {
            unreachable!();
        };
        match corruption {
            0 => *source = LocalId::new(u32::MAX),
            1 => *target = LocalId::new(u32::MAX),
            2 => *then_block = BlockId(u32::MAX),
            _ => {
                let dead = *then_block;
                function.blocks[dead.index() as usize]
                    .statements
                    .push(Statement {
                        kind: StatementKind::Trace(LocalId::new(u32::MAX)),
                        span: function.span,
                    });
            }
        }
        let original = program.clone();
        assert!(validate(&program).is_err());
        prepare_native_uninhabited_sums(&mut program, &types);
        assert_eq!(program, original);
        assert!(validate(&program).is_err());
    }
}
