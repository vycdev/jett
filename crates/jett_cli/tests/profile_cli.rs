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
