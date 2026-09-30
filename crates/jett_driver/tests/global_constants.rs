use std::fs;
use std::path::{Path, PathBuf};

use jett_diagnostics::Severity;
use jett_driver::{
    build_file, build_source, run_file_capture_outcome, run_file_capture_stdout, test_file,
};
use tempfile::TempDir;

struct SourceFixture {
    directory: TempDir,
    entry: PathBuf,
}

impl SourceFixture {
    fn new(source: &str) -> Self {
        let directory = tempfile::tempdir().expect("create constant fixture directory");
        let entry = directory.path().join("main.jett");
        fs::write(&entry, source).expect("write constant fixture source");
        Self { directory, entry }
    }

    fn project(source: &str) -> Self {
        let directory = tempfile::tempdir().expect("create constant project directory");
        fs::create_dir(directory.path().join("src")).expect("create project source directory");
        fs::write(
            directory.path().join("jett.proj"),
            "name: global_constants\nversion: 0.1.0\nentry: src/main.jett\n",
        )
        .expect("write constant project manifest");
        let entry = directory.path().join("src/main.jett");
        fs::write(&entry, source).expect("write constant project entry");
        Self { directory, entry }
    }

    fn write(&self, path: &str, source: &str) {
        fs::write(self.directory.path().join(path), source).expect("write project source");
    }

    fn entry(&self) -> &Path {
        &self.entry
    }
}

fn assert_builds(path: &Path) {
    let built = build_file(path);
    assert!(!built.has_errors, "{:?}", built.diagnostics);
}

fn assert_checks_pass(path: &Path, total: usize) {
    let checks = test_file(path).expect("constant verify and property blocks should execute");
    let failures = checks
        .blocks
        .iter()
        .filter(|block| !block.passed)
        .map(|block| (&block.name, &block.error))
        .collect::<Vec<_>>();
    assert_eq!(checks.total, total, "{failures:?}");
    assert_eq!(checks.passed, total, "{failures:?}");
    assert_eq!(checks.failed, 0, "{failures:?}");
    for property in checks.blocks.iter().filter(|block| block.is_property) {
        assert!(property.iterations.is_some_and(|iterations| iterations > 0));
    }
}

fn assert_constant_rejected_before_execution(path: &Path, expected_message: &str) {
    let built = build_file(path);
    let errors = built
        .diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.severity == Severity::Error)
        .collect::<Vec<_>>();
    assert!(
        built.has_errors,
        "invalid constants must be rejected even when unused"
    );
    assert!(!errors.is_empty(), "{:?}", built.diagnostics);
    assert!(
        errors
            .iter()
            .all(|diagnostic| diagnostic.code.code() == 9001),
        "{:?}",
        built.diagnostics
    );
    assert!(
        errors
            .iter()
            .any(|diagnostic| diagnostic.message.contains(expected_message)),
        "{:?}",
        built.diagnostics
    );
    let failure = run_file_capture_outcome(path)
        .expect_err("invalid compile-time constants must prevent main execution");
    assert!(failure.message.contains("E9001"), "{}", failure.message);
    assert!(
        failure.message.contains(expected_message),
        "{}",
        failure.message
    );
    assert!(failure.output.stdout.is_empty());
    assert!(failure.output.debug_output.is_empty());
    let failure = test_file(path)
        .err()
        .expect("test execution must reject invalid constants");
    assert!(failure.contains("E9001"), "{failure}");
    assert!(failure.contains(expected_message), "{failure}");
}

