//! Malformed-host and real core transition controls, distinct from Source-native acceptance.
use super::super::*;
use super::{context, script, words};

struct Fixture {
    bytes: Vec<u8>,
    source: u32,
    transfer_return: u32,
    complete_scope: u32,
    publish: Option<u32>,
    complete_op: u32,
    complete_root: u32,
    complete_inner: u32,
    close: u32,
    create: Option<u32>,
    borrow: Option<u32>,
    invoke_borrow: Option<u32>,
    end_borrow: Option<u32>,
    take_fail: Option<u32>,
}
fn fixture(shape: u32, resident: bool, reorder: bool) -> Fixture {
    let signatures = vec![
        vec![0, 2, 6, 0, 2, 1, 1],
        vec![1, 2, 7, 0, 2, 4, 2],
        vec![2, 1, 3, 4, 1],
        vec![3, 1, 3, 0, 2],
        if resident {
            vec![4, 2, 3, 4, 2, 0, 2]
        } else {
            vec![4, 1, shape, shape, 1]
        },
    ];
    let shapes = vec![
        vec![0, 6],
        vec![1, 1, 64, 1],
        vec![2, 4],
        vec![3, 5],
        vec![4, 7, 0],
        vec![5, 8, 4],
        vec![6, 9, 4, 2],
        vec![7, 9, 1, 2],
    ];
    let parent = if resident { 1 } else { 2 };
    let frames = vec![
        vec![0, 1, 0, 0, 0, 0, 3, 1, 0, 0],
        vec![1, 2, 0, 0, 0, 1, 3, 1, 1, 0],
        vec![2, 3, 1, 0, 0, 2, 4, 1, 1, 1],
        vec![3, 1, 1, 0, 0, 3, 4, 1, 1, parent],
        vec![4, 2, 1, 0, 0, 4, 4, 1, 1, 3],
    ];
    let path = if shape == 5 {
        vec![1, 1]
    } else if shape == 6 {
        vec![1, 2]
    } else {
        vec![0]
    };
    let mut slots = Vec::new();
    for (id, frame, ty) in [
        (0, 0, shape),
        (1, 1, shape),
        (2, 1, 6),
        (3, 2, shape),
        (4, 3, shape),
        (5, 4, shape),
        (6, 1, 4),
        (7, 0, 4),
    ] {
        let mut row = vec![id, frame, ty];
        row.extend(if ty == 6 {
            vec![1, 2]
        } else if ty == 4 {
            vec![0]
        } else {
            path.clone()
        });
        slots.push(row);
    }
    let mut ops: Vec<(u32, Vec<u32>)> = Vec::new();
    let mut add = |function, row: Vec<u32>| {
        let id = ops.len() as u32;
        ops.push((function, row));
        id
    };
    add(0, vec![1, 1, 0, 2]); // factory -> typed Result slot
    add(0, vec![10, 1, 2, 6]); // occupied result -> independent plain slot
    add(
        0,
        vec![2, 1, 6, if !resident && shape == 4 { 1 } else { 7 }],
    );
    let borrow = if resident {
        Some(add(0, vec![3, 1, 7, 1]))
    } else {
        None
    };
    let mut row = vec![16, 1, 1, 4, 3];
    if resident {
        row.push(0);
        row.extend([
            2,
            if reorder { 1 } else { 0 },
            if reorder { 0 } else { 1 },
            2,
        ]);
        row.extend([
            0,
            if reorder { 1 } else { 0 },
            4,
            4,
            2,
            4,
            2,
            3,
            1,
            borrow.unwrap(),
        ]);
        row.extend([1, if reorder { 0 } else { 1 }, 0, 0, 1, 1, 2, 1]);
        row.extend([1, 3]);
    } else {
        row.extend([1, 2, 1, 0, 1, 0, 0, shape, shape, 1, 2, 1, 2, 1, 4]);
        row.extend([2, shape, 0, 0, 2, 1, 3]);
    }
    let source = add(0, row);
    let transfer_return = if resident {
        u32::MAX
    } else {
        add(1, vec![2, 4, 4, 3])
    };
    let invoke_borrow = if resident {
        Some(add(1, vec![6, 4, 1, 2, 3, 0]))
    } else {
        None
    };
    let complete_inner = add(1, vec![13, 4]);
    let complete_scope = add(1, vec![13, 3]);
    let publish = if resident {
        None
    } else {
        Some(add(1, vec![18, 3, 3]))
    };
    let end_borrow = borrow.map(|borrow| add(0, vec![5, 1, borrow]));
    let complete_op = add(0, vec![13, 1]);
    let complete_root = add(0, vec![13, 0]);
    let close = if !resident && shape != 4 {
        add(0, vec![11, 1, 0])
    } else {
        add(0, vec![7, 1, 2, if resident { 7 } else { 0 }])
    };
    let create = if shape == 5 {
        Some(add(0, vec![19, 1, 1]))
    } else if shape == 6 {
        Some(add(0, vec![20, 1, 1, 2]))
    } else {
        None
    };
    let take_fail = if shape == 6 {
        Some(add(0, vec![17, 1, 0, 2]))
    } else {
        None
    };
    let mut bytes = b"JTRSC001".to_vec();
    words(&mut bytes, &[2, 0]);
    bytes.extend_from_slice(&0u64.to_le_bytes());
    words(
        &mut bytes,
        &[
            1,
            3,
            signatures.len() as u32,
            shapes.len() as u32,
            frames.len() as u32,
            slots.len() as u32,
            ops.len() as u32,
            0,
        ],
    );
    words(&mut bytes, &[0]);
    for row in [[0, 0, 1, 0], [1, 0, 2, 1], [2, 0, 3, 2]] {
        words(&mut bytes, &row);
    }
    for row in signatures
        .into_iter()
        .chain(shapes)
        .chain(frames)
        .chain(slots)
    {
        words(&mut bytes, &row);
    }
    for (id, (function, row)) in ops.into_iter().enumerate() {
        words(&mut bytes, &[id as u32, function, 0, 0, 100 + id as u32]);
        words(&mut bytes, &row);
    }
    let len = bytes.len() as u64;
    bytes[16..24].copy_from_slice(&len.to_le_bytes());
    Fixture {
        bytes,
        source,
        transfer_return,
        complete_scope,
        publish,
        complete_op,
        complete_root,
        complete_inner,
        close,
        create,
        borrow,
        invoke_borrow,
        end_borrow,
        take_fail,
    }
}
fn entry() -> NativeEntry {
    NativeEntry {
        function: 0,
        signature: 3,
        scope: 0,
    }
}
fn setup(
    auth: &AuthenticatedResourceContext,
    f: &Fixture,
    rows: &[(u32, i64, i64, &str)],
) -> (ResourceHandleId, ResourceHandleId, ResourceHandleId, u64) {
    install_scripted(auth, &f.bytes, entry(), script(rows, 1)).unwrap();
    auth.with_resource(|s, v, r| {
        let attempt = s.begin_entry(v, r, entry(), ResourcePurpose::Runtime)?;
        let root = s.root_frame(attempt)?;
        let op = s.begin_operation_frame(v, 1, root)?;
        let network = s.entry_network(attempt, 0)?;
        Ok((attempt, root, op, network))
    })
    .unwrap()
}
fn acquire(
    auth: &AuthenticatedResourceContext,
    op: ResourceHandleId,
    network: u64,
) -> ResourceHandleId {
    auth.with_resource(|s, v, r| {
        let call = s.prepare_hook(v, 0, op, None)?;
        let prepared = s.prepare_factory(v, r, call, network)?;
        let result = s.factory(v, r, prepared, network, 501)?;
        let value = s.sum_take(v, r, 1, op, ResourceHandleId::new(result.value)?, op)?;
        s.transfer(v, r, 2, op, value, s.destination_frame(op, 2)?)
    })
    .unwrap()
}
fn finish(auth: &AuthenticatedResourceContext, attempt: ResourceHandleId) {
    auth.with_resource(|s, v, r| {
        let completion = s.complete_entry(v, r, attempt, 0, false)?;
        assert_eq!(completion.selected_kind, 0);
        let counts = s.counts(v, r);
        assert_eq!(
            (
                counts.owners,
                counts.loans,
                counts.frames,
                counts.registry,
                counts.provisional
            ),
            (0, 0, 0, 0, 0)
        );
        Ok(())
    })
    .unwrap();
}

