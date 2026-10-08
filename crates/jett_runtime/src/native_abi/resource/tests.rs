//! Host protocol controls, not Source admission or linked native acceptance.
use super::*;
use std::mem::{align_of, offset_of, size_of};

fn words(bytes: &mut Vec<u8>, values: &[u32]) {
    for value in values {
        bytes.extend_from_slice(&value.to_le_bytes());
    }
}
fn layout() -> Vec<u8> {
    layout_with_entry_network(true)
}
fn layout_with_entry_network(network: bool) -> Vec<u8> {
    // Independent canonical wire data is checked by the sole real registration issuer.
    let mut bytes = b"JTRSC001".to_vec();
    words(&mut bytes, &[1, 0]);
    bytes.extend_from_slice(&0u64.to_le_bytes());
    words(&mut bytes, &[1, 3, 4, 9, 2, 4, 17, 0]);
    words(&mut bytes, &[0]);
    for row in [[0, 0, 1, 0], [1, 0, 2, 1], [2, 0, 3, 2]] {
        words(&mut bytes, &row);
    }
    // Factory, Borrow, Finalize and the containing entry-function signature.
    words(&mut bytes, &[0, 2, 6, 0, 2, 1, 1]);
    words(&mut bytes, &[1, 2, 7, 0, 2, 4, 2]);
    words(&mut bytes, &[2, 1, 3, 4, 1]);
    if network {
        words(&mut bytes, &[3, 1, 3, 0, 2]);
    } else {
        words(&mut bytes, &[3, 0, 3]);
    }
    for row in [
        vec![0, 6],
        vec![1, 1, 64, 1],
        vec![2, 4],
        vec![3, 5],
        vec![4, 7, 0],
        vec![5, 8, 4],
        vec![6, 9, 4, 2],
        vec![7, 9, 1, 2],
        vec![8, 10, 0],
    ] {
        words(&mut bytes, &row);
    }
    // Exact containing-function signature for both Scope and Operation.
    words(&mut bytes, &[0, 1, 0, 0, 0, 0, 3, 1, 0, 0]);
    words(&mut bytes, &[1, 2, 0, 0, 0, 1, 3, 1, 1, 0]);
    for row in [
        vec![0, 0, 4, 0],
        vec![1, 1, 4, 0],
        vec![2, 1, 6, 1, 2],
        vec![3, 0, 5, 1, 1],
    ] {
        words(&mut bytes, &row);
    }
    let operations: [&[u32]; 17] = [
        &[1, 1, 0, 2],
        &[10, 1, 2, 1],
        &[2, 1, 1, 0],
        &[3, 1, 0, 1],
        &[6, 1, 1, 1, 3],
        &[5, 1, 3],
        &[7, 1, 2, 0],
        &[8, 1, 1],
        &[9, 1, 1, 2],
        &[11, 1, 2],
        &[12, 1, 0, 1],
        &[13, 0],
        &[14, 0],
        &[15, 0, 0, 0],
        &[9, 1, 1, 3],
        &[10, 1, 3, 1],
        &[2, 1, 0, 1],
    ];
    for (ordinal, operation) in operations.into_iter().enumerate() {
        words(&mut bytes, &[ordinal as u32, 0, 0, 0, 100 + ordinal as u32]);
        words(&mut bytes, operation);
    }
    let length = bytes.len() as u64;
    bytes[16..24].copy_from_slice(&length.to_le_bytes());
    bytes
}
fn script(rows: &[(u32, i64, i64, &str)], attempts: u32) -> DecodedScript {
    let mut bytes = b"JTRST001".to_vec();
    words(&mut bytes, &[1, 0]);
    bytes.extend_from_slice(&0u64.to_le_bytes());
    words(&mut bytes, &[attempts, rows.len() as u32]);
    for &(tag, label, result, error) in rows {
        words(&mut bytes, &[tag]);
        bytes.extend_from_slice(&label.to_le_bytes());
        match tag {
            2 | 5 => {
                words(&mut bytes, &[error.len() as u32]);
                bytes.extend_from_slice(error.as_bytes());
            }
            4 => bytes.extend_from_slice(&result.to_le_bytes()),
            _ => {}
        }
    }
    let length = bytes.len() as u64;
    bytes[16..24].copy_from_slice(&length.to_le_bytes());
    DecodedScript::decode(&bytes).unwrap()
}
fn entry() -> NativeEntry {
    NativeEntry {
        function: 0,
        signature: 3,
        scope: 0,
    }
}
pub(super) fn context<T>(operation: impl FnOnce(&AuthenticatedResourceContext) -> T) -> T {
    let mut context = Box::new(JettRuntimeContextV1::retired());
    let mut result = JettRuntimeResultV1::ok();
    let status = unsafe {
        jett_rt_v1_context_create(JETT_RUNTIME_ABI_VERSION_V1, &mut *context, &mut result)
    };
    assert_eq!(status, JettRuntimeStatusV1::OK);
    let authenticated = AuthenticatedResourceContext::acquire(&*context).unwrap();
    let result = operation(&authenticated);
    drop(authenticated); // release the actual lease before retirement waits for it
    let status =
        unsafe { jett_rt_v1_context_destroy(&mut *context, &mut JettRuntimeResultV1::ok()) };
    assert_eq!(status, JettRuntimeStatusV1::OK);
    result
}
fn install(
    authenticated: &AuthenticatedResourceContext,
    rows: &[(u32, i64, i64, &str)],
    attempts: u32,
) {
    install_scripted(authenticated, &layout(), entry(), script(rows, attempts)).unwrap();
}
fn start(
    authenticated: &AuthenticatedResourceContext,
    purpose: ResourcePurpose,
) -> (ResourceHandleId, ResourceHandleId, ResourceHandleId, u64) {
    authenticated
        .with_resource(|state, ordinary, registry| {
            let attempt = state.begin_entry(ordinary, registry, entry(), purpose)?;
            let root = state.root_frame(attempt)?;
            let network = if purpose == ResourcePurpose::Runtime {
                state.entry_network(attempt, 0)?
            } else {
                0
            };
            let operation = state.begin_operation_frame(ordinary, 1, root)?;
            Ok((attempt, root, operation, network))
        })
        .unwrap()
}
fn construct(
    authenticated: &AuthenticatedResourceContext,
    frame: ResourceHandleId,
    network: u64,
    label: i64,
) -> ResourceHandleId {
    authenticated
        .with_resource(|state, ordinary, registry| {
            let call = state.prepare_hook(ordinary, 0, frame, None)?;
            let prepared = state.prepare_factory(ordinary, registry, call, network)?;
            let result = state.factory(ordinary, registry, prepared, network, label)?;
            assert_eq!(result.domain, RESOURCE_DOMAIN_OK);
            let sum = ResourceHandleId::new(result.value)?;
            state.sum_take(ordinary, registry, 1, frame, sum, frame)
        })
        .unwrap()
}
fn complete(
    authenticated: &AuthenticatedResourceContext,
    attempt: ResourceHandleId,
    body: u32,
) -> NativeResourceCompletion {
    authenticated
        .with_resource(|state, ordinary, registry| {
            state.complete_entry(ordinary, registry, attempt, body, false)
        })
        .unwrap()
}
fn events(authenticated: &AuthenticatedResourceContext) -> Vec<(i64, u32)> {
    authenticated
        .with_resource(|state, _, _| {
            Ok(state.provider.observe_events(|events| {
                events
                    .iter()
                    .map(|event| (event.label, event.kind))
                    .collect()
            }))
        })
        .unwrap()
}
fn assert_empty(authenticated: &AuthenticatedResourceContext) {
    authenticated
        .with_resource(|state, ordinary, registry| {
            assert_eq!(
                state.counts(ordinary, registry),
                NativeResourceCounts {
                    ordinary_empty: 1,
                    ..NativeResourceCounts::default()
                }
            );
            Ok(())
        })
        .unwrap();
}

