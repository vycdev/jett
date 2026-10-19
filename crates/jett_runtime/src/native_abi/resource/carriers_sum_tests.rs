use super::*;
use crate::resource_custody::{
    NativeCarrierChildSource, NativeCarrierOperation as CarrierOp, NativeCarrierPath as Path,
    NativeCarrierSelector as Selector, NativeCarrierSource,
};

fn carrier_op(state: &NativeResourceState, select: impl Fn(&CarrierOp) -> bool) -> u32 {
    state
        .layout
        .operations()
        .iter()
        .enumerate()
        .find_map(|(ordinal, row)| {
            let NativeOperation::Carrier { record } = row.operation() else {
                return None;
            };
            select(&state.layout.carriers().operations[usize::try_from(*record).unwrap()].operation)
                .then(|| u32::try_from(ordinal).unwrap())
        })
        .expect("fixture carrier operation")
}

fn legacy_op(state: &NativeResourceState, select: impl Fn(&NativeOperation) -> bool) -> u32 {
    state
        .layout
        .operations()
        .iter()
        .enumerate()
        .find_map(|(ordinal, row)| select(row.operation()).then(|| u32::try_from(ordinal).unwrap()))
        .expect("fixture legacy operation")
}

fn entry() -> NativeEntry {
    NativeEntry {
        function: 0,
        signature: 0,
        scope: 0,
    }
}

fn bridge_context(operation: impl FnOnce(&AuthenticatedResourceContext)) {
    super::super::tests::context(|auth| {
        install_disabled(
            auth,
            &crate::resource_custody::carrier_runtime_fixture_bytes(),
            entry(),
        )
        .unwrap();
        operation(auth);
    });
}

fn start_record(
    state: &mut NativeResourceState,
    ordinary: &mut values::NativeValues,
    registry: &ResourceRegistry,
) -> ResourceResult<(ResourceHandleId, ResourceHandleId, ResourceHandleId)> {
    let attempt = state.begin_entry(ordinary, registry, entry(), ResourcePurpose::Runtime)?;
    let root = state.root_frame(attempt)?;
    let frame = state.begin_operation_frame(ordinary, 1, root)?;
    let none_op = carrier_op(state, |operation| {
        matches!(operation,
        CarrierOp::Construct { destination: 0, selector: 0, children, .. } if children.is_empty())
    });
    let none_builder = state.carrier_construct_begin(ordinary, frame, none_op)?;
    let none = state.carrier_commit(ordinary, none_builder)?;
    let record_op = carrier_op(state, |operation| {
        matches!(operation,
        CarrierOp::Construct { destination: 2, children, .. }
            if children.iter().any(|child| matches!(child.source, NativeCarrierChildSource::Move { slot: 0 })))
    });
    let builder = state.carrier_construct_begin(ordinary, frame, record_op)?;
    state.carrier_child(ordinary, builder, 0, 7)?;
    state.carrier_child(ordinary, builder, 1, none.raw())?;
    let record = state.carrier_commit(ordinary, builder)?;
    let transfer = carrier_op(state, |operation| {
        matches!(
            operation,
            CarrierOp::Transfer {
                source: 2,
                destination: 9,
                ..
            }
        )
    });
    let record = state.carrier_transfer(ordinary, frame, transfer, record)?;
    Ok((attempt, frame, record))
}

fn finish(
    state: &mut NativeResourceState,
    ordinary: &mut values::NativeValues,
    registry: &mut ResourceRegistry,
    attempt: ResourceHandleId,
    frame: ResourceHandleId,
) -> ResourceResult<()> {
    state.end_operation_frame(ordinary, registry, frame)?;
    assert!(state.carrier_adapters.is_empty());
    assert!(!state.handles.values().any(|entry| matches!(
        entry,
        NativeResourceEntry::CarrierRoot(_)
            | NativeResourceEntry::CarrierLoan(_)
            | NativeResourceEntry::CarrierBuilder(_)
            | NativeResourceEntry::Sum(_)
            | NativeResourceEntry::SumLoan(_)
    )));
    assert_eq!(
        state
            .complete_entry(ordinary, registry, attempt, 0, false)?
            .selected_kind,
        0
    );
    assert_eq!(
        state.counts(ordinary, registry),
        NativeResourceCounts {
            ordinary_empty: 1,
            ..NativeResourceCounts::default()
        }
    );
    Ok(())
}

