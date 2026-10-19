//! Authored static candidate tests; execution remains an independent gate.
use super::*;

fn raw(bits: u64) -> NativeField {
    NativeField {
        bits,
        owned: false,
        pending_depth: 0,
    }
}
fn owned(bits: u64) -> NativeField {
    NativeField {
        bits,
        owned: true,
        pending_depth: 0,
    }
}
fn word(value: u32) -> Vec<u8> {
    value.to_le_bytes().to_vec()
}
fn node(tag: u8, payload: &[u8]) -> Vec<u8> {
    let mut result = vec![tag];
    result.extend_from_slice(payload);
    result
}
fn name(value: &str) -> Vec<u8> {
    let mut result = word(value.len() as u32);
    result.extend_from_slice(value.as_bytes());
    result
}
fn debug(root: u32, nodes: &[Vec<u8>]) -> Vec<u8> {
    let mut result = b"JD\x01".to_vec();
    result.extend(word(nodes.len() as u32));
    result.extend(word(root));
    for node in nodes {
        result.extend(word(node.len() as u32));
        result.extend(node);
    }
    result
}
fn supplement(index: u32, kind: u8, payload: &[u8]) -> Vec<u8> {
    let mut result = word(index);
    result.push(kind);
    result.extend_from_slice(payload);
    result
}
fn schema(root: u32, nodes: &[Vec<u8>], supplements: &[Vec<u8>]) -> Vec<u8> {
    let debug = debug(root, nodes);
    let mut result = b"JA\x01".to_vec();
    result.extend(word(debug.len() as u32));
    result.extend(debug);
    result.extend(word(supplements.len() as u32));
    for supplement in supplements {
        result.extend(supplement);
    }
    result
}
fn actor_schema() -> Vec<u8> {
    schema(0, &[node(14, &[])], &[])
}
fn string_schema() -> Vec<u8> {
    schema(0, &[node(0, &word(11))], &[])
}
fn record_node(children: &[u32]) -> Vec<u8> {
    let mut payload = name("Environment");
    payload.extend(word(children.len() as u32));
    for (index, child) in children.iter().enumerate() {
        payload.extend(name(&format!("capture{index}")));
        payload.extend(word(*child));
    }
    node(8, &payload)
}
fn function_schema(code: u64, child: u32, captured: Vec<u8>) -> Vec<u8> {
    let mut payload = code.to_le_bytes().to_vec();
    payload.extend(word(1));
    schema(
        0,
        &[node(13, &[]), record_node(&[child]), captured],
        &[supplement(0, 1, &payload)],
    )
}
fn descriptor(values: &mut NativeValues, code: u64, captured: NativeField) -> (u64, u64, u64) {
    let environment = values.new_struct(1).unwrap();
    values.structs.get_mut(&environment).unwrap().fields[0] = Some(captured);
    let label = values.insert("function(candidate)".into()).unwrap();
    let descriptor = values.new_struct(3).unwrap();
    values.structs.get_mut(&descriptor).unwrap().fields = vec![
        Some(raw(code)),
        Some(owned(environment)),
        Some(owned(label)),
    ];
    (descriptor, environment, label)
}

#[test]
fn graph_base_cycles_and_pending_wrappers_publish_together_without_dispatch() {
    let mut values = NativeValues::default();
    let ticket = values.begin_actor_graph().unwrap();
    let first = values.reserve_actor_graph_base(ticket, 2).unwrap();
    let second = values.reserve_actor_graph_base(ticket, 1).unwrap();
    let pending = values
        .reserve_actor_graph_pending(ticket, second, 3)
        .unwrap();
    for handle in [first, second, pending] {
        assert!(values.actor_identity_parts(handle).is_err());
        assert!(values.check_actor_message(handle).is_err());
        assert!(values.struct_field(handle, 0).is_err());
    }
    let actor = actor_schema();
    values
        .fill_actor_graph(ticket, first, 0, raw(second), &actor)
        .unwrap();
    values
        .fill_actor_graph(ticket, first, 1, raw(pending), &actor)
        .unwrap();
    values
        .fill_actor_graph(ticket, second, 0, raw(first), &actor)
        .unwrap();
    values.commit_actor_graph(ticket).unwrap();
    assert_eq!(values.struct_field(first, 0).unwrap().bits, second);
    assert_eq!(values.struct_field(second, 0).unwrap().bits, first);
    let (identity, depth) = values.actor_identity_parts(pending).unwrap();
    assert_eq!((identity.instance, depth), (second, 3));
    let another = values.run_actor(pending).unwrap();
    assert_eq!(values.actor_identity_parts(another).unwrap().1, 4);
    assert_eq!(values.actor_identity_parts(pending).unwrap().1, 3);
    values.release_actors().unwrap();
    assert!(values.is_empty());
}

#[test]
fn abort_isolates_new_ownership_and_preserves_older_actors_and_failure() {
    let mut values = NativeValues::default();
    let older = values.new_struct(0).unwrap();
    values.register_actor(older).unwrap();
    let ticket = values.begin_actor_graph().unwrap();
    let base = values.reserve_actor_graph_base(ticket, 1).unwrap();
    values.reserve_actor_graph_pending(ticket, base, 2).unwrap();
    let text = values.insert("fresh".into()).unwrap();
    let list = values.new_list(true).unwrap();
    values
        .lists
        .get_mut(&list)
        .unwrap()
        .elements
        .push(Some(text));
    let layout = schema(0, &[node(3, &word(1)), node(0, &word(11))], &[]);
    values
        .fill_actor_graph(ticket, base, 0, owned(list), &layout)
        .unwrap();
    assert!(!values.lists.contains_key(&list));
    assert!(!values.strings.contains_key(&text));
    assert!(values.clone_value(list).is_err());
    values.record_ordinary_failure((JettRuntimeStatusV1::INVALID_ARGUMENT, b"original failure"));
    let prior = values.current_failure();
    values.abort_actor_graph(ticket).unwrap();
    assert_eq!(values.current_failure(), prior);
    assert!(!values.cleanup_failed);
    assert!(values.actor_identity_parts(older).is_ok());
    assert_eq!(values.next_actor_ordinal, 1);
    assert_eq!(values.structs_created - values.structs_destroyed, 1);
    assert_eq!(values.lists_created, values.lists_destroyed);
    assert!(values.actor_graph_imports.is_empty());
    values.release_actors().unwrap();
    assert!(values.is_empty());
}

