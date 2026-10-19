use super::*;

fn actor_record(values: &mut NativeValues, fields: &[NativeField]) -> u64 {
    let id = record(values, fields);
    values.register_actor(id).unwrap()
}

#[test]
fn actor_occupied_catalog_has_exact_versioned_transfer_signature() {
    assert_eq!(
        NativeLeaf::ActorReplaceOccupied.symbol(),
        "jett_rt_v1_actor_replace_occupied"
    );
    assert_eq!(
        NativeLeaf::ActorReplaceOccupied.parameters(),
        &[
            AbiScalar::Pointer,
            AbiScalar::I64,
            AbiScalar::I64,
            AbiScalar::I64,
            AbiScalar::I32,
            AbiScalar::I64,
        ]
    );
    assert_eq!(NativeLeaf::ActorReplaceOccupied.result(), AbiScalar::I64);
}

#[test]
fn actor_occupied_scalar_swap_preserves_siblings_and_changes_task_depth_without_allocation() {
    let mut values = NativeValues::default();
    let actor = actor_record(&mut values, &[scalar(7, 2), scalar(19, 0)]);
    values.allocation_budget = Some(0);
    let displaced = values
        .replace_actor_field_occupied(actor, 0, scalar(8, 4))
        .unwrap();
    assert_eq!(
        (displaced.bits, displaced.pending_depth, displaced.owned),
        (7, 2, false)
    );
    assert_eq!(values.struct_field(actor, 0).unwrap().pending_depth, 4);
    assert_eq!(values.struct_field(actor, 1).unwrap().bits, 19);
    // A persistent alias is not an ordinary owning handle.
    assert_eq!(
        values
            .replace_actor_field_occupied(actor, 1, scalar(actor, 0))
            .unwrap()
            .bits,
        19
    );
    values.release_actors().unwrap();
    assert!(values.is_empty());
}

#[test]
fn actor_occupied_refuses_nonactors_pending_aliases_and_unpublished_graph_storage() {
    let mut values = NativeValues::default();
    let actor = actor_record(&mut values, &[scalar(1, 0)]);
    let pending = values.run_actor(actor).unwrap();
    let ordinary = record(&mut values, &[scalar(2, 0)]);
    let ticket = values.begin_actor_graph().unwrap();
    let staged = values.reserve_actor_graph_base(ticket, 1).unwrap();
    let staged_pending = values
        .reserve_actor_graph_pending(ticket, staged, 1)
        .unwrap();
    let mut foreign = NativeValues::default();
    let foreign_actor = actor_record(&mut foreign, &[scalar(3, 0)]);
    for invalid in [
        0,
        ordinary,
        pending,
        ticket,
        staged,
        staged_pending,
        foreign_actor,
    ] {
        assert_eq!(
            values
                .replace_actor_field_occupied(invalid, 0, scalar(9, 0))
                .err(),
            Some(INVALID_ACTOR)
        );
        assert_eq!(values.struct_field(actor, 0).unwrap().bits, 1);
    }
    assert_eq!(
        values
            .replace_actor_field_occupied(actor, 1, scalar(9, 0))
            .err(),
        Some(INVALID_ACTOR)
    );
    values.abort_actor_graph(ticket).unwrap();
    values.drop_value(ordinary).unwrap();
    values.release_actors().unwrap();
    foreign.release_actors().unwrap();
    assert!(values.is_empty());
    assert!(foreign.is_empty());
}

#[test]
fn actor_occupied_missing_slot_is_never_initialized_and_preserves_incoming_owner() {
    let mut values = NativeValues::default();
    let before = values.insert_bytes(vec![1]).unwrap();
    let incoming = values.insert_bytes(vec![2]).unwrap();
    let actor = actor_record(&mut values, &[owning(before), scalar(17, 3)]);
    let saved = values.structs.get_mut(&actor).unwrap().fields[0].take();
    assert_eq!(
        values
            .replace_actor_field_occupied(actor, 0, owning(incoming))
            .err(),
        Some(INVALID_ACTOR)
    );
    assert!(values.structs[&actor].fields[0].is_none());
    assert_eq!(values.struct_field(actor, 1).unwrap().pending_depth, 3);
    assert!(values.bytes.contains_key(&before));
    assert!(values.bytes.contains_key(&incoming));
    values.structs.get_mut(&actor).unwrap().fields[0] = saved;
    values.drop_value(incoming).unwrap();
    values.release_actors().unwrap();
    assert!(values.is_empty());
}

