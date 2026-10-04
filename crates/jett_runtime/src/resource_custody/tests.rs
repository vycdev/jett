use super::*;
use std::collections::HashSet;
use std::sync::Mutex;

type Events = Arc<Mutex<Vec<i64>>>;

// Constructor-owned test registration only. No production issuer or provider.
fn registration(
    registry: &ResourceRegistry,
    kinds: Vec<ResourceTypeId>,
    slots: Vec<usize>,
) -> Result<RegisteredResourceProgram, CustodyError> {
    let mut nominal = HashSet::new();
    if !kinds.iter().all(|kind| nominal.insert(*kind))
        || slots.iter().any(|kind| *kind >= kinds.len())
    {
        return Err(CustodyError::WrongKind);
    }
    Ok(RegisteredResourceProgram(Arc::new(ProgramLayout {
        context: registry.context_id,
        kinds,
        slots,
    })))
}

fn test_program(registry: &ResourceRegistry) -> RegisteredResourceProgram {
    registration(
        registry,
        vec![ResourceTypeId::new(100), ResourceTypeId::new(101)],
        vec![0, 0, 1, 0],
    )
    .unwrap()
}

fn authority() -> AuthorityProvenance {
    AuthorityProvenance::new(5, 7)
}

fn destination(
    core: &ResourceCustody,
    program: &RegisteredResourceProgram,
    frame: &ResourceFrameToken,
    slot: usize,
) -> ResourceHolderToken {
    core.holder(frame, &program.slot(slot).unwrap()).unwrap()
}

fn acquire(
    core: &mut ResourceCustody,
    registry: &mut ResourceRegistry,
    program: &RegisteredResourceProgram,
    frame: &ResourceFrameToken,
    slot: usize,
    label: i64,
    events: &Events,
    panics: bool,
) -> OwnedResourceToken {
    let holder = destination(core, program, frame, slot);
    let kind = program.kind(program.0.slots[slot]).unwrap();
    let prepared = core.prepare_acquisition(&kind, &holder).unwrap();
    let events = events.clone();
    core.commit_acquisition(registry, prepared, authority(), label, move |value| {
        events.lock().unwrap().push(value);
        if panics {
            panic!("selected custody finalizer");
        }
    })
    .unwrap()
}

fn clean(core: &ResourceCustody, registry: &ResourceRegistry) {
    assert_eq!(
        (
            core.live_owners(),
            core.live_loans(),
            core.active_frames(),
            registry.live_count()
        ),
        (0, 0, 0, 0)
    );
}

#[test]
fn custody_registration_joins_context_program_kind_and_slot_before_acquisition() {
    let mut registry = ResourceRegistry::new();
    let program = test_program(&registry);
    let foreign_registry = ResourceRegistry::new();
    assert!(matches!(
        ResourceCustody::new(program.clone(), &foreign_registry),
        Err(CustodyError::WrongContext)
    ));
    assert!(matches!(
        registration(&registry, vec![ResourceTypeId::new(100); 2], vec![0]),
        Err(CustodyError::WrongKind)
    ));
    assert!(matches!(
        registration(&registry, vec![ResourceTypeId::new(100)], vec![1]),
        Err(CustodyError::WrongKind)
    ));
    let other = registration(&registry, program.0.kinds.clone(), program.0.slots.clone()).unwrap();
    let mut core = ResourceCustody::new(program.clone(), &registry).unwrap();
    let frame = core
        .begin_frame(ResourceFrameKind::Scope, ResourcePurpose::Runtime)
        .unwrap();
    let holder = destination(&core, &program, &frame, 0);
    assert!(matches!(
        core.prepare_acquisition(&other.kind(0).unwrap(), &holder),
        Err(CustodyError::WrongProgram)
    ));
    assert!(matches!(
        core.holder(&frame, &other.slot(0).unwrap()),
        Err(CustodyError::WrongProgram)
    ));
    assert!(matches!(
        core.prepare_acquisition(&program.kind(1).unwrap(), &holder),
        Err(CustodyError::WrongKind)
    ));
    assert!(matches!(
        program.kind(usize::MAX),
        Err(CustodyError::WrongKind)
    ));
    assert!(matches!(
        program.slot(usize::MAX),
        Err(CustodyError::UnknownSlot)
    ));
    assert_eq!((core.live_owners(), registry.live_count()), (0, 0));
    assert_eq!(core.end_frame(&frame, &mut registry).failure(), None);
    clean(&core, &registry);
}

