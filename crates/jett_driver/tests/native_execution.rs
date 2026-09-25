//! Production native execution, without the source tree as runtime cwd.
#![cfg(all(target_os = "linux", target_env = "gnu", target_arch = "x86_64"))]

use jett_driver::native::{NativeLauncherBundle, build_host_executable};
use std::path::Path;
use std::process::Command;
use std::sync::OnceLock;

struct Launcher {
    bundle: NativeLauncherBundle,
    _directory: tempfile::TempDir,
}

impl std::ops::Deref for Launcher {
    type Target = NativeLauncherBundle;

    fn deref(&self) -> &Self::Target {
        &self.bundle
    }
}

fn launcher() -> Launcher {
    // Cache bytes, not a path into a stale build tree or a leaked TempDir.
    static ARCHIVE: OnceLock<Vec<u8>> = OnceLock::new();
    let archive = ARCHIVE.get_or_init(|| {
        let executable = std::env::current_exe().expect("test executable");
        let profile_directory = executable.parent().unwrap().parent().unwrap();
        let profile = profile_directory.file_name().unwrap().to_str().unwrap();
        // Cargo's built-in test profile shares the debug output directory.
        let cargo_profile = if profile == "debug" { "test" } else { profile };
        let host = jett_driver::native::host_target();
        // Share one nested cache, outside the outer Cargo build lock.
        // Explicit host and profile still select the correct archive.
        let target = profile_directory
            .parent()
            .unwrap()
            .join("native-values-launcher");
        let status = Command::new(env!("CARGO"))
            .args(["build", "-q", "-p", "jett_native_launcher"])
            .args([
                "--target",
                &host,
                "--profile",
                cargo_profile,
                "--target-dir",
            ])
            .arg(&target)
            .current_dir(Path::new(env!("CARGO_MANIFEST_DIR")).join("../.."))
            .status()
            .expect("build host- and profile-matched launcher");
        assert!(status.success(), "launcher build failed: {status}");
        let archive = target
            .join(&host)
            .join(profile)
            .join("libjett_native_launcher.a");
        std::fs::read(&archive)
            .unwrap_or_else(|error| panic!("missing archive {}: {error}", archive.display()))
    });
    let directory = tempfile::tempdir().expect("launcher bundle directory");
    let path = directory.path().join("libjett_native_launcher.a");
    std::fs::write(&path, archive).expect("materialize launcher archive");
    Launcher {
        bundle: NativeLauncherBundle::linux_gnu_v1(path),
        _directory: directory,
    }
}

#[test]
fn native_scalar_entry_links_and_executes_without_source_tree() {
    let fixture =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/run_pass/native_scalar_entry.jett");
    let directory = tempfile::tempdir().expect("isolated execution directory");
    let artifact = build_host_executable(
        &fixture,
        &launcher(),
        &directory.path().join("native program"),
    )
    .expect("compile and link genuine Cranelift object with runtime launcher");
    let output = run_bounded(&artifact.path, directory.path());
    assert!(
        output.status.success(),
        "native status: {}; stderr: {}",
        output.status,
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.stdout, b"");
    assert_eq!(output.stderr, b"");
}

#[test]
fn native_actor_spawn_releases_state_with_context() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/native/actor_spawn.jett");
    let expected = jett_driver::run_file_capture_output(&fixture).unwrap();
    assert_eq!(expected.stdout, "spawned\n");
    let directory = tempfile::tempdir().unwrap();
    let binary = directory.path().join("actor_spawn");
    build_host_executable(&fixture, &launcher(), &binary).unwrap();
    let actual = run_bounded(&binary, directory.path());
    assert!(actual.status.success(), "{actual:?}");
    assert_eq!(actual.stdout, expected.stdout.as_bytes());
    assert!(actual.stderr.is_empty(), "{actual:?}");
}

#[test]
fn native_actor_messages_match_interpreter() {
    let fixture =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/native/actor_messages.jett");
    let expected = jett_driver::run_file_capture_output(&fixture).unwrap();
    assert_eq!(expected.stdout, "hits: 5\n");
    let directory = tempfile::tempdir().unwrap();
    let binary = directory.path().join("actor_messages");
    build_host_executable(&fixture, &launcher(), &binary).unwrap();
    let actual = run_bounded(&binary, directory.path());
    assert!(actual.status.success(), "{actual:?}");
    assert_eq!(actual.stdout, expected.stdout.as_bytes());
    assert!(actual.stderr.is_empty(), "{actual:?}");
}

