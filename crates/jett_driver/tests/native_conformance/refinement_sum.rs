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

// Keep the original refined-sum program: the selected numeric domain rejects
// it before its predicate, entry effects, or native publication can run.
#[test]
fn refined_sum_domain_rejects_before_reference_execution_and_native_publication() {
    use jett_driver::native::{NativeBuildError, build_host_executable_with_options};
    use jett_driver::{BuildOptions, build_file_with_options};

    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("main.jett");
    fs::write(&source, EMPTY_REFINED_SUM).unwrap();
    let failure = jett_driver::run_file_capture_outcome(&source)
        .expect_err("refined sums are rejected during checking");
    assert!(failure.message.contains("E0300"), "{}", failure.message);
    assert!(failure.message.contains("numeric primitive for list.sum"));
    assert!(failure.output.stdout.is_empty());
    assert!(failure.output.debug_events.is_empty());

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
        assert!(checked.has_errors, "refined sum must fail checking");
        assert!(
            checked.diagnostics.iter().any(|error| {
                error.code.code() == 300 && error.message.contains("numeric primitive for list.sum")
            }),
            "{:?}",
            checked.diagnostics
        );
        assert!(checked.debug_observations.is_empty());
        let error =
            build_host_executable_with_options(&source, &missing_launcher, &output, options)
                .expect_err(
                    "native refined summation must fail before archive lookup and publication",
                );
        assert!(error.debug_observations().is_empty());
        let NativeBuildError::Lowering { source, .. } = error else {
            panic!("expected frontend sum-domain rejection: {error:?}");
        };
        let built = source.build_result().expect("frontend type diagnostic");
        assert!(built.diagnostics.iter().any(|error| {
            error.code.code() == 300 && error.message.contains("numeric primitive for list.sum")
        }));
        assert_eq!(fs::read(&output).unwrap(), sentinel);
        assert!(!missing_launcher.archive_path.exists());
    }
}
