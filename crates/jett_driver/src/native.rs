//! Host-native object emission and executable publication.
//!
//! Native compilation is deliberately split into an object stage and a link
//! stage. The object stage consumes the exact checked program-entry identity
//! published by [`crate::BackendLoweringResult`]. The Windows MSVC and Linux GNU
//! link stages accept explicit launcher metadata and invoke tools without a shell,
//! and publishes an executable only after a successful bounded link.

use std::ffi::{OsStr, OsString};
use std::fmt;
use std::fs;
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use jett_codegen_cranelift::{CodegenError, emit_host_object, emit_host_program_object};
use jett_hir::FunctionId;

use crate::{
    BackendLoweringError, BackendLoweringResult, lower_file_for_backend,
    lower_file_for_native_tests,
};

/// The sole target accepted by the version 1 native Windows linker.
pub const WINDOWS_MSVC_NATIVE_TARGET: &str = "x86_64-pc-windows-msvc";

/// Supported Linux GNU host; this is not a cross-linking contract.
pub const LINUX_GNU_NATIVE_TARGET: &str = "x86_64-unknown-linux-gnu";

const SUPPORTED_NATIVE_HOSTS: &str = "x86_64-pc-windows-msvc, x86_64-unknown-linux-gnu";

/// System libraries required by the GNU dynamic-CRT launcher archive.
pub const LINUX_GNU_V1_NATIVE_LIBRARIES: &[&str] = &[
    "-lgcc_s",
    "-lutil",
    "-lrt",
    "-lpthread",
    "-lm",
    "-ldl",
    "-lc",
];

/// The runtime ABI expected by the version 1 native launcher.
pub const NATIVE_RUNTIME_ABI_VERSION_V1: u32 = 1;

/// Maximum wall-clock time allowed for one native linker invocation.
pub const DEFAULT_NATIVE_LINK_TIMEOUT: Duration = Duration::from_secs(60);

/// Canonical ordered native-link inputs required by the static-CRT launcher.
pub const WINDOWS_MSVC_STATIC_V1_NATIVE_LIBRARIES: &[&str] = &[
    "advapi32.lib",
    "bcrypt.lib",
    "gdi32.lib",
    "kernel32.lib",
    "msimg32.lib",
    "opengl32.lib",
    "user32.lib",
    "winspool.lib",
    "kernel32.lib",
    "ntdll.lib",
    "userenv.lib",
    "ws2_32.lib",
    "dbghelp.lib",
    "/defaultlib:libcmt",
];

/// C runtime linkage used to build a launcher archive.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeCrtMode {
    Static,
    Dynamic,
}

impl fmt::Display for NativeCrtMode {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Static => formatter.write_str("static"),
            Self::Dynamic => formatter.write_str("dynamic"),
        }
    }
}

/// Explicit metadata for one native launcher archive.
///
/// Keeping this record separate from the archive path prevents the driver from
/// guessing ABI, CRT, target, or transitive native-library requirements. A
/// future on-disk launcher manifest can deserialize directly into this shape.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeLauncherBundle {
    pub archive_path: PathBuf,
    pub target: String,
    pub runtime_abi_version: u32,
    pub crt_mode: NativeCrtMode,
    pub native_library_args: Vec<OsString>,
}

impl NativeLauncherBundle {
    /// Describe a Linux GNU launcher archive built for the compiler host.
    pub fn linux_gnu_v1(archive_path: impl Into<PathBuf>) -> Self {
        Self {
            archive_path: archive_path.into(),
            target: LINUX_GNU_NATIVE_TARGET.to_string(),
            runtime_abi_version: NATIVE_RUNTIME_ABI_VERSION_V1,
            crt_mode: NativeCrtMode::Dynamic,
            native_library_args: LINUX_GNU_V1_NATIVE_LIBRARIES
                .iter()
                .map(OsString::from)
                .collect(),
        }
    }

    /// Describe the canonical version 1 static-CRT launcher for Windows MSVC.
    pub fn windows_msvc_static_v1(archive_path: impl Into<PathBuf>) -> Self {
        Self {
            archive_path: archive_path.into(),
            target: WINDOWS_MSVC_NATIVE_TARGET.to_string(),
            runtime_abi_version: NATIVE_RUNTIME_ABI_VERSION_V1,
            crt_mode: NativeCrtMode::Static,
            native_library_args: WINDOWS_MSVC_STATIC_V1_NATIVE_LIBRARIES
                .iter()
                .map(OsString::from)
                .collect(),
        }
    }
}

/// A checked Jett program lowered through the production Cranelift backend.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeObjectArtifact {
    target: String,
    symbols: Vec<String>,
    bytes: Vec<u8>,
}

impl NativeObjectArtifact {
    pub fn target(&self) -> &str {
        &self.target
    }

    pub fn symbols(&self) -> &[String] {
        &self.symbols
    }

    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
}

/// Native object containing the exported launcher entry wrapper.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeProgramObjectArtifact {
    object: NativeObjectArtifact,
    program_entry: FunctionId,
}

impl NativeProgramObjectArtifact {
    pub fn target(&self) -> &str {
        self.object.target()
    }

    pub fn program_entry(&self) -> FunctionId {
        self.program_entry
    }

    pub fn symbols(&self) -> &[String] {
        self.object.symbols()
    }

    pub fn bytes(&self) -> &[u8] {
        self.object.bytes()
    }
}

/// A successfully linked and atomically published native executable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeExecutableArtifact {
    pub path: PathBuf,
    pub target: String,
    pub program_entry: FunctionId,
    pub symbols: Vec<String>,
}

/// Filesystem role used in actionable native-build diagnostics.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativePathRole {
    Source,
    LauncherArchive,
    Output,
    OutputDirectory,
    TemporaryBuildDirectory,
    Object,
    LinkedExecutable,
}

impl fmt::Display for NativePathRole {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Source => formatter.write_str("source file"),
            Self::LauncherArchive => formatter.write_str("launcher archive"),
            Self::Output => formatter.write_str("output executable"),
            Self::OutputDirectory => formatter.write_str("output directory"),
            Self::TemporaryBuildDirectory => formatter.write_str("temporary build directory"),
            Self::Object => formatter.write_str("generated object"),
            Self::LinkedExecutable => formatter.write_str("linked executable"),
        }
    }
}