#[test]
fn tickets_foreign_older_owned_actor_and_duplicate_fills_refuse() {
    let mut values = NativeValues::default();
    let mut foreign = NativeValues::default();
    let older = values.new_struct(0).unwrap();
    values.register_actor(older).unwrap();
    let foreign_ticket = foreign.begin_actor_graph().unwrap();
    let alien = foreign.reserve_actor_graph_base(foreign_ticket, 0).unwrap();
    let ticket = values.begin_actor_graph().unwrap();
    let base = values.reserve_actor_graph_base(ticket, 1).unwrap();
    let actor = actor_schema();
    assert_eq!(
        values.reserve_actor_graph_base(foreign_ticket, 0),
        Err(INVALID_IMPORT)
    );
    assert_eq!(
        values.reserve_actor_graph_pending(ticket, older, 1),
        Err(INVALID_IMPORT)
    );
    assert_eq!(
        values.reserve_actor_graph_pending(ticket, base, 0),
        Err(INVALID_IMPORT)
    );
    for value in [older, alien] {
        assert_eq!(
            values.fill_actor_graph(ticket, base, 0, raw(value), &actor),
            Err(INVALID_IMPORT)
        );
    }
    assert_eq!(
        values.fill_actor_graph(ticket, base, 0, owned(base), &actor),
        Err(INVALID_IMPORT)
    );
    values
        .fill_actor_graph(ticket, base, 0, raw(base), &actor)
        .unwrap();
    assert_eq!(
        values.fill_actor_graph(ticket, base, 0, raw(base), &actor),
        Err(INVALID_IMPORT)
    );
    values.abort_actor_graph(ticket).unwrap();
    assert_eq!(values.commit_actor_graph(ticket), Err(INVALID_IMPORT));
    assert_eq!(values.abort_actor_graph(ticket), Err(INVALID_IMPORT));
    foreign.abort_actor_graph(foreign_ticket).unwrap();
    values.release_actors().unwrap();
    assert!(values.is_empty());
    assert!(foreign.is_empty());
}

#[test]
fn incomplete_and_exhausted_commit_publish_nothing_and_abort_needs_no_allocation() {
    let mut values = NativeValues::default();
    let ticket = values.begin_actor_graph().unwrap();
    let base = values.reserve_actor_graph_base(ticket, 1).unwrap();
    assert_eq!(values.commit_actor_graph(ticket), Err(INVALID_IMPORT));
    let text = values.insert("ready".into()).unwrap();
    values
        .fill_actor_graph(ticket, base, 0, owned(text), &string_schema())
        .unwrap();
    values.allocation_budget = Some(0);
    assert_eq!(values.commit_actor_graph(ticket), Err(EXHAUSTED));
    assert!(values.actors.is_empty());
    assert!(!values.structs.contains_key(&base));
    values.abort_actor_graph(ticket).unwrap();
    assert!(values.is_empty());
}

#[test]
fn hidden_refinement_and_interface_children_preserve_staged_actor_reference() {
    let mut values = NativeValues::default();
    let ticket = values.begin_actor_graph().unwrap();
    let base = values.reserve_actor_graph_base(ticket, 1).unwrap();
    let metadata = values.insert_bytes(debug(0, &[node(14, &[])])).unwrap();
    let interface = values.new_struct(3).unwrap();
    values.structs.get_mut(&interface).unwrap().fields =
        vec![Some(raw(77)), Some(raw(base)), Some(owned(metadata))];
    let mut interface_proof = 77u64.to_le_bytes().to_vec();
    interface_proof.extend(word(3));
    let layout = schema(
        0,
        &[
            node(16, &[]),
            node(12, &word(2)),
            node(17, &[]),
            node(14, &[]),
        ],
        &[
            supplement(0, 0, &word(1)),
            supplement(2, 2, &interface_proof),
        ],
    );
    values
        .fill_actor_graph(ticket, base, 0, owned(interface), &layout)
        .unwrap();
    assert!(!values.structs.contains_key(&interface));
    values.commit_actor_graph(ticket).unwrap();
    assert_eq!(values.struct_field(interface, 1).unwrap().bits, base);
    values.release_actors().unwrap();
    assert!(values.is_empty());
}

#[test]
fn exact_closure_code_and_typed_captures_validate_before_owned_transfer() {
    let mut values = NativeValues::default();
    let ticket = values.begin_actor_graph().unwrap();
    let base = values.reserve_actor_graph_base(ticket, 1).unwrap();
    let (callback, environment, label) = descriptor(&mut values, 123, raw(base));
    let incorrect = function_schema(124, 2, node(14, &[]));
    assert_eq!(
        values.fill_actor_graph(ticket, base, 0, owned(callback), &incorrect),
        Err(INVALID_IMPORT)
    );
    assert!(values.structs.contains_key(&callback));
    assert!(values.strings.contains_key(&label));
    let correct = function_schema(123, 2, node(14, &[]));
    values
        .fill_actor_graph(ticket, base, 0, owned(callback), &correct)
        .unwrap();
    assert!(!values.structs.contains_key(&environment));
    assert!(values.function_debug_label(callback).is_err());
    values.commit_actor_graph(ticket).unwrap();
    assert_eq!(
        values.function_debug_label(callback).unwrap(),
        "function(candidate)"
    );
    assert_eq!(values.struct_field(environment, 0).unwrap().bits, base);
    values.release_actors().unwrap();
    assert!(values.is_empty());
}

#[test]
fn nested_capability_resource_and_foreign_ordinary_owned_values_refuse() {
    let mut values = NativeValues::default();
    let mut foreign = NativeValues::default();
    let ticket = values.begin_actor_graph().unwrap();
    let base = values.reserve_actor_graph_base(ticket, 1).unwrap();
    let capability = values.grant_opaque_capability("Network").unwrap();
    let (callback, _, _) = descriptor(&mut values, 123, raw(capability));
    let layout = function_schema(123, 2, node(11, &name("Network")));
    assert_eq!(
        values.fill_actor_graph(ticket, base, 0, owned(callback), &layout),
        Err(INVALID_IMPORT)
    );
    let resource = schema(0, &[node(16, &[])], &[supplement(0, 4, &[])]);
    assert_eq!(
        values.fill_actor_graph(ticket, base, 0, raw(999), &resource),
        Err(INVALID_IMPORT)
    );
    let alien = foreign.insert("foreign".into()).unwrap();
    assert_eq!(
        values.fill_actor_graph(ticket, base, 0, owned(alien), &string_schema()),
        Err(INVALID_IMPORT)
    );
    values.drop_value(callback).unwrap();
    values.abort_actor_graph(ticket).unwrap();
    foreign.drop_value(alien).unwrap();
    assert!(values.is_empty());
    assert!(foreign.is_empty());
}

