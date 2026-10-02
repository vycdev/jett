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
fn native_json_inferred_parse_slots_parse_empty_list_matches_reference_without_source_in_both_profiles()
 {
    run_case(
        "parse_empty_list",
        include_str!("json_inferred_parse_slots/parse_empty_list.jett"),
        "valid:length:0\noccupied:decode-error:0: unsupported reflected JSON type: <never>\nmalformed:decode-error:unterminated JSON array\n",
    );
}

#[test]
fn native_json_inferred_parse_slots_parse_absent_optional_matches_reference_without_source_in_both_profiles()
 {
    run_case(
        "parse_absent_optional",
        include_str!("json_inferred_parse_slots/parse_absent_optional.jett"),
        "valid:none\noccupied:decode-error:unsupported reflected JSON type: <never>\nmalformed:decode-error:unterminated JSON array\n",
    );
}

#[test]
fn native_json_inferred_parse_slots_parse_result_missing_error_matches_reference_without_source_in_both_profiles()
 {
    run_case(
        "parse_result_missing_error",
        include_str!("json_inferred_parse_slots/parse_result_missing_error.jett"),
        "valid:success:int64\noccupied:decode-error:fail: unsupported reflected JSON type: <never>\nmalformed:decode-error:expected result object with exactly one key\n",
    );
}

#[test]
fn native_json_inferred_parse_slots_parse_result_missing_success_matches_reference_without_source_in_both_profiles()
 {
    run_case(
        "parse_result_missing_success",
        include_str!("json_inferred_parse_slots/parse_result_missing_success.jett"),
        "valid:failure:string\noccupied:decode-error:ok: unsupported reflected JSON type: <never>\nmalformed:decode-error:expected result object with exactly one key\n",
    );
}

#[test]
fn native_json_inferred_parse_slots_parse_exact_empty_list_matches_reference_without_source_in_both_profiles()
 {
    run_case(
        "parse_exact_empty_list",
        include_str!("json_inferred_parse_slots/parse_exact_empty_list.jett"),
        "valid:length:0\noccupied:decode-error:0: unsupported reflected JSON type: <never>\nmalformed:decode-error:unterminated JSON array\n",
    );
}

#[test]
fn native_json_inferred_parse_slots_parse_exact_absent_optional_matches_reference_without_source_in_both_profiles()
 {
    run_case(
        "parse_exact_absent_optional",
        include_str!("json_inferred_parse_slots/parse_exact_absent_optional.jett"),
        "valid:none\noccupied:decode-error:unsupported reflected JSON type: <never>\nmalformed:decode-error:unterminated JSON array\n",
    );
}

#[test]
fn native_json_inferred_parse_slots_parse_exact_result_missing_error_matches_reference_without_source_in_both_profiles()
 {
    run_case(
        "parse_exact_result_missing_error",
        include_str!("json_inferred_parse_slots/parse_exact_result_missing_error.jett"),
        "valid:success:int64\noccupied:decode-error:fail: unsupported reflected JSON type: <never>\nmalformed:decode-error:expected result object with exactly one key\n",
    );
}

#[test]
fn native_json_inferred_parse_slots_parse_exact_result_missing_success_matches_reference_without_source_in_both_profiles()
 {
    run_case(
        "parse_exact_result_missing_success",
        include_str!("json_inferred_parse_slots/parse_exact_result_missing_success.jett"),
        "valid:failure:string\noccupied:decode-error:ok: unsupported reflected JSON type: <never>\nmalformed:decode-error:expected result object with exactly one key\n",
    );
}

#[test]
fn native_json_inferred_parse_slots_pipeline_parse_empty_list_matches_reference_without_source_in_both_profiles()
 {
    run_case(
        "pipeline_parse_empty_list",
        include_str!("json_inferred_parse_slots/pipeline_parse_empty_list.jett"),
        "valid:length:0\noccupied:decode-error:0: unsupported reflected JSON type: <never>\nmalformed:decode-error:unterminated JSON array\n",
    );
}

#[test]
fn native_json_inferred_parse_slots_pipeline_parse_absent_optional_matches_reference_without_source_in_both_profiles()
 {
    run_case(
        "pipeline_parse_absent_optional",
        include_str!("json_inferred_parse_slots/pipeline_parse_absent_optional.jett"),
        "valid:none\noccupied:decode-error:unsupported reflected JSON type: <never>\nmalformed:decode-error:unterminated JSON array\n",
    );
}