#[test]
fn actor_occupied_rejects_mismatched_owned_representation_and_dead_children_before_publication() {
    let mut values = NativeValues::default();
    let before = values.insert_bytes(vec![1]).unwrap();
    let incoming = values.insert_bytes(vec![2]).unwrap();
    let actor = actor_record(&mut values, &[owning(before), scalar(17, 0)]);
    for (index, field) in [
        (0, scalar(9, 0)),
        (1, owning(incoming)),
        (0, owning(u64::MAX)),
        (0, owning(actor)),
        (
            0,
            NativeField {
                pending_depth: 1,
                ..owning(incoming)
            },
        ),
    ] {
        assert_eq!(
            values
                .replace_actor_field_occupied(actor, index, field)
                .err(),
            Some(INVALID_ACTOR)
        );
        assert_eq!(values.struct_field(actor, 0).unwrap().bits, before);
        assert!(values.bytes.contains_key(&before));
        assert!(values.bytes.contains_key(&incoming));
    }
    values.structs.get_mut(&actor).unwrap().fields[0]
        .as_mut()
        .unwrap()
        .pending_depth = 1;
    assert_eq!(
        values
            .replace_actor_field_occupied(actor, 0, owning(incoming))
            .err(),
        Some(INVALID_ACTOR)
    );
    values.structs.get_mut(&actor).unwrap().fields[0]
        .as_mut()
        .unwrap()
        .pending_depth = 0;
    values.drop_value(incoming).unwrap();
    values.release_actors().unwrap();
    assert!(values.is_empty());
}

#[test]
fn actor_occupied_leaf_refuses_noncanonical_owned_flag_without_mutation() {
    field_leaf_context(JettRuntimeStatusV1::INVALID_ARGUMENT, |pointer| {
        let actor = leaf_values(pointer, |values| actor_record(values, &[scalar(7, 0)]));
        assert_eq!(
            unsafe { jett_rt_v1_actor_replace_occupied(pointer, actor, 0, 8, 2, 0) },
            0
        );
        leaf_values(pointer, |values| {
            assert_eq!(values.struct_field(actor, 0).unwrap().bits, 7);
            values.release_actors().unwrap();
        });
    });
}

