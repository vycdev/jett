//! Context-owned ordinary first-error channels for exact Resource entry lifetimes.
//! Old records are sealed observations; a fresh constructor selects a new record.
use super::*;
#[cfg(test)]
use crate::native_abi::resource::ordinary_reservation_faults::{
    ReservationFaults, ReservationSite,
};
use crate::native_abi::resource::{ResourceOrdinaryCompletion, ResourceOrdinaryOwner};

#[derive(Default)]
struct AttemptFailureChannel {
    failure: Option<Failure>,
    dynamic_failure_message: Option<Vec<u8>>,
    property_case_context: Option<Vec<u8>>,
}

#[derive(Clone, Copy)]
enum OrdinaryAttemptPhase {
    Running,
    Sealed(ResourceOrdinaryCompletion),
}

struct OrdinaryAttemptChannel {
    owner: ResourceOrdinaryOwner,
    phase: OrdinaryAttemptPhase,
    error: AttemptFailureChannel,
}

#[derive(Default)]
pub(super) struct ResourceOrdinaryChannels {
    records: Vec<OrdinaryAttemptChannel>,
    selected: Option<usize>,
    #[cfg(test)]
    reservation_faults: ReservationFaults,
}

/// Borrows the exact values state through Resource frame publication. Its private
/// fields and sole constructor prevent a raw ID from selecting a failure channel.
pub(in crate::native_abi) struct PreparedResourceOrdinaryAttempt<'a> {
    values: &'a mut NativeValues,
    owner: ResourceOrdinaryOwner,
}

impl PreparedResourceOrdinaryAttempt<'_> {
    pub(in crate::native_abi) fn publish(self) {
        let index = self.values.resource_ordinary_channels.records.len();
        self.values
            .resource_ordinary_channels
            .records
            .push(OrdinaryAttemptChannel {
                owner: self.owner,
                phase: OrdinaryAttemptPhase::Running,
                error: AttemptFailureChannel::default(),
            });
        self.values.resource_ordinary_channels.selected = Some(index);
    }
}

impl ResourceOrdinaryChannels {
    fn selected(&self) -> Option<&OrdinaryAttemptChannel> {
        self.selected.and_then(|index| self.records.get(index))
    }

    fn running_mut(&mut self) -> Option<&mut AttemptFailureChannel> {
        let record = self
            .selected
            .and_then(|index| self.records.get_mut(index))?;
        match record.phase {
            OrdinaryAttemptPhase::Running => Some(&mut record.error),
            OrdinaryAttemptPhase::Sealed(_) => None,
        }
    }
}

impl NativeValues {
    pub(in crate::native_abi) fn session_failure(&self) -> Option<Failure> {
        self.failure
    }

    // Between-entry operations belong to the legacy session channel. They never
    // mutate a sealed observation, and any such error still blocks the next begin.
    pub(super) fn operation_failure(&self) -> Option<Failure> {
        self.failure.or_else(|| {
            let record = self.resource_ordinary_channels.selected()?;
            match record.phase {
                OrdinaryAttemptPhase::Running => record.error.failure,
                OrdinaryAttemptPhase::Sealed(_) => None,
            }
        })
    }

    pub(super) fn current_failure(&self) -> Option<Failure> {
        self.failure.or_else(|| {
            self.resource_ordinary_channels
                .selected()
                .and_then(|record| record.error.failure)
        })
    }

    pub(super) fn current_dynamic_failure_message(&self) -> Option<&[u8]> {
        if self.failure.is_some() {
            self.dynamic_failure_message.as_deref()
        } else if let Some(record) = self.resource_ordinary_channels.selected() {
            record.error.dynamic_failure_message.as_deref()
        } else {
            self.dynamic_failure_message.as_deref()
        }
    }

    pub(super) fn current_property_case_context(&self) -> Option<&[u8]> {
        if self.failure.is_some() {
            self.property_case_context.as_deref()
        } else if let Some(record) = self.resource_ordinary_channels.selected() {
            record.error.property_case_context.as_deref()
        } else {
            self.property_case_context.as_deref()
        }
    }

    pub(super) fn set_dynamic_failure_message(&mut self, message: Vec<u8>) {
        if let Some(channel) = self.resource_ordinary_channels.running_mut() {
            if channel.failure.is_none() {
                channel.dynamic_failure_message = Some(message);
            }
        } else if self.failure.is_none() {
            self.dynamic_failure_message = Some(message);
        }
    }

    pub(super) fn set_property_case_context(&mut self, context: Option<Vec<u8>>) {
        if let Some(channel) = self.resource_ordinary_channels.running_mut() {
            if channel.failure.is_none() {
                channel.property_case_context = context;
            }
        } else if self.failure.is_none() {
            self.property_case_context = context;
        }
    }

    pub(super) fn record_ordinary_failure(&mut self, error: Failure) {
        if let Some(channel) = self.resource_ordinary_channels.running_mut() {
            channel.failure.get_or_insert(error);
        } else {
            self.failure.get_or_insert(error);
        }
    }