#[test]
fn custody_registry_nominal_key_generation_and_provenance_remain_exact() {
    let mut registry = ResourceRegistry::new();
    let program = test_program(&registry);
    let events = Arc::new(Mutex::new(Vec::new()));
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
        1,
        &events,
        false,
    );
    let key = core.validate_owned(&owner, &registry).unwrap();
    let other_registry = ResourceRegistry::new();
    assert_eq!(
        core.validate_owned(&owner, &other_registry),
        Err(CustodyError::WrongContext)
    );
    core.owners[owner.owner].key.resource_type = ResourceTypeId::new(101);
    assert_eq!(
        core.validate_owned(&owner, &registry),
        Err(CustodyError::Registry(RegistryError::WrongType))
    );
    core.owners[owner.owner].key = key;
    core.owners[owner.owner].authority = AuthorityProvenance::new(5, 99);
    assert_eq!(
        core.close(&mut owner, &mut registry).failure(),
        Some(ResourceCleanupFailure::Custody(CustodyError::Registry(
            RegistryError::AuthorityMismatch
        )))
    );
    assert_eq!((core.live_owners(), registry.live_count()), (1, 1));
    assert!(events.lock().unwrap().is_empty());
    core.owners[owner.owner].authority = authority();
    core.owners[owner.owner].key.generation += 1;
    assert_eq!(
        core.validate_owned(&owner, &registry),
        Err(CustodyError::Registry(RegistryError::StaleGeneration))
    );
    core.owners[owner.owner].key = key;
    assert_eq!(core.close(&mut owner, &mut registry).failure(), None);
    assert_eq!(
        core.close(&mut owner, &mut registry).failure(),
        Some(ResourceCleanupFailure::Custody(CustodyError::InvalidOwner))
    );
    assert_eq!(core.end_frame(&frame, &mut registry).failure(), None);
    assert_eq!(*events.lock().unwrap(), [1]);
    clean(&core, &registry);
}

#[test]
fn custody_returned_holder_generation_controls_reverse_current_acquisition() {
    let mut registry = ResourceRegistry::new();
    let program = test_program(&registry);
    let events = Arc::new(Mutex::new(Vec::new()));
    let mut core = ResourceCustody::new(program.clone(), &registry).unwrap();
    let outer = core
        .begin_frame(ResourceFrameKind::Scope, ResourcePurpose::Runtime)
        .unwrap();
    let original = destination(&core, &program, &outer, 0);
    let mut owner = acquire(
        &mut core,
        &mut registry,
        &program,
        &outer,
        0,
        1,
        &events,
        false,
    );
    let stale = OwnedResourceToken {
        core: owner.core,
        owner: owner.owner,
        holder_generation: owner.holder_generation,
    };
    let _second = acquire(
        &mut core,
        &mut registry,
        &program,
        &outer,
        1,
        2,
        &events,
        false,
    );
    let inner = core
        .begin_frame(ResourceFrameKind::Return, ResourcePurpose::Runtime)
        .unwrap();
    let returned = destination(&core, &program, &inner, 0);
    core.transfer(&mut owner, &returned, &registry).unwrap();
    assert_eq!(
        core.validate_owned(&stale, &registry),
        Err(CustodyError::StaleHolderGeneration)
    );
    core.transfer(&mut owner, &original, &registry).unwrap();
    assert_eq!(core.end_frame(&inner, &mut registry).failure(), None);
    assert!(events.lock().unwrap().is_empty());
    assert_eq!(core.end_frame(&outer, &mut registry).failure(), None);
    assert_eq!(*events.lock().unwrap(), [1, 2]);
    assert_eq!(
        core.validate_owned(&owner, &registry),
        Err(CustodyError::InvalidOwner)
    );
    clean(&core, &registry);
}

