//! Checked source Resource execution. A carrier is never a cleanup owner.

mod checked;
mod custody;
mod provider;
mod runtime;

use std::fmt;
use std::sync::Arc;

use jett_resolve::DefId;
use jett_runtime::{RegistryError, ResourceKey, ResourceTypeId};
use jett_typecheck::CheckedResourceProgram;
use jett_types::TypeId;

pub use checked::ExecutionPurpose;
pub(crate) use checked::{
    CheckedAttemptKey, CheckedBodyCursor, CheckedBodyReference, CheckedExecution,
    CheckedInvocation, FunctionInvocation, PreparedIntrinsicArguments, PreparedPipelineStep,
    PreparedRequiredExpression,
};
pub(crate) use custody::{
    EvaluatedValue, FrameId, OwnerHolder, OwnerLedger, PayloadStep, ValueCustody,
};
pub use provider::GrantedNetwork;
pub(crate) use runtime::{OperationFrame, ResourceTransport};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ResourceTypeBinding {
    definition: DefId,
    checked_type: TypeId,
    registry_type: ResourceTypeId,
}

/// Opaque physical identity. Cloning this value cannot create a cleanup ticket.
#[derive(Clone)]
pub struct ResourceCarrier {
    program: Arc<CheckedResourceProgram>,
    resource: ResourceTypeBinding,
    key: ResourceKey,
}

impl fmt::Debug for ResourceCarrier {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("ResourceCarrier(<opaque>)")
    }
}

/// A closed callable descriptor is data; construction does not execute a hook.
#[derive(Clone)]
pub struct ResourceHookDescriptor {
    program: Arc<CheckedResourceProgram>,
    definition: DefId,
}

impl fmt::Debug for ResourceHookDescriptor {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("ResourceHookDescriptor(<checked>)")
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ResourceExecutionError {
    InvalidCheckedProgram,
    MissingCheckedBody,
    MissingCheckedInvocation,
    InvalidInvocation,
    ForeignProgram,
    WrongPurpose,
    ProviderDisabled,
    InvalidGrant,
    MissingOwner,
    InvalidOwner,
    InvalidBorrow,
    InvalidPayloadPath,
    InvalidFrame,
    IdentityExhausted,
    Registry(RegistryError),
}

impl fmt::Display for ResourceExecutionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            Self::InvalidCheckedProgram => "resource execution requires a valid checked program",
            Self::MissingCheckedBody => "resource execution has no exact checked body",
            Self::MissingCheckedInvocation => "resource execution has no checked call occurrence",
            Self::InvalidInvocation => "resource execution has invalid checked call metadata",
            Self::ForeignProgram => "resource execution belongs to another checked program",
            Self::WrongPurpose => "resource operation requires runtime execution",
            Self::ProviderDisabled => "resource runtime provider is not installed",
            Self::InvalidGrant => "resource operation has no matching runtime authority",
            Self::MissingOwner => "resource value has no owning cleanup obligation",
            Self::InvalidOwner => "resource cleanup obligation is not available",
            Self::InvalidBorrow => "resource view has no live checked backing owner",
            Self::InvalidPayloadPath => "resource value lost its checked payload custody",
            Self::InvalidFrame => "resource execution has no live owning frame",
            Self::IdentityExhausted => "resource execution identity space is exhausted",
            Self::Registry(_) => "resource carrier is not valid for this runtime operation",
        };
        formatter.write_str(message)
    }
}

impl std::error::Error for ResourceExecutionError {}

impl From<RegistryError> for ResourceExecutionError {
    fn from(error: RegistryError) -> Self {
        Self::Registry(error)
    }
}

#[cfg(test)]
pub(crate) use provider::{ProviderEvent, ScriptOperation};
#[cfg(test)]
pub(crate) mod tests;
