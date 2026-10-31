//! Observations only: no Source, provider, value or custody authority.
#![cfg(all(test, jett_resource_native_test_archive))]
use super::*;

pub(in crate::native_abi) const MAX_HISTORY: usize = 8192;
pub(in crate::native_abi) const MAX_CONTEXT: usize = 65_536;
pub(in crate::native_abi) const MAX_CONTEXT_BYTES: usize = 1_048_576;
pub(in crate::native_abi) const MAX_PARAMETERS: usize = 1024;
pub(in crate::native_abi) const COUNTER_OVERFLOW: u32 = 1;
pub(in crate::native_abi) const EVENT_LIMIT: u32 = 2;
pub(in crate::native_abi) const BYTE_LIMIT: u32 = 4;
pub(in crate::native_abi) const ALLOCATION: u32 = 8;
pub(in crate::native_abi) const SEQUENCE_OVERFLOW: u32 = 16;
pub(in crate::native_abi) const BOUNDARY: u32 = 32;
pub(in crate::native_abi) const KNOWN_FLAGS: u32 = 63;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(in crate::native_abi) struct Counters {
    pub values: [u64; 5],
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::native_abi) struct AttemptIdentity {
    pub id: u64,
    pub ordinal: u32,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::native_abi) enum Channel {
    Session,
    Attempt(AttemptIdentity),
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::native_abi) struct CallIdentity {
    pub call: u64,
    pub scope: u64,
    pub function: u32,
    pub signature: u32,
    pub template: u32,
    pub operation: u32,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::native_abi) enum ParameterValue {
    Int64 { bits: u64 },
    Unobserved,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::native_abi) struct Parameter {
    pub parameter: u32,
    pub shape: u32,
    pub value: ParameterValue,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::native_abi) struct Boundary {
    pub sequence: Option<u64>,
    pub counters: Counters,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::native_abi) struct AttemptObservation {
    pub identity: AttemptIdentity,
    pub begin: Boundary,
    pub end: Option<Boundary>,
}
#[derive(Debug, PartialEq, Eq)]
pub(in crate::native_abi) enum HistoryKind {
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
pub(in crate::native_abi) struct HistoryEvent {
    pub sequence: u64,
    pub channel: Channel,
    pub context: Option<Vec<u8>>,
    pub kind: HistoryKind,
}
#[derive(Debug, PartialEq, Eq)]
pub(in crate::native_abi) struct Snapshot {
    pub counters: Counters,
    pub flags: u32,
    pub attempts: Vec<AttemptObservation>,
    pub history: Vec<HistoryEvent>,
}
#[derive(Clone, Copy)]
struct EnteredCall {
    call: CallIdentity,
    channel: Channel,
    sequence: Option<u64>,
    complete: bool,
}

#[derive(Default)]
pub(in crate::native_abi) struct Recorder {
    counters: Counters,
    flags: u32,
    sequence: u64,
    context_bytes: usize,
    active: Option<AttemptIdentity>,
    attempts: Vec<AttemptObservation>,
    history: Vec<HistoryEvent>,
    calls: Vec<EnteredCall>,
}
impl Recorder {
    pub(in crate::native_abi) fn incomplete(&mut self, flag: u32) {
        self.flags |= flag;
    }
    fn increment(&mut self, index: usize) {
        if let Some(next) = self.counters.values[index].checked_add(1) {
            self.counters.values[index] = next;
        } else {
            self.incomplete(COUNTER_OVERFLOW);
        }
    }
    fn ingress(&mut self, ingress: AcquisitionIngress, acquire: bool) {
        match ingress {
            AcquisitionIngress::HookPrepare { .. } => {
                self.increment(0);
                if acquire {
                    self.increment(1);
                }
            }
            AcquisitionIngress::FactoryPrepare => self.increment(2),
            AcquisitionIngress::FactoryCommit => self.increment(3),
        }
    }
    pub(in crate::native_abi) fn provider_construct_dispatch(&mut self) {
        self.increment(4);
    }
    pub(in crate::native_abi) fn context_budget(&self, length: usize) -> Result<(), u32> {
        if self.history.len() >= MAX_HISTORY {
            return Err(EVENT_LIMIT);
        }
        if length > MAX_CONTEXT
            || self
                .context_bytes
                .checked_add(length)
                .is_none_or(|total| total > MAX_CONTEXT_BYTES)
        {
            return Err(BYTE_LIMIT);
        }
        if self.sequence.checked_add(1).is_none() {
            return Err(SEQUENCE_OVERFLOW);
        }
        Ok(())
    }
    fn append(
        &mut self,
        channel: Channel,
        context: Option<&[u8]>,
        kind: HistoryKind,
    ) -> Option<u64> {
        if self.history.len() >= MAX_HISTORY {
            self.incomplete(EVENT_LIMIT);
            return None;
        }
        let length = context.map_or(0, <[u8]>::len);
        let Some(total) = self.context_bytes.checked_add(length) else {
            self.incomplete(BYTE_LIMIT);
            return None;
        };
        if length > MAX_CONTEXT || total > MAX_CONTEXT_BYTES {
            self.incomplete(BYTE_LIMIT);
            return None;
        }
        let Some(sequence) = self.sequence.checked_add(1) else {
            self.incomplete(SEQUENCE_OVERFLOW);
            return None;
        };
        let copied = match copy_context(context) {
            Ok(copied) => copied,
            Err(flag) => {
                self.incomplete(flag);
                return None;
            }
        };
        if self.history.try_reserve(1).is_err() {
            self.incomplete(ALLOCATION);
            return None;
        }
        self.history.push(HistoryEvent {
            sequence,
            channel,
            context: copied,
            kind,
        });
        self.sequence = sequence;
        self.context_bytes = total;
        Some(sequence)
    }
    pub(in crate::native_abi) fn begin(&mut self, identity: AttemptIdentity) {
        if self.active.is_some()
            || identity.id == 0
            || identity.ordinal as usize != self.attempts.len()
            || self
                .attempts
                .last()
                .is_some_and(|previous| previous.identity.id >= identity.id)
        {
            self.incomplete(BOUNDARY);
        }
        self.active = Some(identity);
        if self.attempts.len() >= MAX_ATTEMPTS {
            self.incomplete(EVENT_LIMIT);
            return;
        }
        let sequence = self.append(Channel::Attempt(identity), None, HistoryKind::Begin);
        let observation = AttemptObservation {
            identity,
            begin: Boundary {
                sequence,
                counters: self.counters,
            },
            end: None,
        };
        if self.attempts.try_reserve(1).is_err() {
            self.incomplete(ALLOCATION);
            return;
        }
        self.attempts.push(observation);
    }
    pub(in crate::native_abi) fn end(&mut self, identity: AttemptIdentity, context: Option<&[u8]>) {
        if self.active != Some(identity) {
            self.incomplete(BOUNDARY);
        }
        if self
            .calls
            .iter()
            .any(|call| call.channel == Channel::Attempt(identity) && !call.complete)
        {
            self.incomplete(BOUNDARY);
        }
        let sequence = self.append(Channel::Attempt(identity), context, HistoryKind::End);
        let boundary = Boundary {
            sequence,
            counters: self.counters,
        };
        match self
            .attempts
            .iter_mut()
            .find(|attempt| attempt.identity == identity)
        {
            Some(attempt) if attempt.end.is_none() => attempt.end = Some(boundary),
            _ => self.flags |= BOUNDARY,
        }
        self.active = None;
    }
    pub(in crate::native_abi) fn property_change(
        &mut self,
        channel: Channel,
        context: Option<&[u8]>,
    ) {
        if let Channel::Attempt(identity) = channel {
            if self.active != Some(identity) {
                self.incomplete(BOUNDARY);
            }
        }
        let kind = if context.is_some() {
            HistoryKind::Set
        } else {
            HistoryKind::Clear
        };
        self.append(channel, context, kind);
    }
    pub(in crate::native_abi) fn call_enter(
        &mut self,
        channel: Channel,
        context: &[u8],
        call: CallIdentity,
        parameters: Vec<Parameter>,
    ) {
        if self
            .calls
            .iter()
            .any(|previous| previous.call.call == call.call || previous.call.scope == call.scope)
        {
            self.incomplete(BOUNDARY);
            return;
        }
        if !matches!(channel, Channel::Attempt(identity) if self.active == Some(identity))
            || call.call == 0
            || call.scope == 0
            || parameters.len() > MAX_PARAMETERS
        {
            self.incomplete(BOUNDARY);
            return;
        }
        if self.calls.len() >= MAX_HISTORY {
            self.incomplete(EVENT_LIMIT);
            return;
        }
        if self.calls.try_reserve(1).is_err() {
            self.incomplete(ALLOCATION);
            return;
        }
        let sequence = self.append(
            channel,
            Some(context),
            HistoryKind::CallEnter { call, parameters },
        );
        self.calls.push(EnteredCall {
            call,
            channel,
            sequence,
            complete: false,
        });
    }
    pub(in crate::native_abi) fn has_entered_call(&self, call: u64) -> bool {
        self.calls.iter().any(|entry| entry.call.call == call)
    }
    pub(in crate::native_abi) fn call_complete(
        &mut self,
        context: Option<&[u8]>,
        call: CallIdentity,
        body_status: u32,
        failure_status: Option<u32>,
    ) {
        let Some(index) = self
            .calls
            .iter()
            .position(|entry| entry.call.call == call.call)
        else {
            // Calls which never had a Property context are outside this observation claim.
            return;
        };
        let entry = self.calls[index];
        if entry.call != call || entry.complete {
            self.incomplete(BOUNDARY);
            return;
        }
        if self
            .calls
            .iter()
            .skip(index + 1)
            .any(|entry| !entry.complete)
        {
            self.incomplete(BOUNDARY);
        }
        self.append(
            entry.channel,
            context,
            HistoryKind::CallComplete {
                call,
                entered_sequence: entry.sequence,
                body_status,
                failure_status,
            },
        );
        self.calls[index].complete = true;
    }
    pub(in crate::native_abi) fn snapshot(&self) -> ResourceResult<Snapshot> {
        let mut attempts = Vec::new();
        attempts
            .try_reserve_exact(self.attempts.len())
            .map_err(|_| NativeResourceError::Capacity)?;
        attempts.extend_from_slice(&self.attempts);
        let mut history = Vec::new();
        history
            .try_reserve_exact(self.history.len())
            .map_err(|_| NativeResourceError::Capacity)?;
        for event in &self.history {
            history.push(copy_event(event).map_err(|_| NativeResourceError::Capacity)?);
        }
        Ok(Snapshot {
            counters: self.counters,
            flags: self.flags,
            attempts,
            history,
        })
    }
}
pub(super) fn copy_context(context: Option<&[u8]>) -> Result<Option<Vec<u8>>, u32> {
    let Some(context) = context else {
        return Ok(None);
    };
    if context.len() > MAX_CONTEXT {
        return Err(BYTE_LIMIT);
    }
    let mut copy = Vec::new();
    copy.try_reserve_exact(context.len())
        .map_err(|_| ALLOCATION)?;
    copy.extend_from_slice(context);
    Ok(Some(copy))
}
fn copy_event(event: &HistoryEvent) -> Result<HistoryEvent, u32> {
    let kind = match &event.kind {
        HistoryKind::Begin => HistoryKind::Begin,
        HistoryKind::End => HistoryKind::End,
        HistoryKind::Set => HistoryKind::Set,
        HistoryKind::Clear => HistoryKind::Clear,
        HistoryKind::CallEnter { call, parameters } => {
            let mut copied = Vec::new();
            copied
                .try_reserve_exact(parameters.len())
                .map_err(|_| ALLOCATION)?;
            copied.extend_from_slice(parameters);
            HistoryKind::CallEnter {
                call: *call,
                parameters: copied,
            }
        }
        HistoryKind::CallComplete {
            call,
            entered_sequence,
            body_status,
            failure_status,
        } => HistoryKind::CallComplete {
            call: *call,
            entered_sequence: *entered_sequence,
            body_status: *body_status,
            failure_status: *failure_status,
        },
    };
    Ok(HistoryEvent {
        sequence: event.sequence,
        channel: event.channel,
        context: copy_context(event.context.as_deref())?,
        kind,
    })
}

pub(super) enum AcquisitionIngress {
    HookPrepare { operation: u32 },
    FactoryPrepare,
    FactoryCommit,
}
fn acquire_operation(resource: &NativeResourceState, operation: u32) -> bool {
    match resource
        .layout
        .operations()
        .get(operation as usize)
        .map(|row| row.operation())
    {
        Some(NativeOperation::Acquire { .. }) => true,
        Some(NativeOperation::InvokeDescriptor { target, .. }) => matches!(
            resource
                .layout
                .operations()
                .get(*target as usize)
                .map(|row| row.operation()),
            Some(NativeOperation::Acquire { .. })
        ),
        _ => false,
    }
}
pub(super) fn observe_ingress(context: *const JettRuntimeContextV1, ingress: AcquisitionIngress) {
    let Ok(authenticated) = AuthenticatedResourceContext::acquire(context) else {
        return;
    };
    let _ = authenticated.with_state(|_, state| {
        let acquire = match &ingress {
            AcquisitionIngress::HookPrepare { operation } => state
                .resource_state
                .as_ref()
                .is_some_and(|resource| acquire_operation(resource, *operation)),
            _ => false,
        };
        state.values.test_observations.ingress(ingress, acquire);
        Ok(())
    });
}
pub(in crate::native_abi) fn snapshot(
    context: *const JettRuntimeContextV1,
) -> ResourceResult<Snapshot> {
    AuthenticatedResourceContext::acquire(context)?
        .with_state(|_, state| state.values.test_observations.snapshot())
}

/// Private executable-mode checks of journal transport semantics, not Source execution.
pub(super) fn recorder_controls() -> Result<(), ()> {
    let first = AttemptIdentity { id: 11, ordinal: 0 };
    let second = AttemptIdentity { id: 19, ordinal: 1 };
    let call = CallIdentity {
        call: 31,
        scope: 32,
        function: 7,
        signature: 8,
        template: 9,
        operation: 10,
    };
    let mut recorder = Recorder::default();
    recorder.begin(first);
    recorder.property_change(Channel::Attempt(first), Some(&[0xff, 0, 1]));
    recorder.call_enter(
        Channel::Attempt(first),
        &[0xff, 0, 1],
        call,
        vec![Parameter {
            parameter: 0,
            shape: 3,
            value: ParameterValue::Int64 { bits: u64::MAX },
        }],
    );
    recorder.property_change(Channel::Attempt(first), Some(b"changed"));
    recorder.call_complete(Some(b"changed"), call, 0, None);
    recorder.property_change(Channel::Attempt(first), None);
    recorder.end(first, None);
    recorder.property_change(Channel::Session, Some(b"legacy"));
    recorder.begin(second);
    recorder.property_change(Channel::Attempt(second), Some(b""));
    recorder.property_change(Channel::Attempt(second), None);
    recorder.end(second, None);
    let snapshot = recorder.snapshot().map_err(|_| ())?;
    if snapshot.flags != 0
        || snapshot.counters != Counters::default()
        || snapshot.attempts.len() != 2
        || snapshot.history.len() != 12
        || snapshot.history[1].context.as_deref() != Some(&[0xff, 0, 1][..])
        || snapshot.history[2].context.as_deref() != Some(&[0xff, 0, 1][..])
        || snapshot.history[7].channel != Channel::Session
        || snapshot.history[9].context.as_deref() != Some(&b""[..])
        || !matches!(
            snapshot.history[4].kind,
            HistoryKind::CallComplete {
                entered_sequence: Some(3),
                body_status: 0,
                failure_status: None,
                ..
            }
        )
        || snapshot.history[4].context.as_deref() != Some(&b"changed"[..])
    {
        return Err(());
    }
    // Repeated prologue validation is not another activation or trial.
    recorder.call_enter(Channel::Attempt(second), b"repeat", call, Vec::new());
    if recorder.flags & BOUNDARY == 0 || recorder.history.len() != 12 {
        return Err(());
    }
    // A saturating stored prefix plus an explicit flag is never a claimed exact zero.
    let mut overflow = Recorder::default();
    overflow.counters.values[0] = u64::MAX;
    overflow.ingress(AcquisitionIngress::HookPrepare { operation: 0 }, false);
    if overflow.counters.values[0] != u64::MAX || overflow.flags != COUNTER_OVERFLOW {
        return Err(());
    }
    let mut bytes = Recorder::default();
    bytes.property_change(Channel::Session, Some(&vec![0; MAX_CONTEXT + 1]));
    if bytes.flags != BYTE_LIMIT || !bytes.history.is_empty() {
        return Err(());
    }
    let mut bounded = Recorder::default();
    for _ in 0..MAX_HISTORY {
        bounded.property_change(Channel::Session, None);
    }
    bounded.property_change(Channel::Session, None);
    if bounded.flags != EVENT_LIMIT
        || bounded.history.len() != MAX_HISTORY
        || bounded.sequence != MAX_HISTORY as u64
    {
        return Err(());
    }
    let mut cumulative = Recorder::default();
    let context = vec![b'x'; MAX_CONTEXT];
    for _ in 0..MAX_CONTEXT_BYTES / MAX_CONTEXT {
        cumulative.property_change(Channel::Session, Some(&context));
    }
    cumulative.property_change(Channel::Session, Some(&context));
    if cumulative.flags != BYTE_LIMIT
        || cumulative.context_bytes != MAX_CONTEXT_BYTES
        || cumulative.history.len() != MAX_CONTEXT_BYTES / MAX_CONTEXT
    {
        return Err(());
    }
    let mut sequence = Recorder::default();
    sequence.sequence = u64::MAX;
    sequence.property_change(Channel::Session, None);
    if sequence.flags != SEQUENCE_OVERFLOW || !sequence.history.is_empty() {
        return Err(());
    }
    Ok(())
}