#[test]
fn custody_live_lease_blocks_move_close_and_older_frame_borrow_before_effect() {
    let mut registry = ResourceRegistry::new();
    let program = test_program(&registry);
    let events = Arc::new(Mutex::new(Vec::new()));
    let mut core = ResourceCustody::new(program.clone(), &registry).unwrap();
    let scope = core
        .begin_frame(ResourceFrameKind::Scope, ResourcePurpose::Runtime)
        .unwrap();
    let operation = core
        .begin_frame(ResourceFrameKind::Operation, ResourcePurpose::Runtime)
        .unwrap();
    let mut owner = acquire(
        &mut core,
        &mut registry,
        &program,
        &operation,
        0,
        3,
        &events,
        false,
    );
    assert!(matches!(
        core.begin_borrow(&owner, &scope, &registry),
        Err(CustodyError::InvalidLoan)
    ));
    let mut loan = core.begin_borrow(&owner, &operation, &registry).unwrap();
    let destination = destination(&core, &program, &scope, 0);
    assert_eq!(
        core.transfer(&mut owner, &destination, &registry),
        Err(CustodyError::ActiveBorrow)
    );
    assert_eq!(
        core.close(&mut owner, &mut registry).failure(),
        Some(ResourceCleanupFailure::Custody(CustodyError::ActiveBorrow))
    );
    assert_eq!(
        core.validate_borrowed(&loan, &registry),
        core.validate_owned(&owner, &registry)
    );
    assert!(events.lock().unwrap().is_empty());
    core.end_borrow(&mut loan).unwrap();
    assert_eq!(core.end_borrow(&mut loan), Err(CustodyError::InvalidLoan));
    core.transfer(&mut owner, &destination, &registry).unwrap();
    assert_eq!(
        core.validate_borrowed(&loan, &registry),
        Err(CustodyError::InvalidLoan)
    );
    assert_eq!(core.end_frame(&operation, &mut registry).failure(), None);
    assert_eq!(core.close(&mut owner, &mut registry).failure(), None);
    assert_eq!(core.end_frame(&scope, &mut registry).failure(), None);
    assert_eq!(*events.lock().unwrap(), [3]);
    clean(&core, &registry);
}

#[test]
fn custody_chronological_scope_operation_unwind_releases_callee_loan_first() {
    let mut registry = ResourceRegistry::new();
    let program = test_program(&registry);
    let events = Arc::new(Mutex::new(Vec::new()));
    let mut core = ResourceCustody::new(program.clone(), &registry).unwrap();
    let caller = core
        .begin_frame(ResourceFrameKind::Scope, ResourcePurpose::Runtime)
        .unwrap();
    let _caller_owner = acquire(
        &mut core,
        &mut registry,
        &program,
        &caller,
        0,
        1,
        &events,
        false,
    );
    let operation = core
        .begin_frame(ResourceFrameKind::Operation, ResourcePurpose::Runtime)
        .unwrap();
    let operation_owner = acquire(
        &mut core,
        &mut registry,
        &program,
        &operation,
        0,
        2,
        &events,
        false,
    );
    let callee = core
        .begin_frame(ResourceFrameKind::Scope, ResourcePurpose::Runtime)
        .unwrap();
    let loan = core
        .begin_borrow(&operation_owner, &callee, &registry)
        .unwrap();
    let _callee_owner = acquire(
        &mut core,
        &mut registry,
        &program,
        &callee,
        0,
        3,
        &events,
        true,
    );
    assert_eq!(
        core.end_frame(&operation, &mut registry).failure(),
        Some(ResourceCleanupFailure::Custody(
            CustodyError::OutOfOrderFrame
        ))
    );
    assert!(events.lock().unwrap().is_empty());
    let body = ResourceBodyOutcome::<(), &str>::HostPanic;
    assert_eq!(
        complete_resource(body, core.unwind_all(&mut registry)),
        Err(ResourceCompletionFailure::Cleanup(
            ResourceCleanupFailure::FinalizerPanic
        ))
    );
    assert_eq!(*events.lock().unwrap(), [3, 2, 1]);
    assert_eq!(
        core.validate_borrowed(&loan, &registry),
        Err(CustodyError::InvalidLoan)
    );
    clean(&core, &registry);
}

