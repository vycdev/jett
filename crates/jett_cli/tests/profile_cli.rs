use std::path::PathBuf;
use std::process::Command;

fn workspace_fixture(relative: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join(relative)
}

fn run_profile(args: &[&str]) -> std::process::Output {
    let fixture = workspace_fixture("tests/run_pass/stdlib_loading.jett");
    let mut command = Command::new(env!("CARGO_BIN_EXE_jett"));
    command.arg("run").arg(fixture);
    command.args(args);
    command.output().expect("profile command should run")
}

#[test]
fn profile_modes_fail_before_execution_when_runtime_support_is_unavailable() {
    for flag in ["--profile", "--profile-memory"] {
        let output = run_profile(&[flag]);

        assert!(!output.status.success(), "{flag} should fail setup");
        assert!(
            output.stdout.is_empty(),
            "program must not execute for {flag}"
        );
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            stderr.contains("profiler: backend unsupported"),
            "unexpected stderr for {flag}: {stderr}"
        );
    }
}

#[test]
fn profile_options_enforce_the_contract_at_the_cli_boundary() {
    let valid = run_profile(&[
        "--profile",
        "--profile-threshold",
        "12.34",
        "--profile-limit",
        "25",
        "--profile-rate",
        "250",
    ]);
    assert!(!valid.status.success());
    assert!(String::from_utf8_lossy(&valid.stderr).contains("profiler: backend unsupported"));

    for args in [
        vec!["--profile", "--profile-memory"],
        vec!["--profile-threshold", "5"],
        vec!["--profile", "--profile-threshold", "1.234"],
        vec!["--profile", "--profile-threshold", "100.01"],
        vec!["--profile", "--profile-limit", "0"],
        vec!["--profile", "--profile-limit", "101"],
        vec!["--profile", "--profile-rate", "0"],
        vec!["--profile", "--profile-rate", "1001"],
        vec!["--profile-memory", "--profile-rate", "100"],
    ] {
        let output = run_profile(&args);
        assert!(
            !output.status.success(),
            "invalid profile arguments were accepted: {args:?}"
        );
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            !stderr.contains("profiler: backend unsupported"),
            "invalid arguments reached profiler setup: {args:?}: {stderr}"
        );
    }
}

#[test]
fn agent_profile_setup_failure_uses_the_run_error_envelope() {
    let output = run_profile(&["--profile", "--agent"]);
    assert!(!output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.starts_with("status: error\n"), "{stdout}");
    assert!(
        stdout.contains("error: profiler: backend unsupported\n"),
        "{stdout}"
    );
    assert!(output.stderr.is_empty());
}

#[test]
fn invalid_source_takes_precedence_over_profiler_setup() {
    let missing = workspace_fixture("tests/missing-profile-source.jett");
    for agent in [false, true] {
        let mut command = Command::new(env!("CARGO_BIN_EXE_jett"));
        command.arg("run").arg(&missing).arg("--profile");
        if agent {
            command.arg("--agent");
        }
        let output = command.output().unwrap();
        assert!(!output.status.success());
        let report = format!(
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(
            !report.contains("profiler: backend unsupported"),
            "{report}"
        );
        assert!(report.contains("missing-profile-source.jett"), "{report}");
    }
}
