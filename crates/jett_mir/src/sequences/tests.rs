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
    let hir = jett_hir::lower(
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

fn main_function(program: &Program) -> &Function {
    program
        .functions
        .iter()
        .find(|function| function.identity.declaration.name == "main")
        .unwrap()
}

fn assert_reachable_and_dense(function: &Function) {
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
    }
}

#[test]
fn uninhabited_iteration_remaps_transitive_local_view_origins() {
    let (mut program, types) = lower_source(
        r#"namespace app
function main() returns int64:
    for element in list():
        trace element
    list[int64] source = list(7, 11)
    list[int64] borrowed = view source
    list[int64] forwarded = borrowed
    trace forwarded
    return 7
"#,
    );
    let original = main_function(&program);
    let source_before = original
        .locals
        .iter()
        .find(|local| local.name == "source")
        .unwrap()
        .id;
    prepare_native_sequences(&mut program, &types);
    validate(&program).unwrap();
    let function = main_function(&program);
    assert_reachable_and_dense(function);
    let source = function
        .locals
        .iter()
        .find(|local| local.name == "source")
        .unwrap();
    let borrowed = function
        .locals
        .iter()
        .find(|local| local.name == "borrowed")
        .unwrap();
    let forwarded = function
        .locals
        .iter()
        .find(|local| local.name == "forwarded")
        .unwrap();
    assert!(source.id.index() < source_before.index());
    assert_eq!(borrowed.view_source, Some(source.id));
    assert_eq!(forwarded.view_source, Some(borrowed.id));
    assert_eq!(function.view_root(forwarded.id), Some(source.id));
    let plan = crate::move_values::MoveValuePlan::analyze(&program, function, &types).unwrap();
    assert!(plan.owned_locals.contains(&(source.id.index() as usize)));
    assert!(!plan.owned_locals.contains(&(borrowed.id.index() as usize)));
    assert!(!plan.owned_locals.contains(&(forwarded.id.index() as usize)));
}

const EMPTY_LOOP: &str = r#"namespace app
function main(flag: bool) returns int64:
    int64 before = 10
    for value in list():
        trace value
    int64 after = 30
    if flag:
        return after
    return before
"#;

#[test]
fn uninhabited_iteration_compacts_dead_blocks_and_locals_with_debug_metadata() {
    let (mut program, types) = lower_source(EMPTY_LOOP);
    let original = main_function(&program).clone();
    let dead_binding = original
        .locals
        .iter()
        .find(|local| local.name == "value")
        .unwrap()
        .id;
    let function = &mut program.functions[original.id.index() as usize];
    let body = function
        .blocks
        .iter()
        .find_map(|block| match block.terminator.kind {
            TerminatorKind::ForEach { body, .. } => Some(body),
            _ => None,
        })
        .unwrap();
    // Pin pruning independently of HIR's omission of checked dead source bodies.
    function.blocks[body.index() as usize]
        .statements
        .push(Statement {
            kind: StatementKind::Trace(dead_binding),
            span: original.span,
        });
    let after = function
        .locals
        .iter_mut()
        .find(|local| local.name == "after")
        .unwrap();
    after.debug_type_name = Some("source-visible integer".into());
    let expected_after = after.clone();
    prepare_native_sequences(&mut program, &types);
    validate(&program).unwrap();
    let function = main_function(&program);
    assert_reachable_and_dense(function);
    assert!(
        !function
            .locals
            .iter()
            .any(|local| local.ty == TypeInterner::NEVER)
    );
    assert_eq!(function.params, original.params);
    let after = function
        .locals
        .iter()
        .find(|local| local.name == "after")
        .unwrap();
    assert!(after.id.index() < expected_after.id.index());
    let mut remapped = expected_after;
    remapped.id = after.id;
    assert_eq!(after, &remapped);
    assert_eq!(
        function
            .blocks
            .iter()
            .flat_map(|block| &block.statements)
            .filter(|statement| matches!(statement.kind, StatementKind::SequenceLength { .. }))
            .count(),
        1
    );
    assert!(
        !function
            .blocks
            .iter()
            .flat_map(|block| &block.statements)
            .any(|statement| matches!(
                statement.kind,
                StatementKind::SequenceGet { .. } | StatementKind::Trace(_)
            ))
    );
    assert!(function.blocks.iter().any(|block| matches!(&block.terminator.kind, TerminatorKind::Return(Some(Expression { kind: ExpressionKind::Local(local), .. })) if *local == after.id)));
    let prepared = program.clone();
    prepare_native_sequences(&mut program, &types);
    assert_eq!(program, prepared, "preparation is idempotent");
}