#[test]
fn native_json_inferred_parse_slots_pipeline_parse_result_missing_error_matches_reference_without_source_in_both_profiles()
 {
    run_case(
        "pipeline_parse_result_missing_error",
        include_str!("json_inferred_parse_slots/pipeline_parse_result_missing_error.jett"),
        "valid:success:int64\noccupied:decode-error:fail: unsupported reflected JSON type: <never>\nmalformed:decode-error:expected result object with exactly one key\n",
    );
}

#[test]
fn native_json_inferred_parse_slots_pipeline_parse_result_missing_success_matches_reference_without_source_in_both_profiles()
 {
    run_case(
        "pipeline_parse_result_missing_success",
        include_str!("json_inferred_parse_slots/pipeline_parse_result_missing_success.jett"),
        "valid:failure:string\noccupied:decode-error:ok: unsupported reflected JSON type: <never>\nmalformed:decode-error:expected result object with exactly one key\n",
    );
}

#[test]
fn native_json_inferred_parse_slots_pipeline_parse_exact_empty_list_matches_reference_without_source_in_both_profiles()
 {
    run_case(
        "pipeline_parse_exact_empty_list",
        include_str!("json_inferred_parse_slots/pipeline_parse_exact_empty_list.jett"),
        "valid:length:0\noccupied:decode-error:0: unsupported reflected JSON type: <never>\nmalformed:decode-error:unterminated JSON array\n",
    );
}

#[test]
fn native_json_inferred_parse_slots_pipeline_parse_exact_absent_optional_matches_reference_without_source_in_both_profiles()
 {
    run_case(
        "pipeline_parse_exact_absent_optional",
        include_str!("json_inferred_parse_slots/pipeline_parse_exact_absent_optional.jett"),
        "valid:none\noccupied:decode-error:unsupported reflected JSON type: <never>\nmalformed:decode-error:unterminated JSON array\n",
    );
}

#[test]
fn native_json_inferred_parse_slots_pipeline_parse_exact_result_missing_error_matches_reference_without_source_in_both_profiles()
 {
    run_case(
        "pipeline_parse_exact_result_missing_error",
        include_str!("json_inferred_parse_slots/pipeline_parse_exact_result_missing_error.jett"),
        "valid:success:int64\noccupied:decode-error:fail: unsupported reflected JSON type: <never>\nmalformed:decode-error:expected result object with exactly one key\n",
    );
}

#[test]
fn native_json_inferred_parse_slots_pipeline_parse_exact_result_missing_success_matches_reference_without_source_in_both_profiles()
 {
    run_case(
        "pipeline_parse_exact_result_missing_success",
        include_str!("json_inferred_parse_slots/pipeline_parse_exact_result_missing_success.jett"),
        "valid:failure:string\noccupied:decode-error:ok: unsupported reflected JSON type: <never>\nmalformed:decode-error:expected result object with exactly one key\n",
    );
}

#[test]
fn native_json_inferred_parse_slots_recursive_01_string_key_empty_map_matches_reference_without_source_in_both_profiles()
 {
    run_case(
        "recursive_01_string_key_empty_map",
        include_str!("json_inferred_parse_slots/recursive_01_string_key_empty_map.jett"),
        "valid:length:0\noccupied:decode-error:x: unsupported reflected JSON type: <never>\nmalformed:decode-error:unterminated JSON object\n",
    );
}

#[test]
fn native_json_inferred_parse_slots_recursive_02_generic_record_empty_list_matches_reference_without_source_in_both_profiles()
 {
    run_case(
        "recursive_02_generic_record_empty_list",
        include_str!("json_inferred_parse_slots/recursive_02_generic_record_empty_list.jett"),
        "valid:length:0\noccupied:decode-error:items: 0: unsupported reflected JSON type: <never>\nmalformed:decode-error:unterminated JSON object\n",
    );
}

#[test]
fn native_json_inferred_parse_slots_recursive_03_nested_empty_list_matches_reference_without_source_in_both_profiles()
 {
    run_case(
        "recursive_03_nested_empty_list",
        include_str!("json_inferred_parse_slots/recursive_03_nested_empty_list.jett"),
        "valid:outer:1:inner:0\noccupied:decode-error:0: 0: unsupported reflected JSON type: <never>\nmalformed:decode-error:unterminated JSON array\n",
    );
}
