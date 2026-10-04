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
                .expect_err("expected terminal caller-ownership fixture failure");
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
fn native_call_ownership_direct_bare_view_argument_relinquishes_the_owner() {
    run_case(
        "01_direct_last_use",
        include_str!("call_ownership/01_direct_last_use.jett"),
        ExpectedOutcome::Success("direct:2\n"),
    );
}

#[test]
fn native_call_ownership_indirect_bare_view_argument_relinquishes_the_owner() {
    run_case(
        "02_indirect_last_use",
        include_str!("call_ownership/02_indirect_last_use.jett"),
        ExpectedOutcome::Success("indirect:2\n"),
    );
}

#[test]
fn native_call_ownership_named_view_arguments_follow_the_checked_permutation() {
    run_case(
        "03_named_last_use",
        include_str!("call_ownership/03_named_last_use.jett"),
        ExpectedOutcome::Success("named:5\n"),
    );
}

#[test]
fn native_call_ownership_pipeline_bare_view_argument_relinquishes_the_owner() {
    run_case(
        "04_pipeline_last_use",
        include_str!("call_ownership/04_pipeline_last_use.jett"),
        ExpectedOutcome::Success("pipeline:2\n"),
    );
}

#[test]
fn native_call_ownership_written_views_and_explicit_clone_preserve_the_owner() {
    run_case(
        "05_explicit_retention",
        include_str!("call_ownership/05_explicit_retention.jett"),
        ExpectedOutcome::Success("retained:2:2:2:2:2:2\n"),
    );
}

#[test]
fn native_call_ownership_source_first_owned_parameter_is_transferred() {
    run_case(
        "06_source_first_owned_transfer",
        include_str!("call_ownership/06_source_first_owned_transfer.jett"),
        ExpectedOutcome::Success("transfer:5:5:2\n"),
    );
}

#[test]
fn native_call_ownership_relinquished_owner_survives_a_later_default_handler() {
    run_case(
        "07_later_handler_default",
        include_str!("call_ownership/07_later_handler_default.jett"),
        ExpectedOutcome::Success("before:default\nlater:default\nanswer:5\n"),
    );
}

#[test]
fn native_call_ownership_aborted_call_cleans_owner_before_independent_owned_return() {
    run_case(
        "08_early_return_cleanup",
        include_str!("call_ownership/08_early_return_cleanup.jett"),
        ExpectedOutcome::Success("before:return\nlater:return\nreturned:3\n"),
    );
}

#[test]
fn native_call_ownership_later_failure_cleans_staged_and_live_owners() {
    run_case(
        "09_terminal_failure_cleanup",
        include_str!("call_ownership/09_terminal_failure_cleanup.jett"),
        ExpectedOutcome::RuntimeFailure {
            stdout: "before:cleanup\nfailure:cleanup\n",
            message: "runtime error: list.__remove_at: index -1 out of bounds",
        },
    );
}

#[test]
fn native_call_ownership_copyable_scalars_and_noncopyable_representations_are_distinct() {
    run_case(
        "10_scalar_refinement_function",
        include_str!("call_ownership/10_scalar_refinement_function.jett"),
        ExpectedOutcome::Success("typed:7:7:7:4\n"),
    );
}

#[test]
fn native_call_ownership_named_owned_producers_are_evaluated_once_in_source_order() {
    run_case(
        "11_named_producer_order",
        include_str!("call_ownership/11_named_producer_order.jett"),
        ExpectedOutcome::Success("source:right\nsource:left\nordered:5\n"),
    );
}

#[test]
fn native_call_ownership_consumed_binding_rereads_fail_before_native_publication() {
    let cases = [
        (
            "negative_direct_reread",
            include_str!("call_ownership/negative_direct_reread.jett"),
        ),
        (
            "negative_parenthesized_reread",
            include_str!("call_ownership/negative_parenthesized_reread.jett"),
        ),
        (
            "negative_indirect_reread",
            include_str!("call_ownership/negative_indirect_reread.jett"),
        ),
        (
            "negative_named_owned_reread",
            include_str!("call_ownership/negative_named_owned_reread.jett"),
        ),
        (
            "negative_pipeline_reread",
            include_str!("call_ownership/negative_pipeline_reread.jett"),
        ),
        (
            "negative_refinement_reread",
            include_str!("call_ownership/negative_refinement_reread.jett"),
        ),
        (
            "negative_function_reread",
            include_str!("call_ownership/negative_function_reread.jett"),
        ),
    ];
    for (name, source_text) in cases {
        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join(format!("{name}.jett"));
        fs::write(&source, source_text).unwrap();
        let launcher = if cfg!(windows) {
            jett_driver::native::NativeLauncherBundle::windows_msvc_static_v1(
                directory.path().join("unused.lib"),
            )
        } else {
            jett_driver::native::NativeLauncherBundle::linux_gnu_v1(
                directory.path().join("unused.a"),
            )
        };
        let output = directory.path().join("preserved.exe");
        let sentinel = b"existing caller ownership publication";
        fs::write(&output, sentinel).unwrap();
        assert!(!launcher.archive_path.exists());
        for release in [false, true] {
            let error = jett_driver::native::build_host_executable_with_options(
                &source,
                &launcher,
                &output,
                jett_driver::BuildOptions { release },
            )
            .expect_err("consumed binding must fail before runtime archive lookup");
            let build = error.build_result().expect("typed frontend failure");
            assert!(build.has_errors, "{name}, release={release}");
            let errors = build
                .diagnostics
                .iter()
                .filter(|diagnostic| diagnostic.severity == jett_diagnostics::Severity::Error)
                .collect::<Vec<_>>();
            assert_eq!(errors.len(), 1, "{name}, release={release}: {errors:?}");
            assert_eq!(errors[0].code.code(), 400, "{name}, release={release}");
            assert!(error.debug_observations().is_empty());
            assert!(build.debug_observations.is_empty());
            assert!(!launcher.archive_path.exists());
            assert_eq!(fs::read(&output).unwrap(), sentinel);
        }
    }
}

#[test]
fn native_call_ownership_present_success_keeps_its_payload_after_pruning() {
    run_case(
        "12_inferred_present_success",
        include_str!("call_ownership/12_inferred_present_success.jett"),
        ExpectedOutcome::Success("before:success\nafter:success\ncount:2\n"),
    );
}

#[test]
fn native_call_ownership_inhabited_error_control_keeps_the_original_branch() {
    run_case(
        "13_inhabited_error_control",
        include_str!("call_ownership/13_inhabited_error_control.jett"),
        ExpectedOutcome::Success("before:success\nafter:success\ncount:2\n"),
    );
}

#[test]
fn native_call_ownership_present_success_still_rejects_pending_before_extraction() {
    run_case(
        "14_pending_present_success",
        include_str!("call_ownership/14_pending_present_success.jett"),
        ExpectedOutcome::RuntimeFailure {
            stdout: "before:success\n",
            message: "runtime error: handle block requires a result or optional value, got pending(pending(ok(list(1, 2))))",
        },
    );
}

#[test]
fn native_call_ownership_converted_handled_view_reads_its_input_and_owns_the_box() {
    run_case(
        "15_converted_handled_view",
        include_str!("call_ownership/15_converted_handled_view.jett"),
        ExpectedOutcome::Success(
            "source\ndefault:source\nlater\ndefault:later\nabsent:item:1:3\nsource\nlater\npresent:item:2:11\n",
        ),
    );
}
