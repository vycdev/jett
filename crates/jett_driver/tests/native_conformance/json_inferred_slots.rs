use super::{run_bounded, suite_options::launcher_for_options};
use jett_driver::native::{
    NativeLauncherBundle, build_host_executable_with_options,
    build_host_property_suite_executable_with_options,
    build_host_verify_suite_executable_with_options,
};
use jett_driver::{BuildOptions, run_file_capture_outcome};
use std::fs;

#[derive(Clone, Copy)]
enum ExpectedOutcome {
    Success(&'static str),
    PureSuites(&'static str),
    RuntimeFailure {
        stdout: &'static str,
        message: &'static str,
    },
}

fn run_case(name: &str, source_text: &str, expected: ExpectedOutcome) {
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join(format!("{name}.jett"));
    fs::write(&source, source_text).unwrap();
    let (reference, stdout, status, stderr) = match expected {
        ExpectedOutcome::Success(stdout) | ExpectedOutcome::PureSuites(stdout) => {
            let output = run_file_capture_outcome(&source)
                .unwrap_or_else(|error| panic!("{name}: checked reference fixture: {error:?}"));
            assert_eq!(output.stdout, stdout, "{name}");
            (output, stdout, 0, String::new())
        }
        ExpectedOutcome::RuntimeFailure { stdout, message } => {
            let failure = run_file_capture_outcome(&source)
                .expect_err("pending carrier must fail before producing JSON");
            assert_eq!(failure.output.stdout, stdout, "{name}");
            assert_eq!(failure.message, message, "{name}");
            (failure.output, stdout, 71, format!("{message}\n"))
        }
    };
    assert!(reference.debug_events.is_empty(), "{name}: {reference:?}");
    assert!(
        reference.frontend_debug_observations.is_empty(),
        "{name}: {reference:?}"
    );
    if matches!(expected, ExpectedOutcome::PureSuites(_)) {
        let checked = jett_driver::test_file_capture_outcome(&source)
            .unwrap_or_else(|error| panic!("{name}: checked reference suites: {error:?}"));
        assert_eq!((checked.total, checked.passed, checked.failed), (2, 2, 0));
        assert_eq!(checked.blocks.len(), 2);
        assert!(!checked.blocks[0].is_property);
        assert!(checked.blocks[1].is_property);
        assert_eq!(checked.blocks[1].iterations, Some(100));
        assert!(checked.debug_observations.is_empty(), "{name}: {checked:?}");
        assert!(
            checked
                .blocks
                .iter()
                .all(|block| block.debug_events.is_empty()),
            "{name}: {checked:?}"
        );
    }
    let mut binaries = Vec::new();
    let mut suite_binaries = Vec::new();
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
        if matches!(expected, ExpectedOutcome::PureSuites(_)) {
            let verify_binary = directory
                .path()
                .join(format!("{name}_verify_{release}.exe"));
            let verify = build_host_verify_suite_executable_with_options(
                &source,
                launcher_for_options(release),
                &verify_binary,
                BuildOptions { release },
            )
            .unwrap_or_else(|error| panic!("{name}, release={release}: verify build: {error}"));
            assert!(verify.debug_observations.is_empty(), "{name}: {verify:?}");
            suite_binaries.push((verify_binary, release));
            let property_binary = directory
                .path()
                .join(format!("{name}_property_{release}.exe"));
            let property = build_host_property_suite_executable_with_options(
                &source,
                launcher_for_options(release),
                &property_binary,
                BuildOptions { release },
            )
            .unwrap_or_else(|error| panic!("{name}, release={release}: property build: {error}"));
            assert!(
                property.debug_observations.is_empty(),
                "{name}: {property:?}"
            );
            suite_binaries.push((property_binary, release));
        }
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
            stdout.as_bytes(),
            "{name}, release={release}"
        );
        assert_eq!(
            actual.stderr,
            stderr.as_bytes(),
            "{name}, release={release}"
        );
    }
    for (binary, release) in suite_binaries {
        let actual = run_bounded(&binary, directory.path());
        assert_eq!(
            actual.status.code(),
            Some(0),
            "{name}, release={release}: {actual:?}"
        );
        assert!(
            actual.stdout.is_empty(),
            "{name}, release={release}: {actual:?}"
        );
        assert!(
            actual.stderr.is_empty(),
            "{name}, release={release}: {actual:?}"
        );
    }
}

#[test]
fn native_json_inferred_slots_empty_list_matches_reference_without_source_in_both_profiles() {
    run_case(
        "empty_list",
        include_str!("json_inferred_slots/inferred_empty_json.jett"),
        ExpectedOutcome::Success("[]\n"),
    );
}

#[test]
fn native_json_inferred_slots_empty_list_public_matches_reference_without_source_in_both_profiles()
{
    run_case(
        "empty_list_public",
        include_str!("json_inferred_slots/inferred_empty_json_public.jett"),
        ExpectedOutcome::Success("[]\n"),
    );
}

#[test]
fn native_json_inferred_slots_typed_empty_list_control_matches_reference_without_source_in_both_profiles()
 {
    run_case(
        "typed_empty_list_control",
        include_str!("json_inferred_slots/typed_empty_json_control.jett"),
        ExpectedOutcome::Success("[]\n"),
    );
}

#[test]
fn native_json_inferred_slots_absent_optional_matches_reference_without_source_in_both_profiles() {
    run_case(
        "absent_optional",
        include_str!("json_inferred_slots/01_absent_optional.jett"),
        ExpectedOutcome::Success("null\n"),
    );
}

