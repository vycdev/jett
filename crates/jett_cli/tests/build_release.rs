use std::path::PathBuf;
use std::process::Command;

fn workspace_fixture(relative: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join(relative)
}

#[test]
fn build_release_rejects_debug_printing() {
    let fixture = workspace_fixture("tests/run_pass/stdlib_loading.jett");

    let debug = Command::new(env!("CARGO_BIN_EXE_jett"))
        .arg("build")
        .arg(&fixture)
        .output()
        .expect("debug build command should run");
    assert!(
        debug.status.success(),
        "non-release build should accept println:\n{}",
        String::from_utf8_lossy(&debug.stderr)
    );

    let release = Command::new(env!("CARGO_BIN_EXE_jett"))
        .arg("build")
        .arg("--release")
        .arg(&fixture)
        .output()
        .expect("release build command should run");
    assert!(
        !release.status.success(),
        "release build should reject println"
    );

    let stderr = String::from_utf8_lossy(&release.stderr);
    assert!(
        stderr.contains("error[E0362]"),
        "unexpected stderr:\n{stderr}"
    );
    assert!(stderr.contains("`Stdout.write` for application output"));
    assert!(stderr.contains("`trace` / `breakpoint` for structured debugging"));
}
