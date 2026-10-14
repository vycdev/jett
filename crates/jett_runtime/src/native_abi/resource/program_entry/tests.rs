//! Startup controls, not a real provider or normal-archive execution receipt.
use super::*;
use crate::native_abi::resource::tests::{context, entry, layout_with_entry_network};

fn pointer(authenticated: &AuthenticatedResourceContext) -> *const JettRuntimeContextV1 {
    authenticated.key.owner_address as *const JettRuntimeContextV1
}
fn manifest(wire: &[u8]) -> ObjectManifest {
    let entry = entry();
    ObjectManifest {
        layout: wire.as_ptr(),
        layout_length: wire.len() as u64,
        function: entry.function,
        signature: entry.signature,
        scope: entry.scope,
        reserved: 0,
    }
}
fn retained(
    authenticated: &AuthenticatedResourceContext,
) -> (usize, NativeResourceCompletion, JettRuntimeStatusV1) {
    authenticated
        .with_state(|_, state| {
            let resource = state.resource_state.as_ref().unwrap();
            let completed = resource.completed.last().unwrap();
            Ok((
                resource.completed.len(),
                completed.completion,
                completed.terminal_status,
            ))
        })
        .unwrap()
}

fn graph_schema(tag: u8, hook: bool) -> Vec<u8> {
    let mut debug = b"JD\x01".to_vec();
    for word in [1_u32, 0, 1] {
        debug.extend_from_slice(&word.to_le_bytes());
    }
    debug.push(tag);
    let mut bytes = b"JA\x01".to_vec();
    bytes.extend_from_slice(&(debug.len() as u32).to_le_bytes());
    bytes.extend(debug);
    bytes.extend_from_slice(&u32::from(hook).to_le_bytes());
    if hook {
        bytes.extend_from_slice(&0_u32.to_le_bytes());
        bytes.push(7);
        for word in [12_u32, 0, 0] {
            bytes.extend_from_slice(&word.to_le_bytes());
        }
    }
    bytes
}

fn stage_graph_hook_actor(auth: &AuthenticatedResourceContext) -> (u64, u64, u64) {
    let pointer = pointer(auth);
    let mut descriptor = 0;
    assert_eq!(
        unsafe { leaves::jett_rt_v1_resource_descriptor(pointer, 12, &mut descriptor) },
        JettRuntimeStatusV1::OK
    );
    let ticket = unsafe { values::jett_rt_v1_actor_graph_begin(pointer) };
    let actor = unsafe { values::jett_rt_v1_actor_graph_reserve_base(pointer, ticket, 1) };
    let mut original = 0;
    assert_eq!(
        unsafe {
            leaves::jett_rt_v1_resource_graph_descriptor_box(
                pointer,
                ticket,
                12,
                descriptor,
                &mut original,
            )
        },
        JettRuntimeStatusV1::OK
    );
    let boxed = unsafe { values::jett_rt_v1_struct_clone(pointer, original) };
    assert_ne!(boxed, 0);
    assert_ne!(boxed, original);
    assert_eq!(
        unsafe { values::jett_rt_v1_value_drop(pointer, original) },
        0
    );
    let hook = graph_schema(13, true);
    assert_eq!(
        unsafe {
            values::jett_rt_v1_actor_graph_fill(
                pointer,
                ticket,
                actor,
                0,
                boxed,
                1,
                0,
                hook.as_ptr(),
                hook.len() as u64,
            )
        },
        0
    );
    (ticket, actor, boxed)
}

