//! Native replay and shrinking against one already validated compiler session.
use super::*;
use crate::native_property_cases::NativePropertyTrial;

#[derive(Clone, Copy, Debug)]
pub struct NativePropertyOptions {
    pub optimize: bool,
    pub attempt_timeout: Duration,
}

impl Default for NativePropertyOptions {
    fn default() -> Self {
        Self {
            optimize: false,
            attempt_timeout: Duration::from_secs(60),
        }
    }
}

#[derive(Debug)]
pub struct NativePropertyFailure {
    pub name: String,
    pub span: jett_common::Span,
    /// One-based iteration within this property.
    pub trial: usize,
    pub counterexample: String,
}

#[derive(Debug)]
pub struct NativePropertySuiteResult {
    /// Trials executed before success or the first failing case.
    pub trials: usize,
    /// Output of the original suite; replay output is never mixed into it.
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    pub failure: Option<NativePropertyFailure>,
    pub frontend_debug_observations: Vec<crate::DebugObservation>,
}

#[derive(Debug)]
pub enum NativePropertyRunError {
    Captured {
        source: Box<NativePropertyRunError>,
        debug_observations: Vec<crate::DebugObservation>,
    },
    Build(NativeBuildError),
    Lowering(BackendLoweringError),
    TemporaryDirectory(io::Error),
    Execution(NativeBuildError),
    UnexpectedExit {
        code: Option<i32>,
        stdout: Vec<u8>,
        stderr: Vec<u8>,
    },
    ReplayPassed {
        stdout: Vec<u8>,
        stderr: Vec<u8>,
    },
    /// A private replay failed after the original suite produced its output.
    ReplayFailed {
        source: Box<NativePropertyRunError>,
        /// Output belongs to the original suite, never the private replay.
        stdout: Vec<u8>,
        stderr: Vec<u8>,
    },
}

impl NativePropertyRunError {
    pub fn debug_observations(&self) -> &[crate::DebugObservation] {
        match self {
            Self::Captured {
                debug_observations, ..
            } => debug_observations,
            Self::Build(error) | Self::Execution(error) => error.debug_observations(),
            Self::Lowering(error) => error.debug_observations(),
            Self::ReplayFailed { source, .. } => source.debug_observations(),
            _ => &[],
        }
    }

    pub fn build_result(&self) -> Option<&crate::BuildResult> {
        match self {
            Self::Captured { source, .. } => source.build_result(),
            Self::Build(error) | Self::Execution(error) => error.build_result(),
            Self::Lowering(error) => error.build_result(),
            Self::ReplayFailed { source, .. } => source.build_result(),
            _ => None,
        }
    }

    /// Original suite streams when that process returned captured output.
    /// An execution error before `CommandOutput` is returned has no such record.
    pub fn original_suite_output(&self) -> Option<(&[u8], &[u8])> {
        match self {
            Self::Captured { source, .. } => source.original_suite_output(),
            Self::UnexpectedExit { stdout, stderr, .. }
            | Self::ReplayPassed { stdout, stderr }
            | Self::ReplayFailed { stdout, stderr, .. } => Some((stdout, stderr)),
            _ => None,
        }
    }

    fn with_original_suite_output(self, stdout: &[u8], stderr: &[u8]) -> Self {
        Self::ReplayFailed {
            source: Box::new(self.without_private_process_output()),
            stdout: stdout.to_vec(),
            stderr: stderr.to_vec(),
        }
    }

    fn without_private_process_output(self) -> Self {
        match self {
            Self::Captured {
                source,
                debug_observations,
            } => Self::Captured {
                source: Box::new(source.without_private_process_output()),
                debug_observations,
            },
            Self::Execution(error) => Self::Execution(without_private_execution_output(error)),
            Self::UnexpectedExit { code, .. } => Self::UnexpectedExit {
                code,
                stdout: Vec::new(),
                stderr: Vec::new(),
            },
            Self::ReplayPassed { .. } => Self::ReplayPassed {
                stdout: Vec::new(),
                stderr: Vec::new(),
            },
            Self::ReplayFailed { source, .. } => Self::ReplayFailed {
                source: Box::new(source.without_private_process_output()),
                stdout: Vec::new(),
                stderr: Vec::new(),
            },
            error => error,
        }
    }

    fn with_debug_observations(self, observations: &[crate::DebugObservation]) -> Self {
        if observations.is_empty() || !self.debug_observations().is_empty() {
            self
        } else {
            Self::Captured {
                source: Box::new(self),
                debug_observations: observations.to_vec(),
            }
        }
    }
}

