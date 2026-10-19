//! Root observation controls; distinct from Source/native linked acceptance.
use super::*;

#[derive(Debug, PartialEq, Eq)]
struct Observation {
    counts: NativeResourceCounts,
    body: Option<u32>,
    cleanup: Option<ResourceCleanupFailure>,
    root_body: Option<u32>,
    ordinary: Option<u32>,
    script_remaining: usize,
}

fn observation(auth: &AuthenticatedResourceContext) -> Observation {
    auth.with_state(|_, state| {
        let resource = state.resource_state.as_ref().unwrap();
        let attempt = resource.attempt.as_ref().unwrap();
        Ok(Observation {
            counts: resource.counts(&state.values, &state.resources),
            body: attempt.body.as_ref().map(|failure| match failure {
                NativeBodyFailure::Resource(error) => error.status().code(),
                NativeBodyFailure::HostPanic => JettRuntimeStatusV1::PANIC.code(),
            }),
            cleanup: attempt.cleanup,
            root_body: attempt.root_body_status,
            ordinary: state
                .values
                .resource_failure()
                .map(|failure| failure.status.code()),
            script_remaining: resource.script_remaining(),
        })
    })
    .unwrap()
}

fn observe(auth: &AuthenticatedResourceContext, scope: ResourceHandleId) -> ResourceResult<u32> {
    auth.with_state(|_, state| {
        state
            .resource_state
            .as_ref()
            .unwrap()
            .entry_outcome(scope, 0, 3, 0)
    })
}

fn pointer(auth: &AuthenticatedResourceContext) -> *const JettRuntimeContextV1 {
    auth.key.owner_address as *const JettRuntimeContextV1
}

fn begin(
    auth: &AuthenticatedResourceContext,
) -> (ResourceHandleId, ResourceHandleId, ResourceHandleId) {
    auth.with_resource(|s, v, r| {
        let attempt = s.begin_entry(v, r, entry(), ResourcePurpose::Runtime)?;
        let root = s.root_frame(attempt)?;
        let op = s.begin_operation_frame(v, 1, root)?;
        Ok((attempt, root, op))
    })
    .unwrap()
}

#[test]
fn native_resource_entry_outcome_c_refusals_are_readonly_and_original_body_survives_first_error() {
    context(|auth| {
        let f = fixture(5, false, false);
        let (attempt, root, op, _) = setup(auth, &f, &[]);
        let initial = observation(auth);
        let mut out = 9001;
        assert_ne!(
            unsafe {
                leaves::jett_rt_v1_resource_entry_outcome(
                    pointer(auth),
                    root.raw(),
                    0,
                    3,
                    0,
                    &mut out,
                )
            },
            JettRuntimeStatusV1::OK
        );
        assert_eq!(out, 9001);
        assert_eq!(observation(auth), initial);

        let child = auth
            .with_resource(|s, v, r| {
                let shell = s.absent_sum(v, op, f.create.unwrap())?;
                let call = s.source_prepare(v, op, f.source)?;
                s.source_actual(v, r, call, 0, 0, shell.raw())?;
                let child = s.source_enter(v, r, call)?;
                let parameter = ResourceHandleId::new(s.source_parameter(v, child, 0)?)?;
                let inner = s.begin_operation_frame(v, 4, child)?;
                let dest = s.destination_frame(inner, f.transfer_return)?;
                let provisional = s.transfer(v, r, f.transfer_return, inner, parameter, dest)?;
                s.end_operation_frame(v, r, inner)?;
                s.scope_complete(v, r, child, f.complete_scope, 0)?;
                let returned = s.return_publish(v, r, child, f.publish.unwrap(), provisional)?;
                s.source_status(v, call)?;
                s.sum_drop(v, r, f.close, op, returned)?;
                Ok(child)
            })
            .unwrap();
        let before_child_query = observation(auth);
        assert_ne!(
            unsafe {
                leaves::jett_rt_v1_resource_entry_outcome(
                    pointer(auth),
                    child.raw(),
                    1,
                    4,
                    3,
                    &mut out,
                )
            },
            JettRuntimeStatusV1::OK
        );
        assert_eq!(out, 9001);
        assert_eq!(observation(auth), before_child_query);

        auth.with_resource(|s, v, r| s.end_operation_frame(v, r, op))
            .unwrap();
        assert_ne!(unsafe { values::jett_rt_v1_assert_fail(pointer(auth)) }, 0);
        assert_ne!(
            unsafe {
                leaves::jett_rt_v1_resource_scope_complete(
                    pointer(auth),
                    root.raw(),
                    f.complete_root,
                    71,
                )
            },
            JettRuntimeStatusV1::OK
        );
        let completed_root = observation(auth);
        assert_eq!(completed_root.root_body, Some(71));
        assert!(completed_root.body.is_some() && completed_root.ordinary.is_some());
        assert_eq!(completed_root.counts.frames, 0);
        for (scope, function, signature, template) in [
            (root, 1, 3, 0),
            (root, 0, 4, 0),
            (root, 0, 3, 3),
            (child, 0, 3, 0),
        ] {
            assert_ne!(
                unsafe {
                    leaves::jett_rt_v1_resource_entry_outcome(
                        pointer(auth),
                        scope.raw(),
                        function,
                        signature,
                        template,
                        &mut out,
                    )
                },
                JettRuntimeStatusV1::OK
            );
            assert_eq!(out, 9001);
            assert_eq!(observation(auth), completed_root);
        }
        for _ in 0..2 {
            assert_eq!(
                unsafe {
                    leaves::jett_rt_v1_resource_entry_outcome(
                        pointer(auth),
                        root.raw(),
                        0,
                        3,
                        0,
                        &mut out,
                    )
                },
                JettRuntimeStatusV1::OK
            );
            assert_eq!(out, 71);
            assert_eq!(observation(auth), completed_root);
        }
        auth.with_resource(|s, v, r| {
            let completion = s.complete_entry(v, r, attempt, out, false)?;
            assert_eq!((completion.body_status, completion.selected_kind), (71, 1));
            assert_eq!(s.completed_messages(attempt)?.1, b"assertion failed");
            Ok(())
        })
        .unwrap();
        out = 9001;
        assert_ne!(
            unsafe {
                leaves::jett_rt_v1_resource_entry_outcome(
                    pointer(auth),
                    root.raw(),
                    0,
                    3,
                    0,
                    &mut out,
                )
            },
            JettRuntimeStatusV1::OK
        );
        assert_eq!(out, 9001);
    });
}