/// Failure while emitting, linking, or publishing a native executable.
#[derive(Debug)]
pub enum NativeBuildError {
    InspectPath {
        role: NativePathRole,
        path: PathBuf,
        source: io::Error,
    },
    ExpectedRegularFile {
        role: NativePathRole,
        path: PathBuf,
    },
    ExpectedDirectory {
        role: NativePathRole,
        path: PathBuf,
    },
    InvalidExtension {
        role: NativePathRole,
        path: PathBuf,
        expected: &'static str,
    },
    ResolveCurrentDirectory(io::Error),
    Lowering {
        source_path: PathBuf,
        source: Box<BackendLoweringError>,
    },
    MissingProgramEntry {
        source_path: PathBuf,
    },
    Codegen {
        source_path: PathBuf,
        source: CodegenError,
    },
    EmptyObject,
    UnsupportedHost {
        actual: String,
        supported: &'static str,
    },
    ObjectTargetMismatch {
        object_target: String,
        host_target: String,
    },
    LauncherTargetMismatch {
        actual: String,
        expected: &'static str,
    },
    LauncherRuntimeAbiMismatch {
        actual: u32,
        expected: u32,
    },
    LauncherCrtMismatch {
        actual: NativeCrtMode,
        expected: NativeCrtMode,
    },
    LauncherNativeLibrariesMismatch {
        actual: Vec<OsString>,
    },
    CreateTemporaryDirectory {
        parent: PathBuf,
        source: io::Error,
    },
    WriteObject {
        path: PathBuf,
        source: io::Error,
    },
    LinkerNotFound {
        target: &'static str,
    },
    WindowsSdkNotFound {
        architecture: &'static str,
    },
    SpawnLinker {
        linker: PathBuf,
        source: io::Error,
    },
    PollLinker {
        linker: PathBuf,
        source: io::Error,
    },
    TerminateLinker {
        linker: PathBuf,
        source: io::Error,
    },
    CaptureLinkerOutput {
        stream: &'static str,
        source: io::Error,
    },
    SpawnLinkerOutputThread {
        stream: &'static str,
        source: io::Error,
    },
    LinkerOutputThreadPanicked {
        stream: &'static str,
    },
    LinkTimedOut {
        linker: PathBuf,
        timeout: Duration,
        stdout: String,
        stderr: String,
    },
    LinkFailed {
        linker: PathBuf,
        status: ExitStatus,
        stdout: String,
        stderr: String,
    },
    MissingLinkedExecutable {
        path: PathBuf,
    },
    PublishExecutable {
        from: PathBuf,
        to: PathBuf,
        source: io::Error,
    },
}

impl fmt::Display for NativeBuildError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InspectPath { role, path, source } => {
                write!(
                    formatter,
                    "cannot inspect {role} `{}`: {source}",
                    path.display()
                )
            }
            Self::ExpectedRegularFile { role, path } => write!(
                formatter,
                "{role} `{}` must be an existing regular file",
                path.display()
            ),
            Self::ExpectedDirectory { role, path } => write!(
                formatter,
                "{role} `{}` must be an existing directory",
                path.display()
            ),
            Self::InvalidExtension {
                role,
                path,
                expected,
            } => write!(
                formatter,
                "{role} `{}` must use the `{expected}` extension",
                path.display()
            ),
            Self::ResolveCurrentDirectory(source) => {
                write!(formatter, "cannot resolve the current directory: {source}")
            }
            Self::Lowering {
                source_path,
                source,
            } => write!(
                formatter,
                "cannot lower native source `{}`: {source}",
                source_path.display()
            ),
            Self::MissingProgramEntry { source_path } => write!(
                formatter,
                "native source `{}` has no checked primary `main` program entry",
                source_path.display()
            ),
            Self::Codegen {
                source_path,
                source,
            } => write!(
                formatter,
                "cannot emit native object for `{}`: {source}",
                source_path.display()
            ),
            Self::EmptyObject => {
                formatter.write_str("native code generation produced an empty object")
            }
            Self::UnsupportedHost { actual, supported } => write!(
                formatter,
                "native executable linking is unsupported on host `{actual}`; supported hosts: {supported}"
            ),
            Self::ObjectTargetMismatch {
                object_target,
                host_target,
            } => write!(
                formatter,
                "native object target `{object_target}` does not match linker host `{host_target}`"
            ),
            Self::LauncherTargetMismatch { actual, expected } => write!(
                formatter,
                "launcher target `{actual}` does not match required target `{expected}`"
            ),
            Self::LauncherRuntimeAbiMismatch { actual, expected } => write!(
                formatter,
                "launcher runtime ABI version {actual} does not match required version {expected}"
            ),
            Self::LauncherCrtMismatch { actual, expected } => write!(
                formatter,
                "launcher CRT mode `{actual}` does not match required mode `{expected}`"
            ),
            Self::LauncherNativeLibrariesMismatch { actual } => write!(
                formatter,
                "launcher native-library arguments {:?} do not match the canonical target-specific version 1 contract",
                actual
            ),
            Self::CreateTemporaryDirectory { parent, source } => write!(
                formatter,
                "cannot create a unique native build directory under `{}`: {source}",
                parent.display()
            ),
            Self::WriteObject { path, source } => write!(
                formatter,
                "cannot write generated object `{}`: {source}",
                path.display()
            ),
            Self::LinkerNotFound { target } => write!(
                formatter,
                "cannot find MSVC `link.exe` and its SDK environment for target `{target}`"
            ),
            Self::WindowsSdkNotFound { architecture } => write!(
                formatter,
                "cannot find a Windows SDK for MSVC architecture `{architecture}`"
            ),
            Self::SpawnLinker { linker, source } => write!(
                formatter,
                "cannot start native linker `{}`: {source}",
                linker.display()
            ),
            Self::PollLinker { linker, source } => write!(
                formatter,
                "cannot wait for native linker `{}`: {source}",
                linker.display()
            ),
            Self::TerminateLinker { linker, source } => write!(
                formatter,
                "cannot terminate timed-out native linker `{}`: {source}",
                linker.display()
            ),
            Self::CaptureLinkerOutput { stream, source } => {
                write!(formatter, "cannot capture native linker {stream}: {source}")
            }
            Self::SpawnLinkerOutputThread { stream, source } => {
                write!(
                    formatter,
                    "cannot start native linker {stream} reader: {source}"
                )
            }
            Self::LinkerOutputThreadPanicked { stream } => {
                write!(formatter, "native linker {stream} reader panicked")
            }
            Self::LinkTimedOut {
                linker,
                timeout,
                stdout,
                stderr,
            } => write!(
                formatter,
                "native linker `{}` exceeded its {:?} deadline\nstdout:\n{}\nstderr:\n{}",
                linker.display(),
                timeout,
                stdout,
                stderr
            ),
            Self::LinkFailed {
                linker,
                status,
                stdout,
                stderr,
            } => write!(
                formatter,
                "native linker `{}` failed with {status}\nstdout:\n{}\nstderr:\n{}",
                linker.display(),
                stdout,
                stderr
            ),
            Self::MissingLinkedExecutable { path } => write!(
                formatter,
                "native linker reported success but did not create `{}`",
                path.display()
            ),
            Self::PublishExecutable { from, to, source } => write!(
                formatter,
                "cannot atomically publish linked executable from `{}` to `{}`: {source}",
                from.display(),
                to.display()
            ),
        }
    }
}

