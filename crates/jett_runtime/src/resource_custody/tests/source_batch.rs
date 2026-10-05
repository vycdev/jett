use super::*;

#[test]
fn custody_source_batch_late_active_borrow_refuses_without_moving_a_prefix() {
    let mut registry = ResourceRegistry::new();
    let program = test_program(&registry);
    let mut core = ResourceCustody::new(program.clone(), &registry).unwrap();
    let events = Arc::new(Mutex::new(Vec::new()));
    let parent = core
        .begin_frame(ResourceFrameKind::Scope, ResourcePurpose::Runtime)
        .unwrap();
    let mut first = acquire(
        &mut core,
        &mut registry,
        &program,
        &parent,
        0,
        501,
        &events,
        false,
    );
    let mut second = acquire(
        &mut core,
        &mut registry,
        &program,
        &parent,
        1,
        502,
        &events,
        false,
    );
    let mut loan = core.begin_borrow(&second, &parent, &registry).unwrap();
    let callee = core
        .begin_frame(ResourceFrameKind::Scope, ResourcePurpose::Runtime)
        .unwrap();
    let slot0 = program.slot(0).unwrap();
    let slot1 = program.slot(1).unwrap();
    assert_eq!(
        core.preflight_transfer_batch(&callee, &[(&first, &slot0), (&second, &slot1)], &registry),
        Err(CustodyError::ActiveBorrow)
    );
    assert_eq!(
        (
            core.owners[first.owner].holder.frame,
            core.owners[second.owner].holder.frame
        ),
        (parent.index, parent.index)
    );
    assert!(core.frames[callee.index].acquisitions.is_empty());
    assert!(events.lock().unwrap().is_empty());
    core.end_borrow(&mut loan).unwrap();
    core.preflight_transfer_batch(&callee, &[(&first, &slot0), (&second, &slot1)], &registry)
        .unwrap();
    assert!(core.frames[callee.index].acquisitions.capacity() >= 2);
    let holder0 = core.holder(&callee, &slot0).unwrap();
    let holder1 = core.holder(&callee, &slot1).unwrap();
    core.transfer(&mut first, &holder0, &registry).unwrap();
    core.transfer(&mut second, &holder1, &registry).unwrap();
    assert!(core.end_frame(&callee, &mut registry).failure().is_none());
    assert!(core.end_frame(&parent, &mut registry).failure().is_none());
    clean(&core, &registry);
    assert_eq!(*events.lock().unwrap(), vec![502, 501]);
}

#[test]
fn custody_source_batch_late_generation_overflow_and_duplicate_owner_refuse_before_prefix() {
    let mut registry = ResourceRegistry::new();
    let program = test_program(&registry);
    let mut core = ResourceCustody::new(program.clone(), &registry).unwrap();
    let events = Arc::new(Mutex::new(Vec::new()));
    let parent = core
        .begin_frame(ResourceFrameKind::Scope, ResourcePurpose::Runtime)
        .unwrap();
    let first = acquire(
        &mut core,
        &mut registry,
        &program,
        &parent,
        0,
        501,
        &events,
        false,
    );
    let mut second = acquire(
        &mut core,
        &mut registry,
        &program,
        &parent,
        1,
        502,
        &events,
        false,
    );
    let callee = core
        .begin_frame(ResourceFrameKind::Scope, ResourcePurpose::Runtime)
        .unwrap();
    let slot0 = program.slot(0).unwrap();
    let slot1 = program.slot(1).unwrap();
    assert_eq!(
        core.preflight_transfer_batch(&callee, &[(&first, &slot0), (&first, &slot1)], &registry),
        Err(CustodyError::OccupiedHolder)
    );
    core.owners[second.owner].generation = u64::MAX;
    second.holder_generation = u64::MAX;
    assert_eq!(
        core.preflight_transfer_batch(&callee, &[(&first, &slot0), (&second, &slot1)], &registry),
        Err(CustodyError::CapacityExhausted)
    );
    assert_eq!(core.owners[first.owner].holder.frame, parent.index);
    assert_eq!(first.holder_generation, 1);
    assert!(core.frames[callee.index].acquisitions.is_empty());
    assert!(events.lock().unwrap().is_empty());
    core.owners[second.owner].generation = 1;
    second.holder_generation = 1;
    assert!(core.unwind_all(&mut registry).failure().is_none());
    clean(&core, &registry);
    assert_eq!(*events.lock().unwrap(), vec![502, 501]);
}