#[test]
fn native_resource_entry_outcome_zero_is_independent_of_cleanup_panic_and_c_output_preflight() {
    context(|auth| {
        let f = fixture(4, false, false);
        let (attempt, root, op, network) = setup(auth, &f, &[(3, 501, 0, "")]);
        let owner = acquire(auth, op, network);
        // Move the owner through the real helper Return into the root holder;
        // otherwise Operation cleanup would finalize it before root completion.
        auth.with_resource(|s, v, r| {
            let call = s.source_prepare(v, op, f.source)?;
            s.source_actual(v, r, call, 0, 0, owner.raw())?;
            let child = s.source_enter(v, r, call)?;
            let parameter = ResourceHandleId::new(s.source_parameter(v, child, 0)?)?;
            let inner = s.begin_operation_frame(v, 4, child)?;
            let destination = s.destination_frame(inner, f.transfer_return)?;
            let provisional = s.transfer(v, r, f.transfer_return, inner, parameter, destination)?;
            s.end_operation_frame(v, r, inner)?;
            s.scope_complete(v, r, child, f.complete_scope, 0)?;
            s.return_publish(v, r, child, f.publish.unwrap(), provisional)?;
            s.source_status(v, call)
        })
        .unwrap();
        let before = observation(auth);
        assert_eq!(before.counts.owners, 1);
        for out in [
            ptr::null_mut::<u32>(),
            pointer(auth).cast_mut().cast::<u32>(),
            pointer(auth)
                .cast::<u8>()
                .wrapping_add(1)
                .cast_mut()
                .cast::<u32>(),
            (usize::MAX - 3) as *mut u32,
        ] {
            assert_eq!(
                unsafe {
                    leaves::jett_rt_v1_resource_entry_outcome(
                        pointer(auth),
                        root.raw(),
                        0,
                        3,
                        0,
                        out,
                    )
                },
                JettRuntimeStatusV1::INVALID_ARGUMENT
            );
            assert_eq!(observation(auth), before);
        }
        auth.with_resource(|s, v, r| {
            s.end_operation_frame(v, r, op)?;
            assert_eq!(
                s.scope_complete(v, r, root, f.complete_root, 0),
                Err(NativeResourceError::Cleanup(
                    ResourceCleanupFailure::FinalizerPanic
                ))
            );
            Ok(())
        })
        .unwrap();
        let after = observation(auth);
        assert_eq!(after.root_body, Some(0));
        assert_eq!(after.cleanup, Some(ResourceCleanupFailure::FinalizerPanic));
        assert_eq!(
            (
                after.counts.owners,
                after.counts.frames,
                after.counts.registry
            ),
            (0, 0, 0)
        );
        let mut out = 9001;
        assert_eq!(
            unsafe {
                leaves::jett_rt_v1_resource_entry_outcome(
                    pointer(auth),
                    root.raw(),
                    0,
                    3,
                    0,
                    &mut out,
                )
            },
            JettRuntimeStatusV1::OK
        );
        assert_eq!(out, 0);
        assert_eq!(observation(auth), after);
        auth.with_resource(|s, v, r| {
            let completion = s.complete_entry(v, r, attempt, out, false)?;
            assert_eq!((completion.body_status, completion.selected_kind), (0, 3));
            assert_ne!(completion.cleanup_status, 0);
            Ok(())
        })
        .unwrap();
    });
}

