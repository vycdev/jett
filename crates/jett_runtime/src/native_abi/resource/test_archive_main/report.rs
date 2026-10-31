//! Canonical owned report transport for the private compiler-test main.
//! These records contain observations, never registry keys or custody tokens.
use super::super::test_observations::{
    AttemptIdentity, Boundary, CallIdentity, Channel, Counters as ObservationCounters, HistoryKind,
    KNOWN_FLAGS, MAX_CONTEXT, MAX_CONTEXT_BYTES, MAX_HISTORY, MAX_PARAMETERS, ParameterValue,
    Snapshot,
};
use super::{Completion, Counts, Event};

pub(super) const MAX_ATTEMPTS: usize = 64;
pub(super) const MAX_EVENTS: usize = 8192;
pub(super) const MAX_MESSAGE: usize = 65_536;
const MAX_REPORT: usize = 16 * 1024 * 1024;

pub(super) struct Attempt {
    pub(super) id: u64,
    pub(super) observed_body_status: Option<u32>,
    pub(super) completion: Option<Completion>,
    pub(super) counts: Option<Counts>,
    pub(super) ordinary_status: Option<u32>,
    pub(super) ordinary_message: Option<Vec<u8>>,
    pub(super) resource_message: Option<Vec<u8>>,
    pub(super) event_sequence_end: Option<u64>,
}
impl Attempt {
    pub(super) fn pending(id: u64) -> Self {
        Self {
            id,
            observed_body_status: None,
            completion: None,
            counts: None,
            ordinary_status: None,
            ordinary_message: None,
            resource_message: None,
            event_sequence_end: None,
        }
    }
    pub(super) fn observations_complete(&self) -> bool {
        self.observed_body_status.is_some()
            && self.completion.is_some()
            && self.counts.is_some()
            && self.ordinary_status.is_some()
            && self.ordinary_message.is_some()
            && self.resource_message.is_some()
            && self.event_sequence_end.is_some()
    }
}

#[derive(Default)]
pub(super) struct Report {
    pub(super) observations: Option<Snapshot>,
    pub(super) attempts: Vec<Attempt>,
    pub(super) events: Option<Vec<Event>>,
    pub(super) remaining: Option<u64>,
    pub(super) final_counts: Option<Counts>,
    pub(super) final_ordinary_status: Option<u32>,
    pub(super) final_ordinary_message: Option<Vec<u8>>,
    pub(super) refused: bool,
    pub(super) created: bool,
    pub(super) destroy_attempts: u32,
    pub(super) destroy_status: u32,
    pub(super) write_status: u32,
}

struct Encoder(Vec<u8>);
impl Encoder {
    fn bytes(&mut self, value: &[u8]) -> Result<(), ()> {
        let length = self.0.len().checked_add(value.len()).ok_or(())?;
        if length > MAX_REPORT {
            return Err(());
        }
        self.0.try_reserve(value.len()).map_err(|_| ())?;
        self.0.extend_from_slice(value);
        Ok(())
    }
    fn u32(&mut self, value: u32) -> Result<(), ()> {
        self.bytes(&value.to_le_bytes())
    }
    fn u64(&mut self, value: u64) -> Result<(), ()> {
        self.bytes(&value.to_le_bytes())
    }
    fn completion(&mut self, value: &Completion) -> Result<(), ()> {
        if value.attempt == 0 || value.reserved != 0 || value.selected_kind > 3 {
            return Err(());
        }
        self.u64(value.attempt)?;
        for field in [
            value.body_status,
            value.cleanup_status,
            value.selected_kind,
            0,
        ] {
            self.u32(field)?;
        }
        Ok(())
    }
    fn counts(&mut self, value: &Counts) -> Result<(), ()> {
        if value.reserved != 0 || value.ordinary_empty > 1 {
            return Err(());
        }
        for field in [
            value.owners,
            value.loans,
            value.frames,
            value.registry_entries,
            value.owner_handles,
            value.loan_handles,
            value.frame_handles,
            value.provisional_returns,
        ] {
            self.u64(field)?;
        }
        self.u32(value.ordinary_empty)?;
        self.u32(0)
    }
    fn attempt(&mut self, value: &Attempt) -> Result<(), ()> {
        if !value.observations_complete() {
            return Err(());
        }
        let completion = value.completion.as_ref().ok_or(())?;
        let counts = value.counts.as_ref().ok_or(())?;
        let ordinary = value.ordinary_message.as_ref().ok_or(())?;
        let resource = value.resource_message.as_ref().ok_or(())?;
        let end = value.event_sequence_end.ok_or(())?;
        if value.id != completion.attempt
            || value.observed_body_status != Some(completion.body_status)
            || ordinary.len() > MAX_MESSAGE
            || resource.len() > MAX_MESSAGE
        {
            return Err(());
        }
        self.completion(completion)?;
        self.counts(counts)?;
        self.u32(ordinary.len() as u32)?;
        self.u32(resource.len() as u32)?;
        self.u64(end)?;
        self.bytes(ordinary)?;
        self.bytes(resource)
    }
}

