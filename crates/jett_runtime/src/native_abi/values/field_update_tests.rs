use super::*;

fn scalar(bits: u64, pending_depth: u64) -> NativeField {
    NativeField {
        bits,
        owned: false,
        pending_depth,
    }
}
fn owning(bits: u64) -> NativeField {
    NativeField {
        bits,
        owned: true,
        pending_depth: 0,
    }
}
fn record(values: &mut NativeValues, fields: &[NativeField]) -> u64 {
    let id = values.new_struct(fields.len() as u64).unwrap();
    for (slot, field) in values
        .structs
        .get_mut(&id)
        .unwrap()
        .fields
        .iter_mut()
        .zip(fields)
    {
        *slot = Some(*field);
    }
    id
}

#[test]
fn occupied_replacement_preserves_siblings_depth_and_clone_independence() {
    let mut values = NativeValues::default();
    let text = values.insert("before".into()).unwrap();
    let parent = record(&mut values, &[scalar(7, 3), owning(text)]);
    let clone = values.clone_struct(parent).unwrap();
    let after = values.insert("after".into()).unwrap();
    values.allocation_budget = Some(0);
    let old = values
        .replace_struct_field(parent, 1, owning(after))
        .unwrap();
    assert_eq!(old.bits, text);
    assert!(old.owned);
    assert_eq!(values.struct_field(parent, 0).unwrap().pending_depth, 3);
    assert_eq!(
        values
            .text(values.struct_field(clone, 1).unwrap().bits)
            .unwrap(),
        "before"
    );
    assert_eq!(
        values
            .text(values.struct_field(parent, 1).unwrap().bits)
            .unwrap(),
        "after"
    );
    // Return transfers displaced ownership; neither side was silently dropped.
    values.drop_value(old.bits).unwrap();
    values.drop_value(parent).unwrap();
    values.drop_value(clone).unwrap();
    assert!(values.is_empty());
}

#[test]
fn occupied_replacement_precommit_refusals_preserve_both_owners() {
    let mut values = NativeValues::default();
    let before = values.insert_bytes(vec![1]).unwrap();
    let replacement = values.insert_bytes(vec![2]).unwrap();
    let parent = record(&mut values, &[owning(before)]);
    for (index, field) in [
        (1, owning(replacement)),
        (0, scalar(8, 0)),
        (0, owning(u64::MAX)),
        (0, owning(parent)),
    ] {
        assert!(values.replace_struct_field(parent, index, field).is_err());
        assert_eq!(values.struct_field(parent, 0).unwrap().bits, before);
        assert!(values.bytes.contains_key(&before));
        assert!(values.bytes.contains_key(&replacement));
    }
    values.structs.get_mut(&parent).unwrap().pending_depth = 1;
    assert!(
        values
            .replace_struct_field(parent, 0, owning(replacement))
            .is_err()
    );
    values.structs.get_mut(&parent).unwrap().pending_depth = 0;
    let empty = values.new_struct(1).unwrap();
    assert!(
        values
            .replace_struct_field(empty, 0, owning(replacement))
            .is_err()
    );
    let mut foreign = NativeValues::default();
    assert!(
        foreign
            .replace_struct_field(parent, 0, owning(replacement))
            .is_err()
    );
    values.drop_value(empty).unwrap();
    values.drop_value(parent).unwrap();
    values.drop_value(replacement).unwrap();
    assert!(values.is_empty());
}

#[test]
fn occupied_replacement_rejects_actor_and_unpublished_graph_storage() {
    let mut values = NativeValues::default();
    let actor = record(&mut values, &[scalar(1, 0)]);
    values.register_actor(actor).unwrap();
    assert!(values.replace_struct_field(actor, 0, scalar(2, 0)).is_err());
    let ticket = values.begin_actor_graph().unwrap();
    let staged = values.reserve_actor_graph_base(ticket, 1).unwrap();
    let pending = values
        .reserve_actor_graph_pending(ticket, staged, 1)
        .unwrap();
    for id in [ticket, staged, pending] {
        assert!(values.replace_struct_field(id, 0, scalar(2, 0)).is_err());
    }
    values.abort_actor_graph(ticket).unwrap();
    values.release_actors().unwrap();
    assert!(values.is_empty());
}

#[test]
fn occupied_replacement_catalog_signature_is_versioned_and_exact() {
    assert_eq!(
        NativeLeaf::StructReplaceOccupied.symbol(),
        "jett_rt_v1_struct_replace_occupied"
    );
    assert_eq!(
        NativeLeaf::StructReplaceOccupied.parameters(),
        &[
            AbiScalar::Pointer,
            AbiScalar::I64,
            AbiScalar::I64,
            AbiScalar::I64,
            AbiScalar::I32,
            AbiScalar::I64
        ]
    );
    assert_eq!(NativeLeaf::StructReplaceOccupied.result(), AbiScalar::I64);
}