#[test]
fn builder_metadata_keeps_exact_owner_and_partial_typed_field_lifecycle() {
    let mut values = NativeValues::default();
    let ticket = values.begin_actor_graph().unwrap();
    let base = values.reserve_actor_graph_base(ticket, 1).unwrap();
    let builder = values.new_struct(2).unwrap();
    values.structs.get_mut(&builder).unwrap().fields[0] = Some(raw(base));
    values.builders.insert(
        builder,
        NativeBuilderInfo {
            owner: "Packet".into(),
            metadata_owner: "Packet".into(),
            unsupported_kind: None,
            variant: None,
            state: None,
            target_state: None,
            field_offset: 0,
            field_names: vec!["actor".into(), "label".into()],
            field_type_names: vec!["Receiver".into(), "string".into()],
            field_types: vec!["Receiver".into(), "string".into()],
            field_debug_layouts: vec![debug(0, &[node(14, &[])]), debug(0, &[node(0, &word(11))])],
            put_order: vec![0],
            validation: vec![BuilderFieldValidation::None, BuilderFieldValidation::None],
        },
    );
    let mut proof = name("Packet");
    proof.extend(word(0));
    proof.extend(word(2));
    proof.extend(word(1));
    proof.extend(word(2));
    let layout = schema(
        0,
        &[node(15, &[]), node(14, &[]), node(0, &word(11))],
        &[supplement(0, 3, &proof)],
    );
    values
        .fill_actor_graph(ticket, base, 0, owned(builder), &layout)
        .unwrap();
    assert!(!values.builders.contains_key(&builder));
    values.commit_actor_graph(ticket).unwrap();
    assert_eq!(values.builders[&builder].put_order, [0]);
    assert!(values.structs[&builder].fields[1].is_none());
    assert_eq!(values.struct_field(builder, 0).unwrap().bits, base);
    values.release_actors().unwrap();
    assert!(values.is_empty());
}

#[test]
fn private_layout_refuses_duplicate_wrong_missing_out_of_range_and_cyclic_evidence() {
    let hidden = node(16, &[]);
    let valid = supplement(0, 0, &word(1));
    let malformed = [
        schema(0, &[hidden.clone(), node(14, &[])], &[valid.clone(), valid]),
        schema(0, &[hidden.clone()], &[supplement(0, 0, &word(0))]),
        schema(0, &[hidden], &[supplement(0, 0, &word(7))]),
        schema(0, &[node(14, &[])], &[supplement(0, 0, &word(0))]),
        schema(0, &[node(12, &word(1)), node(12, &word(0))], &[]),
    ];
    for bytes in malformed {
        assert!(ImportLayout::parse(&bytes).is_err());
    }
    let missing = schema(0, &[node(16, &[])], &[]);
    assert!(ImportLayout::parse(&missing).unwrap().owns(0).is_err());
    let mut trailing = actor_schema();
    trailing.push(0);
    assert!(ImportLayout::parse(&trailing).is_err());
}

#[test]
fn repeated_factory_imports_are_fresh_and_context_teardown_aborts_unfinished_import() {
    let mut values = NativeValues::default();
    let mut first = None;
    for _ in 0..2 {
        let ticket = values.begin_actor_graph().unwrap();
        let base = values.reserve_actor_graph_base(ticket, 0).unwrap();
        values.commit_actor_graph(ticket).unwrap();
        if let Some(previous) = first {
            assert_ne!(base, previous);
        }
        first = Some(base);
    }
    let ticket = values.begin_actor_graph().unwrap();
    let base = values.reserve_actor_graph_base(ticket, 1).unwrap();
    let text = values.insert("unfinished".into()).unwrap();
    values
        .fill_actor_graph(ticket, base, 0, owned(text), &string_schema())
        .unwrap();
    values.release_actors().unwrap();
    assert!(values.is_empty());
    assert_eq!(values.structs_created, values.structs_destroyed);
}

#[test]
fn graph_leaf_scalar_schema_is_explicit_and_owner_actor_register_signature_is_unchanged() {
    use AbiScalar::{I32, I64, Pointer};
    assert_eq!(NativeLeaf::ActorRegister.parameters(), &[Pointer, I64]);
    assert_eq!(NativeLeaf::ActorGraphBegin.parameters(), &[Pointer]);
    assert_eq!(
        NativeLeaf::ActorGraphReserveBase.parameters(),
        &[Pointer, I64, I64]
    );
    assert_eq!(
        NativeLeaf::ActorGraphReservePending.parameters(),
        &[Pointer, I64, I64, I64]
    );
    assert_eq!(
        NativeLeaf::ActorGraphFill.parameters(),
        &[Pointer, I64, I64, I64, I64, I32, I64, Pointer, I64]
    );
    assert_eq!(NativeLeaf::ActorGraphCommit.parameters(), &[Pointer, I64]);
    assert_eq!(NativeLeaf::ActorGraphAbort.parameters(), &[Pointer, I64]);
    assert_eq!(
        NativeLeaf::ActorGraphAbort.symbol(),
        "jett_rt_v1_actor_graph_abort"
    );
}

