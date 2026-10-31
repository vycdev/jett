use super::*;
use jett_diagnostics::Severity;

fn checked(source: &str, release: bool) -> (CheckResult, ResolveResult) {
    let parsed = jett_parser::parse(source, FileId::new(0));
    assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
    let resolved = jett_resolve::resolve(&parsed.module);
    assert!(
        resolved
            .diagnostics
            .iter()
            .all(|diagnostic| diagnostic.severity != Severity::Error),
        "{:?}",
        resolved.diagnostics,
    );
    let result = check_with_options(&parsed.module, &resolved, CheckOptions { release });
    (result, resolved)
}

fn assert_accepted(result: &CheckResult) {
    assert!(
        result
            .diagnostics
            .iter()
            .all(|diagnostic| diagnostic.severity != Severity::Error),
        "{:?}",
        result.diagnostics,
    );
}

#[test]
fn actor_message_arguments_keep_lexical_moves_and_named_parameter_join() {
    let source = r#"namespace app
actor Store:
    receive put(first: list[int64], second: list[int64]):
        trace first
        trace second
function main() returns nothing:
    Store worker = spawn Store()
    list[int64] one = list(1)
    list[int64] two = list(2)
    send worker.put(second: two, first: one)
"#;
    for release in [false, true] {
        let (result, resolved) = checked(source, release);
        assert_accepted(&result);
        let packets = result.actor_message_calls.values().collect::<Vec<_>>();
        let [packet] = packets.as_slice() else {
            panic!("one complete actor message");
        };
        assert_eq!(packet.kind, CheckedActorMessageKind::Send);
        assert_eq!(packet.target.handler_index, 0);
        assert_eq!(
            resolved
                .scope_table
                .def(packet.target.actor_definition)
                .kind,
            DefKind::Actor
        );
        assert_eq!(
            resolved
                .scope_table
                .def(packet.target.actor_definition)
                .name,
            "app.Store"
        );
        assert_eq!(packet.arguments.len(), 2);
        for (source_index, parameter_index, name) in [(0, 1, "two"), (1, 0, "one")] {
            let argument = &packet.arguments[source_index];
            assert_eq!(argument.source_index, source_index);
            assert_eq!(argument.parameter_index, parameter_index);
            assert_eq!(argument.effect, CheckedCallerEffect::TransferOwned);
            let CheckedCallerOrigin::Binding(binding) = &argument.origin else {
                panic!("an exact caller binding");
            };
            assert_eq!(resolved.scope_table.def(binding.definition).name, name);
        }
        assert!(
            result.call_ownership.is_empty(),
            "a message is not a function call"
        );
    }
}

#[test]
fn identically_shaped_actors_keep_distinct_nominal_message_targets() {
    let source = r#"namespace app
actor First:
    receive get responds int64:
        respond 1
actor Second:
    receive get responds int64:
        respond 2
function main() returns int64:
    First first = spawn First()
    Second second = spawn Second()
    int64 left = ask first.get
    int64 right = ask second.get
    return left + right
"#;
    let (result, _) = checked(source, false);
    assert_accepted(&result);
    let packets = result.actor_message_calls.values().collect::<Vec<_>>();
    assert_eq!(packets.len(), 2);
    assert_ne!(
        packets[0].target.actor_definition,
        packets[1].target.actor_definition
    );
    assert_ne!(packets[0].target.actor, packets[1].target.actor);
    assert_eq!(
        packets[0].target.handler_index,
        packets[1].target.handler_index
    );
    for packet in packets {
        assert_eq!(packet.kind, CheckedActorMessageKind::Ask);
        assert_eq!(packet.response_type, TypeInterner::INT64);
        assert!(packet.arguments.is_empty());
    }
}

#[test]
fn actor_view_formal_receives_a_transferred_owner_without_a_borrowed_sender() {
    let source = r#"namespace app
actor Sink:
    receive inspect(view values: list[int64]):
        trace values
function main() returns nothing:
    Sink sink = spawn Sink()
    list[int64] values = list(1)
    send sink.inspect(values)
"#;
    let (result, _) = checked(source, false);
    assert_accepted(&result);
    let packet = result.actor_message_calls.values().next().unwrap();
    assert_eq!(packet.arguments[0].callee_access, CheckedCalleeAccess::View);
    assert_eq!(
        packet.arguments[0].effect,
        CheckedCallerEffect::RelinquishOwned
    );
    assert_eq!(packet.arguments[0].syntax, CheckedCallerSyntax::Bare);
}

#[test]
fn actor_message_facts_remain_in_the_concrete_generic_body() {
    let source = r#"namespace app
actor Store:
    receive put(value: list[int64]):
        trace value
function relay[T](worker: Store, value: T) returns nothing:
    send worker.put(value)
function main() returns nothing:
    Store worker = spawn Store()
    relay(worker, list(1))
"#;
    let (result, _) = checked(source, false);
    assert_accepted(&result);
    assert!(result.actor_message_calls.is_empty());
    assert_eq!(result.generic_function_instantiations.len(), 1);
    let body = &result.generic_function_instantiations[0];
    assert_eq!(body.actor_message_calls.len(), 1);
    let packet = body.actor_message_calls.values().next().unwrap();
    assert_eq!(
        packet.arguments[0].actual_type,
        packet.arguments[0].parameter_type
    );
    assert_eq!(
        packet.arguments[0].effect,
        CheckedCallerEffect::TransferOwned
    );
}

