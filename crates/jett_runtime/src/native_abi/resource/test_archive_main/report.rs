//! Canonical owned report transport for the private compiler-test main.
//! These records contain observations, never registry keys or custody tokens.
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
        output.bytes(b"JTRRP001")?;
        output.u32(1)?;
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
        Report {
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
        }
    }
    #[test]
    fn private_report_v1_keeps_original_status_and_existing_binary_offsets() {
        let bytes = report().encode().unwrap();
        assert_eq!(bytes.len(), 168);
        assert_eq!(&bytes[..8], b"JTRRP001");
        assert_eq!(&bytes[8..12], &1u32.to_le_bytes());
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
}