#[test]
fn native_resource_state_production_default_and_pure_descriptor_installation_have_no_provider_grant()
 {
    context(|auth| {
        auth.with_state(|_, state| {
            assert!(state.resource_state.is_none());
            Ok(())
        })
        .unwrap();
        install_disabled(auth, &layout_with_entry_network(false), entry()).unwrap();
        auth.with_resource(|state, ordinary, registry| {
            let attempt =
                state.begin_entry(ordinary, registry, entry(), ResourcePurpose::Runtime)?;
            let root = state.root_frame(attempt)?;
            let frame = state.begin_operation_frame(ordinary, 1, root)?;
            let descriptor = state.descriptor(ordinary, 12)?;
            let call = state.prepare_hook(ordinary, 13, frame, Some(descriptor))?;
            assert_eq!(
                state.prepare_factory(ordinary, registry, call, 0),
                Err(NativeResourceError::WrongGrant)
            );
            assert!(state.network.is_none());
            assert!(!state.provider.enabled());
            state.complete_entry(ordinary, registry, attempt, 0, false)?;
            Ok(())
        })
        .unwrap();
        assert_empty(auth);
    });
}

#[test]
fn native_resource_state_real_core_construct_borrow_move_close_and_reentry() {
    context(|auth| {
        install(
            auth,
            &[(1, 501, 0, ""), (4, 501, 7, ""), (1, 502, 0, "")],
            2,
        );
        let (attempt, root, frame, network) = start(auth, ResourcePurpose::Runtime);
        let owner = construct(auth, frame, network, 501);
        let owner = auth
            .with_resource(|state, ordinary, registry| {
                state.transfer(ordinary, registry, 2, frame, owner, root)
            })
            .unwrap();
        let loan = auth
            .with_resource(|state, ordinary, registry| {
                state.borrow_begin(ordinary, registry, 3, frame, owner)
            })
            .unwrap();
        let result = auth
            .with_resource(|state, ordinary, registry| {
                let call = state.prepare_hook(ordinary, 4, frame, None)?;
                state.invoke_borrow(ordinary, registry, call, network, loan)
            })
            .unwrap();
        auth.with_resource(|state, ordinary, registry| {
            // A wrong family is rejected without borrowing or provider consumption.
            assert_eq!(
                state.transfer(ordinary, registry, 2, frame, loan, root),
                Err(NativeResourceError::WrongFamily)
            );
            ordinary
                .drop_resource_ordinary_companion(result)
                .map_err(ordinary_error)?;
            state.borrow_end(5, frame, loan)?;
            state.close_or_drop(registry, 6, frame, owner)?;
            state.end_operation_frame(ordinary, registry, frame)
        })
        .unwrap();
        assert_eq!(complete(auth, attempt, 0).selected_kind, 0);
        assert_empty(auth);
        let (attempt, _, frame, network) = start(auth, ResourcePurpose::Runtime);
        let _owner = construct(auth, frame, network, 502);
        assert_eq!(complete(auth, attempt, 0).selected_kind, 0);
        assert_empty(auth);
        assert_eq!(
            events(auth),
            vec![(501, 1), (501, 3), (501, 5), (502, 1), (502, 5)]
        );
    });
}

