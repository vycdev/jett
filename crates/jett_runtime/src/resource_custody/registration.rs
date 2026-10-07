//! Sole native core metadata issuer. Registration never installs a provider or owner.

use super::{
    CustodyError, ProgramLayout, RegisteredOwnerSlot, RegisteredResourceKind,
    RegisteredResourceProgram,
};
use crate::{ResourceRegistry, ResourceTypeId};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

#[path = "registration/schema.rs"]
mod schema;
#[path = "registration/source_validation.rs"]
mod source_validation;
#[path = "registration/validation.rs"]
mod validation;
#[path = "registration/wire.rs"]
mod wire;
pub(crate) use schema::{
    NativeAccess, NativeFormal, NativeFrame, NativeFrameRole, NativeHook, NativeLoanSource,
    NativeOperation, NativeOperationRecord, NativeParent, NativePayloadStep, NativePosition,
    NativeRecipe, NativeShape, NativeSignature, NativeSite, NativeSlot, NativeSourceEffect,
    NativeSourceFormal, NativeSourceInvocation, NativeSourceResult, NativeSourceSyntax,
    NativeSourceValue, NativeSumLoanSource,
};

pub(crate) const NATIVE_RESOURCE_LAYOUT_WIRE_VERSION: u32 = 2;

static NEXT_NATIVE_KIND: AtomicU64 = AtomicU64::new(1);

pub(crate) const RESOURCE_DOMAIN_OK: u32 = 0;
pub(crate) const RESOURCE_DOMAIN_ERROR: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ResourceLayoutError {
    Header,
    Reserved,
    Length,
    Truncated,
    Trailing,
    WireLimit,
    UnknownTag,
    DenseOrdinal,
    InvalidReference,
    NonCanonical,
    UnsupportedLayout,
    RecipeMismatch,
    PayloadPath,
    ShapeMismatch,
    FrameMismatch,
    OperationMismatch,
    CapacityExhausted,
    AlreadyInstalled,
    WrongContext,
    RegistryBusy,
    RegistryShutdown,
    PhysicalStorage,
}
impl std::fmt::Display for ResourceLayoutError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "native Resource layout refused: {self:?}")
    }
}
impl std::error::Error for ResourceLayoutError {}

/// Fixed data record only. Its value is not a registry key or core token.
#[repr(C)]
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub(crate) struct JettResourceCallResultV1 {
    pub(crate) domain: u32,
    pub(crate) reserved: u32,
    pub(crate) value: u64,
}

#[derive(Debug)]
pub(crate) struct RegisteredNativeLayout {
    core: RegisteredResourceProgram,
    wire: schema::WireLayout,
}
impl RegisteredNativeLayout {
    pub(crate) fn wire_version(&self) -> u32 {
        self.wire.version
    }
    pub(crate) fn program(&self) -> &RegisteredResourceProgram {
        &self.core
    }
    pub(crate) fn kind(&self, ordinal: u32) -> Result<RegisteredResourceKind, CustodyError> {
        self.core.kind(ordinal as usize)
    }
    pub(crate) fn slot(&self, ordinal: u32) -> Result<RegisteredOwnerSlot, CustodyError> {
        self.core.slot(ordinal as usize)
    }
    pub(crate) fn kind_id(&self, ordinal: u32) -> Result<ResourceTypeId, ResourceLayoutError> {
        self.core
            .0
            .kinds
            .get(ordinal as usize)
            .copied()
            .ok_or(ResourceLayoutError::InvalidReference)
    }
    pub(crate) fn slot_kind(&self, ordinal: u32) -> Result<u32, ResourceLayoutError> {
        self.core
            .0
            .slots
            .get(ordinal as usize)
            .map(|kind| *kind as u32)
            .ok_or(ResourceLayoutError::InvalidReference)
    }
    pub(crate) fn kinds_len(&self) -> usize {
        self.core.0.kinds.len()
    }
    pub(crate) fn hooks(&self) -> &[NativeHook] {
        &self.wire.hooks
    }
    pub(crate) fn signatures(&self) -> &[NativeSignature] {
        &self.wire.signatures
    }
    pub(crate) fn shapes(&self) -> &[NativeShape] {
        &self.wire.shapes
    }
    pub(crate) fn frames(&self) -> &[NativeFrame] {
        &self.wire.frames
    }
    pub(crate) fn slots(&self) -> &[NativeSlot] {
        &self.wire.slots
    }
    pub(crate) fn operations(&self) -> &[NativeOperationRecord] {
        &self.wire.operations
    }

