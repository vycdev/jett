//! Actor entry identity, real exits, persistent loans and loop ownership.
use super::{assert_no_errors, ordinary};
use crate::{
    CheckedActorBindingRole, CheckedActorHandlerExitKind, CheckedBindingMode, CheckedViewSource,
};
use jett_diagnostics::Severity;

const TYPES: &str = "namespace app\nstruct Inner:\n    value: int64\n";

fn has_error(result: &super::CheckResult, code: u16) -> bool {
    result
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.severity == Severity::Error && diagnostic.code.code() == code)
}

#[test]
fn actor_owned_field_paths_have_private_handler_context_and_exact_roles() {
    let result = ordinary(&format!(
        "{TYPES}actor Store:\n    mutable Inner inner = Inner(value: 1)\n    receive replace(value: int64):\n        inner.value = value\n    receive read responds int64:\n        respond inner.value\n"
    ));
    assert_no_errors(&result);
    assert_eq!(result.actor_handlers.len(), 2);
    for handler in result.actor_handlers.values() {
        assert!(handler.ownership_validated());
        assert_eq!(
            handler
                .bindings()
                .iter()
                .filter(|binding| matches!(binding.role, CheckedActorBindingRole::State { .. }))
                .count(),
            1
        );
        assert!(!handler.exits().is_empty());
        assert!(
            handler
                .body()
                .assignment_places
                .values()
                .all(|place| place.ownership_validated())
        );
    }
    assert_eq!(result.assignment_places.len(), 1);
    assert!(
        result
            .assignment_places
            .values()
            .all(|place| !place.ownership_validated())
    );
}

#[test]
fn actor_message_cannot_shadow_an_existing_state_binding() {
    let source = format!(
        "{TYPES}actor Store:\n    mutable Inner inner = Inner(value: 1)\n    receive replace(mutable inner: Inner):\n        inner.value = 9\n"
    );
    let parsed = jett_parser::parse(&source, jett_common::FileId::new(0));
    assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
    let resolved = jett_resolve::resolve(&parsed.module);
    assert!(
        resolved
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.severity == Severity::Error
                && diagnostic.code.code() == 201),
        "{:?}",
        resolved.diagnostics
    );
}

#[test]
fn actor_move_then_restore_is_allowed_but_unrestored_actual_exits_reject() {
    let result = ordinary(&format!(
        "{TYPES}actor Store:\n    mutable Inner inner = Inner(value: 1)\n    receive replace responds Inner:\n        Inner old = inner\n        inner = Inner(value: 9)\n        respond old\n"
    ));
    assert_no_errors(&result);
    assert!(
        result
            .actor_handlers
            .values()
            .all(|handler| handler.ownership_validated())
    );
    for body in [
        "        respond inner\n",
        "        Inner old = inner\n",
        "        Inner old = inner\n        if true:\n            respond 1\n        inner = Inner(value: 9)\n        respond 2\n",
    ] {
        let response = if body == "        respond inner\n" {
            " responds Inner"
        } else if body.contains("respond") {
            " responds int64"
        } else {
            ""
        };
        let result = ordinary(&format!(
            "{TYPES}actor Store:\n    mutable Inner inner = Inner(value: 1)\n    receive invalid{response}:\n{body}"
        ));
        assert!(has_error(&result, 400), "{:?}", result.diagnostics);
        assert!(
            result
                .actor_handlers
                .values()
                .all(|handler| !handler.ownership_validated())
        );
    }
}

#[test]
fn actor_early_respond_abandons_rhs_commit_and_validates_captures() {
    let result = ordinary(&format!(
        "{TYPES}actor Store:\n    mutable Inner inner = Inner(value: 1)\n    receive update responds int64:\n        optional[int64] absent = none\n        inner.value = absent handle:\n            respond inner.value\n        respond inner.value\n"
    ));
    assert_no_errors(&result);
    let handler = result.actor_handlers.values().next().unwrap();
    assert!(handler.ownership_validated());
    assert_eq!(
        handler
            .exits()
            .iter()
            .filter(|exit| exit.kind == CheckedActorHandlerExitKind::Respond)
            .count(),
        2
    );
    assert!(
        handler
            .body()
            .assignment_places
            .values()
            .all(|place| place.ownership_validated())
    );
    let invalid = ordinary(&format!(
        "{TYPES}actor Store:\n    mutable Inner inner = Inner(value: 1)\n    receive update responds Inner:\n        optional[int64] absent = none\n        inner.value = absent handle:\n            respond inner\n        respond Inner(value: 9)\n"
    ));
    assert!(has_error(&invalid, 400), "{:?}", invalid.diagnostics);
}