#[test]
fn native_resource_state_installation_and_network_pair_are_atomic_and_context_exact() {
    context(|auth| {
        assert_ne!(
            unsafe {
                values::jett_rt_v1_grant_network(
                    auth.key.owner_address as *const JettRuntimeContextV1,
                )
            },
            0
        );
        assert_eq!(
            install_scripted(auth, &layout(), entry(), script(&[], 1)),
            Err(NativeResourceError::OrdinaryStorage(
                JettRuntimeStatusV1::INVALID_ARGUMENT
            ))
        );
        auth.with_state(|_, state| {
            assert!(state.resource_state.is_none());
            assert_eq!(state.resources.live_count(), 0);
            Ok(())
        })
        .unwrap();
    });
    context(|first| {
        context(|second| {
            install(first, &[(1, 1, 0, "")], 1);
            install(second, &[], 1);
            let (_, _, frame, network) = start(first, ResourcePurpose::Runtime);
            first
                .with_resource(|state, ordinary, registry| {
                    let call = state.prepare_hook(ordinary, 0, frame, None)?;
                    assert_eq!(
                        state.prepare_factory(ordinary, registry, call, network + 1),
                        Err(NativeResourceError::WrongGrant)
                    );
                    Ok(())
                })
                .unwrap();
            let foreign = first
                .with_resource(|state, _, _| Ok(state.program_handle))
                .unwrap();
            second
                .with_resource(|state, _, _| {
                    assert!(!state.handles.contains_key(&foreign));
                    Ok(())
                })
                .unwrap();
            assert!(events(first).is_empty());
            // Destruction safety cleanup is not substituted for the explicit completion gate.
            let attempt = first
                .with_resource(|state, _, _| Ok(state.attempt.as_ref().unwrap().id))
                .unwrap();
            complete(first, attempt, 0);
            assert_empty(first);
        })
    });
}

#[test]
fn native_resource_state_factory_domain_error_is_data_and_ordinary_companion_is_retired() {
    context(|auth| {
        install(auth, &[(2, 4, 0, "domain failure")], 1);
        let (attempt, _, frame, network) = start(auth, ResourcePurpose::Runtime);
        auth.with_resource(|state, ordinary, registry| {
            let call = state.prepare_hook(ordinary, 0, frame, None)?;
            let prepared = state.prepare_factory(ordinary, registry, call, network)?;
            let result = state.factory(ordinary, registry, prepared, network, 4)?;
            assert_eq!(result.domain, RESOURCE_DOMAIN_ERROR);
            let handle = ResourceHandleId::new(result.value)?;
            assert_eq!(state.sum_tag(handle)?, 0);
            assert_eq!(state.counts(ordinary, registry).owners, 0);
            state.sum_drop(ordinary, registry, 9, frame, handle)
        })
        .unwrap();
        assert_eq!(complete(auth, attempt, 0).selected_kind, 0);
        assert_empty(auth);
        assert_eq!(events(auth), vec![(4, 2)]);
    });
}

