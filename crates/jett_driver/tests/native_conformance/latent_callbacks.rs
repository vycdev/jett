use super::{run_bounded, suite_options::launcher_for_options};
use jett_driver::native::build_host_executable_with_options;
use jett_driver::{BuildOptions, run_file_capture_outcome};
use std::fs;

fn run_case(name: &str, source_text: &str, expected_stdout: &str) {
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join(format!("{name}.jett"));
    fs::write(&source, source_text).unwrap();
    let reference = run_file_capture_outcome(&source)
        .unwrap_or_else(|error| panic!("{name}: checked reference fixture: {error:?}"));
    assert_eq!(reference.stdout, expected_stdout, "{name}");
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
            Some(0),
            "{name}, release={release}: {actual:?}"
        );
        assert_eq!(
            actual.stdout,
            expected_stdout.as_bytes(),
            "{name}, release={release}"
        );
        assert!(
            actual.stderr.is_empty(),
            "{name}, release={release}: {actual:?}"
        );
    }
}

#[test]
fn native_latent_callbacks_nested_zero_argument_callback_matches_reference_without_source_in_both_profiles()
 {
    run_case(
        "01_nested_zero_argument_callback",
        include_str!("latent_callbacks/01_nested_zero_argument_callback.jett"),
        "nested:0:1:7\n",
    );
}

#[test]
fn native_latent_callbacks_secret_result_descriptor_matches_reference_without_source_in_both_profiles()
 {
    run_case(
        "02_secret_result_callback",
        include_str!("latent_callbacks/02_secret_result_callback.jett"),
        "secret:1:7:1\n",
    );
}

#[test]
fn native_latent_callbacks_concrete_control_matches_reference_without_source_in_both_profiles() {
    run_case(
        "03_concrete_control",
        include_str!("latent_callbacks/03_concrete_control.jett"),
        "control:1:7:9\n",
    );
}
