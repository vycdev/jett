//! Deterministic host allocation-refusal controls, not linked Source acceptance.
use super::*;
use crate::native_abi::resource::ordinary_reservation_faults::ReservationSite;
use std::ptr;

const BOUNDS: &[u8] = b"list.__remove_at: index -1 out of bounds";
type History = Vec<(NativeResourceCompletion, Vec<u8>, Vec<u8>)>;

fn pointer(auth: &AuthenticatedResourceContext) -> *const JettRuntimeContextV1 {
    auth.key.owner_address as *const JettRuntimeContextV1
}

fn message(auth: &AuthenticatedResourceContext) -> Vec<u8> {
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

fn prefix(auth: &AuthenticatedResourceContext, bytes: &[u8]) {
    unsafe {
        let text = values::jett_rt_v1_string_literal(
            pointer(auth),
            bytes.as_ptr(),
            u64::try_from(bytes.len()).unwrap(),
        );
        assert_ne!(text, 0);
        assert_eq!(values::jett_rt_v1_property_case_set(pointer(auth), text), 0);
        assert_eq!(values::jett_rt_v1_string_release(pointer(auth), text), 0);
    }
}

fn bounds(auth: &AuthenticatedResourceContext) {
    unsafe {
        let list = values::jett_rt_v1_list_new(pointer(auth), 0);
        assert_ne!(list, 0);
        assert_eq!(values::jett_rt_v1_list_append(pointer(auth), list, 1), list);
        assert_eq!(
            values::jett_rt_v1_list_remove_at(pointer(auth), list, -1),
            0
        );
        assert_eq!(
            values::jett_rt_v1_value_status(pointer(auth)),
            JettRuntimeStatusV1::INVALID_ARGUMENT.code()
        );
        assert_eq!(values::jett_rt_v1_value_drop(pointer(auth), list), 0);
    }
}

fn dynamic_failure(auth: &AuthenticatedResourceContext, bytes: &[u8]) {
    unsafe {
        let text = values::jett_rt_v1_string_literal(
            pointer(auth),
            bytes.as_ptr(),
            u64::try_from(bytes.len()).unwrap(),
        );
        assert_ne!(text, 0);
        assert_ne!(
            values::jett_rt_v1_assert_fail_message(pointer(auth), text),
            0
        );
        assert_eq!(values::jett_rt_v1_string_release(pointer(auth), text), 0);
    }
}

fn begin_raw(auth: &AuthenticatedResourceContext) -> ResourceResult<ResourceHandleId> {
    // Constructor preflight refusal is not an entered Source body failure.
    auth.with_state(|_, state| {
        state.resource_state.as_mut().unwrap().begin_entry(
            &mut state.values,
            &state.resources,
            entry(),
            ResourcePurpose::Runtime,
        )
    })
}

fn complete_raw(
    auth: &AuthenticatedResourceContext,
    attempt: ResourceHandleId,
    body: u32,
) -> ResourceResult<NativeResourceCompletion> {
    auth.with_state(|_, state| {
        state.resource_state.as_mut().unwrap().complete_entry(
            &mut state.values,
            &mut state.resources,
            attempt,
            body,
            false,
        )
    })
}

fn history(auth: &AuthenticatedResourceContext) -> History {
    auth.with_resource(|state, _, _| {
        Ok(state
            .completed
            .iter()
            .map(|row| {
                (
                    row.completion,
                    row.resource_message.clone(),
                    row.ordinary_message.clone(),
                )
            })
            .collect())
    })
    .unwrap()
}

fn channel_state(
    auth: &AuthenticatedResourceContext,
) -> (
    usize,
    Option<ResourceOrdinaryOwner>,
    Option<ResourceOrdinaryCompletion>,
) {
    auth.with_state(|_, state| Ok(state.values.test_resource_ordinary_channel_state()))
        .unwrap()
}

fn arm(auth: &AuthenticatedResourceContext, site: ReservationSite) {
    auth.with_state(|_, state| {
        match site {
            ReservationSite::OrdinaryChannel => {
                state.values.test_arm_resource_ordinary_reservation()
            }
            ReservationSite::BeginActiveFrames
            | ReservationSite::BeginHistory
            | ReservationSite::ResourceDiagnostic
            | ReservationSite::OrdinaryDiagnostic => state
                .resource_state
                .as_mut()
                .unwrap()
                .reservation_faults
                .arm(site),
        }
        Ok(())
    })
    .unwrap();
}

fn fault_snapshot(
    auth: &AuthenticatedResourceContext,
    site: ReservationSite,
) -> (usize, usize, bool) {
    auth.with_state(|_, state| {
        Ok(match site {
            ReservationSite::OrdinaryChannel => {
                state.values.test_resource_ordinary_reservation_snapshot()
            }
            ReservationSite::BeginActiveFrames
            | ReservationSite::BeginHistory
            | ReservationSite::ResourceDiagnostic
            | ReservationSite::OrdinaryDiagnostic => state
                .resource_state
                .as_ref()
                .unwrap()
                .reservation_faults
                .snapshot(site),
        })
    })
    .unwrap()
}

fn assert_injected(before: (usize, usize, bool), after: (usize, usize, bool)) {
    assert!(before.2);
    assert_eq!(after, (before.0 + 1, before.1 + 1, false));
}

fn expected_begin_error(site: ReservationSite) -> NativeResourceError {
    match site {
        ReservationSite::OrdinaryChannel => {
            NativeResourceError::OrdinaryStorage(JettRuntimeStatusV1::RESOURCE_EXHAUSTED)
        }
        ReservationSite::BeginActiveFrames | ReservationSite::BeginHistory => {
            NativeResourceError::Capacity
        }
        ReservationSite::ResourceDiagnostic | ReservationSite::OrdinaryDiagnostic => {
            panic!("not an entry preflight site")
        }
    }
}

fn assert_idle(auth: &AuthenticatedResourceContext, remaining: usize) {
    assert_empty(auth);
    auth.with_resource(|state, _, _| {
        assert!(state.attempt.is_none());
        assert!(state.only_installation_handles());
        assert_eq!(state.script_remaining(), remaining);
        Ok(())
    })
    .unwrap();
}

fn fill_to_growth(auth: &AuthenticatedResourceContext, site: ReservationSite) -> History {
    let (first, _, _, grant) = start(auth, ResourcePurpose::Runtime);
    bounds(auth);
    assert_eq!(complete(auth, first, 1).selected_kind, 1);
    let capacity = auth
        .with_state(|_, state| {
            Ok(match site {
                ReservationSite::BeginHistory => {
                    state.resource_state.as_ref().unwrap().completed.capacity()
                }
                ReservationSite::OrdinaryChannel => {
                    state.values.test_resource_ordinary_channel_capacity().1
                }
                ReservationSite::BeginActiveFrames
                | ReservationSite::ResourceDiagnostic
                | ReservationSite::OrdinaryDiagnostic => panic!("not a growable history site"),
            })
        })
        .unwrap();
    // Derived from actual Vec capacity, never a fixed assumed growth factor.
    assert!(capacity >= 1 && capacity < MAX_ATTEMPTS);
    for _ in 1..capacity {
        let (attempt, _, _, next_grant) = start(auth, ResourcePurpose::Runtime);
        assert_eq!(next_grant, grant);
        bounds(auth);
        assert_eq!(complete(auth, attempt, 1).selected_kind, 1);
    }
    auth.with_state(|_, state| {
        match site {
            ReservationSite::BeginHistory => {
                let history = &state.resource_state.as_ref().unwrap().completed;
                assert_eq!(history.len(), history.capacity());
            }
            ReservationSite::OrdinaryChannel => {
                let (length, capacity) = state.values.test_resource_ordinary_channel_capacity();
                assert_eq!(length, capacity);
            }
            ReservationSite::BeginActiveFrames
            | ReservationSite::ResourceDiagnostic
            | ReservationSite::OrdinaryDiagnostic => panic!("not a growable history site"),
        }
        Ok(())
    })
    .unwrap();
    assert_idle(auth, 0);
    history(auth)
}

#[test]
fn ordinary_reservation_first_begin_refusals_publish_no_owner_or_provider_effect() {
    for site in [
        ReservationSite::BeginActiveFrames,
        ReservationSite::BeginHistory,
        ReservationSite::OrdinaryChannel,
    ] {
        context(|auth| {
            install(auth, &[(1, 811, 0, "")], 1);
            arm(auth, site);
            let before = fault_snapshot(auth, site);
            assert_eq!(begin_raw(auth), Err(expected_begin_error(site)));
            assert_injected(before, fault_snapshot(auth, site));
            assert_eq!(channel_state(auth), (0, None, None));
            assert!(history(auth).is_empty());
            assert!(message(auth).is_empty());
            assert!(events(auth).is_empty());
            assert_idle(auth, 1);
            assert!(complete_raw(auth, ResourceHandleId::new(123_456).unwrap(), 0).is_err());
            assert_eq!(channel_state(auth), (0, None, None));
            let (attempt, _, frame, grant) = start(auth, ResourcePurpose::Runtime);
            construct(auth, frame, grant, 811);
            assert_eq!(complete(auth, attempt, 0).selected_kind, 0);
            assert_eq!(events(auth), vec![(811, 1), (811, 5)]);
            assert_idle(auth, 0);
        });
    }
}

#[test]
fn ordinary_reservation_next_history_or_channel_growth_preserves_every_sealed_error() {
    for site in [
        ReservationSite::BeginHistory,
        ReservationSite::OrdinaryChannel,
    ] {
        context(|auth| {
            install(auth, &[], u32::try_from(MAX_ATTEMPTS).unwrap());
            let saved = fill_to_growth(auth, site);
            let saved_channel = channel_state(auth);
            let saved_message = message(auth);
            assert_eq!(saved_message, BOUNDS);
            arm(auth, site);
            let before = fault_snapshot(auth, site);
            assert_eq!(begin_raw(auth), Err(expected_begin_error(site)));
            assert_injected(before, fault_snapshot(auth, site));
            assert_eq!(history(auth), saved);
            assert_eq!(channel_state(auth), saved_channel);
            assert_eq!(message(auth), saved_message);
            assert_idle(auth, 0);
            let (next, _, _, _) = start(auth, ResourcePurpose::Runtime);
            assert!(message(auth).is_empty());
            assert_eq!(complete(auth, next, 0).selected_kind, 0);
            let current = history(auth);
            assert_eq!(&current[..saved.len()], &saved);
            assert_eq!(current.len(), saved.len() + 1);
            assert!(events(auth).is_empty());
            assert_idle(auth, 0);
        });
    }
}

#[test]
fn ordinary_reservation_invalid_predecessor_never_reaches_or_consumes_channel_allocation() {
    context(|auth| {
        install(auth, &[], u32::try_from(MAX_ATTEMPTS).unwrap());
        let saved = fill_to_growth(auth, ReservationSite::OrdinaryChannel);
        let saved_channel = channel_state(auth);
        let saved_message = message(auth);
        arm(auth, ReservationSite::OrdinaryChannel);
        let before = fault_snapshot(auth, ReservationSite::OrdinaryChannel);
        auth.with_state(|_, state| {
            let previous = state
                .resource_state
                .as_ref()
                .unwrap()
                .completed
                .last()
                .unwrap()
                .ordinary;
            let mut forged = previous.owner;
            forged.attempt = ResourceHandleId::new(forged.attempt.raw() + 10).unwrap();
            forged.root = ResourceHandleId::new(forged.root.raw() + 10).unwrap();
            forged.ordinal += 1;
            forged.context.token = forged.context.token.wrapping_add(1);
            assert!(
                state
                    .values
                    .prepare_resource_ordinary_attempt(forged, Some(previous))
                    .is_err()
            );
            let mut wrong_saved = previous;
            wrong_saved.completion.body_status += 1;
            let mut next = previous.owner;
            next.attempt = ResourceHandleId::new(next.attempt.raw() + 10).unwrap();
            next.root = ResourceHandleId::new(next.root.raw() + 10).unwrap();
            next.ordinal += 1;
            assert!(
                state
                    .values
                    .prepare_resource_ordinary_attempt(next, Some(wrong_saved))
                    .is_err()
            );
            Ok(())
        })
        .unwrap();
        assert_eq!(
            fault_snapshot(auth, ReservationSite::OrdinaryChannel),
            before
        );
        assert_eq!(channel_state(auth), saved_channel);
        assert_eq!(history(auth), saved);
        assert_eq!(message(auth), saved_message);
        assert_eq!(
            begin_raw(auth),
            Err(expected_begin_error(ReservationSite::OrdinaryChannel))
        );
        assert_injected(
            before,
            fault_snapshot(auth, ReservationSite::OrdinaryChannel),
        );
        assert_eq!(channel_state(auth), saved_channel);
        assert_eq!(history(auth), saved);
        assert_idle(auth, 0);
    });
}

#[test]
fn ordinary_reservation_wrong_or_active_entry_cannot_consume_prepublication_fault() {
    context(|auth| {
        install(auth, &[], 1);
        arm(auth, ReservationSite::BeginActiveFrames);
        let before = fault_snapshot(auth, ReservationSite::BeginActiveFrames);
        auth.with_state(|_, state| {
            let mut wrong = entry();
            wrong.signature += 1;
            assert_eq!(
                state.resource_state.as_mut().unwrap().begin_entry(
                    &mut state.values,
                    &state.resources,
                    wrong,
                    ResourcePurpose::Runtime,
                ),
                Err(NativeResourceError::ActiveAttempt)
            );
            Ok(())
        })
        .unwrap();
        assert_eq!(
            fault_snapshot(auth, ReservationSite::BeginActiveFrames),
            before
        );
        assert_eq!(begin_raw(auth), Err(NativeResourceError::Capacity));
        assert_injected(
            before,
            fault_snapshot(auth, ReservationSite::BeginActiveFrames),
        );
        let (attempt, _, _, _) = start(auth, ResourcePurpose::Runtime);
        arm(auth, ReservationSite::BeginHistory);
        let active_before = fault_snapshot(auth, ReservationSite::BeginHistory);
        let channel = channel_state(auth);
        assert_eq!(begin_raw(auth), Err(NativeResourceError::ActiveAttempt));
        assert_eq!(channel_state(auth), channel);
        assert_eq!(
            fault_snapshot(auth, ReservationSite::BeginHistory),
            active_before
        );
        assert_eq!(complete(auth, attempt, 0).selected_kind, 0);
        assert_eq!(
            fault_snapshot(auth, ReservationSite::BeginHistory),
            active_before
        );
        assert_idle(auth, 0);
    });
}

#[test]
fn ordinary_reservation_sealing_uses_preflight_storage_and_keeps_pending_growth_faults() {
    context(|auth| {
        install(auth, &[(1, 821, 0, "")], 1);
        let (attempt, _, frame, grant) = start(auth, ResourcePurpose::Runtime);
        construct(auth, frame, grant, 821);
        prefix(auth, b"case 17: ");
        bounds(auth);
        let original_message = message(auth);
        arm(auth, ReservationSite::BeginHistory);
        arm(auth, ReservationSite::OrdinaryChannel);
        let history_fault = fault_snapshot(auth, ReservationSite::BeginHistory);
        let channel_fault = fault_snapshot(auth, ReservationSite::OrdinaryChannel);
        let completion = complete(auth, attempt, 1);
        assert_eq!(
            (
                completion.body_status,
                completion.cleanup_status,
                completion.selected_kind
            ),
            (1, 0, 1)
        );
        assert_eq!(
            fault_snapshot(auth, ReservationSite::BeginHistory),
            history_fault
        );
        assert_eq!(
            fault_snapshot(auth, ReservationSite::OrdinaryChannel),
            channel_fault
        );
        assert_eq!(channel_state(auth).2.unwrap().completion, completion);
        assert_eq!(history(auth)[0].2, original_message);
        assert_eq!(message(auth), original_message);
        assert_eq!(events(auth), vec![(821, 1), (821, 5)]);
        let saved = history(auth);
        assert!(complete_raw(auth, attempt, 0).is_err());
        assert_eq!(history(auth), saved);
        assert_eq!(
            fault_snapshot(auth, ReservationSite::BeginHistory),
            history_fault
        );
        assert_eq!(
            fault_snapshot(auth, ReservationSite::OrdinaryChannel),
            channel_fault
        );
        assert_idle(auth, 0);
    });
}

fn assert_diagnostic_tombstone(
    auth: &AuthenticatedResourceContext,
    attempt: ResourceHandleId,
    cleanup: u32,
    selected_kind: u32,
    original_message: &[u8],
) {
    assert_eq!(message(auth), original_message);
    let selected = channel_state(auth).2.unwrap();
    assert!(!selected.diagnostic_ready());
    assert_eq!(
        (
            selected.completion.body_status,
            selected.completion.cleanup_status,
            selected.completion.selected_kind
        ),
        (1, cleanup, selected_kind)
    );
    auth.with_resource(|state, ordinary, _| {
        let row = state.completed.last().unwrap();
        assert_eq!(row.completion.attempt, attempt.raw());
        assert_eq!(row.ordinary, selected);
        assert_eq!(row.report_error, Some(NativeResourceError::Capacity));
        assert!(row.resource_message.is_empty() && row.ordinary_message.is_empty());
        assert_eq!(
            state.completed_attempt(attempt),
            Err(NativeResourceError::Capacity)
        );
        assert_eq!(
            state.completed_messages(attempt),
            Err(NativeResourceError::Capacity)
        );
        assert!(!ordinary.cleanup_failed);
        Ok(())
    })
    .unwrap();
    let saved = history(auth);
    let before_events = events(auth);
    assert!(complete_raw(auth, attempt, 0).is_err());
    assert!(begin_raw(auth).is_err());
    assert_eq!(history(auth), saved);
    assert_eq!(events(auth), before_events);
    assert_eq!(channel_state(auth).2, Some(selected));
    assert_eq!(message(auth), original_message);
    assert_idle(auth, 0);
}

#[test]
fn ordinary_reservation_ordinary_diagnostic_failure_seals_after_real_resource_retirement() {
    context(|auth| {
        install(auth, &[(1, 831, 0, "")], 3);
        let (first, _, _, first_grant) = start(auth, ResourcePurpose::Runtime);
        prefix(auth, b"earlier case: ");
        bounds(auth);
        assert_eq!(complete(auth, first, 1).selected_kind, 1);
        let earlier_history = history(auth);
        let earlier_proof = channel_state(auth).2.unwrap();
        assert_idle(auth, 1);
        let (attempt, _, frame, grant) = start(auth, ResourcePurpose::Runtime);
        assert_eq!(first_grant, grant);
        construct(auth, frame, grant, 831);
        prefix(auth, b"case 23: ");
        bounds(auth);
        let original_message = message(auth);
        let expected = [b"case 23: ".as_slice(), BOUNDS].concat();
        assert_eq!(original_message, expected);
        arm(auth, ReservationSite::OrdinaryDiagnostic);
        let before = fault_snapshot(auth, ReservationSite::OrdinaryDiagnostic);
        assert_eq!(
            complete_raw(auth, attempt, 1),
            Err(NativeResourceError::Capacity)
        );
        assert_injected(
            before,
            fault_snapshot(auth, ReservationSite::OrdinaryDiagnostic),
        );
        assert_eq!(events(auth), vec![(831, 1), (831, 5)]);
        assert_diagnostic_tombstone(auth, attempt, 0, 1, &original_message);
        assert_eq!(&history(auth)[..earlier_history.len()], &earlier_history);
        auth.with_resource(|state, _, _| {
            assert_eq!(state.completed[0].ordinary, earlier_proof);
            Ok(())
        })
        .unwrap();
    });
}

#[test]
fn ordinary_reservation_either_diagnostic_failure_keeps_cleanup_selection_and_first_bounds_error() {
    for site in [
        ReservationSite::ResourceDiagnostic,
        ReservationSite::OrdinaryDiagnostic,
    ] {
        context(|auth| {
            install(auth, &[(3, 841, 0, "")], 2);
            let (attempt, _, frame, grant) = start(auth, ResourcePurpose::Runtime);
            construct(auth, frame, grant, 841);
            bounds(auth);
            assert_eq!(message(auth), BOUNDS);
            arm(auth, site);
            let before = fault_snapshot(auth, site);
            assert_eq!(
                complete_raw(auth, attempt, 1),
                Err(NativeResourceError::Capacity)
            );
            assert_injected(before, fault_snapshot(auth, site));
            assert_eq!(events(auth), vec![(841, 1), (841, 5)]);
            assert_diagnostic_tombstone(
                auth,
                attempt,
                JettRuntimeStatusV1::PANIC.code(),
                3,
                BOUNDS,
            );
        });
    }
}

#[test]
fn ordinary_reservation_empty_diagnostics_do_not_request_or_consume_allocator_fault() {
    context(|auth| {
        install(auth, &[], 1);
        let (attempt, _, _, _) = start(auth, ResourcePurpose::Runtime);
        arm(auth, ReservationSite::ResourceDiagnostic);
        let before = fault_snapshot(auth, ReservationSite::ResourceDiagnostic);
        let completion = complete(auth, attempt, 0);
        assert_eq!(completion.selected_kind, 0);
        assert_eq!(
            fault_snapshot(auth, ReservationSite::ResourceDiagnostic),
            before
        );
        assert!(channel_state(auth).2.unwrap().diagnostic_ready());
        let saved = history(auth);
        assert!(saved[0].1.is_empty() && saved[0].2.is_empty());
        assert!(message(auth).is_empty());
        assert!(events(auth).is_empty());
        assert_idle(auth, 0);
    });
}

#[test]
fn ordinary_reservation_length_refusal_precedes_and_preserves_pending_allocator_fault() {
    context(|auth| {
        install(auth, &[], 2);
        let (attempt, _, _, _) = start(auth, ResourcePurpose::Runtime);
        let original = vec![b'x'; 64 * 1024 + 1];
        dynamic_failure(auth, &original);
        arm(auth, ReservationSite::OrdinaryDiagnostic);
        let before = fault_snapshot(auth, ReservationSite::OrdinaryDiagnostic);
        assert_eq!(
            complete_raw(auth, attempt, 1),
            Err(NativeResourceError::Capacity)
        );
        assert_eq!(
            fault_snapshot(auth, ReservationSite::OrdinaryDiagnostic),
            before
        );
        assert_diagnostic_tombstone(auth, attempt, 0, 1, &original);
        assert_eq!(
            fault_snapshot(auth, ReservationSite::OrdinaryDiagnostic),
            before
        );
        assert!(events(auth).is_empty());
    });
}

#[test]
fn ordinary_reservation_faults_are_owned_by_one_context_and_never_poison_another() {
    context(|first| {
        context(|second| {
            install(first, &[], 1);
            install(second, &[], 1);
            arm(first, ReservationSite::OrdinaryChannel);
            let first_before = fault_snapshot(first, ReservationSite::OrdinaryChannel);
            let (second_attempt, _, _, _) = start(second, ResourcePurpose::Runtime);
            assert_eq!(complete(second, second_attempt, 0).selected_kind, 0);
            assert_eq!(
                fault_snapshot(first, ReservationSite::OrdinaryChannel),
                first_before
            );
            assert_eq!(
                fault_snapshot(second, ReservationSite::OrdinaryChannel).1,
                0
            );
            assert_eq!(
                begin_raw(first),
                Err(expected_begin_error(ReservationSite::OrdinaryChannel))
            );
            assert_injected(
                first_before,
                fault_snapshot(first, ReservationSite::OrdinaryChannel),
            );
            assert_eq!(channel_state(first), (0, None, None));
            let (first_attempt, _, _, _) = start(first, ResourcePurpose::Runtime);
            assert_eq!(complete(first, first_attempt, 0).selected_kind, 0);
            assert!(message(first).is_empty() && message(second).is_empty());
            assert!(events(first).is_empty() && events(second).is_empty());
            assert_idle(first, 0);
            assert_idle(second, 0);
        });
    });
}

#[test]
fn ordinary_reservation_custody_failure_after_preparation_publishes_no_channel_or_frame() {
    use crate::resource_custody::FrameReservationSite;
    for site in [
        FrameReservationSite::Frames,
        FrameReservationSite::ActiveFrames,
    ] {
        context(|auth| {
            install(auth, &[], 1);
            let before = auth
                .with_state(|_, state| {
                    let custody = &mut state.resource_state.as_mut().unwrap().custody;
                    custody.test_arm_frame_reservation(site);
                    Ok(custody.test_frame_reservation_snapshot(site))
                })
                .unwrap();
            assert_eq!(
                begin_raw(auth),
                Err(NativeResourceError::Custody(
                    CustodyError::CapacityExhausted
                ))
            );
            let after = auth
                .with_state(|_, state| {
                    Ok(state
                        .resource_state
                        .as_ref()
                        .unwrap()
                        .custody
                        .test_frame_reservation_snapshot(site))
                })
                .unwrap();
            assert_injected(before, after);
            // The ordinary Vec reservation succeeded before the real core failure,
            // but dropping the prepared borrow did not publish its owner or record.
            assert_eq!(
                fault_snapshot(auth, ReservationSite::OrdinaryChannel),
                (1, 0, false)
            );
            assert_eq!(channel_state(auth), (0, None, None));
            assert!(history(auth).is_empty());
            assert!(message(auth).is_empty());
            assert!(events(auth).is_empty());
            assert_idle(auth, 0);
            let (attempt, _, _, _) = start(auth, ResourcePurpose::Runtime);
            assert_eq!(
                fault_snapshot(auth, ReservationSite::OrdinaryChannel),
                (1, 0, false)
            );
            assert_eq!(channel_state(auth).0, 1);
            assert_eq!(complete(auth, attempt, 0).selected_kind, 0);
            assert!(channel_state(auth).2.unwrap().diagnostic_ready());
            assert_idle(auth, 0);
        });
    }
}

#[test]
fn ordinary_reservation_earlier_history_growth_does_not_make_failed_diagnostics_reenterable() {
    context(|auth| {
        install(auth, &[], u32::try_from(MAX_ATTEMPTS).unwrap());
        let (first, _, _, _) = start(auth, ResourcePurpose::Runtime);
        let capacity = auth
            .with_resource(|state, _, _| Ok(state.completed.capacity()))
            .unwrap();
        assert!(capacity >= 1 && capacity < MAX_ATTEMPTS);
        let mut prepared_first = Some(first);
        let mut tombstone = None;
        for ordinal in 0..capacity {
            let attempt = prepared_first
                .take()
                .unwrap_or_else(|| start(auth, ResourcePurpose::Runtime).0);
            bounds(auth);
            if ordinal + 1 == capacity {
                arm(auth, ReservationSite::OrdinaryDiagnostic);
                assert_eq!(
                    complete_raw(auth, attempt, 1),
                    Err(NativeResourceError::Capacity)
                );
                tombstone = Some(attempt);
            } else {
                assert_eq!(complete(auth, attempt, 1).selected_kind, 1);
            }
        }
        let tombstone = tombstone.unwrap();
        let saved = history(auth);
        let saved_channel = channel_state(auth);
        let original_message = message(auth);
        assert_eq!(original_message, BOUNDS);
        assert!(!saved_channel.2.unwrap().diagnostic_ready());
        auth.with_resource(|state, _, _| {
            assert_eq!(state.completed.len(), state.completed.capacity());
            assert_eq!(
                state.completed_attempt(tombstone),
                Err(NativeResourceError::Capacity)
            );
            Ok(())
        })
        .unwrap();
        arm(auth, ReservationSite::BeginHistory);
        let before = fault_snapshot(auth, ReservationSite::BeginHistory);
        // Preserve the real constructor order: Resource history reserves earlier
        // than exact ordinary predecessor validation. Both refusals are inert.
        assert_eq!(begin_raw(auth), Err(NativeResourceError::Capacity));
        assert_injected(before, fault_snapshot(auth, ReservationSite::BeginHistory));
        assert_eq!(history(auth), saved);
        assert_eq!(channel_state(auth), saved_channel);
        assert_eq!(message(auth), original_message);
        assert_idle(auth, 0);
        assert_eq!(
            begin_raw(auth),
            Err(NativeResourceError::OrdinaryStorage(
                JettRuntimeStatusV1::INVALID_ARGUMENT
            ))
        );
        assert_eq!(history(auth), saved);
        assert_eq!(channel_state(auth), saved_channel);
        assert_eq!(message(auth), original_message);
        assert!(events(auth).is_empty());
        assert_idle(auth, 0);
    });
}
