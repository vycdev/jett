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

pub(super) fn decode(bytes: &[u8]) -> Result<Report, DecodeError> {
    if bytes.len() > MAX_REPORT {
        return Err(DecodeError::TooLarge);
    }
    let mut cursor = Cursor { bytes, offset: 0 };
    if cursor.take(8)? != b"JTRRP001" {
        return Err(DecodeError::Magic);
    }
    let version = cursor.u32()?;
    if version != 1 {
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
    fn fixture() -> Vec<u8> {
        let mut bytes = b"JTRRP001".to_vec();
        word(&mut bytes, 1);
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
        for value in [0, 1, 1, 0] {
            word(&mut bytes, value);
        }
        let length = bytes.len() as u64;
        bytes[16..24].copy_from_slice(&length.to_le_bytes());
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
        let first_event = bytes.len() - 16 - 48;
        let mut wrong = bytes.clone();
        wrong[0] = b'X';
        assert_eq!(decode(&wrong), Err(DecodeError::Magic));
        let mut wrong = bytes.clone();
        set_word(&mut wrong, 8, 2);
        assert_eq!(decode(&wrong), Err(DecodeError::Version(2)));
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
        let first_event = bytes.len() - 16 - 48;
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
}
