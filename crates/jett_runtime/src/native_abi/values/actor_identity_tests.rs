mod actor_identity {
    use super::*;

    fn actor(values: &mut NativeValues, state: u64) -> u64 {
        let id = values.new_struct(1).unwrap();
        values.structs.get_mut(&id).unwrap().fields[0] = Some(NativeField {
            bits: state,
            owned: false,
            pending_depth: 0,
        });
        values.register_actor(id).unwrap()
    }

    #[test]
    fn native_actor_identity_aliases_equal_and_distinct_equal_state_actors_differ() {
        let mut values = NativeValues::default();
        let first = actor(&mut values, 7);
        let second = actor(&mut values, 7);
        assert_ne!(first, second);
        for (left, right, equal) in [(first, first, true), (first, second, false)] {
            assert_eq!(values.same_actor_identity(left, right), Ok(equal));
            assert_eq!(
                values.equal_actor_identity(left, right, 0),
                Ok(u32::from(equal))
            );
            assert_eq!(
                values.equal_actor_identity(left, right, 1),
                Ok(u32::from(!equal))
            );
        }
        values.replace_actor_field(first, 0, 88, false).unwrap();
        assert_eq!(values.equal_actor_identity(first, first, 0), Ok(1));
        assert_eq!(values.equal_actor_identity(first, second, 0), Ok(0));
        values.release_actors().unwrap();
        assert_eq!(values.structs_created, 2);
        assert_eq!(values.structs_destroyed, 2);
        assert!(values.is_empty());
    }

    #[test]
    fn native_actor_identity_validates_current_context_even_for_equal_bits() {
        let mut values = NativeValues::default();
        let mut other_context = NativeValues::default();
        let live = actor(&mut values, 7);
        let foreign = actor(&mut other_context, 7);
        let unregistered = values.new_struct(0).unwrap();
        assert_ne!(live, foreign);
        for invalid in [0, foreign, unregistered] {
            assert_eq!(
                values.same_actor_identity(invalid, invalid),
                Err(INVALID_ACTOR)
            );
            assert_eq!(
                values.equal_actor_identity(live, invalid, 0),
                Err(INVALID_ACTOR)
            );
            assert_eq!(
                values.equal_actor_identity(invalid, live, 1),
                Err(INVALID_ACTOR)
            );
        }
        assert_eq!(
            values.equal_actor_identity(live, live, 2),
            Err(INVALID_ACTOR)
        );
        values.drop_value(unregistered).unwrap();
        values.release_actors().unwrap();
        assert_eq!(values.same_actor_identity(live, live), Err(INVALID_ACTOR));
        other_context.release_actors().unwrap();
        assert!(values.is_empty());
        assert!(other_context.is_empty());
    }

    #[test]
    fn native_actor_identity_requires_a_complete_live_backing_record() {
        let mut values = NativeValues::default();
        let live = actor(&mut values, 7);
        let saved = values.structs.get_mut(&live).unwrap().fields[0].take();
        assert_eq!(values.same_actor_identity(live, live), Err(INVALID_ACTOR));
        values.structs.get_mut(&live).unwrap().fields[0] = saved;
        values.release_actors().unwrap();
        assert!(values.is_empty());
    }

    #[test]
    fn native_actor_identity_nested_layout_validates_ids_and_compares_pending_depth() {
        let mut values = NativeValues::default();
        let first = actor(&mut values, 7);
        let second = actor(&mut values, 7);
        let layout = NativeDebugLayout {
            root: 0,
            nodes: vec![NativeDebugNode::Actor],
        };
        assert_eq!(layout.equal_value(&values, first, first, 0), Ok(true));
        assert_eq!(layout.equal_value(&values, first, second, 0), Ok(false));
        assert_eq!(layout.equal_value(&values, 0, 0, 0), Err(INVALID_ACTOR));
        let pending = values.run_actor(first).unwrap();
        let also_pending = values.run_actor(first).unwrap();
        let twice = values.run_actor(pending).unwrap();
        assert_ne!(pending, also_pending);
        assert_eq!(values.equal_actor_identity(first, first, 0), Ok(1));
        assert_eq!(
            layout.equal_value(&values, pending, also_pending, 0),
            Ok(true)
        );
        assert_eq!(layout.equal_value(&values, first, pending, 0), Ok(false));
        assert_eq!(layout.equal_value(&values, pending, twice, 0), Ok(false));
        let once = values.join_actor(twice).unwrap();
        assert_eq!(layout.equal_value(&values, pending, once, 0), Ok(true));
        assert_eq!(values.join_actor(once), Ok(first));
        assert_eq!(values.structs[&first].fields[0].unwrap().bits, 7);
        values.release_actors().unwrap();
        assert!(values.is_empty());
        for (operator, label) in [(0, "Eq"), (1, "NotEq")] {
            // Each operation needs a fresh ordinary failure channel. The first
            // failure is persistent within an owning context.
            let mut values = NativeValues::default();
            let instance = actor(&mut values, 7);
            let pending = values.run_actor(instance).unwrap();
            assert_eq!(
                values.equal_actor_identity(pending, pending, operator),
                Err(UNSUPPORTED_ACTOR_COMPARISON)
            );
            assert_eq!(
                values.dynamic_failure_message.as_deref(),
                Some(
                    format!(
                        "unsupported binary operation: pending(actor#0) {label} pending(actor#0)"
                    )
                    .as_bytes()
                )
            );
            values.release_actors().unwrap();
            assert!(values.is_empty());
        }
    }

    #[test]
    fn native_actor_identity_leaf_preserves_context_failure_on_foreign_ids() {
        let context = Context::new();
        let foreign_context = Context::new();
        unsafe {
            let local = jett_rt_v1_struct_new(context.pointer(), 0);
            let foreign = jett_rt_v1_struct_new(foreign_context.pointer(), 0);
            assert_eq!(jett_rt_v1_actor_register(context.pointer(), local), local);
            assert_eq!(
                jett_rt_v1_actor_register(foreign_context.pointer(), foreign),
                foreign
            );
            assert_eq!(
                jett_rt_v1_actor_identity_equal(context.pointer(), local, local, 0),
                1
            );
            assert_eq!(jett_rt_v1_value_status(context.pointer()), 0);
            assert_eq!(
                jett_rt_v1_actor_identity_equal(context.pointer(), foreign, foreign, 0),
                JettRuntimeStatusV1::INVALID_CONTEXT.code()
            );
            assert_eq!(
                jett_rt_v1_value_status(context.pointer()),
                JettRuntimeStatusV1::INVALID_ARGUMENT.code()
            );
            assert_eq!(jett_rt_v1_value_status(foreign_context.pointer()), 0);
        }
        // The foreign handle error remains observable until retirement; clean
        // context retirement releases the registered actor and returns OK.
        context.destroy(JettRuntimeStatusV1::OK);
        foreign_context.destroy(JettRuntimeStatusV1::OK);
    }

    #[test]
    fn native_actor_identity_pending_allocation_failure_preserves_ready_aliases() {
        let mut values = NativeValues::default();
        let live = actor(&mut values, 7);
        values.allocation_budget = Some(0);
        assert_eq!(values.run_actor(live), Err(EXHAUSTED));
        assert_eq!(values.equal_actor_identity(live, live, 0), Ok(1));
        assert_eq!(values.structs_created, 1);
        assert_eq!(values.actors.len(), 1);
        values.allocation_budget = None;
        let pending = values.run_actor(live).unwrap();
        assert_eq!(
            values.actor_identity_parts(pending).unwrap().0.instance,
            live
        );
        assert_eq!(values.actor_identity_parts(pending).unwrap().0.ordinal, 0);
        assert_eq!(values.next_actor_ordinal, 1);
        values.release_actors().unwrap();
        assert_eq!(values.structs_created, values.structs_destroyed);
        assert!(values.is_empty());
    }

    #[test]
    fn native_actor_identity_pending_aliases_retire_owned_state_once() {
        let mut values = NativeValues::default();
        let text = values.insert("one owned state payload".into()).unwrap();
        let instance = values.new_struct(1).unwrap();
        values.structs.get_mut(&instance).unwrap().fields[0] = Some(NativeField {
            bits: text,
            owned: true,
            pending_depth: 0,
        });
        values.register_actor(instance).unwrap();
        let pending = values.run_actor(instance).unwrap();
        let nested = values.run_actor(pending).unwrap();
        assert_eq!(values.join_actor(pending), Ok(instance));
        assert_eq!(
            values.actor_identity_parts(nested).unwrap().0.instance,
            instance
        );
        assert_eq!(values.strings[&text].references, 1);
        assert_eq!(values.structs_created, 3);
        assert_eq!(values.structs[&pending].fields.len(), 0);
        assert_eq!(values.structs[&nested].fields.len(), 0);
        values.release_actors().unwrap();
        assert_eq!(values.structs_created, values.structs_destroyed);
        assert!(values.is_empty());
    }
}