#[test]
fn native_actor_fixture_bodies_match_interpreter() {
    for (fixture, main) in [
        (
            "actor_named_arguments.jett",
            "function main() returns nothing:\n    println(actor_results())\n",
        ),
        (
            "namespace_duplicate_leaf_actors.jett",
            "function main() returns nothing:\n    println(direct_duplicate_leaf_actors(), alias_duplicate_leaf_actors())\n",
        ),
        (
            "namespace_qualified_actors.jett",
            "function main() returns nothing:\n    println(direct_actor(), alias_actor())\n",
        ),
        (
            "numeric_literal_contexts.jett",
            "function main() returns nothing:\n    ByteCounter counter = spawn ByteCounter(seed: 4 + 5)\n    send counter.add(10 + 11)\n    println(ask counter.current)\n",
        ),
    ] {
        let source = std::fs::read_to_string(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../tests/run_pass")
                .join(fixture),
        )
        .unwrap();
        let directory = tempfile::tempdir().unwrap();
        let source_path = directory.path().join(fixture);
        std::fs::write(&source_path, format!("{source}\n{main}")).unwrap();
        let expected = jett_driver::run_file_capture_output(&source_path).unwrap();
        let binary = directory.path().join("program");
        build_host_executable(&source_path, &launcher(), &binary)
            .unwrap_or_else(|error| panic!("{fixture}: {error:?}"));
        let actual = run_bounded(&binary, directory.path());
        assert!(actual.status.success(), "{fixture}: {actual:?}");
        assert_eq!(
            actual.stdout,
            expected.stdout.as_bytes(),
            "{fixture}: {actual:?}"
        );
        assert!(actual.stderr.is_empty(), "{fixture}: {actual:?}");
    }
}

#[test]
fn native_structured_concurrency_matches_interpreter() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/native/structured_concurrency.jett");
    let expected = jett_driver::run_file_capture_output(&fixture).unwrap();
    assert_eq!(expected.stdout, "hello:ready:failed\n");
    let directory = tempfile::tempdir().unwrap();
    let binary = directory.path().join("structured_concurrency");
    build_host_executable(&fixture, &launcher(), &binary).unwrap();
    let actual = run_bounded(&binary, directory.path());
    assert!(actual.status.success(), "{actual:?}");
    assert_eq!(actual.stdout, expected.stdout.as_bytes());
    assert!(actual.stderr.is_empty(), "{actual:?}");
}

#[test]
fn native_named_function_callbacks_match_interpreter() {
    let fixture =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/native/function_values.jett");
    let expected = jett_driver::run_file_capture_output(&fixture).unwrap();
    let directory = tempfile::tempdir().unwrap();
    let binary = directory.path().join("function_values");
    build_host_executable(&fixture, &launcher(), &binary).unwrap();
    let actual = run_bounded(&binary, directory.path());
    assert!(actual.status.success(), "{actual:?}");
    assert_eq!(actual.stdout, expected.stdout.as_bytes());
    assert!(actual.stderr.is_empty());
}

#[test]
fn native_json_refinement_source_matches_interpreter() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/native/json_refinement_source.jett");
    let expected = jett_driver::run_file_capture_output(&fixture).unwrap();
    let directory = tempfile::tempdir().unwrap();
    let binary = directory.path().join("json_refinement_source");
    build_host_executable(&fixture, &launcher(), &binary).unwrap();
    let actual = run_bounded(&binary, directory.path());
    assert!(actual.status.success(), "{actual:?}");
    assert_eq!(actual.stdout, expected.stdout.as_bytes());
    assert!(actual.stderr.is_empty(), "{actual:?}");
}

#[test]
fn native_json_bytes_raw_source_matches_interpreter() {
    let fixture =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/native/json_bytes_raw_source.jett");
    let expected = jett_driver::run_file_capture_output(&fixture).unwrap();
    let directory = tempfile::tempdir().unwrap();
    let binary = directory.path().join("json_bytes_raw_source");
    build_host_executable(&fixture, &launcher(), &binary).unwrap();
    let actual = run_bounded(&binary, directory.path());
    assert!(actual.status.success(), "{actual:?}");
    assert_eq!(actual.stdout, expected.stdout.as_bytes());
    assert!(actual.stderr.is_empty(), "{actual:?}");
}