#[test]
fn uninhabited_iteration_keeps_once_evaluation_pending_checks_and_view_loan() {
    let (mut program, types) = lower_source(
        r#"namespace app
function produce[T](items: list[T]) returns list[T]:
    trace items
    return items
function main() returns nothing:
    for value in view run produce(list()):
        trace value
    return nothing
"#,
    );
    let function = main_function(&program);
    let original_iterable = function
        .blocks
        .iter()
        .find_map(|block| match &block.terminator.kind {
            TerminatorKind::ForEach { iterable, .. } => Some(iterable.clone()),
            _ => None,
        })
        .unwrap();
    let expected = if let ExpressionKind::View(value) = original_iterable.kind {
        *value
    } else {
        original_iterable
    };
    prepare_native_sequences(&mut program, &types);
    validate(&program).unwrap();
    let function = main_function(&program);
    assert_reachable_and_dense(function);
    let statements = function
        .blocks
        .iter()
        .flat_map(|block| &block.statements)
        .collect::<Vec<_>>();
    assert_eq!(statements.iter().filter(|statement| matches!(&statement.kind, StatementKind::Let { value, .. } if value == &expected)).count(), 1);
    let mut starts = Vec::new();
    let mut ends = Vec::new();
    let mut lengths = Vec::new();
    for statement in &statements {
        match &statement.kind {
            StatementKind::IterationBorrow {
                source,
                token,
                start,
            } => {
                if *start {
                    starts.push((source.clone(), *token));
                } else {
                    ends.push((source.clone(), *token));
                }
            }
            StatementKind::SequenceLength { source, .. } => lengths.push(source.clone()),
            StatementKind::SequenceGet { .. } => panic!("uninhabited binder must not be extracted"),
            _ => {}
        }
    }
    assert_eq!(starts.len(), 1);
    assert_eq!(starts, ends);
    assert_eq!(lengths, [starts[0].0.clone()]);
}

#[test]
fn uninhabited_iteration_preserves_nested_loops_handlers_and_exits() {
    let (mut program, types) = lower_source(
        r#"namespace app
function main(flag: bool) returns int64:
    mutable int64 count = 0
    for outer in list(1, 2):
        for value in list():
            trace value
        optional[int64] maybe = some(outer)
        int64 next = maybe handle:
            if flag:
                break
            default 5
        count = count + next
        if flag:
            continue
    return count
"#,
    );
    prepare_native_sequences(&mut program, &types);
    validate(&program).unwrap();
    let function = main_function(&program);
    assert_reachable_and_dense(function);
    let statements = function
        .blocks
        .iter()
        .flat_map(|block| &block.statements)
        .collect::<Vec<_>>();
    assert_eq!(
        statements
            .iter()
            .filter(|statement| matches!(statement.kind, StatementKind::SequenceLength { .. }))
            .count(),
        2
    );
    assert_eq!(
        statements
            .iter()
            .filter(|statement| matches!(statement.kind, StatementKind::SequenceGet { .. }))
            .count(),
        1
    );
    assert!(
        statements
            .iter()
            .any(|statement| matches!(statement.kind, StatementKind::SumTag { .. }))
    );
    assert!(
        statements
            .iter()
            .any(|statement| matches!(statement.kind, StatementKind::SumTake { .. }))
    );
    assert!(
        !function
            .locals
            .iter()
            .any(|local| local.ty == TypeInterner::NEVER)
    );
    move_values::MoveValuePlan::analyze(&program, function, &types)
        .expect("retained CFG ownership plan");
}