#[test]
fn actor_occupied_refuses_builder_overlay_without_mutating_state() {
    let mut values = NativeValues::default();
    let actor = actor_record(&mut values, &[scalar(7, 0)]);
    values.builders.insert(
        actor,
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
    assert_eq!(
        values
            .replace_actor_field_occupied(actor, 0, scalar(8, 0))
            .err(),
        Some(INVALID_ACTOR)
    );
    assert_eq!(values.struct_field(actor, 0).unwrap().bits, 7);
    assert!(values.builders.contains_key(&actor));
    values.release_actors().unwrap();
    assert!(values.is_empty());
}

#[test]
fn actor_occupied_leaf_refuses_unique_reuse_and_sibling_alias_before_commit() {
    for kind in 0..5 {
        field_leaf_context(JettRuntimeStatusV1::INVALID_ARGUMENT, |pointer| {
            let (actor, before, incoming) = leaf_values(pointer, |values| {
                let before = match kind {
                    0 => values.insert_bytes(vec![1]).unwrap(),
                    1 => values.new_list(false).unwrap(),
                    2 => record(values, &[scalar(7, 0)]),
                    _ => function_record(values),
                };
                let incoming = if kind == 4 {
                    values.new_list(false).unwrap()
                } else {
                    before
                };
                let fields = if kind == 4 {
                    vec![owning(before), owning(incoming)]
                } else {
                    vec![owning(before)]
                };
                (actor_record(values, &fields), before, incoming)
            });
            assert_eq!(
                unsafe { jett_rt_v1_actor_replace_occupied(pointer, actor, 0, incoming, 1, 0) },
                0
            );
            leaf_values(pointer, |values| {
                assert_eq!(values.struct_field(actor, 0).unwrap().bits, before);
                assert!(values.owned_pending_depth(incoming).is_ok());
                values.release_actors().unwrap();
            });
        });
    }
}

#[test]
fn actor_occupied_leaf_requires_each_incoming_string_credit_and_never_drops_the_old_edge() {
    for fields in [1_usize, 2] {
        for retained in [false, true] {
            let expected = if retained {
                JettRuntimeStatusV1::OK
            } else {
                JettRuntimeStatusV1::INVALID_ARGUMENT
            };
            field_leaf_context(expected, |pointer| {
                let (actor, text) = leaf_values(pointer, |values| {
                    let text = values.insert("shared actor state".into()).unwrap();
                    for _ in 1..fields {
                        values.retain(text).unwrap();
                    }
                    (actor_record(values, &vec![owning(text); fields]), text)
                });
                if retained {
                    assert_eq!(unsafe { jett_rt_v1_string_retain(pointer, text) }, text);
                }
                let displaced =
                    unsafe { jett_rt_v1_actor_replace_occupied(pointer, actor, 0, text, 1, 0) };
                assert_eq!(displaced, if retained { text } else { 0 });
                leaf_values(pointer, |values| {
                    assert_eq!(
                        values.strings[&text].references,
                        fields as u64 + u64::from(retained)
                    )
                });
                if retained {
                    assert_eq!(unsafe { jett_rt_v1_value_drop(pointer, displaced) }, 0);
                }
                leaf_values(pointer, |values| values.release_actors().unwrap());
            });
        }
    }
}

#[test]
fn actor_occupied_leaf_allows_distinct_function_copy_and_owned_child_task_depth() {
    field_leaf_context(JettRuntimeStatusV1::OK, |pointer| {
        let (actor, before, replacement) = leaf_values(pointer, |values| {
            let before = function_record(values);
            let replacement = values.clone_struct(before).unwrap();
            values.structs.get_mut(&replacement).unwrap().pending_depth = 2;
            (actor_record(values, &[owning(before)]), before, replacement)
        });
        assert_eq!(
            unsafe { jett_rt_v1_actor_replace_occupied(pointer, actor, 0, replacement, 1, 0) },
            before
        );
        leaf_values(pointer, |values| {
            assert_eq!(values.struct_field(actor, 0).unwrap().bits, replacement);
            assert_eq!(values.owned_pending_depth(replacement).unwrap(), 2);
            assert!(values.structs.contains_key(&before));
        });
        assert_eq!(unsafe { jett_rt_v1_value_drop(pointer, before) }, 0);
        leaf_values(pointer, |values| values.release_actors().unwrap());
    });
}

#[test]
fn actor_occupied_leaf_publishes_before_old_cleanup_failure_and_transfers_incoming_slot_once() {
    let mut context = JettRuntimeContextV1::retired();
    let mut result = std::mem::MaybeUninit::uninit();
    assert_eq!(
        unsafe { jett_rt_v1_context_create(1, &mut context, result.as_mut_ptr()) },
        JettRuntimeStatusV1::OK
    );
    {
        let pointer = std::ptr::from_ref(&context);
        let (actor, old, incoming, caller) = leaf_values(pointer, |values| {
            let old = record(values, &[owning(u64::MAX)]);
            let incoming = values.insert_bytes(vec![9]).unwrap();
            let actor = actor_record(values, &[owning(old)]);
            let caller = record(values, &[owning(incoming)]);
            (actor, old, incoming, caller)
        });
        let displaced =
            unsafe { jett_rt_v1_actor_replace_occupied(pointer, actor, 0, incoming, 1, 0) };
        assert_eq!(displaced, old);
        assert_eq!(unsafe { jett_rt_v1_value_status(pointer) }, 0);
        // Match the generated handoff: clear the incoming owner slot first.
        assert_eq!(
            unsafe { jett_rt_v1_struct_take(pointer, caller, 0) },
            incoming
        );
        leaf_values(pointer, |values| {
            assert!(values.structs[&caller].fields[0].is_none());
            assert!(values.structs.contains_key(&old));
        });
        unsafe { jett_rt_v1_value_drop(pointer, displaced) };
        assert_eq!(
            unsafe { jett_rt_v1_value_status(pointer) },
            JettRuntimeStatusV1::INVALID_ARGUMENT.code()
        );
        leaf_values(pointer, |values| {
            assert_eq!(values.struct_field(actor, 0).unwrap().bits, incoming);
            assert!(values.bytes.contains_key(&incoming));
            assert!(!values.structs.contains_key(&old));
        });
        assert_eq!(unsafe { jett_rt_v1_value_drop(pointer, caller) }, 0);
        leaf_values(pointer, |values| {
            values.release_actors().unwrap();
            assert_eq!((values.bytes_created, values.bytes_destroyed), (1, 1));
            assert_eq!(values.structs_created, values.structs_destroyed);
            assert!(values.cleanup_failed);
            assert!(values.is_empty());
        });
        assert_eq!(
            unsafe { jett_rt_v1_value_status(pointer) },
            JettRuntimeStatusV1::INVALID_ARGUMENT.code()
        );
    }
    // Registry cleanup does not erase the terminal displaced-value cleanup failure.
    assert_eq!(
        unsafe { jett_rt_v1_context_destroy(&mut context, result.as_mut_ptr()) },
        JettRuntimeStatusV1::INVALID_ARGUMENT
    );
    let result = unsafe { result.assume_init() };
    assert_eq!(result.status, JettRuntimeStatusV1::INVALID_ARGUMENT);
    let length = usize::try_from(result.message.byte_length).unwrap();
    assert_eq!(length, b"native value cleanup failed".len());
    assert!(!result.message.data.is_null());
    let message = unsafe { std::slice::from_raw_parts(result.message.data, length) };
    assert_eq!(message, b"native value cleanup failed");
}