#[test]
fn native_json_bitfield_source_matches_interpreter() {
    let fixture =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/native/json_bitfield_source.jett");
    let expected = jett_driver::run_file_capture_output(&fixture).unwrap();
    let directory = tempfile::tempdir().unwrap();
    let binary = directory.path().join("json_bitfield_source");
    build_host_executable(&fixture, &launcher(), &binary).unwrap();
    let actual = run_bounded(&binary, directory.path());
    assert!(actual.status.success(), "{actual:?}");
    assert_eq!(actual.stdout, expected.stdout.as_bytes());
    assert!(actual.stderr.is_empty(), "{actual:?}");
}

#[test]
fn native_json_enum_raw_source_matches_interpreter() {
    let fixture =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/native/json_enum_raw_source.jett");
    let expected = jett_driver::run_file_capture_output(&fixture).unwrap();
    let directory = tempfile::tempdir().unwrap();
    let binary = directory.path().join("json_enum_raw_source");
    build_host_executable(&fixture, &launcher(), &binary).unwrap();
    let actual = run_bounded(&binary, directory.path());
    assert!(actual.status.success(), "{actual:?}");
    assert_eq!(actual.stdout, expected.stdout.as_bytes());
    assert!(actual.stderr.is_empty(), "{actual:?}");
}

#[test]
fn native_json_pipeline_source_matches_interpreter() {
    let fixture =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/native/json_pipeline_source.jett");
    let expected = jett_driver::run_file_capture_output(&fixture).unwrap();
    let directory = tempfile::tempdir().unwrap();
    let binary = directory.path().join("json_pipeline_source");
    build_host_executable(&fixture, &launcher(), &binary).unwrap();
    let actual = run_bounded(&binary, directory.path());
    assert!(actual.status.success(), "{actual:?}");
    assert_eq!(actual.stdout, expected.stdout.as_bytes());
    assert!(actual.stderr.is_empty(), "{actual:?}");
}

#[test]
fn native_json_refined_record_source_matches_interpreter() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/native/json_refined_record_source.jett");
    let expected = jett_driver::run_file_capture_output(&fixture).unwrap();
    let directory = tempfile::tempdir().unwrap();
    let binary = directory.path().join("json_refined_record_source");
    build_host_executable(&fixture, &launcher(), &binary).unwrap();
    let actual = run_bounded(&binary, directory.path());
    assert!(actual.status.success(), "{actual:?}");
    assert_eq!(actual.stdout, expected.stdout.as_bytes());
    assert!(actual.stderr.is_empty(), "{actual:?}");
}

#[test]
fn native_list_source_values_match_interpreter() {
    let fixture =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/native/list_source_values.jett");
    let expected = jett_driver::run_file_capture_output(&fixture).unwrap();
    let directory = tempfile::tempdir().unwrap();
    let binary = directory.path().join("list_source_values");
    build_host_executable(&fixture, &launcher(), &binary).unwrap();
    let actual = run_bounded(&binary, directory.path());
    assert!(actual.status.success(), "{actual:?}");
    assert_eq!(actual.stdout, expected.stdout.as_bytes());
    assert!(actual.stderr.is_empty());
}

struct ExecutionChild(std::process::Child);