fn publish_graph_hook_actor(auth: &AuthenticatedResourceContext) -> u64 {
    let pointer = pointer(auth);
    let (ticket, actor, boxed) = stage_graph_hook_actor(auth);
    let root = graph_schema(14, false);
    assert_eq!(
        unsafe {
            values::jett_rt_v1_actor_graph_root_fill(
                pointer,
                ticket,
                actor,
                0,
                0,
                root.as_ptr(),
                root.len() as u64,
            )
        },
        0
    );
    assert_eq!(
        unsafe { values::jett_rt_v1_actor_graph_commit(pointer, ticket) },
        0
    );
    assert_eq!(
        unsafe { values::jett_rt_v1_actor_message_check(pointer, actor) },
        0
    );
    auth.with_state(|_, state| {
        assert!(!state.values.is_empty());
        assert!(state.values.published_checked_graph_hook(boxed).is_ok());
        Ok(())
    })
    .unwrap();
    boxed
}

#[test]
fn generated_startup_retires_graph_hook_actor_once_and_reentry_stays_empty() {
    context(|auth| {
        let wire = layout_with_entry_network(false);
        for ordinal in 1..=2 {
            let attempt = begin(auth, &wire, entry()).unwrap();
            let boxed = publish_graph_hook_actor(auth);
            assert_ne!(
                unsafe { jett_rt_v1_resource_program_complete(pointer(auth), attempt + 1, 0) },
                JettRuntimeStatusV1::OK
            );
            auth.with_state(|_, state| {
                assert!(!state.values.is_empty());
                assert!(state.values.published_checked_graph_hook(boxed).is_ok());
                Ok(())
            })
            .unwrap();
            assert_eq!(
                unsafe { jett_rt_v1_resource_program_complete(pointer(auth), attempt, 0) },
                JettRuntimeStatusV1::OK
            );
            assert_eq!(retained(auth).0, ordinal);
            assert_eq!(retained(auth).1.selected_kind, 0);
            auth.with_state(|_, state| {
                assert!(state.values.is_empty());
                assert!(state.values.published_checked_graph_hook(boxed).is_err());
                assert!(!state.values.cleanup_failed);
                Ok(())
            })
            .unwrap();
            let mut kind = 99;
            assert_eq!(
                unsafe { jett_rt_v1_resource_program_unwind(pointer(auth), 0, 0, &mut kind) },
                JettRuntimeStatusV1::OK
            );
            assert_eq!(kind, 0);
            assert_eq!(retained(auth).0, ordinal);
        }
    });
}

#[test]
fn generated_startup_aborts_unfinished_graph_hook_import_before_empty_audit() {
    context(|auth| {
        let wire = layout_with_entry_network(false);
        let attempt = begin(auth, &wire, entry()).unwrap();
        stage_graph_hook_actor(auth);
        assert_eq!(
            unsafe { jett_rt_v1_resource_program_complete(pointer(auth), attempt, 5) },
            JettRuntimeStatusV1(5)
        );
        assert_eq!(retained(auth).1.body_status, 5);
        assert_eq!(retained(auth).1.cleanup_status, 0);
        assert_eq!(retained(auth).1.selected_kind, 1);
        auth.with_state(|_, state| {
            assert!(state.values.is_empty());
            assert!(!state.values.cleanup_failed);
            Ok(())
        })
        .unwrap();
        let next = begin(auth, &wire, entry()).unwrap();
        assert_eq!(
            unsafe { jett_rt_v1_resource_program_complete(pointer(auth), next, 0) },
            JettRuntimeStatusV1::OK
        );
    });
}

#[test]
fn generated_startup_graph_actor_retirement_preserves_first_body_failure() {
    context(|auth| {
        let wire = layout_with_entry_network(false);
        let attempt = begin(auth, &wire, entry()).unwrap();
        publish_graph_hook_actor(auth);
        assert_ne!(unsafe { values::jett_rt_v1_assert_fail(pointer(auth)) }, 0);
        assert_eq!(
            unsafe { jett_rt_v1_resource_program_complete(pointer(auth), attempt, 7) },
            JettRuntimeStatusV1(7)
        );
        assert_eq!(retained(auth).1.body_status, 7);
        assert_eq!(retained(auth).1.cleanup_status, 0);
        assert_eq!(retained(auth).1.selected_kind, 1);
        let mut bytes = [0_u8; 64];
        let mut length = 0;
        let mut result = JettRuntimeResultV1::ok();
        assert_eq!(
            unsafe {
                jett_rt_v1_resource_program_failure_copy(
                    pointer(auth),
                    bytes.as_mut_ptr(),
                    bytes.len() as u64,
                    &mut length,
                    &mut result,
                )
            },
            JettRuntimeStatusV1::OK
        );
        assert_eq!(&bytes[..length as usize], b"assertion failed");
        let next = begin(auth, &wire, entry()).unwrap();
        assert_eq!(
            unsafe { jett_rt_v1_resource_program_complete(pointer(auth), next, 0) },
            JettRuntimeStatusV1::OK
        );
        assert_eq!(retained(auth).0, 2);
    });
}

