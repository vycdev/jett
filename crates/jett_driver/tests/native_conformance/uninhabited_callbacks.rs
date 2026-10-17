use super::{run_bounded, suite_options::launcher_for_options};
use jett_driver::native::build_host_executable_with_options;
use jett_driver::{BuildOptions, run_file_capture_outcome};
use std::fs;

enum CallbackOutcome<'a> {
    Success(&'a str),
    RuntimeFailure { stdout: &'a str, message: &'a str },
}

fn run_case(name: &str, source_text: &str, expected_stdout: &str) {
    run_case_with_outcome(name, source_text, CallbackOutcome::Success(expected_stdout));
}

fn run_case_with_outcome(name: &str, source_text: &str, expected: CallbackOutcome<'_>) {
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join(format!("{name}.jett"));
    fs::write(&source, source_text).unwrap();
    let (reference, status, stderr) = match expected {
        CallbackOutcome::Success(stdout) => {
            let output = run_file_capture_outcome(&source)
                .unwrap_or_else(|error| panic!("{name}: checked reference fixture: {error:?}"));
            assert_eq!(output.stdout, stdout, "{name}");
            (output, 0, String::new())
        }
        CallbackOutcome::RuntimeFailure { stdout, message } => {
            let failure = run_file_capture_outcome(&source)
                .expect_err("independent owned-list operation must fail after descriptor creation");
            assert_eq!(failure.output.stdout, stdout, "{name}");
            assert_eq!(failure.message, message, "{name}");
            (failure.output, 71, format!("{message}\n"))
        }
    };
    assert!(reference.debug_events.is_empty(), "{name}: {reference:?}");
    assert!(
        reference.frontend_debug_observations.is_empty(),
        "{name}: {reference:?}"
    );
    let mut binaries = Vec::new();
    for release in [false, true] {
        let binary = directory.path().join(format!("{name}_{release}.exe"));
        let artifact = build_host_executable_with_options(
            &source,
            launcher_for_options(release),
            &binary,
            BuildOptions { release },
        )
        .unwrap_or_else(|error| panic!("{name}, release={release}: native build: {error}"));
        assert!(
            artifact.debug_observations.is_empty(),
            "{name}: {artifact:?}"
        );
        binaries.push((binary, release));
    }
    fs::remove_file(&source).unwrap();
    assert!(!source.exists());
    for (binary, release) in binaries {
        let actual = run_bounded(&binary, directory.path());
        assert_eq!(
            actual.status.code(),
            Some(status),
            "{name}, release={release}: {actual:?}"
        );
        assert_eq!(
            actual.stdout,
            reference.stdout.as_bytes(),
            "{name}, release={release}"
        );
        assert_eq!(
            actual.stderr,
            stderr.as_bytes(),
            "{name}, release={release}: {actual:?}"
        );
    }
}

#[test]
fn native_uninhabited_callbacks_direct_empty_matches_reference_without_source_in_both_profiles() {
    run_case(
        "01_inferred_empty_inline_callback",
        include_str!("uninhabited_callbacks/01_inferred_empty_inline_callback.jett"),
        "direct:0:-1\n",
    );
}

#[test]
fn native_uninhabited_callbacks_captured_empty_matches_reference_without_source_in_both_profiles() {
    run_case(
        "02_inferred_empty_captured_callback",
        include_str!("uninhabited_callbacks/02_inferred_empty_captured_callback.jett"),
        "captured:0:-1\n",
    );
}

#[test]
fn native_uninhabited_callbacks_concrete_callbacks_matches_reference_without_source_in_both_profiles()
 {
    run_case(
        "03_concrete_callback_controls",
        include_str!("uninhabited_callbacks/03_concrete_callback_controls.jett"),
        "direct:2:7:captured:3:3\n",
    );
}

#[test]
fn native_uninhabited_callbacks_04_descriptor_ownership_matches_reference_without_source_in_both_profiles()
 {
    run_case(
        "04_descriptor_ownership",
        include_str!("uninhabited_callbacks/04_descriptor_ownership.jett"),
        "owners:0:1:1\n",
    );
}

#[test]
fn native_uninhabited_callbacks_05_pending_descriptor_empty_map_matches_reference_without_source_in_both_profiles()
 {
    run_case(
        "05_pending_descriptor_empty_map",
        include_str!("uninhabited_callbacks/05_pending_descriptor_empty_map.jett"),
        "pending:0:0\n",
    );
}

#[test]
fn native_uninhabited_callbacks_06_terminal_cleanup_preserves_failure_and_cleanup_without_source_in_both_profiles()
 {
    run_case_with_outcome(
        "06_terminal_cleanup",
        include_str!("uninhabited_callbacks/06_terminal_cleanup.jett"),
        CallbackOutcome::RuntimeFailure {
            stdout: "before:0:1:5\n",
            message: "runtime error: list.__remove_at: index -1 out of bounds",
        },
    );
}

#[test]
fn native_uninhabited_callbacks_07_identity_matches_reference_without_source_in_both_profiles() {
    run_case(
        "07_uninhabited_identity_callback",
        include_str!("uninhabited_callbacks/07_uninhabited_identity_callback.jett"),
        "identity:0\n",
    );
}

#[test]
fn native_uninhabited_callbacks_08_loop_body_matches_reference_without_source_in_both_profiles() {
    run_case(
        "08_loop_body_callback",
        include_str!("uninhabited_callbacks/08_loop_body_callback.jett"),
        "loop:0:-1:1:3\n",
    );
}

#[test]
fn native_uninhabited_callbacks_09_equality_body_matches_reference_without_source_in_both_profiles()
{
    run_case(
        "09_equality_body_callback",
        include_str!("uninhabited_callbacks/09_equality_body_callback.jett"),
        "equality:0:false:1:true\n",
    );
}