#[cfg(all(
    target_arch = "x86_64",
    any(
        all(target_os = "windows", target_env = "msvc"),
        all(target_os = "linux", target_env = "gnu")
    )
))]
fn assert_native_constant_rejection_preserves_output(path: &Path, expected_message: &str) {
    use jett_driver::native::{
        NativeBuildError, NativeLauncherBundle, build_host_executable_with_options,
        build_host_property_suite_executable, build_host_verify_suite_executable,
    };

    let directory = tempfile::tempdir().expect("create native rejection output directory");
    let output = directory.path().join("preserved.exe");
    let sentinel = b"existing native output";
    fs::write(&output, sentinel).expect("write existing native executable sentinel");
    // Frontend failure must happen before the launcher archive is opened.
    let launcher = if cfg!(windows) {
        NativeLauncherBundle::windows_msvc_static_v1(directory.path().join("unused.lib"))
    } else {
        NativeLauncherBundle::linux_gnu_v1(directory.path().join("unused.a"))
    };
    let check_failure = |error: NativeBuildError| {
        let NativeBuildError::Lowering { source, .. } = error else {
            panic!("expected checked constant rejection before native publication, got {error}");
        };
        let jett_driver::BackendLoweringError::Build(checked) = *source else {
            panic!("expected preserved frontend diagnostics before HIR");
        };
        let errors = checked
            .diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.severity == Severity::Error)
            .collect::<Vec<_>>();
        assert!(!errors.is_empty(), "{:?}", checked.diagnostics);
        assert!(
            errors
                .iter()
                .all(|diagnostic| diagnostic.code.code() == 9001),
            "{:?}",
            checked.diagnostics
        );
        assert!(
            errors
                .iter()
                .any(|diagnostic| diagnostic.message.contains(expected_message)),
            "{:?}",
            checked.diagnostics
        );
        assert_eq!(
            fs::read(&output).expect("read preserved executable"),
            sentinel
        );
    };
    for release in [false, true] {
        check_failure(
            build_host_executable_with_options(
                path,
                &launcher,
                &output,
                jett_driver::BuildOptions { release },
            )
            .expect_err("invalid constants prevent debug and release native publication"),
        );
    }
    check_failure(
        build_host_verify_suite_executable(path, &launcher, &output)
            .expect_err("invalid constants prevent native verify suite publication"),
    );
    check_failure(
        build_host_property_suite_executable(path, &launcher, &output)
            .expect_err("invalid constants prevent native property suite publication"),
    );
}

#[test]
fn build_source_evaluates_constants_before_verification() {
    let built = build_source(
        "namespace app\nint64 base = 42\nint64 answer = base\nverify constants:\n    assert answer == 42\n",
        "constants.jett",
    );
    assert!(!built.has_errors, "{:?}", built.diagnostics);
    let values = built
        .explicit_comptime_values
        .expect("checked constant table");
    assert!(!values.is_empty());
    assert_eq!(values.len(), 2);
}

#[test]
fn literal_and_explicit_constants_match_reference_checks() {
    let fixture = SourceFixture::new(
        r#"namespace app
int64 count = 21
int64 copied = count
int64 doubled = comptime (copied * 2)
int8 maximum = 127
int8 wrapped = comptime (maximum + 1)
uint8 unsigned_maximum = 255
uint8 unsigned_wrapped = comptime (unsigned_maximum + 1)
float32 scale = 1.25
bool enabled = true
string label = comptime "{doubled}:{wrapped}:{unsigned_wrapped}:{scale}:{enabled}"
function main(stdout: Stdout) returns nothing:
    Stdout.write(view stdout, "{label}\n")
verify constants:
    int8 minimum = -128
    assert doubled == 42
    assert wrapped == minimum
    assert unsigned_wrapped == 0
    assert scale == 1.25
    assert enabled
    assert label == "42:-128:0:1.25:true"
property constant_reads:
    given value: int64
    assert value + doubled - doubled == value
    assert label == "42:-128:0:1.25:true"
"#,
    );
    let built = build_file(fixture.entry());
    assert!(!built.has_errors, "{:?}", built.diagnostics);
    let values = built
        .explicit_comptime_values
        .expect("checked constant table");
    assert!(
        !values.is_empty(),
        "namespace declarations have compiler values"
    );
    assert_eq!(values.len(), values.values().count());
    assert_eq!(
        run_file_capture_stdout(fixture.entry()).expect("run literal constant program"),
        "42:-128:0:1.25:true\n"
    );
    assert_checks_pass(fixture.entry(), 2);
}