#[test]
fn ordinary_leaf_failure_automatically_aborts_and_allows_captured_error_reentry() {
    let mut context = Box::new(JettRuntimeContextV1::retired());
    let mut result = std::mem::MaybeUninit::uninit();
    assert_eq!(
        unsafe { jett_rt_v1_context_create(1, &mut *context, result.as_mut_ptr()) },
        JettRuntimeStatusV1::OK
    );
    let pointer = &*context as *const JettRuntimeContextV1;
    let older = unsafe { jett_rt_v1_struct_new(pointer, 1) };
    assert_eq!(
        unsafe { jett_rt_v1_struct_init(pointer, older, 0, 17, 0) },
        0
    );
    assert_eq!(unsafe { jett_rt_v1_actor_register(pointer, older) }, older);
    let ticket = unsafe { jett_rt_v1_actor_graph_begin(pointer) };
    let base = unsafe { jett_rt_v1_actor_graph_reserve_base(pointer, ticket, 1) };
    let text = b"owned";
    let value = unsafe { jett_rt_v1_string_literal(pointer, text.as_ptr(), text.len() as u64) };
    let schema = string_schema();
    assert_eq!(
        unsafe {
            jett_rt_v1_actor_graph_fill(
                pointer,
                ticket,
                base,
                0,
                value,
                1,
                0,
                schema.as_ptr(),
                schema.len() as u64,
            )
        },
        0
    );
    // Failed u32 leaves return the existing sentinel; value_status reports
    // the separately retained first ordinary failure.
    assert_eq!(
        unsafe { jett_rt_v1_actor_message_check(pointer, base) },
        JettRuntimeStatusV1::INVALID_CONTEXT.code()
    );
    assert_eq!(
        unsafe { jett_rt_v1_value_status(pointer) },
        JettRuntimeStatusV1::INVALID_ARGUMENT.code()
    );
    // The caller must not issue a stale explicit Abort after automatic rollback.
    assert_eq!(unsafe { jett_rt_v1_actor_graph_begin(pointer) }, 0);
    {
        let lease = acquire_context(context_key(pointer).unwrap()).unwrap();
        let state = lock_unpoisoned(&lease.entry.state);
        let values = &state.as_ref().unwrap().values;
        assert!(values.actor_graph_imports.is_empty());
        assert!(!values.cleanup_failed);
        assert!(values.actor_identity_parts(older).is_ok());
        assert_eq!(values.struct_field(older, 0).unwrap().bits, 17);
        assert_eq!(values.structs_created - values.structs_destroyed, 1);
        assert_eq!(values.current_failure().unwrap(), INVALID_ACTOR);
    }
    // Exercise the genuine handler capture contract, including its owned text.
    let prefix = b"handler: ";
    let captured = unsafe {
        jett_rt_v1_failure_take_prefixed_text(pointer, prefix.as_ptr(), prefix.len() as u64)
    };
    assert_ne!(captured, 0);
    assert_eq!(unsafe { jett_rt_v1_value_status(pointer) }, 0);
    {
        let lease = acquire_context(context_key(pointer).unwrap()).unwrap();
        let state = lock_unpoisoned(&lease.entry.state);
        let values = &state.as_ref().unwrap().values;
        assert_eq!(
            values.text(captured).unwrap(),
            "handler: invalid native actor handle or field"
        );
        assert!(values.current_failure().is_none());
    }
    assert_eq!(unsafe { jett_rt_v1_string_release(pointer, captured) }, 0);
    let next_ticket = unsafe { jett_rt_v1_actor_graph_begin(pointer) };
    assert!(next_ticket > ticket);
    let next_base = unsafe { jett_rt_v1_actor_graph_reserve_base(pointer, next_ticket, 0) };
    assert!(next_base > base);
    assert_eq!(
        unsafe { jett_rt_v1_actor_graph_commit(pointer, next_ticket) },
        0
    );
    assert_eq!(unsafe { jett_rt_v1_value_status(pointer) }, 0);
    assert_eq!(
        unsafe { jett_rt_v1_context_destroy(&mut *context, result.as_mut_ptr()) },
        JettRuntimeStatusV1::OK
    );
}

#[test]
fn actual_context_teardown_aborts_unfinished_import_without_an_ownership_leak() {
    let mut context = Box::new(JettRuntimeContextV1::retired());
    let mut result = std::mem::MaybeUninit::uninit();
    assert_eq!(
        unsafe { jett_rt_v1_context_create(1, &mut *context, result.as_mut_ptr()) },
        JettRuntimeStatusV1::OK
    );
    let pointer = &*context as *const JettRuntimeContextV1;
    let ticket = unsafe { jett_rt_v1_actor_graph_begin(pointer) };
    let base = unsafe { jett_rt_v1_actor_graph_reserve_base(pointer, ticket, 1) };
    let text = b"unfinished";
    let value = unsafe { jett_rt_v1_string_literal(pointer, text.as_ptr(), text.len() as u64) };
    let schema = string_schema();
    assert_eq!(
        unsafe {
            jett_rt_v1_actor_graph_fill(
                pointer,
                ticket,
                base,
                0,
                value,
                1,
                0,
                schema.as_ptr(),
                schema.len() as u64,
            )
        },
        0
    );
    assert_eq!(
        unsafe { jett_rt_v1_context_destroy(&mut *context, result.as_mut_ptr()) },
        JettRuntimeStatusV1::OK
    );
}

#[test]
fn list_entry_schemas_validate_distinct_exact_callback_addresses_and_cardinality() {
    let mut values = NativeValues::default();
    let ticket = values.begin_actor_graph().unwrap();
    let base = values.reserve_actor_graph_base(ticket, 1).unwrap();
    let mut callbacks = Vec::new();
    for code in [123, 456] {
        let label = values.insert("function(element)".into()).unwrap();
        let callback = values.new_struct(3).unwrap();
        values.structs.get_mut(&callback).unwrap().fields =
            vec![Some(raw(code)), Some(raw(0)), Some(owned(label))];
        callbacks.push(callback);
    }
    let list = values.new_list(true).unwrap();
    values.lists.get_mut(&list).unwrap().elements = callbacks.iter().copied().map(Some).collect();
    // Node1 describes only the list element type; nodes2/3 carry actual code proofs.
    let nodes = [
        node(3, &word(1)),
        node(13, &[]),
        node(13, &[]),
        node(13, &[]),
    ];
    let function_proof = |index, code: u64| {
        let mut payload = code.to_le_bytes().to_vec();
        payload.extend(word(u32::MAX));
        supplement(index, 1, &payload)
    };
    let mut entries = word(2);
    entries.extend(word(2));
    entries.extend(word(3));
    let correct = schema(
        0,
        &nodes,
        &[
            supplement(0, 5, &entries),
            function_proof(2, 123),
            function_proof(3, 456),
        ],
    );
    let mut short = word(1);
    short.extend(word(2));
    let incorrect = schema(
        0,
        &nodes,
        &[
            supplement(0, 5, &short),
            function_proof(2, 123),
            function_proof(3, 456),
        ],
    );
    assert_eq!(
        values.fill_actor_graph(ticket, base, 0, owned(list), &incorrect),
        Err(INVALID_IMPORT)
    );
    assert!(values.lists.contains_key(&list));
    values
        .fill_actor_graph(ticket, base, 0, owned(list), &correct)
        .unwrap();
    values.commit_actor_graph(ticket).unwrap();
    assert_eq!(values.struct_field(callbacks[0], 0).unwrap().bits, 123);
    assert_eq!(values.struct_field(callbacks[1], 0).unwrap().bits, 456);
    values.release_actors().unwrap();
    assert!(values.is_empty());
}

