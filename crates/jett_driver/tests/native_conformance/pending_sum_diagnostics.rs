use super::{run_bounded, suite_options::launcher_for_options};
use jett_driver::native::build_host_executable;
use jett_driver::{build_file, run_file_capture_outcome};
use std::fs;

fn run_case(name: &str, source_bytes: &[u8], stdout: &str, failure: Option<&str>) {
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join(format!("{name}.jett"));
    fs::write(&source, source_bytes).unwrap();
    let checked = build_file(&source);
    assert!(!checked.has_errors, "{name}: {:?}", checked.diagnostics);
    let (reference, status, stderr) = match failure {
        Some(message) => {
            let actual = run_file_capture_outcome(&source).expect_err("terminal receiver failure");
            assert_eq!(actual.message, message, "{name}");
            (actual.output, 71, format!("{message}\n"))
        }
        None => (
            run_file_capture_outcome(&source).expect("ready sum control"),
            0,
            String::new(),
        ),
    };
    assert_eq!(reference.stdout, stdout, "{name}");
    assert!(reference.debug_events.is_empty(), "{name}: {reference:?}");
    assert!(
        reference.frontend_debug_observations.is_empty(),
        "{name}: {reference:?}"
    );
    let binaries = [false, true].map(|release| {
        let binary = directory.path().join(format!("{name}-{release}.exe"));
        let artifact = build_host_executable(&source, launcher_for_options(release), &binary)
            .unwrap_or_else(|error| panic!("{name}, runtime release={release}: {error}"));
        assert!(
            artifact.debug_observations.is_empty(),
            "{name}: {artifact:?}"
        );
        (binary, release)
    });
    fs::remove_file(&source).unwrap();
    assert!(!source.exists());
    for (binary, release) in binaries {
        let actual = run_bounded(&binary, directory.path());
        assert_eq!(
            actual.status.code(),
            Some(status),
            "{name}, runtime release={release}: {actual:?}"
        );
        assert_eq!(
            actual.stdout,
            stdout.as_bytes(),
            "{name}, runtime release={release}"
        );
        assert_eq!(
            actual.stderr,
            stderr.as_bytes(),
            "{name}, runtime release={release}"
        );
    }
}

#[test]
fn native_pending_sum_diagnostics_redact_secret_result_error_without_joining() {
    run_case(
        "01_pending_secret_error",
        include_bytes!("pending_sum_diagnostics/01_pending_secret_error.jett"),
        "before\n",
        Some(
            "runtime error: handle block requires a result or optional value, got pending(pending(fail([redacted])))",
        ),
    );
}

#[test]
fn native_pending_sum_diagnostics_redact_nested_optional_field_without_joining() {
    run_case(
        "02_pending_optional_secret_field",
        include_bytes!("pending_sum_diagnostics/02_pending_optional_secret_field.jett"),
        "before\n",
        Some(
            "runtime error: handle block requires a result or optional value, got pending(pending(some(app.Hidden(token: [redacted]))))",
        ),
    );
}

#[test]
fn native_pending_sum_diagnostics_preserve_pipeline_result_secret_type() {
    run_case(
        "03_pipeline_pending_secret_error",
        include_bytes!("pending_sum_diagnostics/03_pipeline_pending_secret_error.jett"),
        "before\n",
        Some(
            "runtime error: handle block requires a result or optional value, got pending(pending(fail([redacted])))",
        ),
    );
}

#[test]
fn native_pending_sum_diagnostics_keep_ready_payloads_and_handler_error_types() {
    run_case(
        "ready",
        br#"namespace app
function ready(value: int64) returns result[int64, secret[string]]:
    return fail("ready-secret")
function main(stdout: Stdout) returns nothing:
    result[secret[int64], string] success = ok(7)
    secret[int64] direct = success handle error:
        return nothing
    int64 public_value = declassify direct
    int64 fallback = 7 into ready handle error:
        string message = declassify error
        default string.char_count(message)
    optional[secret[string]] candidate = some("ready-optional")
    secret[string] selected = candidate handle:
        return nothing
    string public_text = declassify selected
    Stdout.write(view stdout, "{public_value}:{fallback}:{string.char_count(public_text)}\n")
    return nothing
"#,
        "7:12:14\n",
        None,
    );
}

#[test]
fn native_pending_sum_diagnostics_preserve_prior_target_failure_and_once_order() {
    const SOURCE: &str = r#"namespace app
function failed(view stdout: Stdout) returns result[int64, secret[string]]:
    Stdout.write(view stdout, "producer\n")
    string rejected = string.repeat("ab", 9223372036854775807)
    return ok(string.char_count(rejected))
function main(stdout: Stdout) returns nothing:
    Stdout.write(view stdout, "before\n")
    int64 unused = CANDIDATE handle error:
        Stdout.write(view stdout, "unexpected handler\n")
        return nothing
    Stdout.write(view stdout, "unexpected success\n")
    return nothing
"#;
    for (name, candidate) in [
        ("prior_direct", "failed(view stdout)"),
        ("prior_pipeline", "view stdout into failed"),
    ] {
        run_case(
            name,
            SOURCE.replace("CANDIDATE", candidate).as_bytes(),
            "before\nproducer\n",
            Some("runtime error: string.repeat: requested output is too large"),
        );
    }
}

#[test]
fn native_pending_sum_diagnostics_preserve_named_and_retained_view_pipeline_types() {
    for (name, source) in [
        (
            "parenthesized_view_named",
            include_bytes!("pending_sum_diagnostics/04_parenthesized_view_named_pipeline.jett")
                .as_slice(),
        ),
        (
            "named_view_parameter",
            include_bytes!("pending_sum_diagnostics/06_named_view_parameter_pipeline.jett")
                .as_slice(),
        ),
        (
            "retained_view_parameter",
            include_bytes!("pending_sum_diagnostics/07_retained_view_parameter_pipeline.jett")
                .as_slice(),
        ),
    ] {
        run_case(
            name,
            source,
            "before\n",
            Some(
                "runtime error: handle block requires a result or optional value, got pending(pending(fail([redacted])))",
            ),
        );
    }
}