    pub(super) fn can_capture_refinement_failure(&self) -> bool {
        !matches!(
            self.resource_ordinary_channels
                .selected()
                .map(|record| &record.phase),
            Some(OrdinaryAttemptPhase::Sealed(_))
        )
    }

    // This is the existing Source refinement conversion only. It cannot reach a
    // sealed terminal attempt and is never called by the entry constructor.
    pub(super) fn finish_refinement_failure_capture(&mut self) {
        if let Some(channel) = self.resource_ordinary_channels.running_mut() {
            channel.failure = None;
            channel.dynamic_failure_message = None;
        } else {
            self.failure = None;
            self.dynamic_failure_message = None;
        }
    }

    pub(in crate::native_abi) fn prepare_resource_ordinary_attempt(
        &mut self,
        owner: ResourceOrdinaryOwner,
        predecessor: Option<ResourceOrdinaryCompletion>,
    ) -> LeafResult<PreparedResourceOrdinaryAttempt<'_>> {
        if self.failure.is_some()
            || self.cleanup_failed
            || !self.is_empty()
            || !owner.within_attempt_budget()
            || owner.ordinal() != self.resource_ordinary_channels.records.len()
        {
            return Err(INVALID_HANDLE);
        }
        match (self.resource_ordinary_channels.selected(), predecessor) {
            (None, None) if self.resource_ordinary_channels.records.is_empty() => {}
            (Some(record), Some(previous)) => match record.phase {
                OrdinaryAttemptPhase::Sealed(sealed)
                    if sealed == previous
                        && sealed.diagnostic_ready()
                        && record.owner == sealed.owner()
                        && owner.continues_installation(record.owner) => {}
                OrdinaryAttemptPhase::Running | OrdinaryAttemptPhase::Sealed(_) => {
                    return Err(INVALID_HANDLE);
                }
            },
            _ => return Err(INVALID_HANDLE),
        }
        #[cfg(test)]
        {
            let channels = &mut self.resource_ordinary_channels;
            channels
                .reservation_faults
                .checkpoint(
                    ReservationSite::OrdinaryChannel,
                    channels.records.len() == channels.records.capacity(),
                )
                .map_err(|_| EXHAUSTED)?;
        }
        self.resource_ordinary_channels
            .records
            .try_reserve(1)
            .map_err(|_| EXHAUSTED)?;
        Ok(PreparedResourceOrdinaryAttempt {
            values: self,
            owner,
        })
    }

    pub(in crate::native_abi) fn validate_resource_ordinary_attempt(
        &self,
        owner: ResourceOrdinaryOwner,
    ) -> LeafResult<()> {
        match self.resource_ordinary_channels.selected() {
            Some(record)
                if record.owner == owner
                    && matches!(record.phase, OrdinaryAttemptPhase::Running) =>
            {
                Ok(())
            }
            _ => Err(INVALID_HANDLE),
        }
    }

    pub(in crate::native_abi) fn seal_resource_ordinary_attempt(
        &mut self,
        completion: ResourceOrdinaryCompletion,
    ) -> LeafResult<()> {
        self.validate_resource_ordinary_attempt(completion.owner())?;
        if !completion.matches_owner() {
            return Err(INVALID_HANDLE);
        }
        let record = self
            .resource_ordinary_channels
            .selected
            .and_then(|index| self.resource_ordinary_channels.records.get_mut(index))
            .ok_or(INVALID_HANDLE)?;
        record.phase = OrdinaryAttemptPhase::Sealed(completion);
        Ok(())
    }
}

#[cfg(test)]
impl NativeValues {
    pub(in crate::native_abi) fn test_arm_resource_ordinary_reservation(&mut self) {
        self.resource_ordinary_channels
            .reservation_faults
            .arm(ReservationSite::OrdinaryChannel);
    }

    pub(in crate::native_abi) fn test_resource_ordinary_reservation_snapshot(
        &self,
    ) -> (usize, usize, bool) {
        self.resource_ordinary_channels
            .reservation_faults
            .snapshot(ReservationSite::OrdinaryChannel)
    }

    pub(in crate::native_abi) fn test_resource_ordinary_channel_capacity(&self) -> (usize, usize) {
        let channels = &self.resource_ordinary_channels;
        (channels.records.len(), channels.records.capacity())
    }

    pub(in crate::native_abi) fn test_resource_ordinary_channel_state(
        &self,
    ) -> (
        usize,
        Option<ResourceOrdinaryOwner>,
        Option<ResourceOrdinaryCompletion>,
    ) {
        let channels = &self.resource_ordinary_channels;
        let selected = channels.selected();
        let completion = selected.and_then(|record| match record.phase {
            OrdinaryAttemptPhase::Running => None,
            OrdinaryAttemptPhase::Sealed(completion) => Some(completion),
        });
        (
            channels.records.len(),
            selected.map(|record| record.owner),
            completion,
        )
    }
}
