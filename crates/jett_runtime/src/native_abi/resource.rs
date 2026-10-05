//! Private typed native Resource protocol. No Source or C operation entry is minted here.

use super::*;
use crate::resource_custody::{
    BorrowedResourceToken, CustodyError, JettResourceCallResultV1, NativeFrameRole,
    NativeLayoutInstallation, NativeLoanSource, NativeOperation, NativeParent, NativePayloadStep,
    NativeRecipe, NativeShape, NativeSourceInvocation, NativeSourceResult, NativeSourceValue,
    OwnedResourceToken, PreparedResourceAcquisition, RESOURCE_DOMAIN_ERROR, RESOURCE_DOMAIN_OK,
    RegisteredNativeLayout, ResourceCleanupFailure, ResourceCleanupOutcome, ResourceCustody,
    ResourceFrameKind, ResourceFrameToken, ResourceLayoutError, ResourcePurpose,
    ResourceTransitionFailure,
};
use crate::{AuthorityProvenance, RegistryError};

mod leaves;
mod operations;
mod provider;
mod source;
#[cfg(test)]
pub(super) use provider::{DecodedScript, NativeTestEvent};
use provider::{InstalledProvider, NativeProviderIdentity, NativeRestrictionIdentity};

const MAX_ATTEMPTS: usize = 64;
const REFUSED: &[u8] = b"native Resource protocol refused";
const CLEANUP_FAILED: &[u8] = b"native Resource cleanup failed";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum NativeResourceError {
    Context,
    AlreadyInstalled,
    MissingInstallation,
    InvalidEntry,
    InvalidHandle,
    WrongFamily,
    WrongFrame,
    WrongOperation,
    WrongGrant,
    WrongPurpose,
    ActiveAttempt,
    BodyFailed,
    UnsupportedSourceBoundary,
    Capacity,
    Layout(ResourceLayoutError),
    Custody(CustodyError),
    Registry(RegistryError),
    Cleanup(ResourceCleanupFailure),
    Ordinary(JettRuntimeStatusV1),
    OrdinaryStorage(JettRuntimeStatusV1),
}
type ResourceResult<T> = Result<T, NativeResourceError>;
impl From<ResourceLayoutError> for NativeResourceError {
    fn from(error: ResourceLayoutError) -> Self {
        Self::Layout(error)
    }
}
impl From<CustodyError> for NativeResourceError {
    fn from(error: CustodyError) -> Self {
        Self::Custody(error)
    }
}
impl From<RegistryError> for NativeResourceError {
    fn from(error: RegistryError) -> Self {
        Self::Registry(error)
    }
}
impl NativeResourceError {
    pub(super) fn status(self) -> JettRuntimeStatusV1 {
        match self {
            Self::Capacity => JettRuntimeStatusV1::RESOURCE_EXHAUSTED,
            Self::Context => JettRuntimeStatusV1::INVALID_CONTEXT,
            Self::Ordinary(status) | Self::OrdinaryStorage(status) => status,
            Self::Cleanup(ResourceCleanupFailure::FinalizerPanic) => JettRuntimeStatusV1::PANIC,
            _ => JettRuntimeStatusV1::INVALID_ARGUMENT,
        }
    }
}
fn ordinary_error(error: (JettRuntimeStatusV1, &'static [u8])) -> NativeResourceError {
    NativeResourceError::OrdinaryStorage(error.0)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(super) struct ResourceHandleId(NonZeroU64);
impl ResourceHandleId {
    pub(super) fn new(raw: u64) -> ResourceResult<Self> {
        NonZeroU64::new(raw)
            .map(Self)
            .ok_or(NativeResourceError::InvalidHandle)
    }
    pub(super) fn raw(self) -> u64 {
        self.0.get()
    }
}
fn next_handle(ids: &mut values::ReservedNativeIdentities) -> ResourceResult<ResourceHandleId> {
    ResourceHandleId::new(ids.take().map_err(ordinary_error)?)
}

/// This key and state borrow always come from the same stationary context lease.
pub(super) struct AuthenticatedResourceContext {
    key: ContextRegistryKey,
    lease: NativeContextLease,
}
impl AuthenticatedResourceContext {
    pub(super) fn acquire(context: *const JettRuntimeContextV1) -> ResourceResult<Self> {
        let key = context_key(context).map_err(|_| NativeResourceError::Context)?;
        let lease = acquire_context(key).map_err(|_| NativeResourceError::Context)?;
        Ok(Self { key, lease })
    }
    pub(super) fn with_state<T>(
        &self,
        operation: impl FnOnce(ContextRegistryKey, &mut NativeContextState) -> ResourceResult<T>,
    ) -> ResourceResult<T> {
        let mut state = lock_unpoisoned(&self.lease.entry.state);
        let state = state.as_mut().ok_or(NativeResourceError::Context)?;
        if state
            .resource_state
            .as_ref()
            .is_some_and(|resource| resource.context != self.key)
        {
            return Err(NativeResourceError::Context);
        }
        operation(self.key, state)
    }
    pub(super) fn with_resource<T>(
        &self,
        operation: impl FnOnce(
            &mut NativeResourceState,
            &mut values::NativeValues,
            &mut ResourceRegistry,
        ) -> ResourceResult<T>,
    ) -> ResourceResult<T> {
        self.with_state(|_, state| {
            let resource = state
                .resource_state
                .as_mut()
                .ok_or(NativeResourceError::MissingInstallation)?;
            match catch_unwind(AssertUnwindSafe(|| {
                operation(resource, &mut state.values, &mut state.resources)
            })) {
                Ok(Ok(value)) => Ok(value),
                Ok(Err(error)) => {
                    resource.record_failure(error);
                    Err(error)
                }
                Err(panic) => {
                    discard_panic_payload(panic);
                    if let Some(attempt) = &mut resource.attempt {
                        attempt.body.get_or_insert(NativeBodyFailure::HostPanic);
                    }
                    Err(NativeResourceError::BodyFailed)
                }
            }
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct NativeEntry {
    pub(super) function: u32,
    pub(super) signature: u32,
    pub(super) scope: u32,
}

struct NativeFrameEntry {
    template: u32,
    attempt: ResourceHandleId,
    parent: Option<ResourceHandleId>,
    token: ResourceFrameToken,
    incoming: Vec<Option<NativeResidentLoan>>,
}
struct NativeResidentLoan {
    parent_loan: ResourceHandleId,
    caller_frame: ResourceHandleId,
    invoke_operation: u32,
    callee_parameter: u32,
}
struct NativeOwnerEntry {
    frame: ResourceHandleId,
    slot: u32,
    token: OwnedResourceToken,
}
struct NativeLoanEntry {
    frame: ResourceHandleId,
    borrow_operation: u32,
    source_slot: u32,
    source_frame: ResourceHandleId,
    token: BorrowedResourceToken,
}
struct NativePreparedCall {
    operation: u32,
    target_operation: u32,
    hook: u32,
    frame: ResourceHandleId,
    descriptor: Option<ResourceHandleId>,
}
struct NativePreparedEntry {
    destination_frame: ResourceHandleId,
    call: ResourceHandleId,
    frame: ResourceHandleId,
    destination: u32,
    kind: u32,
    network: ResourceHandleId,
    token: PreparedResourceAcquisition,
    publication_ids: values::ReservedNativeIdentities,
}
struct NativeNetworkGrant {
    context: ContextRegistryKey,
    layout: Arc<RegisteredNativeLayout>,
    provider: NativeProviderIdentity,
    restriction: NativeRestrictionIdentity,
    ordinary_network: u64,
    provenance: AuthorityProvenance,
}
enum NativeResourceEntry {
    Program { layout: Arc<RegisteredNativeLayout> },
    Frame(NativeFrameEntry),
    Owner(NativeOwnerEntry),
    Loan(NativeLoanEntry),
    Prepared(NativePreparedEntry),
    Call(NativePreparedCall),
    Descriptor { hook: u32, signature: u32 },
    NetworkGrant(NativeNetworkGrant),
    Sum(NativeResourceSum),
    SourceCall(source::NativeSourceCall),
}
struct NativeResourceSum {
    frame: ResourceHandleId,
    slot: u32,
    shape: u32,
    payload: NativeSumPayload,
}
enum NativeSumPayload {
    None,
    Some(OwnedResourceToken),
    Ok(OwnedResourceToken),
    Fail { shape: u32, string: u64 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AttemptPhase {
    Running,
    Completing,
}
struct NativeAttempt {
    id: ResourceHandleId,
    entry: NativeEntry,
    purpose: ResourcePurpose,
    root_frame: ResourceHandleId,
    root_body_status: Option<u32>,
    body: Option<NativeBodyFailure>,
    cleanup: Option<ResourceCleanupFailure>,
    phase: AttemptPhase,
}
enum NativeBodyFailure {
    Resource(NativeResourceError),
    HostPanic,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct NativeResourceCompletion {
    pub(super) attempt: u64,
    pub(super) body_status: u32,
    pub(super) cleanup_status: u32,
    pub(super) selected_kind: u32,
    pub(super) reserved: u32,
}
struct NativeCompletedAttempt {
    completion: NativeResourceCompletion,
    resource_message: Vec<u8>,
    ordinary_message: Vec<u8>,
    report_error: Option<NativeResourceError>,
}

#[repr(C)]
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub(super) struct NativeResourceCounts {
    pub(super) owners: u64,
    pub(super) loans: u64,
    pub(super) frames: u64,
    pub(super) registry: u64,
    pub(super) owner_handles: u64,
    pub(super) loan_handles: u64,
    pub(super) frame_handles: u64,
    pub(super) provisional: u64,
    pub(super) ordinary_empty: u32,
    pub(super) reserved: u32,
}

pub(super) struct NativeResourceState {
    context: ContextRegistryKey,
    installation: NativeLayoutInstallation,
    layout: Arc<RegisteredNativeLayout>,
    program_handle: ResourceHandleId,
    custody: ResourceCustody,
    handles: HashMap<ResourceHandleId, NativeResourceEntry>,
    active_frames: Vec<ResourceHandleId>,
    attempt: Option<NativeAttempt>,
    completed: Vec<NativeCompletedAttempt>,
    provider: InstalledProvider,
    entry: NativeEntry,
    network: Option<ResourceHandleId>,
}

impl NativeResourceState {
    fn validate_installation(&self) -> ResourceResult<()> {
        if !self
            .installation
            .installed()
            .is_some_and(|layout| Arc::ptr_eq(layout, &self.layout))
        {
            return Err(NativeResourceError::MissingInstallation);
        }
        match self.handles.get(&self.program_handle) {
            Some(NativeResourceEntry::Program { layout }) if Arc::ptr_eq(layout, &self.layout) => {
                Ok(())
            }
            _ => Err(NativeResourceError::WrongFamily),
        }
    }
    fn reserve(&mut self, count: usize) -> ResourceResult<values::ReservedNativeIdentities> {
        self.handles
            .try_reserve(count)
            .map_err(|_| NativeResourceError::Capacity)?;
        values::reserve_native_identities(
            u64::try_from(count).map_err(|_| NativeResourceError::Capacity)?,
        )
        .map_err(ordinary_error)
    }
    fn running(&self, ordinary: &values::NativeValues) -> ResourceResult<&NativeAttempt> {
        self.validate_installation()?;
        let attempt = self
            .attempt
            .as_ref()
            .ok_or(NativeResourceError::InvalidEntry)?;
        if attempt.phase != AttemptPhase::Running
            || attempt.body.is_some()
            || attempt.cleanup.is_some()
        {
            return Err(NativeResourceError::BodyFailed);
        }
        if let Some(failure) = ordinary.resource_failure() {
            return Err(NativeResourceError::Ordinary(failure.status));
        }
        if ordinary.cleanup_failed {
            return Err(NativeResourceError::BodyFailed);
        }
        Ok(attempt)
    }
    fn frame(&self, handle: ResourceHandleId) -> ResourceResult<&NativeFrameEntry> {
        let NativeResourceEntry::Frame(frame) = self
            .handles
            .get(&handle)
            .ok_or(NativeResourceError::InvalidHandle)?
        else {
            return Err(NativeResourceError::WrongFamily);
        };
        if self.attempt.as_ref().map(|attempt| attempt.id) != Some(frame.attempt)
            || !self.active_frames.contains(&handle)
        {
            return Err(NativeResourceError::WrongFrame);
        }
        Ok(frame)
    }
    fn operation(&self, ordinal: u32) -> ResourceResult<&NativeOperation> {
        self.layout
            .operations()
            .get(ordinal as usize)
            .map(|record| record.operation())
            .ok_or(NativeResourceError::WrongOperation)
    }
    fn operation_frame(&self, ordinal: u32, frame: ResourceHandleId) -> ResourceResult<()> {
        if self.active_frames.last() != Some(&frame) {
            return Err(NativeResourceError::WrongFrame);
        }
        let template = self.frame(frame)?.template;
        let operation = self.operation(ordinal)?;
        let actual = match operation {
            NativeOperation::Acquire { frame, .. }
            | NativeOperation::Transfer { frame, .. }
            | NativeOperation::Borrow { frame, .. }
            | NativeOperation::BoundedBorrowUse { frame, .. }
            | NativeOperation::EndBorrow { frame, .. }
            | NativeOperation::InvokeBorrow { frame, .. }
            | NativeOperation::Close { frame, .. }
            | NativeOperation::Drop { frame, .. }
            | NativeOperation::SumAdopt { frame, .. }
            | NativeOperation::SumTake { frame, .. }
            | NativeOperation::SumDrop { frame, .. }
            | NativeOperation::Replace { frame, .. }
            | NativeOperation::Complete { frame, .. }
            | NativeOperation::InvokeSourceFunction { frame, .. }
            | NativeOperation::TakeFailureCompanion { frame, .. }
            | NativeOperation::PublishReturn { frame, .. }
            | NativeOperation::CreateAbsentSum { frame, .. }
            | NativeOperation::CreateFailureSum { frame, .. } => *frame,
            NativeOperation::Descriptor { .. } | NativeOperation::InvokeDescriptor { .. } => {
                return Err(NativeResourceError::WrongOperation);
            }
        };
        if template != actual {
            return Err(NativeResourceError::WrongFrame);
        }
        Ok(())
    }
    fn cleanup(&mut self, outcome: ResourceCleanupOutcome) -> ResourceResult<()> {
        if let Some(error) = outcome.failure() {
            if let Some(attempt) = &mut self.attempt {
                attempt.cleanup.get_or_insert(error);
            }
            Err(NativeResourceError::Cleanup(error))
        } else {
            Ok(())
        }
    }
    fn record_failure(&mut self, error: NativeResourceError) {
        if let Some(attempt) = &mut self.attempt {
            match error {
                NativeResourceError::Ordinary(_) => {} // current ordinary storage remains authoritative
                NativeResourceError::Cleanup(error) => {
                    attempt.cleanup.get_or_insert(error);
                }
                other => {
                    attempt
                        .body
                        .get_or_insert(NativeBodyFailure::Resource(other));
                }
            }
        }
    }

    pub(super) fn begin_entry(
        &mut self,
        ordinary: &values::NativeValues,
        registry: &ResourceRegistry,
        entry: NativeEntry,
        purpose: ResourcePurpose,
    ) -> ResourceResult<ResourceHandleId> {
        self.validate_installation()?;
        if entry != self.entry || self.attempt.is_some() {
            return Err(NativeResourceError::ActiveAttempt);
        }
        #[cfg(test)]
        if self.completed.len() >= self.provider.attempts() {
            return Err(NativeResourceError::InvalidEntry);
        }
        if self.completed.len() >= MAX_ATTEMPTS
            || self.custody.live_owners() != 0
            || self.custody.live_loans() != 0
            || self.custody.active_frames() != 0
            || registry.live_count() != 0
            || !self.active_frames.is_empty()
            || !self.only_installation_handles()
        {
            return Err(NativeResourceError::InvalidEntry);
        }
        if let Some(failure) = ordinary.resource_failure() {
            return Err(NativeResourceError::Ordinary(failure.status));
        }
        if ordinary.cleanup_failed {
            return Err(NativeResourceError::BodyFailed);
        }
        let mut ids = self.reserve(2)?;
        self.active_frames
            .try_reserve(1)
            .map_err(|_| NativeResourceError::Capacity)?;
        self.completed
            .try_reserve(1)
            .map_err(|_| NativeResourceError::Capacity)?;
        let id = next_handle(&mut ids)?;
        let root_frame = next_handle(&mut ids)?;
        let token = self
            .custody
            .begin_frame(ResourceFrameKind::Scope, purpose)?;
        self.handles.insert(
            root_frame,
            NativeResourceEntry::Frame(NativeFrameEntry {
                template: entry.scope,
                attempt: id,
                parent: None,
                token,
                incoming: Vec::new(),
            }),
        );
        self.active_frames.push(root_frame);
        self.attempt = Some(NativeAttempt {
            id,
            entry,
            purpose,
            root_frame,
            root_body_status: None,
            body: None,
            cleanup: None,
            phase: AttemptPhase::Running,
        });
        Ok(id)
    }
    fn only_installation_handles(&self) -> bool {
        self.handles.values().all(|entry| {
            matches!(
                entry,
                NativeResourceEntry::Program { .. }
                    | NativeResourceEntry::NetworkGrant(_)
                    | NativeResourceEntry::Descriptor { .. }
            )
        })
    }
    pub(super) fn entry_network(
        &self,
        attempt: ResourceHandleId,
        parameter: u32,
    ) -> ResourceResult<u64> {
        self.validate_installation()?;
        self.runtime_purpose()?;
        let running = self
            .attempt
            .as_ref()
            .ok_or(NativeResourceError::InvalidEntry)?;
        if running.id != attempt || running.phase != AttemptPhase::Running {
            return Err(NativeResourceError::InvalidEntry);
        }
        let formal = self.layout.signatures()[self.entry.signature as usize]
            .parameters()
            .get(parameter as usize)
            .ok_or(NativeResourceError::InvalidEntry)?;
        if !matches!(
            self.layout.shapes().get(formal.shape() as usize),
            Some(NativeShape::Network)
        ) {
            return Err(NativeResourceError::InvalidEntry);
        }
        match self
            .handles
            .get(&self.network.ok_or(NativeResourceError::WrongGrant)?)
        {
            Some(NativeResourceEntry::NetworkGrant(grant)) => Ok(grant.ordinary_network),
            _ => Err(NativeResourceError::WrongGrant),
        }
    }
    /// Observe only the sole retired root Scope of this exact Runtime attempt.
    /// Body/ordinary/cleanup failures are independent and must not gate this read.
    pub(super) fn entry_outcome(
        &self,
        scope: ResourceHandleId,
        function: u32,
        signature: u32,
        template: u32,
    ) -> ResourceResult<u32> {
        self.validate_installation()?;
        self.runtime_purpose()?;
        let attempt = self
            .attempt
            .as_ref()
            .ok_or(NativeResourceError::InvalidEntry)?;
        let selected = NativeEntry {
            function,
            signature,
            scope: template,
        };
        if attempt.phase != AttemptPhase::Running
            || attempt.entry != self.entry
            || selected != self.entry
            || attempt.root_frame != scope
        {
            return Err(NativeResourceError::InvalidEntry);
        }
        let row = self
            .layout
            .frames()
            .get(template as usize)
            .ok_or(NativeResourceError::WrongFrame)?;
        if row.role() != NativeFrameRole::Scope
            || row.site().function() != function
            || row.signature() != signature
            || self.handles.contains_key(&scope)
            || self.active_frames.contains(&scope)
        {
            return Err(NativeResourceError::WrongFrame);
        }
        attempt
            .root_body_status
            .ok_or(NativeResourceError::WrongFrame)
    }

    pub(super) fn complete_entry(
        &mut self,
        ordinary: &mut values::NativeValues,
        registry: &mut ResourceRegistry,
        attempt_id: ResourceHandleId,
        body_status: u32,
        host_panic: bool,
    ) -> ResourceResult<NativeResourceCompletion> {
        let attempt = self
            .attempt
            .as_mut()
            .ok_or(NativeResourceError::InvalidEntry)?;
        if attempt.id != attempt_id || attempt.phase != AttemptPhase::Running {
            return Err(NativeResourceError::InvalidEntry);
        }
        attempt.phase = AttemptPhase::Completing;
        if host_panic {
            attempt.body.get_or_insert(NativeBodyFailure::HostPanic);
        }
        // Unwind the real core before retiring token shells or copying diagnostics.
        let cleanup = self.custody.unwind_all(registry);
        if let Some(error) = cleanup.failure() {
            attempt.cleanup.get_or_insert(error);
        }
        let mut companion_failed = false;
        self.handles.retain(|_, entry| {
            if let NativeResourceEntry::Sum(sum) = entry {
                if let NativeSumPayload::Fail { shape, string } = &sum.payload {
                    companion_failed |= ordinary
                        .drop_resource_typed_companion(&self.layout, *shape, *string)
                        .is_err();
                }
            }
            matches!(
                entry,
                NativeResourceEntry::Program { .. }
                    | NativeResourceEntry::NetworkGrant(_)
                    | NativeResourceEntry::Descriptor { .. }
            )
        });
        self.active_frames.clear();
        if companion_failed {
            ordinary.cleanup_failed = true;
        }
        if self.custody.live_owners() != 0
            || self.custody.live_loans() != 0
            || self.custody.active_frames() != 0
            || registry.live_count() != 0
        {
            return Err(NativeResourceError::BodyFailed);
        }
        let attempt = self
            .attempt
            .take()
            .ok_or(NativeResourceError::InvalidEntry)?;
        let ordinary_failure = ordinary.resource_failure();
        let recorded_body_failure = attempt.body.is_some() || ordinary_failure.is_some();
        let cleanup_status = attempt
            .cleanup
            .map(|error| NativeResourceError::Cleanup(error).status().code())
            .unwrap_or(if ordinary.cleanup_failed {
                JettRuntimeStatusV1::INVALID_ARGUMENT.code()
            } else {
                0
            });
        let selected_kind = if cleanup_status != 0 {
            3
        } else if host_panic || matches!(attempt.body, Some(NativeBodyFailure::HostPanic)) {
            2
        } else if body_status != 0 || recorded_body_failure {
            1
        } else {
            0
        };
        let completion = NativeResourceCompletion {
            attempt: attempt_id.raw(),
            body_status,
            cleanup_status,
            selected_kind,
            reserved: 0,
        };
        let resource_message = if attempt.cleanup.is_some() {
            Some(CLEANUP_FAILED)
        } else if ordinary.cleanup_failed {
            Some(b"native value cleanup failed".as_slice())
        } else {
            match attempt.body {
                Some(NativeBodyFailure::HostPanic) => Some(PANIC_MESSAGE),
                Some(NativeBodyFailure::Resource(_)) => Some(REFUSED),
                None => None,
            }
        };
        let diagnostic: ResourceResult<(Vec<u8>, Vec<u8>)> = (|| {
            let resource_message = copy_diagnostic(None, resource_message.unwrap_or_default())?;
            let ordinary_message = if let Some(failure) = ordinary_failure {
                copy_diagnostic(failure.prefix, failure.message)?
            } else {
                Vec::new()
            };
            Ok((resource_message, ordinary_message))
        })();
        let (resource_message, ordinary_message, report_error) = match diagnostic {
            Ok((resource, ordinary)) => (resource, ordinary, None),
            Err(error) => (Vec::new(), Vec::new(), Some(error)),
        };
        // A report failure retains a completed tombstone; cleanup and retirement are not retried.
        self.completed.push(NativeCompletedAttempt {
            completion,
            resource_message,
            ordinary_message,
            report_error,
        });
        if let Some(error) = report_error {
            return Err(error);
        }
        Ok(completion)
    }
    pub(super) fn completed_attempt(
        &self,
        attempt: ResourceHandleId,
    ) -> ResourceResult<NativeResourceCompletion> {
        let completed = self
            .completed
            .iter()
            .find(|entry| entry.completion.attempt == attempt.raw())
            .ok_or(NativeResourceError::InvalidEntry)?;
        if let Some(error) = completed.report_error {
            return Err(error);
        }
        Ok(completed.completion)
    }
    pub(super) fn completed_messages(
        &self,
        attempt: ResourceHandleId,
    ) -> ResourceResult<(&[u8], &[u8])> {
        let completed = self
            .completed
            .iter()
            .find(|entry| entry.completion.attempt == attempt.raw())
            .ok_or(NativeResourceError::InvalidEntry)?;
        if let Some(error) = completed.report_error {
            return Err(error);
        }
        Ok((&completed.resource_message, &completed.ordinary_message))
    }
    #[cfg(test)]
    pub(super) fn script_remaining(&self) -> usize {
        self.provider.remaining()
    }
    #[cfg(test)]
    pub(super) fn observe_events<T>(
        &self,
        observe: impl FnOnce(&[provider::NativeTestEvent]) -> T,
    ) -> T {
        self.provider.observe_events(observe)
    }
    pub(super) fn counts(
        &self,
        ordinary: &values::NativeValues,
        registry: &ResourceRegistry,
    ) -> NativeResourceCounts {
        let mut counts = NativeResourceCounts {
            owners: self.custody.live_owners() as u64,
            loans: self.custody.live_loans() as u64,
            frames: self.custody.active_frames() as u64,
            registry: registry.live_count() as u64,
            ordinary_empty: u32::from(ordinary.is_empty()),
            ..NativeResourceCounts::default()
        };
        for entry in self.handles.values() {
            match entry {
                NativeResourceEntry::Owner(_) => counts.owner_handles += 1,
                NativeResourceEntry::Sum(NativeResourceSum {
                    payload: NativeSumPayload::Some(_) | NativeSumPayload::Ok(_),
                    ..
                }) => counts.owner_handles += 1,
                NativeResourceEntry::Loan(_) => counts.loan_handles += 1,
                NativeResourceEntry::Frame(frame) => {
                    counts.frame_handles += 1;
                    if self.layout.frames()[frame.template as usize].role()
                        == NativeFrameRole::Return
                    {
                        counts.provisional += 1;
                    }
                }
                _ => {}
            }
        }
        counts
    }
    pub(super) fn safety_unwind(
        &mut self,
        ordinary: &mut values::NativeValues,
        registry: &mut ResourceRegistry,
    ) -> bool {
        let outcome = self.custody.unwind_all(registry);
        let mut failed = outcome.failure().is_some();
        for entry in self.handles.values() {
            if let NativeResourceEntry::Sum(NativeResourceSum {
                payload: NativeSumPayload::Fail { string, .. },
                ..
            }) = entry
            {
                failed |= ordinary.drop_resource_ordinary_companion(*string).is_err();
            }
        }
        self.handles.clear();
        self.active_frames.clear();
        self.attempt = None;
        failed
            || self.custody.live_owners() != 0
            || self.custody.live_loans() != 0
            || registry.live_count() != 0
    }
}

fn copy_diagnostic(prefix: Option<&[u8]>, message: &[u8]) -> ResourceResult<Vec<u8>> {
    let prefix = prefix.unwrap_or_default();
    let length = prefix
        .len()
        .checked_add(message.len())
        .ok_or(NativeResourceError::Capacity)?;
    if length > 64 * 1024 {
        return Err(NativeResourceError::Capacity);
    }
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(length)
        .map_err(|_| NativeResourceError::Capacity)?;
    bytes.extend_from_slice(prefix);
    bytes.extend_from_slice(message);
    Ok(bytes)
}

fn validate_entry(layout: &RegisteredNativeLayout, entry: NativeEntry) -> ResourceResult<bool> {
    let frame = layout
        .frames()
        .get(entry.scope as usize)
        .ok_or(NativeResourceError::InvalidEntry)?;
    let signature = layout
        .signatures()
        .get(entry.signature as usize)
        .ok_or(NativeResourceError::InvalidEntry)?;
    if frame.role() != NativeFrameRole::Scope
        || frame.site().function() != entry.function
        || frame.signature() != entry.signature
        || frame.parents() != [NativeParent::Root]
        || !matches!(
            layout.shapes().get(signature.result() as usize),
            Some(NativeShape::Nothing)
        )
    {
        return Err(NativeResourceError::InvalidEntry);
    }
    let mut network = false;
    for formal in signature.parameters() {
        match layout.shapes().get(formal.shape() as usize) {
            Some(NativeShape::Network) => network = true,
            _ => return Err(NativeResourceError::InvalidEntry),
        }
    }
    Ok(network)
}

fn install_candidate(
    key: ContextRegistryKey,
    state: &mut NativeContextState,
    bytes: &[u8],
    entry: NativeEntry,
    mut provider: InstalledProvider,
) -> ResourceResult<ResourceHandleId> {
    if state.resource_state.is_some() {
        return Err(NativeResourceError::AlreadyInstalled);
    }
    if state.values.resource_failure().is_some()
        || state.values.cleanup_failed
        || !state.values.is_empty()
        || state.resources.live_count() != 0
    {
        return Err(NativeResourceError::BodyFailed);
    }
    let mut installation = NativeLayoutInstallation::new(&state.resources);
    let layout = installation.install(&state.resources, bytes)?;
    let needs_network = validate_entry(&layout, entry)?;
    let pair_network = needs_network && provider.enabled();
    let custody = ResourceCustody::new(layout.program().clone(), &state.resources)?;
    let mut handles = HashMap::new();
    handles
        .try_reserve(2)
        .map_err(|_| NativeResourceError::Capacity)?;
    let mut ids = values::reserve_native_identities(5).map_err(ordinary_error)?;
    let program_handle = next_handle(&mut ids)?;
    let network_handle = if pair_network {
        Some(next_handle(&mut ids)?)
    } else {
        None
    };
    let provider_id = NativeProviderIdentity::new(ids.take().map_err(ordinary_error)?)?;
    let restriction = NativeRestrictionIdentity::new(ids.take().map_err(ordinary_error)?)?;
    provider.bind(provider_id, network_handle)?;
    let staged = if pair_network {
        Some(
            state
                .values
                .prepare_resource_network(&mut ids)
                .map_err(ordinary_error)?,
        )
    } else {
        None
    };
    handles.insert(
        program_handle,
        NativeResourceEntry::Program {
            layout: layout.clone(),
        },
    );
    // No fallible publication follows this commit while the authenticated lease owns the mutex.
    if let (Some(handle), Some(staged)) = (network_handle, staged) {
        let ordinary_network = state.values.commit_resource_network(staged);
        handles.insert(
            handle,
            NativeResourceEntry::NetworkGrant(NativeNetworkGrant {
                context: key,
                layout: layout.clone(),
                provider: provider_id,
                restriction,
                ordinary_network,
                provenance: AuthorityProvenance::new(provider_id.raw(), restriction.raw()),
            }),
        );
    }
    state.resource_state = Some(NativeResourceState {
        context: key,
        installation,
        layout,
        program_handle,
        custody,
        handles,
        active_frames: Vec::new(),
        attempt: None,
        completed: Vec::new(),
        provider,
        entry,
        network: network_handle,
    });
    Ok(program_handle)
}

pub(super) fn install_disabled(
    authenticated: &AuthenticatedResourceContext,
    bytes: &[u8],
    entry: NativeEntry,
) -> ResourceResult<ResourceHandleId> {
    authenticated.with_state(|key, state| {
        install_candidate(key, state, bytes, entry, InstalledProvider::Disabled)
    })
}
#[cfg(test)]
pub(super) fn install_scripted(
    authenticated: &AuthenticatedResourceContext,
    bytes: &[u8],
    entry: NativeEntry,
    script: DecodedScript,
) -> ResourceResult<ResourceHandleId> {
    authenticated.with_state(|key, state| {
        install_candidate(
            key,
            state,
            bytes,
            entry,
            InstalledProvider::scripted(script)?,
        )
    })
}

#[cfg(test)]
mod tests;
