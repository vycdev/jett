//! Checked assignment-place and ownership controls.

use super::{CheckOptions, CheckResult, check_with_options};
use crate::{
    CheckedAssignmentPlaceError, CheckedFieldOwner, CheckedFieldUpdateResourceRequirement,
    validate_assignment_place_shape,
};
use jett_common::{FileId, STDLIB_FILE_ID_START};
use jett_diagnostics::Severity;
use jett_types::Type;

const DECLARATIONS: &str = "namespace app\nstruct Inner:\n    value: int64\nstruct Record:\n    inner: Inner\n    label: string\nenum Choice:\n    wrapped(record: Record)\nfunction make() returns Record:\n    return Record(inner: Inner(value: 1), label: \"before\")\n";

mod actor_handler_tests;
mod metadata_tests;
mod ownership_tests;

fn checked(source: &str, file: FileId) -> CheckResult {
    let parsed = jett_parser::parse(source, file);
    assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
    let resolved = jett_resolve::resolve(&parsed.module);
    assert!(
        resolved
            .diagnostics
            .iter()
            .all(|diagnostic| diagnostic.severity != Severity::Error),
        "{:?}",
        resolved.diagnostics
    );
    check_with_options(&parsed.module, &resolved, CheckOptions { release: false })
}

fn ordinary(source: &str) -> CheckResult {
    checked(source, FileId::new(0))
}

fn assert_no_errors(result: &CheckResult) {
    assert!(
        result
            .diagnostics
            .iter()
            .all(|diagnostic| diagnostic.severity != Severity::Error),
        "{:?}",
        result.diagnostics
    );
}

#[test]
fn original_nested_integer_and_string_bodies_export_closed_places() {
    // Same two bodies as projected_assignment.rs, with separate metadata oracles.
    let source = format!(
        "{DECLARATIONS}function local() returns nothing:\n    mutable Record record = make()\n    (record).inner.value = 9\nfunction parameter(mutable record: Record) returns nothing:\n    record.label = \"after\"\n"
    );
    let result = ordinary(&source);
    assert_no_errors(&result);
    assert_eq!(result.assignment_places.len(), 2);
    let mut depths = result
        .assignment_places
        .values()
        .map(|place| {
            validate_assignment_place_shape(place, &result.interner).unwrap();
            assert_eq!(place.root.ty, place.root_type);
            assert_eq!(
                place.resource,
                CheckedFieldUpdateResourceRequirement::Ordinary
            );
            place.projections.len()
        })
        .collect::<Vec<_>>();
    depths.sort_unstable();
    assert_eq!(depths, [1, 2]);
}

#[test]
fn concrete_generic_assignments_do_not_share_a_span_only_map() {
    let result = ordinary(
        "namespace app\nstruct Slot[T]:\n    item: T\nfunction replace[T](mutable slot: Slot[T], next: T) returns nothing:\n    slot.item = next\nfunction callers() returns nothing:\n    replace[int64](Slot[int64](item: 1), 2)\n    replace[string](Slot[string](item: \"before\"), \"after\")\n",
    );
    assert_no_errors(&result);
    let mut places = result
        .generic_function_instantiations
        .iter()
        .flat_map(|body| body.assignment_places.values())
        .collect::<Vec<_>>();
    places.sort_by_key(|place| place.root.ty.index());
    assert_eq!(places.len(), 2);
    assert_eq!(places[0].assignment_span, places[1].assignment_span);
    assert_ne!(places[0].root.ty, places[1].root.ty);
    assert_ne!(
        places[0].projections[0].field_type,
        places[1].projections[0].field_type
    );
    for place in places {
        validate_assignment_place_shape(place, &result.interner).unwrap();
    }
}