fn without_private_execution_output(error: NativeBuildError) -> NativeBuildError {
    match error {
        NativeBuildError::Captured {
            source,
            debug_observations,
        } => NativeBuildError::Captured {
            source: Box::new(without_private_execution_output(*source)),
            debug_observations,
        },
        NativeBuildError::LinkTimedOut {
            linker, timeout, ..
        } => NativeBuildError::LinkTimedOut {
            linker,
            timeout,
            stdout: String::new(),
            stderr: String::new(),
        },
        error => error,
    }
}

impl fmt::Display for NativePropertyRunError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Captured { source, .. } => fmt::Display::fmt(source, f),
            Self::Build(error) => write!(f, "native property build failed: {error}"),
            Self::Lowering(error) => write!(f, "native property lowering failed: {error}"),
            Self::TemporaryDirectory(error) => {
                write!(f, "cannot create native property workspace: {error}")
            }
            Self::Execution(NativeBuildError::LinkTimedOut { timeout, .. }) => {
                write!(f, "native property attempt timed out after {timeout:?}")
            }
            Self::Execution(error) => write!(f, "native property process failed: {error}"),
            Self::UnexpectedExit { code, stderr, .. } => write!(
                f,
                "native property process did not complete with successful cleanup (exit {code:?}): {}",
                String::from_utf8_lossy(stderr)
            ),
            Self::ReplayPassed { stderr, .. } => write!(
                f,
                "native property suite failure could not be reproduced in an isolated trial: {}",
                String::from_utf8_lossy(stderr)
            ),
            Self::ReplayFailed { source, .. } => {
                write!(f, "native property replay failed: {source}")
            }
        }
    }
}

impl std::error::Error for NativePropertyRunError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Build(error) | Self::Execution(error) => Some(error),
            Self::Lowering(error) => Some(error),
            Self::Captured { source, .. } => Some(source.as_ref()),
            Self::ReplayFailed { source, .. } => Some(source.as_ref()),
            Self::TemporaryDirectory(error) => Some(error),
            _ => None,
        }
    }
}

/// Validate, compile, and run the primary file's native property suite. If it
/// fails, replay and shrink its first failing case using only generated native
/// executables. Normal frontend verification is never bypassed.
pub fn run_host_property_suite(
    source_path: &Path,
    launcher: &NativeLauncherBundle,
    options: NativePropertyOptions,
) -> Result<NativePropertySuiteResult, NativePropertyRunError> {
    validate_regular_file(source_path, NativePathRole::Source, Some("jett"))
        .map_err(NativePropertyRunError::Build)?;
    let lowered = lower_file_for_native_property_suite(source_path)
        .map_err(NativePropertyRunError::Lowering)?;
    run_lowered(source_path, &lowered, launcher, options)
}

pub(super) fn run_lowered(
    source_path: &Path,
    lowered: &BackendLoweringResult,
    launcher: &NativeLauncherBundle,
    options: NativePropertyOptions,
) -> Result<NativePropertySuiteResult, NativePropertyRunError> {
    run_lowered_inner(source_path, lowered, launcher, options)
        .map_err(|error| error.with_debug_observations(&lowered.debug_observations))
}

