//! Registry-backed ownership protocol, independent of compiler/interpreter data.
//! Native ABI transport remains a separate boundary from layout registration.

use std::any::Any;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::{
    AuthorityProvenance, RegistryError, ResourceKey, ResourceRegistry, ResourceTypeId,
    discard_panic_payload,
};

static NEXT_CUSTODY: AtomicU64 = AtomicU64::new(1);

#[path = "resource_custody/registration.rs"]
mod registration;
pub(crate) use registration::{
    JettResourceCallResultV1, NATIVE_RESOURCE_LAYOUT_WIRE_VERSION, NativeAccess, NativeFormal,
    NativeFrame, NativeFrameRole, NativeHook, NativeLayoutInstallation, NativeLoanSource,
    NativeOperation, NativeOperationRecord, NativeParent, NativePayloadStep, NativePosition,
    NativeRecipe, NativeShape, NativeSignature, NativeSite, NativeSlot, NativeSourceEffect,
    NativeSourceFormal, NativeSourceInvocation, NativeSourceResult, NativeSourceSyntax,
    NativeSourceValue, RESOURCE_DOMAIN_ERROR, RESOURCE_DOMAIN_OK, RegisteredNativeLayout,
    ResourceLayoutError,
};

#[derive(Debug)]
struct ProgramLayout {
    context: u64,
    kinds: Vec<ResourceTypeId>,
    slots: Vec<usize>,
}

/// Only a constructor-owned registration can create this immutable identity.
/// The private registration child is the sole production issuer; tests also
/// have a private issuer. Neither accepts a registry key as an owner.
#[derive(Debug, Clone)]
pub(crate) struct RegisteredResourceProgram(Arc<ProgramLayout>);

#[derive(Debug, Clone)]
pub(crate) struct RegisteredResourceKind {
    program: Arc<ProgramLayout>,
    index: usize,
}

#[derive(Debug, Clone)]
pub(crate) struct RegisteredOwnerSlot {
    program: Arc<ProgramLayout>,
    index: usize,
}

impl RegisteredResourceProgram {
    pub(crate) fn kind(&self, index: usize) -> Result<RegisteredResourceKind, CustodyError> {
        self.0.kinds.get(index).ok_or(CustodyError::WrongKind)?;
        Ok(RegisteredResourceKind {
            program: self.0.clone(),
            index,
        })
    }