#[test]
fn scoped_assignment_keeps_an_outer_root_in_its_binding_facts() {
    let source = format!(
        "{DECLARATIONS}function update(mutable record: Record) returns nothing:\n    comptime type Bound = type.info[int64]():\n        record.inner.value = 9\n"
    );
    let result = checked(&source, FileId::new(STDLIB_FILE_ID_START));
    assert_no_errors(&result);
    let scopes = result
        .comptime_type_bindings
        .values()
        .flatten()
        .collect::<Vec<_>>();
    assert_eq!(scopes.len(), 1);
    let body = &scopes[0].body;
    assert_eq!(body.assignment_places.len(), 1);
    let place = body.assignment_places.values().next().unwrap();
    assert_eq!(
        body.binding_facts.get(&place.root.declaration_span),
        Some(&place.root)
    );
    validate_assignment_place_shape(place, &result.interner).unwrap();
}

#[test]
fn narrowed_machine_keeps_declared_storage_and_current_state_separate() {
    let result = ordinary(
        "namespace app\nmachine Session:\n    states:\n        guest\n        logged_in(user_id: string)\n    transitions:\n        guest to logged_in\nfunction rename(mutable session: Session) returns nothing:\n    if session at logged_in:\n        session.user_id = \"after\"\n",
    );
    assert_no_errors(&result);
    assert_eq!(result.assignment_places.len(), 1);
    let place = result.assignment_places.values().next().unwrap();
    assert!(matches!(
        result.interner.resolve(place.root.ty),
        Type::Machine(_)
    ));
    let Type::MachineState { machine, state } = result.interner.resolve(place.root_type) else {
        panic!("missing exact narrowed state");
    };
    assert_eq!(
        place.projections[0].owner,
        CheckedFieldOwner::MachineState {
            machine: *machine,
            state: *state
        }
    );
    validate_assignment_place_shape(place, &result.interner).unwrap();
}

#[test]
fn secret_owner_retains_stored_type_and_qualified_access_type() {
    let source = format!(
        "{DECLARATIONS}function update(mutable record: secret[Record]) returns nothing:\n    record.inner.value = 9\n"
    );
    let result = ordinary(&source);
    assert_no_errors(&result);
    let place = result.assignment_places.values().next().unwrap();
    assert_eq!(place.projections.len(), 2);
    for projection in &place.projections {
        assert!(matches!(
            result.interner.resolve(projection.owner_type),
            Type::Secret(_)
        ));
        assert!(
            matches!(result.interner.resolve(projection.field_type), Type::Secret(inner)
            if *inner == projection.declared_field_type)
        );
    }
    validate_assignment_place_shape(place, &result.interner).unwrap();
}

#[test]
fn scalar_field_inside_resource_owner_still_requires_custody() {
    let result = checked(
        "namespace app\nresource Handle\nstruct Holder:\n    token: Handle\n    count: int64\nfunction change(mutable holder: Holder) returns nothing:\n    holder.count = 9\n",
        FileId::new(STDLIB_FILE_ID_START),
    );
    assert_no_errors(&result);
    assert!(result.resource_hooks.is_empty());
    let place = result.assignment_places.values().next().unwrap();
    assert_eq!(
        place.resource,
        CheckedFieldUpdateResourceRequirement::RequiresCustody
    );
    validate_assignment_place_shape(place, &result.interner).unwrap();
    let mut corrupted = place.clone();
    corrupted.resource = CheckedFieldUpdateResourceRequirement::Ordinary;
    assert_eq!(
        validate_assignment_place_shape(&corrupted, &result.interner),
        Err(CheckedAssignmentPlaceError::ResourceClassification)
    );
}

#[test]
fn exact_layout_rejects_changed_ordinals_types_and_root_source() {
    let source = format!(
        "{DECLARATIONS}function update(mutable record: Record) returns nothing:\n    record.inner.value = 9\n"
    );
    let result = ordinary(&source);
    assert_no_errors(&result);
    let place = result.assignment_places.values().next().unwrap();
    let mut ordinal = place.clone();
    ordinal.projections[1].field_index = usize::MAX;
    assert_eq!(
        validate_assignment_place_shape(&ordinal, &result.interner),
        Err(CheckedAssignmentPlaceError::FieldOrdinal)
    );
    let mut field = place.clone();
    field.projections[1].declared_field_type = jett_types::TypeInterner::STRING;
    assert_eq!(
        validate_assignment_place_shape(&field, &result.interner),
        Err(CheckedAssignmentPlaceError::FieldType)
    );
    let mut source = place.clone();
    source.root_use_span = source.root.declaration_span;
    assert_eq!(
        validate_assignment_place_shape(&source, &result.interner),
        Err(CheckedAssignmentPlaceError::SourceAssociation)
    );
}