#[test]
fn actor_boundary_rejects_borrowed_message_data_in_ordinary_and_observed_bodies() {
    let header =
        "namespace app\nactor Store:\n    receive put(value: list[int64]):\n        trace value\n";
    for body in [
        "function invoke(view values: list[int64]) returns nothing:\n    Store worker = spawn Store()\n    send worker.put(values)\n",
        "verify message:\n    list[int64] values = list(1)\n    list[int64] alias = view values\n    Store worker = spawn Store()\n    send worker.put(alias)\n",
    ] {
        let (result, _) = checked(&format!("{header}{body}"), false);
        assert!(
            result.diagnostics.iter().any(|diagnostic| {
                diagnostic.severity == Severity::Error
                    && diagnostic.code.code() == 401
                    && diagnostic
                        .message
                        .contains("cannot send a view to an actor")
            }),
            "{:?}",
            result.diagnostics
        );
        assert!(
            result
                .actor_handlers
                .values()
                .all(|handler| !handler.ownership_validated())
        );
    }
}

#[test]
fn editing_private_handler_body_or_observed_exits_retires_the_ownership_receipt() {
    let source = "namespace app\nactor Store:\n    receive ping:\n        int64 number = 1\n        trace number\n";
    let (result, _) = checked(source, false);
    assert_accepted(&result);
    let handler = result.actor_handlers.values().next().unwrap();
    assert!(handler.ownership_validated());
    let mut changed_body = handler.clone();
    changed_body.body_mut().actor_message_calls.clear();
    assert!(!changed_body.ownership_validated());
    let mut changed_exit = handler.clone();
    changed_exit.record_exits(Vec::new());
    assert!(!changed_exit.ownership_validated());
}

#[test]
fn foreign_same_shaped_handler_and_message_facts_cannot_replace_the_current_session() {
    let source = "namespace app\nactor Store:\n    receive put(value: list[int64]):\n        trace value\nfunction main() returns nothing:\n    Store worker = spawn Store()\n    send worker.put(list(1))\n";
    let (first, _) = checked(source, false);
    let (mut second, _) = checked(source, false);
    assert_accepted(&first);
    assert_accepted(&second);
    let target = *first.actor_handlers.keys().next().unwrap();
    assert!(first.actor_handler(target).is_some());
    assert!(second.actor_handler(target).is_some());
    second
        .actor_handlers
        .insert(target, first.actor_handlers[&target].clone());
    assert!(second.actor_handler(target).is_none());
    let (span, packet) = first.actor_message_calls.iter().next().unwrap();
    assert!(first.validates_actor_message_at(
        packet,
        crate::CheckedActorMessageRoot::Ordinary,
        &[],
        *span
    ));
    second.actor_message_calls.insert(*span, packet.clone());
    assert!(!second.validates_actor_message_at(
        packet,
        crate::CheckedActorMessageRoot::Ordinary,
        &[],
        *span
    ));
    let mut changed = packet.clone();
    changed.arguments[0].effect = CheckedCallerEffect::Copy;
    assert!(!first.validates_actor_message(&changed));
    assert!(!first.validates_actor_message_at(
        packet,
        crate::CheckedActorMessageRoot::Ordinary,
        &[],
        packet.receiver_span
    ));
}

#[test]
fn same_span_generic_message_transplants_do_not_gain_the_other_body_context() {
    let source = r#"namespace app
actor Store:
    receive put(value: list[int64]):
        trace value
function relay[T](worker: Store, tag: T, payload: list[int64]) returns nothing:
    trace tag
    send worker.put(payload)
function main() returns nothing:
    Store worker = spawn Store()
    relay(clone worker, 1, list(1))
    relay(worker, "second", list(2))
"#;
    let (mut result, _) = checked(source, false);
    assert_accepted(&result);
    assert_eq!(result.generic_function_instantiations.len(), 2);
    let (span, packet) = result.generic_function_instantiations[0]
        .actor_message_calls
        .iter()
        .next()
        .map(|(span, call)| (*span, call.clone()))
        .unwrap();
    let other = result.generic_function_instantiations[1].actor_message_calls[&span].clone();
    assert_eq!(packet.target, other.target);
    assert_eq!(packet.arguments, other.arguments);
    assert!(result.validates_actor_message_at(
        &packet,
        crate::CheckedActorMessageRoot::Generic(0),
        &[],
        span
    ));
    assert!(result.validates_actor_message_at(
        &other,
        crate::CheckedActorMessageRoot::Generic(1),
        &[],
        span
    ));
    result.generic_function_instantiations[1]
        .actor_message_calls
        .insert(span, packet.clone());
    assert!(!result.validates_actor_message_at(
        &packet,
        crate::CheckedActorMessageRoot::Generic(1),
        &[],
        span
    ));
    assert!(!result.validates_actor_message_at(
        &packet,
        crate::CheckedActorMessageRoot::Generic(0),
        &[(span, 0)],
        span
    ));
}

#[test]
fn actor_receipt_rejects_changed_original_body_with_all_source_spans_retained() {
    let source = "namespace app\nactor Store:\n    receive ping:\n        int64 number = 1\n        trace number\n";
    let (result, _) = checked(source, false);
    assert_accepted(&result);
    let handler = result.actor_handlers.values().next().unwrap();
    let mut parsed = jett_parser::parse(source, FileId::new(0));
    assert!(parsed.errors.is_empty());
    let actor = parsed
        .module
        .items
        .iter_mut()
        .find_map(|item| match item {
            Item::Actor(actor) => Some(actor),
            _ => None,
        })
        .unwrap();
    assert!(handler.matches_source_actor(actor));
    let original_span = actor.handlers[0].body.span;
    actor.handlers[0].body.stmts.reverse();
    assert_eq!(actor.handlers[0].body.span, original_span);
    assert!(
        handler.ownership_validated(),
        "the original receipt remains intact"
    );
    assert!(!handler.matches_source_actor(actor));
}