#[test]
fn custody_required_purpose_cannot_be_overridden_by_nested_runtime_frame() {
    for purpose in [
        ResourcePurpose::NamespaceConstant,
        ResourcePurpose::ExplicitComptime,
        ResourcePurpose::Verify,
        ResourcePurpose::Property,
    ] {
        let mut registry = ResourceRegistry::new();
        let program = test_program(&registry);
        let mut core = ResourceCustody::new(program.clone(), &registry).unwrap();
        let frame = core.begin_frame(ResourceFrameKind::Scope, purpose).unwrap();
        let holder = destination(&core, &program, &frame, 0);
        assert!(matches!(
            core.prepare_acquisition(&program.kind(0).unwrap(), &holder),
            Err(CustodyError::WrongPurpose)
        ));
        assert!(matches!(
            core.begin_frame(ResourceFrameKind::Operation, ResourcePurpose::Runtime),
            Err(CustodyError::WrongPurpose)
        ));
        assert_eq!(
            (
                core.live_owners(),
                registry.live_count(),
                core.active_frames()
            ),
            (0, 0, 1)
        );
        assert_eq!(core.end_frame(&frame, &mut registry).failure(), None);
        clean(&core, &registry);
    }
}

#[test]
fn custody_failed_prepared_publication_cleans_only_the_unpublished_payload() {
    for panics in [false, true] {
        let mut registry = ResourceRegistry::new();
        let program = test_program(&registry);
        let events = Arc::new(Mutex::new(Vec::new()));
        let mut core = ResourceCustody::new(program.clone(), &registry).unwrap();
        let frame = core
            .begin_frame(ResourceFrameKind::Scope, ResourcePurpose::Runtime)
            .unwrap();
        let holder = destination(&core, &program, &frame, 0);
        let preparation = core
            .prepare_acquisition(&program.kind(0).unwrap(), &holder)
            .unwrap();
        let _original = acquire(
            &mut core,
            &mut registry,
            &program,
            &frame,
            0,
            1,
            &events,
            false,
        );
        let observed = events.clone();
        let failure = core
            .commit_acquisition(
                &mut registry,
                preparation,
                authority(),
                99_i64,
                move |payload| {
                    observed.lock().unwrap().push(payload);
                    if panics {
                        panic!("unpublished payload finalizer");
                    }
                },
            )
            .unwrap_err();
        assert_eq!(
            failure,
            if panics {
                ResourceTransitionFailure::Cleanup(ResourceCleanupFailure::FinalizerPanic)
            } else {
                ResourceTransitionFailure::Rejected(CustodyError::OccupiedHolder)
            }
        );
        assert_eq!((core.live_owners(), registry.live_count()), (1, 1));
        assert_eq!(*events.lock().unwrap(), [99]);
        assert_eq!(core.unwind_all(&mut registry).failure(), None);
        assert_eq!(*events.lock().unwrap(), [99, 1]);
        clean(&core, &registry);
    }
}

#[test]
fn custody_dropped_carrier_token_and_failed_caller_publication_do_not_lose_cleanup() {
    let mut registry = ResourceRegistry::new();
    let program = test_program(&registry);
    let events = Arc::new(Mutex::new(Vec::new()));
    let mut core = ResourceCustody::new(program.clone(), &registry).unwrap();
    let frame = core
        .begin_frame(ResourceFrameKind::Operation, ResourcePurpose::Runtime)
        .unwrap();
    let owner = acquire(
        &mut core,
        &mut registry,
        &program,
        &frame,
        0,
        7,
        &events,
        false,
    );
    let physical = core.validate_owned(&owner, &registry).unwrap();
    let physical_copy = physical;
    assert_eq!(physical, physical_copy);
    drop(owner);
    assert!(events.lock().unwrap().is_empty());
    assert_eq!((core.live_owners(), registry.live_count()), (1, 1));
    assert!(matches!(
        ResourceCustody::new(program.clone(), &registry),
        Err(CustodyError::UnownedRegistry)
    ));
    let body = ResourceBodyOutcome::<(), &str>::Completed(Err("caller publication failed"));
    assert_eq!(
        complete_resource(body, core.unwind_all(&mut registry)),
        Err(ResourceCompletionFailure::Body("caller publication failed"))
    );
    assert_eq!(*events.lock().unwrap(), [7]);
    clean(&core, &registry);
}