#[test]
fn native_resource_source_owned_activation_parameter_return_and_publication_are_clone_free() {
    context(|auth| {
        let f = fixture(4, false, false);
        let (attempt, root, op, network) = setup(auth, &f, &[(1, 501, 0, "")]);
        let owner = acquire(auth, op, network);
        auth.with_resource(|s, v, r| {
            let call = s.source_prepare(v, op, f.source)?;
            s.source_actual(v, r, call, 0, 0, owner.raw())?;
            let scope = s.source_enter(v, r, call)?;
            s.scope_validate(v, scope, 1, 4, 3)?;
            assert!(!s.handles.contains_key(&owner));
            let parameter = ResourceHandleId::new(s.source_parameter(v, scope, 0)?)?;
            let inner = s.begin_operation_frame(v, 4, scope)?;
            let return_frame = s.destination_frame(inner, f.transfer_return)?;
            let provisional =
                s.transfer(v, r, f.transfer_return, inner, parameter, return_frame)?;
            s.end_operation_frame(v, r, inner)?;
            s.scope_complete(v, r, scope, f.complete_scope, 0)?;
            assert_eq!(s.counts(v, r).provisional, 1);
            // Same slot/type from a different activation cannot satisfy Return publication.
            let NativeResourceEntry::Owner(header) = s.handles.get_mut(&provisional).unwrap()
            else {
                panic!("plain provisional owner")
            };
            let recorded_frame = header.frame;
            header.frame = root;
            assert_eq!(
                s.return_publish(v, r, scope, f.publish.unwrap(), provisional),
                Err(NativeResourceError::WrongFrame)
            );
            let NativeResourceEntry::Owner(header) = s.handles.get_mut(&provisional).unwrap()
            else {
                panic!("unchanged owner")
            };
            header.frame = recorded_frame;
            let output = s.return_publish(v, r, scope, f.publish.unwrap(), provisional)?;
            assert!(!s.handles.contains_key(&provisional));
            s.source_status(v, call)?;
            assert_eq!(s.owner(output, 0)?.frame, root);
            s.close_or_drop(r, f.close, op, output)?;
            s.end_operation_frame(v, r, op)?;
            Ok(())
        })
        .unwrap();
        finish(auth, attempt);
        auth.with_resource(|s, _, _| {
            s.observe_events(|events| {
                assert_eq!(
                    events.iter().map(|e| e.kind).collect::<Vec<_>>(),
                    vec![1, 5]
                )
            });
            Ok(())
        })
        .unwrap();
    });
}

