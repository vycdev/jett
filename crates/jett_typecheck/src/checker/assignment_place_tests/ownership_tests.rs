//! Ownership and dependent-loan controls for checked field replacements.

use super::{DECLARATIONS, assert_no_errors, ordinary};
use jett_diagnostics::Severity;

#[test]
fn temporary_call_view_and_terminal_return_admit_the_exact_place() {
    let source = format!(
        "{DECLARATIONS}function next(view record: Record) returns int64:\n    return record.inner.value + 1\nfunction update(mutable record: Record) returns Record:\n    record.inner.value = next(view record)\n    return record\nfunction early(mutable record: Record) returns Record:\n    optional[int64] absent = none\n    record.inner.value = absent handle:\n        return record\n    return record\n"
    );
    let result = ordinary(&source);
    assert_no_errors(&result);
    assert_eq!(result.assignment_places.len(), 2);
    assert!(
        result
            .assignment_places
            .values()
            .all(|place| place.ownership_validated())
    );
}

#[test]
fn rhs_consumption_and_rebinding_that_reach_commit_are_rejected() {
    for body in [
        "function invalid(mutable record: Record) returns nothing:\n    record.inner.value = consume(record)\n",
        "function invalid(mutable record: Record) returns nothing:\n    optional[int64] absent = none\n    record.inner.value = absent handle:\n        ((record)) = make()\n        default 7\n",
    ] {
        let source = format!(
            "{DECLARATIONS}function consume(record: Record) returns int64:\n    return record.inner.value\n{body}"
        );
        let result = ordinary(&source);
        assert!(
            result
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code.code() == 404
                    && diagnostic
                        .message
                        .contains("while evaluating its field replacement")),
            "{:?}",
            result.diagnostics
        );
        assert!(
            result
                .assignment_places
                .values()
                .all(|place| !place.ownership_validated())
        );
    }
}

#[test]
fn dependent_whole_and_field_aliases_do_not_authorize_root_replacement() {
    for alias in [
        "Record alias = view record",
        "Inner alias = view record.inner",
    ] {
        let source = format!(
            "{DECLARATIONS}function invalid(mutable record: Record) returns nothing:\n    {alias}\n    record.inner.value = 9\n    trace alias\n"
        );
        let result = ordinary(&source);
        assert!(
            result
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code.code() == 401
                    && diagnostic.message.contains("dependent local view")),
            "{:?}",
            result.diagnostics
        );
        assert!(
            result
                .assignment_places
                .values()
                .all(|place| !place.ownership_validated())
        );
    }
}

#[test]
fn independent_scalar_and_string_copies_do_not_create_root_loans() {
    let source = format!(
        "{DECLARATIONS}function update(mutable record: Record) returns string:\n    int64 copied = record.inner.value\n    string label = record.label\n    record.inner.value = copied + 1\n    record.label = \"after\"\n    return label\n"
    );
    let result = ordinary(&source);
    assert_no_errors(&result);
    assert_eq!(result.assignment_places.len(), 2);
    assert!(
        result
            .assignment_places
            .values()
            .all(|place| place.ownership_validated())
    );
}

#[test]
fn verify_property_and_inline_owned_roots_receive_real_admission() {
    let source = format!(
        "{DECLARATIONS}function factory() returns function(Record) returns Record:\n    return function(mutable record: Record) returns Record:\n        record.inner.value = 9\n        return record\nverify updated:\n    mutable Record record = make()\n    record.inner.value = 9\n    assert record.inner.value == 9\nproperty updated_from_given:\n    given original: Record\n    mutable Record record = clone original\n    record.inner.value = 9\n    assert record.inner.value == 9\n"
    );
    let result = ordinary(&source);
    assert_no_errors(&result);
    assert_eq!(result.assignment_places.len(), 3);
    assert!(
        result
            .assignment_places
            .values()
            .all(|place| place.ownership_validated())
    );
}

#[test]
fn any_source_error_prevents_private_admission_even_after_diagnostic_editing() {
    let source = format!(
        "{DECLARATIONS}function valid(mutable record: Record) returns nothing:\n    record.inner.value = 9\nfunction invalid() returns nothing:\n    Record record = make()\n    record.inner.value = 9\n"
    );
    let mut result = ordinary(&source);
    assert!(
        result
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.severity == Severity::Error)
    );
    result.diagnostics.clear();
    assert!(
        result
            .assignment_places
            .values()
            .all(|place| !place.ownership_validated())
    );
}

#[test]
fn actor_observation_supplies_no_assignment_ownership_authority() {
    let source = format!(
        "{DECLARATIONS}actor Store:\n    mutable app.Record record = make()\n    receive replace:\n        record.inner.value = 9\n"
    );
    let result = ordinary(&source);
    assert_no_errors(&result);
    assert_eq!(result.assignment_places.len(), 1);
    assert!(
        result
            .assignment_places
            .values()
            .all(|place| !place.ownership_validated())
    );
}