#[test]
fn native_resource_state_active_loan_refuses_move_and_preserves_one_owner() {
    context(|auth| {
        install(auth, &[(1, 6, 0, "")], 1);
        let (attempt, root, frame, network) = start(auth, ResourcePurpose::Runtime);
        let owner = construct(auth, frame, network, 6);
        auth.with_resource(|state, ordinary, registry| {
            // Move into the caller scope before borrowing.
            let owner = state.transfer(ordinary, registry, 2, frame, owner, root)?;
            let loan = state.borrow_begin(ordinary, registry, 3, frame, owner)?;
            assert_eq!(
                state.transfer(ordinary, registry, 16, frame, owner, frame),
                Err(NativeResourceError::Custody(CustodyError::ActiveBorrow))
            );
            assert_eq!(
                state.close_or_drop(registry, 6, frame, owner),
                Err(NativeResourceError::Cleanup(
                    ResourceCleanupFailure::Custody(CustodyError::ActiveBorrow)
                ))
            );
            assert_eq!(state.counts(ordinary, registry).owners, 1);
            state.borrow_end(5, frame, loan)?;
            state.close_or_drop(registry, 6, frame, owner)
        })
        .unwrap();
        // The attempted cleanup failure remains latched independently of token retirement.
        assert_eq!(complete(auth, attempt, 0).selected_kind, 3);
        assert_empty(auth);
        assert_eq!(events(auth), vec![(6, 1), (6, 5)]);
    });
}

#[test]
fn native_resource_state_borrow_panic_records_before_pop_and_cleanup_wins() {
    context(|auth| {
        install(auth, &[(3, 7, 0, ""), (6, 7, 0, "")], 1);
        let (attempt, root, frame, network) = start(auth, ResourcePurpose::Runtime);
        let owner = construct(auth, frame, network, 7);
        let owner = auth
            .with_resource(|state, ordinary, registry| {
                state.transfer(ordinary, registry, 2, frame, owner, root)
            })
            .unwrap();
        let loan = auth
            .with_resource(|state, ordinary, registry| {
                state.borrow_begin(ordinary, registry, 3, frame, owner)
            })
            .unwrap();
        let result = auth.with_resource(|state, ordinary, registry| {
            let call = state.prepare_hook(ordinary, 4, frame, None)?;
            state.invoke_borrow(ordinary, registry, call, network, loan)
        });
        assert_eq!(result, Err(NativeResourceError::BodyFailed));
        let completion = complete(auth, attempt, 0);
        assert_eq!(completion.body_status, 0);
        assert_eq!(completion.cleanup_status, JettRuntimeStatusV1::PANIC.code());
        assert_eq!(completion.selected_kind, 3);
        assert_empty(auth);
        assert_eq!(events(auth), vec![(7, 1), (7, 3), (7, 5)]);
        auth.with_resource(|state, _, _| {
            assert_eq!(state.provider.remaining(), 0);
            Ok(())
        })
        .unwrap();
    });
}

#[test]
fn native_resource_state_required_purpose_rejects_hook_without_provider_effect() {
    for purpose in [
        ResourcePurpose::NamespaceConstant,
        ResourcePurpose::ExplicitComptime,
        ResourcePurpose::Verify,
        ResourcePurpose::Property,
    ] {
        context(|auth| {
            install(auth, &[(1, 9, 0, "")], 1);
            let (attempt, _, frame, _) = start(auth, purpose);
            auth.with_resource(|state, ordinary, _| {
                assert_eq!(
                    state.entry_network(attempt, 0),
                    Err(NativeResourceError::WrongPurpose)
                );
                let descriptor = state.descriptor(ordinary, 12)?;
                assert!(matches!(
                    state.handles.get(&descriptor),
                    Some(NativeResourceEntry::Descriptor { .. })
                ));
                assert_eq!(
                    state.prepare_hook(ordinary, 0, frame, None),
                    Err(NativeResourceError::WrongPurpose)
                );
                Ok(())
            })
            .unwrap();
            complete(auth, attempt, 0);
            assert_empty(auth);
            assert!(events(auth).is_empty());
        });
    }
}

#[test]
fn native_resource_state_sum_moves_do_not_clone_registry_payload() {
    context(|auth| {
        install(auth, &[(1, 11, 0, "")], 1);
        let (attempt, root, frame, network) = start(auth, ResourcePurpose::Runtime);
        let owner = construct(auth, frame, network, 11);
        auth.with_resource(|state, ordinary, registry| {
            let sum = state.sum_adopt(ordinary, registry, 14, frame, owner, root)?;
            assert_eq!(state.sum_tag(sum)?, 1);
            assert_eq!(state.counts(ordinary, registry).owners, 1);
            assert!(!state.handles.contains_key(&owner));
            let owner = state.sum_take(ordinary, registry, 15, frame, sum, frame)?;
            assert!(!state.handles.contains_key(&sum));
            state.close_or_drop(registry, 7, frame, owner)
        })
        .unwrap();
        complete(auth, attempt, 0);
        assert_empty(auth);
        assert_eq!(events(auth), vec![(11, 1), (11, 5)]);
    });
}