#[test]
fn uninhabited_map_iteration_handles_either_component_without_extracting_binders() {
    for (key, value) in [
        (TypeInterner::NEVER, TypeInterner::NEVER),
        (TypeInterner::INT64, TypeInterner::NEVER),
        (TypeInterner::NEVER, TypeInterner::STRING),
    ] {
        let (mut program, mut types) = lower_source(
            r#"namespace app
function main() returns nothing:
    for key, value in map():
        trace key
        trace value
    return nothing
"#,
        );
        let function = &mut program.functions[0];
        let mut bindings = None;
        for block in &mut function.blocks {
            if let TerminatorKind::ForEach {
                key: key_local,
                value: Some(value_local),
                iterable,
                ..
            } = &mut block.terminator.kind
            {
                iterable.ty = types.intern(Type::Map(key, value));
                bindings = Some((*key_local, *value_local));
            }
        }
        let (key_local, value_local) = bindings.unwrap();
        function.locals[key_local.index() as usize].ty = key;
        function.locals[key_local.index() as usize].debug_ty = key;
        function.locals[value_local.index() as usize].ty = value;
        function.locals[value_local.index() as usize].debug_ty = value;
        prepare_native_sequences(&mut program, &types);
        validate(&program).unwrap();
        let function = main_function(&program);
        assert_reachable_and_dense(function);
        assert!(
            !function
                .locals
                .iter()
                .any(|local| local.name == "key" || local.name == "value")
        );
        assert!(
            !function
                .blocks
                .iter()
                .flat_map(|block| &block.statements)
                .any(|statement| matches!(statement.kind, StatementKind::SequenceGet { .. }))
        );
    }
}

