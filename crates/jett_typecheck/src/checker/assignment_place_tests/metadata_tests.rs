//! Compiler metadata identity and field-replacement authority controls.

use super::{assert_no_errors, ordinary};
use crate::{CheckedAssignmentPlaceError, checked_field_owner};
use jett_diagnostics::Severity;

#[test]
fn each_exact_builtin_metadata_owner_rejects_projected_mutation() {
    for (owner, field, value) in [
        ("TypeInfo", "type_name", "\"forged\""),
        ("TypeField", "index", "1"),
        ("TypeBitfield", "network_order", "true"),
        ("TypeBitfieldField", "width", "1"),
        ("TypeMachine", "states", "metadata.states"),
        ("TypeMachineState", "index", "1"),
        ("TypeMachineTransition", "source_index", "1"),
        ("TypeVariant", "index", "1"),
    ] {
        let source = format!(
            "namespace app\nfunction invalid(mutable metadata: {owner}) returns nothing:\n    metadata.{field} = {value}\n"
        );
        let result = ordinary(&source);
        let errors = result
            .diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.severity == Severity::Error)
            .collect::<Vec<_>>();
        assert_eq!(errors.len(), 1, "{source}: {:?}", result.diagnostics);
        assert_eq!(errors[0].code.code(), 404);
        assert!(
            errors[0]
                .message
                .contains("compiler-produced reflection metadata")
        );
        assert!(result.assignment_places.is_empty());
    }
}

#[test]
fn aliases_secret_qualification_and_user_holders_preserve_metadata_barrier() {
    for body in [
        "function invalid(mutable field: FieldAlias) returns nothing:\n    field.index = 1\n",
        "function invalid(mutable field: secret[FieldAlias]) returns nothing:\n    field.index = 1\n",
        "function invalid(mutable holder: Holder) returns nothing:\n    holder.field.index = 1\n",
    ] {
        let source = format!(
            "namespace app\ntype FieldAlias = TypeField\nstruct Holder:\n    field: FieldAlias\n{body}"
        );
        let result = ordinary(&source);
        assert!(
            result
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code.code() == 404
                    && diagnostic
                        .message
                        .contains("compiler-produced reflection metadata")),
            "{source}: {:?}",
            result.diagnostics
        );
        assert!(result.assignment_places.is_empty());
    }
}

#[test]
fn replacing_a_complete_genuine_metadata_field_does_not_enter_it() {
    let result = ordinary(
        "namespace app\nstruct Holder:\n    field: TypeField\nfunction replace(mutable holder: Holder, incoming: TypeField) returns nothing:\n    holder.field = incoming\n",
    );
    assert_no_errors(&result);
    assert_eq!(result.assignment_places.len(), 1);
    let place = result.assignment_places.values().next().unwrap();
    assert_eq!(place.projections.len(), 1);
    assert!(place.ownership_validated());
    let terminal = place.projections[0].declared_field_type;
    assert_eq!(
        checked_field_owner(&result.interner, terminal),
        Err(CheckedAssignmentPlaceError::CompilerMetadataOwner)
    );
}

#[test]
fn unrelated_user_lookalike_type_names_remain_ordinary_nominal_owners() {
    let result = ordinary(
        "namespace app\nstruct TypeField:\n    index: int64\nfunction make() returns app.TypeField:\n    return app.TypeField(index: 0)\nfunction update(mutable field: app.TypeField) returns app.TypeField:\n    field.index = 1\n    return field\nfunction main() returns int64:\n    app.TypeField field = update(make())\n    return field.index\n",
    );
    assert_no_errors(&result);
    assert_eq!(result.assignment_places.len(), 1);
    let place = result.assignment_places.values().next().unwrap();
    assert!(checked_field_owner(&result.interner, place.root_type).is_ok());
    assert!(place.ownership_validated());
}

#[test]
fn canonical_metadata_marker_survives_snapshot_revalidation() {
    let result = ordinary(
        "namespace app\nstruct Holder:\n    field: TypeField\nfunction replace(mutable holder: Holder, incoming: TypeField) returns nothing:\n    holder.field = incoming\n",
    );
    assert_no_errors(&result);
    let place = result.assignment_places.values().next().unwrap();
    let snapshot = result.interner.snapshot();
    assert_eq!(
        checked_field_owner(&snapshot, place.projections[0].declared_field_type),
        Err(CheckedAssignmentPlaceError::CompilerMetadataOwner)
    );
    crate::validate_assignment_place_shape(place, &snapshot).unwrap();
}

#[test]
fn canonical_builtin_constructors_and_aliases_keep_the_existing_refusal() {
    for owner in [
        "TypeInfo",
        "TypeField",
        "TypeBitfield",
        "TypeBitfieldField",
        "TypeMachine",
        "TypeMachineState",
        "TypeMachineTransition",
        "TypeVariant",
    ] {
        for constructor in [owner.to_owned(), "Alias".to_owned()] {
            let source = format!(
                "namespace app\ntype Alias = {owner}\nfunction invalid() returns nothing:\n    {owner} metadata = {constructor}()\n"
            );
            let result = ordinary(&source);
            assert!(
                result
                    .diagnostics
                    .iter()
                    .any(|diagnostic| diagnostic.code.code() == 354),
                "{source}: {:?}",
                result.diagnostics
            );
        }
    }
}

#[test]
fn a_genuine_reflected_field_still_cannot_be_edited_after_construction_gate() {
    let result = ordinary(
        "namespace app\nstruct Record:\n    first: int64\n    second: int64\nfunction invalid() returns nothing:\n    list[TypeField] fields = type.fields[Record]()\n    mutable TypeField field = list.get[TypeField](view fields, 0) handle:\n        return nothing\n    field.index = 1\n",
    );
    assert!(
        result
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code.code() == 404
                && diagnostic
                    .message
                    .contains("compiler-produced reflection metadata")),
        "{:?}",
        result.diagnostics
    );
    assert!(result.assignment_places.is_empty());
}

#[test]
fn extending_a_sealed_holder_path_into_metadata_fails_independent_validation() {
    use crate::{CheckedFieldOwner, CheckedFieldProjection};
    use jett_types::{Type, TypeInterner};

    let result = ordinary(
        "namespace app\nstruct Holder:\n    field: TypeField\nfunction replace(mutable holder: Holder, incoming: TypeField) returns nothing:\n    holder.field = incoming\n",
    );
    assert_no_errors(&result);
    let place = result.assignment_places.values().next().unwrap();
    let metadata = place.projections[0].declared_field_type;
    let Type::Struct(owner) = result.interner.resolve(metadata) else {
        panic!("canonical metadata is a nominal struct");
    };
    let mut forged = place.clone();
    forged.projections.push(CheckedFieldProjection {
        source_span: place.projections[0].source_span,
        owner_type: metadata,
        owner: CheckedFieldOwner::Struct(*owner),
        field_index: 0,
        declared_field_type: TypeInterner::INT64,
        field_type: TypeInterner::INT64,
    });
    assert!(!forged.ownership_validated());
    assert_eq!(
        crate::validate_assignment_place_shape(&forged, &result.interner),
        Err(CheckedAssignmentPlaceError::CompilerMetadataOwner)
    );
}