#[test]
fn actor_break_and_continue_keep_real_state_and_ignore_unreachable_restores() {
    for jump in ["break", "continue"] {
        let result = ordinary(&format!(
            "{TYPES}actor Store:\n    mutable Inner inner = Inner(value: 1)\n    receive update:\n        for item in list(1, 2):\n            Inner old = inner\n            {jump}\n            inner = Inner(value: 9)\n"
        ));
        assert!(has_error(&result, 400), "{jump}: {:?}", result.diagnostics);
        assert!(
            result
                .actor_handlers
                .values()
                .all(|handler| !handler.ownership_validated())
        );
    }
    let result = ordinary(&format!(
        "{TYPES}actor Store:\n    mutable Inner inner = Inner(value: 1)\n    receive update:\n        for item in list(1, 2):\n            Inner old = inner\n            inner = Inner(value: item)\n            continue\n"
    ));
    assert_no_errors(&result);
    assert!(
        result
            .actor_handlers
            .values()
            .all(|handler| handler.ownership_validated())
    );
    let restored_after_break = ordinary(&format!(
        "{TYPES}actor Store:\n    mutable Inner inner = Inner(value: 1)\n    receive update:\n        for item in list(1, 2):\n            Inner old = inner\n            break\n        inner = Inner(value: 9)\n"
    ));
    assert_no_errors(&restored_after_break);
    assert!(
        restored_after_break
            .actor_handlers
            .values()
            .all(|handler| handler.ownership_validated())
    );
}

#[test]
fn actor_loop_backedge_rechecks_availability_before_repeated_reads() {
    for statement in [
        "        for item in list(1, 2):\n            Inner old = inner\n        inner = Inner(value: 9)\n",
        "        while true:\n            Inner old = inner\n        inner = Inner(value: 9)\n",
    ] {
        let result = ordinary(&format!(
            "{TYPES}actor Store:\n    mutable Inner inner = Inner(value: 1)\n    receive update:\n{statement}"
        ));
        assert!(has_error(&result, 400), "{:?}", result.diagnostics);
    }
}

#[test]
fn actor_persistent_view_preserves_prior_state_source_and_owner_loan() {
    let prefix = format!(
        "{TYPES}actor Store:\n    mutable Inner inner = Inner(value: 1)\n    Inner alias = view inner\n"
    );
    let result = ordinary(&format!(
        "{prefix}    receive read responds int64:\n        respond alias.value\n"
    ));
    assert_no_errors(&result);
    let handler = result.actor_handlers.values().next().unwrap();
    assert!(handler.ownership_validated());
    let owner = handler
        .bindings()
        .iter()
        .find(|binding| binding.role == CheckedActorBindingRole::State { field: 0 })
        .unwrap();
    let alias = handler
        .bindings()
        .iter()
        .find(|binding| binding.role == CheckedActorBindingRole::State { field: 1 })
        .unwrap();
    assert_eq!(
        alias.fact.mode,
        CheckedBindingMode::View {
            source: CheckedViewSource::Binding(owner.fact.definition),
        }
    );
    for statement in [
        "inner.value = 9",
        "inner = Inner(value: 9)",
        "Inner old = inner",
    ] {
        let result = ordinary(&format!(
            "{prefix}    receive invalid:\n        {statement}\n"
        ));
        assert!(
            has_error(&result, 401),
            "{statement}: {:?}",
            result.diagnostics
        );
    }
}

#[test]
fn actor_response_views_reject_including_written_copy_views() {
    for (response, value) in [("Inner", "view inner"), ("int64", "view inner.value")] {
        let result = ordinary(&format!(
            "{TYPES}actor Store:\n    mutable Inner inner = Inner(value: 1)\n    receive invalid responds {response}:\n        respond {value}\n"
        ));
        assert!(has_error(&result, 401), "{:?}", result.diagnostics);
    }
    let owned_copy = ordinary(&format!(
        "{TYPES}actor Store:\n    mutable Inner inner = Inner(value: 1)\n    receive read responds int64:\n        respond inner.value\n"
    ));
    assert_no_errors(&owned_copy);
    assert!(
        owned_copy
            .actor_handlers
            .values()
            .all(|handler| handler.ownership_validated())
    );
}

