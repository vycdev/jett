//! Native protocol controls. Genuine Source acceptance is a separate driver gate.
use super::super::*;
use super::{assert_empty, context, events, script, words};

struct Fixture {
    bytes: Vec<u8>,
    result: bool,
    create: u32,
    source_written: u32,
    source_bare: u32,
    observe: u32,
    project: u32,
    read: Option<u32>,
    end_child: u32,
    invoke: u32,
    complete_inner: u32,
    complete_scope: u32,
    complete_root: u32,
    take: u32,
    drop_written: u32,
    drop_bare: u32,
    take_failure: Option<u32>,
    caller_project: u32,
    caller_end: u32,
    close: u32,
}
fn fixture(result: bool, corrupt: u8) -> Fixture {
    let shape = if result { 6 } else { 5 };
    let path = if result { 2 } else { 1 };
    let mut signatures = vec![
        vec![0, 2, 6, 0, 2, 1, 1],
        vec![1, 2, 7, 0, 2, 4, 2],
        vec![2, 1, 3, 4, 1],
        vec![3, 1, 3, 0, 2],
        vec![4, 2, 3, 0, 2, shape, 2],
    ];
    if corrupt == 1 {
        *signatures[4].last_mut().unwrap() = 1;
    }
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
    let frames = vec![
        vec![0, 1, 0, 0, 0, 0, 3, 1, 0, 0],
        vec![1, 2, 0, 0, 0, 1, 3, 1, 1, 0],
        vec![2, 1, 1, 0, 0, 2, 4, 1, 1, 1],
        vec![3, 2, 1, 0, 0, 3, 4, 1, 1, 2],
    ];
    let slots = vec![
        vec![0, 0, shape, 1, path],
        vec![1, 1, shape, 1, path],
        vec![2, 1, 6, 1, 2],
        vec![3, 1, 4, 0],
    ];
    let mut ops: Vec<(u32, Vec<u32>)> = Vec::new();
    let mut add = |function, row| {
        let id = ops.len() as u32;
        ops.push((function, row));
        id
    };
    add(0, vec![1, 1, 0, 2]); // 0 factory
    add(0, vec![10, 1, 2, 3]); // 1 unwrap owning factory result
    add(0, vec![9, 1, 3, 0]); // 2 adopt owner into caller sum
    add(0, vec![2, 1, 0, 1]); // 3 whole bare argument holder transfer
    let create = add(
        0,
        if result {
            vec![20, 1, 1, 2]
        } else {
            vec![19, 1, 1]
        },
    );
    add(0, vec![2, 1, 1, 0]); // 5 preserve initial None/Fail in caller Scope
    add(0, vec![13, 1]); // 6 complete operation
    add(0, vec![21, 1, 0, 1]); // 7 written shell lease
    add(0, vec![21, 1, 1, 1]); // 8 bare argument holder lease
    add(0, vec![25, 1, 7]); // 9 end written shell lease
    add(0, vec![25, 1, 8]); // 10 end bare shell lease
    let source_row = |bare| {
        let mut row = vec![16, 1, 1, 4, 2, 0, 2, 0, 1, 2];
        row.extend([0, 0, 0, 0, 2, 4, 2, 1]);
        row.extend([
            1,
            1,
            shape,
            shape,
            if bare { 1 } else { 2 },
            if bare { 3 } else { 4 },
            2,
            4,
            1,
            if bare {
                if corrupt == 6 { 7 } else { 8 }
            } else {
                7
            },
        ]);
        row.extend([1, 3]);
        row
    };
    let source_written = add(0, source_row(false));
    let source_bare = add(0, source_row(true));
    let observe = add(1, vec![22, 2, 2, 2, if corrupt == 2 { 0 } else { 1 }]);
    let project = add(
        1,
        vec![
            23,
            2,
            2,
            2,
            1,
            if corrupt == 3 {
                if path == 1 { 2 } else { 1 }
            } else {
                path
            },
            2,
        ],
    );
    let end_child = add(1, vec![5, 2, project]);
    let read = result.then(|| add(1, vec![24, 2, 2, 2, 1, if corrupt == 4 { 1 } else { 2 }]));
    let invoke = add(
        if corrupt == 7 { 0 } else { 1 },
        vec![
            6,
            if corrupt == 7 { 1 } else { 3 },
            1,
            3,
            if corrupt == 5 { 7 } else { project },
        ],
    );
    let complete_inner = add(1, vec![13, 3]);
    let complete_scope = add(1, vec![13, 2]);
    let complete_root = add(0, vec![13, 0]);
    let take = add(0, vec![10, 1, 0, 3]);
    let drop_written = add(0, vec![11, 1, 0]);
    let drop_bare = add(0, vec![11, 1, 1]);
    let take_failure = result.then(|| add(0, vec![17, 1, 0, 2]));
    let caller_project = add(0, vec![23, 1, 1, 7, path, 1]);
    let caller_end = add(0, vec![5, 1, caller_project]);
    let close = add(0, vec![7, 1, 2, 3]);
    let mut bytes = b"JTRSC001".to_vec();
    words(&mut bytes, &[2, 0]);
    bytes.extend(0u64.to_le_bytes());
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
    for (index, (function, row)) in ops.into_iter().enumerate() {
        words(
            &mut bytes,
            &[index as u32, function, 0, 0, 100 + index as u32],
        );
        words(&mut bytes, &row);
    }
    let len = bytes.len() as u64;
    bytes[16..24].copy_from_slice(&len.to_le_bytes());
    Fixture {
        bytes,
        result,
        create,
        source_written,
        source_bare,
        observe,
        project,
        read,
        end_child,
        invoke,
        complete_inner,
        complete_scope,
        complete_root,
        take,
        drop_written,
        drop_bare,
        take_failure,
        caller_project,
        caller_end,
        close,
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
    occupied: bool,
    rows: &[(u32, i64, i64, &str)],
) -> (
    ResourceHandleId,
    ResourceHandleId,
    ResourceHandleId,
    ResourceHandleId,
    u64,
) {
    install_scripted(auth, &f.bytes, entry(), script(rows, 1)).unwrap();
    auth.with_resource(|s, v, r| {
        let attempt = s.begin_entry(v, r, entry(), ResourcePurpose::Runtime)?;
        let root = s.root_frame(attempt)?;
        let op = s.begin_operation_frame(v, 1, root)?;
        let net = s.entry_network(attempt, 0)?;
        let shell = if occupied {
            let call = s.prepare_hook(v, 0, op, None)?;
            let prepared = s.prepare_factory(v, r, call, net)?;
            let value = s.factory(v, r, prepared, net, 801)?;
            let owner = s.sum_take(v, r, 1, op, ResourceHandleId::new(value.value)?, op)?;
            s.sum_adopt(v, r, 2, op, owner, root)?
        } else {
            let shell = if f.result {
                let mut ids = v
                    .prepare_resource_ordinary_output()
                    .map_err(ordinary_error)?;
                let text = v
                    .publish_resource_error("empty".to_owned(), &mut ids)
                    .map_err(ordinary_error)?;
                s.failure_sum(v, op, f.create, text)?
            } else {
                s.absent_sum(v, op, f.create)?
            };
            s.transfer(v, r, 5, op, shell, root)?
        };
        Ok((attempt, root, op, shell, net))
    })
    .unwrap()
}
fn finish(
    auth: &AuthenticatedResourceContext,
    attempt: ResourceHandleId,
    root: ResourceHandleId,
    op: ResourceHandleId,
    f: &Fixture,
) {
    auth.with_resource(|s, v, r| {
        s.end_operation_frame(v, r, op)?;
        s.scope_complete(v, r, root, f.complete_root, 0)?;
        let outcome = s.complete_entry(v, r, attempt, 0, false)?;
        assert_eq!(
            (
                outcome.body_status,
                outcome.cleanup_status,
                outcome.selected_kind
            ),
            (0, 0, 0)
        );
        Ok(())
    })
    .unwrap();
    assert_empty(auth);
}

#[test]
fn native_resource_borrowed_sum_absent_and_failure_views_keep_shell_and_copy_failure() {
    for result in [false, true] {
        context(|auth| {
            let f = fixture(result, 0);
            let (attempt, root, op, shell, net) = setup(auth, &f, false, &[]);
            auth.with_resource(|s, v, r| {
                let parent = s.sum_borrow(v, r, 7, op, shell)?;
                assert_eq!(s.custody.live_owners(), 0);
                assert_eq!(s.custody.live_loans(), 0);
                assert_eq!(s.counts(v, r).loan_handles, 1);
                assert_eq!(
                    s.transfer(v, r, 3, op, shell, op),
                    Err(CustodyError::ActiveBorrow.into())
                );
                assert_eq!(
                    s.sum_drop(v, r, f.drop_written, op, shell),
                    Err(CustodyError::ActiveBorrow.into())
                );
                let call = s.source_prepare(v, op, f.source_written)?;
                s.source_actual(v, r, call, 0, 0, net)?;
                s.source_actual(v, r, call, 1, 1, parent.raw())?;
                let scope = s.source_enter(v, r, call)?;
                assert_eq!(s.source_parameter(v, scope, 1)?, parent.raw());
                assert_eq!(s.sum_view_tag(v, r, f.observe, scope, parent)?, 0);
                assert!(s.sum_view_project(v, r, f.project, scope, parent).is_err());
                if let Some(read) = f.read {
                    for _ in 0..2 {
                        let copy = s.sum_failure_read(v, r, read, scope, parent)?;
                        v.drop_resource_typed_companion(&s.layout, 2, copy)
                            .map_err(ordinary_error)?;
                        assert!(s.handles.contains_key(&shell));
                    }
                }
                s.scope_complete(v, r, scope, f.complete_scope, 0)?;
                s.source_status(v, call)?;
                s.sum_borrow_end(r, 9, op, parent)?;
                s.carrier(shell, 0, r)?;
                if let Some(take) = f.take_failure {
                    let text = s.failure_companion_take(v, op, take, shell)?;
                    v.drop_resource_typed_companion(&s.layout, 2, text)
                        .map_err(ordinary_error)?;
                } else {
                    s.sum_drop(v, r, f.drop_written, op, shell)?;
                }
                Ok(())
            })
            .unwrap();
            finish(auth, attempt, root, op, &f);
            assert!(events(auth).is_empty());
        });
    }
}

#[test]
fn native_resource_borrowed_sum_occupied_written_and_bare_keep_one_owner_across_hook_outcomes() {
    for result in [false, true] {
        for bare in [false, true] {
            for failed in [false, true] {
                context(|auth| {
                    let f = fixture(result, 0);
                    let rows = if failed {
                        vec![(1, 801, 0, ""), (5, 801, 0, "borrow failed")]
                    } else {
                        vec![(1, 801, 0, ""), (4, 801, 17, "")]
                    };
                    let (attempt, root, op, original, net) = setup(auth, &f, true, &rows);
                    auth.with_resource(|s, v, r| {
                        let shell = if bare {
                            s.transfer(v, r, 3, op, original, op)?
                        } else {
                            original
                        };
                        let parent = s.sum_borrow(v, r, if bare { 8 } else { 7 }, op, shell)?;
                        let call = s.source_prepare(
                            v,
                            op,
                            if bare {
                                f.source_bare
                            } else {
                                f.source_written
                            },
                        )?;
                        s.source_actual(v, r, call, 0, 0, net)?;
                        s.source_actual(v, r, call, 1, 1, parent.raw())?;
                        let scope = s.source_enter(v, r, call)?;
                        assert_eq!(s.custody.live_owners(), 1);
                        assert_eq!(s.sum_view_tag(v, r, f.observe, scope, parent)?, 1);
                        let child = s.sum_view_project(v, r, f.project, scope, parent)?;
                        let inner = s.begin_operation_frame(v, 3, scope)?;
                        let hook = s.prepare_hook(v, f.invoke, inner, None)?;
                        let output = s.invoke_borrow(v, r, hook, net, child)?;
                        v.drop_resource_typed_companion(&s.layout, 7, output)
                            .map_err(ordinary_error)?;
                        s.end_operation_frame(v, r, inner)?;
                        s.borrow_end(f.end_child, scope, child)?;
                        s.scope_complete(v, r, scope, f.complete_scope, 0)?;
                        s.source_status(v, call)?;
                        s.sum_borrow_end(r, if bare { 10 } else { 9 }, op, parent)?;
                        if bare {
                            assert!(!s.handles.contains_key(&original));
                            s.sum_drop(v, r, f.drop_bare, op, shell)?;
                        } else {
                            s.carrier(original, 0, r)?;
                            let owner = s.sum_take(v, r, f.take, op, original, op)?;
                            s.close_or_drop(r, f.close, op, owner)?;
                        }
                        Ok(())
                    })
                    .unwrap();
                    finish(auth, attempt, root, op, &f);
                    assert_eq!(
                        events(auth),
                        vec![(801, 1), (801, if failed { 4 } else { 3 }), (801, 5)]
                    );
                });
            }
        }
    }
}

#[test]
fn native_resource_borrowed_sum_child_prevents_parent_end_and_owning_adoption_until_retired() {
    for result in [false, true] {
        context(|auth| {
            let f = fixture(result, 0);
            let (attempt, root, op, shell, _) = setup(auth, &f, true, &[(1, 801, 0, "")]);
            auth.with_resource(|s, v, r| {
                let parent = s.sum_borrow(v, r, 7, op, shell)?;
                let child = s.sum_view_project(v, r, f.caller_project, op, parent)?;
                assert_eq!(
                    s.sum_borrow_end(r, 9, op, parent),
                    Err(CustodyError::ActiveBorrow.into())
                );
                assert_eq!(
                    s.sum_take(v, r, f.take, op, shell, op),
                    Err(CustodyError::ActiveBorrow.into())
                );
                assert_eq!(
                    s.sum_drop(v, r, f.drop_written, op, shell),
                    Err(CustodyError::ActiveBorrow.into())
                );
                assert_eq!(s.custody.live_owners(), 1);
                assert_eq!(s.custody.live_loans(), 2);
                assert!(
                    s.exact_loan(
                        child,
                        NativeLoanSource::ExistingBorrow {
                            operation: f.caller_project
                        }
                    )
                    .is_err()
                );
                s.borrow_end(f.caller_end, op, child)?;
                s.sum_borrow_end(r, 9, op, parent)?;
                assert!(s.sum_borrow_end(r, 9, op, parent).is_err());
                assert!(
                    s.exact_loan(
                        child,
                        NativeLoanSource::ProjectedSumPayload {
                            operation: f.caller_project
                        }
                    )
                    .is_err()
                );
                let owner = s.sum_take(v, r, f.take, op, shell, op)?;
                s.close_or_drop(r, f.close, op, owner)?;
                Ok(())
            })
            .unwrap();
            finish(auth, attempt, root, op, &f);
            assert_eq!(events(auth), vec![(801, 1), (801, 5)]);
        });
    }
}

#[test]
fn native_resource_borrowed_sum_abandoned_prefix_retires_leases_before_holder_cleanup() {
    for occupied in [false, true] {
        for result in [false, true] {
            context(|auth| {
                let f = fixture(result, 0);
                let rows = if occupied {
                    vec![(1, 801, 0, "")]
                } else {
                    vec![]
                };
                let (attempt, root, op, original, net) = setup(auth, &f, occupied, &rows);
                auth.with_resource(|s, v, r| {
                    let shell = s.transfer(v, r, 3, op, original, op)?;
                    let parent = s.sum_borrow(v, r, 8, op, shell)?;
                    let call = s.source_prepare(v, op, f.source_bare)?;
                    s.source_actual(v, r, call, 0, 0, net)?;
                    s.source_actual(v, r, call, 1, 1, parent.raw())?;
                    // No SourceEnter or provider invocation: suffix retirement handles the prefix.
                    s.end_operation_frame(v, r, op)?;
                    assert_eq!(s.counts(v, r).loan_handles, 0);
                    assert_eq!(s.custody.live_owners(), 0);
                    s.scope_complete(v, r, root, f.complete_root, 0)?;
                    assert_eq!(s.complete_entry(v, r, attempt, 0, false)?.selected_kind, 0);
                    Ok(())
                })
                .unwrap();
                assert_empty(auth);
                assert_eq!(
                    events(auth),
                    if occupied {
                        vec![(801, 1), (801, 5)]
                    } else {
                        vec![]
                    }
                );
            });
        }
    }
}

#[test]
fn native_resource_borrowed_sum_registration_rejects_wrong_formal_path_companion_and_issuer() {
    for corrupt in 1..=7 {
        context(|auth| {
            let f = fixture(true, corrupt);
            assert!(
                install_disabled(auth, &f.bytes, entry()).is_err(),
                "corruption {corrupt}"
            );
            auth.with_state(|_, state| {
                assert!(state.resource_state.is_none());
                Ok(())
            })
            .unwrap();
        });
    }
    context(|auth| {
        let mut f = fixture(false, 0);
        f.bytes[8..12].copy_from_slice(&1u32.to_le_bytes());
        assert!(install_disabled(auth, &f.bytes, entry()).is_err());
    });
}

#[test]
fn native_resource_borrowed_sum_leaf_output_preflight_is_before_lookup_or_effects() {
    let mut ctx = Box::new(JettRuntimeContextV1::retired());
    let mut result = JettRuntimeResultV1::ok();
    assert_eq!(
        unsafe { jett_rt_v1_context_create(JETT_RUNTIME_ABI_VERSION_V1, &mut *ctx, &mut result) },
        JettRuntimeStatusV1::OK
    );
    let pointer = &*ctx as *const JettRuntimeContextV1;
    let mut sentinel = 0x5555u64;
    unsafe {
        assert_eq!(
            leaves::jett_rt_v1_resource_sum_borrow(pointer, 0, 0, 0, std::ptr::null_mut()),
            JettRuntimeStatusV1::INVALID_ARGUMENT
        );
        assert_eq!(
            leaves::jett_rt_v1_resource_sum_view_tag(pointer, 0, 0, 0, pointer.cast_mut().cast()),
            JettRuntimeStatusV1::INVALID_ARGUMENT
        );
        assert_eq!(
            leaves::jett_rt_v1_resource_sum_view_project(
                pointer,
                0,
                0,
                0,
                (&mut sentinel as *mut u64).cast::<u8>().add(1).cast()
            ),
            JettRuntimeStatusV1::INVALID_ARGUMENT
        );
        assert_eq!(
            leaves::jett_rt_v1_resource_sum_failure_read(pointer, 0, 0, 0, std::ptr::null_mut()),
            JettRuntimeStatusV1::INVALID_ARGUMENT
        );
    }
    assert_eq!(sentinel, 0x5555);
    let auth = AuthenticatedResourceContext::acquire(pointer).unwrap();
    auth.with_state(|_, state| {
        assert!(state.resource_state.is_none());
        Ok(())
    })
    .unwrap();
    drop(auth);
    assert_eq!(
        unsafe { jett_rt_v1_context_destroy(&mut *ctx, &mut result) },
        JettRuntimeStatusV1::OK
    );
}
