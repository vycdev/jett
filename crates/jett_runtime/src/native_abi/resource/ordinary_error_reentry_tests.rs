//! Host lifetime controls. These are not Source admission or linked acceptance.
//! Registered as a child of `resource/tests.rs` to reuse its genuine fixture.
use super::*;
use std::ptr;

const BOUNDS: &[u8] = b"list.__remove_at: index -1 out of bounds";

fn pointer(auth: &AuthenticatedResourceContext) -> *const JettRuntimeContextV1 {
    auth.key.owner_address as *const JettRuntimeContextV1
}

fn current_message(auth: &AuthenticatedResourceContext) -> Vec<u8> {
    let mut length = 0;
    let mut result = JettRuntimeResultV1::ok();
    assert_eq!(
        unsafe {
            values::jett_rt_v1_value_failure_copy(
                pointer(auth),
                ptr::null_mut(),
                0,
                &mut length,
                &mut result,
            )
        },
        JettRuntimeStatusV1::OK
    );
    let mut bytes = vec![0; usize::try_from(length).unwrap()];
    assert_eq!(
        unsafe {
            values::jett_rt_v1_value_failure_copy(
                pointer(auth),
                bytes.as_mut_ptr(),
                length,
                &mut length,
                &mut result,
            )
        },
        JettRuntimeStatusV1::OK
    );
    bytes
}

fn saved(
    auth: &AuthenticatedResourceContext,
    attempt: ResourceHandleId,
) -> (NativeResourceCompletion, Vec<u8>, Vec<u8>) {
    auth.with_resource(|state, _, _| {
        let completion = state.completed_attempt(attempt)?;
        let (resource, ordinary) = state.completed_messages(attempt)?;
        Ok((completion, resource.to_vec(), ordinary.to_vec()))
    })
    .unwrap()
}

fn set_prefix(auth: &AuthenticatedResourceContext, prefix: &[u8]) {
    unsafe {
        let text = values::jett_rt_v1_string_literal(
            pointer(auth),
            prefix.as_ptr(),
            u64::try_from(prefix.len()).unwrap(),
        );
        assert_ne!(text, 0);
        assert_eq!(values::jett_rt_v1_property_case_set(pointer(auth), text), 0);
        assert_eq!(values::jett_rt_v1_string_release(pointer(auth), text), 0);
    }
}

fn bounds_failure(auth: &AuthenticatedResourceContext, index: i64) {
    unsafe {
        let list = values::jett_rt_v1_list_new(pointer(auth), 0);
        assert_ne!(list, 0);
        assert_eq!(values::jett_rt_v1_list_append(pointer(auth), list, 1), list);
        assert_eq!(
            values::jett_rt_v1_list_remove_at(pointer(auth), list, index),
            0
        );
        assert_eq!(
            values::jett_rt_v1_value_status(pointer(auth)),
            JettRuntimeStatusV1::INVALID_ARGUMENT.code()
        );
        assert_eq!(values::jett_rt_v1_value_drop(pointer(auth), list), 0);
    }
}

fn dynamic_failure(auth: &AuthenticatedResourceContext, message: &[u8]) {
    unsafe {
        let text = values::jett_rt_v1_string_literal(
            pointer(auth),
            message.as_ptr(),
            u64::try_from(message.len()).unwrap(),
        );
        assert_ne!(text, 0);
        assert_ne!(
            values::jett_rt_v1_assert_fail_message(pointer(auth), text),
            0
        );
        assert_eq!(values::jett_rt_v1_string_release(pointer(auth), text), 0);
    }
}

fn successful_leaf(auth: &AuthenticatedResourceContext) {
    unsafe {
        let list = values::jett_rt_v1_list_new(pointer(auth), 0);
        assert_ne!(list, 0);
        assert_eq!(
            values::jett_rt_v1_list_append(pointer(auth), list, 17),
            list
        );
        assert_eq!(values::jett_rt_v1_list_length(pointer(auth), list), 1);
        assert_eq!(values::jett_rt_v1_value_drop(pointer(auth), list), 0);
        assert_eq!(values::jett_rt_v1_value_status(pointer(auth)), 0);
    }
}

