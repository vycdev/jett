//! Qualified JSON walks only the selected machine payload; bare requests keep policy checks.
use super::run_bounded;
use super::suite_options::launcher_for_options;
use jett_driver::native::{
    build_host_executable_with_options, build_host_property_suite_executable_with_options,
    build_host_verify_suite_executable_with_options,
};
use jett_driver::{BuildOptions, run_file_capture_outcome};
use std::fs;

#[derive(Clone, Copy)]
enum Oracle {
    Exact(&'static str),
    PureSuites(&'static str),
    ParseErrors { exact: bool },
}

fn run_case(name: &str, source_text: &str, oracle: Oracle) {
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join(format!("{name}.jett"));
    fs::write(&source, source_text).unwrap();
    let reference = run_file_capture_outcome(&source)
        .unwrap_or_else(|error| panic!("{name}: checked reference fixture: {error:?}"));
    assert!(reference.debug_events.is_empty(), "{name}: {reference:?}");
    assert!(
        reference.frontend_debug_observations.is_empty(),
        "{name}: {reference:?}"
    );
    match oracle {
        Oracle::Exact(expected) | Oracle::PureSuites(expected) => {
            assert_eq!(reference.stdout, expected, "{name}");
        }
        Oracle::ParseErrors { exact } => {
            // Exact validation checks malformed field shape before the private
            // key-domain refusal; lenient decoding reaches that refusal first.
            let unsupported = "cached.entries: JSON object maps require string keys, got int64";
            let numeric = if exact {
                "entries: expected object, got number"
            } else {
                unsupported
            };
            let prefix = format!(
                "wrong:error:{unsupported}\nnonempty:error:{unsupported}\nmissing:error:missing required field 'entries' for app.Session at ready.cached\nnumeric:error:{numeric}\n"
            );
            assert!(
                reference.stdout.starts_with(&prefix),
                "{name}: {reference:?}"
            );
            assert_eq!(
                reference.stdout.lines().count(),
                if exact { 8 } else { 7 },
                "{name}: {reference:?}"
            );
            if exact {
                assert!(
                    reference
                        .stdout
                        .contains("envelope:error:unknown field 'version'"),
                    "{name}: {reference:?}"
                );
                assert!(
                    reference
                        .stdout
                        .contains("payload:error:unknown field 'extra'"),
                    "{name}: {reference:?}"
                );
            } else {
                assert!(
                    reference.stdout.contains("lenient:19\n"),
                    "{name}: {reference:?}"
                );
            }
            assert!(
                reference.stdout.ends_with("after:23\nretained:1\n"),
                "{name}: {reference:?}"
            );
        }
    }
    if matches!(oracle, Oracle::PureSuites(_)) {
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
        if matches!(oracle, Oracle::PureSuites(_)) {
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
            Some(0),
            "{name}, release={release}: {actual:?}"
        );
        assert_eq!(
            actual.stdout,
            reference.stdout.as_bytes(),
            "{name}, release={release}"
        );
        assert!(
            actual.stderr.is_empty(),
            "{name}, release={release}: {actual:?}"
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
fn native_qualified_machine_json_direct_serialize_matches_reference_without_source_in_both_profiles()
 {
    run_case(
        "direct_serialize",
        include_str!("json_qualified_machines/direct_serialize.jett"),
        Oracle::Exact("{\"state\":\"ready\",\"payload\":{\"count\":7}}\nalive:7\n"),
    );
}

#[test]
fn native_qualified_machine_json_direct_serialize_public_matches_reference_without_source_in_both_profiles()
 {
    run_case(
        "direct_serialize_public",
        include_str!("json_qualified_machines/direct_serialize_public.jett"),
        Oracle::Exact("{\"state\":\"ready\",\"payload\":{\"count\":8}}\nalive:8\n"),
    );
}

#[test]
fn native_qualified_machine_json_direct_parse_matches_reference_without_source_in_both_profiles() {
    run_case(
        "direct_parse",
        include_str!("json_qualified_machines/direct_parse.jett"),
        Oracle::Exact("9\n"),
    );
}

#[test]
fn native_qualified_machine_json_direct_parse_exact_matches_reference_without_source_in_both_profiles()
 {
    run_case(
        "direct_parse_exact",
        include_str!("json_qualified_machines/direct_parse_exact.jett"),
        Oracle::Exact("11\n"),
    );
}

#[test]
fn native_qualified_machine_json_pipeline_serialize_matches_reference_without_source_in_both_profiles()
 {
    run_case(
        "pipeline_serialize",
        include_str!("json_qualified_machines/pipeline_serialize.jett"),
        Oracle::Exact("{\"state\":\"ready\",\"payload\":{\"count\":12}}\nalive:12\n"),
    );
}

#[test]
fn native_qualified_machine_json_pipeline_serialize_public_matches_reference_without_source_in_both_profiles()
 {
    run_case(
        "pipeline_serialize_public",
        include_str!("json_qualified_machines/pipeline_serialize_public.jett"),
        Oracle::Exact("{\"state\":\"ready\",\"payload\":{\"count\":13}}\nalive:13\n"),
    );
}

#[test]
fn native_qualified_machine_json_pipeline_parse_matches_reference_without_source_in_both_profiles()
{
    run_case(
        "pipeline_parse",
        include_str!("json_qualified_machines/pipeline_parse.jett"),
        Oracle::Exact("14\n"),
    );
}

#[test]
fn native_qualified_machine_json_pipeline_parse_exact_matches_reference_without_source_in_both_profiles()
 {
    run_case(
        "pipeline_parse_exact",
        include_str!("json_qualified_machines/pipeline_parse_exact.jett"),
        Oracle::Exact("15\n"),
    );
}

#[test]
fn native_qualified_machine_json_nested_serialize_alias_preserves_selected_payload() {
    run_case(
        "nested_serialize",
        include_str!("json_qualified_machines/nested_serialize.jett"),
        Oracle::Exact(
            "[{\"state\":\"ready\",\"payload\":{\"count\":21}},{\"state\":\"ready\",\"payload\":{\"count\":22}}]\n21,22,\n",
        ),
    );
}

#[test]
fn native_qualified_machine_json_nested_serialize_public_alias_preserves_selected_payload() {
    run_case(
        "nested_serialize_public",
        include_str!("json_qualified_machines/nested_serialize_public.jett"),
        Oracle::Exact(
            "[{\"state\":\"ready\",\"payload\":{\"count\":21}},{\"state\":\"ready\",\"payload\":{\"count\":22}}]\n21,22,\n",
        ),
    );
}

#[test]
fn native_qualified_machine_json_nested_parse_alias_preserves_selected_payload() {
    run_case(
        "nested_parse",
        include_str!("json_qualified_machines/nested_parse.jett"),
        Oracle::Exact("21,22,\n"),
    );
}

#[test]
fn native_qualified_machine_json_nested_parse_exact_alias_preserves_selected_payload() {
    run_case(
        "nested_parse_exact",
        include_str!("json_qualified_machines/nested_parse_exact.jett"),
        Oracle::Exact("21,22,\n"),
    );
}

#[test]
fn native_qualified_machine_json_direct_parse_errors_preserves_error_order_and_cleanup() {
    run_case(
        "direct_parse_errors",
        include_str!("json_qualified_machines/direct_parse_errors.jett"),
        Oracle::ParseErrors { exact: false },
    );
}

#[test]
fn native_qualified_machine_json_direct_parse_exact_errors_preserves_error_order_and_cleanup() {
    run_case(
        "direct_parse_exact_errors",
        include_str!("json_qualified_machines/direct_parse_exact_errors.jett"),
        Oracle::ParseErrors { exact: true },
    );
}

#[test]
fn native_qualified_machine_json_pipeline_parse_errors_preserves_error_order_and_cleanup() {
    run_case(
        "pipeline_parse_errors",
        include_str!("json_qualified_machines/pipeline_parse_errors.jett"),
        Oracle::ParseErrors { exact: false },
    );
}

#[test]
fn native_qualified_machine_json_pipeline_parse_exact_errors_preserves_error_order_and_cleanup() {
    run_case(
        "pipeline_parse_exact_errors",
        include_str!("json_qualified_machines/pipeline_parse_exact_errors.jett"),
        Oracle::ParseErrors { exact: true },
    );
}

#[test]
fn native_qualified_machine_json_primitive_pipeline_parse_uses_the_source_decoder_independently() {
    run_case(
        "primitive_pipeline_parse",
        include_str!("json_qualified_machines/primitive_pipeline_parse.jett"),
        Oracle::Exact("14\nraw:14\n"),
    );
}

#[test]
fn native_qualified_machine_json_primitive_pipeline_parse_exact_uses_the_source_decoder_independently()
 {
    run_case(
        "primitive_pipeline_parse_exact",
        include_str!("json_qualified_machines/primitive_pipeline_parse_exact.jett"),
        Oracle::Exact("14\nraw:14\n"),
    );
}

#[test]
fn native_qualified_machine_json_pure_comptime_and_compiled_suites_use_selected_payload() {
    run_case(
        "pure_selected_suites",
        include_str!("json_qualified_machines/pure_selected_suites.jett"),
        Oracle::PureSuites("{\"state\":\"ready\",\"payload\":{\"count\":27}}\n"),
    );
}

#[test]
fn native_qualified_machine_json_bare_and_invalid_selected_payloads_keep_policy_rejection() {
    let cases = [
        (
            "reject_bare_serialize",
            include_str!("json_qualified_machines/reject_bare_serialize.jett"),
        ),
        (
            "reject_bare_serialize_public",
            include_str!("json_qualified_machines/reject_bare_serialize_public.jett"),
        ),
        (
            "reject_bare_parse",
            include_str!("json_qualified_machines/reject_bare_parse.jett"),
        ),
        (
            "reject_bare_parse_exact",
            include_str!("json_qualified_machines/reject_bare_parse_exact.jett"),
        ),
        (
            "reject_cached_serialize",
            include_str!("json_qualified_machines/reject_cached_serialize.jett"),
        ),
        (
            "reject_cached_serialize_public",
            include_str!("json_qualified_machines/reject_cached_serialize_public.jett"),
        ),
        (
            "reject_cached_parse",
            include_str!("json_qualified_machines/reject_cached_parse.jett"),
        ),
        (
            "reject_cached_parse_exact",
            include_str!("json_qualified_machines/reject_cached_parse_exact.jett"),
        ),
    ];
    for (name, text) in cases {
        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join(format!("{name}.jett"));
        fs::write(&source, text).unwrap();
        for release in [false, true] {
            let options = BuildOptions { release };
            let checked = jett_driver::build_file_with_options(&source, options);
            let errors = checked
                .diagnostics
                .iter()
                .filter(|diagnostic| diagnostic.severity == jett_diagnostics::Severity::Error)
                .collect::<Vec<_>>();
            assert_eq!(errors.len(), 1, "{name}, release={release}: {errors:?}");
            assert_eq!(
                errors[0].code.code(),
                343,
                "{name}, release={release}: {errors:?}"
            );
            let binary = directory.path().join(format!("{name}_{release}.exe"));
            let sentinel = b"existing qualified JSON publication";
            fs::write(&binary, sentinel).unwrap();
            let error = build_host_executable_with_options(
                &source,
                launcher_for_options(release),
                &binary,
                options,
            )
            .expect_err("unsupported map keys must remain frontend policy errors");
            let build = error.build_result().expect("typed frontend failure");
            assert!(
                build
                    .diagnostics
                    .iter()
                    .any(|diagnostic| diagnostic.code.code() == 343)
            );
            assert_eq!(fs::read(&binary).unwrap(), sentinel);
        }
    }
}

#[test]
fn native_qualified_machine_json_string_key_aliases_keep_exact_canonical_map_type() {
    run_case(
        "string_key_aliases",
        include_str!("json_qualified_machines/string_key_aliases.jett"),
        Oracle::Exact("41:41:{\"answer\":41}\n"),
    );
}

#[test]
fn native_qualified_machine_json_all_state_metadata_loops_keep_alias_and_empty_dispatch() {
    run_case(
        "qualified_state_fields",
        include_str!("json_qualified_machines/qualified_state_fields.jett"),
        Oracle::Exact(concat!(
            "ready:ready.count:app.Count;cached.entries:app.Entries;cached.backup:app.BackupCount;\n",
            "empty:ready.count:app.Count;cached.entries:app.Entries;cached.backup:app.BackupCount;\n",
            "selected:count:app.Count;\n",
            "selected-empty:\n",
            "retained:5\n",
        )),
    );
}

#[test]
fn native_qualified_machine_json_unselected_callable_interface_actor_payloads_skip_raw_decoder() {
    let expected = concat!(
        "{\"state\":\"ready\",\"payload\":{\"count\":31}}\n",
        "{\"state\":\"ready\",\"payload\":{\"count\":31}}\n",
        "32:33:31\n",
    );
    let cases = [
        (
            "unselected_callable",
            include_str!("json_qualified_machines/unselected_callable.jett"),
        ),
        (
            "unselected_interface",
            include_str!("json_qualified_machines/unselected_interface.jett"),
        ),
        (
            "unselected_actor",
            include_str!("json_qualified_machines/unselected_actor.jett"),
        ),
    ];
    for (name, text) in cases {
        run_case(name, text, Oracle::Exact(expected));
    }
}
