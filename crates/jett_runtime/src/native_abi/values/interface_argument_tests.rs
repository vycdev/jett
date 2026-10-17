use super::*;
use interface_conversion::NativeInterfaceConversion as C;

fn boxed_scalar(values: &mut NativeValues, owner: u64, depth: u64) -> u64 {
    // Identity metadata is independent of the redacted payload's display.
    let mut layout = b"JD\x01".to_vec();
    layout.extend_from_slice(&1_u32.to_le_bytes());
    layout.extend_from_slice(&0_u32.to_le_bytes());
    layout.extend_from_slice(&1_u32.to_le_bytes());
    layout.push(NativeDebugTag::Redacted as u8);
    values
        .interface_box(owner, 7, false, depth, &layout)
        .unwrap()
}

fn unbox(owner: u64) -> C {
    C::Unbox {
        concrete: owner,
        owned: false,
        nothing: false,
    }
}

fn input(bits: u64) -> NativeField {
    NativeField {
        bits,
        owned: true,
        pending_depth: 0,
    }
}

#[test]
fn interface_owner_preflight_rejects_exact_wrong_owner_without_conversion() {
    let mut values = NativeValues::default();
    let original = boxed_scalar(&mut values, 10, 0);
    let structs = values.structs.len();
    let bytes = values.bytes.len();
    values.allocation_budget = Some(0);
    assert_eq!(
        unbox(9).validate_owners(&values, input(original)),
        Err(INVALID_INTERFACE_ARGUMENT)
    );
    assert_eq!(values.structs.len(), structs);
    assert_eq!(values.bytes.len(), bytes);
    assert_eq!(values.allocation_budget, Some(0));
    assert_eq!(values.struct_field(original, 0).unwrap().bits, 10);
    values.allocation_budget = None;
    values.drop_value(original).unwrap();
    assert!(values.is_empty());
}

#[test]
fn interface_owner_preflight_matching_pending_payload_preserves_depth_and_ownership() {
    let mut values = NativeValues::default();
    let original = boxed_scalar(&mut values, 9, 2);
    values.allocation_budget = Some(0);
    assert_eq!(unbox(9).validate_owners(&values, input(original)), Ok(()));
    assert_eq!(values.owned_pending_depth(original), Ok(2));
    assert_eq!(values.allocation_budget, Some(0));
    values.allocation_budget = None;
    values.drop_value(original).unwrap();
    assert!(values.is_empty());
}

#[test]
fn interface_owner_preflight_nested_list_optional_catches_later_mismatch_without_clones() {
    let mut values = NativeValues::default();
    let matching = boxed_scalar(&mut values, 9, 0);
    let wrong = boxed_scalar(&mut values, 10, 0);
    let first = values.sum(SUM_SUCCESS, matching, true).unwrap();
    let second = values.sum(SUM_SUCCESS, wrong, true).unwrap();
    let list = values.new_list(true).unwrap();
    values.lists.get_mut(&list).unwrap().elements = vec![Some(first), Some(second)];
    let conversion = C::List(Box::new(C::Optional(Box::new(unbox(9)))));
    let inventory = (
        values.lists.len(),
        values.sums.len(),
        values.structs.len(),
        values.bytes.len(),
    );
    values.allocation_budget = Some(0);
    assert_eq!(
        conversion.validate_owners(&values, input(list)),
        Err(INVALID_INTERFACE_ARGUMENT)
    );
    assert_eq!(
        (
            values.lists.len(),
            values.sums.len(),
            values.structs.len(),
            values.bytes.len()
        ),
        inventory
    );
    assert_eq!(values.allocation_budget, Some(0));
    assert_eq!(values.struct_field(matching, 0).unwrap().bits, 9);
    assert_eq!(values.struct_field(wrong, 0).unwrap().bits, 10);
    values.allocation_budget = None;
    values.drop_value(list).unwrap();
    assert!(values.is_empty());
}

#[test]
fn interface_owner_preflight_skips_absent_and_inactive_owner_payloads() {
    let mut values = NativeValues::default();
    let absent = values.sum(SUM_FAILURE, 0, false).unwrap();
    let error = values.sum(SUM_FAILURE, 17, false).unwrap();
    assert_eq!(
        C::Optional(Box::new(unbox(9))).validate_owners(&values, input(absent)),
        Ok(())
    );
    assert_eq!(
        C::Result(Box::new(unbox(9)), Box::new(C::Copy { owned: false }))
            .validate_owners(&values, input(error)),
        Ok(())
    );
    values.drop_value(absent).unwrap();
    values.drop_value(error).unwrap();
    assert!(values.is_empty());
}

#[test]
fn interface_owner_preflight_unavailable_container_does_not_read_children() {
    let mut values = NativeValues::default();
    let source = values.new_list(true).unwrap();
    values.lists.get_mut(&source).unwrap().pending_depth = 1;
    values.lists.get_mut(&source).unwrap().elements = vec![Some(u64::MAX)];
    assert_eq!(
        C::List(Box::new(unbox(9))).validate_owners(&values, input(source)),
        Err(INVALID_INTERFACE_ARGUMENT)
    );
    values.lists.get_mut(&source).unwrap().elements.clear();
    values.drop_value(source).unwrap();
    assert!(values.is_empty());
}

#[test]
fn interface_owner_preflight_preserves_missing_handle_guard() {
    let values = NativeValues::default();
    assert_eq!(
        unbox(9).validate_owners(&values, input(u64::MAX)),
        Err(INVALID_STRUCT)
    );
    assert!(!C::Copy { owned: true }.requires_owner_validation());
    assert!(!C::FunctionAdapter { code: 123 }.requires_owner_validation());
}