fn begin_only(auth: &AuthenticatedResourceContext) -> ResourceResult<ResourceHandleId> {
    // Deliberately use with_state: a refused host preflight is not a Source body fault.
    auth.with_state(|_, state| {
        let resource = state.resource_state.as_mut().unwrap();
        resource.begin_entry(
            &mut state.values,
            &state.resources,
            entry(),
            ResourcePurpose::Runtime,
        )
    })
}

fn assert_no_effects(auth: &AuthenticatedResourceContext) {
    assert_empty(auth);
    assert!(events(auth).is_empty());
    auth.with_resource(|state, _, _| {
        assert_eq!(state.script_remaining(), 0);
        Ok(())
    })
    .unwrap();
}

fn poisoned_context(operation: impl FnOnce(&AuthenticatedResourceContext)) {
    let mut record = Box::new(JettRuntimeContextV1::retired());
    let mut result = JettRuntimeResultV1::ok();
    assert_eq!(
        unsafe {
            jett_rt_v1_context_create(JETT_RUNTIME_ABI_VERSION_V1, &mut *record, &mut result)
        },
        JettRuntimeStatusV1::OK
    );
    let authenticated = AuthenticatedResourceContext::acquire(&*record).unwrap();
    operation(&authenticated);
    drop(authenticated);
    assert_eq!(
        unsafe { jett_rt_v1_context_destroy(&mut *record, &mut result) },
        JettRuntimeStatusV1::INVALID_ARGUMENT
    );
    assert_eq!(result.status, JettRuntimeStatusV1::INVALID_ARGUMENT);
    // No failure-taking operation or cleanup-poison reset is used for teardown.
}

#[test]
fn ordinary_error_reentry_signed_bounds_twice_keeps_both_records_and_grant() {
    context(|auth| {
        install(auth, &[], 2);
        let (first, _, _, grant) = start(auth, ResourcePurpose::Runtime);
        bounds_failure(auth, -1);
        assert_eq!(current_message(auth), BOUNDS);
        let first_completion = complete(auth, first, 1);
        assert_eq!(
            (
                first_completion.body_status,
                first_completion.cleanup_status,
                first_completion.selected_kind
            ),
            (1, 0, 1)
        );
        let first_saved = saved(auth, first);
        assert_eq!(first_saved.2, BOUNDS);
        assert_eq!(current_message(auth), BOUNDS);
        assert_no_effects(auth);

        let (second, _, _, second_grant) = start(auth, ResourcePurpose::Runtime);
        assert!(second.raw() > first.raw());
        assert_eq!(second_grant, grant);
        assert!(current_message(auth).is_empty());
        assert_eq!(unsafe { values::jett_rt_v1_value_status(pointer(auth)) }, 0);
        bounds_failure(auth, -1);
        let second_completion = complete(auth, second, 1);
        assert_eq!(
            (
                second_completion.body_status,
                second_completion.cleanup_status,
                second_completion.selected_kind
            ),
            (1, 0, 1)
        );
        assert_eq!(saved(auth, second).2, BOUNDS);
        assert_eq!(saved(auth, first), first_saved);
        assert_no_effects(auth);
    });
}

#[test]
fn ordinary_error_reentry_distinct_signed_errors_capture_distinct_prefixes() {
    context(|auth| {
        install(auth, &[], 2);
        let (first, _, _, grant) = start(auth, ResourcePurpose::Runtime);
        set_prefix(auth, b"property 'first' trial 2: ");
        bounds_failure(auth, -1);
        complete(auth, first, 17);
        let first_saved = saved(auth, first);
        assert_eq!(first_saved.0.body_status, 17);
        assert_eq!(
            first_saved.2,
            b"property 'first' trial 2: list.__remove_at: index -1 out of bounds"
        );

        let (second, _, _, second_grant) = start(auth, ResourcePurpose::Runtime);
        assert_eq!(grant, second_grant);
        set_prefix(auth, b"property 'second' trial 9: ");
        bounds_failure(auth, 7);
        complete(auth, second, 23);
        assert_eq!(
            saved(auth, second).2,
            b"property 'second' trial 9: list.__remove_at: index 7 out of bounds"
        );
        assert_eq!(saved(auth, first), first_saved);
        assert_no_effects(auth);
    });
}

