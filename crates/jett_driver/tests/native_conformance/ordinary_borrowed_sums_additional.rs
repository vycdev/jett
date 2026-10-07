use super::run_bounded;
use super::suite_options::launcher_for_options;
use jett_driver::BuildOptions;
use jett_driver::native::{
    build_host_executable_with_options, emit_host_program_object_for_file_with_options,
};
use std::fs;
use std::path::Path;

#[derive(Clone, Copy)]
enum ExpectedOutcome {
    Success(&'static str),
    RuntimeFailure {
        stdout: &'static str,
        message: &'static str,
    },
}

struct SourceCase {
    name: &'static str,
    source: &'static str,
    expected: ExpectedOutcome,
}

const CASES: &[SourceCase] = &[
    SourceCase {
        name: "13_optional_named_handle_failure",
        source: include_str!(
            "ordinary_borrowed_sums_additional/13_optional_named_handle_failure.jett"
        ),
        expected: ExpectedOutcome::RuntimeFailure {
            stdout: concat!(
                "before:optional:8\nprefix:optional:5\nlater:optional\n",
                "default:optional\nfailure:optional\n",
            ),
            message: "runtime error: list.__remove_at: index -1 out of bounds",
        },
    },
    SourceCase {
        name: "14_result_named_handle_failure",
        source: include_str!(
            "ordinary_borrowed_sums_additional/14_result_named_handle_failure.jett"
        ),
        expected: ExpectedOutcome::RuntimeFailure {
            stdout: concat!(
                "before:result:8\nprefix:result:12\nlater:result\n",
                "default:result\nfailure:result\n",
            ),
            message: "runtime error: list.__remove_at: index -1 out of bounds",
        },
    },
    SourceCase {
        name: "15_mutable_copied_payloads",
        source: include_str!("ordinary_borrowed_sums_additional/15_mutable_copied_payloads.jett"),
        expected: ExpectedOutcome::Success("integer:4:3\nstring:changed:kept\n"),
    },
];

struct CheckedOutcome {
    output: jett_driver::RunOutput,
    status: i32,
    stderr: String,
}

fn reference(case: &SourceCase, source: &Path) -> CheckedOutcome {
    let checked = jett_driver::build_file_with_options(source, BuildOptions::default());
    assert!(
        !checked.has_errors,
        "{}: Source must pass checking: {:?}",
        case.name, checked.diagnostics
    );
    let outcome = match case.expected {
        ExpectedOutcome::Success(stdout) => {
            let output = jett_driver::run_file_capture_outcome(source)
                .unwrap_or_else(|failure| panic!("{}: reference: {failure:?}", case.name));
            assert_eq!(output.stdout, stdout, "{}", case.name);
            CheckedOutcome {
                output,
                status: 0,
                stderr: String::new(),
            }
        }
        ExpectedOutcome::RuntimeFailure { stdout, message } => {
            let failure = jett_driver::run_file_capture_outcome(source)
                .expect_err("Source must reach its specified reference runtime failure");
            assert_eq!(failure.output.stdout, stdout, "{}", case.name);
            assert_eq!(failure.message, message, "{}", case.name);
            CheckedOutcome {
                output: failure.output,
                status: 71,
                stderr: format!("{message}\n"),
            }
        }
    };
    assert!(outcome.output.debug_events.is_empty(), "{}", case.name);
    assert!(
        outcome.output.frontend_debug_observations.is_empty(),
        "{}",
        case.name
    );
    eprintln!(
        "supplemental reference {}: status={} stdout={:?} stderr={:?}",
        case.name, outcome.status, outcome.output.stdout, outcome.stderr
    );
    outcome
}

#[test]
fn ordinary_borrowed_sums_additional_reference_matches_three_source_outcomes() {
    assert_eq!(CASES.len(), 3);
    for case in CASES {
        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join(format!("{}.jett", case.name));
        fs::write(&source, case.source).unwrap();
        reference(case, &source);
    }
    eprintln!("supplemental reference: 3 Source cases matched exact outcomes");
}

#[test]
fn ordinary_borrowed_sums_additional_native_matches_six_executions_without_source() {
    assert_eq!(CASES.len(), 3);
    let mut refusals = Vec::new();
    let mut executions = 0;
    for case in CASES {
        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join(format!("{}.jett", case.name));
        fs::write(&source, case.source).unwrap();
        let expected = reference(case, &source);
        let mut binaries = Vec::new();
        for release in [false, true] {
            let options = BuildOptions { release };
            // Refusals are parity failures. Attempt both profiles and remove
            // Source before executing any available linked artifacts.
            if let Err(error) = emit_host_program_object_for_file_with_options(&source, options) {
                let refusal = format!("{}, release={release}: {error:?}", case.name);
                eprintln!("supplemental native refusal {refusal}");
                refusals.push(refusal);
                continue;
            }
            let binary = directory
                .path()
                .join(format!("{}_{release}.exe", case.name));
            match build_host_executable_with_options(
                &source,
                launcher_for_options(release),
                &binary,
                options,
            ) {
                Ok(artifact) => {
                    assert!(artifact.debug_observations.is_empty(), "{}", case.name);
                    binaries.push((binary, release));
                }
                Err(error) => {
                    let refusal = format!("{}, release={release}: {error:?}", case.name);
                    eprintln!("supplemental native refusal {refusal}");
                    refusals.push(refusal);
                }
            }
        }
        fs::remove_file(&source).unwrap();
        assert!(!source.exists(), "{}: Source remains", case.name);
        for (binary, release) in binaries {
            let actual = run_bounded(&binary, directory.path());
            assert_eq!(
                actual.status.code(),
                Some(expected.status),
                "{}, release={release}: {actual:?}",
                case.name
            );
            assert_eq!(
                actual.stdout,
                expected.output.stdout.as_bytes(),
                "{}, release={release}",
                case.name
            );
            assert_eq!(
                actual.stderr,
                expected.stderr.as_bytes(),
                "{}, release={release}: {actual:?}",
                case.name
            );
            executions += 1;
            eprintln!(
                "supplemental native {}: release={release} status={} Source absent",
                case.name, expected.status
            );
        }
    }
    assert!(
        refusals.is_empty(),
        "supplemental ordinary borrowed-sum parity has {} native refusals:\n{}",
        refusals.len(),
        refusals.join("\n")
    );
    assert_eq!(executions, 6, "all 3 Sources require both native profiles");
    eprintln!("supplemental native: 3 Source cases / 6 executions matched exact outcomes");
}
