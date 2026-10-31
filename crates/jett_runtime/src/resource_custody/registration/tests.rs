use super::schema::WireLayout;
use super::wire::{decode, encode_records};
use super::*;
use crate::AuthorityProvenance;
use crate::resource_custody::{
    OwnedResourceToken, ResourceBodyOutcome, ResourceCleanupFailure, ResourceCompletionFailure,
    ResourceCustody, ResourceFrameKind, ResourcePurpose, complete_resource,
};
use std::mem::{align_of, offset_of, size_of};
use std::sync::Mutex;

fn site(function: u32, index: u32) -> NativeSite {
    NativeSite {
        function,
        block: 0,
        position: NativePosition::Statement(index),
    }
}
fn canonical() -> WireLayout {
    let view = |shape| NativeFormal {
        shape,
        access: NativeAccess::View,
    };
    let owned = |shape| NativeFormal {
        shape,
        access: NativeAccess::Owned,
    };
    let signature = |ordinal, parameters, result| NativeSignature {
        ordinal,
        parameters,
        result,
    };
    let frame = |ordinal, role, function, index, signature, parents| NativeFrame {
        ordinal,
        role,
        site: site(function, index),
        signature,
        parents,
    };
    let slot = |ordinal, frame, shape, path| NativeSlot {
        ordinal,
        frame,
        shape,
        path,
    };
    use NativeOperation::*;
    let loan = NativeLoanSource::ExistingBorrow { operation: 3 };
    let incoming = NativeLoanSource::IncomingViewFormal {
        scope: 3,
        parameter: 0,
    };
    let operations = vec![
        Acquire {
            frame: 1,
            hook: 0,
            destination: 2,
        },
        SumTake {
            frame: 1,
            source: 2,
            destination: 1,
        },
        Transfer {
            frame: 0,
            source: 1,
            destination: 0,
        },
        Borrow {
            frame: 1,
            source: 0,
            lease_frame: 1,
        },
        InvokeBorrow {
            frame: 1,
            hook: 1,
            source: loan,
        },
        BoundedBorrowUse {
            frame: 1,
            source: loan,
            callee_frame: 3,
        },
        EndBorrow {
            frame: 1,
            borrow: 3,
        },
        Close {
            frame: 1,
            hook: 2,
            source: 0,
        },
        Drop {
            frame: 0,
            source: 1,
        },
        SumAdopt {
            frame: 1,
            source: 1,
            destination: 2,
        },
        SumDrop {
            frame: 1,
            source: 2,
        },
        Replace {
            frame: 0,
            old: 0,
            replacement: 1,
        },
        Complete { frame: 0 },
        Descriptor { hook: 0 },
        InvokeDescriptor {
            hook: 0,
            signature: 0,
            target: 0,
        },
        InvokeSourceFunction {
            frame: 1,
            callee: 1,
            signature: 4,
            callee_scope: 3,
            source: None,
        },
        InvokeBorrow {
            frame: 1,
            hook: 1,
            source: incoming,
        },
        BoundedBorrowUse {
            frame: 1,
            source: incoming,
            callee_frame: 3,
        },
    ]
    .into_iter()
    .enumerate()
    .map(|(ordinal, operation)| NativeOperationRecord {
        ordinal: ordinal as u32,
        site: site(0, 100 + ordinal as u32),
        operation,
    })
    .collect();
    WireLayout {
        carriers: super::carriers::NativeCarrierLayout::default(),
        version: 1,
        kinds: vec![0, 1],
        hooks: vec![
            NativeHook {
                ordinal: 0,
                kind: 0,
                recipe: NativeRecipe::NetworkFactory,
                signature: 0,
            },
            NativeHook {
                ordinal: 1,
                kind: 0,
                recipe: NativeRecipe::NetworkBorrow,
                signature: 1,
            },
            NativeHook {
                ordinal: 2,
                kind: 0,
                recipe: NativeRecipe::Finalize,
                signature: 2,
            },
            NativeHook {
                ordinal: 3,
                kind: 1,
                recipe: NativeRecipe::NetworkFactory,
                signature: 5,
            },
        ],
        signatures: vec![
            signature(0, vec![view(0), owned(1)], 6),
            signature(1, vec![view(0), view(4)], 7),
            signature(2, vec![owned(4)], 3),
            signature(3, vec![], 3),
            signature(4, vec![view(4)], 3),
            signature(5, vec![view(0), owned(1)], 13),
        ],
        shapes: vec![
            NativeShape::Network,
            NativeShape::Integer {
                bits: 64,
                signed: true,
            },
            NativeShape::String,
            NativeShape::Nothing,
            NativeShape::Resource { kind: 0 },
            NativeShape::Resource { kind: 1 },
            NativeShape::Result { ok: 4, fail: 2 },
            NativeShape::Result { ok: 1, fail: 2 },
            NativeShape::Optional { child: 4 },
            NativeShape::HookDescriptor { hook: 0 },
            NativeShape::Bool,
            NativeShape::Float { bits: 64 },
            NativeShape::Integer {
                bits: 8,
                signed: false,
            },
            NativeShape::Result { ok: 5, fail: 2 },
        ],
        frames: vec![
            frame(0, NativeFrameRole::Scope, 0, 0, 3, vec![NativeParent::Root]),
            frame(
                1,
                NativeFrameRole::Operation,
                0,
                1,
                3,
                vec![NativeParent::Frame(0)],
            ),
            frame(
                2,
                NativeFrameRole::Return,
                0,
                2,
                3,
                vec![NativeParent::Frame(0)],
            ),
            frame(
                3,
                NativeFrameRole::Scope,
                1,
                0,
                4,
                vec![NativeParent::Frame(1)],
            ),
        ],
        slots: vec![
            slot(0, 0, 4, vec![]),
            slot(1, 1, 4, vec![]),
            slot(2, 1, 6, vec![NativePayloadStep::Ok]),
            slot(3, 0, 8, vec![NativePayloadStep::Some]),
            slot(4, 0, 5, vec![]),
        ],
        operations,
    }
}
fn parsed(layout: &WireLayout) -> Result<WireLayout, ResourceLayoutError> {
    let decoded = decode(&encode_records(layout))?;
    validation::validate(&decoded)?;
    Ok(decoded)
}
fn installed(registry: &ResourceRegistry) -> Arc<RegisteredNativeLayout> {
    NativeLayoutInstallation::new(registry)
        .install(registry, &encode_records(&canonical()))
        .unwrap()
}