#[test]
fn map_entry_schemas_validate_distinct_exact_interface_owners() {
    let mut values = NativeValues::default();
    let ticket = values.begin_actor_graph().unwrap();
    let base = values.reserve_actor_graph_base(ticket, 1).unwrap();
    let other = values.reserve_actor_graph_base(ticket, 0).unwrap();
    let mut interfaces = Vec::new();
    for (concrete, actor) in [(77, base), (88, other)] {
        let metadata = values.insert_bytes(debug(0, &[node(14, &[])])).unwrap();
        let interface = values.new_struct(3).unwrap();
        values.structs.get_mut(&interface).unwrap().fields =
            vec![Some(raw(concrete)), Some(raw(actor)), Some(owned(metadata))];
        interfaces.push(interface);
    }
    let map = values.new_map(0, 1).unwrap();
    values.map_append_literal(map, 0, interfaces[0]).unwrap();
    values.map_append_literal(map, 1, interfaces[1]).unwrap();
    let mut map_children = word(1);
    map_children.extend(word(2));
    let nodes = [
        node(5, &map_children),
        node(0, &word(7)),
        node(17, &[]),
        node(17, &[]),
        node(17, &[]),
        node(14, &[]),
    ];
    let proof = |index, concrete: u64| {
        let mut payload = concrete.to_le_bytes().to_vec();
        payload.extend(word(5));
        supplement(index, 2, &payload)
    };
    let mut entries = word(2);
    entries.extend(word(1));
    entries.extend(word(3));
    entries.extend(word(1));
    entries.extend(word(4));
    let layout = schema(
        0,
        &nodes,
        &[supplement(0, 6, &entries), proof(3, 77), proof(4, 88)],
    );
    values
        .fill_actor_graph(ticket, base, 0, owned(map), &layout)
        .unwrap();
    values.commit_actor_graph(ticket).unwrap();
    assert_eq!(values.struct_field(interfaces[0], 0).unwrap().bits, 77);
    assert_eq!(values.struct_field(interfaces[1], 0).unwrap().bits, 88);
    values.release_actors().unwrap();
    assert!(values.is_empty());
}

#[test]
fn failed_automatic_abort_poison_preserves_first_error_message_and_older_actor() {
    let mut context = Box::new(JettRuntimeContextV1::retired());
    let mut result = std::mem::MaybeUninit::uninit();
    assert_eq!(
        unsafe { jett_rt_v1_context_create(1, &mut *context, result.as_mut_ptr()) },
        JettRuntimeStatusV1::OK
    );
    let pointer = &*context as *const JettRuntimeContextV1;
    let older = unsafe { jett_rt_v1_struct_new(pointer, 1) };
    assert_eq!(
        unsafe { jett_rt_v1_struct_init(pointer, older, 0, 17, 0) },
        0
    );
    assert_eq!(unsafe { jett_rt_v1_actor_register(pointer, older) }, older);
    let ticket = unsafe { jett_rt_v1_actor_graph_begin(pointer) };
    let base = unsafe { jett_rt_v1_actor_graph_reserve_base(pointer, ticket, 1) };
    let text = b"rollback fault";
    let value = unsafe { jett_rt_v1_string_literal(pointer, text.as_ptr(), text.len() as u64) };
    let schema = string_schema();
    assert_eq!(
        unsafe {
            jett_rt_v1_actor_graph_fill(
                pointer,
                ticket,
                base,
                0,
                value,
                1,
                0,
                schema.as_ptr(),
                schema.len() as u64,
            )
        },
        0
    );
    {
        let lease = acquire_context(context_key(pointer).unwrap()).unwrap();
        let mut state = lock_unpoisoned(&lease.entry.state);
        // Private test-only corruption forces the otherwise infallible typed drop
        // to fail; no ordinary leaf can reach isolated transferred storage.
        assert!(
            state
                .as_mut()
                .unwrap()
                .values
                .actor_graph_imports
                .get_mut(&ticket)
                .unwrap()
                .ordinary
                .strings
                .remove(&value)
                .is_some()
        );
    }
    let failed: u32 = leaf(pointer, false, |values| {
        values.set_dynamic_failure_message(b"primary sentinel".to_vec());
        Err(INVALID_ACTOR)
    });
    assert_eq!(failed, JettRuntimeStatusV1::INVALID_CONTEXT.code());
    assert_eq!(
        unsafe { jett_rt_v1_value_status(pointer) },
        JettRuntimeStatusV1::INVALID_ARGUMENT.code()
    );
    // Cleanup poison forbids the genuine handler capture from clearing failure.
    assert_eq!(
        unsafe { jett_rt_v1_failure_take_prefixed_text(pointer, std::ptr::null(), 0) },
        0
    );
    assert_eq!(
        unsafe { jett_rt_v1_value_status(pointer) },
        JettRuntimeStatusV1::INVALID_ARGUMENT.code()
    );
    {
        let lease = acquire_context(context_key(pointer).unwrap()).unwrap();
        let state = lock_unpoisoned(&lease.entry.state);
        let values = &state.as_ref().unwrap().values;
        assert!(values.actor_graph_imports.is_empty());
        assert!(values.cleanup_failed);
        assert_eq!(values.current_failure(), Some(INVALID_ACTOR));
        assert_eq!(
            values.current_dynamic_failure_message(),
            Some(&b"primary sentinel"[..])
        );
        assert!(values.actor_identity_parts(older).is_ok());
        assert_eq!(values.struct_field(older, 0).unwrap().bits, 17);
        assert_eq!(values.structs_created - values.structs_destroyed, 1);
    }
    assert_eq!(
        unsafe { jett_rt_v1_context_destroy(&mut *context, result.as_mut_ptr()) },
        JettRuntimeStatusV1::INVALID_ARGUMENT
    );
    let result = unsafe { result.assume_init() };
    let message = unsafe {
        std::slice::from_raw_parts(result.message.data, result.message.byte_length as usize)
    };
    assert_eq!(message, b"native value cleanup failed");
}