#[test]
fn uninhabited_projected_iteration_retains_ancestor_validation_and_matching_loans() {
    let (mut program, types) = lower_source(
        r#"namespace app
struct Holder[T]:
    items: list[T]
function wrap[T](items: list[T]) returns Holder[T]:
    return Holder[T](items: items)
function inspect[T](view holder: Holder[T]) returns nothing:
    for value in holder.items:
        trace value
    return nothing
function inspect_items[T](items: list[T]) returns nothing:
    Holder[T] holder = wrap[T](items)
    inspect[T](view holder)
    return nothing
function main() returns nothing:
    inspect_items(list())
    return nothing
"#,
    );
    prepare_native_sequences(&mut program, &types);
    validate(&program).unwrap();
    let function = program
        .functions
        .iter()
        .find(|function| function.identity.declaration.name == "inspect")
        .unwrap();
    assert_reachable_and_dense(function);
    let statements = function
        .blocks
        .iter()
        .flat_map(|block| &block.statements)
        .collect::<Vec<_>>();
    let source = statements
        .iter()
        .find_map(|statement| match &statement.kind {
            StatementKind::SequenceLength { source, .. } => Some(source),
            _ => None,
        })
        .unwrap();
    let SequenceSource::Projected { owner, path, ty } = source else {
        panic!("projected source must be retained");
    };
    assert_eq!(*owner, function.params[0].local);
    assert_eq!(path.len(), 1);
    assert!(matches!(types.resolve(*ty), Type::List(inner) if *inner == TypeInterner::NEVER));
    let loans = statements
        .iter()
        .filter_map(|statement| match &statement.kind {
            StatementKind::IterationBorrow {
                source: borrowed,
                token,
                start,
            } => {
                assert_eq!(borrowed, source);
                Some((*token, *start))
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(loans.len(), 2);
    assert_eq!(loans[0], (loans[1].0, true));
    assert!(!loans[1].1);
    assert!(
        !statements
            .iter()
            .any(|statement| matches!(statement.kind, StatementKind::SequenceGet { .. }))
    );
    move_values::MoveValuePlan::analyze(&program, function, &types)
        .expect("projected owner is only borrowed");
}

#[test]
fn uninhabited_iteration_remaps_capture_references_without_changing_lifted_functions() {
    let (mut program, types) = lower_source(
        r#"namespace app
function main(unused: int64) returns int64:
    for value in list():
        trace value
    int64 retained = 40
    function(int64) returns int64 callback = function(value: int64) returns int64: return retained + value
    return callback(2)
"#,
    );
    let main_id = main_function(&program).id;
    let lifted = program
        .functions
        .iter()
        .filter(|function| function.id != main_id)
        .cloned()
        .collect::<Vec<_>>();
    prepare_native_sequences(&mut program, &types);
    validate(&program).unwrap();
    assert_eq!(
        program
            .functions
            .iter()
            .filter(|function| function.id != main_id)
            .cloned()
            .collect::<Vec<_>>(),
        lifted
    );
    let function = main_function(&program);
    assert_reachable_and_dense(function);
    assert_eq!(function.params.len(), 1, "unused ABI parameters remain");
    let retained = function
        .locals
        .iter()
        .find(|local| local.name == "retained")
        .unwrap()
        .id;
    let (target, captures) = function
        .blocks
        .iter()
        .flat_map(|block| &block.statements)
        .find_map(|statement| match &statement.kind {
            StatementKind::Let {
                value:
                    Expression {
                        kind: ExpressionKind::ClosureRef { function, captures },
                        ..
                    },
                ..
            } => Some((*function, captures)),
            _ => None,
        })
        .unwrap();
    assert_eq!(captures, &[retained]);
    assert!(
        lifted
            .iter()
            .any(|function| function.id == target && function.capture_count == 1)
    );
}

#[test]
fn uninhabited_iteration_does_not_hide_invalid_dead_ids_or_prune_other_functions() {
    for corruption in 0..6 {
        let (mut program, types) = lower_source(EMPTY_LOOP);
        let function = &mut program.functions[0];
        let body = function
            .blocks
            .iter()
            .find_map(|block| match block.terminator.kind {
                TerminatorKind::ForEach { body, .. } => Some(body),
                _ => None,
            })
            .unwrap();
        let block = &mut function.blocks[body.index() as usize];
        match corruption {
            0 => block.statements.push(Statement {
                kind: StatementKind::Trace(LocalId::new(u32::MAX)),
                span: function.span,
            }),
            1 => block.terminator.kind = TerminatorKind::Goto(BlockId(u32::MAX)),
            2 => block.statements.push(Statement {
                kind: StatementKind::Evaluate(Expression {
                    kind: ExpressionKind::FunctionRef(FunctionId::new(u32::MAX)),
                    ty: TypeInterner::NOTHING,
                    span: function.span,
                }),
                span: function.span,
            }),
            _ => block.statements.push(Statement {
                kind: StatementKind::Evaluate(Expression {
                    kind: ExpressionKind::InlineFunction {
                        scoped_type_bindings: Vec::new(),
                        params: Vec::new(),
                        view_params: match corruption {
                            3 => vec![LocalId::new(u32::MAX)],
                            4 => vec![function.params[0].local],
                            _ => Vec::new(),
                        },
                        local_floor: if corruption == 5 { u32::MAX } else { 0 },
                        body: hir::Block {
                            statements: Vec::new(),
                            span: function.span,
                        },
                    },
                    ty: TypeInterner::NOTHING,
                    span: function.span,
                }),
                span: function.span,
            }),
        }
        let original = program.clone();
        let errors = validate(&program).unwrap_err();
        if corruption >= 3 {
            let expected = match corruption {
                3 => "inline function view parameter references local",
                4 => "inline function view parameter is not a parameter",
                _ => "inline function local floor is out of range",
            };
            assert!(errors.iter().any(|error| error.message.contains(expected)));
        }
        prepare_native_sequences(&mut program, &types);
        assert_eq!(program, original);
        assert!(validate(&program).is_err());
    }
    let (mut program, types) = lower_source(
        r#"namespace app
function unrelated(unused: int64) returns int64:
    return 7
function main() returns nothing:
    for value in list():
        trace value
    return nothing
"#,
    );
    let unrelated = program.functions[0].clone();
    prepare_native_sequences(&mut program, &types);
    assert_eq!(program.functions[0], unrelated);
    validate(&program).unwrap();
}