#[test]
fn sibling_namespaces_keep_same_named_constants_distinct() {
    let fixture = SourceFixture::project(
        r#"namespace app
int64 answer = 80
function combined() returns int64:
    use alpha
    use beta
    return alpha.read() + beta.read() + answer
function main(stdout: Stdout) returns nothing:
    use alpha
    use beta
    Stdout.write(view stdout, "{alpha.read()}:{beta.read()}:{answer}:{combined()}\n")
verify combined:
    assert combined() == 120
property namespace_constants:
    given value: int64
    assert combined() == 120
"#,
    );
    fixture.write(
        "src/00_alpha.jett",
        r#"namespace alpha
int64 answer = 11
export function read() returns int64:
    return answer
verify alpha_constant:
    assert read() == 11
"#,
    );
    fixture.write(
        "src/10_beta.jett",
        r#"namespace beta
int64 answer = 29
export function read() returns int64:
    return answer
verify beta_constant:
    assert read() == 29
"#,
    );
    assert_builds(fixture.entry());
    assert_eq!(
        run_file_capture_stdout(fixture.entry()).expect("run project constant program"),
        "11:29:80:120\n"
    );
    assert_checks_pass(fixture.entry(), 2);
    assert_checks_pass(&fixture.directory.path().join("src/00_alpha.jett"), 1);
    assert_checks_pass(&fixture.directory.path().join("src/10_beta.jett"), 1);
}

#[test]
fn inline_callbacks_and_generic_bodies_read_namespace_constants() {
    let fixture = SourceFixture::new(
        r#"namespace app
int64 offset = 6
string prefix = "baked"
function generic_offset[T](value: T) returns int64:
    return offset
function make_callback[T](value: T) returns function(int64) returns string:
    return function(number: int64) returns string: return "{prefix}:{number + offset}"
function callback_result() returns string:
    function(int64) returns string callback = make_callback[bool](true)
    return callback(4)
function inline_result() returns int64:
    function(int64) returns int64 callback = function(number: int64) returns int64: return number + offset
    return callback(3)
function main(stdout: Stdout) returns nothing:
    int64 integer_offset = generic_offset[int64](10)
    int64 string_offset = generic_offset[string]("ignored")
    Stdout.write(view stdout, "{integer_offset}:{string_offset}:{callback_result()}:{inline_result()}\n")
verify callback_constants:
    assert generic_offset[int64](10) == 6
    assert generic_offset[string]("ignored") == 6
    assert callback_result() == "baked:10"
    assert inline_result() == 9
property generic_constants:
    given value: int64
    assert generic_offset[int64](value) == 6
    assert callback_result() == "baked:10"
"#,
    );
    assert_builds(fixture.entry());
    assert_eq!(
        run_file_capture_stdout(fixture.entry()).expect("run callback constant program"),
        "6:6:baked:10:9\n"
    );
    assert_checks_pass(fixture.entry(), 2);
}

#[test]
fn ordinary_function_initializers_require_explicit_comptime() {
    let fixture = SourceFixture::new(
        r#"namespace app
function compute() returns int64:
    return 42
int64 answer = compute()
function main() returns int64:
    return answer
"#,
    );
    let built = build_file(fixture.entry());
    let errors = built
        .diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.severity == Severity::Error)
        .collect::<Vec<_>>();
    assert!(built.has_errors);
    assert_eq!(errors.len(), 1, "{:?}", built.diagnostics);
    assert_eq!(errors[0].code.code(), 378, "{:?}", built.diagnostics);
    assert!(errors[0].message.contains("comptime expression"));
}

#[test]
fn explicit_comptime_function_initializers_are_baked() {
    let fixture = SourceFixture::new(
        r#"namespace app
int64 base = 21
function compute() returns int64:
    return base * 2
int64 answer = comptime compute()
function main(stdout: Stdout) returns nothing:
    Stdout.write(view stdout, "{answer}\n")
verify evaluated_constant:
    assert answer == 42
property evaluated_constant_reads:
    given value: int64
    assert answer == 42
"#,
    );
    assert_builds(fixture.entry());
    assert_eq!(
        run_file_capture_stdout(fixture.entry()).expect("run evaluated constant program"),
        "42\n"
    );
    assert_checks_pass(fixture.entry(), 2);
}

