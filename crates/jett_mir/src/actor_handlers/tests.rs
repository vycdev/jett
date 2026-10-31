use super::*;
use hir::ExpressionKind;
use jett_common::{FileId, SourceOrigin};
use std::collections::HashMap;
const SOURCE: &str = r#"namespace app
struct State:
    value: int64
    items: list[int64]
actor Counter(seed: int64):
    mutable State state = State(value: seed, items: list(seed))
    receive replace(values: list[int64]):
        state.items = values
        state.value = state.value + 1
    receive read responds int64:
        respond state.value
function main() returns int64:
    Counter first = spawn Counter(seed: 1)
    Counter other = spawn Counter(seed: 2)
    list[int64] supplied = list(3)
    list[int64] wrong = list(4)
    send first.replace(supplied)
    return ask first.read
"#;
fn program(source: &str) -> (Program, TypeInterner) {
    let file = FileId::new(0);
    let parsed = jett_parser::parse(source, file);
    assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
    let resolved = jett_resolve::resolve(&parsed.module);
    let checked = jett_typecheck::check(&parsed.module, &resolved);
    assert!(
        resolved
            .diagnostics
            .iter()
            .chain(&checked.diagnostics)
            .all(|error| error.severity != jett_diagnostics::Severity::Error),
        "{:?} {:?}",
        resolved.diagnostics,
        checked.diagnostics
    );
    let high = hir::lower(
        &parsed.module,
        &resolved,
        &checked,
        &HashMap::from([(file, SourceOrigin::Project)]),
    )
    .unwrap();
    (
        crate::lower(&high, &checked.interner).unwrap(),
        checked.interner,
    )
}
#[test]
fn actor_handler_cfg_and_roles_remain_bound_to_original_entry() {
    let (program, types) = program(SOURCE);
    crate::validate(&program).unwrap();
    crate::validate_call_ownership(&program, &types).unwrap();
    let index = program
        .functions
        .iter()
        .position(|function| {
            function.identity.declaration.kind == hir::DeclarationKind::ActorHandler
        })
        .unwrap();
    let mut changed = program.clone();
    changed.functions[index].params[1].mutable = false;
    assert!(crate::validate(&changed).is_err());
    let mut removed = program.clone();
    removed.functions[index].actor_handler = None;
    assert!(crate::validate(&removed).is_err());
    let mut moved = program.clone();
    moved.functions[index].identity.declaration.kind = hir::DeclarationKind::Function;
    assert!(crate::validate(&moved).is_err());
}
#[test]
fn ordinary_message_caller_rejects_same_typed_receiver_and_argument_substitution() {
    let (program, types) = program(SOURCE);
    let main = program
        .functions
        .iter()
        .position(|function| function.identity.declaration.name == "main")
        .unwrap();
    for receiver in [true, false] {
        let mut changed = program.clone();
        let function = &mut changed.functions[main];
        let replacement = function
            .locals
            .iter()
            .find(|local| local.name == if receiver { "other" } else { "wrong" })
            .unwrap()
            .id;
        let send = function
            .blocks
            .iter_mut()
            .flat_map(|block| &mut block.statements)
            .find_map(|statement| match &mut statement.kind {
                StatementKind::Evaluate(Expression {
                    kind: ExpressionKind::ActorMessage { actor, args, .. },
                    ..
                }) => Some((actor, args)),
                _ => None,
            })
            .unwrap();
        if receiver {
            send.0.kind = ExpressionKind::Local(replacement);
        } else {
            send.1[0].kind = ExpressionKind::Local(replacement);
        }
        assert!(crate::validate_call_ownership(&changed, &types).is_err());
    }
}
#[test]
fn genuine_actor_sequence_preparation_preserves_entry_receipt() {
    let source = SOURCE.replace(
        "        state.value = state.value + 1",
        "        for amount in list(1, 2):\n            state.value = state.value + amount",
    );
    let (mut program, types) = program(&source);
    crate::prepare_native_sequences(&mut program, &types);
    crate::validate(&program).unwrap();
    crate::validate_call_ownership(&program, &types).unwrap();
    for function in &program.functions {
        crate::move_values::MoveValuePlan::analyze(&program, function, &types).unwrap();
    }
}

#[test]
fn handled_actor_receiver_rejoins_exact_original_sum_cfg() {
    let source = SOURCE.replace(
        "function main() returns int64:",
        r#"function maybe_counter(valid: bool) returns result[Counter, string]:
    if valid:
        return ok(spawn Counter(seed: 1))
    return fail("missing")
function target_value(valid: bool) returns int64:
    return ask(maybe_counter(valid) handle error:
        default spawn Counter(seed: 2)
    ).read
function main() returns int64:"#,
    );
    let (mut program, types) = program(&source);
    crate::validate_call_ownership(&program, &types)
        .expect("genuine handled actor receiver before preparation");
    crate::prepare_native_sequences(&mut program, &types);
    crate::validate_call_ownership(&program, &types)
        .expect("genuine handled actor receiver after preparation");
    for function in &program.functions {
        crate::move_values::MoveValuePlan::analyze(&program, function, &types).unwrap();
    }
}