impl std::error::Error for NativeBuildError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::InspectPath { source, .. }
            | Self::CreateTemporaryDirectory { source, .. }
            | Self::WriteObject { source, .. }
            | Self::SpawnLinker { source, .. }
            | Self::PollLinker { source, .. }
            | Self::TerminateLinker { source, .. }
            | Self::CaptureLinkerOutput { source, .. }
            | Self::SpawnLinkerOutputThread { source, .. }
            | Self::PublishExecutable { source, .. } => Some(source),
            Self::ResolveCurrentDirectory(source) => Some(source),
            Self::Lowering { source, .. } => Some(source),
            Self::Codegen { source, .. } => Some(source),
            _ => None,
        }
    }
}

/// Return the exact Cranelift target used by host object emission.
pub fn host_target() -> String {
    jett_codegen_cranelift::host_target().to_string()
}

/// Lower a source file and emit an ordinary reachable host object, including
/// checked `verify` and `property` bodies as callable native test symbols.
///
/// This stage does not require a source `main` and does not add the launcher
/// entry wrapper. Executable publication always uses the distinct program
/// object API below.
pub fn emit_host_object_for_file(
    source_path: &Path,
) -> Result<NativeObjectArtifact, NativeBuildError> {
    validate_regular_file(source_path, NativePathRole::Source, Some("jett"))?;
    let lowered =
        lower_file_for_native_tests(source_path).map_err(|source| NativeBuildError::Lowering {
            source_path: source_path.to_path_buf(),
            source: Box::new(source),
        })?;
    let object = emit_host_object(&lowered.mir, &lowered.interner).map_err(|source| {
        NativeBuildError::Codegen {
            source_path: source_path.to_path_buf(),
            source,
        }
    })?;
    native_object(object.target, object.symbols, object.bytes)
}

/// Lower a source file and emit a host object using its exact checked entry ID.
pub fn emit_host_program_object_for_file(
    source_path: &Path,
) -> Result<NativeProgramObjectArtifact, NativeBuildError> {
    let lowered = lower_checked_file(source_path)?;
    let program_entry =
        lowered
            .program_entry
            .ok_or_else(|| NativeBuildError::MissingProgramEntry {
                source_path: source_path.to_path_buf(),
            })?;
    let object = emit_host_program_object(&lowered.mir, &lowered.interner, program_entry).map_err(
        |source| NativeBuildError::Codegen {
            source_path: source_path.to_path_buf(),
            source,
        },
    )?;
    Ok(NativeProgramObjectArtifact {
        object: native_object(object.target, object.symbols, object.bytes)?,
        program_entry,
    })
}

fn lower_checked_file(source_path: &Path) -> Result<BackendLoweringResult, NativeBuildError> {
    validate_regular_file(source_path, NativePathRole::Source, Some("jett"))?;
    lower_file_for_backend(source_path).map_err(|source| NativeBuildError::Lowering {
        source_path: source_path.to_path_buf(),
        source: Box::new(source),
    })
}

fn native_object(
    target: String,
    symbols: Vec<String>,
    bytes: Vec<u8>,
) -> Result<NativeObjectArtifact, NativeBuildError> {
    if bytes.is_empty() {
        return Err(NativeBuildError::EmptyObject);
    }
    Ok(NativeObjectArtifact {
        target,
        symbols,
        bytes,
    })
}

/// Compile, link, and atomically publish one native executable.
///
/// The launcher archive is supplied by the caller; this function never starts
/// Cargo or attempts to discover build-tree artifacts.
pub fn build_host_executable(
    source_path: &Path,
    launcher: &NativeLauncherBundle,
    output_path: &Path,
) -> Result<NativeExecutableArtifact, NativeBuildError> {
    let object = emit_host_program_object_for_file(source_path)?;
    link_host_object(&object, launcher, output_path)
}

/// Link a production object with an explicit launcher bundle and publish it.
pub fn link_host_object(
    object: &NativeProgramObjectArtifact,
    launcher: &NativeLauncherBundle,
    output_path: &Path,
) -> Result<NativeExecutableArtifact, NativeBuildError> {
    link_host_object_with_timeout(object, launcher, output_path, DEFAULT_NATIVE_LINK_TIMEOUT)
}

