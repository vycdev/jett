use super::*;

const EMPTY_REFINED_SUM: &str = r#"namespace app
function positive_check(value: int64) returns bool:
    string marker = "positive"
    trace marker
    return value > 0
type Positive = int64 where positive_check(value)
function later(view stdout: Stdout) returns optional[string]:
    Stdout.write(view stdout, "later\n")
    return none
function fallback(view stdout: Stdout) returns string:
    Stdout.write(view stdout, "handler\n")
    return "fallback"
function main(stdout: Stdout) returns nothing:
    bytes earlier = bytes.from_string("earlier-owned")
    list[string] retained = list("kept", "owned")
    list[Positive] values = list()
    Stdout.write(view stdout, "before\n")
    Positive total = list.sum[Positive](view values)
    int64 public_total = coarsen total
    string message = "{later(view stdout) handle: default fallback(view stdout)}"
    Stdout.write(view stdout, "after:{public_total}:{message}:{bytes.to_hex(view earlier)}:{list.length(view retained)}\n")
"#;

// This is an existing reference invariant and a native admission refusal, not
// linked runtime parity or a new contract for refined list.sum specializations.
#[test]
fn refined_sum_results_validate_in_reference_and_preserve_native_refusal() {
    use jett_driver::native::{NativeBuildError, build_host_executable_with_options};
    use jett_driver::{BuildOptions, build_file_with_options};

    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("main.jett");
    fs::write(&source, EMPTY_REFINED_SUM).unwrap();
    let failure = jett_driver::run_file_capture_outcome(&source)
        .expect_err("the empty unchecked sum produces zero, which cannot inhabit Positive");
    assert_eq!(
        failure.message,
        "runtime error: refinement type constraint failed for 'app.Positive'"
    );
    assert_eq!(failure.output.stdout, "before\n");
    assert_eq!(
        debug_trace_lines(&failure.output.debug_events),
        ["trace marker: string = positive"]
    );

    let output = directory.path().join("preserved.exe");
    let sentinel = b"existing native publication";
    fs::write(&output, sentinel).unwrap();
    let missing_launcher = if cfg!(windows) {
        NativeLauncherBundle::windows_msvc_static_v1(directory.path().join("unused.lib"))
    } else {
        NativeLauncherBundle::linux_gnu_v1(directory.path().join("unused.a"))
    };
    assert!(!missing_launcher.archive_path.exists());
    for release in [false, true] {
        let options = BuildOptions { release };
        let checked = build_file_with_options(&source, options);
        assert!(
            !checked.has_errors,
            "the source remains frontend-admitted without selecting a refined-sum policy: {:?}",
            checked.diagnostics
        );
        let error =
            build_host_executable_with_options(&source, &missing_launcher, &output, options)
                .expect_err(
                    "native refined summation must fail before archive lookup and publication",
                );
        let NativeBuildError::Codegen { source, .. } = error else {
            panic!("expected conservative native sum admission: {error:?}");
        };
        let jett_codegen_cranelift::CodegenError::InvalidMirContract { message, .. } = source
        else {
            panic!("expected the unsupported sum signature: {source:?}");
        };
        assert_eq!(message, "invalid native list signature for list.__sum");
        assert_eq!(fs::read(&output).unwrap(), sentinel);
        assert!(!missing_launcher.archive_path.exists());
    }
}