fn source_v2() -> WireLayout {
    let mut layout = canonical();
    layout.version = 2;
    let NativeOperation::InvokeSourceFunction { source, .. } = &mut layout.operations[15].operation
    else {
        panic!("canonical Source row")
    };
    *source = Some(NativeSourceInvocation {
        callee_return: None,
        evaluation_order: vec![0],
        formals: vec![NativeSourceFormal {
            parameter: 0,
            source_index: 0,
            actual_shape: 4,
            callee_shape: 4,
            syntax: NativeSourceSyntax::WrittenView,
            effect: NativeSourceEffect::RetainBorrow,
            access: NativeAccess::View,
            value: NativeSourceValue::ResidentView {
                source: NativeLoanSource::ExistingBorrow { operation: 3 },
            },
        }],
        result: NativeSourceResult::Ordinary { shape: 3 },
    });
    layout
}

#[test]
fn native_registration_source_v2_roundtrips_the_complete_boundary_without_upgrading_v1() {
    let layout = source_v2();
    assert_eq!(parsed(&layout).unwrap(), layout);
    let registry = ResourceRegistry::new();
    let mut installation = NativeLayoutInstallation::new(&registry);
    let installed = installation
        .install(&registry, &encode_records(&layout))
        .unwrap();
    assert_eq!(installed.wire_version(), 2);
    assert_eq!(parsed(&canonical()).unwrap().version, 1);
    let mut missing = layout.clone();
    let NativeOperation::InvokeSourceFunction { source, .. } =
        &mut missing.operations[15].operation
    else {
        unreachable!()
    };
    *source = None;
    assert!(parsed(&missing).is_err());
    let mut wrong_version = layout;
    wrong_version.version = 1;
    assert!(parsed(&wrong_version).is_err());
}