fn link_host_object_with_timeout(
    object: &NativeProgramObjectArtifact,
    launcher: &NativeLauncherBundle,
    output_path: &Path,
    timeout: Duration,
) -> Result<NativeExecutableArtifact, NativeBuildError> {
    validate_link_host(object)?;
    validate_host_launcher(launcher)?;
    validate_regular_file(
        &launcher.archive_path,
        NativePathRole::LauncherArchive,
        Some(if launcher.target == LINUX_GNU_NATIVE_TARGET {
            "a"
        } else {
            "lib"
        }),
    )?;
    // Linking runs in a temporary directory, so relative archive inputs must
    // be resolved against the callers current directory first.
    let mut launcher = launcher.clone();
    launcher.archive_path = absolute_output_path(&launcher.archive_path)?;
    let output_path = absolute_output_path(output_path)?;
    validate_output_path(&output_path)?;

    let output_parent =
        output_path
            .parent()
            .ok_or_else(|| NativeBuildError::ExpectedDirectory {
                role: NativePathRole::OutputDirectory,
                path: output_path.clone(),
            })?;
    let build_directory = tempfile::Builder::new()
        .prefix(".jett-native-")
        .tempdir_in(output_parent)
        .map_err(|source| NativeBuildError::CreateTemporaryDirectory {
            parent: output_parent.to_path_buf(),
            source,
        })?;
    let object_path = build_directory.path().join("program.obj");
    let linked_executable = build_directory.path().join("program.exe");
    let linked_pdb = build_directory.path().join("program.pdb");
    fs::write(&object_path, object.bytes()).map_err(|source| NativeBuildError::WriteObject {
        path: object_path.clone(),
        source,
    })?;

    if launcher.target == LINUX_GNU_NATIVE_TARGET {
        link_linux_gnu(
            &object_path,
            &launcher,
            &linked_executable,
            build_directory.path(),
            timeout,
        )?;
    } else {
        link_windows_msvc(
            &object_path,
            &launcher,
            &linked_executable,
            &linked_pdb,
            build_directory.path(),
            timeout,
        )?;
    }
    let linked_metadata = fs::metadata(&linked_executable).map_err(|source| {
        if source.kind() == io::ErrorKind::NotFound {
            NativeBuildError::MissingLinkedExecutable {
                path: linked_executable.clone(),
            }
        } else {
            NativeBuildError::InspectPath {
                role: NativePathRole::LinkedExecutable,
                path: linked_executable.clone(),
                source,
            }
        }
    })?;
    if !linked_metadata.is_file() {
        return Err(NativeBuildError::ExpectedRegularFile {
            role: NativePathRole::LinkedExecutable,
            path: linked_executable,
        });
    }

    publish_executable(&linked_executable, &output_path)?;
    Ok(NativeExecutableArtifact {
        path: output_path,
        target: object.target().to_string(),
        program_entry: object.program_entry,
        symbols: object.symbols().to_vec(),
    })
}

fn validate_link_host(object: &NativeProgramObjectArtifact) -> Result<(), NativeBuildError> {
    validate_link_host_for(object, jett_codegen_cranelift::host_target().to_string())
}

fn validate_link_host_for(
    object: &NativeProgramObjectArtifact,
    host_target: String,
) -> Result<(), NativeBuildError> {
    let supported = (cfg!(all(target_os = "windows", target_env = "msvc"))
        && host_target == WINDOWS_MSVC_NATIVE_TARGET)
        || (cfg!(all(target_os = "linux", target_env = "gnu"))
            && host_target == LINUX_GNU_NATIVE_TARGET);
    if !supported {
        return Err(NativeBuildError::UnsupportedHost {
            actual: host_target,
            supported: SUPPORTED_NATIVE_HOSTS,
        });
    }
    if object.target() != host_target {
        return Err(NativeBuildError::ObjectTargetMismatch {
            object_target: object.target().to_string(),
            host_target,
        });
    }
    if object.bytes().is_empty() {
        return Err(NativeBuildError::EmptyObject);
    }
    Ok(())
}

fn validate_host_launcher(launcher: &NativeLauncherBundle) -> Result<(), NativeBuildError> {
    if cfg!(all(
        target_os = "linux",
        target_env = "gnu",
        target_arch = "x86_64"
    )) {
        validate_launcher_contract(
            launcher,
            LINUX_GNU_NATIVE_TARGET,
            NativeCrtMode::Dynamic,
            LINUX_GNU_V1_NATIVE_LIBRARIES,
        )
    } else {
        validate_launcher(launcher)
    }
}

fn validate_launcher(launcher: &NativeLauncherBundle) -> Result<(), NativeBuildError> {
    validate_launcher_contract(
        launcher,
        WINDOWS_MSVC_NATIVE_TARGET,
        NativeCrtMode::Static,
        WINDOWS_MSVC_STATIC_V1_NATIVE_LIBRARIES,
    )
}

fn validate_launcher_contract(
    launcher: &NativeLauncherBundle,
    target: &'static str,
    crt: NativeCrtMode,
    libraries: &[&str],
) -> Result<(), NativeBuildError> {
    if launcher.target != target {
        return Err(NativeBuildError::LauncherTargetMismatch {
            actual: launcher.target.clone(),
            expected: target,
        });
    }
    if launcher.runtime_abi_version != NATIVE_RUNTIME_ABI_VERSION_V1 {
        return Err(NativeBuildError::LauncherRuntimeAbiMismatch {
            actual: launcher.runtime_abi_version,
            expected: NATIVE_RUNTIME_ABI_VERSION_V1,
        });
    }
    if launcher.crt_mode != crt {
        return Err(NativeBuildError::LauncherCrtMismatch {
            actual: launcher.crt_mode,
            expected: crt,
        });
    }
    let expected: Vec<OsString> = libraries.iter().map(OsString::from).collect();
    if launcher.native_library_args != expected {
        return Err(NativeBuildError::LauncherNativeLibrariesMismatch {
            actual: launcher.native_library_args.clone(),
        });
    }
    Ok(())
}

fn validate_regular_file(
    path: &Path,
    role: NativePathRole,
    extension: Option<&'static str>,
) -> Result<(), NativeBuildError> {
    let metadata = fs::metadata(path).map_err(|source| NativeBuildError::InspectPath {
        role,
        path: path.to_path_buf(),
        source,
    })?;
    if !metadata.is_file() {
        return Err(NativeBuildError::ExpectedRegularFile {
            role,
            path: path.to_path_buf(),
        });
    }
    if let Some(expected) = extension
        && !has_ascii_case_insensitive_extension(path, expected)
    {
        return Err(NativeBuildError::InvalidExtension {
            role,
            path: path.to_path_buf(),
            expected,
        });
    }
    Ok(())
}