#[test]
fn private_admission_is_distinct_in_same_span_concrete_generic_contexts() {
    let result = ordinary(
        "namespace app\nstruct Slot[T]:\n    item: T\nfunction replace[T](mutable slot: Slot[T], next: T) returns nothing:\n    slot.item = next\nfunction callers() returns nothing:\n    replace[int64](Slot[int64](item: 1), 2)\n    replace[string](Slot[string](item: \"before\"), \"after\")\n",
    );
    assert_no_errors(&result);
    let places = result
        .generic_function_instantiations
        .iter()
        .flat_map(|body| body.assignment_places.values())
        .collect::<Vec<_>>();
    assert_eq!(places.len(), 2);
    assert_eq!(places[0].assignment_span, places[1].assignment_span);
    assert_ne!(places[0].root.ty, places[1].root.ty);
    assert!(places.iter().all(|place| place.ownership_validated()));
}

#[test]
fn an_unvisited_same_span_context_cannot_inherit_a_private_stamp() {
    use crate::assignment_places::{
        AssignmentContext, AssignmentContextRoot, AssignmentSite, seal_assignment_ownership,
    };
    use std::collections::{HashMap, HashSet};

    let source = format!(
        "{DECLARATIONS}function update(mutable record: Record) returns nothing:\n    record.inner.value = 9\n"
    );
    let result = ordinary(&source);
    assert_no_errors(&result);
    let place = result.assignment_places.values().next().unwrap().clone();
    let span = place.assignment_span;
    let first = AssignmentContext {
        root: AssignmentContextRoot::Generic(0),
        scopes: Vec::new(),
    };
    let second = AssignmentContext {
        root: AssignmentContextRoot::Generic(1),
        scopes: Vec::new(),
    };
    let validated = HashSet::from([AssignmentSite {
        context: first.clone(),
        span,
    }]);
    let mut first_places = HashMap::from([(span, place.clone())]);
    let mut second_places = HashMap::from([(span, place)]);
    seal_assignment_ownership(&mut first_places, &mut HashMap::new(), &first, &validated);
    seal_assignment_ownership(&mut second_places, &mut HashMap::new(), &second, &validated);
    assert!(first_places[&span].ownership_validated());
    assert!(!second_places[&span].ownership_validated());
}

#[test]
fn changing_a_same_typed_sibling_ordinal_invalidates_private_admission() {
    let result = ordinary(
        "namespace app\nstruct Pair:\n    first: int64\n    second: int64\nfunction update(mutable pair: Pair) returns nothing:\n    pair.first = 9\n",
    );
    assert_no_errors(&result);
    let place = result.assignment_places.values().next().unwrap();
    assert!(place.ownership_validated());
    let mut changed = place.clone();
    changed.projections[0].field_index = 1;
    crate::validate_assignment_place_shape(&changed, &result.interner).unwrap();
    assert!(!changed.ownership_validated());
    assert!(place.ownership_validated());
}

#[test]
fn active_view_iteration_borrows_the_enclosing_owned_root() {
    let result = ordinary(
        "namespace app\nstruct Group:\n    items: list[int64]\n    count: int64\nfunction invalid(mutable group: Group) returns nothing:\n    for item in view group.items:\n        group.count = item\n",
    );
    assert!(
        result
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code.code() == 401
                && diagnostic.message.contains("dependent local view")),
        "{:?}",
        result.diagnostics
    );
    assert!(
        result
            .assignment_places
            .values()
            .all(|place| !place.ownership_validated())
    );
}

#[test]
fn completed_view_iteration_does_not_leave_a_phantom_loop_loan() {
    let result = ordinary(
        "namespace app\nstruct Group:\n    items: list[int64]\n    count: int64\nfunction update(mutable group: Group) returns nothing:\n    mutable int64 total = 0\n    for item in view group.items:\n        total = total + item\n    group.count = total\n",
    );
    assert_no_errors(&result);
    assert_eq!(result.assignment_places.len(), 1);
    assert!(
        result
            .assignment_places
            .values()
            .all(|place| place.ownership_validated())
    );
}

#[test]
fn borrowed_match_payload_retains_the_enclosing_owned_root() {
    let result = ordinary(
        "namespace app\nstruct Inner:\n    count: int64\nenum Choice:\n    item(data: Inner)\nstruct Envelope:\n    choice: Choice\n    count: int64\nfunction invalid(mutable envelope: Envelope) returns nothing:\n    match view envelope.choice:\n        item(data):\n            envelope.count = 9\n            trace data\n",
    );
    assert!(
        result
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code.code() == 401
                && diagnostic.message.contains("dependent local view")),
        "{:?}",
        result.diagnostics
    );
    assert!(
        result
            .assignment_places
            .values()
            .all(|place| !place.ownership_validated())
    );
}