#[test]
fn native_resource_state_descriptor_and_prepared_handles_cannot_be_replayed() {
    context(|auth| {
        install(auth, &[(1, 12, 0, "")], 1);
        let (attempt, _, frame, network) = start(auth, ResourcePurpose::Runtime);
        auth.with_resource(|state, ordinary, registry| {
            let descriptor = state.descriptor(ordinary, 12)?;
            let call = state.prepare_hook(ordinary, 13, frame, Some(descriptor))?;
            let prepared = state.prepare_factory(ordinary, registry, call, network)?;
            state.factory(ordinary, registry, prepared, network, 12)?;
            assert_eq!(
                state.factory(ordinary, registry, prepared, network, 12),
                Err(NativeResourceError::WrongFamily)
            );
            assert_eq!(
                state.prepare_hook(ordinary, 13, frame, Some(state.program_handle)),
                Err(NativeResourceError::WrongFamily)
            );
            Ok(())
        })
        .unwrap();
        complete(auth, attempt, 0);
        assert_empty(auth);
        assert_eq!(events(auth), vec![(12, 1), (12, 5)]);
    });
}

#[test]
fn native_resource_state_ordinary_failure_is_sampled_not_reset_or_permanently_latched() {
    context(|auth| {
        install(auth, &[], 2);
        let (attempt, _, _, _) = start(auth, ResourcePurpose::Runtime);
        assert_ne!(
            unsafe {
                values::jett_rt_v1_assert_fail(
                    auth.key.owner_address as *const JettRuntimeContextV1,
                )
            },
            0
        );
        let completion = complete(auth, attempt, 0);
        assert_eq!(completion.body_status, 0);
        assert_eq!(completion.selected_kind, 1);
        assert_empty(auth);
        auth.with_resource(|state, _, _| {
            assert_eq!(state.completed_attempt(attempt)?, completion);
            assert_eq!(state.completed_messages(attempt)?.1, b"assertion failed");
            Ok(())
        })
        .unwrap();
        auth.with_state(|_, state| {
            assert!(state.values.resource_failure().is_some());
            assert_eq!(
                state.resource_state.as_mut().unwrap().begin_entry(
                    &state.values,
                    &state.resources,
                    entry(),
                    ResourcePurpose::Runtime
                ),
                Err(NativeResourceError::Ordinary(
                    JettRuntimeStatusV1::INVALID_ARGUMENT
                ))
            );
            Ok(())
        })
        .unwrap();
        // This is the real existing Source operation, not an adapter reset hook.
        let prefix = b"handled: ";
        let text = unsafe {
            values::jett_rt_v1_failure_take_prefixed_text(
                auth.key.owner_address as *const JettRuntimeContextV1,
                prefix.as_ptr(),
                prefix.len() as u64,
            )
        };
        assert_ne!(text, 0);
        auth.with_state(|_, state| {
            state
                .values
                .drop_resource_ordinary_companion(text)
                .map_err(ordinary_error)
        })
        .unwrap();
        let (attempt, _, _, _) = start(auth, ResourcePurpose::Runtime);
        assert_eq!(complete(auth, attempt, 0).selected_kind, 0);
        assert_empty(auth);
    });
    context(|auth| {
        install(auth, &[], 1);
        let (attempt, _, _, _) = start(auth, ResourcePurpose::Runtime);
        let pointer = auth.key.owner_address as *const JettRuntimeContextV1;
        assert_ne!(unsafe { values::jett_rt_v1_assert_fail(pointer) }, 0);
        let text =
            unsafe { values::jett_rt_v1_failure_take_prefixed_text(pointer, ptr::null(), 0) };
        assert_ne!(text, 0);
        auth.with_state(|_, state| {
            state
                .values
                .drop_resource_ordinary_companion(text)
                .map_err(ordinary_error)
        })
        .unwrap();
        // A Source handler legally turned the ordinary error into data before completion.
        let completion = complete(auth, attempt, 0);
        assert_eq!(completion.body_status, 0);
        assert_eq!(completion.cleanup_status, 0);
        assert_eq!(completion.selected_kind, 0);
        assert_empty(auth);
    });
    context(|auth| {
        install(auth, &[], 1);
        let (attempt, _, frame, _) = start(auth, ResourcePurpose::Runtime);
        let refusal =
            auth.with_resource(|state, ordinary, _| state.prepare_hook(ordinary, 999, frame, None));
        assert_eq!(refusal, Err(NativeResourceError::WrongOperation));
        let pointer = auth.key.owner_address as *const JettRuntimeContextV1;
        assert_ne!(unsafe { values::jett_rt_v1_assert_fail(pointer) }, 0);
        let text =
            unsafe { values::jett_rt_v1_failure_take_prefixed_text(pointer, ptr::null(), 0) };
        auth.with_state(|_, state| {
            state
                .values
                .drop_resource_ordinary_companion(text)
                .map_err(ordinary_error)
        })
        .unwrap();
        // Clearing only the ordinary channel cannot erase the Resource-owned refusal.
        let completion = complete(auth, attempt, 0);
        assert_eq!(completion.body_status, 0);
        assert_eq!(completion.selected_kind, 1);
        assert_empty(auth);
    });
}

