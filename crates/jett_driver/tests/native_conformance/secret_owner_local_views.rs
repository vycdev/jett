use super::{run_bounded, suite_options::launcher_for_options};
use jett_driver::native::build_host_executable_with_options;
use jett_driver::{BuildOptions, run_file_capture_outcome};
use std::fs;

enum SecretOutcome<'a> {
    Success(&'a str),
    RuntimeFailure {
        stdout: &'a str,
        reference_message: &'a str,
        native_message: &'a str,
    },
}

fn run_case(name: &str, source_text: &str, expected_stdout: &str) {
    run_case_with_outcome(name, source_text, SecretOutcome::Success(expected_stdout));
}

fn run_case_with_outcome(name: &str, source_text: &str, expected: SecretOutcome<'_>) {
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join(format!("{name}.jett"));
    fs::write(&source, source_text).unwrap();
    let (reference, status, stderr) = match expected {
        SecretOutcome::Success(stdout) => {
            let output = run_file_capture_outcome(&source)
                .unwrap_or_else(|error| panic!("{name}: checked reference fixture: {error:?}"));
            assert_eq!(output.stdout, stdout, "{name}");
            (output, 0, String::new())
        }
        SecretOutcome::RuntimeFailure {
            stdout,
            reference_message,
            native_message,
        } => {
            let failure = run_file_capture_outcome(&source)
                .expect_err("expected terminal secret receiver fixture failure");
            assert_eq!(failure.output.stdout, stdout, "{name}");
            assert_eq!(failure.message, reference_message, "{name}");
            (failure.output, 71, format!("{native_message}\n"))
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
fn native_secret_owner_local_views_01_secret_owner_direct_alias_matches_reference_without_source_in_both_profiles()
 {
    run_case(
        "01_secret_owner_direct_alias",
        include_str!("secret_owner_local_views/01_secret_owner_direct_alias.jett"),
        "direct:4142:2:4142\n",
    );
}

#[test]
fn native_secret_owner_local_views_02_secret_owner_nested_forwarded_alias_matches_reference_without_source_in_both_profiles()
 {
    run_case(
        "02_secret_owner_nested_forwarded_alias",
        include_str!("secret_owner_local_views/02_secret_owner_nested_forwarded_alias.jett"),
        "nested:4344:2:4344\n",
    );
}

#[test]
fn native_secret_owner_local_views_03_secret_owner_owned_control_matches_reference_without_source_in_both_profiles()
 {
    run_case(
        "03_secret_owner_owned_control",
        include_str!("secret_owner_local_views/03_secret_owner_owned_control.jett"),
        "owned:4344:2:4344\n",
    );
}

#[test]
fn native_secret_owner_local_views_04_secret_bitfield_payload_matches_reference_without_source_in_both_profiles()
 {
    run_case(
        "04_secret_bitfield_payload",
        include_str!("secret_owner_local_views/04_secret_bitfield_payload.jett"),
        "bitfield:2:3:2:4007ff\n",
    );
}

#[test]
fn native_secret_owner_local_views_05_secret_machine_owned_root_matches_reference_without_source_in_both_profiles()
 {
    run_case(
        "05_secret_machine_owned_root",
        include_str!("secret_owner_local_views/05_secret_machine_owned_root.jett"),
        "owned:4142:3:4142:2\nowner:4142:2\n",
    );
}

#[test]
fn native_secret_owner_local_views_06_secret_machine_view_parameter_matches_reference_without_source_in_both_profiles()
 {
    run_case(
        "06_secret_machine_view_parameter",
        include_str!("secret_owner_local_views/06_secret_machine_view_parameter.jett"),
        "view:4344:3:4344:2\nagain:4344:3:4344:2\nowner:4344:2\n",
    );
}

#[test]
fn native_secret_owner_local_views_07_pending_secret_struct_receiver_preserves_native_redaction_without_source_in_both_profiles()
 {
    run_case_with_outcome(
        "07_pending_secret_struct_receiver",
        include_str!("secret_owner_local_views/07_pending_secret_struct_receiver.jett"),
        SecretOutcome::RuntimeFailure {
            stdout: "before:secret\n",
            reference_message: "runtime error: field access is not supported on pending(pending(app.Packet(payload: app.Payload(data: bytes(65, 66)))))",
            native_message: "runtime error: field access is not supported on [redacted]",
        },
    );
}
