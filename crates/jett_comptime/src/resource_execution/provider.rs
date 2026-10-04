use std::fmt;

use jett_runtime::{AuthorityProvenance, ResourceRegistry};

use super::{ResourceExecutionError, ResourceTypeBinding};

/// This carrier can be copied as capability authority, never as a Resource owner.
#[derive(Clone)]
pub struct GrantedNetwork {
    context: u64,
    authority: AuthorityProvenance,
}

impl fmt::Debug for GrantedNetwork {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("GrantedNetwork(<runtime authority>)")
    }
}

pub(crate) enum InstalledResourceProvider {
    Disabled,
    #[cfg(test)]
    Scripted(ScriptedResourceProvider),
}

impl InstalledResourceProvider {
    pub(crate) fn validate_grant(
        &self,
        grant: &GrantedNetwork,
    ) -> Result<AuthorityProvenance, ResourceExecutionError> {
        match self {
            Self::Disabled => Err(ResourceExecutionError::ProviderDisabled),
            #[cfg(test)]
            Self::Scripted(provider)
                if provider.grant.context == grant.context
                    && provider.grant.authority == grant.authority =>
            {
                Ok(grant.authority)
            }
            #[cfg(test)]
            Self::Scripted(_) => Err(ResourceExecutionError::InvalidGrant),
        }
    }

    pub(crate) fn construct(
        &mut self,
        registry: &mut ResourceRegistry,
        resource: ResourceTypeBinding,
        grant: &GrantedNetwork,
        label: i64,
    ) -> Result<Result<jett_runtime::ResourceKey, String>, ResourceExecutionError> {
        let authority = self.validate_grant(grant)?;
        match self {
            Self::Disabled => Err(ResourceExecutionError::ProviderDisabled),
            #[cfg(test)]
            Self::Scripted(provider) => provider.construct(registry, resource, authority, label),
        }
    }

    pub(crate) fn borrow(
        &mut self,
        registry: &mut ResourceRegistry,
        resource: ResourceTypeBinding,
        grant: &GrantedNetwork,
        key: jett_runtime::ResourceKey,
    ) -> Result<Result<i64, String>, ResourceExecutionError> {
        let authority = self.validate_grant(grant)?;
        match self {
            Self::Disabled => Err(ResourceExecutionError::ProviderDisabled),
            #[cfg(test)]
            Self::Scripted(provider) => provider.borrow(registry, resource, authority, key),
        }
    }

    pub(crate) fn ensure_installed(&self) -> Result<(), ResourceExecutionError> {
        match self {
            Self::Disabled => Err(ResourceExecutionError::ProviderDisabled),
            #[cfg(test)]
            Self::Scripted(_) => Ok(()),
        }
    }
}

// No production provider, public provider spelling or host-hook callback map.
// Compiler unit tests alone can supply the selected exact three recipe script.
#[cfg(test)]
use std::collections::VecDeque;
#[cfg(test)]
use std::sync::atomic::{AtomicU64, Ordering};
#[cfg(test)]
use std::sync::{Arc, Mutex};

#[cfg(test)]
static NEXT_TEST_GRANT: AtomicU64 = AtomicU64::new(1);

#[cfg(test)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ProviderEvent {
    Constructed(i64),
    ConstructionFailed(i64),
    Borrowed(i64),
    BorrowFailed(i64),
    Finalized(i64),
}

#[cfg(test)]
#[derive(Clone)]
pub(crate) enum ScriptOperation {
    Construct {
        label: i64,
        outcome: Result<(), String>,
    },
    ConstructFinalizerPanic {
        label: i64,
    },
    Borrow {
        label: i64,
        outcome: Result<i64, String>,
    },
    BorrowPanic {
        label: i64,
    },
}

#[cfg(test)]
struct FakePayload {
    label: i64,
}

#[cfg(test)]
pub(crate) struct ScriptedResourceProvider {
    grant: GrantedNetwork,
    operations: VecDeque<ScriptOperation>,
    events: Arc<Mutex<Vec<ProviderEvent>>>,
}