#[test]
fn native_resource_state_reverse_drop_and_replacement_cleanup_are_real_once_only_core_events() {
    context(|auth| {
        install(auth, &[(1, 21, 0, ""), (3, 22, 0, "")], 1);
        let (attempt, root, frame, network) = start(auth, ResourcePurpose::Runtime);
        let first = construct(auth, frame, network, 21);
        auth.with_resource(|state, ordinary, registry| {
            state.transfer(ordinary, registry, 2, frame, first, root)
        })
        .unwrap();
        construct(auth, frame, network, 22);
        let completion = complete(auth, attempt, 0);
        assert_eq!(completion.selected_kind, 3);
        assert_empty(auth);
        assert_eq!(events(auth), vec![(21, 1), (22, 1), (22, 5), (21, 5)]);
    });
    for panic_on_old in [false, true] {
        context(|auth| {
            install(
                auth,
                &[
                    (if panic_on_old { 3 } else { 1 }, 31, 0, ""),
                    (1, 32, 0, ""),
                ],
                1,
            );
            let (attempt, root, frame, network) = start(auth, ResourcePurpose::Runtime);
            let first = construct(auth, frame, network, 31);
            let first = auth
                .with_resource(|state, ordinary, registry| {
                    state.transfer(ordinary, registry, 2, frame, first, root)
                })
                .unwrap();
            let replacement = construct(auth, frame, network, 32);
            let result = auth.with_resource(|state, ordinary, registry| {
                state.replace(ordinary, registry, 10, frame, first, replacement)
            });
            if panic_on_old {
                assert_eq!(
                    result,
                    Err(NativeResourceError::Cleanup(
                        ResourceCleanupFailure::FinalizerPanic
                    ))
                );
            } else {
                assert!(result.is_ok());
            }
            let completion = complete(auth, attempt, 0);
            assert_eq!(completion.selected_kind, if panic_on_old { 3 } else { 0 });
            assert_empty(auth);
            assert_eq!(events(auth), vec![(31, 1), (32, 1), (31, 5), (32, 5)]);
        });
    }
}

#[test]
fn native_resource_state_closed_script_and_completion_layout_refuse_malformed_inputs() {
    assert_eq!(size_of::<NativeResourceCompletion>(), 24);
    assert_eq!(align_of::<NativeResourceCompletion>(), 8);
    assert_eq!(offset_of!(NativeResourceCompletion, selected_kind), 16);
    assert_eq!(size_of::<NativeResourceCounts>(), 72);
    assert_eq!(size_of::<provider::NativeTestEvent>(), 24);
    assert!(DecodedScript::decode(b"JTRST001").is_err());
    context(|auth| {
        install(auth, &[], 1);
        let (attempt, _, _, _) = start(auth, ResourcePurpose::Runtime);
        auth.with_resource(|state, ordinary, registry| {
            assert_eq!(
                state.complete_entry(
                    ordinary,
                    registry,
                    ResourceHandleId::new(attempt.raw() + 1)?,
                    0,
                    false
                ),
                Err(NativeResourceError::InvalidEntry)
            );
            Ok(())
        })
        .unwrap();
        complete(auth, attempt, 0);
        auth.with_resource(|state, ordinary, registry| {
            assert_eq!(
                state.complete_entry(ordinary, registry, attempt, 0, false),
                Err(NativeResourceError::InvalidEntry)
            );
            Ok(())
        })
        .unwrap();
        assert_empty(auth);
    });
}

#[test]
fn native_resource_state_completion_keeps_observed_nonzero_status_distinct_from_recorded_failures()
{
    for resource_failure in [false, true] {
        context(|auth| {
            install(auth, &[], 1);
            let (attempt, _, frame, _) = start(auth, ResourcePurpose::Runtime);
            if resource_failure {
                assert_eq!(
                    auth.with_resource(
                        |state, ordinary, _| state.prepare_hook(ordinary, 999, frame, None)
                    ),
                    Err(NativeResourceError::WrongOperation)
                );
            } else {
                let pointer = auth.key.owner_address as *const JettRuntimeContextV1;
                assert_ne!(unsafe { values::jett_rt_v1_assert_fail(pointer) }, 0);
            }
            let observed = JettRuntimeStatusV1::IO_FAILURE.code();
            assert_ne!(
                observed,
                NativeResourceError::WrongOperation.status().code()
            );
            assert_ne!(observed, JettRuntimeStatusV1::INVALID_ARGUMENT.code());
            let completion = complete(auth, attempt, observed);
            assert_eq!(completion.body_status, observed);
            assert_eq!(completion.cleanup_status, 0);
            assert_eq!(completion.selected_kind, 1);
            assert_empty(auth);
            auth.with_resource(|state, _, _| {
                assert_eq!(state.completed_attempt(attempt)?, completion);
                let (resource, ordinary) = state.completed_messages(attempt)?;
                if resource_failure {
                    assert_eq!(resource, REFUSED);
                    assert!(ordinary.is_empty());
                } else {
                    assert!(resource.is_empty());
                    assert_eq!(ordinary, b"assertion failed");
                }
                Ok(())
            })
            .unwrap();
        });
    }
}

