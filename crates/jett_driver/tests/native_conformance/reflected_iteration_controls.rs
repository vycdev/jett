//! Ordinary reflected metadata controls for runtime, comptime, and native suites.
use super::run_bounded;
use super::suite_options::launcher_for_options;
use jett_driver::BuildOptions;
use jett_driver::native::{
    build_host_executable_with_options, build_host_property_suite_executable_with_options,
    build_host_verify_suite_executable_with_options,
};
use std::fs;

struct Case {
    name: &'static str,
    source: &'static str,
    stdout: &'static str,
}

macro_rules! case {
    ($name:literal) => {
        Case {
            name: $name,
            source: include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../tests/native/reflected_iteration_controls/",
                $name,
                ".jett"
            )),
            stdout: include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../tests/native/reflected_iteration_controls/",
                $name,
                ".stdout"
            )),
        }
    };
}

fn run_form(name: &str, source_text: &str, expected_stdout: &str) {
    let expected_stdout = expected_stdout.replace("\r\n", "\n");
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("main.jett");
    fs::write(&source, source_text).unwrap();
    let reference = jett_driver::run_file_capture_outcome(&source)
        .unwrap_or_else(|failure| panic!("{name}: {failure:?}"));
    assert_eq!(reference.stdout, expected_stdout, "{name}");
    assert!(reference.debug_events.is_empty(), "{name}: {reference:?}");
    let mut programs = Vec::new();
    let mut suites = Vec::new();
    for release in [false, true] {
        let binary = directory.path().join(format!("iteration_{release}.exe"));
        build_host_executable_with_options(
            &source,
            launcher_for_options(release),
            &binary,
            BuildOptions { release },
        )
        .unwrap_or_else(|failure| panic!("{name}: {failure}"));
        programs.push(binary);
        let verify = directory
            .path()
            .join(format!("iteration_verify_{release}.exe"));
        let property = directory
            .path()
            .join(format!("iteration_property_{release}.exe"));
        build_host_verify_suite_executable_with_options(
            &source,
            launcher_for_options(release),
            &verify,
            BuildOptions { release },
        )
        .unwrap_or_else(|failure| panic!("{name}: {failure}"));
        build_host_property_suite_executable_with_options(
            &source,
            launcher_for_options(release),
            &property,
            BuildOptions { release },
        )
        .unwrap_or_else(|failure| panic!("{name}: {failure}"));
        suites.extend([verify, property]);
    }
    fs::remove_file(&source).unwrap();
    for binary in programs {
        let actual = run_bounded(&binary, directory.path());
        assert_eq!(actual.status.code(), Some(0), "{name}: {actual:?}");
        assert_eq!(
            actual.stdout,
            expected_stdout.as_bytes(),
            "{name}: {actual:?}"
        );
        assert!(actual.stderr.is_empty(), "{name}: {actual:?}");
    }
    for binary in suites {
        let actual = run_bounded(&binary, directory.path());
        assert_eq!(actual.status.code(), Some(0), "{name}: {actual:?}");
        assert!(actual.stdout.is_empty(), "{name}: {actual:?}");
        assert!(actual.stderr.is_empty(), "{name}: {actual:?}");
    }
}

#[test]
fn native_ordinary_equal_type_metadata_branches_and_pure_helpers_match_reference() {
    for case in [
        case!("nominal"),
        case!("members"),
        case!("arguments_nested"),
    ] {
        run_form(case.name, case.source, case.stdout);
        if matches!(case.name, "nominal" | "members") {
            let mut piped = case.source.to_owned();
            for owner in ["Record", "T", "Header"] {
                piped = piped.replace(
                    &format!("type.field_value[{owner}, Field](view source, view field)"),
                    &format!("source into view type.field_value[{owner}, Field](view field)"),
                );
            }
            assert_ne!(piped, case.source, "{}: getter pipeline control", case.name);
            run_form(&format!("{}_pipeline", case.name), &piped, case.stdout);
        }
    }
}