#[test]
fn transparent_borrowed_matches_retain_the_enclosing_owned_root() {
    for (declaration, choice, scrutinee) in [
        ("", "Choice", "(view envelope.choice)"),
        ("", "secret[Choice]", "declassify(view envelope.choice)"),
        (
            "type RefinedChoice = Choice where true\n",
            "RefinedChoice",
            "coarsen(view envelope.choice)",
        ),
    ] {
        let source = format!(
            "namespace app\nstruct Inner:\n    count: int64\nenum Choice:\n    item(data: Inner)\n{declaration}struct Envelope:\n    choice: {choice}\n    count: int64\nfunction invalid(mutable envelope: Envelope) returns nothing:\n    match {scrutinee}:\n        item(data):\n            envelope.count = 9\n            trace data\n"
        );
        let result = ordinary(&source);
        let errors = result
            .diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.severity == Severity::Error)
            .collect::<Vec<_>>();
        assert!(!errors.is_empty(), "{source}");
        assert!(
            errors.iter().all(|diagnostic| diagnostic.code.code() == 401
                && diagnostic.message.contains("dependent local view")),
            "{source}\n{errors:?}"
        );
        assert_eq!(result.assignment_places.len(), 1);
        assert!(
            result
                .assignment_places
                .values()
                .all(|place| !place.ownership_validated())
        );
    }
}

#[test]
fn transparent_borrowed_match_copyable_payloads_remain_independent() {
    for payload in ["int64", "string"] {
        for (declaration, choice, scrutinee) in [
            ("", "Choice", "(view envelope.choice)"),
            ("", "secret[Choice]", "declassify(view envelope.choice)"),
            (
                "type RefinedChoice = Choice where true\n",
                "RefinedChoice",
                "coarsen(view envelope.choice)",
            ),
        ] {
            let source = format!(
                "namespace app\nenum Choice:\n    item(data: {payload})\n{declaration}struct Envelope:\n    choice: {choice}\n    count: int64\nfunction update(mutable envelope: Envelope) returns nothing:\n    match {scrutinee}:\n        item(data):\n            envelope.count = 9\n            trace data\n"
            );
            let result = ordinary(&source);
            assert_no_errors(&result);
            assert_eq!(result.assignment_places.len(), 1);
            assert!(
                result
                    .assignment_places
                    .values()
                    .all(|place| place.ownership_validated())
            );
        }
    }
}

#[test]
fn cloning_a_transparent_match_borrow_keeps_the_new_payload_independent() {
    for (declaration, choice, scrutinee) in [
        ("", "Choice", "(view envelope.choice)"),
        ("", "secret[Choice]", "declassify(view envelope.choice)"),
        (
            "type RefinedChoice = Choice where true\n",
            "RefinedChoice",
            "coarsen(view envelope.choice)",
        ),
    ] {
        let source = format!(
            "namespace app\nstruct Inner:\n    count: int64\nenum Choice:\n    item(data: Inner)\n{declaration}struct Envelope:\n    choice: {choice}\n    count: int64\nfunction update(mutable envelope: Envelope) returns nothing:\n    match clone ({scrutinee}):\n        item(data):\n            envelope.count = 9\n            trace data\n"
        );
        let result = ordinary(&source);
        assert_no_errors(&result);
        assert_eq!(result.assignment_places.len(), 1);
        assert!(
            result
                .assignment_places
                .values()
                .all(|place| place.ownership_validated())
        );
    }
}

#[test]
fn pending_owned_child_is_transferred_without_becoming_a_source_rejection() {
    let declarations = "namespace app\nstruct Inner:\n    value: int64\nstruct Holder:\n    child: Inner\nfunction make_inner() returns Inner:\n    return Inner(value: 1)\n";
    let accepted = ordinary(&format!(
        "{declarations}function update(mutable holder: Holder) returns Holder:\n    Inner incoming = run make_inner()\n    holder.child = incoming\n    return holder\n"
    ));
    assert_no_errors(&accepted);
    assert_eq!(accepted.assignment_places.len(), 1);
    assert!(
        accepted
            .assignment_places
            .values()
            .all(|place| place.ownership_validated())
    );
    let rejected = ordinary(&format!(
        "{declarations}function invalid(mutable holder: Holder) returns nothing:\n    Inner incoming = run make_inner()\n    holder.child = incoming\n    trace incoming\n"
    ));
    assert!(
        rejected
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code.code() == 400),
        "{:?}",
        rejected.diagnostics
    );
    assert!(
        rejected
            .assignment_places
            .values()
            .all(|place| !place.ownership_validated())
    );
}