#[test]
fn native_registration_source_v2_rejects_unused_permutation_formal_and_resident_substitutions() {
    for case in 0..7 {
        let mut layout = source_v2();
        let NativeOperation::InvokeSourceFunction {
            source: Some(source),
            ..
        } = &mut layout.operations[15].operation
        else {
            unreachable!()
        };
        match case {
            0 => source.evaluation_order[0] = 1,
            1 => source.formals[0].source_index = 1,
            2 => source.formals[0].syntax = NativeSourceSyntax::Bare,
            3 => source.formals[0].effect = NativeSourceEffect::TransferOwned,
            4 => source.formals[0].value = NativeSourceValue::Ordinary,
            5 => {
                source.formals[0].value = NativeSourceValue::ResidentView {
                    source: NativeLoanSource::ExistingBorrow { operation: 0 },
                }
            }
            6 => source.formals[0].actual_shape = 5,
            _ => unreachable!(),
        }
        assert!(
            parsed(&layout).is_err(),
            "unused Source metadata case {case}"
        );
    }
}

#[test]
fn native_registration_source_v2_rejects_unbound_return_and_occupied_failure_companion_roles() {
    for operation in [
        NativeOperation::PublishReturn {
            frame: 3,
            source_return_slot: 0,
        },
        NativeOperation::TakeFailureCompanion {
            frame: 1,
            source_sum_slot: 2,
            failure_shape: 4,
        },
        NativeOperation::CreateFailureSum {
            frame: 1,
            destination_slot: 2,
            failure_shape: 4,
        },
    ] {
        let mut layout = source_v2();
        let ordinal = layout.operations.len() as u32;
        layout.operations.push(NativeOperationRecord {
            ordinal,
            site: site(1, 999),
            operation,
        });
        assert!(parsed(&layout).is_err());
    }
}

#[test]
fn native_registration_roundtrips_closed_records_and_owns_input() {
    let layout = canonical();
    assert_eq!(parsed(&layout).unwrap(), layout);
    let mut registry = ResourceRegistry::new();
    let mut installation = NativeLayoutInstallation::new(&registry);
    let mut bytes = encode_records(&layout);
    let registered = installation.install(&registry, &bytes).unwrap();
    bytes.fill(0);
    assert!(Arc::ptr_eq(installation.installed().unwrap(), &registered));
    assert_eq!(registered.hooks()[0].recipe(), NativeRecipe::NetworkFactory);
    assert_eq!(registered.shapes(), layout.shapes);
    assert_eq!(registered.operations(), layout.operations);
    assert_eq!(
        (registered.kinds_len(), registered.slot_kind(2).unwrap()),
        (2, 0)
    );
    assert_ne!(registered.kind_id(0).unwrap(), ResourceTypeId::new(0));
    registered
        .preflight_physical_acquisition(&mut registry)
        .unwrap();
    assert_eq!(registry.live_count(), 0);
    assert_eq!(
        (
            size_of::<JettResourceCallResultV1>(),
            align_of::<JettResourceCallResultV1>()
        ),
        (16, 8)
    );
    assert_eq!(
        (
            offset_of!(JettResourceCallResultV1, domain),
            offset_of!(JettResourceCallResultV1, reserved),
            offset_of!(JettResourceCallResultV1, value)
        ),
        (0, 4, 8)
    );
    assert_eq!((RESOURCE_DOMAIN_OK, RESOURCE_DOMAIN_ERROR), (0, 1));
}