#[test]
fn native_resource_state_completion_selects_host_panic_and_cleanup_without_rewriting_observed_status()
 {
    context(|auth| {
        install(auth, &[], 1);
        let (attempt, _, frame, _) = start(auth, ResourcePurpose::Runtime);
        assert_eq!(
            auth.with_resource(|state, ordinary, _| state.prepare_hook(ordinary, 999, frame, None)),
            Err(NativeResourceError::WrongOperation)
        );
        let completion = auth
            .with_resource(|state, ordinary, registry| {
                state.complete_entry(ordinary, registry, attempt, 0, true)
            })
            .unwrap();
        assert_eq!(completion.body_status, 0);
        assert_eq!(completion.cleanup_status, 0);
        assert_eq!(completion.selected_kind, 2);
        assert_empty(auth);
        auth.with_resource(|state, _, _| {
            // The earlier Resource refusal remains recorded independently of the host outcome.
            assert_eq!(state.completed_messages(attempt)?.0, REFUSED);
            Ok(())
        })
        .unwrap();
    });
    context(|auth| {
        install(auth, &[(3, 41, 0, "")], 1);
        let (attempt, _, frame, network) = start(auth, ResourcePurpose::Runtime);
        construct(auth, frame, network, 41);
        let observed = JettRuntimeStatusV1::IO_FAILURE.code();
        let completion = auth
            .with_resource(|state, ordinary, registry| {
                state.complete_entry(ordinary, registry, attempt, observed, true)
            })
            .unwrap();
        assert_eq!(completion.body_status, observed);
        assert_eq!(completion.cleanup_status, JettRuntimeStatusV1::PANIC.code());
        assert_eq!(completion.selected_kind, 3);
        assert_empty(auth);
        assert_eq!(events(auth), vec![(41, 1), (41, 5)]);
        auth.with_resource(|state, _, _| {
            assert_eq!(state.completed_messages(attempt)?.0, CLEANUP_FAILED);
            Ok(())
        })
        .unwrap();
    });
}

#[test]
fn native_resource_state_borrow_domain_error_retires_ordinary_sum_and_string() {
    context(|auth| {
        install(auth, &[(1, 503, 0, ""), (5, 503, 0, "borrow failure")], 1);
        let (attempt, root, frame, network) = start(auth, ResourcePurpose::Runtime);
        let owner = construct(auth, frame, network, 503);
        let owner = auth
            .with_resource(|state, ordinary, registry| {
                state.transfer(ordinary, registry, 2, frame, owner, root)
            })
            .unwrap();
        let loan = auth
            .with_resource(|state, ordinary, registry| {
                state.borrow_begin(ordinary, registry, 3, frame, owner)
            })
            .unwrap();
        let result = auth
            .with_resource(|state, ordinary, registry| {
                let call = state.prepare_hook(ordinary, 4, frame, None)?;
                state.invoke_borrow(ordinary, registry, call, network, loan)
            })
            .unwrap();
        let pointer = auth.key.owner_address as *const JettRuntimeContextV1;
        assert_eq!(unsafe { values::jett_rt_v1_sum_tag(pointer, result) }, 0);
        auth.with_resource(|state, ordinary, registry| {
            assert!(ordinary.resource_failure().is_none());
            assert_eq!(state.counts(ordinary, registry).ordinary_empty, 0);
            ordinary
                .drop_resource_ordinary_companion(result)
                .map_err(ordinary_error)?;
            assert_eq!(state.counts(ordinary, registry).ordinary_empty, 1);
            assert!(ordinary.drop_resource_ordinary_companion(result).is_err());
            state.borrow_end(5, frame, loan)?;
            state.close_or_drop(registry, 6, frame, owner)?;
            state.end_operation_frame(ordinary, registry, frame)
        })
        .unwrap();
        assert_eq!(complete(auth, attempt, 0).selected_kind, 0);
        assert_empty(auth);
        assert_eq!(events(auth), vec![(503, 1), (503, 4), (503, 5)]);
    });
}

#[path = "source_tests.rs"]
mod source_tests;