#[test]
fn generated_startup_graph_actor_cleanup_poison_keeps_failure_and_blocks_reentry() {
    let mut record = Box::new(JettRuntimeContextV1::retired());
    let mut result = JettRuntimeResultV1::ok();
    assert_eq!(
        unsafe {
            jett_rt_v1_context_create(JETT_RUNTIME_ABI_VERSION_V1, &mut *record, &mut result)
        },
        JettRuntimeStatusV1::OK
    );
    let auth = AuthenticatedResourceContext::acquire(&*record).unwrap();
    let wire = layout_with_entry_network(false);
    let attempt = begin(&auth, &wire, entry()).unwrap();
    publish_graph_hook_actor(&auth);
    let list = unsafe { values::jett_rt_v1_list_new(pointer(&auth), 0) };
    assert_ne!(list, 0);
    assert_ne!(unsafe { values::jett_rt_v1_assert_fail(pointer(&auth)) }, 0);
    assert_eq!(
        unsafe { values::jett_rt_v1_value_drop(pointer(&auth), list) },
        0
    );
    assert_ne!(
        unsafe { values::jett_rt_v1_value_drop(pointer(&auth), list) },
        0
    );
    assert_eq!(
        unsafe { jett_rt_v1_resource_program_complete(pointer(&auth), attempt, 7) },
        JettRuntimeStatusV1::INVALID_ARGUMENT
    );
    assert_eq!(retained(&auth).1.body_status, 7);
    assert_eq!(retained(&auth).1.selected_kind, 3);
    assert_ne!(retained(&auth).1.cleanup_status, 0);
    let mut bytes = [0_u8; 64];
    let mut length = 0;
    assert_eq!(
        unsafe {
            jett_rt_v1_resource_program_failure_copy(
                pointer(&auth),
                bytes.as_mut_ptr(),
                bytes.len() as u64,
                &mut length,
                &mut result,
            )
        },
        JettRuntimeStatusV1::OK
    );
    assert_eq!(&bytes[..length as usize], b"native value cleanup failed");
    assert_eq!(
        unsafe {
            values::jett_rt_v1_value_failure_copy(
                pointer(&auth),
                bytes.as_mut_ptr(),
                bytes.len() as u64,
                &mut length,
                &mut result,
            )
        },
        JettRuntimeStatusV1::OK
    );
    assert_eq!(&bytes[..length as usize], b"assertion failed");
    auth.with_state(|_, state| {
        assert!(state.values.is_empty());
        assert!(state.values.cleanup_failed);
        Ok(())
    })
    .unwrap();
    assert!(begin(&auth, &wire, entry()).is_err());
    assert_eq!(retained(&auth).0, 1);
    drop(auth);
    assert_eq!(
        unsafe { jett_rt_v1_context_destroy(&mut *record, &mut result) },
        JettRuntimeStatusV1::INVALID_ARGUMENT
    );
}

