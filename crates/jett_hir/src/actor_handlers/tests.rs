use super::*;
use jett_diagnostics::Severity;
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
fn checked(
    source: &str,
) -> (
    ast::Module,
    jett_resolve::ResolveResult,
    CheckResult,
    HashMap<FileId, SourceOrigin>,
) {
    let file = FileId::new(0);
    let parsed = jett_parser::parse(source, file);
    assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
    let resolved = jett_resolve::resolve(&parsed.module);
    assert!(
        resolved
            .diagnostics
            .iter()
            .all(|error| error.severity != Severity::Error),
        "{:?}",
        resolved.diagnostics
    );
    let check = jett_typecheck::check(&parsed.module, &resolved);
    assert!(
        check
            .diagnostics
            .iter()
            .all(|error| error.severity != Severity::Error),
        "{:?}",
        check.diagnostics
    );
    (
        parsed.module,
        resolved,
        check,
        HashMap::from([(file, SourceOrigin::Project)]),
    )
}
fn program() -> (Program, CheckResult) {
    let (module, resolved, check, origins) = checked(SOURCE);
    let program = lower(&module, &resolved, &check, &origins).expect("genuine actor Source lowers");
    (program, check)
}
#[test]
fn original_actor_entry_and_message_operands_reject_corruption() {
    let (original, check) = program();
    validate(&original).unwrap();
    validate_backend_types(&original, &check.interner).unwrap();
    let main = original
        .functions
        .iter()
        .position(|function| function.identity.declaration.name == "main")
        .unwrap();
    let mut wrong_receiver = original.clone();
    let other = wrong_receiver.functions[main]
        .locals
        .iter()
        .find(|local| local.name == "other")
        .unwrap()
        .id;
    let StatementKind::Expression(Expression {
        kind: ExpressionKind::ActorMessage { actor, .. },
        ..
    }) = &mut wrong_receiver.functions[main].body.statements[4].kind
    else {
        panic!("original send");
    };
    actor.kind = ExpressionKind::Local(other);
    assert!(validate_backend_types(&wrong_receiver, &check.interner).is_err());
    let mut wrong_argument = original.clone();
    let wrong = wrong_argument.functions[main]
        .locals
        .iter()
        .find(|local| local.name == "wrong")
        .unwrap()
        .id;
    let StatementKind::Expression(Expression {
        kind: ExpressionKind::ActorMessage { args, .. },
        ..
    }) = &mut wrong_argument.functions[main].body.statements[4].kind
    else {
        panic!("original send");
    };
    args[0].kind = ExpressionKind::Local(wrong);
    assert!(validate_backend_types(&wrong_argument, &check.interner).is_err());
    let mut wrong_role = original.clone();
    let handler = wrong_role
        .functions
        .iter_mut()
        .find(|function| function.actor_handler.is_some())
        .unwrap();
    handler.params[1].mutable = false;
    assert!(validate(&wrong_role).is_err());
}
#[test]
fn same_span_changed_original_actor_cannot_mint_native_receipt() {
    let (mut module, resolved, check, origins) = checked(SOURCE);
    let actor = module
        .items
        .iter_mut()
        .find_map(|item| {
            if let Item::Actor(actor) = item {
                Some(actor)
            } else {
                None
            }
        })
        .unwrap();
    actor.handlers[0].body.stmts.swap(0, 1);
    let errors = lower(&module, &resolved, &check, &origins)
        .expect_err("retained original actor syntax is immutable");
    assert!(
        errors
            .iter()
            .any(|error| error.message.contains("original checked ownership receipt")),
        "{errors:?}"
    );
}
#[test]
fn projected_persistent_state_view_retains_original_initializer_and_constructor_scope() {
    let source = r#"namespace app
struct Inner:
    value: int64
struct Outer:
    inner: Inner
actor Counter(seed: int64):
    mutable Outer owned = Outer(inner: Inner(value: seed))
    Inner projected = view owned.inner
    Inner alias = view projected
    mutable Inner independent = Inner(value: seed)
    receive advance(amount: int64):
        independent.value = independent.value + amount
    receive read responds int64:
        respond projected.value + alias.value + independent.value
function main() returns int64:
    Counter first = spawn Counter(seed: 1)
    send first.advance(2)
    return ask first.read
"#;
    let (module, resolved, check, origins) = checked(source);
    let original = lower(&module, &resolved, &check, &origins).unwrap();
    validate_backend_types(&original, &check.interner).unwrap();
    let constructor = original
        .functions
        .iter()
        .find(|function| !function.actor_state_views.is_empty())
        .unwrap();
    assert_eq!(constructor.actor_state_views.len(), 2);
    let view = constructor.actor_state_views[0].clone();
    assert!(view.matches_constructor(constructor));
    let mut foreign = original.clone();
    let main = foreign
        .functions
        .iter_mut()
        .find(|function| function.identity.declaration.name == "main")
        .unwrap();
    main.actor_state_views.push(view);
    assert!(validate(&foreign).is_err());
    let handler = original
        .functions
        .iter()
        .find(|function| function.identity.declaration.name == "Counter.read")
        .unwrap();
    let entry = handler.actor_handler.as_ref().unwrap();
    assert_eq!(
        entry
            .roles()
            .iter()
            .filter(|role| role.state_view().is_some())
            .count(),
        2
    );
}
