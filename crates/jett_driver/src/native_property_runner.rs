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
}

#[derive(Debug)]
pub enum NativePropertyRunError {
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
}

impl fmt::Display for NativePropertyRunError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
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
        }
    }
}

impl std::error::Error for NativePropertyRunError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Build(error) | Self::Execution(error) => Some(error),
            Self::Lowering(error) => Some(error),
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
        });
    }

    // Locate the first failure without interpreting human-readable output.
    // Every prefix executes in a fresh process. The isolated replay below also
    // rejects a failure that depends on earlier suite state.
    let mut passing = 0;
    let mut failing = plan.trials.len();
    while passing + 1 < failing {
        let middle = passing + (failing - passing) / 2;
        if runner.replay(&plan.trials[..middle])? {
            failing = middle;
        } else {
            passing = middle;
        }
    }
    let trial = &plan.trials[failing - 1];
    if !runner.replay(std::slice::from_ref(trial))? {
        return Err(NativePropertyRunError::ReplayPassed {
            stdout: original.stdout,
            stderr: original.stderr,
        });
    }
    let shrunk =
        jett_comptime::verify::shrink_property_inputs(trial.case.arguments.clone(), |arguments| {
            let mut candidate = trial.clone();
            candidate.case.arguments = arguments.to_vec();
            runner.replay(&[candidate])
        })?;
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
    fn replay(&self, trials: &[NativePropertyTrial]) -> Result<bool, NativePropertyRunError> {
        let mir = self
            .lowered
            .native_property_plan
            .as_ref()
            .unwrap()
            .replay_mir(self.lowered, trials)
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