#[test]
fn generated_startup_retains_wire_and_disabled_refusal() {
    context(|auth| {
        let wire = layout_with_entry_network(true);
        let object = manifest(&wire);
        let mut attempt = 99;
        let mut result = JettRuntimeResultV1::ok();
        assert_eq!(
            unsafe {
                jett_rt_v1_resource_program_begin(pointer(auth), &object, &mut attempt, &mut result)
            },
            JettRuntimeStatusV1::OK
        );
        assert_ne!(attempt, 0);
        auth.with_state(|_, state| {
            let resource = state.resource_state.as_ref().unwrap();
            assert_eq!(resource.installed_wire, wire);
            assert!(!resource.provider.enabled());
            assert!(resource.network.is_none());
            assert_eq!(
                resource.entry_network(ResourceHandleId::new(attempt)?, 0),
                Err(NativeResourceError::WrongGrant)
            );
            assert_eq!(
                resource.attempt.as_ref().unwrap().purpose,
                ResourcePurpose::Runtime
            );
            Ok(())
        })
        .unwrap();
        assert_eq!(
            unsafe { jett_rt_v1_resource_program_complete(pointer(auth), attempt, 0) },
            JettRuntimeStatusV1::OK
        );
        assert_eq!(retained(auth).1.body_status, 0);
    });
}

#[test]
fn exact_reentry_preserves_history_and_refuses_swapped_manifest() {
    context(|auth| {
        let wire = layout_with_entry_network(false);
        let first = begin(auth, &wire, entry()).unwrap();
        assert_eq!(
            complete(auth, ResourceHandleId::new(first).unwrap(), 5, false)
                .unwrap()
                .code(),
            5
        );
        let mut swapped = wire.clone();
        *swapped.last_mut().unwrap() ^= 1;
        assert!(matches!(
            begin(auth, &swapped, entry()),
            Err(NativeResourceError::InvalidEntry)
        ));
        assert_eq!(retained(auth).0, 1);
        assert_eq!(retained(auth).1.body_status, 5);
        let second = begin(auth, &wire, entry()).unwrap();
        assert_ne!(first, second);
        assert_eq!(
            complete(auth, ResourceHandleId::new(second).unwrap(), 0, false).unwrap(),
            JettRuntimeStatusV1::OK
        );
        assert_eq!(retained(auth).0, 2);
        auth.with_state(|_, state| {
            let resource = state.resource_state.as_ref().unwrap();
            assert_eq!(resource.completed[0].completion.body_status, 5);
            assert_eq!(resource.completed[0].terminal_status.code(), 5);
            assert_eq!(resource.custody.live_owners(), 0);
            assert_eq!(resource.custody.live_loans(), 0);
            assert_eq!(state.resources.live_count(), 0);
            Ok(())
        })
        .unwrap();
    });
}

#[test]
fn active_reentry_and_wrong_context_do_not_select_or_retire_an_attempt() {
    context(|first_context| {
        context(|second_context| {
            let wire = layout_with_entry_network(false);
            let attempt = begin(first_context, &wire, entry()).unwrap();
            assert!(matches!(
                begin(first_context, &wire, entry()),
                Err(NativeResourceError::ActiveAttempt)
            ));
            let second = begin(second_context, &wire, entry()).unwrap();
            assert!(
                complete(
                    second_context,
                    ResourceHandleId::new(attempt).unwrap(),
                    0,
                    false
                )
                .is_err()
            );
            assert_eq!(
                complete(
                    first_context,
                    ResourceHandleId::new(attempt).unwrap(),
                    0,
                    false
                )
                .unwrap(),
                JettRuntimeStatusV1::OK
            );
            assert_eq!(
                complete(
                    second_context,
                    ResourceHandleId::new(second).unwrap(),
                    0,
                    false
                )
                .unwrap(),
                JettRuntimeStatusV1::OK
            );
            assert!(
                complete(
                    first_context,
                    ResourceHandleId::new(attempt).unwrap(),
                    0,
                    false
                )
                .is_err()
            );
            assert_eq!(retained(first_context).0, 1);
        })
    });
}