#[test]
fn custody_repeated_entry_preserves_identity_and_rejects_stale_frames_and_tokens() {
    let mut registry = ResourceRegistry::new();
    let program = test_program(&registry);
    let events = Arc::new(Mutex::new(Vec::new()));
    let mut core = ResourceCustody::new(program.clone(), &registry).unwrap();
    let id = core.id;
    for label in 1..=3 {
        let frame = core
            .begin_frame(ResourceFrameKind::Scope, ResourcePurpose::Runtime)
            .unwrap();
        let owner = acquire(
            &mut core,
            &mut registry,
            &program,
            &frame,
            0,
            label,
            &events,
            label == 2,
        );
        let cleanup = core.unwind_all(&mut registry);
        assert_eq!(
            cleanup.failure(),
            if label == 2 {
                Some(ResourceCleanupFailure::FinalizerPanic)
            } else {
                None
            }
        );
        assert_eq!(
            core.end_frame(&frame, &mut registry).failure(),
            Some(ResourceCleanupFailure::Custody(CustodyError::InvalidFrame))
        );
        assert_eq!(
            core.validate_owned(&owner, &registry),
            Err(CustodyError::InvalidOwner)
        );
        assert_eq!(core.id, id);
        clean(&core, &registry);
    }
    assert_eq!(*events.lock().unwrap(), [1, 2, 3]);
}

#[test]
fn custody_counter_exhaustion_refuses_transfer_without_reassigning_holder() {
    let mut registry = ResourceRegistry::new();
    let program = test_program(&registry);
    let events = Arc::new(Mutex::new(Vec::new()));
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
        1,
        &events,
        false,
    );
    core.owners[owner.owner].generation = u64::MAX;
    owner.holder_generation = u64::MAX;
    let before = core.owners[owner.owner].holder;
    let log = core.frames[frame.index].acquisitions.len();
    let destination = destination(&core, &program, &frame, 1);
    assert_eq!(
        core.transfer(&mut owner, &destination, &registry),
        Err(CustodyError::CapacityExhausted)
    );
    assert_eq!(core.owners[owner.owner].holder, before);
    assert_eq!(core.frames[frame.index].acquisitions.len(), log);
    assert!(events.lock().unwrap().is_empty());
    assert_eq!(core.close(&mut owner, &mut registry).failure(), None);
    assert_eq!(core.end_frame(&frame, &mut registry).failure(), None);
    clean(&core, &registry);
}

#[test]
fn custody_registry_shutdown_and_postinsert_cleanup_failures_remain_separate() {
    let mut registry = ResourceRegistry::new();
    let program = test_program(&registry);
    let events = Arc::new(Mutex::new(Vec::new()));
    let mut core = ResourceCustody::new(program.clone(), &registry).unwrap();
    let frame = core
        .begin_frame(ResourceFrameKind::Scope, ResourcePurpose::Runtime)
        .unwrap();
    let holder = destination(&core, &program, &frame, 0);
    let prepared = core
        .prepare_acquisition(&program.kind(0).unwrap(), &holder)
        .unwrap();
    registry.shutdown();
    let observed = events.clone();
    assert_eq!(
        core.commit_acquisition(&mut registry, prepared, authority(), 8_i64, move |value| {
            observed.lock().unwrap().push(value)
        })
        .unwrap_err(),
        ResourceTransitionFailure::Rejected(CustodyError::Registry(RegistryError::ShuttingDown))
    );
    assert_eq!(*events.lock().unwrap(), [8]);
    assert_eq!(core.unwind_all(&mut registry).failure(), None);
    clean(&core, &registry);

    let mut registry = ResourceRegistry::new();
    let program = test_program(&registry);
    let events = Arc::new(Mutex::new(Vec::new()));
    let mut core = ResourceCustody::new(program.clone(), &registry).unwrap();
    let frame = core
        .begin_frame(ResourceFrameKind::Scope, ResourcePurpose::Runtime)
        .unwrap();
    let first = acquire(
        &mut core,
        &mut registry,
        &program,
        &frame,
        0,
        1,
        &events,
        false,
    );
    let _second = acquire(
        &mut core,
        &mut registry,
        &program,
        &frame,
        1,
        2,
        &events,
        false,
    );
    let key = core.validate_owned(&first, &registry).unwrap();
    registry.close(key, ResourceTypeId::new(100)).unwrap();
    let outcome = core.unwind_all(&mut registry);
    assert!(matches!(
        outcome.failure(),
        Some(ResourceCleanupFailure::Custody(CustodyError::Registry(
            RegistryError::StaleGeneration
        )))
    ));
    assert_eq!(*events.lock().unwrap(), [1, 2]);
    clean(&core, &registry);
}