#[test]
fn owning_respond_handle_is_lowered_before_actor_publication() {
    let source = SOURCE
        .replace(
            "actor Counter(seed: int64):",
            r#"function maybe_values(valid: bool, value: int64) returns result[list[int64], string]:
    if valid:
        return ok(list(value))
    return fail("missing")
actor Counter(seed: int64):"#,
        )
        .replace(
            "    receive read responds int64:",
            r#"    receive answer(valid: bool) responds list[int64]:
        state.value = state.value + 1
        respond maybe_values(valid, state.value) handle error:
            default list(9)
    receive read responds int64:"#,
        );
    let (program, types) = program(&source);
    let answer = program
        .functions
        .iter()
        .find(|function| function.identity.declaration.name == "Counter.answer")
        .unwrap();
    assert!(answer.blocks.iter().any(|block|matches!(&block.terminator.kind,TerminatorKind::Respond(value) if matches!(value.kind,ExpressionKind::Local(_)))));
    assert!(
        answer
            .blocks
            .iter()
            .any(|block| matches!(block.terminator.kind, TerminatorKind::Branch { .. }))
    );
    crate::validate(&program).unwrap();
    crate::validate_call_ownership(&program, &types).unwrap();
    crate::move_values::MoveValuePlan::analyze(&program, answer, &types).unwrap();
}

#[test]
fn persistent_actor_views_transport_with_their_exact_owned_constructor_backing() {
    let source = r#"namespace app
struct Inner:
    value: int64
struct Outer:
    prefix: int64
    inner: Inner
actor Counter(seed: int64):
    mutable Outer owned = Outer(prefix: 9, inner: Inner(value: seed))
    Inner projected = view owned.inner
    Inner alias = view projected
    mutable Inner independent = Inner(value: seed)
    receive advance(amount: int64):
        independent.value = independent.value + amount + alias.value
    receive read responds int64:
        respond projected.value + alias.value + independent.value
function main() returns int64:
    Counter first = spawn Counter(seed: 1)
    send first.advance(2)
    return ask first.read
"#;
    let (mut original, types) = program(source);
    crate::validate_call_ownership(&original, &types).unwrap();
    for function in &original.functions {
        crate::move_values::MoveValuePlan::analyze(&original, function, &types).unwrap();
    }
    crate::prepare_native_sequences(&mut original, &types);
    for function in &original.functions {
        crate::move_values::MoveValuePlan::analyze(&original, function, &types).unwrap();
    }
    let constructor = original
        .functions
        .iter()
        .position(|function| !function.actor_state_views.is_empty())
        .unwrap();
    let mut missing = original.clone();
    missing.functions[constructor].actor_handler = None;
    assert!(crate::validate(&missing).is_err());
    let mut foreign = original.clone();
    foreign.functions[constructor].identity.declaration.kind = hir::DeclarationKind::Function;
    assert!(crate::validate(&foreign).is_err());
    let mut reordered = original.clone();
    let allocation = reordered.functions[constructor]
        .blocks
        .iter_mut()
        .find_map(|block| match &mut block.terminator.kind {
            TerminatorKind::Return(Some(Expression {
                kind: ExpressionKind::ActorSpawn { args, .. },
                ..
            })) => Some(args),
            _ => None,
        })
        .unwrap();
    allocation.swap(1, 2);
    assert!(crate::validate(&reordered).is_err());
}

#[test]
fn nonresponding_return_nothing_keeps_owned_state_for_publication() {
    let source = SOURCE.replace(
        "        state.value = state.value + 1",
        "        state.value = state.value + 1\n        return nothing",
    );
    let (mut program, types) = program(&source);
    for prepared in [false, true] {
        if prepared {
            crate::prepare_native_sequences(&mut program, &types);
        }
        crate::validate(&program).unwrap();
        crate::validate_call_ownership(&program, &types).unwrap();
        let handler_index = program
            .functions
            .iter()
            .position(|function| function.identity.declaration.name == "Counter.replace")
            .unwrap();
        let handler = &program.functions[handler_index];
        let state = handler
            .actor_handler_entry()
            .unwrap()
            .unwrap()
            .roles()
            .iter()
            .find(|role| {
                matches!(
                    role.role(),
                    jett_typecheck::CheckedActorBindingRole::State { .. }
                )
            })
            .unwrap()
            .local()
            .index() as usize;
        assert_eq!(
            handler
                .blocks
                .iter()
                .flat_map(|block| &block.statements)
                .filter(|statement| matches!(statement.kind, StatementKind::UpdateField { .. }))
                .count(),
            2,
        );
        let return_block = handler
            .blocks
            .iter()
            .position(|block| {
                matches!(
                    &block.terminator.kind,
                    TerminatorKind::Return(Some(value))
                        if value.ty == TypeInterner::NOTHING
                            && matches!(value.kind, ExpressionKind::Nothing)
                )
            })
            .expect("the genuine no-response exit stays Return(nothing)");
        let plan = crate::move_values::MoveValuePlan::analyze(&program, handler, &types).unwrap();
        assert!(plan.owned_locals.contains(&state));
        assert!(plan.live_in[return_block].contains(&state));
        assert!(plan.live_out[return_block].contains(&state));
        assert!(
            plan.live_after_statement[return_block]
                .iter()
                .all(|live| live.contains(&state))
        );

        let mut changed = program.clone();
        let terminator = &mut changed.functions[handler_index].blocks[return_block].terminator;
        let TerminatorKind::Return(Some(value)) = &terminator.kind else {
            unreachable!()
        };
        terminator.kind = TerminatorKind::Respond(value.clone());
        assert!(crate::validate(&changed).is_err());
    }
}