#[test]
fn native_resource_source_named_actual_order_and_resident_view_preserve_one_parent_loan() {
    context(|auth| {
        let f = fixture(4, true, true);
        let (attempt, _, op, network) = setup(auth, &f, &[(1, 501, 0, ""), (4, 501, 7, "")]);
        let owner = acquire(auth, op, network);
        auth.with_resource(|s, v, r| {
            let loan = s.borrow_begin(v, r, f.borrow.unwrap(), op, owner)?;
            let call = s.source_prepare(v, op, f.source)?;
            // Formal zero is the SECOND source actual. No evaluation occurs inside these leaves.
            s.source_actual(v, r, call, 0, 1, network)?;
            s.source_actual(v, r, call, 1, 0, loan.raw())?;
            let scope = s.source_enter(v, r, call)?;
            assert_eq!(s.source_parameter(v, scope, 0)?, loan.raw());
            let inner = s.begin_operation_frame(v, 4, scope)?;
            let hook = s.prepare_hook(v, f.invoke_borrow.unwrap(), inner, None)?;
            let result = s.invoke_borrow(v, r, hook, network, loan)?;
            v.drop_resource_ordinary_companion(result)
                .map_err(ordinary_error)?;
            s.end_operation_frame(v, r, inner)?;
            s.scope_complete(v, r, scope, f.complete_scope, 0)?;
            s.source_status(v, call)?;
            assert_eq!(s.counts(v, r).loans, 1);
            s.borrow_end(f.end_borrow.unwrap(), op, loan)?;
            s.close_or_drop(r, f.close, op, owner)?;
            s.end_operation_frame(v, r, op)?;
            Ok(())
        })
        .unwrap();
        finish(auth, attempt);
    });
}

