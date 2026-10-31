//! Private host ABI observations, never evidence of compiled Source execution.
use super::super::resource_report as report;
use jett_runtime::native_abi::JettRuntimeStatusV1;

const MAX_NESTED: usize = 16 * 1024 * 1024;
const MAX_HOST: usize = MAX_NESTED + 256;
#[derive(Debug, PartialEq, Eq)]
pub(super) enum HostDecodeError {
    TooLarge,
    Truncated,
    Magic,
    Version(u32),
    Reserved(u32),
    Length,
    Tag(&'static str, u32),
    Presence,
    Trailing,
    Nested(report::DecodeError),
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ControlOrigin {
    HostAbi,
    PureRecorder,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ControlPurpose {
    Runtime,
    Property,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ControlResult {
    Unavailable,
    HostStatus(u32),
    RecorderOk,
    RecorderErr,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum RecordedBody {
    NoError,
    WrongPurpose,
    InvalidHandle,
    WrongOperation,
    WrongFrame,
    OtherResource(u32),
    HostPanic,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct OversizeProbe {
    pub(super) before_length: u64,
    pub(super) after_length: u64,
    pub(super) bytes_preserved: bool,
    pub(super) cleared: bool,
}
#[derive(Debug, PartialEq, Eq)]
pub(super) struct HostControl {
    pub(super) mode: u32,
    pub(super) origin: ControlOrigin,
    pub(super) failed: bool,
    pub(super) write_status: u32,
    pub(super) result: ControlResult,
    pub(super) recorded_body: Option<RecordedBody>,
    pub(super) oversize_probe: Option<OversizeProbe>,
    pub(super) acquire_operation: Option<u32>,
    pub(super) attempt: Option<u64>,
    pub(super) purpose: Option<ControlPurpose>,
    pub(super) created: bool,
    pub(super) destroy_attempts: u32,
    pub(super) destroy_status: Option<u32>,
    pub(super) report: Option<report::Report>,
}
struct Cursor<'a> {
    bytes: &'a [u8],
    offset: usize,
}
impl<'a> Cursor<'a> {
    fn take(&mut self, length: usize) -> Result<&'a [u8], HostDecodeError> {
        let end = self
            .offset
            .checked_add(length)
            .ok_or(HostDecodeError::TooLarge)?;
        let value = self
            .bytes
            .get(self.offset..end)
            .ok_or(HostDecodeError::Truncated)?;
        self.offset = end;
        Ok(value)
    }
    fn word(&mut self) -> Result<u32, HostDecodeError> {
        Ok(u32::from_le_bytes(
            self.take(4)?
                .try_into()
                .map_err(|_| HostDecodeError::Truncated)?,
        ))
    }
    fn wide(&mut self) -> Result<u64, HostDecodeError> {
        Ok(u64::from_le_bytes(
            self.take(8)?
                .try_into()
                .map_err(|_| HostDecodeError::Truncated)?,
        ))
    }
    fn flag(&mut self, name: &'static str) -> Result<bool, HostDecodeError> {
        match self.word()? {
            0 => Ok(false),
            1 => Ok(true),
            value => Err(HostDecodeError::Tag(name, value)),
        }
    }
    fn optional_word(&mut self, name: &'static str) -> Result<Option<u32>, HostDecodeError> {
        if self.flag(name)? {
            self.word().map(Some)
        } else {
            Ok(None)
        }
    }
    fn optional_wide(&mut self, name: &'static str) -> Result<Option<u64>, HostDecodeError> {
        if self.flag(name)? {
            self.wide().map(Some)
        } else {
            Ok(None)
        }
    }
}
pub(super) fn decode_host(bytes: &[u8]) -> Result<HostControl, HostDecodeError> {
    if bytes.len() > MAX_HOST {
        return Err(HostDecodeError::TooLarge);
    }
    let mut input = Cursor { bytes, offset: 0 };
    if input.take(8)? != b"JTRHC002" {
        return Err(HostDecodeError::Magic);
    }
    let version = input.word()?;
    if version != 2 {
        return Err(HostDecodeError::Version(version));
    }
    let reserved = input.word()?;
    if reserved != 0 {
        return Err(HostDecodeError::Reserved(reserved));
    }
    if input.wide()? != bytes.len() as u64 {
        return Err(HostDecodeError::Length);
    }
    let mode = input.word()?;
    if !(1..=12).contains(&mode) {
        return Err(HostDecodeError::Tag("mode", mode));
    }
    let origin = match input.word()? {
        1 => ControlOrigin::HostAbi,
        2 => ControlOrigin::PureRecorder,
        value => return Err(HostDecodeError::Tag("origin", value)),
    };
    if (mode == 11) != (origin == ControlOrigin::PureRecorder) {
        return Err(HostDecodeError::Presence);
    }
    let failed = input.flag("failed")?;
    let write_status = input.word()?;
    let result = match input.word()? {
        0 => ControlResult::Unavailable,
        1 => ControlResult::HostStatus(input.word()?),
        2 => ControlResult::RecorderOk,
        3 => ControlResult::RecorderErr,
        value => return Err(HostDecodeError::Tag("result", value)),
    };
    let recorded_body = match input.word()? {
        0 => None,
        1 => Some(RecordedBody::NoError),
        2 => Some(RecordedBody::WrongPurpose),
        3 => Some(RecordedBody::InvalidHandle),
        4 => Some(RecordedBody::WrongOperation),
        5 => Some(RecordedBody::WrongFrame),
        6 => {
            let status = input.word()?;
            if status == 0 {
                return Err(HostDecodeError::Presence);
            }
            Some(RecordedBody::OtherResource(status))
        }
        7 => Some(RecordedBody::HostPanic),
        value => return Err(HostDecodeError::Tag("reason", value)),
    };
    let oversize_probe = if input.flag("probe")? {
        Some(OversizeProbe {
            before_length: input.wide()?,
            after_length: input.wide()?,
            bytes_preserved: input.flag("bytes_preserved")?,
            cleared: input.flag("cleared")?,
        })
    } else {
        None
    };
    let acquire_operation = input.optional_word("acquire")?;
    let attempt = input.optional_wide("entry")?;
    let purpose = if attempt.is_some() {
        Some(match input.word()? {
            1 => ControlPurpose::Runtime,
            2 => ControlPurpose::Property,
            value => return Err(HostDecodeError::Tag("purpose", value)),
        })
    } else {
        None
    };
    let created = input.flag("created")?;
    let destroy_attempts = input.word()?;
    if destroy_attempts > 1 {
        return Err(HostDecodeError::Presence);
    }
    let destroy_status = input.optional_word("destroy_status")?;
    let nested = input.flag("nested")?;
    let nested_report = if nested {
        if origin != ControlOrigin::HostAbi {
            return Err(HostDecodeError::Presence);
        }
        let length = usize::try_from(input.wide()?).map_err(|_| HostDecodeError::TooLarge)?;
        if length > MAX_NESTED {
            return Err(HostDecodeError::TooLarge);
        }
        Some(report::decode(input.take(length)?).map_err(HostDecodeError::Nested)?)
    } else {
        None
    };
    if input.offset != bytes.len() {
        return Err(HostDecodeError::Trailing);
    }
    let value = HostControl {
        mode,
        origin,
        failed,
        write_status,
        result,
        recorded_body,
        oversize_probe,
        acquire_operation,
        attempt,
        purpose,
        created,
        destroy_attempts,
        destroy_status,
        report: nested_report,
    };
    value.validate_shape()?;
    Ok(value)
}
impl HostControl {
    fn validate_shape(&self) -> Result<(), HostDecodeError> {
        let bad = HostDecodeError::Presence;
        if self.origin == ControlOrigin::PureRecorder {
            if matches!(self.result, ControlResult::HostStatus(_))
                || self.recorded_body.is_some()
                || self.oversize_probe.is_some()
                || self.acquire_operation.is_some()
                || self.attempt.is_some()
                || self.purpose.is_some()
                || self.created
                || self.destroy_attempts != 0
                || self.destroy_status.is_some()
                || self.report.is_some()
            {
                return Err(bad);
            }
            if self.result != ControlResult::RecorderOk && !self.failed {
                return Err(bad);
            }
        } else {
            if matches!(
                self.result,
                ControlResult::RecorderOk | ControlResult::RecorderErr
            ) || self.attempt == Some(0)
                || self.purpose.is_some() != self.attempt.is_some()
                || (self.oversize_probe.is_some() && self.mode != 12)
                || (self.acquire_operation.is_some() && self.mode == 12)
                || (self.destroy_status.is_some() != (self.destroy_attempts == 1))
                || (!self.created
                    && (self.destroy_attempts != 0
                        || self.attempt.is_some()
                        || self.report.is_some()))
                || self.purpose.is_some_and(|purpose| {
                    (purpose == ControlPurpose::Property) != (self.mode == 10)
                })
            {
                return Err(bad);
            }
            if !self.failed
                && (self.result == ControlResult::Unavailable
                    || self.recorded_body.is_none()
                    || self.attempt.is_none()
                    || !self.created
                    || self.report.is_none()
                    || self.destroy_status.is_none()
                    || (self.mode == 12 && self.oversize_probe.is_none())
                    || (self.mode != 12 && self.acquire_operation.is_none()))
            {
                return Err(bad);
            }
            if let Some(report) = &self.report {
                let ControlResult::HostStatus(status) = self.result else {
                    return Err(bad);
                };
                if report.attempts.len() != 1
                    || Some(report.attempts[0].completion.attempt) != self.attempt
                    || report.attempts[0].completion.body_status != status
                    || report.created != self.created
                    || report.destroy_attempts != self.destroy_attempts
                    || Some(report.destroy_status) != self.destroy_status
                    || report.write_status != self.write_status
                {
                    return Err(bad);
                }
            }
        }
        if self.write_status != 0 && !self.failed {
            return Err(bad);
        }
        Ok(())
    }
    pub(super) fn passed(&self) -> bool {
        if self.failed || self.write_status != 0 || self.validate_shape().is_err() {
            return false;
        }
        if self.origin == ControlOrigin::PureRecorder {
            return self.mode == 11 && self.result == ControlResult::RecorderOk;
        }
        let oversized = self.mode == 12;
        let expected_status = if oversized {
            JettRuntimeStatusV1::OK.code()
        } else {
            JettRuntimeStatusV1::INVALID_ARGUMENT.code()
        };
        let expected_reason = match self.mode {
            3 => RecordedBody::WrongOperation,
            6 | 9 => RecordedBody::InvalidHandle,
            10 => RecordedBody::WrongPurpose,
            _ => RecordedBody::NoError,
        };
        let expected = match self.mode {
            1 | 2 | 10 => [1, 1, 0, 0, 0],
            3 => [1, 0, 0, 0, 0],
            4..=6 => [0, 0, 1, 0, 0],
            7..=9 => [0, 0, 0, 1, 0],
            12 => [0; 5],
            _ => return false,
        };
        let Some(report) = &self.report else {
            return false;
        };
        let Some(observation) = report.observations.attempts.first() else {
            return false;
        };
        let Some(end) = observation.end else {
            return false;
        };
        let expected_history = if oversized { 3 } else { 2 };
        let expected_channel = if oversized {
            report::FailureChannel::Clean
        } else {
            report::FailureChannel::Body
        };
        let expected_flags = if oversized { 4 } else { 0 };
        let context_ok = if oversized {
            self.oversize_probe.is_some_and(|probe| {
                probe.before_length == 65_537
                    && probe.after_length == probe.before_length
                    && probe.bytes_preserved
                    && probe.cleared
            })
        } else {
            self.oversize_probe.is_none()
        };
        self.result == ControlResult::HostStatus(expected_status)
            && self.recorded_body == Some(expected_reason)
            && self.created
            && self.destroy_attempts == 1
            && self.destroy_status == Some(0)
            && report.resources_retired()
            && report.ordinary_storage_empty()
            && report.cleanup_succeeded()
            && report.teardown_succeeded()
            && report.events.is_empty()
            && report.events_all_assigned()
            && report.attempts[0].completion.cleanup_status == 0
            && report.attempts[0].completion.selected_channel == expected_channel
            && report.observations.flags == expected_flags
            && report.observations.counters.values == expected
            && context_ok
            && report.observations.attempts.len() == 1
            && observation.identity.id == self.attempt.unwrap_or(0)
            && observation.identity.ordinal == 0
            && observation.begin.counters.values == [0; 5]
            && end.counters.values == expected
            && observation.begin.sequence == Some(1)
            && end.sequence == Some(expected_history as u64)
            && report.observations.history.len() == expected_history
            && report
                .observations
                .history
                .iter()
                .enumerate()
                .all(|(index, event)| {
                    event.sequence == index as u64 + 1
                        && event.channel == report::Channel::Attempt(observation.identity)
                        && event.context.is_none()
                        && if index == 0 {
                            matches!(event.kind, report::HistoryKind::Begin)
                        } else if index + 1 == expected_history {
                            matches!(event.kind, report::HistoryKind::End)
                        } else {
                            oversized && matches!(event.kind, report::HistoryKind::Clear)
                        }
                })
    }
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
    fn set_word(bytes: &mut [u8], offset: usize, value: u32) {
        bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
    }
    fn set_wide(bytes: &mut [u8], offset: usize, value: u64) {
        bytes[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
    }
    fn length(bytes: &mut [u8]) {
        let actual = bytes.len() as u64;
        set_wide(bytes, 16, actual);
    }
    fn counts(mode: u32) -> [u64; 5] {
        match mode {
            1 | 2 | 10 => [1, 1, 0, 0, 0],
            3 => [1, 0, 0, 0, 0],
            4..=6 => [0, 0, 1, 0, 0],
            7..=9 => [0, 0, 0, 1, 0],
            12 => [0; 5],
            _ => panic!("host mode"),
        }
    }
    fn nested(mode: u32) -> Vec<u8> {
        let oversized = mode == 12;
        let status = u32::from(!oversized);
        let history = if oversized { 3 } else { 2 };
        let mut bytes = b"JTRRP002".to_vec();
        word(&mut bytes, 2);
        word(&mut bytes, 0);
        wide(&mut bytes, 0);
        word(&mut bytes, 1);
        word(&mut bytes, 0);
        wide(&mut bytes, 2); // genuine unconsumed provider operations
        wide(&mut bytes, 41);
        for value in [status, 0, status, 0] {
            word(&mut bytes, value);
        }
        for _ in 0..8 {
            wide(&mut bytes, 0);
        }
        word(&mut bytes, 1);
        word(&mut bytes, 0);
        word(&mut bytes, 0);
        word(&mut bytes, 0);
        wide(&mut bytes, 0);
        for value in counts(mode) {
            wide(&mut bytes, value);
        }
        word(&mut bytes, if oversized { 4 } else { 0 });
        word(&mut bytes, 1);
        word(&mut bytes, history);
        wide(&mut bytes, 0);
        wide(&mut bytes, history as u64);
        wide(&mut bytes, 41);
        word(&mut bytes, 0);
        word(&mut bytes, 1);
        wide(&mut bytes, 1);
        for _ in 0..5 {
            wide(&mut bytes, 0);
        }
        word(&mut bytes, 1);
        word(&mut bytes, 1);
        wide(&mut bytes, history as u64);
        for value in counts(mode) {
            wide(&mut bytes, value);
        }
        for index in 0..history {
            wide(&mut bytes, u64::from(index) + 1);
            word(
                &mut bytes,
                if index == 0 {
                    1
                } else if index + 1 == history {
                    6
                } else {
                    5
                },
            );
            word(&mut bytes, 1);
            wide(&mut bytes, 41);
            word(&mut bytes, 0);
            word(&mut bytes, 0);
        }
        for value in [0, 1, 1, 0] {
            word(&mut bytes, value);
        }
        length(&mut bytes);
        report::decode(&bytes).unwrap();
        bytes
    }
    fn fixture(mode: u32) -> Vec<u8> {
        let pure = mode == 11;
        let mut bytes = b"JTRHC002".to_vec();
        word(&mut bytes, 2);
        word(&mut bytes, 0);
        wide(&mut bytes, 0);
        word(&mut bytes, mode);
        word(&mut bytes, if pure { 2 } else { 1 });
        word(&mut bytes, 0);
        word(&mut bytes, 0);
        if pure {
            word(&mut bytes, 2);
        } else {
            word(&mut bytes, 1);
            word(&mut bytes, u32::from(mode != 12));
        }
        word(
            &mut bytes,
            match mode {
                11 => 0,
                3 => 4,
                6 | 9 => 3,
                10 => 2,
                _ => 1,
            },
        );
        word(&mut bytes, u32::from(mode == 12));
        if mode == 12 {
            wide(&mut bytes, 65_537);
            wide(&mut bytes, 65_537);
            word(&mut bytes, 1);
            word(&mut bytes, 1);
        }
        word(&mut bytes, u32::from(!pure && mode != 12));
        if !pure && mode != 12 {
            word(&mut bytes, 0);
        }
        word(&mut bytes, u32::from(!pure));
        if !pure {
            wide(&mut bytes, 41);
            word(&mut bytes, if mode == 10 { 2 } else { 1 });
        }
        word(&mut bytes, u32::from(!pure));
        word(&mut bytes, u32::from(!pure));
        word(&mut bytes, u32::from(!pure));
        if !pure {
            word(&mut bytes, 0);
        }
        word(&mut bytes, u32::from(!pure));
        if !pure {
            let report = nested(mode);
            wide(&mut bytes, report.len() as u64);
            bytes.extend_from_slice(&report);
        }
        length(&mut bytes);
        assert!(
            decode_host(&bytes).unwrap().passed(),
            "uncorrupted host baseline mode {mode}"
        );
        bytes
    }
    #[test]
    fn host_control_all_twelve_producer_shapes_preserve_host_origin_and_unconsumed_script() {
        for mode in 1..=12 {
            let bytes = fixture(mode);
            let control = decode_host(&bytes).unwrap();
            assert!(control.passed());
            assert_eq!(control.mode, mode);
            assert!(matches!(
                report::decode(&bytes),
                Err(report::DecodeError::Magic)
            ));
            if mode == 11 {
                assert_eq!(control.result, ControlResult::RecorderOk);
                assert!(control.report.is_none());
            } else {
                assert_eq!(control.report.as_ref().unwrap().script_remaining, 2);
            }
        }
    }
    #[test]
    fn host_control_rejects_truncation_trailing_and_closed_tag_corruptions() {
        let baseline = fixture(10);
        for end in 0..baseline.len() {
            let mut changed = baseline[..end].to_vec();
            if end >= 24 {
                length(&mut changed);
            }
            assert!(decode_host(&changed).is_err(), "truncation {end}");
        }
        let mut trailing = baseline.clone();
        trailing.push(0);
        length(&mut trailing);
        assert_eq!(decode_host(&trailing), Err(HostDecodeError::Trailing));
        for (offset, value) in [
            (8, 1),
            (12, 1),
            (24, 0),
            (24, 13),
            (28, 3),
            (32, 2),
            (40, 4),
            (48, 8),
            (52, 2),
            (56, 2),
            (64, 2),
            (76, 3),
            (80, 2),
            (84, 2),
            (88, 2),
            (96, 2),
        ] {
            let mut changed = baseline.clone();
            set_word(&mut changed, offset, value);
            assert!(decode_host(&changed).is_err(), "closed field {offset}");
        }
        let mut wrong_length = baseline;
        set_wide(&mut wrong_length, 16, u64::MAX);
        assert!(decode_host(&wrong_length).is_err());
    }
    #[test]
    fn host_control_rejects_missing_duplicate_reordered_fields_and_origin_substitution() {
        let baseline = fixture(6);
        let mut missing = baseline.clone();
        missing.drain(44..48);
        length(&mut missing);
        assert!(decode_host(&missing).is_err());
        let mut duplicate = baseline.clone();
        duplicate.splice(44..44, 1u32.to_le_bytes());
        length(&mut duplicate);
        assert!(decode_host(&duplicate).is_err());
        let mut reordered = baseline.clone();
        let reason = reordered[48..52].to_vec();
        let purpose = reordered[76..80].to_vec();
        reordered[48..52].copy_from_slice(&purpose);
        reordered[76..80].copy_from_slice(&reason);
        assert!(decode_host(&reordered).is_err());
        let mut pure_origin = baseline.clone();
        set_word(&mut pure_origin, 28, 2);
        assert!(decode_host(&pure_origin).is_err());
        let mut pure_mode = baseline;
        set_word(&mut pure_mode, 24, 11);
        assert!(decode_host(&pure_mode).is_err());
        let mut wrong_pure = fixture(11);
        set_word(&mut wrong_pure, 40, 1);
        assert!(decode_host(&wrong_pure).is_err());
    }
    #[test]
    fn host_control_unknown_differs_from_observed_no_error_and_recorder_success() {
        let baseline = fixture(1);
        assert_eq!(
            decode_host(&baseline).unwrap().recorded_body,
            Some(RecordedBody::NoError)
        );
        let mut unknown = baseline;
        set_word(&mut unknown, 32, 1);
        set_word(&mut unknown, 48, 0);
        let control = decode_host(&unknown).unwrap();
        assert_eq!(control.recorded_body, None);
        assert!(!control.passed());
        let mut no_report = fixture(1);
        set_word(&mut no_report, 32, 1);
        set_word(&mut no_report, 96, 0);
        no_report.truncate(100);
        length(&mut no_report);
        let control = decode_host(&no_report).unwrap();
        assert!(control.report.is_none());
        assert!(!control.passed());
        let mut pure_unknown = fixture(11);
        set_word(&mut pure_unknown, 32, 1);
        set_word(&mut pure_unknown, 40, 0);
        let control = decode_host(&pure_unknown).unwrap();
        assert_eq!(control.result, ControlResult::Unavailable);
        assert!(!control.passed());
    }
    #[test]
    fn host_control_distinguishes_wrong_purpose_from_wrong_frame_and_checks_teardown_probe_flags() {
        let mut wrong_frame = fixture(10);
        set_word(&mut wrong_frame, 48, 5);
        let control = decode_host(&wrong_frame).unwrap();
        assert_eq!(control.recorded_body, Some(RecordedBody::WrongFrame));
        assert!(!control.passed());
        let mut failed_destroy = fixture(1);
        set_word(&mut failed_destroy, 32, 1);
        set_word(&mut failed_destroy, 92, 9);
        assert!(
            decode_host(&failed_destroy).is_err(),
            "outer/inner teardown must agree"
        );
        let baseline = fixture(12);
        for (offset, value) in [(56, 65_536), (64, 65_536), (72, 0), (76, 0)] {
            let mut changed = baseline.clone();
            set_word(&mut changed, offset, value);
            assert!(
                !decode_host(&changed).unwrap().passed(),
                "oversize probe {offset}"
            );
        }
        for flags in [0, 1, 2, 8, 16, 32, 63] {
            let mut changed = baseline.clone();
            set_word(&mut changed, 128 + 152 + 40, flags);
            assert!(
                !decode_host(&changed).unwrap().passed(),
                "only BYTE_LIMIT is expected for oversize"
            );
        }
        let mut ordinary_flagged = fixture(1);
        set_word(&mut ordinary_flagged, 108 + 152 + 40, 4);
        assert!(!decode_host(&ordinary_flagged).unwrap().passed());
    }
}

const OBSERVATION_EVIDENCE_ENV: &str = "JETT_RESOURCE_OBSERVATION_EVIDENCE_V2";
const HOST_CONTROL_ENV: &str = "JETT_RESOURCE_NATIVE_TEST_CONTROL_V2";
const HOST_CONTROL_FILE: &str = "resource-native-host-abi-control-v2.bin";
const SOURCE_REPORT_FILE: &str = "resource-native-report-v1.bin";

struct ObservationProgram {
    directory: tempfile::TempDir,
    binary: std::path::PathBuf,
    binary_sha256: String,
    object_sha256: String,
    source_sha256: String,
}
fn observation_evidence(name: &str, files: &[(&str, &[u8])], metadata: serde_json::Value) {
    let Some(root) = std::env::var_os(OBSERVATION_EVIDENCE_ENV) else {
        return;
    };
    let root = std::path::PathBuf::from(root);
    std::fs::create_dir_all(&root).unwrap();
    let directory = root.join(name);
    std::fs::create_dir(&directory)
        .expect("fresh observation evidence segment; preserve previous evidence");
    for (name, bytes) in files {
        std::fs::write(directory.join(name), bytes).unwrap();
    }
    std::fs::write(
        directory.join("observations.json"),
        serde_json::to_vec_pretty(&metadata).unwrap(),
    )
    .unwrap();
}
fn observation_program(
    case: &super::cases::Case,
    release: bool,
    archive: &super::Archive,
) -> ObservationProgram {
    let directory = tempfile::tempdir().unwrap();
    let project = directory.path().join("project");
    let stdlib = directory.path().join("synthetic-stdlib");
    std::fs::create_dir(&project).unwrap();
    std::fs::create_dir(&stdlib).unwrap();
    let source = project.join("main.jett");
    let support = stdlib.join("resource_probe.jett");
    std::fs::write(&source, case.source).unwrap();
    std::fs::write(&support, super::cases::SUPPORT).unwrap();
    let artifact = super::object(&source, &support, release);
    assert_eq!(artifact.target, archive.target);
    assert!(
        artifact
            .symbols
            .iter()
            .any(|symbol| symbol == jett_codegen_cranelift::JETT_AOT_ENTRY_SYMBOL_V1)
    );
    assert!(
        artifact
            .symbols
            .iter()
            .any(|symbol| symbol == "jett_aot_resource_v1_manifest")
    );
    let object = directory.path().join("resource.obj");
    let binary = directory.path().join(if cfg!(windows) {
        "resource.exe"
    } else {
        "resource"
    });
    let object_sha256 = super::digest(&artifact.bytes);
    std::fs::write(&object, &artifact.bytes).unwrap();
    super::link(archive, &object, &binary, directory.path());
    let binary_bytes = std::fs::read(&binary).unwrap();
    let binary_sha256 = super::digest(&binary_bytes);
    let source_sha256 = super::digest(case.source.as_bytes());
    observation_evidence(
        &format!("{}-{}-link", archive.profile, case.name),
        &[
            ("resource.obj", &artifact.bytes),
            (
                if cfg!(windows) {
                    "resource.exe"
                } else {
                    "resource"
                },
                &binary_bytes,
            ),
        ],
        serde_json::json!({
            "kind":"linked-program", "source_sha256":source_sha256, "support_sha256":super::digest(super::cases::SUPPORT.as_bytes()),
            "object_sha256":object_sha256, "binary_sha256":binary_sha256, "archive_sha256":archive.archive_sha256,
            "target":archive.target, "profile":archive.profile, "source_optimization":release,
            "linker_path":archive.linker, "linker_sha256":archive.linker_sha256,
            "ordered_native_library_args":archive.libraries, "sdk_LIB":archive.sdk_lib,
            "runtime_abi":1, "private_report_protocol":2,
        }),
    );
    std::fs::remove_dir_all(&project).unwrap();
    std::fs::remove_dir_all(&stdlib).unwrap();
    std::fs::remove_file(&object).unwrap();
    assert!(
        !source.exists()
            && !support.exists()
            && !project.exists()
            && !stdlib.exists()
            && !object.exists()
    );
    ObservationProgram {
        directory,
        binary,
        binary_sha256,
        object_sha256,
        source_sha256,
    }
}
fn observation_run(
    program: &ObservationProgram,
    archive: &super::Archive,
    name: &str,
    encoded_script: &str,
    mode: Option<&str>,
) -> (std::process::Output, Vec<u8>) {
    assert!(!program.directory.path().join("project").exists());
    assert!(!program.directory.path().join("synthetic-stdlib").exists());
    assert!(!program.directory.path().join("resource.obj").exists());
    assert_eq!(
        super::digest(&std::fs::read(&program.binary).unwrap()),
        program.binary_sha256
    );
    let directory = tempfile::tempdir().unwrap();
    let mut environment = vec![("JETT_RESOURCE_NATIVE_TEST_SCRIPT_V1", encoded_script)];
    if let Some(mode) = mode {
        environment.push((HOST_CONTROL_ENV, mode));
    }
    let actual = super::super::run_bounded_with_environment(
        &program.binary,
        directory.path(),
        &environment,
        Some(("JETT_RESOURCE_NATIVE_TEST_SCRIPT_V1", encoded_script)),
    );
    let file = if mode.is_some() {
        HOST_CONTROL_FILE
    } else {
        SOURCE_REPORT_FILE
    };
    let other = if mode.is_some() {
        SOURCE_REPORT_FILE
    } else {
        HOST_CONTROL_FILE
    };
    assert!(
        !directory.path().join(other).exists(),
        "separate report origins"
    );
    let report_path = directory.path().join(file);
    assert!(std::fs::metadata(&report_path).unwrap().len() <= 16 * 1024 * 1024 + 256);
    let bytes = std::fs::read(&report_path).unwrap();
    observation_evidence(
        &format!("{}-{name}", archive.profile),
        &[
            (file, &bytes),
            ("stdout.bin", &actual.stdout),
            ("stderr.bin", &actual.stderr),
        ],
        serde_json::json!({
            "kind": if mode.is_some() { "private-host-control" } else { "compiled-original-source" },
            "mode":mode, "source_sha256":program.source_sha256, "object_sha256":program.object_sha256,
            "binary_sha256":program.binary_sha256, "archive_sha256":archive.archive_sha256,
            "profile":archive.profile, "exit":actual.status.code(), "script_hex":encoded_script,
            "report_sha256":super::digest(&bytes), "stdout_sha256":super::digest(&actual.stdout),
            "stderr_sha256":super::digest(&actual.stderr), "source_and_object_deleted_before_execution":true,
        }),
    );
    assert!(
        actual.stdout.is_empty() && actual.stderr.is_empty(),
        "{name}: {actual:?}"
    );
    (actual, bytes)
}
fn assert_acquisition_boundaries(observed: &super::report::Report, expected: &[[u64; 5]]) {
    let snapshot = &observed.observations;
    assert_eq!(snapshot.flags, 0);
    assert_eq!(snapshot.attempts.len(), expected.len());
    assert_eq!(observed.attempts.len(), expected.len());
    assert_eq!(
        snapshot.history.len(),
        expected.len() * 2,
        "Runtime entries have no Property-context call history"
    );
    let mut previous = [0; 5];
    for (index, (attempt, expected)) in snapshot.attempts.iter().zip(expected).enumerate() {
        assert_eq!(
            attempt.identity.id,
            observed.attempts[index].completion.attempt
        );
        assert_eq!(attempt.identity.ordinal, index as u32);
        assert_eq!(attempt.begin.counters.values, previous);
        assert_eq!(attempt.end.as_ref().unwrap().counters.values, *expected);
        let begin = &snapshot.history[index * 2];
        let end = &snapshot.history[index * 2 + 1];
        assert_eq!(
            begin.channel,
            super::report::Channel::Attempt(attempt.identity)
        );
        assert_eq!(end.channel, begin.channel);
        assert_eq!(begin.context, None);
        assert_eq!(end.context, None);
        assert!(matches!(begin.kind, super::report::HistoryKind::Begin));
        assert!(matches!(end.kind, super::report::HistoryKind::End));
        assert_eq!(attempt.begin.sequence, Some(begin.sequence));
        assert_eq!(attempt.end.as_ref().unwrap().sequence, Some(end.sequence));
        previous = *expected;
    }
    assert_eq!(snapshot.counters.values, previous);
    assert_eq!(snapshot.context_bytes, 0);
    assert_eq!(snapshot.final_sequence, (expected.len() * 2) as u64);
}
#[test]
#[ignore = "requires fresh pinned measured protocol2 private Debug/Release archives"]
fn native_resource_observation_v2_measures_acquisition_host_ingress_and_reentry() {
    for release in [false, true] {
        let archive = super::archive(release);
        let connected = super::cases::CASES
            .iter()
            .find(|case| case.name == "connected")
            .unwrap();
        let factory_fail = super::cases::CASES
            .iter()
            .find(|case| case.name == "factory_fail")
            .unwrap();
        assert_eq!(connected.source, factory_fail.source);
        let program = observation_program(connected, release, &archive);
        for (case, counters) in [
            (connected, [3, 1, 1, 1, 1]),
            (factory_fail, [1, 1, 1, 1, 1]),
        ] {
            let encoded = super::script(case);
            let (actual, bytes) = observation_run(&program, &archive, case.name, &encoded, None);
            let observed = super::report::decode(&bytes).unwrap();
            assert_eq!(actual.status.code(), Some(case.exit), "{observed:?}");
            super::assert_report(case, &observed);
            assert_acquisition_boundaries(&observed, &[counters]);
        }
        let encoded = super::script_operations(&[super::cases::Script::Construct(2)], 1);
        let (actual, bytes) = observation_run(
            &program,
            &archive,
            "provider-label-mismatch",
            &encoded,
            None,
        );
        let observed = super::report::decode(&bytes).unwrap();
        assert_eq!(actual.status.code(), Some(71));
        assert_eq!(observed.attempts.len(), 1);
        let attempt = &observed.attempts[0];
        assert_eq!(attempt.completion.body_status, 1);
        assert_eq!(attempt.completion.cleanup_status, 0);
        assert_eq!(
            attempt.completion.selected_channel,
            super::report::FailureChannel::Body
        );
        assert!(attempt.ordinary_message.is_empty());
        assert_eq!(
            attempt.resource_message,
            b"native Resource protocol refused"
        );
        assert!(observed.events.is_empty());
        assert_eq!(
            observed.script_remaining, 1,
            "rejected dispatch does not consume provider script"
        );
        assert!(
            observed.resources_retired()
                && observed.ordinary_storage_empty()
                && observed.teardown_succeeded()
        );
        assert!(
            !observed.observations_ready_for_acceptance() && !observed.all_attempts_succeeded()
        );
        assert_acquisition_boundaries(&observed, &[[1, 1, 1, 1, 1]]);
        run_closed_host_controls(&program, &archive, connected);
        drop(program);
        let implicit = super::cases::CASES
            .iter()
            .find(|case| case.name == "implicit_drop")
            .unwrap();
        let program = observation_program(implicit, release, &archive);
        let encoded = super::script_operations(
            &[
                super::cases::Script::Construct(1),
                super::cases::Script::Construct(1),
            ],
            2,
        );
        let (actual, bytes) =
            observation_run(&program, &archive, "two-entry-reentry", &encoded, None);
        let observed = super::report::decode(&bytes).unwrap();
        assert_eq!(actual.status.code(), Some(0));
        assert_eq!(observed.attempts.len(), 2);
        assert!(observed.observations_ready_for_acceptance() && observed.all_attempts_succeeded());
        assert_ne!(
            observed.attempts[0].completion.attempt,
            observed.attempts[1].completion.attempt
        );
        assert!(
            observed
                .attempts
                .iter()
                .all(|attempt| attempt.ordinary_message.is_empty()
                    && attempt.resource_message.is_empty())
        );
        assert_eq!(observed.attempts[0].event_sequence_end, 2);
        assert_eq!(observed.attempts[1].event_sequence_end, 4);
        let expected = [
            super::cases::Event::Constructed(1),
            super::cases::Event::Finalized(1),
            super::cases::Event::Constructed(1),
            super::cases::Event::Finalized(1),
        ];
        assert_eq!(observed.events.len(), expected.len());
        for (index, (actual, expected)) in observed.events.iter().zip(expected).enumerate() {
            let (label, kind) = super::event(expected);
            assert_eq!(
                (actual.sequence, actual.label, actual.kind),
                ((index + 1) as u64, label, kind)
            );
        }
        assert_acquisition_boundaries(&observed, &[[1; 5], [2; 5]]);
    }
}
fn run_closed_host_controls(
    program: &ObservationProgram,
    archive: &super::Archive,
    connected: &super::cases::Case,
) {
    const MODES: [&str; 12] = [
        "hook-null-output",
        "hook-context-overlap-output",
        "hook-unknown-operation",
        "factory-prepare-null-output",
        "factory-prepare-context-overlap-output",
        "factory-prepare-zero-call",
        "factory-commit-null-output",
        "factory-commit-context-overlap-output",
        "factory-commit-zero-prepared",
        "property-acquire-hook",
        "recorder-only",
        "oversized-property-context",
    ];
    let encoded = super::script(connected);
    for (index, mode) in MODES.iter().enumerate() {
        let (actual, bytes) = observation_run(program, archive, mode, &encoded, Some(mode));
        assert!(
            super::report::decode(&bytes).is_err(),
            "host envelope never certifies a Source report"
        );
        let observed = decode_host(&bytes).unwrap();
        assert_eq!(observed.mode, index as u32 + 1);
        assert_eq!(actual.status.code(), Some(0), "{observed:?}");
        assert!(observed.passed(), "{observed:?}");
        if let Some(report) = &observed.report {
            assert_eq!(report.script_remaining, connected.script.len() as u64);
            assert!(report.events.is_empty());
        }
    }
}