#[test]
fn ordinary_error_reentry_failure_then_success_then_different_failure_runs_real_leaves() {
    context(|auth| {
        install(auth, &[], 3);
        let (first, _, _, grant) = start(auth, ResourcePurpose::Runtime);
        bounds_failure(auth, -1);
        complete(auth, first, 1);
        let first_saved = saved(auth, first);
        let (second, _, _, second_grant) = start(auth, ResourcePurpose::Runtime);
        assert_eq!(grant, second_grant);
        successful_leaf(auth);
        let success = complete(auth, second, 0);
        assert_eq!(
            (
                success.body_status,
                success.cleanup_status,
                success.selected_kind
            ),
            (0, 0, 0)
        );
        assert!(current_message(auth).is_empty());
        let success_saved = saved(auth, second);
        assert!(success_saved.1.is_empty() && success_saved.2.is_empty());

        let (third, _, _, third_grant) = start(auth, ResourcePurpose::Runtime);
        assert!(first.raw() < second.raw() && second.raw() < third.raw());
        assert_eq!(grant, third_grant);
        dynamic_failure(auth, b"third terminal message");
        complete(auth, third, 29);
        assert_eq!(saved(auth, third).2, b"third terminal message");
        assert_eq!(saved(auth, first), first_saved);
        assert_eq!(saved(auth, second), success_saved);
        assert_no_effects(auth);
    });
}

#[test]
fn ordinary_error_reentry_success_then_error_does_not_reuse_success_channel() {
    context(|auth| {
        install(auth, &[], 2);
        let (first, _, _, _) = start(auth, ResourcePurpose::Runtime);
        successful_leaf(auth);
        complete(auth, first, 0);
        let first_saved = saved(auth, first);
        let (second, _, _, _) = start(auth, ResourcePurpose::Runtime);
        bounds_failure(auth, -1);
        complete(auth, second, 1);
        assert_eq!(saved(auth, second).2, BOUNDS);
        assert_eq!(saved(auth, first), first_saved);
        assert_no_effects(auth);
    });
}

#[test]
fn ordinary_error_reentry_first_failure_blocks_later_normal_leaf_without_replacing_message() {
    context(|auth| {
        install(auth, &[], 1);
        let (attempt, _, _, _) = start(auth, ResourcePurpose::Runtime);
        let replacement = b"must never replace the first diagnostic";
        let text = unsafe {
            values::jett_rt_v1_string_literal(
                pointer(auth),
                replacement.as_ptr(),
                u64::try_from(replacement.len()).unwrap(),
            )
        };
        assert_ne!(text, 0);
        bounds_failure(auth, -1);
        assert_ne!(
            unsafe { values::jett_rt_v1_assert_fail_message(pointer(auth), text) },
            0
        );
        assert_eq!(unsafe { values::jett_rt_v1_list_new(pointer(auth), 0) }, 0);
        assert_eq!(current_message(auth), BOUNDS);
        assert_eq!(
            unsafe { values::jett_rt_v1_string_release(pointer(auth), text) },
            0
        );
        complete(auth, attempt, 31);
        assert_eq!(saved(auth, attempt).2, BOUNDS);
        assert_no_effects(auth);
    });
}

#[test]
fn ordinary_error_reentry_pre_entry_session_failure_is_not_issued_an_attempt() {
    context(|auth| {
        install(auth, &[], 2);
        dynamic_failure(auth, b"session before first entry");
        let message = current_message(auth);
        assert!(begin_only(auth).is_err());
        assert!(begin_only(auth).is_err());
        assert_eq!(current_message(auth), message);
        auth.with_resource(|state, _, _| {
            assert!(state.attempt.is_none() && state.completed.is_empty());
            Ok(())
        })
        .unwrap();
        assert_no_effects(auth);
    });
}

#[test]
fn ordinary_error_reentry_between_entry_error_is_session_owned_and_old_receipt_survives() {
    context(|auth| {
        install(auth, &[], 2);
        let (first, _, _, _) = start(auth, ResourcePurpose::Runtime);
        successful_leaf(auth);
        complete(auth, first, 0);
        let first_saved = saved(auth, first);
        dynamic_failure(auth, b"session between entries");
        assert_eq!(current_message(auth), b"session between entries");
        assert!(begin_only(auth).is_err());
        assert_eq!(saved(auth, first), first_saved);
        assert_eq!(current_message(auth), b"session between entries");
        assert_no_effects(auth);
    });
}