#[test]
fn adapter_projection_rejects_a_different_same_shape_sibling() {
    assert!(!adapter_projection_matches(
        &[Path::Field(0)],
        &[Path::Field(1)]
    ));
    assert!(!adapter_projection_matches(
        &[Path::EnumPayload {
            variant: 0,
            field: 0
        }],
        &[Path::EnumPayload {
            variant: 1,
            field: 0
        }],
    ));
    assert!(!adapter_projection_matches(
        &[Path::MachinePayload { state: 0, field: 0 }],
        &[Path::MachinePayload { state: 1, field: 0 }],
    ));
}

#[test]
fn adapter_dynamic_selection_keeps_its_collection_projection_kind() {
    assert!(adapter_projection_matches(
        &[Path::Field(2), Path::MapValue(Selector::Dynamic)],
        &[Path::Field(2), Path::MapValue(Selector::Static(5))],
    ));
    assert!(!adapter_projection_matches(
        &[Path::Field(2), Path::MapValue(Selector::Dynamic)],
        &[Path::Field(2), Path::MapKey(Selector::Static(5))],
    ));
    assert!(!adapter_projection_matches(
        &[Path::List(Selector::Static(4))],
        &[Path::List(Selector::Static(5))],
    ));
    assert!(!adapter_projection_matches(
        &[Path::List(Selector::Dynamic)],
        &[Path::List(Selector::Static(5)), Path::Some],
    ));
}

#[test]
fn borrowed_adapter_survives_a_source_scope_and_pins_then_releases_the_exact_root() {
    bridge_context(|auth| {
        auth.with_resource(|state, ordinary, registry| {
            let (attempt, frame, record) = start_record(state, ordinary, registry)?;
            let adapt = carrier_op(state, |operation| {
                matches!(
                    operation,
                    CarrierOp::AdaptSum {
                        source: NativeCarrierSource::Slot(9),
                        borrowed: true,
                        ..
                    }
                )
            });
            let loan = state.carrier_adapt(ordinary, frame, adapt, record, 0)?;
            let Some(NativeResourceEntry::SumLoan(sum)) = state.handles.get(&loan) else {
                panic!("adapter sum loan");
            };
            let shell = sum.shell;
            let origin_loan = state.carrier_adapters[&shell].loan;
            assert!(matches!(state.handles.get(&origin_loan),
                Some(NativeResourceEntry::CarrierLoan(loan)) if loan.root == record));
            let reverse = carrier_op(state, |operation| {
                matches!(
                    operation,
                    CarrierOp::Transfer {
                        source: 9,
                        destination: 2,
                        ..
                    }
                )
            });
            assert_eq!(
                state.carrier_transfer(ordinary, frame, reverse, record),
                Err(NativeResourceError::Custody(CustodyError::ActiveBorrow))
            );

            let invoke = legacy_op(state, |operation| {
                matches!(
                    operation,
                    NativeOperation::InvokeSourceFunction { callee: 1, .. }
                )
            });
            let call = state.source_prepare(ordinary, frame, invoke)?;
            state.source_actual(ordinary, registry, call, 0, 0, loan.raw())?;
            let scope = state.source_enter(ordinary, registry, call)?;
            assert_eq!(state.source_parameter(ordinary, scope, 0)?, loan.raw());
            let tag = legacy_op(state, |operation| {
                matches!(
                    operation,
                    NativeOperation::ObserveSumView {
                        frame: 2,
                        source: NativeSumLoanSource::IncomingViewFormal {
                            scope: 2,
                            parameter: 0,
                        },
                    }
                )
            });
            assert_eq!(state.sum_view_tag(ordinary, registry, tag, scope, loan)?, 0);
            let complete = legacy_op(state, |operation| {
                matches!(operation, NativeOperation::Complete { frame: 2 })
            });
            state.scope_complete(ordinary, registry, scope, complete, 0)?;
            state.source_status(ordinary, call)?;

            let end = legacy_op(state, |operation| {
                matches!(operation,
                NativeOperation::EndSumBorrow { borrow, .. } if *borrow == adapt)
            });
            state.sum_borrow_end(registry, end, frame, loan)?;
            assert!(!state.handles.contains_key(&loan));
            assert!(!state.handles.contains_key(&origin_loan));
            assert!(state.carrier_adapters.contains_key(&shell));
            let before_refusals = state.counts(ordinary, registry);
            assert_eq!(before_refusals.loan_handles, 0);
            assert_eq!(before_refusals.owner_handles, 2);
            assert_eq!(before_refusals.provisional, 2);
            assert_eq!(before_refusals.frame_handles, 2);
            assert_eq!(before_refusals.registry, 0);
            assert!(matches!(state.handles.get(&shell),
                Some(NativeResourceEntry::Sum(sum))
                    if sum.frame == frame && sum.slot == 0 && sum.shape == 5
                        && matches!(sum.payload, NativeSumPayload::None)));
            // The seal denies the owning family even after its exact loan ends.
            assert_eq!(
                state.carrier(shell, 0, registry),
                Err(NativeResourceError::WrongFamily)
            );
            assert_eq!(
                state.sum_tag(shell),
                Err(NativeResourceError::WrongOperation)
            );
            assert_eq!(
                state.sum_borrow_end(registry, end, frame, loan),
                Err(NativeResourceError::WrongFamily)
            );
            assert_eq!(state.counts(ordinary, registry), before_refusals);
            assert!(state.carrier_adapters.contains_key(&shell));
            assert_eq!(
                state.carrier_root(ordinary, frame, record, 9)?.generation,
                record.raw()
            );
            state.carrier_transfer(ordinary, frame, reverse, record)?;
            finish(state, ordinary, registry, attempt, frame)
        })
        .unwrap();
    });
}