#[test]
fn native_resource_source_absent_shell_moves_through_formal_and_return_without_owner_recovery() {
    context(|auth| {
        let f = fixture(5, false, false);
        let (attempt, _, op, _) = setup(auth, &f, &[]);
        auth.with_resource(|s, v, r| {
            let shell = s.absent_sum(v, op, f.create.unwrap())?;
            let call = s.source_prepare(v, op, f.source)?;
            s.source_actual(v, r, call, 0, 0, shell.raw())?;
            let scope = s.source_enter(v, r, call)?;
            let parameter = ResourceHandleId::new(s.source_parameter(v, scope, 0)?)?;
            let inner = s.begin_operation_frame(v, 4, scope)?;
            let destination = s.destination_frame(inner, f.transfer_return)?;
            let value = s.transfer(v, r, f.transfer_return, inner, parameter, destination)?;
            assert_eq!(s.sum_tag(value)?, 0);
            assert_eq!(s.custody.live_owners(), 0);
            s.end_operation_frame(v, r, inner)?;
            s.scope_complete(v, r, scope, f.complete_scope, 0)?;
            let value = s.return_publish(v, r, scope, f.publish.unwrap(), value)?;
            s.source_status(v, call)?;
            s.sum_drop(v, r, f.close, op, value)?;
            s.end_operation_frame(v, r, op)?;
            Ok(())
        })
        .unwrap();
        finish(auth, attempt);
    });
}

#[test]
fn native_resource_source_failure_shell_companion_moves_once_and_never_adopts_a_resource_owner() {
    context(|auth| {
        let f = fixture(6, false, false);
        let (attempt, _, op, _) = setup(auth, &f, &[]);
        auth.with_resource(|s, v, r| {
            let mut ids = v
                .prepare_resource_ordinary_output()
                .map_err(ordinary_error)?;
            let text = v
                .publish_resource_error("ordinary domain failure".to_owned(), &mut ids)
                .map_err(ordinary_error)?;
            let shell = s.failure_sum(v, op, f.create.unwrap(), text)?;
            let call = s.source_prepare(v, op, f.source)?;
            s.source_actual(v, r, call, 0, 0, shell.raw())?;
            let scope = s.source_enter(v, r, call)?;
            let parameter = ResourceHandleId::new(s.source_parameter(v, scope, 0)?)?;
            let inner = s.begin_operation_frame(v, 4, scope)?;
            let dest = s.destination_frame(inner, f.transfer_return)?;
            let value = s.transfer(v, r, f.transfer_return, inner, parameter, dest)?;
            s.end_operation_frame(v, r, inner)?;
            s.scope_complete(v, r, scope, f.complete_scope, 0)?;
            let value = s.return_publish(v, r, scope, f.publish.unwrap(), value)?;
            let taken = s.failure_companion_take(v, op, f.take_fail.unwrap(), value)?;
            assert_eq!(taken, text);
            assert_eq!(s.custody.live_owners(), 0);
            assert!(!s.handles.contains_key(&value));
            v.drop_resource_ordinary_companion(taken)
                .map_err(ordinary_error)?;
            s.end_operation_frame(v, r, op)?;
            Ok(())
        })
        .unwrap();
        finish(auth, attempt);
    });
}

#[test]
fn native_resource_source_missing_or_duplicate_actuals_cannot_open_a_callee() {
    context(|auth| {
        let f = fixture(4, false, false);
        let (attempt, _, op, network) = setup(auth, &f, &[(1, 501, 0, "")]);
        let owner = acquire(auth, op, network);
        // Inspect direct refused transitions without latching the separate body channel.
        auth.with_resource(|s, v, r| {
            let call = s.source_prepare(v, op, f.source)?;
            assert_eq!(
                s.source_enter(v, r, call),
                Err(NativeResourceError::WrongOperation)
            );
            assert_eq!(s.custody.active_frames(), 2);
            s.source_actual(v, r, call, 0, 0, owner.raw())?;
            assert_eq!(
                s.source_actual(v, r, call, 0, 0, owner.raw()),
                Err(NativeResourceError::WrongOperation)
            );
            assert_eq!(s.custody.live_owners(), 1);
            assert!(s.handles.contains_key(&owner));
            s.end_operation_frame(v, r, op)?;
            Ok(())
        })
        .unwrap();
        finish(auth, attempt);
    });
}