#[test]
fn ordinary_error_reentry_refused_begin_preserves_sealed_error_and_can_retry_after_owner_drop() {
    context(|auth| {
        install(auth, &[], 2);
        let (first, _, _, _) = start(auth, ResourcePurpose::Runtime);
        bounds_failure(auth, -1);
        complete(auth, first, 1);
        let first_saved = saved(auth, first);
        let text =
            unsafe { values::jett_rt_v1_string_literal(pointer(auth), b"resident".as_ptr(), 8) };
        assert_ne!(text, 0);
        assert!(begin_only(auth).is_err());
        assert_eq!(saved(auth, first), first_saved);
        assert_eq!(current_message(auth), BOUNDS);
        assert_eq!(
            unsafe { values::jett_rt_v1_string_release(pointer(auth), text) },
            0
        );
        let second = begin_only(auth).unwrap();
        successful_leaf(auth);
        assert_eq!(complete(auth, second, 0).selected_kind, 0);
        assert_eq!(saved(auth, first), first_saved);
        assert_no_effects(auth);
    });
}

#[test]
fn ordinary_error_reentry_wrong_entry_active_and_duplicate_completion_do_not_advance_history() {
    context(|auth| {
        install(auth, &[], 2);
        let (first, _, _, _) = start(auth, ResourcePurpose::Runtime);
        bounds_failure(auth, -1);
        auth.with_state(|_, state| {
            let resource = state.resource_state.as_mut().unwrap();
            assert!(
                resource
                    .begin_entry(
                        &mut state.values,
                        &state.resources,
                        entry(),
                        ResourcePurpose::Runtime
                    )
                    .is_err()
            );
            let wrong = ResourceHandleId::new(first.raw().checked_add(1).unwrap())?;
            assert!(
                resource
                    .complete_entry(&mut state.values, &mut state.resources, wrong, 0, false)
                    .is_err()
            );
            assert!(resource.completed.is_empty());
            Ok(())
        })
        .unwrap();
        assert_eq!(current_message(auth), BOUNDS);
        complete(auth, first, 1);
        let first_saved = saved(auth, first);
        auth.with_state(|_, state| {
            let resource = state.resource_state.as_mut().unwrap();
            let mut wrong_entry = entry();
            wrong_entry.signature = wrong_entry.signature.checked_add(1).unwrap();
            assert!(
                resource
                    .begin_entry(
                        &mut state.values,
                        &state.resources,
                        wrong_entry,
                        ResourcePurpose::Runtime
                    )
                    .is_err()
            );
            assert!(
                resource
                    .complete_entry(&mut state.values, &mut state.resources, first, 0, false)
                    .is_err()
            );
            assert_eq!(resource.completed.len(), 1);
            Ok(())
        })
        .unwrap();
        assert_eq!(saved(auth, first), first_saved);
        let second = begin_only(auth).unwrap();
        successful_leaf(auth);
        complete(auth, second, 0);
        assert_eq!(saved(auth, first), first_saved);
        assert_no_effects(auth);
    });
}

#[test]
fn ordinary_error_reentry_foreign_attempt_cannot_complete_or_query_this_context() {
    context(|first_auth| {
        context(|second_auth| {
            install(first_auth, &[], 1);
            install(second_auth, &[], 1);
            let (first, _, _, _) = start(first_auth, ResourcePurpose::Runtime);
            let (second, _, _, _) = start(second_auth, ResourcePurpose::Runtime);
            assert_ne!(first, second);
            bounds_failure(first_auth, -1);
            dynamic_failure(second_auth, b"second context error");
            for (auth, foreign) in [(first_auth, second), (second_auth, first)] {
                auth.with_state(|_, state| {
                    let resource = state.resource_state.as_mut().unwrap();
                    assert!(
                        resource
                            .complete_entry(
                                &mut state.values,
                                &mut state.resources,
                                foreign,
                                0,
                                false
                            )
                            .is_err()
                    );
                    assert!(resource.completed_attempt(foreign).is_err());
                    assert!(resource.completed_messages(foreign).is_err());
                    Ok(())
                })
                .unwrap();
            }
            complete(first_auth, first, 1);
            complete(second_auth, second, 1);
            assert_eq!(saved(first_auth, first).2, BOUNDS);
            assert_eq!(saved(second_auth, second).2, b"second context error");
            assert_no_effects(first_auth);
            assert_no_effects(second_auth);
        });
    });
}