    pub(crate) fn slot(&self, index: usize) -> Result<RegisteredOwnerSlot, CustodyError> {
        self.0.slots.get(index).ok_or(CustodyError::UnknownSlot)?;
        Ok(RegisteredOwnerSlot {
            program: self.0.clone(),
            index,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ResourcePurpose {
    Runtime,
    NamespaceConstant,
    ExplicitComptime,
    Verify,
    Property,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ResourceFrameKind {
    Scope,
    Operation,
    Return,
}

#[derive(Debug)]
pub(crate) struct ResourceFrameToken {
    core: u64,
    index: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Holder {
    frame: usize,
    slot: usize,
}

#[derive(Debug)]
pub(crate) struct ResourceHolderToken {
    core: u64,
    holder: Holder,
}

/// Neither Clone/Copy nor a finalizing Rust Drop. The ledger owns cleanup.
#[derive(Debug)]
pub(crate) struct OwnedResourceToken {
    core: u64,
    owner: usize,
    holder_generation: u64,
}

#[derive(Debug)]
pub(crate) struct BorrowedResourceToken {
    core: u64,
    loan: usize,
}

/// Preflight is unoccupied metadata, not a provider grant or resource owner.
#[derive(Debug)]
pub(crate) struct PreparedResourceAcquisition {
    core: u64,
    holder: Holder,
    kind: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CustodyError {
    WrongContext,
    WrongProgram,
    WrongKind,
    UnknownSlot,
    InvalidFrame,
    OutOfOrderFrame,
    InvalidOwner,
    StaleHolderGeneration,
    InvalidLoan,
    ActiveBorrow,
    OccupiedHolder,
    WrongPurpose,
    CapacityExhausted,
    UnownedRegistry,
    Registry(RegistryError),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ResourceCleanupFailure {
    Custody(CustodyError),
    FinalizerPanic,
}

#[derive(Debug, Default, PartialEq, Eq)]
pub(crate) struct ResourceCleanupOutcome {
    first: Option<ResourceCleanupFailure>,
}

impl ResourceCleanupOutcome {
    pub(crate) fn failure(&self) -> Option<ResourceCleanupFailure> {
        self.first
    }
    fn fail(&mut self, failure: ResourceCleanupFailure) {
        self.first.get_or_insert(failure);
    }
    fn merge(&mut self, other: Self) {
        if let Some(failure) = other.first {
            self.fail(failure);
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum ResourceTransitionFailure {
    Rejected(CustodyError),
    Cleanup(ResourceCleanupFailure),
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum ResourceBodyOutcome<T, E> {
    Completed(Result<T, E>),
    HostPanic,
}

impl<T, E> ResourceBodyOutcome<T, E> {
    pub(crate) fn caught(outcome: std::thread::Result<Result<T, E>>) -> Self {
        match outcome {
            Ok(result) => Self::Completed(result),
            Err(payload) => {
                discard_panic_payload(payload);
                Self::HostPanic
            }
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum ResourceCompletionFailure<E> {
    Body(E),
    BodyPanic,
    Cleanup(ResourceCleanupFailure),
}

pub(crate) fn complete_resource<T, E>(
    body: ResourceBodyOutcome<T, E>,
    cleanup: ResourceCleanupOutcome,
) -> Result<T, ResourceCompletionFailure<E>> {
    if let Some(failure) = cleanup.first {
        return Err(ResourceCompletionFailure::Cleanup(failure));
    }
    match body {
        ResourceBodyOutcome::Completed(Ok(value)) => Ok(value),
        ResourceBodyOutcome::Completed(Err(error)) => Err(ResourceCompletionFailure::Body(error)),
        ResourceBodyOutcome::HostPanic => Err(ResourceCompletionFailure::BodyPanic),
    }
}

#[derive(Clone, Copy)]
struct Acquisition {
    owner: usize,
    holder: Holder,
    generation: u64,
}

struct Frame {
    kind: ResourceFrameKind,
    purpose: ResourcePurpose,
    live: bool,
    acquisitions: Vec<Acquisition>,
    loans: Vec<usize>,
}

struct Owner {
    kind: usize,
    key: ResourceKey,
    authority: AuthorityProvenance,
    holder: Holder,
    generation: u64,
    live: bool,
}

struct Loan {
    owner: usize,
    generation: u64,
    frame: usize,
    live: bool,
}

pub(crate) struct ResourceCustody {
    id: u64,
    program: RegisteredResourceProgram,
    frames: Vec<Frame>,
    active: Vec<usize>,
    owners: Vec<Owner>,
    loans: Vec<Loan>,
}

impl ResourceCustody {
    pub(crate) fn new(
        program: RegisteredResourceProgram,
        registry: &ResourceRegistry,
    ) -> Result<Self, CustodyError> {
        if program.0.context != registry.context_id {
            return Err(CustodyError::WrongContext);
        }
        if registry.live_count() != 0 {
            return Err(CustodyError::UnownedRegistry);
        }
        let id = NEXT_CUSTODY
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |next| {
                next.checked_add(1)
            })
            .map_err(|_| CustodyError::CapacityExhausted)?;
        Ok(Self {
            id,
            program,
            frames: Vec::new(),
            active: Vec::new(),
            owners: Vec::new(),
            loans: Vec::new(),
        })
    }

    fn registry(&self, registry: &ResourceRegistry) -> Result<(), CustodyError> {
        if self.program.0.context == registry.context_id {
            Ok(())
        } else {
            Err(CustodyError::WrongContext)
        }
    }

    fn frame(&self, index: usize) -> Result<&Frame, CustodyError> {
        self.frames
            .get(index)
            .filter(|frame| frame.live)
            .ok_or(CustodyError::InvalidFrame)
    }

    fn frame_token(&self, token: &ResourceFrameToken) -> Result<usize, CustodyError> {
        if token.core != self.id {
            return Err(CustodyError::WrongProgram);
        }
        self.frame(token.index)?;
        Ok(token.index)
    }

    fn holder_token(&self, token: &ResourceHolderToken) -> Result<Holder, CustodyError> {
        if token.core != self.id {
            return Err(CustodyError::WrongProgram);
        }
        self.frame(token.holder.frame)?;
        self.program
            .0
            .slots
            .get(token.holder.slot)
            .ok_or(CustodyError::UnknownSlot)?;
        Ok(token.holder)
    }

    fn available(&self, holder: Holder) -> Result<(), CustodyError> {
        if self
            .owners
            .iter()
            .any(|owner| owner.live && owner.holder == holder)
        {
            return Err(CustodyError::OccupiedHolder);
        }
        Ok(())
    }

    fn runtime_frame(&self, frame: usize) -> Result<(), CustodyError> {
        if self.frame(frame)?.purpose == ResourcePurpose::Runtime {
            Ok(())
        } else {
            Err(CustodyError::WrongPurpose)
        }
    }

    fn kind(&self, token: &RegisteredResourceKind) -> Result<usize, CustodyError> {
        if !Arc::ptr_eq(&token.program, &self.program.0) {
            return Err(CustodyError::WrongProgram);
        }
        self.program
            .0
            .kinds
            .get(token.index)
            .ok_or(CustodyError::WrongKind)?;
        Ok(token.index)
    }

    pub(crate) fn begin_frame(
        &mut self,
        kind: ResourceFrameKind,
        purpose: ResourcePurpose,
    ) -> Result<ResourceFrameToken, CustodyError> {
        if let Some(parent) = self.active.last() {
            if self.frame(*parent)?.purpose != purpose {
                return Err(CustodyError::WrongPurpose);
            }
        } else if self.live_owners() != 0 || self.live_loans() != 0 {
            return Err(CustodyError::InvalidFrame);
        }
        self.frames
            .try_reserve(1)
            .map_err(|_| CustodyError::CapacityExhausted)?;
        self.active
            .try_reserve(1)
            .map_err(|_| CustodyError::CapacityExhausted)?;
        let index = self.frames.len();
        self.frames.push(Frame {
            kind,
            purpose,
            live: true,
            acquisitions: Vec::new(),
            loans: Vec::new(),
        });
        self.active.push(index);
        Ok(ResourceFrameToken {
            core: self.id,
            index,
        })
    }

    pub(crate) fn holder(
        &self,
        frame: &ResourceFrameToken,
        slot: &RegisteredOwnerSlot,
    ) -> Result<ResourceHolderToken, CustodyError> {
        let frame = self.frame_token(frame)?;
        if !Arc::ptr_eq(&slot.program, &self.program.0) {
            return Err(CustodyError::WrongProgram);
        }
        self.program
            .0
            .slots
            .get(slot.index)
            .ok_or(CustodyError::UnknownSlot)?;
        Ok(ResourceHolderToken {
            core: self.id,
            holder: Holder {
                frame,
                slot: slot.index,
            },
        })
    }

    fn acquisition_storage(&mut self, holder: Holder, kind: usize) -> Result<(), CustodyError> {
        self.runtime_frame(holder.frame)?;
        if self.program.0.slots.get(holder.slot) != Some(&kind) {
            return Err(CustodyError::WrongKind);
        }
        self.available(holder)?;
        self.owners
            .try_reserve(1)
            .map_err(|_| CustodyError::CapacityExhausted)?;
        self.frames[holder.frame]
            .acquisitions
            .try_reserve(1)
            .map_err(|_| CustodyError::CapacityExhausted)?;
        Ok(())
    }

    pub(crate) fn prepare_acquisition(
        &mut self,
        kind: &RegisteredResourceKind,
        destination: &ResourceHolderToken,
    ) -> Result<PreparedResourceAcquisition, CustodyError> {
        let kind = self.kind(kind)?;
        let holder = self.holder_token(destination)?;
        self.acquisition_storage(holder, kind)?;
        Ok(PreparedResourceAcquisition {
            core: self.id,
            holder,
            kind,
        })
    }

    pub(crate) fn commit_acquisition<T, F>(
        &mut self,
        registry: &mut ResourceRegistry,
        prepared: PreparedResourceAcquisition,
        authority: AuthorityProvenance,
        payload: T,
        finalizer: F,
    ) -> Result<OwnedResourceToken, ResourceTransitionFailure>
    where
        T: Any + Send,
        F: FnOnce(T) + Send + 'static,
    {
        let checked = (|| {
            self.registry(registry)?;
            if prepared.core != self.id {
                return Err(CustodyError::WrongProgram);
            }
            self.program
                .0
                .kinds
                .get(prepared.kind)
                .ok_or(CustodyError::WrongKind)?;
            self.frame(prepared.holder.frame)?;
            let next_slot = registry
                .free_slots
                .last()
                .copied()
                .unwrap_or(registry.slots.len());
            u32::try_from(next_slot).map_err(|_| CustodyError::CapacityExhausted)?;
            self.acquisition_storage(prepared.holder, prepared.kind)
        })();
        if let Err(error) = checked {
            let cleanup = catch_unwind(AssertUnwindSafe(|| finalizer(payload)));
            return Err(match cleanup {
                Ok(()) => ResourceTransitionFailure::Rejected(error),
                Err(panic) => {
                    discard_panic_payload(panic);
                    ResourceTransitionFailure::Cleanup(ResourceCleanupFailure::FinalizerPanic)
                }
            });
        }
        let registry_type = self.program.0.kinds[prepared.kind];
        let inserted = catch_unwind(AssertUnwindSafe(|| {
            registry.insert(registry_type, payload, authority, finalizer)
        }));
        let key = match inserted {
            Ok(Ok(key)) => key,
            Ok(Err(error)) => {
                return Err(ResourceTransitionFailure::Rejected(CustodyError::Registry(
                    error,
                )));
            }
            Err(panic) => {
                discard_panic_payload(panic);
                return Err(ResourceTransitionFailure::Cleanup(
                    ResourceCleanupFailure::FinalizerPanic,
                ));
            }
        };
        // All ledger capacity is reserved before physical insertion. No fallible
        // publication remains between insertion and the logged owning record.
        let owner = self.owners.len();
        self.owners.push(Owner {
            kind: prepared.kind,
            key,
            authority,
            holder: prepared.holder,
            generation: 1,
            live: true,
        });
        self.frames[prepared.holder.frame]
            .acquisitions
            .push(Acquisition {
                owner,
                holder: prepared.holder,
                generation: 1,
            });
        Ok(OwnedResourceToken {
            core: self.id,
            owner,
            holder_generation: 1,
        })
    }

    fn owner(&self, token: &OwnedResourceToken) -> Result<&Owner, CustodyError> {
        if token.core != self.id {
            return Err(CustodyError::WrongProgram);
        }
        let owner = self
            .owners
            .get(token.owner)
            .filter(|owner| owner.live)
            .ok_or(CustodyError::InvalidOwner)?;
        if owner.generation != token.holder_generation {
            return Err(CustodyError::StaleHolderGeneration);
        }
        self.frame(owner.holder.frame)?;
        if self.program.0.slots.get(owner.holder.slot) != Some(&owner.kind) {
            return Err(CustodyError::WrongKind);
        }
        Ok(owner)
    }

    fn validate_registry_owner(
        &self,
        owner: &Owner,
        registry: &ResourceRegistry,
    ) -> Result<ResourceKey, CustodyError> {
        self.registry(registry)?;
        let registry_type = *self
            .program
            .0
            .kinds
            .get(owner.kind)
            .ok_or(CustodyError::WrongKind)?;
        let index = registry
            .validate_slot(owner.key, registry_type)
            .map_err(CustodyError::Registry)?;
        let entry = registry.slots[index]
            .entry
            .as_ref()
            .ok_or(CustodyError::Registry(RegistryError::Retired))?;
        if entry.authority != owner.authority {
            return Err(CustodyError::Registry(RegistryError::AuthorityMismatch));
        }
        Ok(owner.key)
    }

    fn borrowed(&self, owner: usize) -> bool {
        self.loans
            .iter()
            .any(|loan| loan.live && loan.owner == owner)
    }

    pub(crate) fn validate_owned(
        &self,
        owner: &OwnedResourceToken,
        registry: &ResourceRegistry,
    ) -> Result<ResourceKey, CustodyError> {
        self.validate_registry_owner(self.owner(owner)?, registry)
    }

    fn checked_transfer(
        &self,
        token: &OwnedResourceToken,
        destination: &ResourceHolderToken,
        registry: &ResourceRegistry,
    ) -> Result<(Holder, u64), CustodyError> {
        let holder = self.holder_token(destination)?;
        self.runtime_frame(holder.frame)?;
        let owner = self.owner(token)?;
        self.validate_registry_owner(owner, registry)?;
        if self.borrowed(token.owner) {
            return Err(CustodyError::ActiveBorrow);
        }
        if self.program.0.slots.get(holder.slot) != Some(&owner.kind) {
            return Err(CustodyError::WrongKind);
        }
        self.available(holder)?;
        let generation = owner
            .generation
            .checked_add(1)
            .ok_or(CustodyError::CapacityExhausted)?;
        Ok((holder, generation))
    }

    /// A Source activation reserves its entire exact formal handoff before any
    /// owner moves. References are existing live tokens and registered metadata;
    /// this creates neither ownership nor a reusable transfer permission.
    pub(crate) fn preflight_transfer_batch(
        &mut self,
        frame: &ResourceFrameToken,
        transfers: &[(&OwnedResourceToken, &RegisteredOwnerSlot)],
        registry: &ResourceRegistry,
    ) -> Result<(), CustodyError> {
        let frame_index = self.frame_token(frame)?;
        let mut checked: Vec<(usize, Holder)> = Vec::new();
        checked
            .try_reserve_exact(transfers.len())
            .map_err(|_| CustodyError::CapacityExhausted)?;
        for &(token, slot) in transfers {
            let destination = self.holder(frame, slot)?;
            let (holder, _) = self.checked_transfer(token, &destination, registry)?;
            if checked
                .iter()
                .any(|(owner, previous)| *owner == token.owner || *previous == holder)
            {
                return Err(CustodyError::OccupiedHolder);
            }
            checked.push((token.owner, holder));
        }
        self.frames[frame_index]
            .acquisitions
            .try_reserve(transfers.len())
            .map_err(|_| CustodyError::CapacityExhausted)?;
        Ok(())
    }

    pub(crate) fn transfer(
        &mut self,
        token: &mut OwnedResourceToken,
        destination: &ResourceHolderToken,
        registry: &ResourceRegistry,
    ) -> Result<(), CustodyError> {
        let (holder, generation) = self.checked_transfer(token, destination, registry)?;
        self.frames[holder.frame]
            .acquisitions
            .try_reserve(1)
            .map_err(|_| CustodyError::CapacityExhausted)?;
        self.owners[token.owner].holder = holder;
        self.owners[token.owner].generation = generation;
        self.frames[holder.frame].acquisitions.push(Acquisition {
            owner: token.owner,
            holder,
            generation,
        });
        token.holder_generation = generation;
        Ok(())
    }

    pub(crate) fn begin_borrow(
        &mut self,
        token: &OwnedResourceToken,
        frame: &ResourceFrameToken,
        registry: &ResourceRegistry,
    ) -> Result<BorrowedResourceToken, CustodyError> {
        let frame = self.frame_token(frame)?;
        self.runtime_frame(frame)?;
        let owner = self.owner(token)?;
        self.validate_registry_owner(owner, registry)?;
        let owner_position = self
            .active
            .iter()
            .position(|value| *value == owner.holder.frame)
            .ok_or(CustodyError::InvalidFrame)?;
        let borrower_position = self
            .active
            .iter()
            .position(|value| *value == frame)
            .ok_or(CustodyError::InvalidFrame)?;
        if borrower_position < owner_position {
            return Err(CustodyError::InvalidLoan);
        }
        let generation = owner.generation;
        self.loans
            .try_reserve(1)
            .map_err(|_| CustodyError::CapacityExhausted)?;
        self.frames[frame]
            .loans
            .try_reserve(1)
            .map_err(|_| CustodyError::CapacityExhausted)?;
        let loan = self.loans.len();
        self.loans.push(Loan {
            owner: token.owner,
            generation,
            frame,
            live: true,
        });
        self.frames[frame].loans.push(loan);
        Ok(BorrowedResourceToken {
            core: self.id,
            loan,
        })
    }

    fn loan(&self, token: &BorrowedResourceToken) -> Result<&Loan, CustodyError> {
        if token.core != self.id {
            return Err(CustodyError::WrongProgram);
        }
        let loan = self
            .loans
            .get(token.loan)
            .filter(|loan| loan.live)
            .ok_or(CustodyError::InvalidLoan)?;
        self.frame(loan.frame)?;
        let owner = self
            .owners
            .get(loan.owner)
            .filter(|owner| owner.live && owner.generation == loan.generation)
            .ok_or(CustodyError::InvalidLoan)?;
        self.frame(owner.holder.frame)?;
        Ok(loan)
    }

    pub(crate) fn validate_borrowed(
        &self,
        token: &BorrowedResourceToken,
        registry: &ResourceRegistry,
    ) -> Result<ResourceKey, CustodyError> {
        let loan = self.loan(token)?;
        self.validate_registry_owner(&self.owners[loan.owner], registry)
    }

    pub(crate) fn end_borrow(
        &mut self,
        token: &mut BorrowedResourceToken,
    ) -> Result<(), CustodyError> {
        self.loan(token)?;
        self.loans[token.loan].live = false;
        Ok(())
    }

    fn finalize(
        &mut self,
        owner: usize,
        registry: &mut ResourceRegistry,
    ) -> ResourceCleanupOutcome {
        let mut outcome = ResourceCleanupOutcome::default();
        if self.borrowed(owner) {
            outcome.fail(ResourceCleanupFailure::Custody(CustodyError::ActiveBorrow));
            return outcome;
        }
        let record = &self.owners[owner];
        if let Err(error) = self.validate_registry_owner(record, registry) {
            // An externally retired/stale entry is already unavailable. Retire
            // its obligation too, never invoke a different generation's finalizer.
            if matches!(
                error,
                CustodyError::Registry(
                    RegistryError::UnknownSlot
                        | RegistryError::StaleGeneration
                        | RegistryError::Retired
                )
            ) {
                self.owners[owner].live = false;
            }
            outcome.fail(ResourceCleanupFailure::Custody(error));
            return outcome;
        }
        let key = record.key;
        let registry_type = self.program.0.kinds[record.kind];
        self.owners[owner].live = false;
        match catch_unwind(AssertUnwindSafe(|| registry.close(key, registry_type))) {
            Ok(Ok(())) => {}
            Ok(Err(error)) => outcome.fail(ResourceCleanupFailure::Custody(
                CustodyError::Registry(error),
            )),
            Err(panic) => {
                discard_panic_payload(panic);
                outcome.fail(ResourceCleanupFailure::FinalizerPanic);
            }
        }
        outcome
    }

    pub(crate) fn close(
        &mut self,
        token: &mut OwnedResourceToken,
        registry: &mut ResourceRegistry,
    ) -> ResourceCleanupOutcome {
        let mut outcome = ResourceCleanupOutcome::default();
        let checked = self
            .registry(registry)
            .and_then(|()| self.owner(token))
            .and_then(|owner| {
                self.runtime_frame(owner.holder.frame)?;
                self.validate_registry_owner(owner, registry).map(|_| ())
            });
        if let Err(error) = checked {
            outcome.fail(ResourceCleanupFailure::Custody(error));
            return outcome;
        }
        self.finalize(token.owner, registry)
    }

    fn end_index(
        &mut self,
        frame: usize,
        registry: &mut ResourceRegistry,
    ) -> ResourceCleanupOutcome {
        let mut outcome = ResourceCleanupOutcome::default();
        let checked = self
            .registry(registry)
            .and_then(|()| self.frame(frame).map(|_| ()))
            .and_then(|()| {
                if self.active.last() == Some(&frame) {
                    Ok(())
                } else {
                    Err(CustodyError::OutOfOrderFrame)
                }
            });
        if let Err(error) = checked {
            outcome.fail(ResourceCleanupFailure::Custody(error));
            return outcome;
        }
        for loan in &self.frames[frame].loans {
            self.loans[*loan].live = false;
        }
        let mut acquisitions = std::mem::take(&mut self.frames[frame].acquisitions);
        for acquisition in acquisitions.iter().rev() {
            let owner = &self.owners[acquisition.owner];
            if !owner.live
                || owner.holder != acquisition.holder
                || owner.generation != acquisition.generation
            {
                continue;
            }
            outcome.merge(self.finalize(acquisition.owner, registry));
        }
        acquisitions.retain(|acquisition| {
            let owner = &self.owners[acquisition.owner];
            owner.live
                && owner.holder == acquisition.holder
                && owner.generation == acquisition.generation
        });
        if acquisitions.is_empty() {
            self.frames[frame].live = false;
            self.active.pop();
        } else {
            // A malformed longer-lived lease must not turn refusal into frame
            // retirement. Retain its acquisitions for a later valid completion.
            self.frames[frame].acquisitions = acquisitions;
        }
        outcome
    }

    pub(crate) fn end_frame(
        &mut self,
        frame: &ResourceFrameToken,
        registry: &mut ResourceRegistry,
    ) -> ResourceCleanupOutcome {
        match self.frame_token(frame) {
            Ok(index) => self.end_index(index, registry),
            Err(error) => ResourceCleanupOutcome {
                first: Some(ResourceCleanupFailure::Custody(error)),
            },
        }
    }

    pub(crate) fn unwind_all(&mut self, registry: &mut ResourceRegistry) -> ResourceCleanupOutcome {
        let mut outcome = ResourceCleanupOutcome::default();
        while let Some(frame) = self.active.last().copied() {
            outcome.merge(self.end_index(frame, registry));
            if self.active.last() == Some(&frame) {
                break;
            }
        }
        outcome
    }

    pub(crate) fn live_owners(&self) -> usize {
        self.owners.iter().filter(|owner| owner.live).count()
    }
    pub(crate) fn live_loans(&self) -> usize {
        self.loans.iter().filter(|loan| loan.live).count()
    }
    pub(crate) fn active_frames(&self) -> usize {
        self.active.len()
    }
}

#[cfg(test)]
#[path = "resource_custody/tests.rs"]
mod tests;
