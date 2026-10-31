use super::run_bounded;
use super::suite_options::launcher_for_options;
use std::fs;

#[derive(Clone, Copy)]
enum ExpectedOutcome {
    Success(&'static str),
    RuntimeFailure {
        stdout: &'static str,
        message: &'static str,
    },
}

fn run_case(name: &str, source_text: &str, expected: ExpectedOutcome) {
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join(format!("{name}.jett"));
    fs::write(&source, source_text).unwrap();
    let (reference, status, terminal_stderr) = match expected {
        ExpectedOutcome::Success(stdout) => {
            let output = jett_driver::run_file_capture_outcome(&source)
                .unwrap_or_else(|error| panic!("{name}: checked reference fixture: {error:?}"));
            assert_eq!(output.stdout, stdout, "{name}");
            (output, 0, String::new())
        }
        ExpectedOutcome::RuntimeFailure { stdout, message } => {
            let failure = jett_driver::run_file_capture_outcome(&source)
                .expect_err("expected terminal scoped-view fixture failure");
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
        let artifact = jett_driver::native::build_host_executable_with_options(
            &source,
            launcher_for_options(release),
            &binary,
            jett_driver::BuildOptions { release },
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
            terminal_stderr.as_bytes(),
            "{name}, release={release}: {actual:?}"
        );
    }
}

#[test]
fn native_scoped_call_view_field_is_observed_before_the_later_handler() {
    run_case(
        "01_field_marker",
        include_str!("scoped_call_views/01_field_marker.jett"),
        ExpectedOutcome::Success("before:ready\nlater\nvalue:7:3\nowner:value:7\n"),
    );
}

#[test]
fn native_scoped_call_view_pending_field_fails_before_the_later_marker() {
    run_case(
        "02_pending_field",
        include_str!("scoped_call_views/02_pending_field.jett"),
        ExpectedOutcome::RuntimeFailure {
            stdout: "before:pending\n",
            message: "runtime error: field access is not supported on pending(pending(app.Packet(member: 7)))",
        },
    );
}

#[test]
fn native_scoped_call_view_declared_named_alias_control_preserves_owner() {
    run_case(
        "03_declared_alias",
        include_str!("scoped_call_views/03_declared_alias.jett"),
        ExpectedOutcome::Success("before:alias\nlater\nvalue:7:3\nowner:value:7\n"),
    );
}

#[test]
fn native_scoped_call_view_ends_before_post_call_owner_consumption() {
    run_case(
        "04_post_call_consume",
        include_str!("scoped_call_views/04_post_call_consume.jett"),
        ExpectedOutcome::Success("before:consume\nlater\nvalue:7:3\nconsumed:value:7\n"),
    );
}

#[test]
fn native_scoped_call_view_aborted_call_ends_before_owned_return_operand() {
    run_case(
        "05_early_return",
        include_str!("scoped_call_views/05_early_return.jett"),
        ExpectedOutcome::Success(
            "before:return\nearly\nreturned:value:7\nbefore:return\nanswer:value:9:4\nreturned:value:9\n",
        ),
    );
}

#[test]
fn native_scoped_call_view_mutable_field_owner_can_rebind_after_call() {
    run_case(
        "06_mutable_field",
        include_str!("scoped_call_views/06_mutable_field.jett"),
        ExpectedOutcome::Success("before:mutable\nlater\nvalue:7:3\nowner:value:9\n"),
    );
}

#[test]
fn native_scoped_call_view_owned_field_temporary_is_evaluated_once() {
    run_case(
        "07_owned_field_temporary",
        include_str!("scoped_call_views/07_owned_field_temporary.jett"),
        ExpectedOutcome::Success(
            "before:temporary\nsource:temporary\nlater\nvalue:7:3\nafter:temporary\n",
        ),
    );
}

#[test]
fn native_scoped_call_view_direct_mutable_named_owner_can_rebind_after_call() {
    run_case(
        "08_mutable_named",
        include_str!("scoped_call_views/08_mutable_named.jett"),
        ExpectedOutcome::Success("before:mutable-value\nlater\nvalue:7:3\nowner:value:9\n"),
    );
}

#[test]
fn native_scoped_call_view_direct_owned_named_temporary_is_evaluated_once() {
    run_case(
        "09_owned_named_temporary",
        include_str!("scoped_call_views/09_owned_named_temporary.jett"),
        ExpectedOutcome::Success(
            "before:temporary-value\nsource:temporary-value\nlater\nvalue:7:3\nafter:temporary-value\n",
        ),
    );
}

#[test]
fn native_scoped_call_view_indirect_field_call_ends_before_owner_consumption() {
    run_case(
        "10_indirect_field_call",
        include_str!("scoped_call_views/10_indirect_field_call.jett"),
        ExpectedOutcome::Success("before:indirect\nlater:indirect\nvalue:7:3\nconsumed:value:7\n"),
    );
}

#[test]
fn native_scoped_call_view_capability_formal_is_borrowed_until_operation_completes() {
    run_case(
        "11_capability_call",
        include_str!("scoped_call_views/11_capability_call.jett"),
        ExpectedOutcome::Success(
            "before:capability\nlater:capability\nwritten:capability\nafter:capability\n",
        ),
    );
}

#[test]
fn native_scoped_call_view_reflected_read_completes_validation_before_owner_consumption() {
    run_case(
        "12_reflected_read",
        include_str!("scoped_call_views/12_reflected_read.jett"),
        ExpectedOutcome::Success(
            "before:reflection\nselector:reflection\nvalue:12\nconsumed:value:7:12\n",
        ),
    );
}

#[test]
fn native_scoped_call_view_temporary_later_failure_preserves_first_error_and_cleanup() {
    run_case(
        "13_owned_temporary_later_failure",
        include_str!("scoped_call_views/13_owned_temporary_later_failure.jett"),
        ExpectedOutcome::RuntimeFailure {
            stdout: "before:cleanup\nsource:cleanup\nfailure:cleanup\n",
            message: "runtime error: list.__remove_at: index -1 out of bounds",
        },
    );
}