#[test]
fn ordinary_error_reentry_exhausted_budget_preserves_the_last_completed_error() {
    context(|auth| {
        install(auth, &[], 1);
        let (attempt, _, _, _) = start(auth, ResourcePurpose::Runtime);
        bounds_failure(auth, -1);
        complete(auth, attempt, 1);
        let before = saved(auth, attempt);
        assert!(begin_only(auth).is_err());
        assert!(begin_only(auth).is_err());
        assert_eq!(saved(auth, attempt), before);
        assert_eq!(current_message(auth), BOUNDS);
        assert_no_effects(auth);
    });
}

#[test]
fn ordinary_error_reentry_maximum_retained_history_is_bounded_and_exact() {
    context(|auth| {
        install(auth, &[], u32::try_from(MAX_ATTEMPTS).unwrap());
        let mut history = Vec::new();
        for _ in 0..MAX_ATTEMPTS {
            let (attempt, _, _, _) = start(auth, ResourcePurpose::Runtime);
            assert_eq!(unsafe { values::jett_rt_v1_value_status(pointer(auth)) }, 0);
            bounds_failure(auth, -1);
            complete(auth, attempt, 1);
            let observation = saved(auth, attempt);
            assert_eq!(observation.2, BOUNDS);
            history.push((attempt, observation));
        }
        assert_eq!(history.len(), MAX_ATTEMPTS);
        assert!(begin_only(auth).is_err());
        assert!(begin_only(auth).is_err());
        for (attempt, observation) in history {
            assert_eq!(saved(auth, attempt), observation);
        }
        assert_eq!(current_message(auth), BOUNDS);
        assert_no_effects(auth);
    });
}

#[test]
fn ordinary_error_reentry_diagnostic_overflow_retains_tombstone_without_cleanup_retry() {
    context(|auth| {
        install(auth, &[], 2);
        let (attempt, _, _, _) = start(auth, ResourcePurpose::Runtime);
        let message = vec![b'x'; 64 * 1024 + 1];
        dynamic_failure(auth, &message);
        assert_eq!(current_message(auth), message);
        auth.with_state(|_, state| {
            let resource = state.resource_state.as_mut().unwrap();
            assert_eq!(
                resource.complete_entry(&mut state.values, &mut state.resources, attempt, 1, false),
                Err(NativeResourceError::Capacity)
            );
            assert_eq!(resource.completed.len(), 1);
            assert_eq!(
                resource.completed_attempt(attempt),
                Err(NativeResourceError::Capacity)
            );
            assert_eq!(
                resource.completed_messages(attempt),
                Err(NativeResourceError::Capacity)
            );
            assert!(
                resource
                    .complete_entry(&mut state.values, &mut state.resources, attempt, 1, false)
                    .is_err()
            );
            Ok(())
        })
        .unwrap();
        assert!(begin_only(auth).is_err());
        assert_eq!(current_message(auth), message);
        assert_no_effects(auth);
    });
}

#[test]
fn ordinary_error_reentry_real_cleanup_poison_preserves_first_error_and_blocks_constructor() {
    poisoned_context(|auth| {
        install(auth, &[], 2);
        let (attempt, _, _, _) = start(auth, ResourcePurpose::Runtime);
        let list = unsafe { values::jett_rt_v1_list_new(pointer(auth), 0) };
        assert_ne!(list, 0);
        assert_eq!(
            unsafe { values::jett_rt_v1_list_append(pointer(auth), list, 1) },
            list
        );
        assert_eq!(
            unsafe { values::jett_rt_v1_list_remove_at(pointer(auth), list, -1) },
            0
        );
        assert_eq!(
            unsafe { values::jett_rt_v1_value_drop(pointer(auth), list) },
            0
        );
        assert_ne!(
            unsafe { values::jett_rt_v1_value_drop(pointer(auth), list) },
            0
        );
        assert_eq!(current_message(auth), BOUNDS);
        let completion = complete(auth, attempt, 1);
        assert_eq!(completion.selected_kind, 3);
        assert_ne!(completion.cleanup_status, 0);
        let before = saved(auth, attempt);
        assert_eq!(before.2, BOUNDS);
        assert!(begin_only(auth).is_err());
        assert_eq!(saved(auth, attempt), before);
        assert_no_effects(auth);
    });
}