#[test]
fn native_resource_source_failed_scope_cancels_provisional_return_before_any_publication() {
    context(|auth| {
        let f = fixture(4, false, false);
        let (attempt, _, op, network) = setup(auth, &f, &[(1, 501, 0, "")]);
        let owner = acquire(auth, op, network);
        auth.with_resource(|s, v, r| {
            let call = s.source_prepare(v, op, f.source)?;
            s.source_actual(v, r, call, 0, 0, owner.raw())?;
            let scope = s.source_enter(v, r, call)?;
            let parameter = ResourceHandleId::new(s.source_parameter(v, scope, 0)?)?;
            let inner = s.begin_operation_frame(v, 4, scope)?;
            let dest = s.destination_frame(inner, f.transfer_return)?;
            let provisional = s.transfer(v, r, f.transfer_return, inner, parameter, dest)?;
            s.end_operation_frame(v, r, inner)?;
            assert_eq!(
                s.scope_complete(v, r, scope, f.complete_scope, 71),
                Err(NativeResourceError::BodyFailed)
            );
            assert!(!s.handles.contains_key(&provisional));
            assert_eq!(s.counts(v, r).provisional, 0);
            assert_eq!(s.source_completed_body_status(call)?, Some(71));
            let original = NativeResourceError::SourceBodyStatus(JettRuntimeStatusV1(71));
            assert_eq!(s.source_status(v, call), Err(original));
            assert_eq!(original.status().code(), 71);
            // A different current activation or tuple cannot borrow this outcome.
            let root = s.root_frame(attempt)?;
            s.active_frames.push(root);
            assert_eq!(
                s.source_status(v, call),
                Err(NativeResourceError::WrongFrame)
            );
            s.active_frames.pop();
            let Some(NativeResourceEntry::Frame(header)) = s.handles.get_mut(&op) else {
                panic!("exact live caller operation")
            };
            let template = header.template;
            header.template = 0;
            assert_eq!(
                s.source_status(v, call),
                Err(NativeResourceError::WrongFrame)
            );
            let Some(NativeResourceEntry::Frame(header)) = s.handles.get_mut(&op) else {
                panic!("unchanged live caller operation")
            };
            header.template = template;
            assert_eq!(s.source_status(v, call), Err(original));
            assert!(
                s.return_publish(v, r, scope, f.publish.unwrap(), provisional)
                    .is_err()
            );
            s.end_operation_frame(v, r, op)?;
            assert_eq!(
                s.source_status(v, call),
                Err(NativeResourceError::WrongFamily)
            );
            let completion = s.complete_entry(v, r, attempt, 71, false)?;
            assert_eq!((completion.body_status, completion.selected_kind), (71, 1));
            assert_eq!(s.counts(v, r).owners, 0);
            Ok(())
        })
        .unwrap();
    });
}

#[test]
fn native_resource_source_c_outputs_refuse_overlap_before_opening_any_operation_or_provider() {
    let mut raw = Box::new(JettRuntimeContextV1::retired());
    let mut result = JettRuntimeResultV1::ok();
    let ptr = &mut *raw as *mut JettRuntimeContextV1;
    assert_eq!(
        unsafe { jett_rt_v1_context_create(JETT_RUNTIME_ABI_VERSION_V1, ptr, &mut result) },
        JettRuntimeStatusV1::OK
    );
    let auth = AuthenticatedResourceContext::acquire(ptr).unwrap();
    let f = fixture(4, false, false);
    let (attempt, root, _, _) = setup(&auth, &f, &[(1, 501, 0, "")]);
    // A writable result overlapping the immutable Context cannot be an output carrier.
    assert_eq!(
        unsafe {
            leaves::jett_rt_v1_resource_operation_begin(ptr, root.raw(), 1, ptr.cast::<u64>())
        },
        JettRuntimeStatusV1::INVALID_ARGUMENT
    );
    assert_eq!(
        unsafe {
            leaves::jett_rt_v1_resource_entry_scope(
                ptr,
                &mut 0u64,
                ptr.cast::<JettRuntimeResultV1>(),
            )
        },
        JettRuntimeStatusV1::INVALID_ARGUMENT
    );
    auth.with_resource(|s, v, r| {
        assert_eq!(s.custody.active_frames(), 2);
        assert_eq!(s.script_remaining(), 1);
        let completion = s.complete_entry(v, r, attempt, 0, false)?;
        assert_eq!(completion.selected_kind, 0);
        Ok(())
    })
    .unwrap();
    drop(auth);
    assert_eq!(
        unsafe { jett_rt_v1_context_destroy(ptr, &mut result) },
        JettRuntimeStatusV1::OK
    );
}