#[test]
fn occupied_scalar_replacement_returns_displaced_depth_and_installs_new_depth() {
    let mut values = NativeValues::default();
    let parent = record(&mut values, &[scalar(5, 2)]);
    let displaced = values
        .replace_struct_field(parent, 0, scalar(8, 4))
        .unwrap();
    assert_eq!(
        (displaced.bits, displaced.pending_depth, displaced.owned),
        (5, 2, false)
    );
    assert_eq!(values.struct_field(parent, 0).unwrap().pending_depth, 4);
    values.drop_value(parent).unwrap();
    assert!(values.is_empty());
}

#[test]
fn occupied_replacement_cleanup_failure_cannot_retake_replacement() {
    let mut values = NativeValues::default();
    let damaged = record(&mut values, &[owning(u64::MAX)]);
    let parent = record(&mut values, &[owning(damaged)]);
    let replacement = values.insert_bytes(vec![9]).unwrap();
    let old = values
        .replace_struct_field(parent, 0, owning(replacement))
        .unwrap();
    assert!(values.drop_value(old.bits).is_err());
    assert_eq!(values.struct_field(parent, 0).unwrap().bits, replacement);
    values.drop_value(parent).unwrap();
    assert!(values.is_empty());
}

#[test]
fn occupied_replacement_refuses_live_builder_shell_without_displacing_it() {
    let mut values = NativeValues::default();
    let parent = record(&mut values, &[scalar(3, 0)]);
    values.builders.insert(
        parent,
        NativeBuilderInfo {
            owner: "Record".into(),
            metadata_owner: "Record".into(),
            unsupported_kind: None,
            variant: None,
            state: None,
            target_state: None,
            field_offset: 0,
            field_names: vec!["value".into()],
            field_type_names: vec!["int64".into()],
            field_types: vec!["int64".into()],
            field_debug_layouts: vec![Vec::new()],
            put_order: vec![0],
            validation: vec![BuilderFieldValidation::None],
        },
    );
    assert!(
        values
            .replace_struct_field(parent, 0, scalar(4, 0))
            .is_err()
    );
    assert_eq!(values.struct_field(parent, 0).unwrap().bits, 3);
    values.drop_value(parent).unwrap();
    assert!(values.is_empty());
}

// These controls call the actual normal ABI leaf, including its failure gate
// and cleanup path. Test-only state access observes precommit rollback.
fn leaf_values<T>(
    pointer: *const JettRuntimeContextV1,
    action: impl FnOnce(&mut NativeValues) -> T,
) -> T {
    let lease = acquire_context(context_key(pointer).unwrap()).unwrap();
    let mut state = lock_unpoisoned(&lease.entry.state);
    action(&mut state.as_mut().unwrap().values)
}

fn field_leaf_context(
    expected: JettRuntimeStatusV1,
    action: impl FnOnce(*const JettRuntimeContextV1),
) {
    let mut context = JettRuntimeContextV1::retired();
    let mut result = std::mem::MaybeUninit::uninit();
    assert_eq!(
        unsafe { jett_rt_v1_context_create(1, &mut context, result.as_mut_ptr()) },
        JettRuntimeStatusV1::OK
    );
    action(&context);
    assert_eq!(
        unsafe { jett_rt_v1_value_status(&context) },
        expected.code()
    );
    assert!(leaf_values(&context, |values| values.is_empty()));
    // Context destruction reports cleanup, independently of the earlier leaf error.
    assert_eq!(
        unsafe { jett_rt_v1_context_destroy(&mut context, result.as_mut_ptr()) },
        JettRuntimeStatusV1::OK
    );
}

fn function_record(values: &mut NativeValues) -> u64 {
    let label = values.insert("field-update-test".into()).unwrap();
    record(values, &[scalar(u64::MAX, 0), scalar(0, 0), owning(label)])
}