fn run_lowered_inner(
    source_path: &Path,
    lowered: &BackendLoweringResult,
    launcher: &NativeLauncherBundle,
    options: NativePropertyOptions,
) -> Result<NativePropertySuiteResult, NativePropertyRunError> {
    let plan = lowered.native_property_plan.as_ref().ok_or_else(|| {
        NativePropertyRunError::Build(NativeBuildError::MissingPropertyBodies {
            source_path: source_path.to_owned(),
        })
    })?;
    let directory = tempfile::tempdir().map_err(NativePropertyRunError::TemporaryDirectory)?;
    let runner = Runner {
        source_path,
        lowered,
        launcher,
        options,
        executable: directory.path().join("property-attempt.exe"),
        entry: plan.entry,
    };
    let original = runner.execute(&lowered.mir)?;
    if !failed(&original)? {
        return Ok(NativePropertySuiteResult {
            trials: plan.trials.len(),
            stdout: original.stdout,
            stderr: original.stderr,
            failure: None,
            frontend_debug_observations: lowered.debug_observations.clone(),
        });
    }

    // Locate the first failure without interpreting human-readable output.
    // Every prefix executes in a fresh process. The isolated replay below also
    // rejects a failure that depends on earlier suite state.
    let mut passing = 0;
    let mut failing = plan.trials.len();
    while passing + 1 < failing {
        let middle = passing + (failing - passing) / 2;
        if runner
            .replay(&plan.trials[..middle], false)
            .map_err(|error| error.with_original_suite_output(&original.stdout, &original.stderr))?
        {
            failing = middle;
        } else {
            passing = middle;
        }
    }
    let trial = &plan.trials[failing - 1];
    if !runner
        .replay(std::slice::from_ref(trial), false)
        .map_err(|error| error.with_original_suite_output(&original.stdout, &original.stderr))?
    {
        return Err(NativePropertyRunError::ReplayPassed {
            stdout: original.stdout,
            stderr: original.stderr,
        });
    }
    let shrunk =
        jett_comptime::verify::shrink_property_inputs(trial.case.arguments.clone(), |arguments| {
            let mut candidate = trial.clone();
            candidate.case.arguments = arguments.to_vec();
            runner.replay(&[candidate], true)
        })
        .map_err(|error| error.with_original_suite_output(&original.stdout, &original.stderr))?;
    let counterexample = trial
        .given_names
        .iter()
        .zip(&shrunk)
        .map(|(name, value)| format!("{name} = {value}"))
        .collect::<Vec<_>>()
        .join(", ");
    Ok(NativePropertySuiteResult {
        trials: failing,
        stdout: original.stdout,
        stderr: original.stderr,
        frontend_debug_observations: lowered.debug_observations.clone(),
        failure: Some(NativePropertyFailure {
            name: trial.name.clone(),
            span: trial.span,
            trial: trial.case.iteration + 1,
            counterexample,
        }),
    })
}

fn failed(output: &CommandOutput) -> Result<bool, NativePropertyRunError> {
    match output.status.code() {
        Some(0) => Ok(false),
        // The launcher returns 71 only if entry failed and cleanup succeeded.
        Some(71) => Ok(true),
        code => Err(NativePropertyRunError::UnexpectedExit {
            code,
            stdout: output.stdout.clone(),
            stderr: output.stderr.clone(),
        }),
    }
}

struct Runner<'a> {
    source_path: &'a Path,
    lowered: &'a BackendLoweringResult,
    launcher: &'a NativeLauncherBundle,
    options: NativePropertyOptions,
    executable: PathBuf,
    entry: FunctionId,
}

impl Runner<'_> {
    fn replay(
        &self,
        trials: &[NativePropertyTrial],
        validate_inputs: bool,
    ) -> Result<bool, NativePropertyRunError> {
        let mir = self
            .lowered
            .native_property_plan
            .as_ref()
            .unwrap()
            .replay_mir(self.lowered, trials, validate_inputs)
            .map_err(NativePropertyRunError::Lowering)?;
        failed(&self.execute(&mir)?)
    }

    fn execute(&self, mir: &jett_mir::Program) -> Result<CommandOutput, NativePropertyRunError> {
        let emitted = jett_codegen_cranelift::emit_host_program_object_with_options(
            mir,
            &self.lowered.interner,
            self.entry,
            jett_codegen_cranelift::CodegenOptions {
                optimize: self.options.optimize,
            },
        )
        .map_err(|source| {
            NativePropertyRunError::Build(NativeBuildError::Codegen {
                source_path: self.source_path.to_owned(),
                source,
            })
        })?;
        let object = NativeProgramObjectArtifact {
            object: native_object(emitted.target, emitted.symbols, emitted.bytes)
                .map_err(NativePropertyRunError::Build)?,
            program_entry: self.entry,
            diagnostics: self.lowered.diagnostics.clone(),
            source: self.lowered.source.clone(),
        };
        link_host_object(&object, self.launcher, &self.executable)
            .map_err(NativePropertyRunError::Build)?;
        let mut command = Command::new(&self.executable);
        command.stdin(Stdio::null());
        run_command_with_timeout(command, &self.executable, self.options.attempt_timeout)
            .map_err(NativePropertyRunError::Execution)
    }
}

#[cfg(test)]
mod debug_capture_tests {
    use super::*;

    fn frontend_observation() -> crate::DebugObservation {
        crate::DebugObservation {
            phase: crate::DebugPhase::Comptime,
            event: crate::DebugEvent {
                kind: crate::DebugEventKind::Print,
                text: "frontend:".into(),
            },
        }
    }