#[test]
fn native_json_inferred_slots_result_missing_error_matches_reference_without_source_in_both_profiles()
 {
    run_case(
        "result_missing_error",
        include_str!("json_inferred_slots/02_result_missing_error.jett"),
        ExpectedOutcome::Success("{\"ok\":7}\n"),
    );
}

#[test]
fn native_json_inferred_slots_result_missing_success_matches_reference_without_source_in_both_profiles()
 {
    run_case(
        "result_missing_success",
        include_str!("json_inferred_slots/03_result_missing_success.jett"),
        ExpectedOutcome::Success("{\"fail\":\"bad\"}\n"),
    );
}

#[test]
fn native_json_inferred_slots_string_key_empty_map_matches_reference_without_source_in_both_profiles()
 {
    run_case(
        "string_key_empty_map",
        include_str!("json_inferred_slots/04_string_key_empty_map.jett"),
        ExpectedOutcome::Success("{}\n"),
    );
}

#[test]
fn native_json_inferred_slots_generic_record_empty_list_matches_reference_without_source_in_both_profiles()
 {
    run_case(
        "generic_record_empty_list",
        include_str!("json_inferred_slots/05_generic_record_empty_list.jett"),
        ExpectedOutcome::Success("{\"items\":[]}\n"),
    );
}

#[test]
fn native_json_inferred_slots_nested_empty_list_matches_reference_without_source_in_both_profiles()
{
    run_case(
        "nested_empty_list",
        include_str!("json_inferred_slots/06_nested_empty_list.jett"),
        ExpectedOutcome::Success("[[]]\n"),
    );
}

#[test]
fn native_json_inferred_slots_pending_empty_list_matches_reference_without_source_in_both_profiles()
{
    run_case(
        "pending_empty_list",
        include_str!("json_inferred_slots/07_pending_empty_list.jett"),
        ExpectedOutcome::RuntimeFailure {
            stdout: "before\n",
            message: "runtime error: for loop requires a list, string, map, or set value",
        },
    );
}

#[test]
fn native_json_inferred_slots_pure_comptime_and_compiled_suites_match_reference() {
    run_case(
        "pure_facade_suites",
        include_str!("json_inferred_slots/01_pure_facade_suites.jett"),
        ExpectedOutcome::PureSuites("[]:[]:[]:[]\n"),
    );
}

#[test]
fn native_json_inferred_slots_public_secret_field_projection_matches_reference() {
    run_case(
        "public_secret_field_projection",
        include_str!("json_inferred_slots/05_public_secret_field_projection.jett"),
        ExpectedOutcome::Success("{\"items\":[]}\n"),
    );
}

#[test]
fn native_json_inferred_slots_policy_and_source_bodies_fail_before_publication() {
    let cases = [
        (
            "inhabited_function_empty_list",
            include_str!("json_inferred_slots/02_inhabited_function_empty_list.jett"),
            347,
        ),
        (
            "public_inhabited_function_empty_list",
            include_str!("json_inferred_slots/03_public_inhabited_function_empty_list.jett"),
            347,
        ),
        (
            "public_empty_secret_slot",
            include_str!("json_inferred_slots/04_public_empty_secret_slot.jett"),
            603,
        ),
        (
            "invalid_uninhabited_source_body",
            include_str!("json_inferred_slots/06_invalid_uninhabited_source_body.jett"),
            311,
        ),
    ];
    for (name, source_text, expected_code) in cases {
        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join(format!("{name}.jett"));
        fs::write(&source, source_text).unwrap();
        let launcher = if cfg!(windows) {
            NativeLauncherBundle::windows_msvc_static_v1(directory.path().join("unused.lib"))
        } else {
            NativeLauncherBundle::linux_gnu_v1(directory.path().join("unused.a"))
        };
        assert!(!launcher.archive_path.exists());
        let output = directory.path().join("preserved.exe");
        let sentinel = b"existing inferred JSON publication";
        fs::write(&output, sentinel).unwrap();
        for release in [false, true] {
            let error = build_host_executable_with_options(
                &source,
                &launcher,
                &output,
                BuildOptions { release },
            )
            .expect_err("JSON policy or invalid source body must fail before archive lookup");
            let build = error
                .build_result()
                .expect("typed frontend failure before publication");
            assert!(build.has_errors, "{name}, release={release}");
            let errors = build
                .diagnostics
                .iter()
                .filter(|diagnostic| diagnostic.severity == jett_diagnostics::Severity::Error)
                .collect::<Vec<_>>();
            assert_eq!(errors.len(), 1, "{name}, release={release}: {errors:?}");
            assert_eq!(
                errors[0].code.code(),
                expected_code,
                "{name}, release={release}"
            );
            assert!(
                error.debug_observations().is_empty(),
                "{name}, release={release}"
            );
            assert!(
                build.debug_observations.is_empty(),
                "{name}, release={release}"
            );
            assert!(!launcher.archive_path.exists());
            assert_eq!(
                fs::read(&output).unwrap(),
                sentinel,
                "{name}, release={release}"
            );
        }
    }
}

#[test]
fn native_json_inferred_slots_projected_alias_and_cloned_owner_remain_readable() {
    run_case(
        "owner_rereads",
        include_str!("json_inferred_slots/owner_rereads.jett"),
        ExpectedOutcome::Success("[]:[]:{\"items\":[]}:0:0:0\n"),
    );
}