#[cfg(test)]
impl ScriptedResourceProvider {
    pub(crate) fn new(operations: Vec<ScriptOperation>) -> Result<Self, ResourceExecutionError> {
        let context = NEXT_TEST_GRANT
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |value| {
                value.checked_add(1)
            })
            .map_err(|_| ResourceExecutionError::IdentityExhausted)?;
        Ok(Self {
            grant: GrantedNetwork {
                context,
                authority: AuthorityProvenance::new(context, 1),
            },
            operations: operations.into(),
            events: Arc::new(Mutex::new(Vec::new())),
        })
    }

    pub(crate) fn grant(&self) -> GrantedNetwork {
        self.grant.clone()
    }

    pub(crate) fn events(&self) -> Result<Vec<ProviderEvent>, ResourceExecutionError> {
        self.events
            .lock()
            .map(|events| events.clone())
            .map_err(|_| ResourceExecutionError::InvalidGrant)
    }

    pub(crate) fn remaining(&self) -> usize {
        self.operations.len()
    }

    fn record(&self, event: ProviderEvent) -> Result<(), ResourceExecutionError> {
        self.events
            .lock()
            .map_err(|_| ResourceExecutionError::InvalidGrant)?
            .push(event);
        Ok(())
    }

    fn construct(
        &mut self,
        registry: &mut ResourceRegistry,
        resource: ResourceTypeBinding,
        authority: AuthorityProvenance,
        label: i64,
    ) -> Result<Result<jett_runtime::ResourceKey, String>, ResourceExecutionError> {
        let (expected, outcome, finalizer_panic) = match self.operations.front() {
            Some(ScriptOperation::Construct { label, outcome }) => (*label, outcome.clone(), false),
            Some(ScriptOperation::ConstructFinalizerPanic { label }) => (*label, Ok(()), true),
            _ => return Err(ResourceExecutionError::InvalidInvocation),
        };
        if expected != label {
            return Err(ResourceExecutionError::InvalidInvocation);
        }
        if let Err(message) = outcome {
            self.record(ProviderEvent::ConstructionFailed(label))?;
            self.operations.pop_front();
            return Ok(Err(message));
        }
        // Record bookkeeping before publication; a poisoned test log cannot
        // publish an owner and then report construction failure.
        self.record(ProviderEvent::Constructed(label))?;
        let finalized_events = Arc::clone(&self.events);
        let key = registry.insert(
            resource.registry_type,
            FakePayload { label },
            authority,
            move |payload| {
                // Test telemetry is separate from source stdout/debug surfaces.
                if let Ok(mut events) = finalized_events.lock() {
                    events.push(ProviderEvent::Finalized(payload.label));
                }
                if finalizer_panic {
                    panic!("selected test resource cleanup panic");
                }
            },
        )?;
        self.operations.pop_front();
        Ok(Ok(key))
    }

    fn borrow(
        &mut self,
        registry: &mut ResourceRegistry,
        resource: ResourceTypeBinding,
        authority: AuthorityProvenance,
        key: jett_runtime::ResourceKey,
    ) -> Result<Result<i64, String>, ResourceExecutionError> {
        // Carrier/provenance validation occurs before consuming script or event.
        let label = registry.access(
            key,
            resource.registry_type,
            &authority,
            |payload: &mut FakePayload| payload.label,
        )?;
        let (expected, outcome) = match self.operations.front() {
            Some(ScriptOperation::Borrow { label, outcome }) => (*label, Some(outcome.clone())),
            Some(ScriptOperation::BorrowPanic { label }) => (*label, None),
            _ => return Err(ResourceExecutionError::InvalidInvocation),
        };
        if expected != label {
            return Err(ResourceExecutionError::InvalidInvocation);
        }
        let Some(outcome) = outcome else {
            self.record(ProviderEvent::Borrowed(label))?;
            self.operations.pop_front();
            panic!("selected test borrow provider panic");
        };
        self.record(if outcome.is_ok() {
            ProviderEvent::Borrowed(label)
        } else {
            ProviderEvent::BorrowFailed(label)
        })?;
        self.operations.pop_front();
        Ok(outcome)
    }
}