    fn assert_private_streams_empty(error: &NativePropertyRunError) {
        match error {
            NativePropertyRunError::Captured { source, .. }
            | NativePropertyRunError::ReplayFailed { source, .. } => {
                assert_private_streams_empty(source);
            }
            NativePropertyRunError::UnexpectedExit {
                code,
                stdout,
                stderr,
            } => {
                assert_eq!(*code, Some(72));
                assert!(stdout.is_empty());
                assert!(stderr.is_empty());
            }
            NativePropertyRunError::Execution(error) => {
                let mut error = error;
                while let NativeBuildError::Captured { source, .. } = error {
                    error = source;
                }
                let NativeBuildError::LinkTimedOut {
                    timeout,
                    stdout,
                    stderr,
                    ..
                } = error
                else {
                    panic!("expected private timeout cause: {error}");
                };
                assert_eq!(*timeout, Duration::from_millis(25));
                assert!(stdout.is_empty());
                assert!(stderr.is_empty());
            }
            error => panic!("unexpected replay cause: {error}"),
        }
    }

    #[test]
    fn replay_cleanup_and_timeout_errors_keep_original_streams_and_hide_private_streams() {
        let errors = [
            NativePropertyRunError::UnexpectedExit {
                code: Some(72),
                stdout: b"private replay stdout".to_vec(),
                stderr: b"private replay stderr".to_vec(),
            },
            NativePropertyRunError::Captured {
                source: Box::new(NativePropertyRunError::Execution(
                    NativeBuildError::Captured {
                        source: Box::new(NativeBuildError::LinkTimedOut {
                            linker: PathBuf::from("private-trial.exe"),
                            timeout: Duration::from_millis(25),
                            stdout: "private replay stdout".into(),
                            stderr: "private replay stderr".into(),
                        }),
                        debug_observations: Vec::new(),
                    },
                )),
                debug_observations: Vec::new(),
            },
        ];
        for error in errors {
            let failure = error
                .with_original_suite_output(b"original application", b"original debug\n")
                .with_debug_observations(&[frontend_observation()]);
            assert_eq!(
                failure.original_suite_output(),
                Some((&b"original application"[..], &b"original debug\n"[..]))
            );
            assert_eq!(failure.debug_observations(), [frontend_observation()]);
            assert_private_streams_empty(&failure);
            let display = failure.to_string();
            assert!(display.starts_with("native property replay failed:"));
            assert!(!display.contains("private replay"), "{display}");
            assert!(!format!("{failure:?}").contains("private replay"));
            let mut cause = std::error::Error::source(&failure);
            while let Some(error) = cause {
                assert!(!error.to_string().contains("private replay"));
                assert!(!format!("{error:?}").contains("private replay"));
                cause = error.source();
            }
        }
    }

    #[test]
    fn original_attempt_timeout_keeps_its_existing_typed_partial_capture() {
        let failure = NativePropertyRunError::Execution(NativeBuildError::LinkTimedOut {
            linker: PathBuf::from("original-suite.exe"),
            timeout: Duration::from_millis(25),
            stdout: "partial application".into(),
            stderr: "partial debug".into(),
        });
        assert!(failure.original_suite_output().is_none());
        assert_eq!(
            failure.to_string(),
            "native property attempt timed out after 25ms"
        );
        let NativePropertyRunError::Execution(NativeBuildError::LinkTimedOut {
            stdout,
            stderr,
            ..
        }) = failure
        else {
            panic!("original attempt timeout retains its typed contract");
        };
        assert_eq!(stdout, "partial application");
        assert_eq!(stderr, "partial debug");
    }

    fn replay_test_launcher() -> NativeLauncherBundle {
        // Match the existing property test harness so this control also runs
        // independently, including a cold launcher archive.
        let executable = std::env::current_exe().unwrap();
        let profile_directory = executable.parent().unwrap().parent().unwrap();
        let profile = profile_directory.file_name().unwrap().to_str().unwrap();
        let target = profile_directory
            .parent()
            .unwrap()
            .join("native-values-launcher");
        let host = host_target();
        let status = Command::new(env!("CARGO"))
            .args([
                "build",
                "--locked",
                "-q",
                "-p",
                "jett_native_launcher",
                "--target",
                &host,
            ])
            .args([
                "--profile",
                if profile == "debug" { "test" } else { profile },
            ])
            .arg("--target-dir")
            .arg(&target)
            .current_dir(Path::new(env!("CARGO_MANIFEST_DIR")).join("../.."))
            .status()
            .unwrap();
        assert!(status.success(), "launcher build: {status}");
        let directory = target.join(host).join(profile);
        if cfg!(windows) {
            NativeLauncherBundle::windows_msvc_static_v1(directory.join("jett_native_launcher.lib"))
        } else {
            NativeLauncherBundle::linux_gnu_v1(directory.join("libjett_native_launcher.a"))
        }
    }