fn counters_follow(now: ObservationCounters, previous: ObservationCounters) -> bool {
    now.values[1] <= now.values[0]
        && now
            .values
            .iter()
            .zip(previous.values)
            .all(|(now, before)| *now >= before)
}

fn validate_observations(snapshot: &Snapshot, attempts: &[Attempt]) -> Result<(), ()> {
    let bad = || ();
    let zero = ObservationCounters { values: [0; 5] };
    let identity_known = |identity: AttemptIdentity| {
        attempts
            .get(identity.ordinal as usize)
            .is_some_and(|attempt| attempt.id == identity.id)
    };
    let mut previous = zero;
    let mut previous_ordinal = None;
    let mut boundary_sequence = 0;
    for attempt in &snapshot.attempts {
        if !identity_known(attempt.identity)
            || previous_ordinal.is_some_and(|ordinal| attempt.identity.ordinal <= ordinal)
            || !counters_follow(attempt.begin.counters, previous)
        {
            return Err(bad());
        }
        previous_ordinal = Some(attempt.identity.ordinal);
        previous = attempt.begin.counters;
        for (boundary, beginning) in [(Some(attempt.begin), true), (attempt.end, false)] {
            let Some(boundary) = boundary else { continue };
            if !counters_follow(boundary.counters, previous) {
                return Err(bad());
            }
            previous = boundary.counters;
            if let Some(sequence) = boundary.sequence {
                if sequence <= boundary_sequence {
                    return Err(bad());
                }
                let event = snapshot
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
    if !counters_follow(snapshot.counters, previous) {
        return Err(bad());
    }
    let mut active = None;
    let mut current_context: Option<&[u8]> = None;
    let mut call_stack = Vec::new();
    call_stack
        .try_reserve_exact(snapshot.history.len())
        .map_err(|_| ())?;
    for (index, event) in snapshot.history.iter().enumerate() {
        if event.sequence != u64::try_from(index).map_err(|_| bad())? + 1 {
            return Err(bad());
        }
        if let Channel::Attempt(identity) = event.channel {
            if !identity_known(identity) {
                return Err(bad());
            }
            if snapshot.flags == 0 && active.is_some_and(|current| current != identity) {
                return Err(bad());
            }
        }
        match &event.kind {
            HistoryKind::Begin => {
                let Channel::Attempt(identity) = event.channel else {
                    return Err(bad());
                };
                if (snapshot.flags == 0 && active.is_some()) || event.context.is_some() {
                    return Err(bad());
                }
                active = Some(identity);
                current_context = None;
                if snapshot.flags == 0
                    && !snapshot.attempts.iter().any(|attempt| {
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
                if snapshot.flags == 0 && active != Some(identity) {
                    return Err(bad());
                }
                if snapshot.flags == 0
                    && !snapshot.attempts.iter().any(|attempt| {
                        attempt.identity == identity
                            && attempt
                                .end
                                .is_some_and(|end| end.sequence == Some(event.sequence))
                    })
                {
                    return Err(bad());
                }
                if snapshot.flags == 0 && !call_stack.is_empty() {
                    return Err(bad());
                }
                if snapshot.flags == 0 && event.context.as_deref() != current_context {
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
                if snapshot.flags == 0 {
                    if event.context.as_deref() != current_context {
                        return Err(bad());
                    }
                    call_stack.push(event.sequence);
                }
                if snapshot.history[..index].iter().any(|prior| matches!(&prior.kind, HistoryKind::CallEnter { call: old, .. } if old.call == call.call || old.scope == call.scope)) { return Err(bad()); }
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
                if *failure_status == Some(0) || (failure_status.is_none() && *body_status != 0) {
                    return Err(bad());
                }
                if snapshot.flags == 0 && event.context.as_deref() != current_context {
                    return Err(bad());
                }
                if snapshot.flags == 0 && call_stack.pop() != *entered_sequence {
                    return Err(bad());
                }
                if let Some(sequence) = entered_sequence {
                    if *sequence == 0 || *sequence >= event.sequence {
                        return Err(bad());
                    }
                    let entered = snapshot
                        .history
                        .get(usize::try_from(sequence - 1).map_err(|_| bad())?)
                        .ok_or_else(bad)?;
                    if entered.channel != event.channel
                        || !matches!(&entered.kind, HistoryKind::CallEnter { call: original, .. } if original == call)
                    {
                        return Err(bad());
                    }
                    if snapshot.history[..index].iter().any(|prior| matches!(&prior.kind, HistoryKind::CallComplete { entered_sequence: Some(old), .. } if old == sequence)) { return Err(bad()); }
                } else if snapshot.flags == 0 {
                    return Err(bad());
                }
            }
            HistoryKind::Set | HistoryKind::Clear => {
                if snapshot.flags == 0 && event.channel == Channel::Session && active.is_some() {
                    return Err(bad());
                }
                if matches!(event.channel, Channel::Attempt(_)) {
                    current_context = event.context.as_deref();
                }
            }
        }
        if snapshot.flags == 0
            && matches!(event.channel, Channel::Attempt(_))
            && active.is_none()
            && !matches!(event.kind, HistoryKind::End)
        {
            return Err(bad());
        }
    }

    if snapshot.flags == 0 && snapshot.history.iter().any(|event| matches!(event.kind, HistoryKind::CallEnter { .. })
            && !snapshot.history.iter().any(|completed| matches!(&completed.kind, HistoryKind::CallComplete { entered_sequence: Some(sequence), .. } if *sequence == event.sequence))) {
            return Err(bad());
        }
    if snapshot.flags == 0
        && (active.is_some()
            || !(snapshot.attempts.len() == attempts.len()
                && snapshot.attempts.iter().all(|attempt| {
                    attempt.begin.sequence.is_some()
                        && attempt.end.is_some_and(|end| end.sequence.is_some())
                })
                && snapshot.history.iter().all(|event| {
                    !matches!(
                        event.kind,
                        HistoryKind::CallComplete {
                            entered_sequence: None,
                            ..
                        }
                    )
                })))
    {
        return Err(bad());
    }
    Ok(())
}

impl Encoder {
    fn optional_u64(&mut self, value: Option<u64>) -> Result<(), ()> {
        self.u32(u32::from(value.is_some()))?;
        if let Some(value) = value {
            self.u64(value)?;
        }
        Ok(())
    }
    fn optional_u32(&mut self, value: Option<u32>) -> Result<(), ()> {
        self.u32(u32::from(value.is_some()))?;
        if let Some(value) = value {
            self.u32(value)?;
        }
        Ok(())
    }
    fn observation_counters(&mut self, counters: ObservationCounters) -> Result<(), ()> {
        for value in counters.values {
            self.u64(value)?;
        }
        Ok(())
    }
    fn observation_boundary(&mut self, boundary: Boundary) -> Result<(), ()> {
        self.optional_u64(boundary.sequence)?;
        self.observation_counters(boundary.counters)
    }
    fn observation_identity(&mut self, identity: AttemptIdentity) -> Result<(), ()> {
        self.u64(identity.id)?;
        self.u32(identity.ordinal)
    }
    fn call_identity(&mut self, call: CallIdentity) -> Result<(), ()> {
        self.u64(call.call)?;
        self.u64(call.scope)?;
        for value in [call.function, call.signature, call.template, call.operation] {
            self.u32(value)?;
        }
        Ok(())
    }
    fn observations(&mut self, snapshot: &Snapshot, attempts: &[Attempt]) -> Result<(), ()> {
        if snapshot.flags & !KNOWN_FLAGS != 0 || snapshot.attempts.len() > MAX_ATTEMPTS || snapshot.history.len() > MAX_HISTORY
            || snapshot.history.iter().any(|event| matches!(&event.kind, HistoryKind::CallEnter { parameters, .. } if parameters.len() > MAX_PARAMETERS)) { return Err(()); }
        validate_observations(snapshot, attempts)?;
        let mut context_bytes = 0u64;
        for event in &snapshot.history {
            let length = event.context.as_ref().map_or(0, Vec::len);
            if length > MAX_CONTEXT {
                return Err(());
            }
            context_bytes = context_bytes
                .checked_add(u64::try_from(length).map_err(|_| ())?)
                .ok_or(())?;
        }
        if context_bytes > MAX_CONTEXT_BYTES as u64 {
            return Err(());
        }
        self.observation_counters(snapshot.counters)?;
        self.u32(snapshot.flags)?;
        self.u32(u32::try_from(snapshot.attempts.len()).map_err(|_| ())?)?;
        self.u32(u32::try_from(snapshot.history.len()).map_err(|_| ())?)?;
        self.u64(context_bytes)?;
        self.u64(u64::try_from(snapshot.history.len()).map_err(|_| ())?)?;
        for attempt in &snapshot.attempts {
            self.observation_identity(attempt.identity)?;
            self.observation_boundary(attempt.begin)?;
            self.u32(u32::from(attempt.end.is_some()))?;
            if let Some(end) = attempt.end {
                self.observation_boundary(end)?;
            }
        }
        for event in &snapshot.history {
            self.u64(event.sequence)?;
            self.u32(match event.kind {
                HistoryKind::Begin => 1,
                HistoryKind::Set => 2,
                HistoryKind::CallEnter { .. } => 3,
                HistoryKind::CallComplete { .. } => 4,
                HistoryKind::Clear => 5,
                HistoryKind::End => 6,
            })?;
            match event.channel {
                Channel::Session => self.u32(0)?,
                Channel::Attempt(identity) => {
                    self.u32(1)?;
                    self.observation_identity(identity)?;
                }
            }
            self.u32(u32::from(event.context.is_some()))?;
            if let Some(context) = &event.context {
                self.u32(u32::try_from(context.len()).map_err(|_| ())?)?;
                self.bytes(context)?;
            }
            match &event.kind {
                HistoryKind::CallEnter { call, parameters } => {
                    if parameters.len() > MAX_PARAMETERS {
                        return Err(());
                    }
                    self.call_identity(*call)?;
                    self.u32(u32::try_from(parameters.len()).map_err(|_| ())?)?;
                    for parameter in parameters {
                        self.u32(parameter.parameter)?;
                        self.u32(parameter.shape)?;
                        match parameter.value {
                            ParameterValue::Unobserved => self.u32(0)?,
                            ParameterValue::Int64 { bits } => {
                                self.u32(1)?;
                                self.u64(bits)?;
                            }
                        }
                    }
                }
                HistoryKind::CallComplete {
                    call,
                    entered_sequence,
                    body_status,
                    failure_status,
                } => {
                    self.call_identity(*call)?;
                    self.optional_u64(*entered_sequence)?;
                    self.u32(*body_status)?;
                    self.optional_u32(*failure_status)?;
                }
                HistoryKind::Begin | HistoryKind::End | HistoryKind::Set | HistoryKind::Clear => {}
            }
        }
        Ok(())
    }
}

impl Report {
    pub(super) fn encode(&self) -> Result<Vec<u8>, ()> {
        if self.refused
            || self.attempts.len() > MAX_ATTEMPTS
            || self.events.as_ref().ok_or(())?.len() > MAX_EVENTS
            || self.remaining.is_none()
            || self.final_counts.is_none()
            || self.final_ordinary_status.is_none()
            || self.final_ordinary_message.is_none()
            || self.destroy_attempts > 1
            || (self.created && self.destroy_attempts != 1)
            || self
                .final_counts
                .as_ref()
                .is_some_and(|counts| counts.reserved != 0 || counts.ordinary_empty > 1)
            || (!self.created && self.destroy_attempts != 0)
        {
            return Err(());
        }
        let events = self.events.as_ref().ok_or(())?;
        let mut output = Encoder(Vec::new());
        output.bytes(b"JTRRP002")?;
        output.u32(2)?;
        output.u32(0)?;
        output.u64(0)?;
        output.u32(self.attempts.len() as u32)?;
        output.u32(events.len() as u32)?;
        output.u64(self.remaining.ok_or(())?)?;
        let mut last_attempt = 0;
        let mut last_end = 0;
        for attempt in &self.attempts {
            let completion = attempt.completion.as_ref().ok_or(())?;
            let end = attempt.event_sequence_end.ok_or(())?;
            if completion.attempt <= last_attempt || end < last_end {
                return Err(());
            }
            output.attempt(attempt)?;
            last_attempt = completion.attempt;
            last_end = end;
        }
        let mut last_sequence = 0;
        for event in events {
            if event.sequence <= last_sequence
                || event.reserved != 0
                || !(1..=5).contains(&event.kind)
            {
                return Err(());
            }
            output.u64(event.sequence)?;
            output.u64(event.label as u64)?;
            output.u32(event.kind)?;
            output.u32(0)?;
            last_sequence = event.sequence;
        }
        if last_end > last_sequence {
            return Err(());
        }
        output.observations(self.observations.as_ref().ok_or(())?, &self.attempts)?;
        for field in [
            self.destroy_status,
            u32::from(self.created),
            self.destroy_attempts,
            self.write_status,
        ] {
            output.u32(field)?;
        }
        let length = output.0.len() as u64;
        output.0[16..24].copy_from_slice(&length.to_le_bytes());
        Ok(output.0)
    }
}

// Runs through the actual private writer without acquiring a Runtime context or Source authority.
pub(super) fn writer_controls() -> Result<(), ()> {
    let baseline = writer_control_report();
    let bytes = baseline.encode()?;
    if bytes.get(..8) != Some(&b"JTRRP002"[..])
        || bytes.get(8..12) != Some(&2u32.to_le_bytes()[..])
        || bytes.get(16..24) != Some(&u64::try_from(bytes.len()).map_err(|_| ())?.to_le_bytes()[..])
    {
        return Err(());
    }
    for mutation in 0..25 {
        let mut changed = writer_control_report();
        changed.encode()?;
        match mutation {
            0 => changed.observations = None,
            1 => changed.attempts[0].completion.as_mut().ok_or(())?.reserved = 1,
            2 => changed.attempts[0].counts.as_mut().ok_or(())?.reserved = 1,
            3 => changed.attempts[0].observed_body_status = None,
            4 => changed.refused = true,
            5 => changed.final_counts.as_mut().ok_or(())?.ordinary_empty = 2,
            6 => changed.attempts[0].completion.as_mut().ok_or(())?.attempt = 42,
            7..=24 => {
                let snapshot = changed.observations.as_mut().ok_or(())?;
                match mutation {
                    7 => snapshot.flags = KNOWN_FLAGS + 1,
                    8 => snapshot.counters.values[1] = 4,
                    9 => snapshot.counters.values[0] = 2,
                    10 => snapshot.attempts[0].identity.id = 42,
                    11 => snapshot.attempts[0].identity.ordinal = 1,
                    12 => snapshot.history[2].sequence = 2,
                    13 => {
                        if let HistoryKind::CallComplete { failure_status, .. } =
                            &mut snapshot.history[4].kind
                        {
                            *failure_status = Some(0);
                        } else {
                            return Err(());
                        }
                    }
                    14 => {
                        if let HistoryKind::CallComplete { body_status, .. } =
                            &mut snapshot.history[4].kind
                        {
                            *body_status = 71;
                        } else {
                            return Err(());
                        }
                    }
                    15 => {
                        if let HistoryKind::CallComplete {
                            entered_sequence, ..
                        } = &mut snapshot.history[4].kind
                        {
                            *entered_sequence = None;
                        } else {
                            return Err(());
                        }
                    }
                    16 => {
                        if let HistoryKind::CallComplete { call, .. } =
                            &mut snapshot.history[4].kind
                        {
                            call.scope += 1;
                        } else {
                            return Err(());
                        }
                    }
                    17 => snapshot.history[4].context = Some(b"wrong".to_vec()),
                    18 => snapshot.history[7].context = Some(Vec::new()),
                    19 => {
                        snapshot.history[1].kind = HistoryKind::Clear;
                        snapshot.history[1].context = None;
                    }
                    20 => snapshot.history[1].channel = Channel::Session,
                    21 => {
                        let child =
                            std::mem::replace(&mut snapshot.history[4].kind, HistoryKind::Clear);
                        let parent = std::mem::replace(&mut snapshot.history[5].kind, child);
                        snapshot.history[4].kind = parent;
                    }
                    22 => snapshot.history[5].kind = HistoryKind::Set,
                    23 => {
                        let HistoryKind::CallEnter { call: parent, .. } = &snapshot.history[2].kind
                        else {
                            return Err(());
                        };
                        let parent = *parent;
                        let HistoryKind::CallEnter { call, .. } = &mut snapshot.history[3].kind
                        else {
                            return Err(());
                        };
                        *call = parent;
                    }
                    24 => {
                        let HistoryKind::CallEnter { parameters, .. } =
                            &mut snapshot.history[2].kind
                        else {
                            return Err(());
                        };
                        let parameter = *parameters.first().ok_or(())?;
                        parameters.push(parameter);
                    }
                    _ => return Err(()),
                }
            }
            _ => return Err(()),
        }
        if changed.encode().is_ok() {
            return Err(());
        }
    }
    // Completion records current bytes, so a real intervening Clear must remain valid.
    let mut cleared = writer_control_report();
    let snapshot = cleared.observations.as_mut().ok_or(())?;
    let clear = snapshot.history.remove(6);
    snapshot.history.insert(4, clear);
    for (index, event) in snapshot.history.iter_mut().enumerate() {
        event.sequence = u64::try_from(index).map_err(|_| ())? + 1;
    }
    snapshot.history[5].context = None;
    snapshot.history[6].context = None;
    cleared.encode()?;
    Ok(())
}

fn writer_control_report() -> Report {
    use super::super::test_observations::{AttemptObservation, HistoryEvent, Parameter};
    let counts = || Counts {
        owners: 0,
        loans: 0,
        frames: 0,
        registry_entries: 0,
        owner_handles: 0,
        loan_handles: 0,
        frame_handles: 0,
        provisional_returns: 0,
        ordinary_empty: 1,
        reserved: 0,
    };
    let identity = AttemptIdentity { id: 41, ordinal: 0 };
    let zero = ObservationCounters { values: [0; 5] };
    let counters = ObservationCounters {
        values: [3, 1, 1, 1, 1],
    };
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
    let history = [
        (None, HistoryKind::Begin),
        (Some(b"trial".to_vec()), HistoryKind::Set),
        (
            Some(b"trial".to_vec()),
            HistoryKind::CallEnter {
                call: parent,
                parameters: vec![Parameter {
                    parameter: 0,
                    shape: 1,
                    value: ParameterValue::Int64 { bits: 42 },
                }],
            },
        ),
        (
            Some(b"trial".to_vec()),
            HistoryKind::CallEnter {
                call: child,
                parameters: vec![Parameter {
                    parameter: 0,
                    shape: 2,
                    value: ParameterValue::Unobserved,
                }],
            },
        ),
        (
            Some(b"trial".to_vec()),
            HistoryKind::CallComplete {
                call: child,
                entered_sequence: Some(4),
                body_status: 0,
                failure_status: None,
            },
        ),
        (
            Some(b"trial".to_vec()),
            HistoryKind::CallComplete {
                call: parent,
                entered_sequence: Some(3),
                body_status: 0,
                failure_status: None,
            },
        ),
        (None, HistoryKind::Clear),
        (None, HistoryKind::End),
    ]
    .into_iter()
    .zip(1u64..)
    .map(|((context, kind), sequence)| HistoryEvent {
        sequence,
        channel: Channel::Attempt(identity),
        context,
        kind,
    })
    .collect();
    let observations = Snapshot {
        counters,
        flags: 0,
        attempts: vec![AttemptObservation {
            identity,
            begin: Boundary {
                sequence: Some(1),
                counters: zero,
            },
            end: Some(Boundary {
                sequence: Some(8),
                counters,
            }),
        }],
        history,
    };
    let mut attempt = Attempt::pending(41);
    attempt.observed_body_status = Some(0);
    attempt.completion = Some(Completion {
        attempt: 41,
        body_status: 0,
        cleanup_status: 0,
        selected_kind: 0,
        reserved: 0,
    });
    attempt.counts = Some(counts());
    attempt.ordinary_status = Some(0);
    attempt.ordinary_message = Some(Vec::new());
    attempt.resource_message = Some(Vec::new());
    attempt.event_sequence_end = Some(0);
    Report {
        observations: Some(observations),
        attempts: vec![attempt],
        events: Some(Vec::new()),
        remaining: Some(2),
        final_counts: Some(counts()),
        final_ordinary_status: Some(0),
        final_ordinary_message: Some(Vec::new()),
        refused: false,
        created: true,
        destroy_attempts: 1,
        destroy_status: 0,
        write_status: 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn counts() -> Counts {
        Counts {
            owners: 0,
            loans: 0,
            frames: 0,
            registry_entries: 0,
            owner_handles: 0,
            loan_handles: 0,
            frame_handles: 0,
            provisional_returns: 0,
            ordinary_empty: 1,
            reserved: 0,
        }
    }

    fn observations() -> Snapshot {
        use super::super::super::test_observations::{AttemptObservation, HistoryEvent, Parameter};
        let identity = AttemptIdentity { id: 41, ordinal: 0 };
        let counters = ObservationCounters { values: [0; 5] };
        let call = CallIdentity {
            call: 11,
            scope: 12,
            function: 7,
            signature: 8,
            template: 9,
            operation: 10,
        };
        Snapshot {
            counters,
            flags: 0,
            attempts: vec![AttemptObservation {
                identity,
                begin: Boundary {
                    sequence: Some(1),
                    counters,
                },
                end: Some(Boundary {
                    sequence: Some(6),
                    counters,
                }),
            }],
            history: [
                (None, HistoryKind::Begin),
                (Some(Vec::new()), HistoryKind::Set),
                (
                    Some(Vec::new()),
                    HistoryKind::CallEnter {
                        call,
                        parameters: vec![Parameter {
                            parameter: 0,
                            shape: 1,
                            value: ParameterValue::Int64 { bits: 42 },
                        }],
                    },
                ),
                (
                    Some(Vec::new()),
                    HistoryKind::CallComplete {
                        call,
                        entered_sequence: Some(3),
                        body_status: 0,
                        failure_status: None,
                    },
                ),
                (None, HistoryKind::Clear),
                (None, HistoryKind::End),
            ]
            .into_iter()
            .enumerate()
            .map(|(index, (context, kind))| HistoryEvent {
                sequence: index as u64 + 1,
                channel: Channel::Attempt(identity),
                context,
                kind,
            })
            .collect(),
        }
    }

    fn report() -> Report {
        let mut attempt = Attempt::pending(41);
        attempt.observed_body_status = Some(71);
        attempt.completion = Some(Completion {
            attempt: 41,
            body_status: 71,
            cleanup_status: 0,
            selected_kind: 1,
            reserved: 0,
        });
        attempt.counts = Some(counts());
        attempt.ordinary_status = Some(0);
        attempt.ordinary_message = Some(Vec::new());
        attempt.resource_message = Some(Vec::new());
        attempt.event_sequence_end = Some(0);
        let baseline = Report {
            observations: Some(observations()),
            attempts: vec![attempt],
            events: Some(Vec::new()),
            remaining: Some(0),
            final_counts: Some(counts()),
            final_ordinary_status: Some(0),
            final_ordinary_message: Some(Vec::new()),
            refused: false,
            created: true,
            destroy_attempts: 1,
            destroy_status: 0,
            write_status: 0,
        };
        baseline
            .encode()
            .expect("valid uncorrupted v2 baseline before any mutation");
        baseline
    }
    #[test]
    fn private_report_v2_keeps_original_status_and_existing_binary_offsets() {
        let bytes = report().encode().unwrap();
        assert_eq!(bytes.len(), 668);
        assert_eq!(&bytes[..8], b"JTRRP002");
        assert_eq!(&bytes[8..12], &2u32.to_le_bytes());
        assert_eq!(&bytes[40..48], &41u64.to_le_bytes());
        assert_eq!(&bytes[48..52], &71u32.to_le_bytes());
        assert_eq!(&bytes[56..60], &1u32.to_le_bytes());
    }
    #[test]
    fn private_report_completed_prefix_survives_a_late_unknown_copy_without_encoding_success() {
        let mut value = report();
        value.attempts[0].resource_message = None;
        assert!(value.encode().is_err());
        let completed = value.attempts[0].completion.as_ref().unwrap();
        assert_eq!((completed.attempt, completed.body_status), (41, 71));
        assert!(value.attempts[0].counts.as_ref().unwrap().clean());
        assert_eq!(value.attempts[0].ordinary_message.as_deref(), Some(&[][..]));
    }
    #[test]
    fn private_report_unknown_observation_differs_from_a_known_empty_carrier() {
        for field in 0..6 {
            let mut value = report();
            match field {
                0 => value.events = None,
                1 => value.remaining = None,
                2 => value.attempts[0].counts = None,
                3 => value.attempts[0].ordinary_status = None,
                4 => value.final_ordinary_message = None,
                5 => value.final_counts = None,
                _ => unreachable!(),
            }
            assert!(value.encode().is_err(), "unknown observation {field}");
        }
        assert!(report().encode().is_ok());
    }
    #[test]
    fn private_report_never_forges_original_body_or_exactly_once_teardown() {
        let mut value = report();
        value.attempts[0].completion.as_mut().unwrap().body_status = 17;
        assert!(value.encode().is_err());
        for attempts in [0, 2] {
            let mut value = report();
            value.destroy_attempts = attempts;
            assert!(value.encode().is_err());
        }
        let mut value = report();
        value.refused = true;
        assert!(value.encode().is_err());
    }
    #[test]
    fn private_report_v2_refuses_missing_or_invalid_snapshot_after_valid_baseline() {
        report().encode().unwrap();
        let mut missing = report();
        missing.observations = None;
        assert!(missing.encode().is_err());
        for mutation in 0..12 {
            let mut value = report();
            let snapshot = value.observations.as_mut().unwrap();
            match mutation {
                0 => snapshot.flags = KNOWN_FLAGS + 1,
                1 => snapshot.counters.values[1] = 1,
                2 => snapshot.attempts[0].identity.id = 42,
                3 => snapshot.attempts[0].end.as_mut().unwrap().sequence = Some(1),
                4 => snapshot.history[1].sequence = 1,
                5 => {
                    if let HistoryKind::CallComplete { call, .. } = &mut snapshot.history[3].kind {
                        call.scope += 1;
                    }
                }
                6 => {
                    if let HistoryKind::CallComplete {
                        entered_sequence, ..
                    } = &mut snapshot.history[3].kind
                    {
                        *entered_sequence = Some(4);
                    }
                }
                7 => snapshot.history[3].kind = HistoryKind::Set,
                8 => {
                    if let HistoryKind::CallComplete { failure_status, .. } =
                        &mut snapshot.history[3].kind
                    {
                        *failure_status = Some(0);
                    }
                }
                9 => {
                    if let HistoryKind::CallComplete { body_status, .. } =
                        &mut snapshot.history[3].kind
                    {
                        *body_status = 71;
                    }
                }
                10 => snapshot.history[3].context = None,
                11 => snapshot.history[5].context = Some(vec![]),
                _ => unreachable!(),
            }
            assert!(value.encode().is_err(), "snapshot mutation {mutation}");
        }
    }
    #[test]
    fn private_report_v2_encodes_explicit_incomplete_boundaries_and_empty_context() {
        report().encode().unwrap();
        let mut value = report();
        let snapshot = value.observations.as_mut().unwrap();
        snapshot.flags = 8;
        snapshot.attempts[0].begin.sequence = None;
        assert_eq!(snapshot.history[1].context.as_deref(), Some(&[][..]));
        assert_eq!(snapshot.history[4].context, None);
        let bytes = value.encode().unwrap();
        assert_eq!(&bytes[..8], b"JTRRP002");
        let mut missing = report();
        missing.observations = None;
        assert!(missing.encode().is_err());
    }
}