#[test]
fn owned_adapter_extraction_returns_to_a_carrier_once() {
    bridge_context(|auth| {
        auth.with_resource(|state, ordinary, registry| {
            let (attempt, frame, record) = start_record(state, ordinary, registry)?;
            let adapt = carrier_op(state, |operation| {
                matches!(
                    operation,
                    CarrierOp::AdaptSum {
                        source: NativeCarrierSource::Slot(9),
                        borrowed: false,
                        ..
                    }
                )
            });
            let shell = state.carrier_adapt(ordinary, frame, adapt, record, 0)?;
            assert_eq!(state.sum_tag(shell)?, 0);
            assert!(!state.carrier_adapters.contains_key(&shell));
            assert_eq!(
                state.carrier_adapt(ordinary, frame, adapt, record, 0),
                Err(NativeResourceError::WrongOperation)
            );
            let construct = carrier_op(state, |operation| {
                matches!(operation,
                CarrierOp::Construct { destination: 2, children, .. }
                    if children.iter().any(|child| matches!(child.source,
                        NativeCarrierChildSource::LeafSum { slot: 0 })))
            });
            let builder = state.carrier_construct_begin(ordinary, frame, construct)?;
            state.carrier_child(ordinary, builder, 0, 19)?;
            state.carrier_child(ordinary, builder, 1, shell.raw())?;
            let restored = state.carrier_commit(ordinary, builder)?;
            assert!(!state.handles.contains_key(&shell));
            assert_eq!(
                state.sum_tag(shell),
                Err(NativeResourceError::InvalidHandle)
            );
            state.carrier_root(ordinary, frame, restored, 2)?;
            finish(state, ordinary, registry, attempt, frame)
        })
        .unwrap();
    });
}