#[test]
fn successful_result_failure_value_keeps_import_active_and_publishes_normally() {
    let mut context = Box::new(JettRuntimeContextV1::retired());
    let mut result = std::mem::MaybeUninit::uninit();
    assert_eq!(
        unsafe { jett_rt_v1_context_create(1, &mut *context, result.as_mut_ptr()) },
        JettRuntimeStatusV1::OK
    );
    let pointer = &*context as *const JettRuntimeContextV1;
    let ticket = unsafe { jett_rt_v1_actor_graph_begin(pointer) };
    let base = unsafe { jett_rt_v1_actor_graph_reserve_base(pointer, ticket, 1) };
    let fail_value = unsafe { jett_rt_v1_sum_new(pointer, SUM_FAILURE, 42, 0) };
    assert_ne!(fail_value, 0);
    assert_eq!(unsafe { jett_rt_v1_value_status(pointer) }, 0);
    {
        let lease = acquire_context(context_key(pointer).unwrap()).unwrap();
        let state = lock_unpoisoned(&lease.entry.state);
        assert!(
            state
                .as_ref()
                .unwrap()
                .values
                .actor_graph_imports
                .contains_key(&ticket)
        );
    }
    let mut children = word(1);
    children.extend(word(1));
    let schema = schema(0, &[node(7, &children), node(0, &word(7))], &[]);
    assert_eq!(
        unsafe {
            jett_rt_v1_actor_graph_fill(
                pointer,
                ticket,
                base,
                0,
                fail_value,
                1,
                0,
                schema.as_ptr(),
                schema.len() as u64,
            )
        },
        0
    );
    assert_eq!(unsafe { jett_rt_v1_actor_graph_commit(pointer, ticket) }, 0);
    assert_eq!(
        unsafe { jett_rt_v1_sum_tag(pointer, fail_value) },
        SUM_FAILURE
    );
    assert_eq!(unsafe { jett_rt_v1_value_status(pointer) }, 0);
    assert_eq!(
        unsafe { jett_rt_v1_context_destroy(&mut *context, result.as_mut_ptr()) },
        JettRuntimeStatusV1::OK
    );
}

#[test]
fn mapped_native_panic_aborts_staging_before_original_status_is_observed() {
    let mut context = Box::new(JettRuntimeContextV1::retired());
    let mut result = std::mem::MaybeUninit::uninit();
    assert_eq!(
        unsafe { jett_rt_v1_context_create(1, &mut *context, result.as_mut_ptr()) },
        JettRuntimeStatusV1::OK
    );
    let pointer = &*context as *const JettRuntimeContextV1;
    let ticket = unsafe { jett_rt_v1_actor_graph_begin(pointer) };
    assert_ne!(
        unsafe { jett_rt_v1_actor_graph_reserve_base(pointer, ticket, 0) },
        0
    );
    let failed: u32 = leaf(pointer, false, |_| panic!("private native import fault"));
    assert_eq!(failed, JettRuntimeStatusV1::INVALID_CONTEXT.code());
    assert_eq!(
        unsafe { jett_rt_v1_value_status(pointer) },
        JettRuntimeStatusV1::PANIC.code()
    );
    {
        let lease = acquire_context(context_key(pointer).unwrap()).unwrap();
        let state = lock_unpoisoned(&lease.entry.state);
        let values = &state.as_ref().unwrap().values;
        assert!(values.actor_graph_imports.is_empty());
        assert!(values.is_empty());
        assert!(!values.cleanup_failed);
        assert_eq!(
            values.current_failure(),
            Some((JettRuntimeStatusV1::PANIC, PANIC_MESSAGE))
        );
    }
    assert_eq!(
        unsafe { jett_rt_v1_context_destroy(&mut *context, result.as_mut_ptr()) },
        JettRuntimeStatusV1::OK
    );
}

#[test]
fn empty_list_and_map_placeholders_carry_no_capability_resource_or_never_authority() {
    for placeholder in [node(11, &name("Network")), node(16, &[]), node(18, &[])] {
        let is_resource = placeholder[0] == 16;
        let mut values = NativeValues::default();
        let ticket = values.begin_actor_graph().unwrap();
        let base = values.reserve_actor_graph_base(ticket, 2).unwrap();
        let actual_proof = if is_resource {
            vec![supplement(0, 4, &[])]
        } else {
            vec![]
        };
        let actual = schema(0, &[placeholder.clone()], &actual_proof);
        assert!(!ImportLayout::parse(&actual).unwrap().owns(0).unwrap());
        assert_eq!(
            values.fill_actor_graph(ticket, base, 0, raw(0), &actual),
            Err(INVALID_IMPORT)
        );
        let list = values.new_list(false).unwrap();
        let map = values.new_map(0, 0).unwrap();
        let list_proof = if is_resource {
            vec![supplement(1, 4, &[])]
        } else {
            vec![]
        };
        let list_schema = schema(0, &[node(3, &word(1)), placeholder.clone()], &list_proof);
        let mut map_children = word(1);
        map_children.extend(word(2));
        let map_proof = if is_resource {
            vec![supplement(2, 4, &[])]
        } else {
            vec![]
        };
        let map_schema = schema(
            0,
            &[node(5, &map_children), node(0, &word(7)), placeholder],
            &map_proof,
        );
        values
            .fill_actor_graph(ticket, base, 0, owned(list), &list_schema)
            .unwrap();
        values
            .fill_actor_graph(ticket, base, 1, owned(map), &map_schema)
            .unwrap();
        values.commit_actor_graph(ticket).unwrap();
        assert!(values.lists[&list].elements.is_empty());
        assert!(values.maps[&map].entries.is_empty());
        values.release_actors().unwrap();
        assert!(values.is_empty());
    }
}

#[test]
fn inactive_optional_and_result_arms_do_not_visit_authority_or_never_placeholders() {
    for placeholder in [node(11, &name("Network")), node(16, &[]), node(18, &[])] {
        let is_resource = placeholder[0] == 16;
        let mut values = NativeValues::default();
        let ticket = values.begin_actor_graph().unwrap();
        let base = values.reserve_actor_graph_base(ticket, 2).unwrap();
        let proof = if is_resource {
            vec![supplement(1, 4, &[])]
        } else {
            vec![]
        };
        let none = values.sum(SUM_FAILURE, 0, false).unwrap();
        let none_schema = schema(0, &[node(6, &word(1)), placeholder.clone()], &proof);
        values
            .fill_actor_graph(ticket, base, 0, owned(none), &none_schema)
            .unwrap();
        let message = values.insert("ordinary fail".into()).unwrap();
        let fail = values.sum(SUM_FAILURE, message, true).unwrap();
        let mut children = word(1);
        children.extend(word(2));
        let fail_schema = schema(
            0,
            &[node(7, &children), placeholder, node(0, &word(11))],
            &proof,
        );
        values
            .fill_actor_graph(ticket, base, 1, owned(fail), &fail_schema)
            .unwrap();
        values.commit_actor_graph(ticket).unwrap();
        assert_eq!(values.sums[&none].tag, SUM_FAILURE);
        assert_eq!(values.sums[&fail].bits, message);
        values.release_actors().unwrap();
        assert!(values.is_empty());
    }
}