#[test]
fn native_registration_refuses_wire_headers_lengths_tags_and_ordinals() {
    let bytes = encode_records(&canonical());
    for end in 0..bytes.len() {
        assert!(decode(&bytes[..end]).is_err(), "prefix {end}");
    }
    let mutate = |offset, value: u32| {
        let mut altered = bytes.clone();
        altered[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
        altered
    };
    for unsupported_version in [0, NATIVE_RESOURCE_LAYOUT_WIRE_VERSION + 1, u32::MAX] {
        assert_eq!(
            decode(&mutate(8, unsupported_version)).unwrap_err(),
            ResourceLayoutError::Header
        );
    }
    assert_eq!(
        decode(&mutate(12, 1)).unwrap_err(),
        ResourceLayoutError::Reserved
    );
    assert_eq!(
        decode(&mutate(52, 1)).unwrap_err(),
        ResourceLayoutError::Reserved
    );
    assert_eq!(
        decode(&mutate(24, 4097)).unwrap_err(),
        ResourceLayoutError::WireLimit
    );
    assert_eq!(
        decode(&mutate(56, 1)).unwrap_err(),
        ResourceLayoutError::DenseOrdinal
    );
    // Header56 + two kind rows8 + hook ordinal/kind8 = first recipe tag72.
    assert_eq!(
        decode(&mutate(72, 99)).unwrap_err(),
        ResourceLayoutError::UnknownTag
    );
    let mut trailing = bytes.clone();
    trailing.push(0);
    let length = trailing.len() as u64;
    trailing[16..24].copy_from_slice(&length.to_le_bytes());
    assert_eq!(
        decode(&trailing).unwrap_err(),
        ResourceLayoutError::Trailing
    );
    assert_eq!(
        decode(&vec![0; wire::MAX_BYTES + 1]).unwrap_err(),
        ResourceLayoutError::WireLimit
    );
}

#[test]
fn native_registration_validates_unused_rows_recipes_shapes_and_active_paths() {
    let mut layout = canonical();
    layout.hooks[3].signature = 0;
    assert_eq!(
        parsed(&layout).unwrap_err(),
        ResourceLayoutError::RecipeMismatch
    );
    let mut layout = canonical();
    layout.shapes.push(NativeShape::String);
    assert_eq!(
        parsed(&layout).unwrap_err(),
        ResourceLayoutError::NonCanonical
    );
    let mut layout = canonical();
    let mut duplicate = layout.signatures[0].clone();
    duplicate.ordinal = layout.signatures.len() as u32;
    layout.signatures.push(duplicate);
    assert_eq!(
        parsed(&layout).unwrap_err(),
        ResourceLayoutError::NonCanonical
    );
    let mut layout = canonical();
    layout.shapes[8] = NativeShape::Optional { child: 8 };
    assert_eq!(
        parsed(&layout).unwrap_err(),
        ResourceLayoutError::InvalidReference
    );
    let mut layout = canonical();
    layout.shapes.push(NativeShape::Optional { child: 8 });
    assert_eq!(
        parsed(&layout).unwrap_err(),
        ResourceLayoutError::UnsupportedLayout
    );
    let mut layout = canonical();
    layout.shapes.push(NativeShape::Result { ok: 1, fail: 4 });
    assert_eq!(
        parsed(&layout).unwrap_err(),
        ResourceLayoutError::UnsupportedLayout
    );
    let mut layout = canonical();
    layout.slots[2].path = vec![NativePayloadStep::Fail];
    assert_eq!(
        parsed(&layout).unwrap_err(),
        ResourceLayoutError::PayloadPath
    );
    let mut layout = canonical();
    layout.slots[4].shape = 99;
    assert_eq!(
        parsed(&layout).unwrap_err(),
        ResourceLayoutError::InvalidReference
    );
    // Equal tuples can describe distinct original hook declarations/storage sites.
    let mut layout = canonical();
    let mut hook = layout.hooks[0].clone();
    hook.ordinal = 4;
    layout.hooks.push(hook);
    let mut slot = layout.slots[0].clone();
    slot.ordinal = 5;
    layout.slots.push(slot);
    assert!(parsed(&layout).is_ok());
}

#[test]
fn native_registration_checks_all_frame_operation_and_loan_source_relations() {
    let mut layout = canonical();
    layout.frames[1].parents = vec![NativeParent::Frame(0), NativeParent::Frame(0)];
    assert_eq!(
        parsed(&layout).unwrap_err(),
        ResourceLayoutError::NonCanonical
    );
    let mut layout = canonical();
    layout.frames[1].parents = vec![NativeParent::Frame(99)];
    assert_eq!(
        parsed(&layout).unwrap_err(),
        ResourceLayoutError::InvalidReference
    );
    let mut layout = canonical();
    layout.operations[4].operation = NativeOperation::InvokeBorrow {
        frame: 1,
        hook: 0,
        source: NativeLoanSource::ExistingBorrow { operation: 3 },
    };
    assert_eq!(
        parsed(&layout).unwrap_err(),
        ResourceLayoutError::OperationMismatch
    );
    let mut layout = canonical();
    layout.operations[6].operation = NativeOperation::EndBorrow {
        frame: 1,
        borrow: 12,
    };
    assert_eq!(
        parsed(&layout).unwrap_err(),
        ResourceLayoutError::OperationMismatch
    );
    let mut layout = canonical();
    layout.operations[14].operation = NativeOperation::InvokeDescriptor {
        hook: 0,
        signature: 0,
        target: 13,
    };
    assert_eq!(
        parsed(&layout).unwrap_err(),
        ResourceLayoutError::OperationMismatch
    );
    let mut layout = canonical();
    layout.operations[15].operation = NativeOperation::InvokeSourceFunction {
        frame: 1,
        callee: 0,
        signature: 4,
        callee_scope: 3,
        source: None,
    };
    assert_eq!(
        parsed(&layout).unwrap_err(),
        ResourceLayoutError::FrameMismatch
    );
    let mut layout = canonical();
    layout.operations[16].operation = NativeOperation::InvokeBorrow {
        frame: 1,
        hook: 1,
        source: NativeLoanSource::IncomingViewFormal {
            scope: 0,
            parameter: 0,
        },
    };
    assert_eq!(
        parsed(&layout).unwrap_err(),
        ResourceLayoutError::InvalidReference
    );
    let registry = ResourceRegistry::new();
    let registered = installed(&registry);
    let core = ResourceCustody::new(registered.program().clone(), &registry).unwrap();
    // Lawful metadata never mints a resident loan, active scope, owner or provider.
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
fn native_registration_function_frame_signatures_cannot_depend_on_instruction_site() {
    let layout = canonical();
    assert_ne!(layout.frames[0].site, layout.frames[1].site);
    assert_ne!(layout.frames[0].site, layout.frames[2].site);
    assert!(parsed(&layout).is_ok());
    for frame in [1, 2] {
        let mut altered = layout.clone();
        altered.frames[frame].signature = 0;
        assert_eq!(
            parsed(&altered).unwrap_err(),
            ResourceLayoutError::FrameMismatch
        );
    }
}

#[test]
fn native_registration_issuer_checks_context_program_kind_slot_and_exhaustion() {
    let mut registry = ResourceRegistry::new();
    let foreign = ResourceRegistry::new();
    let bytes = encode_records(&canonical());
    let mut installation = NativeLayoutInstallation::new(&registry);
    assert_eq!(
        installation.install(&foreign, &bytes).unwrap_err(),
        ResourceLayoutError::WrongContext
    );
    let registered = installation.install(&registry, &bytes).unwrap();
    assert_eq!(
        installation.install(&registry, &bytes).unwrap_err(),
        ResourceLayoutError::AlreadyInstalled
    );
    let other = installed(&registry);
    assert_ne!(registered.kind_id(0).unwrap(), other.kind_id(0).unwrap());
    assert!(matches!(
        ResourceCustody::new(registered.program().clone(), &foreign),
        Err(CustodyError::WrongContext)
    ));
    let mut core = ResourceCustody::new(registered.program().clone(), &registry).unwrap();
    let frame = core
        .begin_frame(ResourceFrameKind::Scope, ResourcePurpose::Runtime)
        .unwrap();
    let holder = core.holder(&frame, &registered.slot(0).unwrap()).unwrap();
    assert_eq!(
        core.prepare_acquisition(&other.kind(0).unwrap(), &holder)
            .unwrap_err(),
        CustodyError::WrongProgram
    );
    assert_eq!(
        core.prepare_acquisition(&registered.kind(1).unwrap(), &holder)
            .unwrap_err(),
        CustodyError::WrongKind
    );
    assert!(matches!(
        registered.slot(99),
        Err(CustodyError::UnknownSlot)
    ));
    assert!(matches!(registered.kind(99), Err(CustodyError::WrongKind)));
    let counter = AtomicU64::new(u64::from(u32::MAX));
    assert_eq!(
        issued_kinds(&counter, 2).unwrap_err(),
        ResourceLayoutError::CapacityExhausted
    );
    assert_eq!(
        issued_kinds(&counter, 1).unwrap(),
        vec![ResourceTypeId::new(u32::MAX)]
    );
    assert_eq!(
        issued_kinds(&counter, 1).unwrap_err(),
        ResourceLayoutError::CapacityExhausted
    );
    assert_eq!(core.end_frame(&frame, &mut registry).failure(), None);
}

#[test]
fn native_registration_physical_preflight_refuses_before_provider_or_publication() {
    let bytes = encode_records(&canonical());
    let mut busy = ResourceRegistry::new();
    let busy_key = busy
        .insert(
            ResourceTypeId::new(7),
            (),
            AuthorityProvenance::new(1, 1),
            |_| {},
        )
        .unwrap();
    assert_eq!(
        NativeLayoutInstallation::new(&busy)
            .install(&busy, &bytes)
            .unwrap_err(),
        ResourceLayoutError::RegistryBusy
    );
    busy.close(busy_key, ResourceTypeId::new(7)).unwrap();
    let mut registry = ResourceRegistry::new();
    let registered = installed(&registry);
    registry.shutting_down = true;
    assert_eq!(
        NativeLayoutInstallation::new(&registry)
            .install(&registry, &bytes)
            .unwrap_err(),
        ResourceLayoutError::RegistryShutdown
    );
    assert_eq!(
        registered.preflight_physical_acquisition(&mut registry),
        Err(ResourceLayoutError::RegistryShutdown)
    );
    registry.shutting_down = false;
    registry.next_creation_sequence = u64::MAX;
    assert_eq!(
        registered.preflight_physical_acquisition(&mut registry),
        Err(ResourceLayoutError::CapacityExhausted)
    );
    registry.next_creation_sequence = 1;
    registry.free_slots.push(0);
    assert_eq!(
        registered.preflight_physical_acquisition(&mut registry),
        Err(ResourceLayoutError::PhysicalStorage)
    );
    registry.free_slots.clear();
    registered
        .preflight_physical_acquisition(&mut registry)
        .unwrap();
    assert_eq!(registry.live_count(), 0);
    registry.slots.push(crate::Slot {
        generation: u64::MAX,
        entry: None,
    });
    registry.free_slots.push(0);
    assert_eq!(
        registered.preflight_physical_acquisition(&mut registry),
        Err(ResourceLayoutError::CapacityExhausted)
    );
    registry.slots[0].generation = 0;
    assert_eq!(
        registered.preflight_physical_acquisition(&mut registry),
        Err(ResourceLayoutError::PhysicalStorage)
    );
    registry.slots[0].generation = 1;
    registered
        .preflight_physical_acquisition(&mut registry)
        .unwrap();
    if let Some(index) = (u32::MAX as usize).checked_add(1) {
        assert_eq!(
            checked_physical_slot(index),
            Err(ResourceLayoutError::CapacityExhausted)
        );
    }
    let mut foreign = ResourceRegistry::new();
    assert_eq!(
        registered.preflight_physical_acquisition(&mut foreign),
        Err(ResourceLayoutError::WrongContext)
    );
}

#[test]
fn native_registration_required_purposes_cannot_acquire_from_signature_metadata() {
    for purpose in [
        ResourcePurpose::NamespaceConstant,
        ResourcePurpose::ExplicitComptime,
        ResourcePurpose::Verify,
        ResourcePurpose::Property,
    ] {
        let mut registry = ResourceRegistry::new();
        let registered = installed(&registry);
        let mut core = ResourceCustody::new(registered.program().clone(), &registry).unwrap();
        let frame = core.begin_frame(ResourceFrameKind::Scope, purpose).unwrap();
        let holder = core.holder(&frame, &registered.slot(0).unwrap()).unwrap();
        assert_eq!(
            core.prepare_acquisition(&registered.kind(0).unwrap(), &holder)
                .unwrap_err(),
            CustodyError::WrongPurpose
        );
        assert_eq!(
            (core.live_owners(), core.live_loans(), registry.live_count()),
            (0, 0, 0)
        );
        assert_eq!(core.end_frame(&frame, &mut registry).failure(), None);
    }
}

#[test]
fn native_registration_real_core_preserves_holder_generation_loans_and_once_cleanup() {
    let mut registry = ResourceRegistry::new();
    let registered = installed(&registry);
    let mut core = ResourceCustody::new(registered.program().clone(), &registry).unwrap();
    let events = Arc::new(Mutex::new(Vec::<i64>::new()));
    let root = core
        .begin_frame(ResourceFrameKind::Scope, ResourcePurpose::Runtime)
        .unwrap();
    let holder = core.holder(&root, &registered.slot(0).unwrap()).unwrap();
    registered
        .preflight_physical_acquisition(&mut registry)
        .unwrap();
    let prepared = core
        .prepare_acquisition(&registered.kind(0).unwrap(), &holder)
        .unwrap();
    let captured = events.clone();
    let mut owner = core
        .commit_acquisition(
            &mut registry,
            prepared,
            AuthorityProvenance::new(5, 7),
            501i64,
            move |value| captured.lock().unwrap().push(value),
        )
        .unwrap();
    let stale = OwnedResourceToken {
        core: owner.core,
        owner: owner.owner,
        holder_generation: owner.holder_generation,
    };
    let operation = core
        .begin_frame(ResourceFrameKind::Operation, ResourcePurpose::Runtime)
        .unwrap();
    let other_holder = core
        .holder(&operation, &registered.slot(1).unwrap())
        .unwrap();
    registered
        .preflight_physical_acquisition(&mut registry)
        .unwrap();
    let prepared = core
        .prepare_acquisition(&registered.kind(0).unwrap(), &other_holder)
        .unwrap();
    let captured = events.clone();
    let _other = core
        .commit_acquisition(
            &mut registry,
            prepared,
            AuthorityProvenance::new(5, 7),
            502i64,
            move |value| captured.lock().unwrap().push(value),
        )
        .unwrap();
    let destination = core
        .holder(&operation, &registered.slot(2).unwrap())
        .unwrap();
    let mut loan = core.begin_borrow(&owner, &operation, &registry).unwrap();
    assert!(core.validate_borrowed(&loan, &registry).is_ok());
    assert_eq!(
        core.transfer(&mut owner, &destination, &registry),
        Err(CustodyError::ActiveBorrow)
    );
    core.end_borrow(&mut loan).unwrap();
    core.transfer(&mut owner, &destination, &registry).unwrap();
    assert_eq!(
        core.validate_owned(&stale, &registry),
        Err(CustodyError::StaleHolderGeneration)
    );
    assert_eq!(core.end_frame(&operation, &mut registry).failure(), None);
    assert_eq!(*events.lock().unwrap(), [501, 502]);
    assert_eq!(core.end_frame(&root, &mut registry).failure(), None);
    assert_eq!(*events.lock().unwrap(), [501, 502]);
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
fn native_registration_real_core_continues_cleanup_and_preserves_cleanup_precedence() {
    let mut registry = ResourceRegistry::new();
    let registered = installed(&registry);
    let mut core = ResourceCustody::new(registered.program().clone(), &registry).unwrap();
    let events = Arc::new(Mutex::new(Vec::<i64>::new()));
    let root = core
        .begin_frame(ResourceFrameKind::Scope, ResourcePurpose::Runtime)
        .unwrap();
    let holder = core.holder(&root, &registered.slot(0).unwrap()).unwrap();
    registered
        .preflight_physical_acquisition(&mut registry)
        .unwrap();
    let prepared = core
        .prepare_acquisition(&registered.kind(0).unwrap(), &holder)
        .unwrap();
    let captured = events.clone();
    let _first = core
        .commit_acquisition(
            &mut registry,
            prepared,
            AuthorityProvenance::new(5, 7),
            601i64,
            move |label| captured.lock().unwrap().push(label),
        )
        .unwrap();
    let operation = core
        .begin_frame(ResourceFrameKind::Operation, ResourcePurpose::Runtime)
        .unwrap();
    let holder = core
        .holder(&operation, &registered.slot(1).unwrap())
        .unwrap();
    registered
        .preflight_physical_acquisition(&mut registry)
        .unwrap();
    let prepared = core
        .prepare_acquisition(&registered.kind(0).unwrap(), &holder)
        .unwrap();
    let captured = events.clone();
    let _second = core
        .commit_acquisition(
            &mut registry,
            prepared,
            AuthorityProvenance::new(5, 7),
            602i64,
            move |label| {
                captured.lock().unwrap().push(label);
                panic!("selected registered-layout finalizer failure");
            },
        )
        .unwrap();
    let cleanup = core.unwind_all(&mut registry);
    assert_eq!(
        complete_resource(
            ResourceBodyOutcome::<(), &str>::Completed(Err("selected body failure")),
            cleanup
        ),
        Err(ResourceCompletionFailure::Cleanup(
            ResourceCleanupFailure::FinalizerPanic
        ))
    );
    assert_eq!(*events.lock().unwrap(), [602, 601]);
    assert_eq!(
        (
            core.live_owners(),
            core.live_loans(),
            core.active_frames(),
            registry.live_count()
        ),
        (0, 0, 0, 0)
    );
    assert_eq!(core.unwind_all(&mut registry).failure(), None);
    assert_eq!(*events.lock().unwrap(), [602, 601]);
}