#[test]
fn shape_export_does_not_override_use_after_move_diagnostics() {
    let source = format!(
        "{DECLARATIONS}function consume(record: Record) returns int64:\n    return record.inner.value\nfunction invalid() returns nothing:\n    mutable Record record = make()\n    int64 ignored = consume(record)\n    record.inner.value = 9\n"
    );
    let result = ordinary(&source);
    assert!(
        result
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code.code() == 400),
        "{:?}",
        result.diagnostics
    );
    // A shape fact can exist in a failed CheckResult. Consumers must retain
    // whole-program success and independent live-root validation prerequisites.
    assert_eq!(result.assignment_places.len(), 1);
}

#[test]
fn enclosing_refinements_bitfields_and_view_roots_export_no_place() {
    for body in [
        "function invalid(mutable record: Wrapped) returns nothing:\n    (coarsen record).inner.value = 9\n",
        "function invalid(view mutable record: Record) returns nothing:\n    record.inner.value = 9\n",
        "bitfield Bits:\n    count: 4 bits\nfunction invalid(mutable flags: Bits) returns nothing:\n    flags.count = 3\n",
    ] {
        let source = format!("{DECLARATIONS}type Wrapped = Record where true\n{body}");
        let result = ordinary(&source);
        assert!(result.assignment_places.is_empty());
    }
}

#[test]
fn refined_and_bitfield_owners_receive_static_policy_diagnostics() {
    for body in [
        "function invalid(mutable record: Wrapped) returns nothing:\n    record.inner.value = 9\n",
        "struct RefinedChild:\n    inner: Wrapped\nfunction invalid(mutable record: RefinedChild) returns nothing:\n    record.inner.inner.value = 9\n",
        "bitfield Bits:\n    count: 4 bits\nfunction invalid(mutable flags: Bits) returns nothing:\n    flags.count = 3\n",
    ] {
        let source = format!("{DECLARATIONS}type Wrapped = Record where true\n{body}");
        let result = ordinary(&source);
        let errors = result
            .diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.severity == Severity::Error)
            .collect::<Vec<_>>();
        assert_eq!(errors.len(), 1, "{:?}", result.diagnostics);
        assert_eq!(errors[0].code.code(), 404);
        assert!(result.assignment_places.is_empty());
    }
}

#[test]
fn complete_refined_and_bitfield_terminal_fields_remain_replaceable() {
    let source = "namespace app\ntype Positive = int64 where value > 0\nbitfield Bits:\n    count: 4 bits\nstruct Envelope:\n    count: Positive\n    flags: Bits\nfunction replace(mutable envelope: Envelope, count: Positive, flags: Bits) returns nothing:\n    envelope.count = count\n    envelope.flags = flags\n";
    let result = ordinary(source);
    assert_no_errors(&result);
    assert_eq!(result.assignment_places.len(), 2);
    for place in result.assignment_places.values() {
        assert_eq!(place.projections.len(), 1);
        validate_assignment_place_shape(place, &result.interner).unwrap();
    }
}

#[test]
fn whole_root_reassignment_clears_prior_nonzero_fact_after_rhs() {
    let result = ordinary(
        "namespace app\nfunction invalid(mutable divisor: int64) returns int64:\n    if divisor != 0:\n        divisor = 0\n        return 8 / divisor\n    return 0\n",
    );
    assert!(
        result
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.severity == Severity::Error),
        "{:?}",
        result.diagnostics
    );
}
