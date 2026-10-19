//! Strict owned observations from the matched private Resource launcher.
//! This module grants no Source, provider, registry or custody authority.

const MAX_REPORT: usize = 16 * 1024 * 1024;
const MAX_ATTEMPTS: u32 = 64;
const MAX_EVENTS: u32 = 8192;
const MAX_MESSAGE: u32 = 65_536;
const MAX_SCRIPT_REMAINING: u64 = 4096;

#[derive(Debug, PartialEq, Eq)]
pub(super) enum DecodeError {
    TooLarge,
    Truncated {
        offset: usize,
        needed: usize,
    },
    Magic,
    Version(u32),
    Length {
        declared: u64,
        actual: usize,
    },
    Reserved {
        field: &'static str,
        value: u32,
    },
    Limit {
        field: &'static str,
        value: u64,
        maximum: u64,
    },
    Flag {
        field: &'static str,
        value: u32,
    },
    Channel(u32),
    EventKind(u32),
    AttemptOrder(usize),
    EventOrder(usize),
    EventEnd {
        attempt: usize,
        end: u64,
    },
    Completion(usize),
    Teardown,
    Trailing(usize),
    Allocation,
    Observation,
    ObservationFlags(u32),
    HistoryKind(u32),
    ParameterValue(u32),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum FailureChannel {
    Clean,
    Body,
    HostPanic,
    Cleanup,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum EventKind {
    Constructed,
    ConstructionFailed,
    Borrowed,
    BorrowFailed,
    Finalized,
}

#[derive(Debug, PartialEq, Eq)]
pub(super) struct Completion {
    pub(super) attempt: u64,
    pub(super) body_status: u32,
    pub(super) cleanup_status: u32,
    pub(super) selected_channel: FailureChannel,
}
#[derive(Debug, PartialEq, Eq)]
pub(super) struct Counts {
    pub(super) owners: u64,
    pub(super) loans: u64,
    pub(super) frames: u64,
    pub(super) registry_entries: u64,
    pub(super) owner_handles: u64,
    pub(super) loan_handles: u64,
    pub(super) frame_handles: u64,
    pub(super) provisional_returns: u64,
    pub(super) ordinary_empty: bool,
}
impl Counts {
    pub(super) fn resources_retired(&self) -> bool {
        [
            self.owners,
            self.loans,
            self.frames,
            self.registry_entries,
            self.owner_handles,
            self.loan_handles,
            self.frame_handles,
            self.provisional_returns,
        ]
        .into_iter()
        .all(|count| count == 0)
    }
}
#[derive(Debug, PartialEq, Eq)]
pub(super) struct Attempt {
    pub(super) completion: Completion,
    pub(super) counts: Counts,
    pub(super) ordinary_message: Vec<u8>,
    pub(super) resource_message: Vec<u8>,
    pub(super) event_sequence_end: u64,
}
#[derive(Debug, PartialEq, Eq)]
pub(super) struct Event {
    pub(super) sequence: u64,
    pub(super) label: i64,
    pub(super) kind: EventKind,
}
#[derive(Debug, PartialEq, Eq)]
pub(super) struct Report {
    pub(super) observations: Snapshot,
    pub(super) attempts: Vec<Attempt>,
    pub(super) events: Vec<Event>,
    pub(super) script_remaining: u64,
    pub(super) destroy_status: u32,
    pub(super) created: bool,
    pub(super) destroy_attempts: u32,
    pub(super) write_status: u32,
}
impl Report {
    /// Counts are the real observations copied BEFORE destruction. A cleanup
    /// panic can still retire every owner, and remains visible in its channel.
    pub(super) fn resources_retired(&self) -> bool {
        self.attempts
            .iter()
            .all(|attempt| attempt.counts.resources_retired())
    }
    pub(super) fn ordinary_storage_empty(&self) -> bool {
        self.attempts
            .iter()
            .all(|attempt| attempt.counts.ordinary_empty)
    }
    pub(super) fn cleanup_succeeded(&self) -> bool {
        self.attempts
            .iter()
            .all(|attempt| attempt.completion.cleanup_status == 0)
    }
    pub(super) fn script_consumed(&self) -> bool {
        self.script_remaining == 0
    }
    pub(super) fn teardown_succeeded(&self) -> bool {
        self.created
            && self.destroy_attempts == 1
            && self.destroy_status == 0
            && self.write_status == 0
    }
    pub(super) fn events_all_assigned(&self) -> bool {
        self.attempts
            .last()
            .map_or(0, |attempt| attempt.event_sequence_end)
            == self.events.last().map_or(0, |event| event.sequence)
    }
    /// Expected Source body/cleanup outcomes, process exit, stdout and stderr
    /// remain separate exact harness oracles. This never treats a panic as clean.
    pub(super) fn observations_ready_for_acceptance(&self) -> bool {
        !self.attempts.is_empty()
            && self.observations.complete(self.attempts.len())
            && self.resources_retired()
            && self.ordinary_storage_empty()
            && self.script_consumed()
            && self.teardown_succeeded()
            && self.events_all_assigned()
    }
    pub(super) fn all_attempts_succeeded(&self) -> bool {
        !self.attempts.is_empty()
            && self.attempts.iter().all(|attempt| {
                attempt.completion.selected_channel == FailureChannel::Clean
                    && attempt.completion.body_status == 0
                    && attempt.completion.cleanup_status == 0
            })
    }
}

const MAX_HISTORY: u32 = 8192;
const MAX_CONTEXT: u32 = 65_536;
const MAX_CONTEXT_BYTES: u64 = 1_048_576;
const MAX_PARAMETERS: u32 = 1024;
const KNOWN_OBSERVATION_FLAGS: u32 = 63;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Counters {
    pub(super) values: [u64; 5],
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct AttemptIdentity {
    pub(super) id: u64,
    pub(super) ordinal: u32,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Channel {
    Session,
    Attempt(AttemptIdentity),
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct CallIdentity {
    pub(super) call: u64,
    pub(super) scope: u64,
    pub(super) function: u32,
    pub(super) signature: u32,
    pub(super) template: u32,
    pub(super) operation: u32,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ParameterValue {
    Int64 { bits: u64 },
    Unobserved,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Parameter {
    pub(super) parameter: u32,
    pub(super) shape: u32,
    pub(super) value: ParameterValue,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Boundary {
    pub(super) sequence: Option<u64>,
    pub(super) counters: Counters,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct AttemptObservation {
    pub(super) identity: AttemptIdentity,
    pub(super) begin: Boundary,
    pub(super) end: Option<Boundary>,
}
#[derive(Debug, PartialEq, Eq)]
pub(super) enum HistoryKind {
    Begin,
    End,
    Set,
    Clear,
    CallEnter {
        call: CallIdentity,
        parameters: Vec<Parameter>,
    },
    CallComplete {
        call: CallIdentity,
        entered_sequence: Option<u64>,
        body_status: u32,
        failure_status: Option<u32>,
    },
}
#[derive(Debug, PartialEq, Eq)]
pub(super) struct HistoryEvent {
    pub(super) sequence: u64,
    pub(super) channel: Channel,
    pub(super) context: Option<Vec<u8>>,
    pub(super) kind: HistoryKind,
}
#[derive(Debug, PartialEq, Eq)]
pub(super) struct Snapshot {
    pub(super) counters: Counters,
    pub(super) flags: u32,
    pub(super) attempts: Vec<AttemptObservation>,
    pub(super) history: Vec<HistoryEvent>,
    pub(super) context_bytes: u64,
    pub(super) final_sequence: u64,
}
impl Counters {
    fn follows(self, previous: Self) -> bool {
        self.values[1] <= self.values[0]
            && self
                .values
                .iter()
                .zip(previous.values)
                .all(|(now, before)| *now >= before)
    }
}
impl Snapshot {
    fn complete(&self, attempts: usize) -> bool {
        self.flags == 0
            && self.attempts.len() == attempts
            && self.attempts.iter().all(|attempt| {
                attempt.begin.sequence.is_some()
                    && attempt.end.is_some_and(|end| end.sequence.is_some())
            })
            && self.history.iter().all(|event| {
                !matches!(
                    event.kind,
                    HistoryKind::CallComplete {
                        entered_sequence: None,
                        ..
                    }
                )
            })
    }
    fn validate(&self, attempts: &[Attempt]) -> Result<(), DecodeError> {
        let bad = || DecodeError::Observation;
        let zero = Counters { values: [0; 5] };
        let identity_known = |identity: AttemptIdentity| {
            attempts
                .get(identity.ordinal as usize)
                .is_some_and(|attempt| attempt.completion.attempt == identity.id)
        };
        let mut previous = zero;
        let mut previous_ordinal = None;
        let mut boundary_sequence = 0;
        for attempt in &self.attempts {
            if !identity_known(attempt.identity)
                || previous_ordinal.is_some_and(|ordinal| attempt.identity.ordinal <= ordinal)
                || !attempt.begin.counters.follows(previous)
            {
                return Err(bad());
            }
            previous_ordinal = Some(attempt.identity.ordinal);
            previous = attempt.begin.counters;
            for (boundary, beginning) in [(Some(attempt.begin), true), (attempt.end, false)] {
                let Some(boundary) = boundary else { continue };
                if !boundary.counters.follows(previous) {
                    return Err(bad());
                }
                previous = boundary.counters;
                if let Some(sequence) = boundary.sequence {
                    if sequence <= boundary_sequence {
                        return Err(bad());
                    }
                    let event = self
                        .history
                        .get(usize::try_from(sequence - 1).map_err(|_| bad())?)
                        .ok_or_else(bad)?;
                    if event.sequence != sequence
                        || event.channel != Channel::Attempt(attempt.identity)
                        || if beginning {
                            !matches!(event.kind, HistoryKind::Begin)
                        } else {
                            !matches!(event.kind, HistoryKind::End)
                        }
                    {
                        return Err(bad());
                    }
                    boundary_sequence = sequence;
                }
            }
        }
        if !self.counters.follows(previous) {
            return Err(bad());
        }
        let mut active = None;
        let mut current_context: Option<&[u8]> = None;
        let mut call_stack = Vec::new();
        call_stack
            .try_reserve_exact(self.history.len())
            .map_err(|_| DecodeError::Allocation)?;
        for (index, event) in self.history.iter().enumerate() {
            if event.sequence != u64::try_from(index).map_err(|_| bad())? + 1 {
                return Err(bad());
            }
            if let Channel::Attempt(identity) = event.channel {
                if !identity_known(identity) {
                    return Err(bad());
                }
                if self.flags == 0 && active.is_some_and(|current| current != identity) {
                    return Err(bad());
                }
            }
            match &event.kind {
                HistoryKind::Begin => {
                    let Channel::Attempt(identity) = event.channel else {
                        return Err(bad());
                    };
                    if (self.flags == 0 && active.is_some()) || event.context.is_some() {
                        return Err(bad());
                    }
                    active = Some(identity);
                    current_context = None;
                    if self.flags == 0
                        && !self.attempts.iter().any(|attempt| {
                            attempt.identity == identity
                                && attempt.begin.sequence == Some(event.sequence)
                        })
                    {
                        return Err(bad());
                    }
                }
                HistoryKind::End => {
                    let Channel::Attempt(identity) = event.channel else {
                        return Err(bad());
                    };
                    if self.flags == 0 && active != Some(identity) {
                        return Err(bad());
                    }
                    if self.flags == 0
                        && !self.attempts.iter().any(|attempt| {
                            attempt.identity == identity
                                && attempt
                                    .end
                                    .is_some_and(|end| end.sequence == Some(event.sequence))
                        })
                    {
                        return Err(bad());
                    }
                    if self.flags == 0 && !call_stack.is_empty() {
                        return Err(bad());
                    }
                    if self.flags == 0 && event.context.as_deref() != current_context {
                        return Err(bad());
                    }
                    active = None;
                    current_context = None;
                }
                HistoryKind::Set if event.context.is_none() => return Err(bad()),
                HistoryKind::Clear if event.context.is_some() => return Err(bad()),
                HistoryKind::CallEnter { call, parameters } => {
                    if !matches!(event.channel, Channel::Attempt(_))
                        || event.context.is_none()
                        || call.call == 0
                        || call.scope == 0
                    {
                        return Err(bad());
                    }
                    if self.flags == 0 {
                        if event.context.as_deref() != current_context {
                            return Err(bad());
                        }
                        call_stack.push(event.sequence);
                    }
                    if self.history[..index].iter().any(|prior| matches!(&prior.kind, HistoryKind::CallEnter { call: old, .. } if old.call == call.call || old.scope == call.scope)) { return Err(bad()); }
                    if parameters.iter().enumerate().any(|(index, parameter)| {
                        parameters[..index]
                            .iter()
                            .any(|prior| prior.parameter == parameter.parameter)
                    }) {
                        return Err(bad());
                    }
                }
                HistoryKind::CallComplete {
                    call,
                    entered_sequence,
                    body_status,
                    failure_status,
                } => {
                    if *failure_status == Some(0) || (failure_status.is_none() && *body_status != 0)
                    {
                        return Err(bad());
                    }
                    if self.flags == 0 && event.context.as_deref() != current_context {
                        return Err(bad());
                    }
                    if self.flags == 0 && call_stack.pop() != *entered_sequence {
                        return Err(bad());
                    }
                    if let Some(sequence) = entered_sequence {
                        if *sequence == 0 || *sequence >= event.sequence {
                            return Err(bad());
                        }
                        let entered = self
                            .history
                            .get(usize::try_from(sequence - 1).map_err(|_| bad())?)
                            .ok_or_else(bad)?;
                        if entered.channel != event.channel
                            || !matches!(&entered.kind, HistoryKind::CallEnter { call: original, .. } if original == call)
                        {
                            return Err(bad());
                        }
                        if self.history[..index].iter().any(|prior| matches!(&prior.kind, HistoryKind::CallComplete { entered_sequence: Some(old), .. } if old == sequence)) { return Err(bad()); }
                    } else if self.flags == 0 {
                        return Err(bad());
                    }
                }
                HistoryKind::Set | HistoryKind::Clear => {
                    if self.flags == 0 && event.channel == Channel::Session && active.is_some() {
                        return Err(bad());
                    }
                    if matches!(event.channel, Channel::Attempt(_)) {
                        current_context = event.context.as_deref();
                    }
                }
            }
            if self.flags == 0
                && matches!(event.channel, Channel::Attempt(_))
                && active.is_none()
                && !matches!(event.kind, HistoryKind::End)
            {
                return Err(bad());
            }
        }

        if self.flags == 0 && self.history.iter().any(|event| matches!(event.kind, HistoryKind::CallEnter { .. })
            && !self.history.iter().any(|completed| matches!(&completed.kind, HistoryKind::CallComplete { entered_sequence: Some(sequence), .. } if *sequence == event.sequence))) {
            return Err(bad());
        }
        if self.flags == 0 && (active.is_some() || !self.complete(attempts.len())) {
            return Err(bad());
        }
        Ok(())
    }
}

struct Cursor<'a> {
    bytes: &'a [u8],
    offset: usize,
}
impl<'a> Cursor<'a> {
    fn take(&mut self, length: usize) -> Result<&'a [u8], DecodeError> {
        let end = self
            .offset
            .checked_add(length)
            .ok_or(DecodeError::TooLarge)?;
        let output = self
            .bytes
            .get(self.offset..end)
            .ok_or(DecodeError::Truncated {
                offset: self.offset,
                needed: length,
            })?;
        self.offset = end;
        Ok(output)
    }
    fn u32(&mut self) -> Result<u32, DecodeError> {
        let data = self.take(4)?;
        Ok(u32::from_le_bytes([data[0], data[1], data[2], data[3]]))
    }
    fn u64(&mut self) -> Result<u64, DecodeError> {
        let data = self.take(8)?;
        Ok(u64::from_le_bytes([
            data[0], data[1], data[2], data[3], data[4], data[5], data[6], data[7],
        ]))
    }
    fn zero(&mut self, field: &'static str) -> Result<(), DecodeError> {
        let value = self.u32()?;
        if value != 0 {
            return Err(DecodeError::Reserved { field, value });
        }
        Ok(())
    }
    fn flag(&mut self, field: &'static str) -> Result<bool, DecodeError> {
        match self.u32()? {
            0 => Ok(false),
            1 => Ok(true),
            value => Err(DecodeError::Flag { field, value }),
        }
    }
    fn bounded(&mut self, field: &'static str, maximum: u32) -> Result<u32, DecodeError> {
        let value = self.u32()?;
        if value > maximum {
            return Err(DecodeError::Limit {
                field,
                value: u64::from(value),
                maximum: u64::from(maximum),
            });
        }
        Ok(value)
    }
    fn counts(&mut self) -> Result<Counts, DecodeError> {
        let result = Counts {
            owners: self.u64()?,
            loans: self.u64()?,
            frames: self.u64()?,
            registry_entries: self.u64()?,
            owner_handles: self.u64()?,
            loan_handles: self.u64()?,
            frame_handles: self.u64()?,
            provisional_returns: self.u64()?,
            ordinary_empty: self.flag("ordinary_empty")?,
        };
        self.zero("counts")?;
        Ok(result)
    }
    fn message(&mut self, length: u32) -> Result<Vec<u8>, DecodeError> {
        let bytes = self.take(length as usize)?;
        let mut result = Vec::new();
        result
            .try_reserve_exact(bytes.len())
            .map_err(|_| DecodeError::Allocation)?;
        result.extend_from_slice(bytes);
        Ok(result)
    }
    fn attempt(&mut self, index: usize) -> Result<Attempt, DecodeError> {
        let attempt = self.u64()?;
        let body_status = self.u32()?;
        let cleanup_status = self.u32()?;
        let selected_channel = match self.u32()? {
            0 => FailureChannel::Clean,
            1 => FailureChannel::Body,
            2 => FailureChannel::HostPanic,
            3 => FailureChannel::Cleanup,
            value => return Err(DecodeError::Channel(value)),
        };
        self.zero("completion")?;
        if attempt == 0
            || match selected_channel {
                FailureChannel::Clean => body_status != 0 || cleanup_status != 0,
                FailureChannel::Body | FailureChannel::HostPanic => cleanup_status != 0,
                FailureChannel::Cleanup => cleanup_status == 0,
            }
        {
            return Err(DecodeError::Completion(index));
        }
        let counts = self.counts()?;
        let ordinary_length = self.bounded("ordinary_message", MAX_MESSAGE)?;
        let resource_length = self.bounded("resource_message", MAX_MESSAGE)?;
        let event_sequence_end = self.u64()?;
        let ordinary_message = self.message(ordinary_length)?;
        let resource_message = self.message(resource_length)?;
        Ok(Attempt {
            completion: Completion {
                attempt,
                body_status,
                cleanup_status,
                selected_channel,
            },
            counts,
            ordinary_message,
            resource_message,
            event_sequence_end,
        })
    }
    fn event(&mut self) -> Result<Event, DecodeError> {
        let sequence = self.u64()?;
        let label = i64::from_le_bytes(
            self.take(8)?
                .try_into()
                .map_err(|_| DecodeError::TooLarge)?,
        );
        let kind = match self.u32()? {
            1 => EventKind::Constructed,
            2 => EventKind::ConstructionFailed,
            3 => EventKind::Borrowed,
            4 => EventKind::BorrowFailed,
            5 => EventKind::Finalized,
            value => return Err(DecodeError::EventKind(value)),
        };
        self.zero("event")?;
        Ok(Event {
            sequence,
            label,
            kind,
        })
    }
}

impl Cursor<'_> {
    fn optional_u64(&mut self, field: &'static str) -> Result<Option<u64>, DecodeError> {
        if self.flag(field)? {
            self.u64().map(Some)
        } else {
            Ok(None)
        }
    }
    fn optional_u32(&mut self, field: &'static str) -> Result<Option<u32>, DecodeError> {
        if self.flag(field)? {
            self.u32().map(Some)
        } else {
            Ok(None)
        }
    }
    fn observation_counters(&mut self) -> Result<Counters, DecodeError> {
        Ok(Counters {
            values: [
                self.u64()?,
                self.u64()?,
                self.u64()?,
                self.u64()?,
                self.u64()?,
            ],
        })
    }
    fn observation_boundary(&mut self) -> Result<Boundary, DecodeError> {
        Ok(Boundary {
            sequence: self.optional_u64("boundary_sequence")?,
            counters: self.observation_counters()?,
        })
    }
    fn observation_identity(&mut self) -> Result<AttemptIdentity, DecodeError> {
        Ok(AttemptIdentity {
            id: self.u64()?,
            ordinal: self.u32()?,
        })
    }
    fn call_identity(&mut self) -> Result<CallIdentity, DecodeError> {
        Ok(CallIdentity {
            call: self.u64()?,
            scope: self.u64()?,
            function: self.u32()?,
            signature: self.u32()?,
            template: self.u32()?,
            operation: self.u32()?,
        })
    }
    fn observations(&mut self, attempts: &[Attempt]) -> Result<Snapshot, DecodeError> {
        let counters = self.observation_counters()?;
        let flags = self.u32()?;
        if flags & !KNOWN_OBSERVATION_FLAGS != 0 {
            return Err(DecodeError::ObservationFlags(flags));
        }
        let count = self.bounded("observed_attempts", MAX_ATTEMPTS)?;
        let history_count = self.bounded("history", MAX_HISTORY)?;
        let context_bytes = self.u64()?;
        if context_bytes > MAX_CONTEXT_BYTES {
            return Err(DecodeError::Limit {
                field: "context_bytes",
                value: context_bytes,
                maximum: MAX_CONTEXT_BYTES,
            });
        }
        let final_sequence = self.u64()?;
        let minimum = (count as usize)
            .checked_mul(60)
            .and_then(|rows| {
                (history_count as usize)
                    .checked_mul(20)
                    .and_then(|events| rows.checked_add(events))
            })
            .and_then(|value| value.checked_add(16))
            .ok_or(DecodeError::TooLarge)?;
        if minimum > self.bytes.len() - self.offset {
            return Err(DecodeError::Truncated {
                offset: self.offset,
                needed: minimum,
            });
        }
        let mut observed = Vec::new();
        let mut history = Vec::new();
        observed
            .try_reserve_exact(count as usize)
            .map_err(|_| DecodeError::Allocation)?;
        history
            .try_reserve_exact(history_count as usize)
            .map_err(|_| DecodeError::Allocation)?;
        for _ in 0..count {
            let identity = self.observation_identity()?;
            let begin = self.observation_boundary()?;
            let end = if self.flag("end_present")? {
                Some(self.observation_boundary()?)
            } else {
                None
            };
            observed.push(AttemptObservation {
                identity,
                begin,
                end,
            });
        }
        let mut copied_context = 0u64;
        for _ in 0..history_count {
            let sequence = self.u64()?;
            let kind = self.u32()?;
            let channel = if self.flag("attempt_present")? {
                Channel::Attempt(self.observation_identity()?)
            } else {
                Channel::Session
            };
            let context = if self.flag("context_present")? {
                let length = self.bounded("context_length", MAX_CONTEXT)?;
                copied_context = copied_context
                    .checked_add(u64::from(length))
                    .ok_or(DecodeError::TooLarge)?;
                if copied_context > context_bytes {
                    return Err(DecodeError::Observation);
                }
                Some(self.message(length)?)
            } else {
                None
            };
            let kind = match kind {
                1 => HistoryKind::Begin,
                2 => HistoryKind::Set,
                3 => {
                    let call = self.call_identity()?;
                    let count = self.bounded("parameters", MAX_PARAMETERS)?;
                    let minimum = (count as usize)
                        .checked_mul(12)
                        .ok_or(DecodeError::TooLarge)?;
                    if minimum > self.bytes.len() - self.offset {
                        return Err(DecodeError::Truncated {
                            offset: self.offset,
                            needed: minimum,
                        });
                    }
                    let mut parameters = Vec::new();
                    parameters
                        .try_reserve_exact(count as usize)
                        .map_err(|_| DecodeError::Allocation)?;
                    for _ in 0..count {
                        let parameter = self.u32()?;
                        let shape = self.u32()?;
                        let tag = self.u32()?;
                        let value = match tag {
                            0 => ParameterValue::Unobserved,
                            1 => ParameterValue::Int64 { bits: self.u64()? },
                            _ => return Err(DecodeError::ParameterValue(tag)),
                        };
                        parameters.push(Parameter {
                            parameter,
                            shape,
                            value,
                        });
                    }
                    HistoryKind::CallEnter { call, parameters }
                }
                4 => HistoryKind::CallComplete {
                    call: self.call_identity()?,
                    entered_sequence: self.optional_u64("entered_sequence")?,
                    body_status: self.u32()?,
                    failure_status: self.optional_u32("failure_status")?,
                },
                5 => HistoryKind::Clear,
                6 => HistoryKind::End,
                value => return Err(DecodeError::HistoryKind(value)),
            };
            history.push(HistoryEvent {
                sequence,
                channel,
                context,
                kind,
            });
        }
        if copied_context != context_bytes || final_sequence != u64::from(history_count) {
            return Err(DecodeError::Observation);
        }
        let result = Snapshot {
            counters,
            flags,
            attempts: observed,
            history,
            context_bytes,
            final_sequence,
        };
        result.validate(attempts)?;
        Ok(result)
    }
}

pub(super) fn decode(bytes: &[u8]) -> Result<Report, DecodeError> {
    if bytes.len() > MAX_REPORT {
        return Err(DecodeError::TooLarge);
    }
    let mut cursor = Cursor { bytes, offset: 0 };
    if cursor.take(8)? != b"JTRRP002" {
        return Err(DecodeError::Magic);
    }
    let version = cursor.u32()?;
    if version != 2 {
        return Err(DecodeError::Version(version));
    }
    cursor.zero("header")?;
    let declared = cursor.u64()?;
    if declared != bytes.len() as u64 {
        return Err(DecodeError::Length {
            declared,
            actual: bytes.len(),
        });
    }
    let attempt_count = cursor.bounded("attempts", MAX_ATTEMPTS)?;
    let event_count = cursor.bounded("events", MAX_EVENTS)?;
    let script_remaining = cursor.u64()?;
    if script_remaining > MAX_SCRIPT_REMAINING {
        return Err(DecodeError::Limit {
            field: "script_remaining",
            value: script_remaining,
            maximum: MAX_SCRIPT_REMAINING,
        });
    }
    let mut attempts = Vec::new();
    let mut events = Vec::new();
    attempts
        .try_reserve_exact(attempt_count as usize)
        .map_err(|_| DecodeError::Allocation)?;
    events
        .try_reserve_exact(event_count as usize)
        .map_err(|_| DecodeError::Allocation)?;
    let mut previous_attempt = 0;
    let mut previous_end = 0;
    for index in 0..attempt_count as usize {
        let attempt = cursor.attempt(index)?;
        if attempt.completion.attempt <= previous_attempt {
            return Err(DecodeError::AttemptOrder(index));
        }
        if attempt.event_sequence_end < previous_end {
            return Err(DecodeError::EventEnd {
                attempt: index,
                end: attempt.event_sequence_end,
            });
        }
        previous_attempt = attempt.completion.attempt;
        previous_end = attempt.event_sequence_end;
        attempts.push(attempt);
    }
    let mut previous_sequence = 0;
    for index in 0..event_count as usize {
        let event = cursor.event()?;
        if event.sequence <= previous_sequence {
            return Err(DecodeError::EventOrder(index));
        }
        previous_sequence = event.sequence;
        events.push(event);
    }
    for (index, attempt) in attempts.iter().enumerate() {
        let end = attempt.event_sequence_end;
        if end != 0
            && events
                .binary_search_by_key(&end, |event| event.sequence)
                .is_err()
        {
            return Err(DecodeError::EventEnd {
                attempt: index,
                end,
            });
        }
    }
    let observations = cursor.observations(&attempts)?;
    let destroy_status = cursor.u32()?;
    let created = cursor.flag("created")?;
    let destroy_attempts = cursor.u32()?;
    let write_status = cursor.u32()?;
    if destroy_attempts != u32::from(created)
        || (!created && (destroy_status != 0 || !attempts.is_empty() || !events.is_empty()))
    {
        return Err(DecodeError::Teardown);
    }
    if cursor.offset != bytes.len() {
        return Err(DecodeError::Trailing(bytes.len() - cursor.offset));
    }
    Ok(Report {
        observations,
        attempts,
        events,
        script_remaining,
        destroy_status,
        created,
        destroy_attempts,
        write_status,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn word(bytes: &mut Vec<u8>, value: u32) {
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    fn wide(bytes: &mut Vec<u8>, value: u64) {
        bytes.extend_from_slice(&value.to_le_bytes());
    }

    const FIRST_PROVIDER_EVENT: usize = 40 + 112 + 10 + 112;
    struct ObservationOffsets {
        start: usize,
        rows: [usize; 2],
        history: [usize; 8],
        parameter_tag: usize,
    }
    fn observation_fixture(bytes: &mut Vec<u8>) -> ObservationOffsets {
        let start = bytes.len();
        for _ in 0..5 {
            wide(bytes, 0);
        }
        word(bytes, 0);
        word(bytes, 2);
        word(bytes, 8);
        wide(bytes, 6);
        wide(bytes, 8);
        let mut rows = [0; 2];
        for (index, (id, begin, end)) in [(41, 1, 6), (44, 7, 8)].into_iter().enumerate() {
            rows[index] = bytes.len();
            wide(bytes, id);
            word(bytes, index as u32);
            word(bytes, 1);
            wide(bytes, begin);
            for _ in 0..5 {
                wide(bytes, 0);
            }
            word(bytes, 1);
            word(bytes, 1);
            wide(bytes, end);
            for _ in 0..5 {
                wide(bytes, 0);
            }
        }
        let mut history = [0; 8];
        let mut parameter_tag = 0;
        for (index, kind) in [1, 2, 3, 4, 5, 6, 1, 6].into_iter().enumerate() {
            history[index] = bytes.len();
            wide(bytes, index as u64 + 1);
            word(bytes, kind);
            word(bytes, 1);
            wide(bytes, if index < 6 { 41 } else { 44 });
            word(bytes, u32::from(index >= 6));
            if (1..=3).contains(&index) {
                word(bytes, 1);
                word(bytes, 2);
                bytes.extend_from_slice(&[0, 255]);
            } else {
                word(bytes, 0);
            }
            if kind == 3 || kind == 4 {
                wide(bytes, 11);
                wide(bytes, 12);
                for value in [7, 8, 9, 10] {
                    word(bytes, value);
                }
                if kind == 3 {
                    word(bytes, 1);
                    word(bytes, 0);
                    word(bytes, 1);
                    parameter_tag = bytes.len();
                    word(bytes, 1);
                    wide(bytes, i64::MIN as u64);
                } else {
                    word(bytes, 1);
                    wide(bytes, 3);
                    word(bytes, 0);
                    word(bytes, 0);
                }
            }
        }
        ObservationOffsets {
            start,
            rows,
            history,
            parameter_tag,
        }
    }
    fn observation_offsets() -> ObservationOffsets {
        let mut bytes = vec![0; FIRST_PROVIDER_EVENT + 48];
        observation_fixture(&mut bytes)
    }

    fn fixture() -> Vec<u8> {
        let mut bytes = b"JTRRP002".to_vec();
        word(&mut bytes, 2);
        word(&mut bytes, 0);
        wide(&mut bytes, 0);
        word(&mut bytes, 2);
        word(&mut bytes, 2);
        wide(&mut bytes, 3);
        for (attempt, body, channel, end, ordinary, resource) in [
            (41, 71, 1, 1, &b"ordinary"[..], &[0xff, b'x'][..]),
            (44, 0, 0, 4, &[][..], &[][..]),
        ] {
            wide(&mut bytes, attempt);
            for value in [body, 0, channel, 0] {
                word(&mut bytes, value);
            }
            for _ in 0..8 {
                wide(&mut bytes, 0);
            }
            word(&mut bytes, 1);
            word(&mut bytes, 0);
            word(&mut bytes, ordinary.len() as u32);
            word(&mut bytes, resource.len() as u32);
            wide(&mut bytes, end);
            bytes.extend_from_slice(ordinary);
            bytes.extend_from_slice(resource);
        }
        for (sequence, label, kind) in [(1, -501i64, 3), (4, i64::MIN, 5)] {
            wide(&mut bytes, sequence);
            wide(&mut bytes, label as u64);
            word(&mut bytes, kind);
            word(&mut bytes, 0);
        }
        observation_fixture(&mut bytes);
        for value in [0, 1, 1, 0] {
            word(&mut bytes, value);
        }
        let length = bytes.len() as u64;
        bytes[16..24].copy_from_slice(&length.to_le_bytes());
        decode(&bytes).expect("valid uncorrupted v2 baseline before any mutation");
        bytes
    }
    fn set_word(bytes: &mut [u8], offset: usize, value: u32) {
        bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
    }
    fn set_wide(bytes: &mut [u8], offset: usize, value: u64) {
        bytes[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
    }
    #[test]
    fn resource_report_preserves_original_fields_and_keeps_acceptance_predicates_separate() {
        let report = decode(&fixture()).unwrap();
        assert_eq!(report.attempts.len(), 2);
        assert_eq!(report.attempts[0].completion.body_status, 71);
        assert_eq!(
            report.attempts[0].completion.selected_channel,
            FailureChannel::Body
        );
        assert_eq!(report.attempts[0].ordinary_message, b"ordinary");
        assert_eq!(report.attempts[0].resource_message, [0xff, b'x']);
        assert_eq!(report.events[0].label, -501);
        assert_eq!(report.events[1].label, i64::MIN);
        assert_eq!(report.events[0].kind, EventKind::Borrowed);
        assert!(
            report.resources_retired()
                && report.ordinary_storage_empty()
                && report.cleanup_succeeded()
        );
        assert!(report.teardown_succeeded() && report.events_all_assigned());
        assert!(
            !report.script_consumed()
                && !report.all_attempts_succeeded()
                && !report.observations_ready_for_acceptance()
        );
    }
    #[test]
    fn resource_report_rejects_every_truncation_trailing_bytes_and_declared_length_lies() {
        let bytes = fixture();
        for end in 0..bytes.len() {
            let mut prefix = bytes[..end].to_vec();
            if end >= 24 {
                set_wide(&mut prefix, 16, end as u64);
            }
            assert!(decode(&prefix).is_err(), "truncated prefix {end}");
        }
        let mut trailing = bytes.clone();
        trailing.push(0);
        let length = trailing.len() as u64;
        set_wide(&mut trailing, 16, length);
        assert_eq!(decode(&trailing), Err(DecodeError::Trailing(1)));
        let mut wrong = bytes.clone();
        set_wide(&mut wrong, 16, u64::MAX);
        assert_eq!(
            decode(&wrong),
            Err(DecodeError::Length {
                declared: u64::MAX,
                actual: bytes.len()
            })
        );
    }
    #[test]
    fn resource_report_rejects_oversized_storage_before_copying_or_allocating_messages() {
        assert_eq!(decode(&vec![0; MAX_REPORT + 1]), Err(DecodeError::TooLarge));
        for (offset, value, field, maximum) in [
            (24, 65, "attempts", 64),
            (28, 8193, "events", 8192),
            (136, 65537, "ordinary_message", 65536),
            (140, 65537, "resource_message", 65536),
        ] {
            let mut bytes = fixture();
            set_word(&mut bytes, offset, value);
            assert_eq!(
                decode(&bytes),
                Err(DecodeError::Limit {
                    field,
                    value: u64::from(value),
                    maximum
                })
            );
        }
        let mut bytes = fixture();
        set_wide(&mut bytes, 32, 4097);
        assert_eq!(
            decode(&bytes),
            Err(DecodeError::Limit {
                field: "script_remaining",
                value: 4097,
                maximum: 4096
            })
        );
    }
    #[test]
    fn resource_report_rejects_reserved_unknown_kinds_and_nonboolean_flags() {
        let bytes = fixture();
        let first_event = FIRST_PROVIDER_EVENT;
        let mut wrong = bytes.clone();
        wrong[0] = b'X';
        assert_eq!(decode(&wrong), Err(DecodeError::Magic));
        let mut wrong = bytes.clone();
        set_word(&mut wrong, 8, 1);
        assert_eq!(decode(&wrong), Err(DecodeError::Version(1)));
        for (offset, field) in [
            (12, "header"),
            (60, "completion"),
            (132, "counts"),
            (first_event + 20, "event"),
        ] {
            let mut wrong = bytes.clone();
            set_word(&mut wrong, offset, 9);
            assert_eq!(
                decode(&wrong),
                Err(DecodeError::Reserved { field, value: 9 })
            );
        }
        let mut wrong = bytes.clone();
        set_word(&mut wrong, 56, 4);
        assert_eq!(decode(&wrong), Err(DecodeError::Channel(4)));
        let mut wrong = bytes.clone();
        set_word(&mut wrong, first_event + 16, 6);
        assert_eq!(decode(&wrong), Err(DecodeError::EventKind(6)));
        let mut wrong = bytes.clone();
        set_word(&mut wrong, 128, 2);
        assert_eq!(
            decode(&wrong),
            Err(DecodeError::Flag {
                field: "ordinary_empty",
                value: 2
            })
        );
        let mut wrong = bytes.clone();
        let created = wrong.len() - 12;
        set_word(&mut wrong, created, 2);
        assert_eq!(
            decode(&wrong),
            Err(DecodeError::Flag {
                field: "created",
                value: 2
            })
        );
    }
    #[test]
    fn resource_report_rejects_nonmonotonic_identities_and_impossible_event_prefixes() {
        let bytes = fixture();
        let second_attempt = 162;
        let first_event = FIRST_PROVIDER_EVENT;
        let mut wrong = bytes.clone();
        set_wide(&mut wrong, second_attempt, 41);
        assert_eq!(decode(&wrong), Err(DecodeError::AttemptOrder(1)));
        let mut wrong = bytes.clone();
        set_wide(&mut wrong, first_event, 0);
        assert_eq!(decode(&wrong), Err(DecodeError::EventOrder(0)));
        let mut wrong = bytes.clone();
        set_wide(&mut wrong, first_event + 24, 1);
        assert_eq!(decode(&wrong), Err(DecodeError::EventOrder(1)));
        for end in [2, u64::MAX] {
            let mut wrong = bytes.clone();
            set_wide(&mut wrong, 144, end);
            assert!(matches!(decode(&wrong), Err(DecodeError::EventEnd { .. })));
        }
        let mut wrong = bytes.clone();
        set_wide(&mut wrong, second_attempt + 104, 0);
        assert_eq!(
            decode(&wrong),
            Err(DecodeError::EventEnd { attempt: 1, end: 0 })
        );
    }
    #[test]
    fn resource_report_never_masks_live_custody_cleanup_panic_or_teardown_failure() {
        let mut bytes = fixture();
        set_wide(&mut bytes, 32, 0);
        let mut report = decode(&bytes).unwrap();
        assert!(report.observations_ready_for_acceptance());
        assert!(!report.all_attempts_succeeded());
        report.attempts[0].counts.owners = 1;
        assert!(!report.resources_retired() && !report.observations_ready_for_acceptance());
        for index in 0..8 {
            let mut live = fixture();
            set_wide(&mut live, 64 + index * 8, 17);
            let report = decode(&live).unwrap();
            assert!(!report.resources_retired(), "live count {index}");
        }
        let mut unknown_end = fixture();
        set_wide(&mut unknown_end, 162 + 104, 1);
        let report = decode(&unknown_end).unwrap();
        assert!(!report.events_all_assigned());
        let mut panic = fixture();
        set_word(&mut panic, 48, 0);
        set_word(&mut panic, 56, 2);
        let report = decode(&panic).unwrap();
        assert_eq!(report.attempts[0].completion.body_status, 0);
        assert_eq!(
            report.attempts[0].completion.selected_channel,
            FailureChannel::HostPanic
        );
        assert!(!report.all_attempts_succeeded());
        let mut bytes = fixture();
        set_word(&mut bytes, 52, 37);
        set_word(&mut bytes, 56, 3);
        let report = decode(&bytes).unwrap();
        assert_eq!(report.attempts[0].completion.cleanup_status, 37);
        assert_eq!(report.attempts[0].completion.body_status, 71);
        assert_eq!(
            report.attempts[0].completion.selected_channel,
            FailureChannel::Cleanup
        );
        assert!(!report.cleanup_succeeded());
        assert!(report.resources_retired());
        let mut wrong = fixture();
        set_word(&mut wrong, 52, 37);
        assert_eq!(decode(&wrong), Err(DecodeError::Completion(0)));
        let mut wrong = fixture();
        set_word(&mut wrong, 56, 3);
        assert_eq!(decode(&wrong), Err(DecodeError::Completion(0)));
        let mut wrong = fixture();
        set_word(&mut wrong, 56, 0);
        assert_eq!(decode(&wrong), Err(DecodeError::Completion(0)));
        let mut wrong = fixture();
        let footer = wrong.len() - 16;
        set_word(&mut wrong, footer + 8, 2);
        assert_eq!(decode(&wrong), Err(DecodeError::Teardown));
        let mut failed = fixture();
        let footer = failed.len() - 16;
        set_word(&mut failed, footer, 13);
        set_word(&mut failed, footer + 12, 14);
        let report = decode(&failed).unwrap();
        assert_eq!((report.destroy_status, report.write_status), (13, 14));
        assert!(!report.teardown_succeeded());
    }
    #[test]
    fn resource_report_v2_retains_real_boundaries_raw_context_and_parameter_bits() {
        let mut bytes = fixture();
        set_wide(&mut bytes, 32, 0);
        let report = decode(&bytes).unwrap();
        assert!(report.observations_ready_for_acceptance());
        assert_eq!(report.observations.final_sequence, 8);
        assert_eq!(report.observations.context_bytes, 6);
        assert_eq!(
            report.observations.attempts[0].identity,
            AttemptIdentity { id: 41, ordinal: 0 }
        );
        assert_eq!(
            report.observations.history[1].context.as_deref(),
            Some(&[0, 255][..])
        );
        let HistoryKind::CallEnter { call, parameters } = &report.observations.history[2].kind
        else {
            panic!("actual entered call")
        };
        assert_eq!((call.call, call.scope, call.function), (11, 12, 7));
        assert_eq!(
            parameters[0].value,
            ParameterValue::Int64 {
                bits: i64::MIN as u64
            }
        );
        assert_eq!(report.observations.history[4].context, None);
    }
    #[test]
    fn resource_report_v2_rejects_closed_tag_limit_and_legacy_corruptions_after_valid_baseline() {
        let baseline = fixture();
        decode(&baseline).unwrap();
        let at = observation_offsets();
        let mut legacy = baseline.clone();
        legacy[..8].copy_from_slice(b"JTRRP001");
        assert_eq!(decode(&legacy), Err(DecodeError::Magic));
        for (offset, value) in [
            (at.start + 40, 64),
            (at.start + 44, 65),
            (at.start + 48, 8193),
            (at.rows[0] + 12, 2),
            (at.history[0] + 8, 99),
            (at.history[0] + 12, 2),
            (at.history[0] + 28, 2),
            (at.parameter_tag, 2),
        ] {
            let mut changed = baseline.clone();
            set_word(&mut changed, offset, value);
            assert!(decode(&changed).is_err(), "closed field {offset}");
        }
        let mut wrong_function = baseline.clone();
        set_word(&mut wrong_function, at.history[3] + 54, 99);
        assert!(decode(&wrong_function).is_err());
        let mut context_limit = baseline.clone();
        set_wide(&mut context_limit, at.start + 52, MAX_CONTEXT_BYTES + 1);
        assert!(decode(&context_limit).is_err());
    }
    #[test]
    fn resource_report_v2_rejects_counter_boundary_and_exact_call_join_corruption() {
        let baseline = fixture();
        decode(&baseline).unwrap();
        let at = observation_offsets();
        for (offset, value) in [
            (at.start + 8, 1),        // acquire subtype exceeds all-hook
            (at.rows[0] + 24, 1),     // begin counter decreases at End
            (at.rows[0] + 72, 1),     // duplicate Begin/End sequence
            (at.rows[1], 41),         // foreign/duplicate attempt identity
            (at.history[1], 1),       // noncontiguous sequence
            (at.history[3] + 16, 44), // completion transported to another attempt
            (at.history[3] + 38, 77), // completed call handle differs
            (at.history[3] + 46, 77), // completed scope differs
            (at.history[3] + 74, 4),  // completion does not reference a prior CallEnter
        ] {
            let mut changed = baseline.clone();
            set_wide(&mut changed, offset, value);
            assert!(decode(&changed).is_err(), "identity/counter {offset}");
        }
    }
    #[test]
    fn resource_report_v2_preserves_incomplete_and_unobserved_without_inventing_zero() {
        let mut baseline = fixture();
        set_wide(&mut baseline, 32, 0);
        decode(&baseline).unwrap();
        let at = observation_offsets();
        for flag in [1, 2, 4, 8, 16, 32] {
            let mut changed = baseline.clone();
            set_word(&mut changed, at.start + 40, flag);
            let report = decode(&changed).unwrap();
            assert_eq!(report.observations.flags, flag);
            assert!(!report.observations_ready_for_acceptance());
        }
        // A known counter1 is retained as1, including an incomplete report.
        let mut changed = baseline.clone();
        set_wide(&mut changed, at.start, 1);
        set_word(&mut changed, at.start + 40, 8);
        let report = decode(&changed).unwrap();
        assert_eq!(report.observations.counters.values[0], 1);
        assert!(!report.observations_ready_for_acceptance());
        // Explicit missing sequence consumes no invented sequence word.
        let mut unknown = baseline.clone();
        set_word(&mut unknown, at.start + 40, 8);
        set_word(&mut unknown, at.rows[0] + 12, 0);
        unknown.drain(at.rows[0] + 16..at.rows[0] + 24);
        let length = unknown.len() as u64;
        set_wide(&mut unknown, 16, length);
        let report = decode(&unknown).unwrap();
        assert_eq!(report.observations.attempts[0].begin.sequence, None);
        assert!(!report.observations_ready_for_acceptance());
        set_word(&mut unknown, at.start + 40, 0);
        assert!(
            decode(&unknown).is_err(),
            "unknown boundary cannot claim complete telemetry"
        );
        let mut unobserved = baseline.clone();
        set_word(&mut unobserved, at.parameter_tag, 0);
        unobserved.drain(at.parameter_tag + 4..at.parameter_tag + 12);
        let length = unobserved.len() as u64;
        set_wide(&mut unobserved, 16, length);
        let report = decode(&unobserved).unwrap();
        let HistoryKind::CallEnter { parameters, .. } = &report.observations.history[2].kind else {
            panic!("call")
        };
        assert_eq!(parameters[0].value, ParameterValue::Unobserved);
    }

    #[test]
    fn resource_report_v2_preserves_empty_context_and_refuses_missing_completion() {
        let baseline = fixture();
        decode(&baseline).unwrap();
        let at = observation_offsets();
        let mut empty = baseline.clone();
        for index in [3, 2, 1] {
            set_word(&mut empty, at.history[index] + 32, 0);
            empty.drain(at.history[index] + 36..at.history[index] + 38);
        }
        set_wide(&mut empty, at.start + 52, 0);
        let length = empty.len() as u64;
        set_wide(&mut empty, 16, length);
        let report = decode(&empty).unwrap();
        assert_eq!(
            report.observations.history[1].context.as_deref(),
            Some(&[][..])
        );
        assert_eq!(report.observations.history[4].context, None);
        let mut report = decode(&baseline).unwrap();
        report.observations.history[3].kind = HistoryKind::Set;
        assert!(
            report.observations.validate(&report.attempts).is_err(),
            "an entered call must actually complete before End"
        );
    }

    fn nested_clean_report() -> Report {
        let mut value = decode(&fixture()).unwrap();
        let identity = AttemptIdentity { id: 41, ordinal: 0 };
        let second = AttemptIdentity { id: 44, ordinal: 1 };
        let parent = CallIdentity {
            call: 11,
            scope: 12,
            function: 7,
            signature: 8,
            template: 9,
            operation: 10,
        };
        let child = CallIdentity {
            call: 21,
            scope: 22,
            function: 17,
            signature: 18,
            template: 19,
            operation: 20,
        };
        let kinds = [
            (identity, None, HistoryKind::Begin),
            (identity, Some(b"trial".to_vec()), HistoryKind::Set),
            (
                identity,
                Some(b"trial".to_vec()),
                HistoryKind::CallEnter {
                    call: parent,
                    parameters: vec![],
                },
            ),
            (
                identity,
                Some(b"trial".to_vec()),
                HistoryKind::CallEnter {
                    call: child,
                    parameters: vec![],
                },
            ),
            (
                identity,
                Some(b"trial".to_vec()),
                HistoryKind::CallComplete {
                    call: child,
                    entered_sequence: Some(4),
                    body_status: 0,
                    failure_status: None,
                },
            ),
            (
                identity,
                Some(b"trial".to_vec()),
                HistoryKind::CallComplete {
                    call: parent,
                    entered_sequence: Some(3),
                    body_status: 0,
                    failure_status: None,
                },
            ),
            (identity, None, HistoryKind::Clear),
            (identity, None, HistoryKind::End),
            (second, None, HistoryKind::Begin),
            (second, None, HistoryKind::End),
        ];
        value.observations.history = kinds
            .into_iter()
            .enumerate()
            .map(|(index, (identity, context, kind))| HistoryEvent {
                sequence: index as u64 + 1,
                channel: Channel::Attempt(identity),
                context,
                kind,
            })
            .collect();
        value.observations.context_bytes = 25;
        value.observations.final_sequence = 10;
        value.observations.attempts[0]
            .end
            .as_mut()
            .unwrap()
            .sequence = Some(8);
        value.observations.attempts[1].begin.sequence = Some(9);
        value.observations.attempts[1]
            .end
            .as_mut()
            .unwrap()
            .sequence = Some(10);
        value.observations.validate(&value.attempts).unwrap();
        value
    }
    #[test]
    fn resource_report_v2_actual_success_none_and_nested_context_lifo_are_checked() {
        let baseline = nested_clean_report();
        assert!(matches!(
            baseline.observations.history[4].kind,
            HistoryKind::CallComplete {
                failure_status: None,
                body_status: 0,
                ..
            }
        ));
        for mutation in 0..8 {
            let mut changed = nested_clean_report();
            match mutation {
                0 => {
                    let left = std::mem::replace(
                        &mut changed.observations.history[4].kind,
                        HistoryKind::Clear,
                    );
                    let right = std::mem::replace(&mut changed.observations.history[5].kind, left);
                    changed.observations.history[4].kind = right;
                }
                1 => {
                    changed.observations.history[1].kind = HistoryKind::Clear;
                    changed.observations.history[1].context = None;
                }
                2 => changed.observations.history[2].context = Some(b"wrong".to_vec()),
                3 => changed.observations.history[1].channel = Channel::Session,
                4 => {
                    if let HistoryKind::CallComplete { failure_status, .. } =
                        &mut changed.observations.history[4].kind
                    {
                        *failure_status = Some(0);
                    }
                }
                5 => {
                    if let HistoryKind::CallComplete { body_status, .. } =
                        &mut changed.observations.history[4].kind
                    {
                        *body_status = 71;
                    }
                }
                6 => changed.observations.history[4].context = Some(b"wrong".to_vec()),
                7 => changed.observations.history[7].context = Some(vec![]),
                _ => unreachable!(),
            }
            assert!(
                changed.observations.validate(&changed.attempts).is_err(),
                "actual completion/context mutation {mutation}"
            );
        }
        let mut cleared = nested_clean_report();
        let clear = cleared.observations.history.remove(6);
        cleared.observations.history.insert(4, clear);
        for (index, event) in cleared.observations.history.iter_mut().enumerate() {
            event.sequence = index as u64 + 1;
        }
        cleared.observations.history[5].context = None;
        cleared.observations.history[6].context = None;
        cleared.observations.context_bytes = 15;
        cleared.observations.validate(&cleared.attempts).unwrap();
        let mut failed = nested_clean_report();
        if let HistoryKind::CallComplete { failure_status, .. } =
            &mut failed.observations.history[4].kind
        {
            *failure_status = Some(71);
        }
        failed.observations.validate(&failed.attempts).unwrap();
    }
}