#[test]
fn actor_return_and_missing_reply_stay_unsealed_without_new_source_ban() {
    for body in [
        "        return 1\n",
        "        int64 observed = inner.value\n",
    ] {
        let result = ordinary(&format!(
            "{TYPES}actor Store:\n    mutable Inner inner = Inner(value: 1)\n    receive future responds int64:\n{body}"
        ));
        assert_no_errors(&result);
        let handler = result.actor_handlers.values().next().unwrap();
        assert!(!handler.ownership_validated());
        assert_eq!(handler.exits().len(), 1);
    }
}

#[test]
fn actor_transparent_copy_replies_observe_borrowed_and_persistent_carriers() {
    for (declaration, carrier, expression) in [
        ("", "secret[int64]", "declassify hidden"),
        ("type Count = int64 where true\n", "Count", "coarsen hidden"),
    ] {
        let borrowed = ordinary(&format!(
            "{TYPES}{declaration}actor Store:\n    receive read(view hidden: {carrier}) responds int64:\n        respond {expression}\n"
        ));
        assert_no_errors(&borrowed);
        assert!(
            borrowed
                .actor_handlers
                .values()
                .all(|handler| handler.ownership_validated())
        );
        if !declaration.is_empty() {
            continue;
        }
        let persistent = ordinary(&format!(
            "{TYPES}{declaration}actor Store:\n    {carrier} hidden = 1\n    receive read responds int64:\n        respond {expression}\n"
        ));
        assert_no_errors(&persistent);
        assert!(
            persistent
                .actor_handlers
                .values()
                .all(|handler| handler.ownership_validated())
        );
    }
}

#[test]
fn actor_no_response_void_returns_have_checked_exit_receipts() {
    for operand in ["", " nothing", " finish()"] {
        let result = ordinary(&format!(
            "{TYPES}function finish() returns nothing:\n    return nothing\nactor Store:\n    mutable Inner inner = Inner(value: 1)\n    receive update:\n        inner.value = 9\n        return{operand}\n"
        ));
        assert_no_errors(&result);
        let handler = result.actor_handlers.values().next().unwrap();
        assert!(handler.ownership_validated());
        assert_eq!(handler.exits().len(), 1);
        assert_eq!(handler.exits()[0].kind, CheckedActorHandlerExitKind::Return);
        assert!(
            handler
                .body()
                .assignment_places
                .values()
                .all(|place| place.ownership_validated())
        );
    }
    let restored = ordinary(&format!(
        "{TYPES}actor Store:\n    mutable Inner inner = Inner(value: 1)\n    receive update:\n        Inner old = inner\n        inner = Inner(value: 9)\n        return nothing\n"
    ));
    assert_no_errors(&restored);
    assert!(
        restored
            .actor_handlers
            .values()
            .all(|handler| handler.ownership_validated())
    );
}

#[test]
fn actor_void_return_still_checks_actual_operand_effects_and_persistent_state() {
    for body in [
        "        Inner old = inner\n        return nothing\n",
        "        return discard(inner)\n",
        "        optional[int64] missing = none\n        inner.value = missing handle:\n            return discard(inner)\n",
    ] {
        let result = ordinary(&format!(
            "{TYPES}function discard(value: Inner) returns nothing:\n    return nothing\nactor Store:\n    mutable Inner inner = Inner(value: 1)\n    receive update:\n{body}"
        ));
        assert!(has_error(&result, 400), "{:?}", result.diagnostics);
        assert!(!has_error(&result, 404), "{:?}", result.diagnostics);
        assert!(
            result
                .actor_handlers
                .values()
                .all(|handler| !handler.ownership_validated())
        );
    }
}

#[test]
fn actor_void_receipt_does_not_admit_values_views_or_responding_returns() {
    for (header, body) in [
        ("receive future:", "return 1"),
        ("receive future:", "return view nothing"),
        ("receive future responds int64:", "return 1"),
        ("receive future responds nothing:", "return nothing"),
        (
            "receive future responds nothing:",
            "int64 observed = inner.value",
        ),
    ] {
        let result = ordinary(&format!(
            "{TYPES}actor Store:\n    mutable Inner inner = Inner(value: 1)\n    {header}\n        {body}\n"
        ));
        assert_no_errors(&result);
        let handler = result.actor_handlers.values().next().unwrap();
        assert!(!handler.ownership_validated());
        assert_eq!(handler.exits().len(), 1);
    }
}
