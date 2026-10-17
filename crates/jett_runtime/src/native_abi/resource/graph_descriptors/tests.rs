//! Host controls for the private checked metadata carrier, using real ABI leaves.
use super::*;

fn pointer(auth: &AuthenticatedResourceContext) -> *const JettRuntimeContextV1 {
    auth.key.owner_address as *const JettRuntimeContextV1
}

fn setup(
    auth: &AuthenticatedResourceContext,
) -> (ResourceHandleId, ResourceHandleId, ResourceHandleId) {
    install_disabled(auth, &layout_with_entry_network(false), entry()).unwrap();
    auth.with_resource(|state, ordinary, registry| {
        let attempt = state.begin_entry(ordinary, registry, entry(), ResourcePurpose::Runtime)?;
        let root = state.root_frame(attempt)?;
        let frame = state.begin_operation_frame(ordinary, 1, root)?;
        let descriptor = state.descriptor(ordinary, 12)?;
        Ok((attempt, frame, descriptor))
    })
    .unwrap()
}

fn finish(auth: &AuthenticatedResourceContext, attempt: ResourceHandleId, expected_body: u32) {
    auth.with_resource(|state, ordinary, registry| {
        ordinary.release_actors().map_err(ordinary_error)?;
        assert!(ordinary.is_empty());
        let completion = state.complete_entry(ordinary, registry, attempt, 0, false)?;
        assert_eq!(completion.selected_kind, expected_body);
        Ok(())
    })
    .unwrap();
    assert_empty(auth);
}

fn schema(operation: u32, hook: u32, signature: u32) -> Vec<u8> {
    let mut debug = b"JD\x01".to_vec();
    words(&mut debug, &[1, 0, 1]);
    debug.push(13); // Function; supplement7 carries protected hook metadata.
    let mut bytes = b"JA\x01".to_vec();
    words(&mut bytes, &[debug.len() as u32]);
    bytes.extend(debug);
    words(&mut bytes, &[1, 0]);
    bytes.push(7);
    words(&mut bytes, &[operation, hook, signature]);
    bytes
}

fn boxed(auth: &AuthenticatedResourceContext, ticket: u64, descriptor: ResourceHandleId) -> u64 {
    let mut out = 0;
    assert_eq!(
        unsafe {
            leaves::jett_rt_v1_resource_graph_descriptor_box(
                pointer(auth),
                ticket,
                12,
                descriptor.raw(),
                &mut out,
            )
        },
        JettRuntimeStatusV1::OK
    );
    assert_ne!(out, 0);
    out
}

fn fill(
    auth: &AuthenticatedResourceContext,
    ticket: u64,
    base: u64,
    field: u64,
    value: u64,
    bytes: &[u8],
) -> u32 {
    unsafe {
        values::jett_rt_v1_actor_graph_fill(
            pointer(auth),
            ticket,
            base,
            field,
            value,
            1,
            0,
            bytes.as_ptr(),
            bytes.len() as u64,
        )
    }
}

fn capture(auth: &AuthenticatedResourceContext) {
    let captured =
        unsafe { values::jett_rt_v1_failure_take_prefixed_text(pointer(auth), ptr::null(), 0) };
    assert_ne!(captured, 0);
    assert_eq!(unsafe { values::jett_rt_v1_value_status(pointer(auth)) }, 0);
    assert_eq!(
        unsafe { values::jett_rt_v1_string_release(pointer(auth), captured) },
        0
    );
}