    #[test]
    fn post_original_replay_lowering_failure_preserves_original_output_and_frontend_capture() {
        if !matches!(
            host_target().as_str(),
            WINDOWS_MSVC_NATIVE_TARGET | LINUX_GNU_NATIVE_TARGET
        ) {
            return;
        }
        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join("property.jett");
        fs::write(
            &source,
            r#"namespace app
function observe(label: string) returns int64:
    print(view label)
    return 7
int64 cached = comptime observe("frontend:")
property stable:
    given number: int64
    print("original:")
    assert number == number
"#,
        )
        .unwrap();
        let mut lowered = lower_file_for_native_property_suite(&source).unwrap();
        let frontend = lowered.debug_observations.clone();
        assert_eq!(frontend.len(), 101);
        let property_id = lowered.native_property_plan.as_ref().unwrap().trials[0].function;
        let property = lowered
            .mir
            .functions
            .iter_mut()
            .find(|function| function.id == property_id)
            .unwrap();
        let mut changed = 0;
        for block in &mut property.blocks {
            for statement in &mut block.statements {
                if let jett_mir::StatementKind::Assert { condition, .. } = &mut statement.kind {
                    condition.kind = jett_hir::ExpressionKind::Bool(false);
                    changed += 1;
                }
            }
        }
        assert_eq!(changed, 1);
        // The already emitted original MIR stays runnable. Checked replay must
        // diagnose a lost source identity instead of discarding original output.
        lowered
            .hir
            .functions
            .retain(|function| function.id != property_id);
        fs::remove_file(&source).unwrap();
        let failure = run_lowered(
            &source,
            &lowered,
            &replay_test_launcher(),
            NativePropertyOptions::default(),
        )
        .unwrap_err();
        assert_eq!(failure.debug_observations(), frontend);
        assert_eq!(
            failure.original_suite_output(),
            Some((
                &b""[..],
                &b"original:runtime error: property 'stable' trial 1: assertion failed\n"[..],
            ))
        );
        let NativePropertyRunError::Captured { source, .. } = &failure else {
            panic!("original frontend capture survives replay failure: {failure}");
        };
        let NativePropertyRunError::ReplayFailed { source, .. } = source.as_ref() else {
            panic!("original runtime streams survive replay failure: {failure}");
        };
        let NativePropertyRunError::Lowering(BackendLoweringError::Hir(errors)) = source.as_ref()
        else {
            panic!("replay retains its actionable typed lowering cause: {failure}");
        };
        assert_eq!(
            errors[0].message,
            "native property replay lost its checked function"
        );
        assert!(
            failure
                .to_string()
                .contains("native property replay lost its checked function")
        );
    }

    #[test]
    fn property_link_failure_retains_original_frontend_phase_capture() {
        let directory = tempfile::tempdir().unwrap();
        let launcher = match host_target().as_str() {
            WINDOWS_MSVC_NATIVE_TARGET => {
                NativeLauncherBundle::windows_msvc_static_v1(directory.path().join("missing.lib"))
            }
            LINUX_GNU_NATIVE_TARGET => {
                NativeLauncherBundle::linux_gnu_v1(directory.path().join("missing.a"))
            }
            _ => return,
        };
        let source = directory.path().join("property.jett");
        fs::write(
            &source,
            r#"namespace app
function observe(label: string) returns int64:
    print(view label)
    return 7
int64 cached = comptime observe("compile:")
property complete:
    given number: int64
    print("trial:")
    assert number == number
"#,
        )
        .unwrap();
        let error = run_host_property_suite(&source, &launcher, NativePropertyOptions::default())
            .unwrap_err();
        let observations = error.debug_observations();
        assert_eq!(observations.len(), 101);
        assert_eq!(observations[0].phase, crate::DebugPhase::Comptime);
        assert_eq!(observations[0].event.kind, crate::DebugEventKind::Print);
        assert_eq!(observations[0].event.text, "compile:");
        assert!(observations[1..].iter().all(|observation| {
            observation.phase == crate::DebugPhase::FrontendVerify
                && observation.event.kind == crate::DebugEventKind::Print
                && observation.event.text == "trial:"
        }));
        assert!(error.to_string().contains("missing."), "{error}");
        assert!(error.build_result().is_none());
        assert!(std::error::Error::source(&error).is_some());
    }
}
