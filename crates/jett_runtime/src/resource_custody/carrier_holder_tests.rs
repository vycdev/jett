use super::*;
use crate::resource_custody::carrier_holders::{CarrierRootToken, RegisteredCarrierSlot};

// Deliberate constructor-owned test metadata; no public path/word issues a token.
fn carrier_slot(
    program: &RegisteredResourceProgram,
    index: usize,
    wrong_second: bool,
) -> RegisteredCarrierSlot {
    let field = |name: &[u8], node| NativeCarrierField {
        name: name.to_vec(),
        node,
    };
    RegisteredCarrierSlot {
        program: program.0.clone(),
        graph: Arc::new(NativeCarrierLayout {
            nodes: vec![
                NativeCarrierNode::Resource { kind: 0 },
                NativeCarrierNode::Ordinary { shape: 0 },
                NativeCarrierNode::Struct {
                    identity: b"app.OwnedPair".to_vec(),
                    arguments: vec![],
                    fields: vec![
                        field(b"first", 0),
                        field(b"second", if wrong_second { 3 } else { 0 }),
                        field(b"marker", 1),
                    ],
                },
                NativeCarrierNode::Resource { kind: 1 },
            ],
            slots: (0..4)
                .map(|_| NativeCarrierSlot { frame: 0, node: 2 })
                .collect(),
            operations: vec![],
        }),
        index,
    }
}

fn begin_root(
    core: &mut ResourceCustody,
    program: &RegisteredResourceProgram,
    frame: &ResourceFrameToken,
    slot: usize,
) -> CarrierRootToken {
    core.begin_carrier_root(frame, &carrier_slot(program, slot, false))
        .unwrap()
}

#[test]
fn carrier_holders_interleave_scalar_carrier_scalar_and_release_original_slot() {
    let mut registry = ResourceRegistry::new();
    let program = test_program(&registry);
    let events = Arc::new(Mutex::new(vec![]));
    let mut core = ResourceCustody::new(program.clone(), &registry).unwrap();
    let frame = core
        .begin_frame(ResourceFrameKind::Scope, ResourcePurpose::Runtime)
        .unwrap();
    let _a = acquire(
        &mut core,
        &mut registry,
        &program,
        &frame,
        0,
        1,
        &events,
        false,
    );
    let mut b = acquire(
        &mut core,
        &mut registry,
        &program,
        &frame,
        1,
        2,
        &events,
        false,
    );
    let root = begin_root(&mut core, &program, &frame, 0);
    core.transfer_into_carrier(&mut b, &root, &[NativeCarrierPath::Field(0)], &registry)
        .unwrap();
    core.complete_carrier_root(&root).unwrap();
    let _c = acquire(
        &mut core,
        &mut registry,
        &program,
        &frame,
        3,
        3,
        &events,
        false,
    );
    assert_eq!(core.end_frame(&frame, &mut registry).failure(), None);
    assert_eq!(*events.lock().unwrap(), [3, 2, 1]);
    core.retire_carrier_metadata(&root).unwrap();
    clean(&core, &registry);
    assert_eq!(core.live_carrier_roots(), 0);

    let frame = core
        .begin_frame(ResourceFrameKind::Scope, ResourcePurpose::Runtime)
        .unwrap();
    let mut first = acquire(
        &mut core,
        &mut registry,
        &program,
        &frame,
        1,
        4,
        &events,
        false,
    );
    let root = begin_root(&mut core, &program, &frame, 0);
    core.transfer_into_carrier(&mut first, &root, &[NativeCarrierPath::Field(0)], &registry)
        .unwrap();
    // The occupied tree remains live while its old scalar slot is reused.
    let _replacement = acquire(
        &mut core,
        &mut registry,
        &program,
        &frame,
        1,
        5,
        &events,
        false,
    );
    assert_eq!(core.live_owners(), 2);
    assert_eq!(core.end_frame(&frame, &mut registry).failure(), None);
    assert_eq!(*events.lock().unwrap(), [3, 2, 1, 5, 4]);
    core.retire_carrier_metadata(&root).unwrap();
    clean(&core, &registry);
    assert_eq!(core.live_carrier_roots(), 0);
}