#[test]
fn checked_graph_hook_mint_requires_exact_descriptor_operation_family_signature_and_ticket() {
    context(|auth| {
        let (attempt, _, descriptor) = setup(auth);
        let ticket = unsafe { values::jett_rt_v1_actor_graph_begin(pointer(auth)) };
        auth.with_resource(|state, ordinary, _| {
            assert_eq!(
                state.graph_descriptor_box(ordinary, ticket, 0, descriptor),
                Err(NativeResourceError::WrongOperation)
            );
            assert_eq!(
                state.graph_descriptor_box(ordinary, ticket, 13, descriptor),
                Err(NativeResourceError::WrongOperation)
            );
            assert_eq!(
                state.graph_descriptor_box(ordinary, ticket, 12, state.program_handle),
                Err(NativeResourceError::WrongFamily)
            );
            state.handles.insert(
                descriptor,
                NativeResourceEntry::Descriptor {
                    hook: 1,
                    signature: 1,
                },
            );
            assert_eq!(
                state.graph_descriptor_box(ordinary, ticket, 12, descriptor),
                Err(NativeResourceError::WrongFamily)
            );
            state.handles.insert(
                descriptor,
                NativeResourceEntry::Descriptor {
                    hook: 0,
                    signature: 1,
                },
            );
            assert_eq!(
                state.graph_descriptor_box(ordinary, ticket, 12, descriptor),
                Err(NativeResourceError::WrongFamily)
            );
            state.handles.insert(
                descriptor,
                NativeResourceEntry::Descriptor {
                    hook: 0,
                    signature: 0,
                },
            );
            assert_eq!(
                state.graph_descriptor_box(ordinary, 0, 12, descriptor),
                Err(NativeResourceError::OrdinaryStorage(
                    JettRuntimeStatusV1::INVALID_ARGUMENT
                ))
            );
            Ok(())
        })
        .unwrap();
        let value = boxed(auth, ticket, descriptor);
        auth.with_resource(|state, ordinary, _| {
            assert!(
                state
                    .graph_descriptor_scalar(ordinary, 13, ResourceHandleId::new(value)?)
                    .is_err()
            );
            Ok(())
        })
        .unwrap();
        assert_eq!(
            unsafe { values::jett_rt_v1_value_drop(pointer(auth), value) },
            0
        );
        assert_eq!(
            unsafe { values::jett_rt_v1_actor_graph_abort(pointer(auth), ticket) },
            0
        );
        finish(auth, attempt, 0);
    });
}

#[test]
fn checked_graph_hook_schema_mismatches_roll_back_without_taking_caller_ownership() {
    for wrong in [(13, 0, 0), (12, 1, 0), (12, 0, 1)] {
        context(|auth| {
            let (attempt, _, descriptor) = setup(auth);
            let ticket = unsafe { values::jett_rt_v1_actor_graph_begin(pointer(auth)) };
            let base =
                unsafe { values::jett_rt_v1_actor_graph_reserve_base(pointer(auth), ticket, 1) };
            let value = boxed(auth, ticket, descriptor);
            assert_eq!(
                fill(
                    auth,
                    ticket,
                    base,
                    0,
                    value,
                    &schema(wrong.0, wrong.1, wrong.2)
                ),
                JettRuntimeStatusV1::INVALID_CONTEXT.code()
            );
            assert_eq!(
                unsafe { values::jett_rt_v1_value_status(pointer(auth)) },
                JettRuntimeStatusV1::INVALID_ARGUMENT.code()
            );
            capture(auth);
            assert_eq!(
                unsafe { values::jett_rt_v1_value_drop(pointer(auth), value) },
                0
            );
            let next = unsafe { values::jett_rt_v1_actor_graph_begin(pointer(auth)) };
            assert!(next > ticket);
            assert_eq!(
                unsafe { values::jett_rt_v1_actor_graph_abort(pointer(auth), next) },
                0
            );
            finish(auth, attempt, 0);
        });
    }
}

