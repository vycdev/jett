use super::*;
use jett_common::{FileId, SourceOrigin};
use jett_hir::ExpressionKind;
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

fn assert_dense_reachable(function: &Function) {
    let cfg = ControlFlowGraph::analyze(function).unwrap();
    let mut pending = vec![function.entry];
    let mut reachable = HashSet::new();
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

const AFTER_LOOP: &str = r#"namespace app
function main(unused: int64) returns int64:
    for impossible in list():
        trace impossible
    int64 kept = 40
    function(view int64, int64) returns int64 callback = function(view borrowed: int64, value: int64) returns int64: return kept + value
    return callback(view 10, 2)
"#;

#[test]
fn live_captured_callback_discards_parent_never_slots_without_changing_capture_abi() {
    let (mut program, types) = lower_source(AFTER_LOOP);
    prepare_native_sequences(&mut program, &types);
    prepare_native_uninhabited_sums(&mut program, &types);
    let original = program
        .functions
        .iter()
        .find(|function| function.debug_kind == hir::FunctionDebugKind::Inline)
        .unwrap()
        .clone();
    assert!(
        original
            .locals
            .iter()
            .any(|local| local.ty == TypeInterner::NEVER)
    );
    let named = program
        .functions
        .iter()
        .filter(|function| function.debug_kind != hir::FunctionDebugKind::Inline)
        .cloned()
        .collect::<Vec<_>>();
    prepare_native_generated_functions(&mut program);
    validate(&program).unwrap();
    let function = &program.functions[original.id.index() as usize];
    assert_dense_reachable(function);
    assert!(
        !function
            .locals
            .iter()
            .any(|local| local.ty == TypeInterner::NEVER)
    );
    assert_eq!(function.id, original.id);
    assert_eq!(function.identity, original.identity);
    assert_eq!(function.debug_kind, original.debug_kind);
    assert_eq!(function.return_type, original.return_type);
    assert_eq!(function.span, original.span);
    assert_eq!(function.capture_count, 1);
    assert_eq!(function.capture_count, original.capture_count);
    assert_eq!(function.params.len(), original.params.len());
    for (parameter, old_parameter) in function.params.iter().zip(&original.params) {
        let mut expected = old_parameter.clone();
        expected.local = parameter.local;
        assert_eq!(parameter, &expected);
        let mut expected_local = original.local(old_parameter.local).unwrap().clone();
        expected_local.id = parameter.local;
        assert_eq!(function.local(parameter.local), Some(&expected_local));
    }
    assert_eq!(function.params[0].name, "kept");
    assert_eq!(function.params[1].name, "borrowed");
    assert_eq!(function.params[1].mode, ParamMode::View);
    assert_eq!(function.params[2].name, "value");
    assert!(function.blocks.iter().any(|block| matches!(
        &block.terminator.kind,
        TerminatorKind::Return(Some(Expression {
            kind: ExpressionKind::Binary { left, right, .. },
            ..
        })) if matches!(left.kind, ExpressionKind::Local(local) if local == function.params[0].local)
            && matches!(right.kind, ExpressionKind::Local(local) if local == function.params[2].local)
    )));
    assert_eq!(
        program
            .functions
            .iter()
            .filter(|function| function.debug_kind != hir::FunctionDebugKind::Inline)
            .cloned()
            .collect::<Vec<_>>(),
        named,
        "caller capture references remain in the caller's own local table"
    );
    move_values::MoveValuePlan::analyze(&program, function, &types)
        .expect("captured parameter and body references remain valid");
    let prepared = program.clone();
    prepare_native_generated_functions(&mut program);
    assert_eq!(program, prepared);
}

#[test]
fn callbacks_after_absent_error_arms_preserve_live_capture_and_dead_function_ids() {
    let (mut program, types) = lower_source(
        r#"namespace app
function main() returns int64:
    int64 ready = ok(1) handle error:
        function() returns int64 ignored = function() returns int64: return 9
        default ignored()
    function() returns int64 callback = function() returns int64: return ready
    return callback()
"#,
    );
    prepare_native_uninhabited_sums(&mut program, &types);
    let original = program.clone();
    assert_eq!(
        program
            .functions
            .iter()
            .filter(|function| function.debug_kind == hir::FunctionDebugKind::Inline)
            .count(),
        2
    );
    prepare_native_generated_functions(&mut program);
    validate(&program).unwrap();
    assert_eq!(program.functions.len(), original.functions.len());
    for (function, previous) in program.functions.iter().zip(&original.functions) {
        assert_eq!(function.id, previous.id);
        if function.debug_kind != hir::FunctionDebugKind::Inline {
            assert_eq!(function, previous);
            continue;
        }
        assert_dense_reachable(function);
        assert!(
            previous
                .locals
                .iter()
                .any(|local| local.ty == TypeInterner::NEVER)
        );
        assert!(
            !function
                .locals
                .iter()
                .any(|local| local.ty == TypeInterner::NEVER)
        );
        assert_eq!(function.capture_count, previous.capture_count);
        move_values::MoveValuePlan::analyze(&program, function, &types)
            .expect("independent callback local table");
    }
    let live = program
        .functions
        .iter()
        .find(|function| {
            function.debug_kind == hir::FunctionDebugKind::Inline && function.capture_count == 1
        })
        .unwrap();
    assert_eq!(live.params[0].name, "ready");
    assert!(live.blocks.iter().any(|block| matches!(
        &block.terminator.kind,
        TerminatorKind::Return(Some(Expression { kind: ExpressionKind::Local(local), .. })) if *local == live.params[0].local
    )));
}

#[test]
fn dead_inline_never_parameters_remain_uninhabited_instead_of_becoming_values() {
    let (mut program, types) = lower_source(
        r#"namespace app
function choose[T, E](outcome: result[T, E], fallback: T) returns T:
    return outcome handle error:
        function(E) returns nothing ignored = function(item: E) returns nothing: return nothing
        ignored(error)
        default fallback
function main() returns int64:
    return choose(ok(7), 0)
"#,
    );
    prepare_native_uninhabited_sums(&mut program, &types);
    let original = program
        .functions
        .iter()
        .find(|function| function.debug_kind == hir::FunctionDebugKind::Inline)
        .unwrap()
        .clone();
    assert_eq!(original.params[0].ty, TypeInterner::NEVER);
    prepare_native_generated_functions(&mut program);
    validate(&program).unwrap();
    let function = &program.functions[original.id.index() as usize];
    assert_dense_reachable(function);
    assert_eq!(function.id, original.id);
    assert_eq!(function.capture_count, 0);
    assert_eq!(function.params.len(), 1);
    assert_eq!(function.params[0].ty, TypeInterner::NEVER);
    assert_eq!(function.locals.len(), 1, "unused ABI parameter remains");
    assert_eq!(function.locals[0].ty, TypeInterner::NEVER);
    assert_eq!(function.return_type, TypeInterner::NOTHING);
    // Reachability, rather than invented storage for never, decides whether this
    // separately lifted body needs a callable native declaration.
}

#[test]
fn generated_metadata_selection_preserves_named_functions_even_with_inline_like_names() {
    let (mut program, _) = lower_source(AFTER_LOOP);
    let function = program
        .functions
        .iter_mut()
        .find(|function| function.debug_kind == hir::FunctionDebugKind::Inline)
        .unwrap();
    function.identity.declaration.name = "parent$inline123_456".into();
    function.debug_kind = hir::FunctionDebugKind::Named("app.source_visible_name".into());
    validate(&program).unwrap();
    let original = program.clone();
    prepare_native_generated_functions(&mut program);
    assert_eq!(program, original);
}

#[test]
fn generated_compaction_does_not_hide_invalid_unreachable_or_parameter_ids() {
    for corruption in 0..3 {
        let (mut program, _) = lower_source(AFTER_LOOP);
        let function = program
            .functions
            .iter_mut()
            .find(|function| function.debug_kind == hir::FunctionDebugKind::Inline)
            .unwrap();
        match corruption {
            0 => function.entry = BlockId(u32::MAX),
            1 => function.params[0].local = LocalId::new(u32::MAX),
            _ => function.blocks.push(BasicBlock {
                id: BlockId(function.blocks.len() as u32),
                statements: vec![Statement {
                    kind: StatementKind::Trace(LocalId::new(u32::MAX)),
                    span: function.span,
                }],
                terminator: Terminator {
                    kind: TerminatorKind::Unreachable,
                    span: function.span,
                },
            }),
        }
        assert!(validate(&program).is_err());
        let original = program.clone();
        prepare_native_generated_functions(&mut program);
        assert_eq!(program, original);
        assert!(validate(&program).is_err());
    }
}