fn validate_output_path(path: &Path) -> Result<(), NativeBuildError> {
    if cfg!(windows) && !has_ascii_case_insensitive_extension(path, "exe") {
        return Err(NativeBuildError::InvalidExtension {
            role: NativePathRole::Output,
            path: path.to_path_buf(),
            expected: "exe",
        });
    }
    let Some(parent) = path.parent() else {
        return Err(NativeBuildError::ExpectedDirectory {
            role: NativePathRole::OutputDirectory,
            path: path.to_path_buf(),
        });
    };
    let parent_metadata = fs::metadata(parent).map_err(|source| NativeBuildError::InspectPath {
        role: NativePathRole::OutputDirectory,
        path: parent.to_path_buf(),
        source,
    })?;
    if !parent_metadata.is_dir() {
        return Err(NativeBuildError::ExpectedDirectory {
            role: NativePathRole::OutputDirectory,
            path: parent.to_path_buf(),
        });
    }
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_file() => Ok(()),
        Ok(_) => Err(NativeBuildError::ExpectedRegularFile {
            role: NativePathRole::Output,
            path: path.to_path_buf(),
        }),
        Err(source) if source.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(source) => Err(NativeBuildError::InspectPath {
            role: NativePathRole::Output,
            path: path.to_path_buf(),
            source,
        }),
    }
}

fn has_ascii_case_insensitive_extension(path: &Path, expected: &str) -> bool {
    path.extension()
        .and_then(OsStr::to_str)
        .is_some_and(|extension| extension.eq_ignore_ascii_case(expected))
}

fn absolute_output_path(path: &Path) -> Result<PathBuf, NativeBuildError> {
    if path.is_absolute() {
        return Ok(path.to_path_buf());
    }
    std::env::current_dir()
        .map(|current| current.join(path))
        .map_err(NativeBuildError::ResolveCurrentDirectory)
}