#[test]
fn native_resource_source_entry_getters_return_real_root_and_paired_network_without_new_frames() {
    let mut raw = Box::new(JettRuntimeContextV1::retired());
    let mut result = JettRuntimeResultV1::ok();
    let ptr = &mut *raw as *mut JettRuntimeContextV1;
    assert_eq!(
        unsafe { jett_rt_v1_context_create(JETT_RUNTIME_ABI_VERSION_V1, ptr, &mut result) },
        JettRuntimeStatusV1::OK
    );
    let auth = AuthenticatedResourceContext::acquire(ptr).unwrap();
    let f = fixture(4, false, false);
    let (attempt, root, _, network) = setup(&auth, &f, &[]);
    let mut scope = 0;
    let mut capability = 0;
    assert_eq!(
        unsafe { leaves::jett_rt_v1_resource_entry_scope(ptr, &mut scope, &mut result) },
        JettRuntimeStatusV1::OK
    );
    assert_eq!(
        unsafe { leaves::jett_rt_v1_resource_entry_network(ptr, 0, &mut capability, &mut result) },
        JettRuntimeStatusV1::OK
    );
    assert_eq!((scope, capability), (root.raw(), network));
    auth.with_resource(|s, v, r| {
        assert_eq!(s.custody.active_frames(), 2);
        s.complete_entry(v, r, attempt, 0, false)?;
        Ok(())
    })
    .unwrap();
    drop(auth);
    assert_eq!(
        unsafe { jett_rt_v1_context_destroy(ptr, &mut result) },
        JettRuntimeStatusV1::OK
    );
}

#[test]
fn native_resource_source_return_cleanup_panic_precedes_nonzero_observed_body_status() {
    context(|auth| {
        let f = fixture(4, false, false);
        let (attempt, _, op, network) = setup(auth, &f, &[(3, 501, 0, "")]);
        let owner = acquire(auth, op, network);
        auth.with_resource(|s, v, r| {
            let call = s.source_prepare(v, op, f.source)?;
            s.source_actual(v, r, call, 0, 0, owner.raw())?;
            let scope = s.source_enter(v, r, call)?;
            let parameter = ResourceHandleId::new(s.source_parameter(v, scope, 0)?)?;
            let inner = s.begin_operation_frame(v, 4, scope)?;
            let dest = s.destination_frame(inner, f.transfer_return)?;
            let provisional = s.transfer(v, r, f.transfer_return, inner, parameter, dest)?;
            s.end_operation_frame(v, r, inner)?;
            assert_eq!(
                s.scope_complete(v, r, scope, f.complete_scope, 71),
                Err(NativeResourceError::Cleanup(
                    ResourceCleanupFailure::FinalizerPanic
                ))
            );
            assert!(!s.handles.contains_key(&provisional));
            assert_eq!(s.counts(v, r).provisional, 0);
            assert!(
                s.return_publish(v, r, scope, f.publish.unwrap(), provisional)
                    .is_err()
            );
            s.end_operation_frame(v, r, op)?;
            let completion = s.complete_entry(v, r, attempt, 71, false)?;
            assert_eq!((completion.body_status, completion.selected_kind), (71, 3));
            let counts = s.counts(v, r);
            assert_eq!(
                (
                    counts.owners,
                    counts.loans,
                    counts.frames,
                    counts.registry,
                    counts.provisional
                ),
                (0, 0, 0, 0, 0)
            );
            Ok(())
        })
        .unwrap();
    });
}

#[path = "entry_outcome_tests.rs"]
mod entry_outcome_tests;