#[test]
fn checked_graph_hook_published_metadata_copies_keep_scalar_identity_and_exact_indirect_checks() {
    context(|auth| {
        let (attempt, frame, descriptor) = setup(auth);
        let ticket = unsafe { values::jett_rt_v1_actor_graph_begin(pointer(auth)) };
        let base = unsafe { values::jett_rt_v1_actor_graph_reserve_base(pointer(auth), ticket, 1) };
        let original = boxed(auth, ticket, descriptor);
        let value = unsafe { values::jett_rt_v1_struct_clone(pointer(auth), original) };
        assert_ne!(original, value);
        assert_ne!(value, 0);
        assert_eq!(
            unsafe { values::jett_rt_v1_value_drop(pointer(auth), original) },
            0
        );
        auth.with_resource(|state, ordinary, _| {
            assert!(
                state
                    .prepare_hook(ordinary, 13, frame, Some(ResourceHandleId::new(value)?))
                    .is_err()
            );
            Ok(())
        })
        .unwrap();
        assert_eq!(fill(auth, ticket, base, 0, value, &schema(12, 0, 0)), 0);
        assert_eq!(
            unsafe { values::jett_rt_v1_actor_graph_commit(pointer(auth), ticket) },
            0
        );
        let copy = unsafe { values::jett_rt_v1_struct_clone(pointer(auth), value) };
        let pending = unsafe { values::jett_rt_v1_record_run(pointer(auth), copy) };
        let joined = unsafe { values::jett_rt_v1_record_task_join(pointer(auth), pending) };
        assert_ne!(copy, 0);
        assert_ne!(pending, 0);
        assert_ne!(joined, 0);
        auth.with_resource(|state, ordinary, _| {
            assert!(
                state
                    .graph_descriptor_scalar(ordinary, 13, ResourceHandleId::new(pending)?)
                    .is_err()
            );
            assert_eq!(
                state.graph_descriptor_scalar(ordinary, 12, ResourceHandleId::new(copy)?),
                Err(NativeResourceError::WrongOperation)
            );
            assert_eq!(
                state.graph_descriptor_scalar(ordinary, 13, ResourceHandleId::new(joined)?)?,
                descriptor
            );
            let call =
                state.prepare_hook(ordinary, 13, frame, Some(ResourceHandleId::new(value)?))?;
            assert_eq!(state.checked_call(call)?.descriptor, Some(descriptor));
            assert_eq!(state.checked_call(call)?.target_operation, 0);
            state.handles.remove(&call);
            assert_eq!(
                state.prepare_hook(ordinary, 0, frame, Some(ResourceHandleId::new(value)?)),
                Err(NativeResourceError::WrongOperation)
            );
            Ok(())
        })
        .unwrap();
        for value in [copy, pending, joined] {
            assert_eq!(
                unsafe { values::jett_rt_v1_value_drop(pointer(auth), value) },
                0
            );
        }
        finish(auth, attempt, 0);
    });
}

#[test]
fn checked_graph_hook_refuses_ordinary_callable_and_field_abi_in_both_phases() {
    for publish in [false, true] {
        context(|auth| {
            let (attempt, _, descriptor) = setup(auth);
            let ticket = unsafe { values::jett_rt_v1_actor_graph_begin(pointer(auth)) };
            let value = boxed(auth, ticket, descriptor);
            if publish {
                let base = unsafe {
                    values::jett_rt_v1_actor_graph_reserve_base(pointer(auth), ticket, 1)
                };
                assert_eq!(fill(auth, ticket, base, 0, value, &schema(12, 0, 0)), 0);
                assert_eq!(
                    unsafe { values::jett_rt_v1_actor_graph_commit(pointer(auth), ticket) },
                    0
                );
            }
            assert_eq!(
                unsafe { values::jett_rt_v1_function_callable_check(pointer(auth), value) },
                JettRuntimeStatusV1::INVALID_CONTEXT.code()
            );
            assert_eq!(
                unsafe { values::jett_rt_v1_value_status(pointer(auth)) },
                JettRuntimeStatusV1::INVALID_ARGUMENT.code()
            );
            capture(auth);
            assert_eq!(
                unsafe { values::jett_rt_v1_struct_field(pointer(auth), value, 0) },
                0
            );
            assert_eq!(
                unsafe { values::jett_rt_v1_value_status(pointer(auth)) },
                JettRuntimeStatusV1::INVALID_ARGUMENT.code()
            );
            capture(auth);
            if !publish {
                // The ordinary failure already aborted the ticket; unfilled box stays caller-owned.
                assert_eq!(
                    unsafe { values::jett_rt_v1_value_drop(pointer(auth), value) },
                    0
                );
            }
            finish(auth, attempt, 0);
        });
    }
}