#[test]
fn native_resource_entry_outcome_is_retained_only_after_actual_root_retirement() {
    context(|auth| {
        let f = fixture(5, false, false);
        let (attempt, root, op, _) = setup(auth, &f, &[]);
        auth.with_resource(|s, v, r| {
            s.end_operation_frame(v, r, op)?;
            let mut foreign = ResourceRegistry::new();
            assert!(matches!(
                s.scope_complete(v, &mut foreign, root, f.complete_root, 0),
                Err(NativeResourceError::Cleanup(_))
            ));
            assert!(s.frame(root).is_ok());
            assert_eq!(s.attempt.as_ref().unwrap().root_body_status, None);
            Ok(())
        })
        .unwrap();
        let before = observation(auth);
        assert!(observe(auth, root).is_err());
        assert_eq!(observation(auth), before);
        auth.with_resource(|s, v, r| {
            // The prior cleanup refusal remains selected; this does not reset it.
            assert_eq!(
                s.scope_complete(v, r, root, f.complete_root, 0),
                Err(NativeResourceError::BodyFailed)
            );
            assert!(s.frame(root).is_err());
            Ok(())
        })
        .unwrap();
        assert_eq!(observe(auth, root), Ok(0));
        auth.with_resource(|s, v, r| {
            let completion = s.complete_entry(v, r, attempt, 0, false)?;
            assert_eq!(completion.selected_kind, 3);
            Ok(())
        })
        .unwrap();
    });
}

#[test]
fn native_resource_entry_outcome_refuses_foreign_and_previous_attempt_roots() {
    context(|auth| {
        let f = fixture(5, false, false);
        install_scripted(auth, &f.bytes, entry(), script(&[], 2)).unwrap();
        let (first, first_root, first_op) = begin(auth);
        auth.with_resource(|s, v, r| {
            s.end_operation_frame(v, r, first_op)?;
            s.scope_complete(v, r, first_root, f.complete_root, 0)?;
            Ok(())
        })
        .unwrap();
        assert_eq!(observe(auth, first_root), Ok(0));
        auth.with_resource(|s, v, r| s.complete_entry(v, r, first, 0, false))
            .unwrap();
        let (second, second_root, second_op) = begin(auth);
        let current = observation(auth);
        assert!(observe(auth, first_root).is_err());
        assert_eq!(observation(auth), current);
        auth.with_resource(|s, v, r| {
            s.end_operation_frame(v, r, second_op)?;
            s.scope_complete(v, r, second_root, f.complete_root, 0)?;
            Ok(())
        })
        .unwrap();
        let retired = observation(auth);
        context(|foreign_auth| {
            let (foreign_attempt, foreign_root, foreign_op, _) = setup(foreign_auth, &f, &[]);
            foreign_auth
                .with_resource(|s, v, r| {
                    s.end_operation_frame(v, r, foreign_op)?;
                    s.scope_complete(v, r, foreign_root, f.complete_root, 0)?;
                    Ok(())
                })
                .unwrap();
            let foreign_before = observation(foreign_auth);
            assert_ne!(foreign_root, second_root);
            assert!(observe(auth, foreign_root).is_err());
            assert!(observe(foreign_auth, second_root).is_err());
            assert_eq!(observation(auth), retired);
            assert_eq!(observation(foreign_auth), foreign_before);
            assert_eq!(observe(foreign_auth, foreign_root), Ok(0));
            foreign_auth
                .with_resource(|s, v, r| s.complete_entry(v, r, foreign_attempt, 0, false))
                .unwrap();
        });
        assert!(observe(auth, first_root).is_err());
        assert_eq!(observe(auth, second_root), Ok(0));
        auth.with_resource(|s, v, r| s.complete_entry(v, r, second, 0, false))
            .unwrap();
        assert!(observe(auth, second_root).is_err());
    });
}

#[test]
fn native_resource_entry_outcome_refuses_required_worker_purpose_without_latching() {
    context(|auth| {
        let f = fixture(5, false, false);
        install_scripted(auth, &f.bytes, entry(), script(&[], 1)).unwrap();
        let (attempt, root) = auth
            .with_resource(|s, v, r| {
                let attempt = s.begin_entry(v, r, entry(), ResourcePurpose::ExplicitComptime)?;
                let root = s.root_frame(attempt)?;
                s.scope_complete(v, r, root, f.complete_root, 0)?;
                Ok((attempt, root))
            })
            .unwrap();
        let before = observation(auth);
        assert_eq!(observe(auth, root), Err(NativeResourceError::WrongPurpose));
        assert_eq!(observation(auth), before);
        auth.with_resource(|s, v, r| {
            let completion = s.complete_entry(v, r, attempt, 0, false)?;
            assert_eq!(completion.selected_kind, 0);
            Ok(())
        })
        .unwrap();
    });
}