#[test]
fn ordinary_function_adapter_internal_label_aliases_transfer_once_and_drop_per_edge() {
    for publish in [false, true] {
        let mut values = NativeValues::default();
        let ticket = values.begin_actor_graph().unwrap();
        let base = values.reserve_actor_graph_base(ticket, 1).unwrap();
        let label = values.insert("function(source)".into()).unwrap();
        let source = values.new_struct(3).unwrap();
        values.structs.get_mut(&source).unwrap().fields =
            vec![Some(raw(321)), Some(raw(0)), Some(owned(label))];
        let adapter = values.function_adapter(source, 654).unwrap();
        values.drop_value(source).unwrap();
        assert_eq!(values.strings[&label].references, 2);
        let environment = values.struct_field(adapter, 1).unwrap().bits;
        let captured = values.struct_field(environment, 0).unwrap().bits;
        assert_eq!(values.struct_field(adapter, 2).unwrap().bits, label);
        assert_eq!(values.struct_field(captured, 2).unwrap().bits, label);
        let mut outer = 654u64.to_le_bytes().to_vec();
        outer.extend(word(1));
        let mut inner = 321u64.to_le_bytes().to_vec();
        inner.extend(word(u32::MAX));
        let layout = schema(
            0,
            &[
                node(13, &[]),
                record_node(&[2, 3]),
                node(13, &[]),
                node(0, &word(7)),
            ],
            &[supplement(0, 1, &outer), supplement(2, 1, &inner)],
        );
        values
            .fill_actor_graph(ticket, base, 0, owned(adapter), &layout)
            .unwrap();
        assert!(!values.strings.contains_key(&label));
        let staged = &values.actor_graph_imports[&ticket].ordinary;
        assert_eq!(staged.strings.len(), 1);
        assert_eq!(staged.strings[&label].references, 2);
        if publish {
            values.commit_actor_graph(ticket).unwrap();
            assert_eq!(values.strings[&label].references, 2);
            values.release_actors().unwrap();
        } else {
            values.abort_actor_graph(ticket).unwrap();
        }
        assert!(values.is_empty());
        assert_eq!(values.structs_created, values.structs_destroyed);
    }
}

#[test]
fn unaccounted_external_older_foreign_strings_and_shared_mutable_owners_still_refuse() {
    let mut values = NativeValues::default();
    let mut foreign = NativeValues::default();
    let older = values.insert("older".into()).unwrap();
    let ticket = values.begin_actor_graph().unwrap();
    let base = values.reserve_actor_graph_base(ticket, 1).unwrap();
    let fresh = values.insert("external alias".into()).unwrap();
    values.retain(fresh).unwrap();
    assert_eq!(
        values.fill_actor_graph(ticket, base, 0, owned(fresh), &string_schema()),
        Err(INVALID_IMPORT)
    );
    assert_eq!(values.strings[&fresh].references, 2);
    assert!(
        values.actor_graph_imports[&ticket]
            .ordinary
            .strings
            .is_empty()
    );
    assert_eq!(
        values.fill_actor_graph(ticket, base, 0, owned(older), &string_schema()),
        Err(INVALID_IMPORT)
    );
    let alien = foreign.insert("foreign".into()).unwrap();
    assert_eq!(
        values.fill_actor_graph(ticket, base, 0, owned(alien), &string_schema()),
        Err(INVALID_IMPORT)
    );
    let list = values.new_list(false).unwrap();
    let record = values.new_struct(2).unwrap();
    values.structs.get_mut(&record).unwrap().fields = vec![Some(owned(list)), Some(owned(list))];
    let layout = schema(
        0,
        &[record_node(&[1, 1]), node(3, &word(2)), node(0, &word(7))],
        &[],
    );
    assert_eq!(
        values.fill_actor_graph(ticket, base, 0, owned(record), &layout),
        Err(INVALID_IMPORT)
    );
    assert!(values.structs.contains_key(&record));
    assert!(values.lists.contains_key(&list));
    // Repair only this deliberately malformed test input's duplicate owner.
    values.structs.get_mut(&record).unwrap().fields[1] = None;
    values.drop_value(record).unwrap();
    values.drop_value(fresh).unwrap();
    values.drop_value(fresh).unwrap();
    values.drop_value(older).unwrap();
    foreign.drop_value(alien).unwrap();
    values.abort_actor_graph(ticket).unwrap();
    assert!(values.is_empty());
    assert!(foreign.is_empty());
}

#[test]
fn graph_output_root_is_isolated_until_commit_without_a_dummy_actor_or_ordinal() {
    let mut values = NativeValues::default();
    let ticket = values.begin_actor_graph().unwrap();
    let actor = values.reserve_actor_graph_base(ticket, 0).unwrap();
    let text = values.insert("root".into()).unwrap();
    let root = values.new_struct(2).unwrap();
    values.structs.get_mut(&root).unwrap().fields = vec![Some(raw(actor)), Some(owned(text))];
    let layout = schema(
        0,
        &[record_node(&[1, 2]), node(14, &[]), node(0, &word(11))],
        &[],
    );
    assert_eq!(
        values.fill_actor_graph_root(ticket, owned(root), &layout),
        Ok(0)
    );
    assert!(!values.structs.contains_key(&root));
    assert!(!values.strings.contains_key(&text));
    assert!(values.struct_field(root, 0).is_err());
    assert!(values.actor_identity_parts(actor).is_err());
    let staged = &values.actor_graph_imports[&ticket];
    assert_eq!(staged.output.unwrap().bits, root);
    assert!(staged.ordinary.structs.contains_key(&root));
    assert_eq!(values.next_actor_ordinal, 0);
    values.commit_actor_graph(ticket).unwrap();
    assert_eq!(values.struct_field(root, 0).unwrap().bits, actor);
    assert_eq!(values.text(text).unwrap(), "root");
    assert_eq!(values.actors.len(), 1);
    assert!(!values.actors.contains_key(&root));
    assert_eq!(values.next_actor_ordinal, 1);
    values.drop_value(root).unwrap();
    values.release_actors().unwrap();
    assert!(values.is_empty());
}

#[test]
fn graph_output_only_import_publishes_owned_and_nonowned_roots_without_actor_identity() {
    let mut values = NativeValues::default();
    let ticket = values.begin_actor_graph().unwrap();
    let text = values.insert("output only".into()).unwrap();
    values
        .fill_actor_graph_root(ticket, owned(text), &string_schema())
        .unwrap();
    values.commit_actor_graph(ticket).unwrap();
    assert_eq!(values.text(text).unwrap(), "output only");
    assert!(values.actors.is_empty());
    assert_eq!(values.next_actor_ordinal, 0);
    values.drop_value(text).unwrap();
    let ticket = values.begin_actor_graph().unwrap();
    let scalar = schema(0, &[node(0, &word(10))], &[]);
    values
        .fill_actor_graph_root(ticket, raw(0), &scalar)
        .unwrap();
    values.commit_actor_graph(ticket).unwrap();
    assert_eq!(values.next_actor_ordinal, 0);
    assert!(values.is_empty());
}