    /// Before a provider effect; core and future opaque-token table preflights
    /// remain independently mandatory. No payload/grant/preparation token is returned.
    pub(crate) fn preflight_physical_acquisition(
        &self,
        registry: &mut ResourceRegistry,
    ) -> Result<(), ResourceLayoutError> {
        if registry.context_id != self.core.0.context {
            return Err(ResourceLayoutError::WrongContext);
        }
        if registry.shutting_down {
            return Err(ResourceLayoutError::RegistryShutdown);
        }
        if registry.next_creation_sequence == u64::MAX {
            return Err(ResourceLayoutError::CapacityExhausted);
        }
        if let Some(&index) = registry.free_slots.last() {
            checked_physical_slot(index)?;
            let slot = registry
                .slots
                .get(index)
                .ok_or(ResourceLayoutError::PhysicalStorage)?;
            if slot.entry.is_some() || slot.generation == 0 {
                return Err(ResourceLayoutError::PhysicalStorage);
            }
            if slot.generation == u64::MAX {
                return Err(ResourceLayoutError::CapacityExhausted);
            }
        } else {
            checked_physical_slot(registry.slots.len())?;
            registry
                .slots
                .try_reserve(1)
                .map_err(|_| ResourceLayoutError::CapacityExhausted)?;
        }
        Ok(())
    }
}

fn checked_physical_slot(index: usize) -> Result<(), ResourceLayoutError> {
    u32::try_from(index)
        .map(|_| ())
        .map_err(|_| ResourceLayoutError::CapacityExhausted)
}

fn issued_kinds(
    counter: &AtomicU64,
    count: usize,
) -> Result<Vec<ResourceTypeId>, ResourceLayoutError> {
    let mut kinds = wire::storage(count)?;
    if count == 0 {
        return Ok(kinds);
    }
    let count = u64::try_from(count).map_err(|_| ResourceLayoutError::CapacityExhausted)?;
    let first = counter
        .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |next| {
            if next == 0 {
                return None;
            }
            next.checked_add(count)
                .filter(|end| *end <= u64::from(u32::MAX) + 1)
        })
        .map_err(|_| ResourceLayoutError::CapacityExhausted)?;
    for offset in 0..count {
        kinds.push(ResourceTypeId::new((first + offset) as u32));
    }
    Ok(kinds)
}

/// One stationary runtime context owns this holder. Fields cannot be assembled
/// from independently supplied program/kind/slot metadata.
pub(crate) struct NativeLayoutInstallation {
    context: u64,
    installed: Option<Arc<RegisteredNativeLayout>>,
}
impl NativeLayoutInstallation {
    pub(crate) fn new(registry: &ResourceRegistry) -> Self {
        Self {
            context: registry.context_id,
            installed: None,
        }
    }
    pub(crate) fn installed(&self) -> Option<&Arc<RegisteredNativeLayout>> {
        self.installed.as_ref()
    }
    pub(crate) fn install(
        &mut self,
        registry: &ResourceRegistry,
        bytes: &[u8],
    ) -> Result<Arc<RegisteredNativeLayout>, ResourceLayoutError> {
        if registry.context_id != self.context {
            return Err(ResourceLayoutError::WrongContext);
        }
        if self.installed.is_some() {
            return Err(ResourceLayoutError::AlreadyInstalled);
        }
        if registry.shutting_down {
            return Err(ResourceLayoutError::RegistryShutdown);
        }
        if registry.live_count != 0 {
            return Err(ResourceLayoutError::RegistryBusy);
        }
        let wire = wire::decode(bytes)?;
        let slots = validation::validate(&wire)?;
        // All fallible table/projection allocation and checks precede identity issuance.
        let kinds = issued_kinds(&NEXT_NATIVE_KIND, wire.kinds.len())?;
        let core = RegisteredResourceProgram(Arc::new(ProgramLayout {
            context: self.context,
            kinds,
            slots,
        }));
        let layout = Arc::new(RegisteredNativeLayout { core, wire });
        self.installed = Some(layout.clone());
        Ok(layout)
    }
}

#[cfg(test)]
#[path = "registration/tests.rs"]
mod tests;