#[test]
fn checked_graph_hook_abort_drops_each_metadata_clone_and_old_factory_boxes_stay_unpublished() {
    context(|auth| {
        let (attempt, _, descriptor) = setup(auth);
        let ticket = unsafe { values::jett_rt_v1_actor_graph_begin(pointer(auth)) };
        let base = unsafe { values::jett_rt_v1_actor_graph_reserve_base(pointer(auth), ticket, 2) };
        let value = boxed(auth, ticket, descriptor);
        let copy = unsafe { values::jett_rt_v1_struct_clone(pointer(auth), value) };
        assert_eq!(fill(auth, ticket, base, 0, value, &schema(12, 0, 0)), 0);
        assert_eq!(fill(auth, ticket, base, 1, copy, &schema(12, 0, 0)), 0);
        assert_eq!(
            unsafe { values::jett_rt_v1_actor_graph_abort(pointer(auth), ticket) },
            0
        );
        auth.with_resource(|_, ordinary, _| {
            assert!(ordinary.is_empty());
            Ok(())
        })
        .unwrap();
        let ticket = unsafe { values::jett_rt_v1_actor_graph_begin(pointer(auth)) };
        let stale = boxed(auth, ticket, descriptor);
        assert_eq!(
            unsafe { values::jett_rt_v1_actor_graph_abort(pointer(auth), ticket) },
            0
        );
        let next = unsafe { values::jett_rt_v1_actor_graph_begin(pointer(auth)) };
        let base = unsafe { values::jett_rt_v1_actor_graph_reserve_base(pointer(auth), next, 1) };
        assert!(stale < next);
        auth.with_resource(|state, ordinary, _| {
            assert!(
                state
                    .graph_descriptor_scalar(ordinary, 13, ResourceHandleId::new(stale)?)
                    .is_err()
            );
            Ok(())
        })
        .unwrap();
        assert_eq!(
            fill(auth, next, base, 0, stale, &schema(12, 0, 0)),
            JettRuntimeStatusV1::INVALID_CONTEXT.code()
        );
        capture(auth);
        assert_eq!(
            unsafe { values::jett_rt_v1_value_drop(pointer(auth), stale) },
            0
        );
        finish(auth, attempt, 0);
    });
}

#[test]
fn checked_graph_hook_foreign_descriptors_and_boxes_never_select_same_context_authority() {
    context(|first| {
        context(|second| {
            let (first_attempt, _, first_descriptor) = setup(first);
            let (second_attempt, _, second_descriptor) = setup(second);
            let first_ticket = unsafe { values::jett_rt_v1_actor_graph_begin(pointer(first)) };
            let second_ticket = unsafe { values::jett_rt_v1_actor_graph_begin(pointer(second)) };
            first
                .with_resource(|state, ordinary, _| {
                    assert_eq!(
                        state.graph_descriptor_box(ordinary, first_ticket, 12, second_descriptor),
                        Err(NativeResourceError::WrongFamily)
                    );
                    assert!(
                        state
                            .graph_descriptor_box(ordinary, second_ticket, 12, first_descriptor)
                            .is_err()
                    );
                    Ok(())
                })
                .unwrap();
            let alien = boxed(second, second_ticket, second_descriptor);
            let base = unsafe {
                values::jett_rt_v1_actor_graph_reserve_base(pointer(first), first_ticket, 1)
            };
            assert_eq!(
                fill(first, first_ticket, base, 0, alien, &schema(12, 0, 0)),
                JettRuntimeStatusV1::INVALID_CONTEXT.code()
            );
            capture(first);
            assert_eq!(
                unsafe { values::jett_rt_v1_value_drop(pointer(second), alien) },
                0
            );
            assert_eq!(
                unsafe { values::jett_rt_v1_actor_graph_abort(pointer(second), second_ticket) },
                0
            );
            finish(first, first_attempt, 0);
            finish(second, second_attempt, 0);
        })
    });
}

