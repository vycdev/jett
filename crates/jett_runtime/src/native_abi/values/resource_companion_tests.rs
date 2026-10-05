//! Malformed-host controls for the actual typed Resource failure-companion drop.
use super::*;
use crate::resource_custody::{NativeLayoutInstallation, RegisteredNativeLayout};
use std::sync::Arc;

fn layout() -> Arc<RegisteredNativeLayout> {
    let mut bytes = b"JTRSC001".to_vec();
    let words = |bytes: &mut Vec<u8>, values: &[u32]| {
        for value in values {
            bytes.extend_from_slice(&value.to_le_bytes());
        }
    };
    words(&mut bytes, &[2, 0]);
    bytes.extend_from_slice(&0u64.to_le_bytes());
    words(&mut bytes, &[0, 0, 1, 8, 1, 0, 0, 0]);
    words(&mut bytes, &[0, 0, 0]); // pure Nothing signature
    for row in [
        vec![0, 5],
        vec![1, 1, 64, 1],
        vec![2, 4],
        vec![3, 8, 1],
        vec![4, 8, 2],
        vec![5, 8, 4],
        vec![6, 9, 1, 4],
        vec![7, 9, 4, 1],
    ] {
        words(&mut bytes, &row);
    }
    words(&mut bytes, &[0, 1, 0, 0, 0, 0, 0, 1, 0, 0]);
    let length = bytes.len() as u64;
    bytes[16..24].copy_from_slice(&length.to_le_bytes());
    let registry = ResourceRegistry::new();
    NativeLayoutInstallation::new(&registry)
        .install(&registry, &bytes)
        .unwrap()
}

#[test]
fn resource_ordinary_companion_owned_integer_cannot_drop_an_unrelated_live_string() {
    let layout = layout();
    let mut values = NativeValues::default();
    let unrelated = values.insert("unrelated owner".into()).unwrap();
    let malformed = values.sum(SUM_SUCCESS, unrelated, true).unwrap();
    assert!(
        values
            .validate_resource_ordinary(malformed, &layout, 3)
            .is_err()
    );
    assert!(
        values
            .drop_resource_typed_companion(&layout, 3, malformed)
            .is_err()
    );
    assert!(values.strings.contains_key(&unrelated));
    assert!(values.sums.contains_key(&malformed));
    values.sums.get_mut(&malformed).unwrap().owned = false;
    values
        .drop_resource_typed_companion(&layout, 3, malformed)
        .unwrap();
    assert!(values.strings.contains_key(&unrelated));
    values.drop_value(unrelated).unwrap();
    assert!(values.is_empty());
}

#[test]
fn resource_ordinary_companion_nested_failure_requires_each_immediate_owner_bit() {
    let layout = layout();
    for missing_owner in 0..2 {
        let mut values = NativeValues::default();
        let text = values.insert("failure payload".into()).unwrap();
        let inner = values.sum(SUM_SUCCESS, text, missing_owner != 0).unwrap();
        let outer = values.sum(SUM_FAILURE, inner, missing_owner != 1).unwrap();
        assert!(
            values
                .validate_resource_ordinary(outer, &layout, 6)
                .is_err()
        );
        assert!(
            values
                .drop_resource_typed_companion(&layout, 6, outer)
                .is_err()
        );
        assert!(values.strings.contains_key(&text));
        assert!(values.sums.contains_key(&inner) && values.sums.contains_key(&outer));
        values.sums.get_mut(&inner).unwrap().owned = true;
        values.sums.get_mut(&outer).unwrap().owned = true;
        values
            .drop_resource_typed_companion(&layout, 6, outer)
            .unwrap();
        assert!(values.is_empty());
        assert!(
            values
                .drop_resource_typed_companion(&layout, 6, outer)
                .is_err()
        );
    }
}

#[test]
fn resource_ordinary_companion_absence_and_primitive_failure_do_not_own_payload_bits() {
    let layout = layout();
    let mut values = NativeValues::default();
    let unrelated = values.insert("still live".into()).unwrap();
    let failed = values.sum(SUM_FAILURE, unrelated, true).unwrap();
    assert!(
        values
            .drop_resource_typed_companion(&layout, 7, failed)
            .is_err()
    );
    assert!(values.strings.contains_key(&unrelated));
    values.sums.get_mut(&failed).unwrap().owned = false;
    values
        .drop_resource_typed_companion(&layout, 7, failed)
        .unwrap();
    assert!(values.strings.contains_key(&unrelated));
    let absent = values.sum(SUM_FAILURE, 0, false).unwrap();
    values
        .drop_resource_typed_companion(&layout, 5, absent)
        .unwrap();
    let inner = values.sum(SUM_SUCCESS, unrelated, true).unwrap();
    let nested = values.sum(SUM_SUCCESS, inner, true).unwrap();
    values
        .drop_resource_typed_companion(&layout, 5, nested)
        .unwrap();
    assert!(values.is_empty());
}
