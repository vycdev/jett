use super::{launcher, run_bounded};
use jett_driver::native::{
    build_host_executable, build_host_executable_with_options,
    build_host_property_suite_executable, build_host_verify_suite_executable,
};
use jett_driver::{
    BuildOptions, DebugEvent, DebugEventKind, DebugObservation, DebugPhase, render_debug_events,
    run_file_capture_outcome,
};
use std::fs;

fn event(kind: DebugEventKind, text: &str) -> DebugEvent {
    DebugEvent {
        kind,
        text: text.into(),
    }
}

const MIXED_OBSERVATIONS: &str = r#"namespace app
function build_observe() returns int64:
    print("compile:")
    println("built")
    return 7
function observations() returns nothing:
    int64 marker = 42
    print()
    print("partial:")
    trace marker
    println()
    println("trace fake\nbreakpoint hit\noutput,lambda:λ🙂\\\r")
    breakpoint true
    print("tail")
function main(stdout: Stdout) returns nothing:
    int64 baked = comptime build_observe()
    Stdout.write(view stdout, "application:{baked}")
    observations()
verify checked:
    println("frontend:")
    assert true
"#;

#[test]
fn native_debug_events_preserve_exact_boundaries_and_exclude_frontend_capture() {
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("mixed.jett");
    fs::write(&source, MIXED_OBSERVATIONS).unwrap();
    let reference = run_file_capture_outcome(&source).unwrap();
    assert_eq!(reference.stdout, "application:7");
    assert_eq!(
        reference.frontend_debug_observations,
        [
            DebugObservation {
                phase: DebugPhase::Comptime,
                event: event(DebugEventKind::Print, "compile:"),
            },
            DebugObservation {
                phase: DebugPhase::Comptime,
                event: event(DebugEventKind::Println, "built\n"),
            },
            DebugObservation {
                phase: DebugPhase::FrontendVerify,
                event: event(DebugEventKind::Println, "frontend:\n"),
            },
        ],
    );
    assert_eq!(
        reference.debug_events,
        [
            event(DebugEventKind::Print, ""),
            event(DebugEventKind::Print, "partial:"),
            event(DebugEventKind::Trace, "trace marker: int64 = 42\n"),
            event(DebugEventKind::Println, "\n"),
            event(
                DebugEventKind::Println,
                "trace fake\nbreakpoint hit\noutput,lambda:λ🙂\\\r\n",
            ),
            event(
                DebugEventKind::Breakpoint,
                "breakpoint hit: marker: int64 = 42\n"
            ),
            event(DebugEventKind::Print, "tail"),
        ],
    );
    let binary = directory.path().join("mixed.exe");
    let artifact = build_host_executable(&source, launcher(), &binary).unwrap();
    assert_eq!(
        artifact.debug_observations,
        reference.frontend_debug_observations
    );
    fs::remove_file(&source).unwrap();
    let actual = run_bounded(&binary, directory.path());
    assert_eq!(actual.status.code(), Some(0), "{actual:?}");
    assert_eq!(actual.stdout, reference.stdout.as_bytes());
    assert_eq!(
        actual.stderr,
        render_debug_events(&reference.debug_events).as_bytes()
    );
}

const ARGUMENT_FAILURE: &str = r#"namespace app
function argument() returns list[string]:
    print("argument:")
    return list("held")
function failed() returns int64:
    println("failing")
    string rejected = string.repeat("ab", 9223372036854775807)
    return string.char_count(rejected)
function main(stdout: Stdout) returns nothing:
    Stdout.write(view stdout, "application-before")
    print("before:")
    println(argument(), failed())
    println("unreachable")
"#;