#[test]
fn borrowed_adapter_refuses_forged_projection_and_legacy_owned_child_adoption() {
    bridge_context(|auth| {
        auth.with_resource(|state, ordinary, registry| {
            let (attempt, frame, record) = start_record(state, ordinary, registry)?;
            let adapt = carrier_op(state, |operation| {
                matches!(
                    operation,
                    CarrierOp::AdaptSum {
                        source: NativeCarrierSource::Slot(9),
                        borrowed: true,
                        ..
                    }
                )
            });
            let loan = state.carrier_adapt(ordinary, frame, adapt, record, 0)?;
            let Some(NativeResourceEntry::SumLoan(sum)) = state.handles.get(&loan) else {
                panic!("adapter sum loan");
            };
            let shell = sum.shell;
            let origin_loan = state.carrier_adapters[&shell].loan;
            let Some(NativeResourceEntry::CarrierLoan(origin)) =
                state.handles.get_mut(&origin_loan)
            else {
                panic!("adapter carrier loan");
            };
            origin.path[0] = Path::Field(0);
            assert_eq!(
                state.carrier_adapter_validate(shell, loan),
                Err(NativeResourceError::WrongOperation)
            );
            assert!(state.handles.contains_key(&loan));
            assert!(state.handles.contains_key(&record));
            let Some(NativeResourceEntry::CarrierLoan(origin)) =
                state.handles.get_mut(&origin_loan)
            else {
                panic!("adapter carrier loan");
            };
            origin.path[0] = Path::Field(1);
            state.carrier_adapter_validate(shell, loan)?;
            let end = legacy_op(state, |operation| {
                matches!(operation,
                NativeOperation::EndSumBorrow { borrow, .. } if *borrow == adapt)
            });
            state.sum_borrow_end(registry, end, frame, loan)?;

            let construct = carrier_op(state, |operation| {
                matches!(operation,
                CarrierOp::Construct { destination: 2, children, .. }
                    if children.iter().any(|child| matches!(child.source,
                        NativeCarrierChildSource::LeafSum { slot: 0 })))
            });
            let builder = state.carrier_construct_begin(ordinary, frame, construct)?;
            state.carrier_child(ordinary, builder, 0, 23)?;
            assert_eq!(
                state.carrier_child(ordinary, builder, 1, shell.raw()),
                Err(NativeResourceError::WrongFamily)
            );
            assert!(state.carrier_adapters.contains_key(&shell));
            assert!(state.handles.contains_key(&shell));
            assert!(state.handles.contains_key(&record));
            finish(state, ordinary, registry, attempt, frame)
        })
        .unwrap();
    });
}

#[test]
fn nested_adapter_keeps_each_parent_lease_until_its_child_ends() {
    bridge_context(|auth| {
        auth.with_resource(|state, ordinary, registry| {
            let (attempt, frame, record) = start_record(state, ordinary, registry)?;
            let full_op = carrier_op(state, |operation| matches!(operation,
                CarrierOp::Borrow { source: NativeCarrierSource::Slot(9), path, .. }
                    if path.is_empty()));
            let full = state.carrier_borrow(ordinary, frame, full_op, record, 0)?;
            let child_op = carrier_op(state, |operation| matches!(operation,
                CarrierOp::Borrow {
                    source: NativeCarrierSource::Loan(NativeCarrierLoanSource::ExistingBorrow { operation: parent }),
                    path, ..
                } if *parent == full_op && path.as_slice() == &[Path::Field(1)]));
            let child = state.carrier_borrow(ordinary, frame, child_op, full, 0)?;
            let adapt = carrier_op(state, |operation| matches!(operation,
                CarrierOp::AdaptSum {
                    source: NativeCarrierSource::Loan(NativeCarrierLoanSource::ExistingBorrow { operation: parent }),
                    path, borrowed: true, ..
                } if *parent == child_op && path.is_empty()));
            let loan = state.carrier_adapt(ordinary, frame, adapt, child, 0)?;
            let child_end = carrier_op(state, |operation| matches!(operation,
                CarrierOp::EndBorrow { borrow, .. } if *borrow == child_op));
            let full_end = carrier_op(state, |operation| matches!(operation,
                CarrierOp::EndBorrow { borrow, .. } if *borrow == full_op));
            assert_eq!(state.carrier_end(frame, child_end, child),
                Err(NativeResourceError::Custody(CustodyError::ActiveBorrow)));
            assert_eq!(state.carrier_end(frame, full_end, full),
                Err(NativeResourceError::Custody(CustodyError::ActiveBorrow)));
            let tag = legacy_op(state, |operation| matches!(operation,
                NativeOperation::ObserveSumView {
                    source: NativeSumLoanSource::CarrierProjected { operation: origin }, ..
                } if *origin == adapt));
            assert_eq!(state.sum_view_tag(ordinary, registry, tag, frame, loan)?, 0);
            let end = legacy_op(state, |operation| matches!(operation,
                NativeOperation::EndSumBorrow { borrow, .. } if *borrow == adapt));
            state.sum_borrow_end(registry, end, frame, loan)?;
            state.carrier_end(frame, child_end, child)?;
            state.carrier_end(frame, full_end, full)?;
            let reverse = carrier_op(state, |operation| matches!(operation,
                CarrierOp::Transfer { source: 9, destination: 2, .. }));
            state.carrier_transfer(ordinary, frame, reverse, record)?;
            finish(state, ordinary, registry, attempt, frame)
        }).unwrap();
    });
}