#[test]
fn graph_output_abort_drops_internal_adapter_label_aliases_and_preserves_older_actor() {
    let mut values = NativeValues::default();
    let older = values.new_struct(0).unwrap();
    values.register_actor(older).unwrap();
    let ticket = values.begin_actor_graph().unwrap();
    let label = values.insert("function(source)".into()).unwrap();
    let source = values.new_struct(3).unwrap();
    values.structs.get_mut(&source).unwrap().fields =
        vec![Some(raw(321)), Some(raw(0)), Some(owned(label))];
    let root = values.function_adapter(source, 654).unwrap();
    values.drop_value(source).unwrap();
    let mut outer = 654u64.to_le_bytes().to_vec();
    outer.extend(word(1));
    let mut inner = 321u64.to_le_bytes().to_vec();
    inner.extend(word(u32::MAX));
    let layout = schema(
        0,
        &[
            node(13, &[]),
            record_node(&[2, 3]),
            node(13, &[]),
            node(0, &word(7)),
        ],
        &[supplement(0, 1, &outer), supplement(2, 1, &inner)],
    );
    values
        .fill_actor_graph_root(ticket, owned(root), &layout)
        .unwrap();
    assert_eq!(
        values.actor_graph_imports[&ticket].ordinary.strings[&label].references,
        2
    );
    values.abort_actor_graph(ticket).unwrap();
    assert!(values.actor_identity_parts(older).is_ok());
    assert_eq!(values.next_actor_ordinal, 1);
    assert_eq!(values.structs_created - values.structs_destroyed, 1);
    assert!(values.strings.is_empty());
    values.release_actors().unwrap();
    assert!(values.is_empty());
}

#[test]
fn graph_output_duplicate_external_foreign_and_allocation_refusals_preserve_caller_owners() {
    let mut values = NativeValues::default();
    let ticket = values.begin_actor_graph().unwrap();
    let first = values.insert("first".into()).unwrap();
    values
        .fill_actor_graph_root(ticket, owned(first), &string_schema())
        .unwrap();
    let second = values.insert("second".into()).unwrap();
    assert_eq!(
        values.fill_actor_graph_root(ticket, owned(second), &string_schema()),
        Err(INVALID_IMPORT)
    );
    assert!(values.strings.contains_key(&second));
    values.abort_actor_graph(ticket).unwrap();
    values.drop_value(second).unwrap();
    let ticket = values.begin_actor_graph().unwrap();
    let shared = values.insert("external".into()).unwrap();
    values.retain(shared).unwrap();
    assert_eq!(
        values.fill_actor_graph_root(ticket, owned(shared), &string_schema()),
        Err(INVALID_IMPORT)
    );
    assert_eq!(values.strings[&shared].references, 2);
    values.drop_value(shared).unwrap();
    values.drop_value(shared).unwrap();
    let mut foreign = NativeValues::default();
    let alien = foreign.insert("alien".into()).unwrap();
    assert_eq!(
        values.fill_actor_graph_root(ticket, owned(alien), &string_schema()),
        Err(INVALID_IMPORT)
    );
    foreign.drop_value(alien).unwrap();
    let fresh = values.insert("allocation".into()).unwrap();
    values.allocation_budget = Some(0);
    assert_eq!(
        values.fill_actor_graph_root(ticket, owned(fresh), &string_schema()),
        Err(EXHAUSTED)
    );
    assert!(values.strings.contains_key(&fresh));
    values.allocation_budget = None;
    values.drop_value(fresh).unwrap();
    values.abort_actor_graph(ticket).unwrap();
    assert!(values.is_empty());
    assert!(foreign.is_empty());
}

#[test]
fn graph_root_fill_native_failure_aborts_owned_output_before_genuine_capture_and_reentry() {
    use AbiScalar::{I32, I64, Pointer};
    assert_eq!(
        NativeLeaf::ActorGraphRootFill.parameters(),
        &[Pointer, I64, I64, I32, I64, Pointer, I64]
    );
    let mut context = Box::new(JettRuntimeContextV1::retired());
    let mut result = std::mem::MaybeUninit::uninit();
    assert_eq!(
        unsafe { jett_rt_v1_context_create(1, &mut *context, result.as_mut_ptr()) },
        JettRuntimeStatusV1::OK
    );
    let pointer = &*context as *const JettRuntimeContextV1;
    let ticket = unsafe { jett_rt_v1_actor_graph_begin(pointer) };
    let text = b"root";
    let root = unsafe { jett_rt_v1_string_literal(pointer, text.as_ptr(), text.len() as u64) };
    let layout = string_schema();
    assert_eq!(
        unsafe {
            jett_rt_v1_actor_graph_root_fill(
                pointer,
                ticket,
                root,
                1,
                0,
                layout.as_ptr(),
                layout.len() as u64,
            )
        },
        0
    );
    assert_eq!(unsafe { jett_rt_v1_string_scalar_count(pointer, root) }, 0);
    assert_eq!(
        unsafe { jett_rt_v1_value_status(pointer) },
        JettRuntimeStatusV1::INVALID_ARGUMENT.code()
    );
    {
        let lease = acquire_context(context_key(pointer).unwrap()).unwrap();
        let state = lock_unpoisoned(&lease.entry.state);
        let values = &state.as_ref().unwrap().values;
        assert!(values.is_empty());
        assert!(!values.cleanup_failed);
        assert!(!values.strings.contains_key(&root));
    }
    let captured = unsafe { jett_rt_v1_failure_take_prefixed_text(pointer, std::ptr::null(), 0) };
    assert_ne!(captured, 0);
    assert_eq!(unsafe { jett_rt_v1_string_release(pointer, captured) }, 0);
    let next = unsafe { jett_rt_v1_actor_graph_begin(pointer) };
    assert!(next > ticket);
    let scalar = schema(0, &[node(0, &word(10))], &[]);
    assert_eq!(
        unsafe {
            jett_rt_v1_actor_graph_root_fill(
                pointer,
                next,
                0,
                0,
                0,
                scalar.as_ptr(),
                scalar.len() as u64,
            )
        },
        0
    );
    assert_eq!(unsafe { jett_rt_v1_actor_graph_commit(pointer, next) }, 0);
    assert_eq!(unsafe { jett_rt_v1_value_status(pointer) }, 0);
    assert_eq!(
        unsafe { jett_rt_v1_context_destroy(&mut *context, result.as_mut_ptr()) },
        JettRuntimeStatusV1::OK
    );
}