#[test]
fn native_resource_replacement_preflight_keeps_both_owners_before_old_or_rhs_loan_refusal() {
    for borrow_old in [false, true] {
        context(|auth| {
            install(auth, &[(1, 41, 0, ""), (1, 42, 0, "")], 1);
            let (attempt, root, frame, network) = start(auth, ResourcePurpose::Runtime);
            let original = construct(auth, frame, network, 41);
            let old = auth
                .with_resource(|state, ordinary, registry| {
                    state.transfer(ordinary, registry, 2, frame, original, root)
                })
                .unwrap();
            let replacement = construct(auth, frame, network, 42);
            auth.with_resource(|state, ordinary, registry| {
                let NativeResourceEntry::Owner(borrowed) = state
                    .handles
                    .get(&if borrow_old { old } else { replacement })
                    .unwrap()
                else {
                    panic!("exact scripted owner");
                };
                let NativeResourceEntry::Frame(borrower) = state.handles.get(&frame).unwrap()
                else {
                    panic!("exact operation frame");
                };
                let mut loan =
                    state
                        .custody
                        .begin_borrow(&borrowed.token, &borrower.token, registry)?;
                let before = state.counts(ordinary, registry);
                assert_eq!((before.owners, before.registry, before.loans), (2, 2, 1));
                let old_key = state
                    .custody
                    .validate_owned(&state.owner(old, 0)?.token, registry)?;
                let rhs_key = state
                    .custody
                    .validate_owned(&state.owner(replacement, 1)?.token, registry)?;
                assert_eq!(
                    state.replace(ordinary, registry, 10, frame, old, replacement),
                    Err(NativeResourceError::Custody(CustodyError::ActiveBorrow))
                );
                assert_eq!(state.counts(ordinary, registry), before);
                assert_eq!(
                    state
                        .custody
                        .validate_owned(&state.owner(old, 0)?.token, registry)?,
                    old_key
                );
                assert_eq!(
                    state
                        .custody
                        .validate_owned(&state.owner(replacement, 1)?.token, registry)?,
                    rhs_key
                );
                assert_eq!(
                    state.provider.observe_events(|events| events
                        .iter()
                        .map(|event| (event.label, event.kind))
                        .collect::<Vec<_>>()),
                    vec![(41, 1), (42, 1)]
                );
                state.custody.end_borrow(&mut loan)?;
                let installed = state.replace(ordinary, registry, 10, frame, old, replacement)?;
                assert!(!state.handles.contains_key(&old));
                assert!(!state.handles.contains_key(&replacement));
                assert_eq!(
                    state
                        .custody
                        .validate_owned(&state.owner(installed, 0)?.token, registry)?,
                    rhs_key
                );
                assert_eq!(
                    (
                        state.counts(ordinary, registry).owners,
                        registry.live_count()
                    ),
                    (1, 1)
                );
                Ok(())
            })
            .unwrap();
            assert_eq!(events(auth), vec![(41, 1), (42, 1), (41, 5)]);
            let completion = complete(auth, attempt, 0);
            assert_eq!(
                (
                    completion.body_status,
                    completion.cleanup_status,
                    completion.selected_kind
                ),
                (0, 0, 0)
            );
            assert_empty(auth);
            assert_eq!(events(auth), vec![(41, 1), (42, 1), (41, 5), (42, 5)]);
        });
    }
}

#[test]
fn native_resource_replacement_preflight_stale_rhs_handle_has_no_finalizer_effect() {
    context(|auth| {
        install(auth, &[(1, 51, 0, ""), (1, 52, 0, "")], 1);
        let (attempt, root, frame, network) = start(auth, ResourcePurpose::Runtime);
        let stale = construct(auth, frame, network, 51);
        let old = auth
            .with_resource(|state, ordinary, registry| {
                state.transfer(ordinary, registry, 2, frame, stale, root)
            })
            .unwrap();
        let replacement = construct(auth, frame, network, 52);
        auth.with_resource(|state, ordinary, registry| {
            let before = state.counts(ordinary, registry);
            assert_eq!((before.owners, before.registry), (2, 2));
            assert!(!state.handles.contains_key(&stale));
            assert_eq!(
                state.replace(ordinary, registry, 10, frame, old, stale),
                Err(NativeResourceError::InvalidHandle)
            );
            assert_eq!(state.counts(ordinary, registry), before);
            state
                .custody
                .validate_owned(&state.owner(old, 0)?.token, registry)?;
            state
                .custody
                .validate_owned(&state.owner(replacement, 1)?.token, registry)?;
            assert_eq!(
                state.provider.observe_events(|events| events
                    .iter()
                    .map(|event| (event.label, event.kind))
                    .collect::<Vec<_>>()),
                vec![(51, 1), (52, 1)]
            );
            state.replace(ordinary, registry, 10, frame, old, replacement)?;
            Ok(())
        })
        .unwrap();
        assert_eq!(events(auth), vec![(51, 1), (52, 1), (51, 5)]);
        assert_eq!(complete(auth, attempt, 0).selected_kind, 0);
        assert_empty(auth);
        assert_eq!(events(auth), vec![(51, 1), (52, 1), (51, 5), (52, 5)]);
    });
}

#[path = "borrowed_sum_tests.rs"]
mod borrowed_sum_tests;