#[test]
fn occupied_leaf_rejects_same_unique_owner_before_commit_and_cleans_once() {
    for kind in 0..4 {
        field_leaf_context(JettRuntimeStatusV1::INVALID_ARGUMENT, |pointer| {
            let (parent, child) = leaf_values(pointer, |values| {
                let child = match kind {
                    0 => values.insert_bytes(vec![1, 2]).unwrap(),
                    1 => values.new_list(false).unwrap(),
                    2 => record(values, &[scalar(3, 0)]),
                    _ => function_record(values),
                };
                (record(values, &[owning(child), scalar(19, 2)]), child)
            });
            assert_eq!(
                unsafe { jett_rt_v1_struct_replace_occupied(pointer, parent, 0, child, 1, 0) },
                0
            );
            assert_eq!(
                unsafe { jett_rt_v1_value_status(pointer) },
                JettRuntimeStatusV1::INVALID_ARGUMENT.code()
            );
            leaf_values(pointer, |values| {
                assert_eq!(values.struct_field(parent, 0).unwrap().bits, child);
                assert_eq!(values.struct_field(parent, 1).unwrap().bits, 19);
                assert_eq!(values.struct_field(parent, 1).unwrap().pending_depth, 2);
                assert!(values.owned_pending_depth(child).is_ok());
            });
            assert_eq!(unsafe { jett_rt_v1_value_drop(pointer, parent) }, 0);
        });
    }
}

#[test]
fn occupied_leaf_rejects_unique_sibling_alias_and_preserves_both_fields() {
    field_leaf_context(JettRuntimeStatusV1::INVALID_ARGUMENT, |pointer| {
        let (parent, before, sibling) = leaf_values(pointer, |values| {
            let before = values.new_list(false).unwrap();
            let sibling = values.new_list(false).unwrap();
            (
                record(values, &[owning(before), owning(sibling)]),
                before,
                sibling,
            )
        });
        assert_eq!(
            unsafe { jett_rt_v1_struct_replace_occupied(pointer, parent, 0, sibling, 1, 0) },
            0
        );
        assert_ne!(unsafe { jett_rt_v1_value_status(pointer) }, 0);
        leaf_values(pointer, |values| {
            assert_eq!(values.struct_field(parent, 0).unwrap().bits, before);
            assert_eq!(values.struct_field(parent, 1).unwrap().bits, sibling);
            assert!(values.lists.contains_key(&before));
            assert!(values.lists.contains_key(&sibling));
        });
        assert_eq!(unsafe { jett_rt_v1_value_drop(pointer, parent) }, 0);
    });
}

#[test]
fn occupied_leaf_requires_string_budget_and_allows_retained_identical_edges() {
    for fields in [1_usize, 2] {
        for retain_replacement in [false, true] {
            let expected = if retain_replacement {
                JettRuntimeStatusV1::OK
            } else {
                JettRuntimeStatusV1::INVALID_ARGUMENT
            };
            field_leaf_context(expected, |pointer| {
                let (parent, text) = leaf_values(pointer, |values| {
                    let text = values.insert("shared".into()).unwrap();
                    for _ in 1..fields {
                        values.retain(text).unwrap();
                    }
                    (record(values, &vec![owning(text); fields]), text)
                });
                if retain_replacement {
                    assert_eq!(unsafe { jett_rt_v1_string_retain(pointer, text) }, text);
                }
                let displaced =
                    unsafe { jett_rt_v1_struct_replace_occupied(pointer, parent, 0, text, 1, 0) };
                assert_eq!(displaced, if retain_replacement { text } else { 0 });
                leaf_values(pointer, |values| {
                    assert_eq!(values.struct_field(parent, 0).unwrap().bits, text);
                    assert_eq!(
                        values.strings[&text].references,
                        fields as u64 + u64::from(retain_replacement)
                    );
                });
                if retain_replacement {
                    assert_eq!(unsafe { jett_rt_v1_value_drop(pointer, displaced) }, 0);
                    leaf_values(pointer, |values| {
                        assert_eq!(values.strings[&text].references, fields as u64);
                    });
                } else {
                    assert_ne!(unsafe { jett_rt_v1_value_status(pointer) }, 0);
                }
                assert_eq!(unsafe { jett_rt_v1_value_drop(pointer, parent) }, 0);
            });
        }
    }
}

#[test]
fn occupied_leaf_accepts_distinct_function_copy_and_retires_displaced_descriptor() {
    field_leaf_context(JettRuntimeStatusV1::OK, |pointer| {
        let (parent, original) = leaf_values(pointer, |values| {
            let original = function_record(values);
            (record(values, &[owning(original)]), original)
        });
        let copied = unsafe { jett_rt_v1_struct_clone(pointer, original) };
        assert_ne!(copied, 0);
        assert_ne!(original, copied);
        let displaced =
            unsafe { jett_rt_v1_struct_replace_occupied(pointer, parent, 0, copied, 1, 0) };
        assert_eq!(displaced, original);
        leaf_values(pointer, |values| {
            assert_eq!(values.struct_field(parent, 0).unwrap().bits, copied);
        });
        assert_eq!(unsafe { jett_rt_v1_value_drop(pointer, displaced) }, 0);
        assert_eq!(unsafe { jett_rt_v1_value_drop(pointer, parent) }, 0);
        assert_eq!(unsafe { jett_rt_v1_value_status(pointer) }, 0);
    });
}
