//! Production native execution, without the source tree as runtime cwd.
#![cfg(all(target_os = "linux", target_env = "gnu", target_arch = "x86_64"))]

use jett_driver::native::{NativeLauncherBundle, build_host_executable};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;

fn launcher() -> NativeLauncherBundle {
    static ARCHIVE: OnceLock<PathBuf> = OnceLock::new();
    let archive = ARCHIVE.get_or_init(|| {
        let executable = std::env::current_exe().expect("test executable");
        let debug = executable.parent().unwrap().parent().unwrap();
        let target = debug.parent().unwrap();
        let status = Command::new(env!("CARGO"))
            .args(["build", "-q", "-p", "jett_native_launcher", "--target-dir"])
            .arg(target)
            .current_dir(Path::new(env!("CARGO_MANIFEST_DIR")).join("../.."))
            .status()
            .expect("build target-matched launcher");
        assert!(status.success(), "launcher build failed: {status}");
        let archive = debug.join("libjett_native_launcher.a");
        assert!(archive.is_file(), "missing archive: {}", archive.display());
        archive
    });
    NativeLauncherBundle::linux_gnu_v1(archive.clone())
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

fn run_bounded(executable: &Path, directory: &Path) -> std::process::Output {
    use std::process::Stdio;
    use std::time::{Duration, Instant};
    let mut child = Command::new(executable)
        .current_dir(directory)
        .env_clear()
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("execute native artifact");
    // Drain concurrently so output larger than a pipe does not deadlock.
    use std::io::Read;
    let mut stdout = child.stdout.take().unwrap();
    let mut stderr = child.stderr.take().unwrap();
    let out = std::thread::spawn(move || {
        let mut b = Vec::new();
        stdout.read_to_end(&mut b).unwrap();
        b
    });
    let err = std::thread::spawn(move || {
        let mut b = Vec::new();
        stderr.read_to_end(&mut b).unwrap();
        b
    });
    let start = Instant::now();
    let status = loop {
        if let Some(status) = child.try_wait().expect("poll native artifact") {
            break status;
        }
        if start.elapsed() > Duration::from_secs(10) {
            child.kill().expect("terminate stuck native artifact");
            child.wait().expect("reap stuck native artifact");
            panic!("native executable exceeded 10 second deadline");
        }
        std::thread::sleep(Duration::from_millis(10));
    };
    std::process::Output {
        status,
        stdout: out.join().unwrap(),
        stderr: err.join().unwrap(),
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