impl Drop for ExecutionChild {
    fn drop(&mut self) {
        // Also runs on poll errors and timeout panics. No reader threads outlive
        // the child, and an already reaped Child will not signal a reused PID.
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn run_bounded(executable: &Path, directory: &Path) -> std::process::Output {
    run_bounded_with_timeout(executable, directory, std::time::Duration::from_secs(10))
}

fn run_bounded_with_timeout(
    executable: &Path,
    directory: &Path,
    timeout: std::time::Duration,
) -> std::process::Output {
    use std::io::Read;
    use std::process::Stdio;
    use std::time::{Duration, Instant};
    // A descendant retaining stdout/stderr cannot keep a pipe reader alive.
    let mut stdout = tempfile::NamedTempFile::new().unwrap();
    let mut stderr = tempfile::NamedTempFile::new().unwrap();
    let mut child = ExecutionChild(
        Command::new(executable)
            .current_dir(directory)
            .env_clear()
            .stdin(Stdio::null())
            .stdout(stdout.reopen().unwrap())
            .stderr(stderr.reopen().unwrap())
            .spawn()
            .expect("execute native artifact"),
    );
    let start = Instant::now();
    let status = loop {
        if let Some(status) = child.0.try_wait().expect("poll native artifact") {
            break status;
        }
        assert!(
            start.elapsed() < timeout,
            "native executable exceeded {timeout:?} deadline"
        );
        std::thread::sleep(Duration::from_millis(10));
    };
    let snapshot = |file: &mut std::fs::File| {
        let length = file.metadata().unwrap().len();
        let mut bytes = Vec::new();
        file.take(length).read_to_end(&mut bytes).unwrap();
        bytes
    };
    std::process::Output {
        status,
        stdout: snapshot(stdout.as_file_mut()),
        stderr: snapshot(stderr.as_file_mut()),
    }
}

#[test]
fn native_explicit_comptime_is_baked_before_runtime_codegen() {
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("constant.jett");
    std::fs::write(&source, "function main() returns nothing:\n    int64 baked = comptime math.factorial(5)\n    while baked != 120:\n        int64 unexpected = 0\n    return nothing\n").unwrap();
    let binary = directory.path().join("constant");
    build_host_executable(&source, &launcher(), &binary)
        .expect("bake pure stdlib computation and link");
    std::fs::remove_file(&source).unwrap();
    let output = run_bounded(&binary, directory.path());
    assert!(output.status.success(), "{output:?}");
    assert!(output.stdout.is_empty());
    assert!(output.stderr.is_empty());
}

#[test]
fn native_uint32_wrapping_contract_matches_interpreter() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/runtime_fail/uint32_multiplication_overflow.jett");
    let expected = jett_driver::run_file_capture_output(&fixture).unwrap();
    let directory = tempfile::tempdir().unwrap();
    let binary = directory.path().join("wrapping");
    build_host_executable(&fixture, &launcher(), &binary).unwrap();
    let actual = run_bounded(&binary, directory.path());
    assert!(actual.status.success());
    assert_eq!(actual.stdout, expected.stdout.as_bytes());
    assert!(actual.stderr.is_empty());
    assert!(expected.debug_output.is_empty());

    // The original fixture has no observable arithmetic result. Supplement it
    // with a runtime branch so a non-wrapping result cannot pass merely by exit.
    let source = directory.path().join("observe.jett");
    std::fs::write(&source, "function main() returns nothing:\n    uint32 maximum = 4294967295\n    uint32 wrapped = maximum * maximum\n    while wrapped != 1:\n        int64 unexpected = 0\n    return nothing\n").unwrap();
    build_host_executable(&source, &launcher(), &binary).unwrap();
    std::fs::remove_file(source).unwrap();
    assert!(run_bounded(&binary, directory.path()).status.success());
}

#[test]
fn native_harness_does_not_wait_for_inherited_output_pipes() {
    use std::os::unix::fs::PermissionsExt;
    let directory = tempfile::tempdir().unwrap();
    let executable = directory.path().join("detached-output");
    std::fs::write(
        &executable,
        "#!/bin/sh\n/bin/sleep 4 &\nprintf captured\nprintf diagnostic >&2\n",
    )
    .unwrap();
    std::fs::set_permissions(&executable, std::fs::Permissions::from_mode(0o755)).unwrap();
    let start = std::time::Instant::now();
    let output = run_bounded(&executable, directory.path());
    assert!(
        start.elapsed() < std::time::Duration::from_secs(2),
        "inherited pipes defeated the deadline"
    );
    assert!(output.status.success());
    assert_eq!(output.stdout, b"captured");
    assert_eq!(output.stderr, b"diagnostic");
}

#[test]
fn native_harness_timeout_reaps_the_direct_child() {
    use std::os::unix::fs::PermissionsExt;
    use std::time::{Duration, Instant};
    let directory = tempfile::tempdir().unwrap();
    let executable = directory.path().join("timeout");
    std::fs::write(
        &executable,
        "#!/bin/sh\nprintf '%s' \"$$\" > child.pid\nexec /bin/sleep 10\n",
    )
    .unwrap();
    std::fs::set_permissions(&executable, std::fs::Permissions::from_mode(0o755)).unwrap();
    let start = Instant::now();
    let result = std::panic::catch_unwind(|| {
        run_bounded_with_timeout(&executable, directory.path(), Duration::from_millis(100))
    });
    assert!(result.is_err(), "stuck native child must time out");
    assert!(start.elapsed() < Duration::from_secs(2));
    let pid = std::fs::read_to_string(directory.path().join("child.pid")).unwrap();
    assert!(
        !Path::new("/proc").join(pid.trim()).exists(),
        "child must be reaped"
    );
}