#[test]
fn custody_completion_has_cleanup_first_matrix_without_changing_domain_values() {
    for cleanup in [
        None,
        Some(ResourceCleanupFailure::Custody(CustodyError::InvalidFrame)),
        Some(ResourceCleanupFailure::FinalizerPanic),
    ] {
        for body in [
            ResourceBodyOutcome::Completed(Ok(7_u8)),
            ResourceBodyOutcome::Completed(Err("entry")),
            ResourceBodyOutcome::HostPanic,
        ] {
            let expected = match cleanup {
                Some(failure) => Err(ResourceCompletionFailure::Cleanup(failure)),
                None => match &body {
                    ResourceBodyOutcome::Completed(Ok(value)) => Ok(*value),
                    ResourceBodyOutcome::Completed(Err(error)) => {
                        Err(ResourceCompletionFailure::Body(*error))
                    }
                    ResourceBodyOutcome::HostPanic => Err(ResourceCompletionFailure::BodyPanic),
                },
            };
            assert_eq!(
                complete_resource(body, ResourceCleanupOutcome { first: cleanup }),
                expected
            );
        }
    }
    let domain = ResourceBodyOutcome::<Result<u8, &str>, &str>::Completed(Ok(Err("domain")));
    assert_eq!(
        complete_resource(domain, ResourceCleanupOutcome::default()),
        Ok(Err("domain"))
    );
    let caught = ResourceBodyOutcome::<(), &str>::caught(catch_unwind(AssertUnwindSafe(
        || -> Result<(), &str> {
            panic!("body panic");
        },
    )));
    assert_eq!(
        complete_resource(caught, ResourceCleanupOutcome::default()),
        Err(ResourceCompletionFailure::BodyPanic)
    );
    let mut first = ResourceCleanupOutcome {
        first: Some(ResourceCleanupFailure::Custody(CustodyError::InvalidFrame)),
    };
    first.merge(ResourceCleanupOutcome {
        first: Some(ResourceCleanupFailure::FinalizerPanic),
    });
    assert_eq!(
        first.failure(),
        Some(ResourceCleanupFailure::Custody(CustodyError::InvalidFrame))
    );
}

#[test]
fn custody_failed_provenance_cleanup_preserves_unretired_frame_and_continues_eligible_owners() {
    let mut registry = ResourceRegistry::new();
    let program = test_program(&registry);
    let events = Arc::new(Mutex::new(Vec::new()));
    let mut core = ResourceCustody::new(program.clone(), &registry).unwrap();
    let frame = core
        .begin_frame(ResourceFrameKind::Scope, ResourcePurpose::Runtime)
        .unwrap();
    let first = acquire(
        &mut core,
        &mut registry,
        &program,
        &frame,
        0,
        1,
        &events,
        false,
    );
    let _second = acquire(
        &mut core,
        &mut registry,
        &program,
        &frame,
        1,
        2,
        &events,
        false,
    );
    core.owners[first.owner].authority = AuthorityProvenance::new(5, 99);
    assert_eq!(
        core.end_frame(&frame, &mut registry).failure(),
        Some(ResourceCleanupFailure::Custody(CustodyError::Registry(
            RegistryError::AuthorityMismatch
        )))
    );
    assert_eq!(*events.lock().unwrap(), [2]);
    assert_eq!(
        (
            core.live_owners(),
            registry.live_count(),
            core.active_frames()
        ),
        (1, 1, 1)
    );
    core.owners[first.owner].authority = authority();
    assert_eq!(core.end_frame(&frame, &mut registry).failure(), None);
    assert_eq!(*events.lock().unwrap(), [2, 1]);
    clean(&core, &registry);
}