fn link_linux_gnu(
    object: &Path,
    launcher: &NativeLauncherBundle,
    executable: &Path,
    working_directory: &Path,
    timeout: Duration,
) -> Result<(), NativeBuildError> {
    // One literal executable path, never shell text or space-split flags.
    // Resolve Linux paths containing a separator against the caller's cwd,
    // before the child moves into the build directory. Bare names use PATH.
    let linker =
        PathBuf::from(std::env::var_os("JETT_NATIVE_CC").unwrap_or_else(|| OsString::from("cc")));
    let linker = if linker.is_relative() && linker.as_os_str().as_encoded_bytes().contains(&b'/') {
        absolute_output_path(&linker)?
    } else {
        linker
    };
    let mut command = Command::new(&linker);
    command
        .current_dir(working_directory)
        .arg("-no-pie")
        .arg("-o")
        .arg(executable)
        .arg(object)
        .arg(&launcher.archive_path)
        .args(&launcher.native_library_args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let output = run_command_with_timeout(command, &linker, timeout)?;
    if !output.status.success() {
        return Err(NativeBuildError::LinkFailed {
            linker,
            status: output.status,
            stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        });
    }
    Ok(())
}

#[cfg(windows)]
fn link_windows_msvc(
    object_path: &Path,
    launcher: &NativeLauncherBundle,
    executable_path: &Path,
    pdb_path: &Path,
    working_directory: &Path,
    timeout: Duration,
) -> Result<(), NativeBuildError> {
    find_msvc_tools::find_windows_sdk("x86_64").ok_or(NativeBuildError::WindowsSdkNotFound {
        architecture: "x86_64",
    })?;
    let tool = find_msvc_tools::find_tool(WINDOWS_MSVC_NATIVE_TARGET, "link.exe").ok_or(
        NativeBuildError::LinkerNotFound {
            target: WINDOWS_MSVC_NATIVE_TARGET,
        },
    )?;
    let linker_path = tool.path().to_path_buf();
    let mut command = tool.to_command();
    command
        .current_dir(working_directory)
        .args(linker_arguments(
            object_path,
            launcher,
            executable_path,
            pdb_path,
        ))
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let output = run_command_with_timeout(command, &linker_path, timeout)?;
    if !output.status.success() {
        return Err(NativeBuildError::LinkFailed {
            linker: linker_path,
            status: output.status,
            stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        });
    }
    Ok(())
}

#[cfg(not(windows))]
fn link_windows_msvc(
    _object_path: &Path,
    _launcher: &NativeLauncherBundle,
    _executable_path: &Path,
    _pdb_path: &Path,
    _working_directory: &Path,
    _timeout: Duration,
) -> Result<(), NativeBuildError> {
    Err(NativeBuildError::UnsupportedHost {
        actual: jett_codegen_cranelift::host_target().to_string(),
        supported: SUPPORTED_NATIVE_HOSTS,
    })
}

#[cfg(any(windows, test))]
fn linker_arguments(
    object_path: &Path,
    launcher: &NativeLauncherBundle,
    executable_path: &Path,
    pdb_path: &Path,
) -> Vec<OsString> {
    let mut arguments = vec![
        OsString::from("/NOLOGO"),
        OsString::from("/MACHINE:X64"),
        OsString::from("/SUBSYSTEM:CONSOLE"),
        OsString::from("/INCREMENTAL:NO"),
        OsString::from("/MANIFEST:EMBED"),
        prefixed_path_argument("/OUT:", executable_path),
        prefixed_path_argument("/PDB:", pdb_path),
        object_path.as_os_str().to_owned(),
        launcher.archive_path.as_os_str().to_owned(),
    ];
    arguments.extend(launcher.native_library_args.iter().cloned());
    arguments
}

#[cfg(any(windows, test))]
fn prefixed_path_argument(prefix: &str, path: &Path) -> OsString {
    let mut argument = OsString::from(prefix);
    argument.push(path.as_os_str());
    argument
}

#[derive(Debug)]
struct CommandOutput {
    status: ExitStatus,
    stdout: Vec<u8>,
    stderr: Vec<u8>,
}

fn run_command_with_timeout(
    mut command: Command,
    program: &Path,
    timeout: Duration,
) -> Result<CommandOutput, NativeBuildError> {
    // Files do not require EOF from every descendant that inherited a handle.
    // Reopen gives writers independent offsets; capture only the exit snapshot.
    let mut stdout = capture_file("stdout")?;
    let mut stderr = capture_file("stderr")?;
    command
        .stdout(
            stdout
                .reopen()
                .map_err(|source| NativeBuildError::CaptureLinkerOutput {
                    stream: "stdout",
                    source,
                })?,
        )
        .stderr(
            stderr
                .reopen()
                .map_err(|source| NativeBuildError::CaptureLinkerOutput {
                    stream: "stderr",
                    source,
                })?,
        );
    let child = command
        .spawn()
        .map_err(|source| NativeBuildError::SpawnLinker {
            linker: program.to_path_buf(),
            source,
        })?;
    // Every return path either observes a reaped child or terminates/reaps it.
    let mut child = ManagedChild::new(child);
    let start = Instant::now();

    let (status, timed_out) = loop {
        match child.try_wait() {
            Ok(Some(status)) => break (status, false),
            Ok(None) if start.elapsed() >= timeout => {
                let status = child.terminate_and_wait(program)?;
                break (status, true);
            }
            Ok(None) => {
                let remaining = timeout.saturating_sub(start.elapsed());
                thread::sleep(remaining.min(Duration::from_millis(10)));
            }
            Err(source) => {
                return Err(NativeBuildError::PollLinker {
                    linker: program.to_path_buf(),
                    source,
                });
            }
        }
    };

    let stdout = read_output_snapshot(stdout.as_file_mut(), "stdout")?;
    let stderr = read_output_snapshot(stderr.as_file_mut(), "stderr")?;
    if timed_out {
        return Err(NativeBuildError::LinkTimedOut {
            linker: program.to_path_buf(),
            timeout,
            stdout: String::from_utf8_lossy(&stdout).into_owned(),
            stderr: String::from_utf8_lossy(&stderr).into_owned(),
        });
    }
    Ok(CommandOutput {
        status,
        stdout,
        stderr,
    })
}

struct ManagedChild {
    child: Child,
    reaped: bool,
}

impl ManagedChild {
    fn new(child: Child) -> Self {
        Self {
            child,
            reaped: false,
        }
    }

    fn try_wait(&mut self) -> io::Result<Option<ExitStatus>> {
        let status = self.child.try_wait()?;
        if status.is_some() {
            self.reaped = true;
        }
        Ok(status)
    }

    fn terminate_and_wait(&mut self, program: &Path) -> Result<ExitStatus, NativeBuildError> {
        self.child
            .kill()
            .map_err(|source| NativeBuildError::TerminateLinker {
                linker: program.to_path_buf(),
                source,
            })?;
        let status = self
            .child
            .wait()
            .map_err(|source| NativeBuildError::PollLinker {
                linker: program.to_path_buf(),
                source,
            })?;
        self.reaped = true;
        Ok(status)
    }
}

impl Drop for ManagedChild {
    fn drop(&mut self) {
        if !self.reaped {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
    }
}

fn capture_file(stream: &'static str) -> Result<tempfile::NamedTempFile, NativeBuildError> {
    tempfile::NamedTempFile::new()
        .map_err(|source| NativeBuildError::CaptureLinkerOutput { stream, source })
}

fn read_output_snapshot(
    file: &mut fs::File,
    stream: &'static str,
) -> Result<Vec<u8>, NativeBuildError> {
    let mut read = || -> io::Result<Vec<u8>> {
        let length = file.metadata()?.len();
        let mut bytes = Vec::new();
        file.take(length).read_to_end(&mut bytes)?;
        Ok(bytes)
    };
    read().map_err(|source| NativeBuildError::CaptureLinkerOutput { stream, source })
}

#[cfg(windows)]
fn publish_executable(from: &Path, to: &Path) -> Result<(), NativeBuildError> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Storage::FileSystem::{
        MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH, MoveFileExW,
    };

    let from_wide: Vec<u16> = from.as_os_str().encode_wide().chain(Some(0)).collect();
    let to_wide: Vec<u16> = to.as_os_str().encode_wide().chain(Some(0)).collect();
    // SAFETY: both buffers are live, immutable, NUL-terminated UTF-16 paths
    // for the duration of the call. The destination was validated as either
    // absent or a regular file, and REPLACE_EXISTING preserves it if the move
    // itself fails.
    let published = unsafe {
        MoveFileExW(
            from_wide.as_ptr(),
            to_wide.as_ptr(),
            MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
        )
    };
    if published == 0 {
        return Err(NativeBuildError::PublishExecutable {
            from: from.to_path_buf(),
            to: to.to_path_buf(),
            source: io::Error::last_os_error(),
        });
    }
    Ok(())
}

#[cfg(not(windows))]
fn publish_executable(from: &Path, to: &Path) -> Result<(), NativeBuildError> {
    fs::rename(from, to).map_err(|source| NativeBuildError::PublishExecutable {
        from: from.to_path_buf(),
        to: to.to_path_buf(),
        source,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unsupported_host_diagnostic_names_both_native_hosts() {
        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join("main.jett");
        fs::write(
            &source,
            "function main() returns nothing:\n    return nothing\n",
        )
        .unwrap();
        let object = emit_host_program_object_for_file(&source).unwrap();
        let error = validate_link_host_for(&object, "aarch64-unknown-linux-gnu".to_owned())
            .expect_err("unsupported hosts must not silently cross-link");
        assert!(matches!(error, NativeBuildError::UnsupportedHost { .. }));
        let diagnostic = error.to_string();
        assert!(
            diagnostic.contains("aarch64-unknown-linux-gnu"),
            "{diagnostic}"
        );
        assert!(
            diagnostic.contains(WINDOWS_MSVC_NATIVE_TARGET),
            "{diagnostic}"
        );
        assert!(diagnostic.contains(LINUX_GNU_NATIVE_TARGET), "{diagnostic}");
    }

    #[cfg(all(target_os = "linux", target_env = "gnu", target_arch = "x86_64"))]
    #[test]
    fn linux_cc_override_child() {
        let Some(directory) = std::env::var_os("JETT_TEST_CC_DIRECTORY") else {
            return;
        };
        let directory = PathBuf::from(directory);
        let result = link_linux_gnu(
            &directory.join("main.o"),
            &NativeLauncherBundle::linux_gnu_v1(directory.join("support.o")),
            &directory.join("program"),
            &directory.join("build directory"),
            Duration::from_secs(10),
        );
        if std::env::var_os("JETT_TEST_CC_MISSING").is_some() {
            assert!(
                matches!(result, Err(NativeBuildError::SpawnLinker { source, .. })
                if source.kind() == io::ErrorKind::NotFound)
            );
        } else {
            result.expect("literal compiler path must link from a different cwd");
            assert_eq!(
                Command::new(directory.join("program"))
                    .status()
                    .unwrap()
                    .code(),
                Some(42)
            );
        }
    }

    #[cfg(all(target_os = "linux", target_env = "gnu", target_arch = "x86_64"))]
    #[test]
    fn linux_cc_overrides_are_literal_and_relative_to_the_caller() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path();
        fs::create_dir(root.join("tool chain")).unwrap();
        fs::create_dir(root.join("build directory")).unwrap();
        let cc = std::env::split_paths(&std::env::var_os("PATH").unwrap())
            .map(|path| path.join("cc"))
            .find(|path| path.is_file())
            .expect("host C compiler");
        let cc = fs::canonicalize(cc).unwrap();
        let compiler = root.join("tool chain/cc literal");
        std::os::unix::fs::symlink(&cc, &compiler).unwrap();
        std::os::unix::fs::symlink(&cc, root.join("tool chain/cc")).unwrap();
        for (name, source) in [
            (
                "main",
                "extern int answer(void); int main(void) { return answer(); }",
            ),
            ("support", "int answer(void) { return 42; }"),
        ] {
            fs::write(root.join(format!("{name}.c")), source).unwrap();
            assert!(
                Command::new(&cc)
                    .arg("-c")
                    .arg(root.join(format!("{name}.c")))
                    .arg("-o")
                    .arg(root.join(format!("{name}.o")))
                    .status()
                    .unwrap()
                    .success()
            );
        }
        let path = std::env::join_paths(
            std::iter::once(root.join("tool chain"))
                .chain(std::env::split_paths(&std::env::var_os("PATH").unwrap())),
        )
        .unwrap();
        for (compiler, missing) in [
            (compiler, false),
            (PathBuf::from("cc"), false),
            (PathBuf::from("./tool chain/missing compiler"), true),
            (PathBuf::from("./tool chain/cc literal"), false),
        ] {
            let mut command = Command::new(std::env::current_exe().unwrap());
            command
                .args([
                    "--exact",
                    "native::tests::linux_cc_override_child",
                    "--nocapture",
                ])
                .current_dir(root)
                .env("JETT_NATIVE_CC", &compiler)
                .env("JETT_TEST_CC_DIRECTORY", root)
                .env("PATH", &path)
                .env_remove("JETT_TEST_CC_MISSING");
            if missing {
                command.env("JETT_TEST_CC_MISSING", "1");
            }
            let output = command.output().unwrap();
            assert!(
                output.status.success(),
                "compiler {}: {output:?}",
                compiler.display()
            );
        }
    }

    #[cfg(all(target_os = "linux", target_env = "gnu", target_arch = "x86_64"))]
    #[test]
    fn linux_launcher_contract_rejects_incompatible_metadata() {
        let canonical = NativeLauncherBundle::linux_gnu_v1("launcher.a");
        assert!(validate_host_launcher(&canonical).is_ok());
        let mut wrong = canonical.clone();
        wrong.target = WINDOWS_MSVC_NATIVE_TARGET.to_owned();
        assert!(matches!(
            validate_host_launcher(&wrong),
            Err(NativeBuildError::LauncherTargetMismatch { .. })
        ));
        wrong = canonical.clone();
        wrong.runtime_abi_version += 1;
        assert!(matches!(
            validate_host_launcher(&wrong),
            Err(NativeBuildError::LauncherRuntimeAbiMismatch { .. })
        ));
        wrong = canonical.clone();
        wrong.crt_mode = NativeCrtMode::Static;
        assert!(matches!(
            validate_host_launcher(&wrong),
            Err(NativeBuildError::LauncherCrtMismatch { .. })
        ));
        wrong = canonical;
        wrong.native_library_args.pop();
        assert!(matches!(
            validate_host_launcher(&wrong),
            Err(NativeBuildError::LauncherNativeLibrariesMismatch { .. })
        ));
    }

    #[cfg(unix)]
    #[test]
    fn command_runner_captures_large_streams_and_failure_status() {
        let stdout = "o".repeat(96 * 1024);
        let stderr = "e".repeat(96 * 1024);
        let mut command = Command::new("sh");
        command
            .args([
                "-c",
                "printf '%s' \"$1\"; printf '%s' \"$2\" >&2; exit 7",
                "sh",
                &stdout,
                &stderr,
            ])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let output =
            run_command_with_timeout(command, Path::new("sh"), Duration::from_secs(3)).unwrap();
        assert_eq!(output.status.code(), Some(7));
        assert_eq!(output.stdout, stdout.as_bytes());
        assert_eq!(output.stderr, stderr.as_bytes());
    }

    #[cfg(unix)]
    #[test]
    fn command_deadline_does_not_wait_for_inherited_output_pipes() {
        let mut command = Command::new("sh");
        command
            .args([
                "-c",
                "printf captured; printf diagnostic >&2; sleep 4 & wait",
            ])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let start = Instant::now();
        let error = run_command_with_timeout(command, Path::new("sh"), Duration::from_millis(100))
            .expect_err("shell must time out even when its child retains output handles");
        assert!(
            start.elapsed() < Duration::from_secs(2),
            "inherited pipes defeated the deadline"
        );
        match error {
            NativeBuildError::LinkTimedOut { stdout, stderr, .. } => {
                assert_eq!(stdout, "captured");
                assert_eq!(stderr, "diagnostic");
            }
            error => panic!("unexpected error: {error}"),
        }
    }

    #[cfg(unix)]
    #[test]
    fn command_runner_enforces_unix_deadline_and_reaps_child() {
        let mut command = Command::new("sleep");
        command
            .arg("10")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let start = Instant::now();
        assert!(matches!(
            run_command_with_timeout(command, Path::new("sleep"), Duration::from_millis(25)),
            Err(NativeBuildError::LinkTimedOut { .. })
        ));
        assert!(start.elapsed() < Duration::from_secs(3));
    }

    #[test]
    fn canonical_launcher_metadata_is_complete_and_static() {
        let bundle = NativeLauncherBundle::windows_msvc_static_v1("launcher.lib");

        assert_eq!(bundle.target, WINDOWS_MSVC_NATIVE_TARGET);
        assert_eq!(bundle.runtime_abi_version, NATIVE_RUNTIME_ABI_VERSION_V1);
        assert_eq!(bundle.crt_mode, NativeCrtMode::Static);
        assert_eq!(
            bundle.native_library_args,
            WINDOWS_MSVC_STATIC_V1_NATIVE_LIBRARIES
                .iter()
                .map(OsString::from)
                .collect::<Vec<_>>()
        );
        assert!(validate_launcher(&bundle).is_ok());
    }

    #[test]
    fn launcher_contract_rejects_each_incompatible_dimension() {
        let canonical = NativeLauncherBundle::windows_msvc_static_v1("launcher.lib");

        let mut target = canonical.clone();
        target.target = "aarch64-pc-windows-msvc".to_string();
        assert!(matches!(
            validate_launcher(&target),
            Err(NativeBuildError::LauncherTargetMismatch { .. })
        ));

        let mut abi = canonical.clone();
        abi.runtime_abi_version += 1;
        assert!(matches!(
            validate_launcher(&abi),
            Err(NativeBuildError::LauncherRuntimeAbiMismatch { .. })
        ));

        let mut crt = canonical.clone();
        crt.crt_mode = NativeCrtMode::Dynamic;
        assert!(matches!(
            validate_launcher(&crt),
            Err(NativeBuildError::LauncherCrtMismatch { .. })
        ));

        let mut libraries = canonical;
        libraries.native_library_args.pop();
        assert!(matches!(
            validate_launcher(&libraries),
            Err(NativeBuildError::LauncherNativeLibrariesMismatch { .. })
        ));
    }

    #[test]
    fn linker_paths_with_spaces_remain_single_arguments() {
        let bundle =
            NativeLauncherBundle::windows_msvc_static_v1(r"C:\launcher bundle\jett launcher.lib");
        let arguments = linker_arguments(
            Path::new(r"C:\build area\program.obj"),
            &bundle,
            Path::new(r"C:\build area\program.exe"),
            Path::new(r"C:\build area\program.pdb"),
        );

        assert!(arguments.contains(&OsString::from(r"C:\build area\program.obj")));
        assert!(arguments.contains(&OsString::from(r"C:\launcher bundle\jett launcher.lib")));
        assert!(arguments.contains(&OsString::from(r"/OUT:C:\build area\program.exe")));
        assert!(arguments.contains(&OsString::from(r"/PDB:C:\build area\program.pdb")));
        assert!(arguments.contains(&OsString::from("/MACHINE:X64")));
        assert!(!arguments.iter().any(|argument| argument == "cmd.exe"));
    }

    #[test]
    fn program_object_uses_the_checked_entry_and_exports_only_its_wrapper_variant() {
        let directory = tempfile::tempdir().expect("temporary source directory");
        let source_path = directory.path().join("entry.jett");
        fs::write(
            &source_path,
            "function main() returns nothing:\n    int64 answer = 42\n",
        )
        .expect("write native source");

        let program = emit_host_program_object_for_file(&source_path)
            .expect("program object should lower and emit");
        assert_eq!(
            program.symbols().last().map(String::as_str),
            Some(jett_codegen_cranelift::JETT_AOT_ENTRY_SYMBOL_V1)
        );

        let ordinary =
            emit_host_object_for_file(&source_path).expect("ordinary object should emit");
        assert!(
            !ordinary
                .symbols()
                .iter()
                .any(|symbol| symbol == jett_codegen_cranelift::JETT_AOT_ENTRY_SYMBOL_V1)
        );
    }

    #[test]
    fn program_object_does_not_invent_an_entry_for_a_verification_only_file() {
        let source_path =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/run_pass/simple.jett");

        assert!(matches!(
            emit_host_program_object_for_file(&source_path),
            Err(NativeBuildError::MissingProgramEntry { .. })
        ));
    }

    #[cfg(windows)]
    #[test]
    fn command_runner_enforces_its_deadline_and_reaps_the_process() {
        let mut command = Command::new("ping.exe");
        command
            .args(["-n", "6", "127.0.0.1"])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        let error =
            run_command_with_timeout(command, Path::new("ping.exe"), Duration::from_millis(25))
                .expect_err("the long-running child must time out");
        assert!(matches!(error, NativeBuildError::LinkTimedOut { .. }));
    }

    #[test]
    fn publication_atomically_replaces_an_existing_executable() {
        let directory = tempfile::tempdir().expect("temporary publication directory");
        let linked = directory.path().join("linked.exe");
        let output = directory.path().join("published.exe");
        fs::write(&linked, b"new executable").expect("write linked executable");
        fs::write(&output, b"old executable").expect("write existing executable");

        publish_executable(&linked, &output).expect("publish executable");

        assert_eq!(
            fs::read(&output).expect("read published executable"),
            b"new executable"
        );
        assert!(!linked.exists());
    }

    #[test]
    fn failed_publication_preserves_an_existing_executable() {
        let directory = tempfile::tempdir().expect("temporary publication directory");
        let missing = directory.path().join("missing.exe");
        let output = directory.path().join("published.exe");
        fs::write(&output, b"old executable").expect("write existing executable");

        assert!(matches!(
            publish_executable(&missing, &output),
            Err(NativeBuildError::PublishExecutable { .. })
        ));
        assert_eq!(
            fs::read(&output).expect("read preserved executable"),
            b"old executable"
        );
    }

    #[cfg(windows)]
    #[test]
    fn output_validation_rejects_a_non_executable_extension_before_linking() {
        let path = std::env::current_dir()
            .expect("current directory")
            .join("native-output.txt");

        assert!(matches!(
            validate_output_path(&path),
            Err(NativeBuildError::InvalidExtension {
                role: NativePathRole::Output,
                expected: "exe",
                ..
            })
        ));
    }
}