#[test]
fn carrier_holders_caller_callee_move_is_exact_and_invalidates_old_generation() {
    let mut registry = ResourceRegistry::new();
    let program = test_program(&registry);
    let events = Arc::new(Mutex::new(vec![]));
    let mut core = ResourceCustody::new(program.clone(), &registry).unwrap();
    let caller = core
        .begin_frame(ResourceFrameKind::Scope, ResourcePurpose::Runtime)
        .unwrap();
    let mut first = acquire(
        &mut core,
        &mut registry,
        &program,
        &caller,
        0,
        10,
        &events,
        false,
    );
    let mut second = acquire(
        &mut core,
        &mut registry,
        &program,
        &caller,
        1,
        11,
        &events,
        false,
    );
    let source = begin_root(&mut core, &program, &caller, 0);
    // Constructor lexical order differs from declared field order.
    core.transfer_into_carrier(
        &mut second,
        &source,
        &[NativeCarrierPath::Field(1)],
        &registry,
    )
    .unwrap();
    core.transfer_into_carrier(
        &mut first,
        &source,
        &[NativeCarrierPath::Field(0)],
        &registry,
    )
    .unwrap();
    core.complete_carrier_root(&source).unwrap();
    let stale = OwnedResourceToken {
        core: first.core,
        owner: first.owner,
        holder_generation: first.holder_generation,
    };
    let callee = core
        .begin_frame(ResourceFrameKind::Scope, ResourcePurpose::Runtime)
        .unwrap();
    let destination = begin_root(&mut core, &program, &callee, 1);
    core.move_carrier_inventory(
        &source,
        &destination,
        &mut [
            (
                &mut second,
                vec![NativeCarrierPath::Field(1)],
                vec![NativeCarrierPath::Field(1)],
            ),
            (
                &mut first,
                vec![NativeCarrierPath::Field(0)],
                vec![NativeCarrierPath::Field(0)],
            ),
        ],
        &registry,
        true,
    )
    .unwrap();
    assert!(matches!(
        core.validate_owned(&stale, &registry),
        Err(CustodyError::StaleHolderGeneration)
    ));
    assert!(matches!(
        core.validate_carrier_ticket(&source, &[NativeCarrierPath::Field(0)], &first),
        Err(CustodyError::StaleHolderGeneration)
    ));
    assert_eq!(core.end_frame(&callee, &mut registry).failure(), None);
    assert_eq!(*events.lock().unwrap(), [10, 11]);
    core.retire_carrier_metadata(&destination).unwrap();
    assert_eq!(core.end_frame(&caller, &mut registry).failure(), None);
    clean(&core, &registry);
    assert_eq!(core.live_carrier_roots(), 0);
}

#[test]
fn carrier_holders_late_batch_wrong_kind_rolls_back_and_can_retry() {
    let mut registry = ResourceRegistry::new();
    let program = test_program(&registry);
    let events = Arc::new(Mutex::new(vec![]));
    let mut core = ResourceCustody::new(program.clone(), &registry).unwrap();
    let frame = core
        .begin_frame(ResourceFrameKind::Scope, ResourcePurpose::Runtime)
        .unwrap();
    let mut first = acquire(
        &mut core,
        &mut registry,
        &program,
        &frame,
        0,
        20,
        &events,
        false,
    );
    let mut second = acquire(
        &mut core,
        &mut registry,
        &program,
        &frame,
        1,
        21,
        &events,
        false,
    );
    let source = begin_root(&mut core, &program, &frame, 0);
    core.transfer_into_carrier(
        &mut first,
        &source,
        &[NativeCarrierPath::Field(0)],
        &registry,
    )
    .unwrap();
    core.transfer_into_carrier(
        &mut second,
        &source,
        &[NativeCarrierPath::Field(1)],
        &registry,
    )
    .unwrap();
    core.complete_carrier_root(&source).unwrap();
    let before = (first.holder_generation, second.holder_generation);
    let wrong = core
        .begin_carrier_root(&frame, &carrier_slot(&program, 1, true))
        .unwrap();
    let result = core.move_carrier_inventory(
        &source,
        &wrong,
        &mut [
            (
                &mut first,
                vec![NativeCarrierPath::Field(0)],
                vec![NativeCarrierPath::Field(0)],
            ),
            (
                &mut second,
                vec![NativeCarrierPath::Field(1)],
                vec![NativeCarrierPath::Field(1)],
            ),
        ],
        &registry,
        true,
    );
    assert_eq!(result, Err(CustodyError::WrongKind));
    assert_eq!(before, (first.holder_generation, second.holder_generation));
    core.validate_carrier_inventory(
        &source,
        &[
            (&first, vec![NativeCarrierPath::Field(0)]),
            (&second, vec![NativeCarrierPath::Field(1)]),
        ],
    )
    .unwrap();
    assert!(events.lock().unwrap().is_empty());
    core.retire_carrier_metadata(&wrong).unwrap();
    let good = begin_root(&mut core, &program, &frame, 1);
    core.move_carrier_inventory(
        &source,
        &good,
        &mut [
            (
                &mut first,
                vec![NativeCarrierPath::Field(0)],
                vec![NativeCarrierPath::Field(0)],
            ),
            (
                &mut second,
                vec![NativeCarrierPath::Field(1)],
                vec![NativeCarrierPath::Field(1)],
            ),
        ],
        &registry,
        true,
    )
    .unwrap();
    assert_eq!(core.end_frame(&frame, &mut registry).failure(), None);
    assert_eq!(*events.lock().unwrap(), [21, 20]);
    core.retire_carrier_metadata(&good).unwrap();
    clean(&core, &registry);
    assert_eq!(core.live_carrier_roots(), 0);
}