#[test]
fn native_debug_argument_failure_keeps_prior_events_and_retires_outer_owners() {
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("arguments.jett");
    fs::write(&source, ARGUMENT_FAILURE).unwrap();
    let failure = run_file_capture_outcome(&source).unwrap_err();
    assert_eq!(failure.output.stdout, "application-before");
    assert!(failure.output.frontend_debug_observations.is_empty());
    assert_eq!(
        failure.output.debug_events,
        [
            event(DebugEventKind::Print, "before:"),
            event(DebugEventKind::Print, "argument:"),
            event(DebugEventKind::Println, "failing\n"),
        ],
    );
    assert_eq!(
        failure.message,
        "runtime error: string.repeat: requested output is too large"
    );
    let binary = directory.path().join("arguments.exe");
    build_host_executable(&source, launcher(), &binary).unwrap();
    fs::remove_file(&source).unwrap();
    let actual = run_bounded(&binary, directory.path());
    // A leaked outer argument owner would replace entry status 71 with 72.
    assert_eq!(actual.status.code(), Some(71), "{actual:?}");
    assert_eq!(actual.stdout, failure.output.stdout.as_bytes());
    let expected = format!(
        "{}{}\n",
        render_debug_events(&failure.output.debug_events),
        failure.message
    );
    assert_eq!(actual.stderr, expected.as_bytes());
}

#[test]
fn native_partial_debug_text_is_adjacent_to_terminal_error_without_inserted_newline() {
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("partial_failure.jett");
    fs::write(
        &source,
        "namespace app\nfunction main(stdout: Stdout) returns nothing:\n    Stdout.write(view stdout, \"application\")\n    print(\"unterminated:\")\n    list[string] retained = list(\"owner\")\n    string rejected = string.repeat(\"ab\", 9223372036854775807)\n",
    ).unwrap();
    let failure = run_file_capture_outcome(&source).unwrap_err();
    assert_eq!(failure.output.stdout, "application");
    assert_eq!(
        failure.output.debug_events,
        [event(DebugEventKind::Print, "unterminated:")]
    );
    let binary = directory.path().join("partial_failure.exe");
    build_host_executable(&source, launcher(), &binary).unwrap();
    fs::remove_file(&source).unwrap();
    let actual = run_bounded(&binary, directory.path());
    assert_eq!(actual.status.code(), Some(71), "{actual:?}");
    assert_eq!(actual.stdout, b"application");
    assert_eq!(
        actual.stderr,
        format!("unterminated:{}\n", failure.message).as_bytes()
    );
}

#[test]
fn native_compiled_suites_emit_only_their_actual_debug_calls_after_source_deletion() {
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("suites.jett");
    fs::write(
        &source,
        "namespace app\nfunction main() returns nothing:\n    return nothing\nverify diagnostic:\n    print(\"verify:\")\n    println(\"tail\")\n    assert true\nproperty diagnostic_trials:\n    given chosen: bool\n    print(\"trial:\")\n    println(chosen)\n    assert chosen == chosen\n",
    ).unwrap();
    let verify = directory.path().join("verify.exe");
    let property = directory.path().join("property.exe");
    build_host_verify_suite_executable(&source, launcher(), &verify).unwrap();
    build_host_property_suite_executable(&source, launcher(), &property).unwrap();
    fs::remove_file(&source).unwrap();
    for (binary, diagnostic) in [
        (verify, "verify:tail\n".to_owned()),
        (property, "trial:true\ntrial:false\n".repeat(50)),
    ] {
        let actual = run_bounded(&binary, directory.path());
        assert_eq!(actual.status.code(), Some(0), "{actual:?}");
        assert!(actual.stdout.is_empty(), "{actual:?}");
        assert_eq!(actual.stderr, diagnostic.as_bytes());
    }
}

#[test]
fn native_print_release_rejection_keeps_existing_publication_and_checks_arguments() {
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("release_print.jett");
    let binary = directory.path().join("preserved.exe");
    let sentinel = b"existing native publication";
    fs::write(&binary, sentinel).unwrap();
    fs::write(
        &source,
        "namespace app\nfunction main() returns nothing:\n    println(1 + true)\n",
    )
    .unwrap();
    let result = jett_driver::build_file_with_options(&source, BuildOptions { release: true });
    assert!(result.has_errors);
    assert!(
        result
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code.code() == 362)
    );
    assert!(
        result
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code.code() == 301)
    );
    build_host_executable_with_options(
        &source,
        launcher(),
        &binary,
        BuildOptions { release: true },
    )
    .expect_err("release source and its arguments must be checked before publication");
    assert_eq!(fs::read(&binary).unwrap(), sentinel);
}