#[test]
fn ordinary_first_error_is_copied_without_rewriting_body_or_poisoning_reentry() {
    context(|auth| {
        let wire = layout_with_entry_network(false);
        let attempt = begin(auth, &wire, entry()).unwrap();
        assert_ne!(unsafe { values::jett_rt_v1_assert_fail(pointer(auth)) }, 0);
        assert_eq!(
            complete(auth, ResourceHandleId::new(attempt).unwrap(), 0, false).unwrap(),
            JettRuntimeStatusV1::INVALID_ARGUMENT
        );
        let (_, completed, _) = retained(auth);
        assert_eq!(completed.body_status, 0);
        assert_eq!(completed.selected_kind, 1);
        let mut bytes = [0_u8; 64];
        let mut length = 0;
        let mut result = JettRuntimeResultV1::ok();
        assert_eq!(
            unsafe {
                jett_rt_v1_resource_program_failure_copy(
                    pointer(auth),
                    bytes.as_mut_ptr(),
                    bytes.len() as u64,
                    &mut length,
                    &mut result,
                )
            },
            JettRuntimeStatusV1::OK
        );
        assert_eq!(&bytes[..length as usize], b"assertion failed");
        let next = begin(auth, &wire, entry()).unwrap();
        assert_eq!(
            complete(auth, ResourceHandleId::new(next).unwrap(), 0, false).unwrap(),
            JettRuntimeStatusV1::OK
        );
        assert_eq!(retained(auth).0, 2);
    });
}

#[test]
fn first_resource_failure_and_cleanup_priority_retain_original_completion() {
    context(|auth| {
        let wire = layout_with_entry_network(false);
        let attempt = begin(auth, &wire, entry()).unwrap();
        auth.with_state(|_, state| {
            let resource = state.resource_state.as_mut().unwrap();
            resource.record_failure(NativeResourceError::Capacity);
            resource.record_failure(NativeResourceError::WrongGrant);
            Ok(())
        })
        .unwrap();
        assert_eq!(
            complete(auth, ResourceHandleId::new(attempt).unwrap(), 0, false).unwrap(),
            JettRuntimeStatusV1::RESOURCE_EXHAUSTED
        );
        assert_eq!(retained(auth).1.body_status, 0);
        let next = begin(auth, &wire, entry()).unwrap();
        auth.with_state(|_, state| {
            state
                .resource_state
                .as_mut()
                .unwrap()
                .record_failure(NativeResourceError::Cleanup(
                    ResourceCleanupFailure::FinalizerPanic,
                ));
            Ok(())
        })
        .unwrap();
        assert_eq!(
            complete(auth, ResourceHandleId::new(next).unwrap(), 5, true).unwrap(),
            JettRuntimeStatusV1::PANIC
        );
        assert_eq!(retained(auth).1.body_status, 5);
        assert_eq!(
            retained(auth).1.cleanup_status,
            JettRuntimeStatusV1::PANIC.code()
        );
        assert_eq!(retained(auth).1.selected_kind, 3);
    });
}

#[test]
fn recoverable_panic_completes_once_and_ordinary_context_is_noop() {
    context(|auth| {
        let mut kind = 99;
        assert_eq!(
            unsafe { jett_rt_v1_resource_program_unwind(pointer(auth), 255, 1, &mut kind) },
            JettRuntimeStatusV1::OK
        );
        assert_eq!(kind, 0);
        let wire = layout_with_entry_network(false);
        begin(auth, &wire, entry()).unwrap();
        assert_eq!(
            unsafe { jett_rt_v1_resource_program_unwind(pointer(auth), 255, 1, &mut kind) },
            JettRuntimeStatusV1::PANIC
        );
        assert_eq!(kind, 2);
        assert_eq!(retained(auth).1.body_status, 255);
        assert_eq!(retained(auth).0, 1);
        assert_eq!(
            unsafe { jett_rt_v1_resource_program_unwind(pointer(auth), 255, 1, &mut kind) },
            JettRuntimeStatusV1::OK
        );
        assert_eq!(retained(auth).0, 1);
    });
}