#[test]
fn carrier_holders_wrong_path_root_generation_foreign_and_borrow_refuse_without_effects() {
    let mut registry = ResourceRegistry::new();
    let program = test_program(&registry);
    let events = Arc::new(Mutex::new(vec![]));
    let mut core = ResourceCustody::new(program.clone(), &registry).unwrap();
    let frame = core
        .begin_frame(ResourceFrameKind::Scope, ResourcePurpose::Runtime)
        .unwrap();
    let mut owner = acquire(
        &mut core,
        &mut registry,
        &program,
        &frame,
        0,
        30,
        &events,
        false,
    );
    let root = begin_root(&mut core, &program, &frame, 0);
    assert_eq!(
        core.transfer_into_carrier(&mut owner, &root, &[NativeCarrierPath::Field(2)], &registry),
        Err(CustodyError::WrongKind)
    );
    let stale = CarrierRootToken {
        core: root.core,
        index: root.index,
        generation: root.generation + 1,
    };
    assert_eq!(
        core.transfer_into_carrier(
            &mut owner,
            &stale,
            &[NativeCarrierPath::Field(0)],
            &registry
        ),
        Err(CustodyError::StaleHolderGeneration)
    );
    let foreign = CarrierRootToken {
        core: root.core + 1,
        index: root.index,
        generation: root.generation,
    };
    assert_eq!(
        core.transfer_into_carrier(
            &mut owner,
            &foreign,
            &[NativeCarrierPath::Field(0)],
            &registry
        ),
        Err(CustodyError::WrongProgram)
    );
    let mut loan = core.begin_borrow(&owner, &frame, &registry).unwrap();
    assert_eq!(
        core.transfer_into_carrier(&mut owner, &root, &[NativeCarrierPath::Field(0)], &registry),
        Err(CustodyError::ActiveBorrow)
    );
    core.end_borrow(&mut loan).unwrap();
    core.transfer_into_carrier(&mut owner, &root, &[NativeCarrierPath::Field(0)], &registry)
        .unwrap();
    let other = begin_root(&mut core, &program, &frame, 1);
    assert_eq!(
        core.validate_carrier_ticket(&other, &[NativeCarrierPath::Field(0)], &owner),
        Err(CustodyError::InvalidOwner)
    );
    assert_eq!(
        core.validate_carrier_inventory(&root, &[]),
        Err(CustodyError::InvalidOwner)
    );
    assert_eq!(
        core.validate_carrier_inventory(
            &root,
            &[
                (&owner, vec![NativeCarrierPath::Field(0)]),
                (&owner, vec![NativeCarrierPath::Field(0)])
            ]
        ),
        Err(CustodyError::OccupiedHolder)
    );
    assert!(events.lock().unwrap().is_empty());
    core.retire_carrier_metadata(&other).unwrap();
    assert_eq!(core.end_frame(&frame, &mut registry).failure(), None);
    core.retire_carrier_metadata(&root).unwrap();
    clean(&core, &registry);
    assert_eq!(core.live_carrier_roots(), 0);
}

#[test]
fn carrier_holders_partial_operation_panic_retires_once_and_continues_unified_order() {
    let mut registry = ResourceRegistry::new();
    let program = test_program(&registry);
    let events = Arc::new(Mutex::new(vec![]));
    let mut core = ResourceCustody::new(program.clone(), &registry).unwrap();
    let parent = core
        .begin_frame(ResourceFrameKind::Scope, ResourcePurpose::Runtime)
        .unwrap();
    let _parent = acquire(
        &mut core,
        &mut registry,
        &program,
        &parent,
        0,
        40,
        &events,
        false,
    );
    let operation = core
        .begin_frame(ResourceFrameKind::Operation, ResourcePurpose::Runtime)
        .unwrap();
    let mut first = acquire(
        &mut core,
        &mut registry,
        &program,
        &operation,
        0,
        41,
        &events,
        false,
    );
    let root = begin_root(&mut core, &program, &operation, 0);
    core.transfer_into_carrier(&mut first, &root, &[NativeCarrierPath::Field(0)], &registry)
        .unwrap();
    let _later_scalar = acquire(
        &mut core,
        &mut registry,
        &program,
        &operation,
        1,
        42,
        &events,
        true,
    );
    assert_eq!(
        core.end_frame(&operation, &mut registry).failure(),
        Some(ResourceCleanupFailure::FinalizerPanic)
    );
    assert_eq!(*events.lock().unwrap(), [42, 41]);
    core.retire_carrier_metadata(&root).unwrap();
    assert_eq!(core.end_frame(&parent, &mut registry).failure(), None);
    assert_eq!(*events.lock().unwrap(), [42, 41, 40]);
    clean(&core, &registry);
    assert_eq!(core.live_carrier_roots(), 0);
}
