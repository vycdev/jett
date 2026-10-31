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
        name: "01_optional_branches",
        source: include_str!("ordinary_borrowed_sums/01_optional_branches.jett"),
        expected: ExpectedOutcome::Success(concat!(
            "before:written-some\nsome:written-some:2:5:5\n",
            "before:written-none\nnone:written-none:7\n",
            "before:bare-some\nsome:bare-some:2:12:12\n",
            "before:bare-none\nnone:bare-none:7\n",
        )),
    },
    SourceCase {
        name: "02_result_branches",
        source: include_str!("ordinary_borrowed_sums/02_result_branches.jett"),
        expected: ExpectedOutcome::Success(concat!(
            "before:written-ok\nok:written-ok:2:5:5\n",
            "before:written-fail\nfail:written-fail:written error\n",
            "before:bare-ok\nok:bare-ok:2:12:12\n",
            "before:bare-fail\nfail:bare-fail:bare error\n",
        )),
    },
    SourceCase {
        name: "03_owned_scoped_forwarded",
        source: include_str!("ordinary_borrowed_sums/03_owned_scoped_forwarded.jett"),
        expected: ExpectedOutcome::Success(concat!(
            "optional:nested:5\noptional:5:18:15:5\n",
            "result:nested:12\nresult:12:29:36:12\n",
            "generic:1:1\n",
        )),
    },
    SourceCase {
        name: "04_named_prefix_return",
        source: include_str!("ordinary_borrowed_sums/04_named_prefix_return.jett"),
        expected: ExpectedOutcome::Success(concat!(
            "prefix:optional:5\nlater:optional\nreturn:optional\n",
            "prefix:result:12\nlater:result\nreturn:result\n",
            "answers:7:11\n",
            "prefix:optional:5\nlater:optional\nreturn:optional\n",
            "prefix:result:12\nlater:result\nreturn:result\n",
            "retained:7:11\n",
        )),
    },
    SourceCase {
        name: "05_optional_terminal_prefix",
        source: include_str!("ordinary_borrowed_sums/05_optional_terminal_prefix.jett"),
        expected: ExpectedOutcome::RuntimeFailure {
            stdout: "before:optional:8\nprefix:optional:5\nfailure:optional\n",
            message: "runtime error: list.__remove_at: index -1 out of bounds",
        },
    },
    SourceCase {
        name: "06_result_terminal_prefix",
        source: include_str!("ordinary_borrowed_sums/06_result_terminal_prefix.jett"),
        expected: ExpectedOutcome::RuntimeFailure {
            stdout: "before:result:8\nprefix:result:12\nfailure:result\n",
            message: "runtime error: list.__remove_at: index -1 out of bounds",
        },
    },
    SourceCase {
        name: "07_outer_pending_some",
        source: include_str!("ordinary_borrowed_sums/07_outer_pending_some.jett"),
        expected: ExpectedOutcome::RuntimeFailure {
            stdout: "before\n",
            message: "runtime error: handle block requires a result or optional value, got pending(pending(some(list(2, 3))))",
        },
    },
    SourceCase {
        name: "08_outer_pending_none",
        source: include_str!("ordinary_borrowed_sums/08_outer_pending_none.jett"),
        expected: ExpectedOutcome::RuntimeFailure {
            stdout: "before\n",
            message: "runtime error: handle block requires a result or optional value, got pending(pending(none))",
        },
    },
    SourceCase {
        name: "09_outer_pending_ok",
        source: include_str!("ordinary_borrowed_sums/09_outer_pending_ok.jett"),
        expected: ExpectedOutcome::RuntimeFailure {
            stdout: "before\n",
            message: "runtime error: handle block requires a result or optional value, got pending(pending(ok(list(2, 3))))",
        },
    },
    SourceCase {
        name: "10_outer_pending_fail",
        source: include_str!("ordinary_borrowed_sums/10_outer_pending_fail.jett"),
        expected: ExpectedOutcome::RuntimeFailure {
            stdout: "before\n",
            message: "runtime error: handle block requires a result or optional value, got pending(pending(fail(empty)))",
        },
    },
    SourceCase {
        name: "11_pending_optional_payload",
        source: include_str!("ordinary_borrowed_sums/11_pending_optional_payload.jett"),
        expected: ExpectedOutcome::RuntimeFailure {
            stdout: "afteralias\n",
            message: "runtime error: list.__sum: argument must be a list",
        },
    },
    SourceCase {
        name: "12_pending_result_payload",
        source: include_str!("ordinary_borrowed_sums/12_pending_result_payload.jett"),
        expected: ExpectedOutcome::RuntimeFailure {
            stdout: "afteralias\n",
            message: "runtime error: list.__sum: argument must be a list",
        },
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
        "reference {}: status={} stdout={:?} stderr={:?}",
        case.name, outcome.status, outcome.output.stdout, outcome.stderr
    );
    outcome
}

#[test]
fn ordinary_borrowed_sums_reference_packet_matches_exact_source_outcomes() {
    assert_eq!(CASES.len(), 12);
    for case in CASES {
        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join(format!("{}.jett", case.name));
        fs::write(&source, case.source).unwrap();
        reference(case, &source);
    }
}

#[test]
fn ordinary_borrowed_sums_native_packet_matches_reference_without_source() {
    let mut refusals = Vec::new();
    for case in CASES {
        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join(format!("{}.jett", case.name));
        fs::write(&source, case.source).unwrap();
        let expected = reference(case, &source);
        let mut binaries = Vec::new();
        for release in [false, true] {
            let options = BuildOptions { release };
            // Reach native verification before building a runtime archive. A
            // refused object is a real parity failure, never an expected pass.
            if let Err(error) = emit_host_program_object_for_file_with_options(&source, options) {
                let refusal = format!("{}, release={release}: {error:?}", case.name);
                eprintln!("native refusal {refusal}");
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
                    eprintln!("native refusal {refusal}");
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
        }
    }
    assert!(
        refusals.is_empty(),
        "ordinary borrowed-sum parity has {} native refusals:\n{}",
        refusals.len(),
        refusals.join("\n")
    );
}