#[test]
fn native_resource_source_clean_body_cleanup_panic_has_exact_completed_failure_status() {
    context(|auth| {
        let f = fixture(4, false, false);
        let (attempt, _, op, network) = setup(auth, &f, &[(3, 501, 0, "")]);
        let owner = acquire(auth, op, network);
        auth.with_resource(|s, v, r| {
            let call = s.source_prepare(v, op, f.source)?;
            s.source_actual(v, r, call, 0, 0, owner.raw())?;
            let scope = s.source_enter(v, r, call)?;
            let parameter = ResourceHandleId::new(s.source_parameter(v, scope, 0)?)?;
            let cleanup = NativeResourceError::Cleanup(ResourceCleanupFailure::FinalizerPanic);
            assert_eq!(
                s.scope_complete(v, r, scope, f.complete_scope, 0),
                Err(cleanup)
            );
            assert_eq!(s.source_completed_body_status(call)?, Some(0));
            assert_eq!(s.source_status(v, call), Err(cleanup));
            assert_eq!(cleanup.status(), JettRuntimeStatusV1::PANIC);
            assert!(!s.handles.contains_key(&scope));
            assert!(!s.handles.contains_key(&parameter));
            assert!(
                s.return_publish(v, r, scope, f.publish.unwrap(), parameter)
                    .is_err()
            );
            s.end_operation_frame(v, r, op)?;
            let completion = s.complete_entry(v, r, attempt, 255, false)?;
            assert_eq!(
                (
                    completion.body_status,
                    completion.cleanup_status,
                    completion.selected_kind
                ),
                (255, 255, 3)
            );
            let counts = s.counts(v, r);
            assert_eq!(
                (
                    counts.owners,
                    counts.loans,
                    counts.frames,
                    counts.registry,
                    counts.provisional
                ),
                (0, 0, 0, 0, 0)
            );
            s.observe_events(|events| {
                assert_eq!(
                    events.iter().map(|event| event.kind).collect::<Vec<_>>(),
                    [1, 5]
                )
            });
            Ok(())
        })
        .unwrap();
    });
}

#[test]
fn native_resource_source_unfinished_and_merely_retired_frames_have_no_completion_receipt() {
    context(|auth| {
        let f = fixture(4, false, false);
        let (attempt, root, op, network) = setup(auth, &f, &[(1, 501, 0, "")]);
        let owner = acquire(auth, op, network);
        auth.with_resource(|s, v, r| {
            assert_eq!(
                s.source_prepare(v, op, f.close),
                Err(NativeResourceError::UnsupportedSourceBoundary)
            );
            assert_eq!(
                s.source_status(v, root),
                Err(NativeResourceError::WrongFamily)
            );
            let call = s.source_prepare(v, op, f.source)?;
            assert_eq!(s.source_completed_body_status(call)?, None);
            assert_eq!(
                s.source_status(v, call),
                Err(NativeResourceError::BodyFailed)
            );
            assert_eq!(
                s.source_enter(v, r, call),
                Err(NativeResourceError::WrongOperation)
            );
            assert_eq!(
                s.source_status(v, call),
                Err(NativeResourceError::BodyFailed)
            );
            s.source_actual(v, r, call, 0, 0, owner.raw())?;
            let scope = s.source_enter(v, r, call)?;
            assert_eq!(s.source_completed_body_status(call)?, None);
            assert_eq!(
                s.source_status(v, call),
                Err(NativeResourceError::WrongFrame)
            );
            s.retire_frame(v, r, scope)?;
            let return_frame = *s.active_frames.last().unwrap();
            assert_ne!(return_frame, op);
            s.retire_frame(v, r, return_frame)?;
            // Physical retirement alone never mints a trusted Source completion.
            assert_eq!(s.source_completed_body_status(call)?, None);
            assert_eq!(
                s.source_status(v, call),
                Err(NativeResourceError::BodyFailed)
            );
            assert!(
                s.return_publish(v, r, scope, f.publish.unwrap(), owner)
                    .is_err()
            );
            s.end_operation_frame(v, r, op)?;
            assert_eq!(
                s.source_status(v, call),
                Err(NativeResourceError::WrongFamily)
            );
            Ok(())
        })
        .unwrap();
        finish(auth, attempt);
    });
}
