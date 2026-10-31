//! Opt-in host ABI controls; never invokes or certifies a compiled Source body.
#![cfg(all(test, jett_resource_native_test_archive))]
use super::*;
use crate::native_abi::resource::{
    AuthenticatedResourceContext, NativeBodyFailure, NativeResourceError, leaves, test_observations,
};
use crate::resource_custody::{JettResourceCallResultV1, NativeOperation, ResourcePurpose};

const MODE_ENV: &str = "JETT_RESOURCE_NATIVE_TEST_CONTROL_V2";
const CONTROL_FILE: &str = "resource-native-host-abi-control-v2.bin";
const SOURCE_REPORT_FILE: &str = "resource-native-report-v1.bin";
const MAX_CONTROL: usize = 16 * 1024 * 1024 + 256;

#[derive(Clone, Copy)]
#[repr(u32)]
enum Mode {
    HookNullOutput = 1,
    HookContextOverlapOutput = 2,
    HookUnknownOperation = 3,
    FactoryPrepareNullOutput = 4,
    FactoryPrepareContextOverlapOutput = 5,
    FactoryPrepareZeroCall = 6,
    FactoryCommitNullOutput = 7,
    FactoryCommitContextOverlapOutput = 8,
    FactoryCommitZeroPrepared = 9,
    PropertyAcquireHook = 10,
    RecorderOnly = 11,
    OversizedPropertyContext = 12,
}
impl Mode {
    fn parse(value: &str) -> Result<Self, ()> {
        match value {
            "hook-null-output" => Ok(Self::HookNullOutput),
            "hook-context-overlap-output" => Ok(Self::HookContextOverlapOutput),
            "hook-unknown-operation" => Ok(Self::HookUnknownOperation),
            "factory-prepare-null-output" => Ok(Self::FactoryPrepareNullOutput),
            "factory-prepare-context-overlap-output" => {
                Ok(Self::FactoryPrepareContextOverlapOutput)
            }
            "factory-prepare-zero-call" => Ok(Self::FactoryPrepareZeroCall),
            "factory-commit-null-output" => Ok(Self::FactoryCommitNullOutput),
            "factory-commit-context-overlap-output" => Ok(Self::FactoryCommitContextOverlapOutput),
            "factory-commit-zero-prepared" => Ok(Self::FactoryCommitZeroPrepared),
            "property-acquire-hook" => Ok(Self::PropertyAcquireHook),
            "recorder-only" => Ok(Self::RecorderOnly),
            "oversized-property-context" => Ok(Self::OversizedPropertyContext),
            _ => Err(()),
        }
    }
    fn purpose(self) -> ResourcePurpose {
        match self {
            Self::PropertyAcquireHook => ResourcePurpose::Property,
            _ => ResourcePurpose::Runtime,
        }
    }
    fn expected_status(self) -> u32 {
        match self {
            Self::OversizedPropertyContext => JettRuntimeStatusV1::OK.code(),
            Self::RecorderOnly => unreachable!("recorder mode has no host status"),
            _ => JettRuntimeStatusV1::INVALID_ARGUMENT.code(),
        }
    }
    fn expected_body(self) -> RecordedBody {
        match self {
            Self::HookNullOutput
            | Self::HookContextOverlapOutput
            | Self::FactoryPrepareNullOutput
            | Self::FactoryPrepareContextOverlapOutput
            | Self::FactoryCommitNullOutput
            | Self::FactoryCommitContextOverlapOutput => RecordedBody::None,
            Self::HookUnknownOperation => RecordedBody::WrongOperation,
            Self::FactoryPrepareZeroCall | Self::FactoryCommitZeroPrepared => {
                RecordedBody::InvalidHandle
            }
            Self::PropertyAcquireHook => RecordedBody::WrongPurpose,
            Self::OversizedPropertyContext => RecordedBody::None,
            Self::RecorderOnly => unreachable!("recorder mode has no host body"),
        }
    }
    fn counters(self) -> [u64; 5] {
        match self {
            Self::HookNullOutput | Self::HookContextOverlapOutput | Self::PropertyAcquireHook => {
                [1, 1, 0, 0, 0]
            }
            Self::HookUnknownOperation => [1, 0, 0, 0, 0],
            Self::OversizedPropertyContext => [0; 5],
            Self::FactoryPrepareNullOutput
            | Self::FactoryPrepareContextOverlapOutput
            | Self::FactoryPrepareZeroCall => [0, 0, 1, 0, 0],
            Self::FactoryCommitNullOutput
            | Self::FactoryCommitContextOverlapOutput
            | Self::FactoryCommitZeroPrepared => [0, 0, 0, 1, 0],
            Self::RecorderOnly => unreachable!("recorder mode has no host counters"),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum RecordedBody {
    None,
    WrongPurpose,
    InvalidHandle,
    WrongOperation,
    WrongFrame,
    OtherResource(u32),
    HostPanic,
}

struct OversizeProbe {
    before_length: u64,
    after_length: u64,
    bytes_preserved: bool,
    cleared: bool,
}

struct Outcome {
    mode: Mode,
    host_status: Option<u32>,
    recorder_result: Option<bool>,
    recorded_body: Option<RecordedBody>,
    oversize_probe: Option<OversizeProbe>,
    acquire_operation: Option<u32>,
    attempt: Option<u64>,
    report: Option<Report>,
    failed: bool,
    created: bool,
    destroy_attempts: u32,
    destroy_status: Option<u32>,
    write_status: u32,
}
impl Outcome {
    fn pending(mode: Mode) -> Self {
        Self {
            mode,
            host_status: None,
            recorder_result: None,
            recorded_body: None,
            oversize_probe: None,
            acquire_operation: None,
            attempt: None,
            report: None,
            failed: false,
            created: false,
            destroy_attempts: 0,
            destroy_status: None,
            write_status: 0,
        }
    }
}

// This uses the original installed layout and genuine begin_entry issuer. The
// Property control deliberately supplies its real root, because purpose rejects
// before operation-frame validation. No operation activation is manufactured.
fn begin_host(
    context: *const JettRuntimeContextV1,
    mode: Mode,
) -> Result<(u64, u64, Option<u32>), ()> {
    AuthenticatedResourceContext::acquire(context)
        .and_then(|authenticated| {
            authenticated.with_state(|_, state| {
                let resource = state
                    .resource_state
                    .as_mut()
                    .ok_or(NativeResourceError::MissingInstallation)?;
                resource.validate_installation()?;
                let operation = if matches!(mode, Mode::OversizedPropertyContext) {
                    None
                } else {
                    let ordinal = resource
                        .layout
                        .operations()
                        .iter()
                        .enumerate()
                        .find_map(|(ordinal, row)| {
                            matches!(row.operation(), NativeOperation::Acquire { .. })
                                .then_some(ordinal)
                        })
                        .ok_or(NativeResourceError::WrongOperation)?;
                    Some(u32::try_from(ordinal).map_err(|_| NativeResourceError::Capacity)?)
                };
                let attempt = resource.begin_entry(
                    &mut state.values,
                    &state.resources,
                    resource.entry,
                    mode.purpose(),
                )?;
                let root = resource.root_frame(attempt)?;
                Ok((attempt.raw(), root.raw(), operation))
            })
        })
        .map_err(|_| ())
}

fn observed_body(context: *const JettRuntimeContextV1) -> Result<RecordedBody, ()> {
    AuthenticatedResourceContext::acquire(context)
        .and_then(|authenticated| {
            authenticated.with_state(|_, state| {
                let resource = state
                    .resource_state
                    .as_ref()
                    .ok_or(NativeResourceError::MissingInstallation)?;
                let attempt = resource
                    .attempt
                    .as_ref()
                    .ok_or(NativeResourceError::InvalidEntry)?;
                Ok(match &attempt.body {
                    None => RecordedBody::None,
                    Some(NativeBodyFailure::HostPanic) => RecordedBody::HostPanic,
                    Some(NativeBodyFailure::Resource(error)) => match error {
                        NativeResourceError::WrongPurpose => RecordedBody::WrongPurpose,
                        NativeResourceError::InvalidHandle => RecordedBody::InvalidHandle,
                        NativeResourceError::WrongOperation => RecordedBody::WrongOperation,
                        NativeResourceError::WrongFrame => RecordedBody::WrongFrame,
                        other => RecordedBody::OtherResource(other.status().code()),
                    },
                })
            })
        })
        .map_err(|_| ())
}

fn invoke_oversized(
    context: *mut JettRuntimeContextV1,
    root: u64,
    outcome: &mut Outcome,
) -> Result<(), ()> {
    let length = test_observations::MAX_CONTEXT.checked_add(1).ok_or(())?;
    let mut oversized = Vec::new();
    oversized.try_reserve_exact(length).map_err(|_| ())?;
    oversized.resize(length, b'x');
    let authenticated = AuthenticatedResourceContext::acquire(context).map_err(|_| ())?;
    let (function, signature, template, before_length) = authenticated
        .with_state(|_, state| {
            let entry = state
                .resource_state
                .as_ref()
                .ok_or(NativeResourceError::MissingInstallation)?
                .entry;
            state
                .values
                .test_observation_set_property_context(Some(oversized));
            let bytes = state
                .values
                .test_observation_property_context()
                .ok_or(NativeResourceError::InvalidEntry)?;
            let before_length =
                u64::try_from(bytes.len()).map_err(|_| NativeResourceError::Capacity)?;
            Ok((entry.function, entry.signature, entry.scope, before_length))
        })
        .map_err(|_| ())?;
    // The real root header is valid. Oversize telemetry must refuse copying
    // before any SourceCall lookup, without changing this successful leaf.
    let status = unsafe {
        leaves::jett_rt_v1_resource_scope_validate(context, root, function, signature, template)
    };
    outcome.host_status = Some(status.code());
    let probe = authenticated
        .with_state(|_, state| {
            let bytes = state
                .values
                .test_observation_property_context()
                .ok_or(NativeResourceError::InvalidEntry)?;
            let after_length =
                u64::try_from(bytes.len()).map_err(|_| NativeResourceError::Capacity)?;
            let bytes_preserved = bytes.len() == length && bytes.iter().all(|byte| *byte == b'x');
            state.values.test_observation_set_property_context(None);
            let cleared = state.values.test_observation_property_context().is_none();
            Ok(OversizeProbe {
                before_length,
                after_length,
                bytes_preserved,
                cleared,
            })
        })
        .map_err(|_| ())?;
    outcome.oversize_probe = Some(probe);
    Ok(())
}

fn invoke_host(
    context: *mut JettRuntimeContextV1,
    mode: Mode,
    root: u64,
    acquire: u32,
) -> JettRuntimeStatusV1 {
    let mut scalar = MaybeUninit::<u64>::uninit();
    let mut result = MaybeUninit::<JettResourceCallResultV1>::uninit();
    // Each malformed output is rejected by the real leaf before any write.
    // Other cases use valid exclusive output storage and a live stationary context.
    unsafe {
        match mode {
            Mode::HookNullOutput => {
                leaves::jett_rt_v1_resource_hook_prepare(context, root, acquire, 0, ptr::null_mut())
            }
            Mode::HookContextOverlapOutput => {
                leaves::jett_rt_v1_resource_hook_prepare(context, root, acquire, 0, context.cast())
            }
            Mode::HookUnknownOperation => leaves::jett_rt_v1_resource_hook_prepare(
                context,
                root,
                u32::MAX,
                0,
                scalar.as_mut_ptr(),
            ),
            Mode::PropertyAcquireHook => leaves::jett_rt_v1_resource_hook_prepare(
                context,
                root,
                acquire,
                0,
                scalar.as_mut_ptr(),
            ),
            Mode::FactoryPrepareNullOutput => {
                leaves::jett_rt_v1_resource_factory_prepare(context, 0, 0, ptr::null_mut())
            }
            Mode::FactoryPrepareContextOverlapOutput => {
                leaves::jett_rt_v1_resource_factory_prepare(context, 0, 0, context.cast())
            }
            Mode::FactoryPrepareZeroCall => {
                leaves::jett_rt_v1_resource_factory_prepare(context, 0, 0, scalar.as_mut_ptr())
            }
            Mode::FactoryCommitNullOutput => {
                leaves::jett_rt_v1_resource_factory_commit(context, 0, 0, 0, ptr::null_mut())
            }
            Mode::FactoryCommitContextOverlapOutput => {
                leaves::jett_rt_v1_resource_factory_commit(context, 0, 0, 0, context.cast())
            }
            Mode::FactoryCommitZeroPrepared => {
                leaves::jett_rt_v1_resource_factory_commit(context, 0, 0, 0, result.as_mut_ptr())
            }
            Mode::RecorderOnly | Mode::OversizedPropertyContext => {
                unreachable!("mode uses its distinct bounded control route")
            }
        }
    }
}

fn expected_host(outcome: &Outcome, report: &Report) -> bool {
    if outcome.host_status != Some(outcome.mode.expected_status())
        || outcome.recorded_body != Some(outcome.mode.expected_body())
        || report.refused
        || report.attempts.len() != 1
        || !report.final_counts.as_ref().is_some_and(Counts::clean)
        || !report.events.as_ref().is_some_and(Vec::is_empty)
        || report.remaining.is_none()
        || !report.created
        || report.destroy_attempts != 1
        || report.destroy_status != 0
    {
        return false;
    }
    let attempt = &report.attempts[0];
    let Some(completion) = &attempt.completion else {
        return false;
    };
    let Some(snapshot) = &report.observations else {
        return false;
    };
    let Some(observation) = snapshot.attempts.first() else {
        return false;
    };
    let Some(end) = &observation.end else {
        return false;
    };
    let expected = outcome.mode.counters();
    let oversized = matches!(outcome.mode, Mode::OversizedPropertyContext);
    let selected_kind = u32::from(!oversized);
    let flags = if oversized {
        test_observations::BYTE_LIMIT
    } else {
        0
    };
    let context_ok = if oversized {
        outcome.oversize_probe.as_ref().is_some_and(|probe| {
            probe.before_length == test_observations::MAX_CONTEXT as u64 + 1
                && probe.after_length == probe.before_length
                && probe.bytes_preserved
                && probe.cleared
        })
    } else {
        outcome.oversize_probe.is_none()
    };
    let history_ok = if oversized {
        snapshot.history.len() == 3
            && matches!(
                &snapshot.history[1].kind,
                test_observations::HistoryKind::Clear
            )
    } else {
        snapshot.history.len() == 2
    };
    outcome.attempt == Some(completion.attempt)
        && attempt.observed_body_status == outcome.host_status
        && Some(completion.body_status) == outcome.host_status
        && completion.cleanup_status == 0
        && completion.selected_kind == selected_kind
        && attempt.counts.as_ref().is_some_and(Counts::clean)
        && attempt.ordinary_status == Some(0)
        && snapshot.flags == flags
        && snapshot.counters.values == expected
        && context_ok
        && history_ok
        && snapshot.attempts.len() == 1
        && observation.identity.id == completion.attempt
        && observation.identity.ordinal == 0
        && observation.begin.counters.values == [0; 5]
        && end.counters.values == expected
        && matches!(
            snapshot.history.first().map(|event| &event.kind),
            Some(test_observations::HistoryKind::Begin)
        )
        && matches!(
            snapshot.history.last().map(|event| &event.kind),
            Some(test_observations::HistoryKind::End)
        )
}

fn run_host(mode: Mode) -> Outcome {
    let mut outcome = Outcome::pending(mode);
    let mut session = Session::new();
    let mut report = Report::default();
    let ran = contained(|| -> Result<(), ()> {
        let (script, attempts) = script()?;
        if attempts != 1 {
            return Err(());
        }
        session.create()?;
        install(&mut session, &script)?;
        report.attempts.try_reserve(1).map_err(|_| ())?;
        let (id, root, acquire) = begin_host(session.pointer(), mode)?;
        outcome.acquire_operation = acquire;
        outcome.attempt = Some(id);
        report.attempts.push(Attempt::pending(id));
        session.active = Some(ActiveAttempt {
            id,
            index: 0,
            body_status: None,
            panicked: 0,
            completion_attempted: false,
        });
        let status = if matches!(mode, Mode::OversizedPropertyContext) {
            invoke_oversized(session.pointer(), root, &mut outcome)?;
            outcome.host_status.ok_or(())?
        } else {
            invoke_host(session.pointer(), mode, root, acquire.ok_or(())?).code()
        };
        outcome.host_status = Some(status);
        // This completion body field is the actual host ABI leaf return, not an
        // observed Source body. Only the outer host envelope gives it that origin.
        report.attempts[0].observed_body_status = Some(status);
        session.active.as_mut().ok_or(())?.body_status = Some(status);
        outcome.recorded_body = Some(observed_body(session.pointer())?);
        complete_active(&mut session, &mut report);
        observe_attempt(&mut session, &mut report, 0);
        Ok(())
    });
    if !matches!(ran, Ok(Ok(()))) {
        outcome.failed = true;
    }
    // Preserve an already returned host status even if a subsequent diagnostic
    // probe failed before the ordinary completion bookkeeping was reached.
    if let (Some(status), Some(active)) = (outcome.host_status, session.active.as_mut()) {
        if active.body_status.is_none() {
            active.body_status = Some(status);
            report.attempts[active.index].observed_body_status = Some(status);
        }
    }
    if contained(|| observe_final(&mut session, &mut report)).is_err() {
        report.refused = true;
        outcome.failed = true;
    }
    session.destroy_once();
    report.created = session.created;
    report.destroy_attempts = session.destroy_attempts;
    report.destroy_status = session.destroy_status;
    outcome.created = session.created;
    outcome.destroy_attempts = session.destroy_attempts;
    outcome.destroy_status = (session.destroy_attempts != 0).then_some(session.destroy_status);
    if !expected_host(&outcome, &report) {
        outcome.failed = true;
    }
    outcome.report = Some(report);
    outcome
}

struct Encoder(Vec<u8>);
impl Encoder {
    fn bytes(&mut self, bytes: &[u8]) -> Result<(), ()> {
        let length = self.0.len().checked_add(bytes.len()).ok_or(())?;
        if length > MAX_CONTROL {
            return Err(());
        }
        self.0.try_reserve(bytes.len()).map_err(|_| ())?;
        self.0.extend_from_slice(bytes);
        Ok(())
    }
    fn u32(&mut self, value: u32) -> Result<(), ()> {
        self.bytes(&value.to_le_bytes())
    }
    fn u64(&mut self, value: u64) -> Result<(), ()> {
        self.bytes(&value.to_le_bytes())
    }
    fn optional_u32(&mut self, value: Option<u32>) -> Result<(), ()> {
        self.u32(u32::from(value.is_some()))?;
        if let Some(value) = value {
            self.u32(value)?;
        }
        Ok(())
    }
    fn optional_u64(&mut self, value: Option<u64>) -> Result<(), ()> {
        self.u32(u32::from(value.is_some()))?;
        if let Some(value) = value {
            self.u64(value)?;
        }
        Ok(())
    }
}

fn encode(outcome: &Outcome) -> Result<(Vec<u8>, bool), ()> {
    let pure = matches!(outcome.mode, Mode::RecorderOnly);
    let nested = match &outcome.report {
        Some(report) => report.encode().ok(),
        None => None,
    };
    if pure
        && (outcome.host_status.is_some()
            || outcome.recorded_body.is_some()
            || outcome.oversize_probe.is_some()
            || outcome.acquire_operation.is_some()
            || outcome.attempt.is_some()
            || outcome.report.is_some()
            || outcome.created
            || outcome.destroy_attempts != 0
            || outcome.destroy_status.is_some())
    {
        return Err(());
    }
    let failed = outcome.failed
        || (!pure && nested.is_none())
        || (pure && outcome.recorder_result != Some(true));
    let mut output = Encoder(Vec::new());
    output.bytes(b"JTRHC002")?;
    output.u32(2)?;
    output.u32(0)?;
    output.u64(0)?;
    output.u32(outcome.mode as u32)?;
    output.u32(if pure { 2 } else { 1 })?; // Pure recorder / host ABI leaf.
    output.u32(u32::from(failed))?;
    output.u32(outcome.write_status)?;
    if pure {
        match outcome.recorder_result {
            Some(true) => output.u32(2)?,
            Some(false) => output.u32(3)?,
            None => output.u32(0)?,
        }
    } else if let Some(status) = outcome.host_status {
        output.u32(1)?;
        output.u32(status)?;
    } else {
        output.u32(0)?;
    }
    match outcome.recorded_body {
        None => output.u32(0)?, // Unavailable; distinct from an observed empty body error.
        Some(RecordedBody::None) => output.u32(1)?,
        Some(RecordedBody::WrongPurpose) => output.u32(2)?,
        Some(RecordedBody::InvalidHandle) => output.u32(3)?,
        Some(RecordedBody::WrongOperation) => output.u32(4)?,
        Some(RecordedBody::WrongFrame) => output.u32(5)?,
        Some(RecordedBody::OtherResource(status)) => {
            output.u32(6)?;
            output.u32(status)?;
        }
        Some(RecordedBody::HostPanic) => output.u32(7)?,
    }
    output.u32(u32::from(outcome.oversize_probe.is_some()))?;
    if let Some(probe) = &outcome.oversize_probe {
        output.u64(probe.before_length)?;
        output.u64(probe.after_length)?;
        output.u32(u32::from(probe.bytes_preserved))?;
        output.u32(u32::from(probe.cleared))?;
    }
    output.optional_u32(outcome.acquire_operation)?;
    output.optional_u64(outcome.attempt)?;
    if outcome.attempt.is_some() {
        output.u32(if matches!(outcome.mode, Mode::PropertyAcquireHook) {
            2
        } else {
            1
        })?;
    }
    output.u32(u32::from(outcome.created))?;
    output.u32(outcome.destroy_attempts)?;
    output.optional_u32(outcome.destroy_status)?;
    output.u32(u32::from(nested.is_some()))?;
    if let Some(nested) = nested {
        output.u64(nested.len() as u64)?;
        output.bytes(&nested)?;
    }
    let length = output.0.len() as u64;
    output.0[16..24].copy_from_slice(&length.to_le_bytes());
    Ok((output.0, failed))
}

fn remove_previous() -> Result<(), ()> {
    for file in [CONTROL_FILE, SOURCE_REPORT_FILE] {
        match std::fs::remove_file(file) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(_) => return Err(()),
        }
    }
    Ok(())
}
fn publish(mut outcome: Outcome) -> Result<c_int, ()> {
    let (bytes, failed) = encode(&outcome)?;
    if std::fs::write(CONTROL_FILE, bytes).is_ok() {
        return Ok(if failed { EXIT_INFRASTRUCTURE } else { 0 });
    }
    outcome.failed = true;
    outcome.write_status = JettRuntimeStatusV1::IO_FAILURE.code();
    if let Some(report) = &mut outcome.report {
        report.write_status = outcome.write_status;
    }
    if let Ok((bytes, _)) = encode(&outcome) {
        let _ = std::fs::write(CONTROL_FILE, bytes);
    }
    Ok(EXIT_INFRASTRUCTURE)
}

pub(super) fn dispatch() -> Result<Option<c_int>, ()> {
    let Some(requested) = std::env::var_os(MODE_ENV) else {
        return Ok(None);
    };
    contained(|| {
        // An unknown mode cannot leave an earlier successful control or Source report.
        remove_previous()?;
        let mode = Mode::parse(requested.to_str().ok_or(())?)?;
        std::panic::set_hook(Box::new(|_| {}));
        let outcome = if matches!(mode, Mode::RecorderOnly) {
            let mut outcome = Outcome::pending(mode);
            match contained(|| {
                test_observations::recorder_controls().and_then(|()| report::writer_controls())
            }) {
                Ok(Ok(())) => outcome.recorder_result = Some(true),
                Ok(Err(())) => {
                    outcome.recorder_result = Some(false);
                    outcome.failed = true;
                }
                Err(()) => outcome.failed = true,
            }
            outcome
        } else {
            run_host(mode)
        };
        publish(outcome).map(Some)
    })?
}