#[test]
fn checked_graph_hook_failed_leaf_aborts_staging_and_preserves_older_actor_and_first_failure() {
    context(|auth| {
        let (attempt, _, descriptor) = setup(auth);
        let older = unsafe { values::jett_rt_v1_struct_new(pointer(auth), 0) };
        assert_eq!(
            unsafe { values::jett_rt_v1_actor_register(pointer(auth), older) },
            older
        );
        let ticket = unsafe { values::jett_rt_v1_actor_graph_begin(pointer(auth)) };
        let base = unsafe { values::jett_rt_v1_actor_graph_reserve_base(pointer(auth), ticket, 1) };
        let value = boxed(auth, ticket, descriptor);
        assert_eq!(fill(auth, ticket, base, 0, value, &schema(12, 0, 0)), 0);
        let mut out = 123;
        assert_eq!(
            unsafe {
                leaves::jett_rt_v1_resource_graph_descriptor_box(
                    pointer(auth),
                    ticket,
                    13,
                    descriptor.raw(),
                    &mut out,
                )
            },
            JettRuntimeStatusV1::INVALID_ARGUMENT
        );
        assert_eq!(out, 123);
        assert_eq!(
            unsafe { values::jett_rt_v1_value_status(pointer(auth)) },
            JettRuntimeStatusV1::INVALID_ARGUMENT.code()
        );
        capture(auth);
        assert_eq!(
            unsafe { values::jett_rt_v1_actor_message_check(pointer(auth), older) },
            0
        );
        let next = unsafe { values::jett_rt_v1_actor_graph_begin(pointer(auth)) };
        assert!(next > ticket);
        assert_eq!(
            unsafe { values::jett_rt_v1_actor_graph_abort(pointer(auth), next) },
            0
        );
        finish(auth, attempt, 1);
    });
}

#[test]
fn checked_graph_hook_root_fill_publishes_metadata_without_dummy_actor_and_returns_owned_root() {
    context(|auth| {
        let (attempt, frame, descriptor) = setup(auth);
        let ticket = unsafe { values::jett_rt_v1_actor_graph_begin(pointer(auth)) };
        let root = boxed(auth, ticket, descriptor);
        let bytes = schema(12, 0, 0);
        assert_eq!(
            unsafe {
                values::jett_rt_v1_actor_graph_root_fill(
                    pointer(auth),
                    ticket,
                    root,
                    1,
                    0,
                    bytes.as_ptr(),
                    bytes.len() as u64,
                )
            },
            0
        );
        auth.with_resource(|state, ordinary, _| {
            assert!(
                state
                    .graph_descriptor_scalar(ordinary, 13, ResourceHandleId::new(root)?)
                    .is_err()
            );
            Ok(())
        })
        .unwrap();
        assert_eq!(
            unsafe { values::jett_rt_v1_actor_graph_commit(pointer(auth), ticket) },
            0
        );
        auth.with_resource(|state, ordinary, _| {
            let call =
                state.prepare_hook(ordinary, 13, frame, Some(ResourceHandleId::new(root)?))?;
            assert_eq!(state.checked_call(call)?.descriptor, Some(descriptor));
            state.handles.remove(&call);
            Ok(())
        })
        .unwrap();
        assert_eq!(
            unsafe { values::jett_rt_v1_value_drop(pointer(auth), root) },
            0
        );
        finish(auth, attempt, 0);
    });
}