#[test]
fn context_overlap_and_reserved_manifest_refuse_before_installation() {
    context(|auth| {
        let wire = layout_with_entry_network(false);
        let mut object = manifest(&wire);
        let mut attempt = 99;
        let mut result = JettRuntimeResultV1::ok();
        assert_eq!(
            unsafe {
                jett_rt_v1_resource_program_begin(
                    pointer(auth),
                    &object,
                    &mut attempt,
                    pointer(auth).cast_mut().cast(),
                )
            },
            JettRuntimeStatusV1::INVALID_ARGUMENT
        );
        object.reserved = 1;
        assert_eq!(
            unsafe {
                jett_rt_v1_resource_program_begin(pointer(auth), &object, &mut attempt, &mut result)
            },
            JettRuntimeStatusV1::INVALID_ARGUMENT
        );
        assert_eq!(attempt, 99);
        auth.with_state(|_, state| {
            assert!(state.resource_state.is_none());
            Ok(())
        })
        .unwrap();
    });
}

#[test]
fn external_nonruntime_attempts_cannot_be_adopted_by_generated_startup() {
    for purpose in [
        ResourcePurpose::ExplicitComptime,
        ResourcePurpose::Verify,
        ResourcePurpose::Property,
    ] {
        context(|auth| {
            let wire = layout_with_entry_network(false);
            install_disabled(auth, &wire, entry()).unwrap();
            auth.with_state(|_, state| {
                let resource = state.resource_state.as_mut().unwrap();
                resource.begin_entry(&mut state.values, &state.resources, entry(), purpose)?;
                Ok(())
            })
            .unwrap();
            assert!(matches!(
                begin(auth, &wire, entry()),
                Err(NativeResourceError::ActiveAttempt)
            ));
            let mut kind = 99;
            assert_eq!(
                unsafe { jett_rt_v1_resource_program_unwind(pointer(auth), 0, 0, &mut kind) },
                JettRuntimeStatusV1::OK
            );
            assert_eq!(kind, 0);
            auth.with_state(|_, state| {
                let resource = state.resource_state.as_mut().unwrap();
                let attempt = resource.attempt.as_ref().unwrap().id;
                assert_eq!(resource.attempt.as_ref().unwrap().purpose, purpose);
                resource.complete_entry(
                    &mut state.values,
                    &mut state.resources,
                    attempt,
                    0,
                    false,
                )?;
                Ok(())
            })
            .unwrap();
        });
    }
}

#[cfg(all(test, jett_resource_native_test_archive))]
#[test]
fn private_scripted_handoff_borrows_only_the_exact_runtime_attempt() {
    use crate::native_abi::resource::tests::script;
    for purpose in [
        ResourcePurpose::Runtime,
        ResourcePurpose::ExplicitComptime,
        ResourcePurpose::Verify,
        ResourcePurpose::Property,
    ] {
        context(|auth| {
            let wire = layout_with_entry_network(false);
            install_scripted(auth, &wire, entry(), script(&[], 1)).unwrap();
            let attempt = auth
                .with_state(|_, state| {
                    let resource = state.resource_state.as_mut().unwrap();
                    resource.begin_entry(&mut state.values, &state.resources, entry(), purpose)
                })
                .unwrap();
            if purpose == ResourcePurpose::Runtime {
                assert_eq!(begin(auth, &wire, entry()).unwrap(), 0);
            } else {
                assert!(matches!(
                    begin(auth, &wire, entry()),
                    Err(NativeResourceError::ActiveAttempt)
                ));
            }
            assert!(complete(auth, attempt, 0, false).is_err());
            auth.with_state(|_, state| {
                let resource = state.resource_state.as_mut().unwrap();
                assert_eq!(resource.entry_owner, EntryOwner::External);
                assert_eq!(resource.attempt.as_ref().unwrap().id, attempt);
                resource.complete_entry(
                    &mut state.values,
                    &mut state.resources,
                    attempt,
                    0,
                    false,
                )?;
                assert_eq!(resource.completed.len(), 1);
                Ok(())
            })
            .unwrap();
        });
    }
}