#[test]
fn ordinary_error_reentry_sealed_take_refuses_and_poison_cannot_erase_history() {
    poisoned_context(|auth| {
        install(auth, &[], 2);
        let (attempt, _, _, _) = start(auth, ResourcePurpose::Runtime);
        bounds_failure(auth, -1);
        complete(auth, attempt, 1);
        let before = saved(auth, attempt);
        assert_eq!(
            unsafe { values::jett_rt_v1_failure_take_prefixed_text(pointer(auth), ptr::null(), 0) },
            0
        );
        assert_eq!(saved(auth, attempt), before);
        // The invalid between-entry take belongs to the sticky session channel.
        // Its poison may take observation precedence; the sealed body stays exact.
        let session_message = current_message(auth);
        assert!(!session_message.is_empty());
        assert_eq!(before.2, BOUNDS);
        assert!(begin_only(auth).is_err());
        assert_eq!(saved(auth, attempt), before);
        assert_eq!(current_message(auth), session_message);
        assert_no_effects(auth);
    });
}

#[test]
fn ordinary_error_reentry_active_refinement_capture_remains_data_before_completion() {
    context(|auth| {
        install(auth, &[], 2);
        let (first, _, _, _) = start(auth, ResourcePurpose::Runtime);
        bounds_failure(auth, -1);
        let captured = unsafe {
            values::jett_rt_v1_failure_take_prefixed_text(pointer(auth), b"handled: ".as_ptr(), 9)
        };
        assert_ne!(captured, 0);
        assert_eq!(unsafe { values::jett_rt_v1_value_status(pointer(auth)) }, 0);
        let expected = b"handled: list.__remove_at: index -1 out of bounds";
        let expected_text = unsafe {
            values::jett_rt_v1_string_literal(
                pointer(auth),
                expected.as_ptr(),
                u64::try_from(expected.len()).unwrap(),
            )
        };
        assert_ne!(expected_text, 0);
        assert_eq!(
            unsafe { values::jett_rt_v1_string_equal(pointer(auth), captured, expected_text) },
            1
        );
        assert_eq!(
            unsafe { values::jett_rt_v1_string_release(pointer(auth), captured) },
            0
        );
        assert_eq!(
            unsafe { values::jett_rt_v1_string_release(pointer(auth), expected_text) },
            0
        );
        successful_leaf(auth);
        assert_eq!(complete(auth, first, 0).selected_kind, 0);
        let first_saved = saved(auth, first);
        assert!(first_saved.2.is_empty());
        let (second, _, _, _) = start(auth, ResourcePurpose::Runtime);
        bounds_failure(auth, -1);
        complete(auth, second, 1);
        assert_eq!(saved(auth, second).2, BOUNDS);
        assert_eq!(saved(auth, first), first_saved);
        assert_no_effects(auth);
    });
}

#[test]
fn ordinary_error_reentry_historical_resource_cleanup_panic_still_allows_clean_next_entry() {
    context(|auth| {
        install(auth, &[(3, 41, 0, ""), (1, 42, 0, "")], 2);
        let (first, _, frame, grant) = start(auth, ResourcePurpose::Runtime);
        construct(auth, frame, grant, 41);
        let first_completion = complete(auth, first, 0);
        assert_eq!(
            (
                first_completion.body_status,
                first_completion.cleanup_status,
                first_completion.selected_kind
            ),
            (0, JettRuntimeStatusV1::PANIC.code(), 3)
        );
        let first_saved = saved(auth, first);
        assert_eq!(first_saved.1, CLEANUP_FAILED);
        assert!(first_saved.2.is_empty());
        assert_empty(auth);

        let (second, _, frame, second_grant) = start(auth, ResourcePurpose::Runtime);
        assert_eq!(grant, second_grant);
        construct(auth, frame, second_grant, 42);
        assert_eq!(complete(auth, second, 0).selected_kind, 0);
        assert_eq!(saved(auth, first), first_saved);
        assert_eq!(events(auth), vec![(41, 1), (41, 5), (42, 1), (42, 5)]);
        assert_empty(auth);
        auth.with_resource(|state, _, _| {
            assert_eq!(state.script_remaining(), 0);
            Ok(())
        })
        .unwrap();
    });
}