#[test]
fn unused_constant_evaluation_failure_prevents_runtime_execution() {
    let fixture = SourceFixture::new(
        r#"namespace app
function impossible() returns string:
    return string.repeat("ab", 9223372036854775807)
string unused = comptime impossible()
function main(stdout: Stdout) returns nothing:
    Stdout.write(view stdout, "main must not run")
verify would_otherwise_pass:
    assert true
"#,
    );
    let built = build_file(fixture.entry());
    let errors = built
        .diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.severity == Severity::Error)
        .collect::<Vec<_>>();
    assert!(
        built.has_errors,
        "failing constants must be evaluated even when unused"
    );
    assert!(!errors.is_empty(), "{:?}", built.diagnostics);
    assert!(
        errors
            .iter()
            .all(|diagnostic| diagnostic.code.code() == 9001)
    );
    assert!(
        errors
            .iter()
            .any(|diagnostic| diagnostic.message.contains("requested output is too large")),
        "{:?}",
        built.diagnostics
    );
    let failure = run_file_capture_outcome(fixture.entry())
        .expect_err("failed compile-time constants must prevent main execution");
    assert!(failure.message.contains("E9001"), "{}", failure.message);
    assert!(failure.output.stdout.is_empty());
    assert!(failure.output.debug_output.is_empty());
    let failure = test_file(fixture.entry())
        .err()
        .expect("test execution must reject failed constants");
    assert!(failure.contains("E9001"), "{failure}");
}

#[test]
fn namespace_constants_cannot_be_rebound() {
    let fixture = SourceFixture::new(
        r#"namespace app
int64 answer = 42
function main() returns nothing:
    answer = 43
"#,
    );
    let built = build_file(fixture.entry());
    let errors = built
        .diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.severity == Severity::Error)
        .collect::<Vec<_>>();
    assert!(built.has_errors);
    assert_eq!(errors.len(), 1, "{:?}", built.diagnostics);
    assert_eq!(errors[0].code.code(), 404, "{:?}", built.diagnostics);
    assert!(errors[0].message.contains("cannot reassign `answer`"));
}

#[test]
fn unused_aggregate_constants_are_rejected_before_execution() {
    for declaration in [
        "list[int64] unused = comptime list(1, 2)",
        "Record unused = comptime Record(value: 42)",
    ] {
        let fixture = SourceFixture::new(&format!(
            "namespace app\nstruct Record:\n    value: int64\n{declaration}\nfunction main(stdout: Stdout) returns nothing:\n    Stdout.write(view stdout, \"main must not run\")\nverify would_otherwise_pass:\n    assert true\nproperty constant_check:\n    given value: int64\n    assert value == value\n"
        ));
        let expected = "implicitly copyable primitive type";
        assert_constant_rejected_before_execution(fixture.entry(), expected);
        #[cfg(all(
            target_arch = "x86_64",
            any(
                all(target_os = "windows", target_env = "msvc"),
                all(target_os = "linux", target_env = "gnu")
            )
        ))]
        assert_native_constant_rejection_preserves_output(fixture.entry(), expected);
    }
}

#[test]
fn hidden_pending_primitive_constants_are_rejected_before_execution() {
    for expression in ["run(42)", "run(run(42))"] {
        let fixture = SourceFixture::new(&format!(
            "namespace app\nint64 pending_value = comptime {expression}\nfunction main(stdout: Stdout) returns nothing:\n    Stdout.write(view stdout, \"main must not run\")\nverify would_otherwise_pass:\n    assert true\nproperty constant_check:\n    given value: int64\n    assert value == value\n"
        ));
        let expected = "does not match its declared primitive type";
        assert_constant_rejected_before_execution(fixture.entry(), expected);
        #[cfg(all(
            target_arch = "x86_64",
            any(
                all(target_os = "windows", target_env = "msvc"),
                all(target_os = "linux", target_env = "gnu")
            )
        ))]
        assert_native_constant_rejection_preserves_output(fixture.entry(), expected);
    }
}
