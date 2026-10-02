use std::fs;
use std::path::Path;

use jett_diagnostics::Severity;
use jett_driver::{BuildResult, build_file, build_source, run_file_capture_outcome, test_file};

const DECLARATIONS: &str = "namespace app\nstruct Inner:\n    value: int64\nstruct Record:\n    inner: Inner\n    label: string\nenum Choice:\n    wrapped(record: Record)\nfunction make() returns Record:\n    return Record(inner: Inner(value: 1), label: \"before\")\n";
const ENTRIES: &str = "function main(stdout: Stdout) returns nothing:\n    Stdout.write(view stdout, \"must not run\")\nverify valid_check:\n    assert true\nproperty valid_trials:\n    given number: int64\n    assert number == number\n";

fn rejected_cases() -> Vec<(&'static str, String, u16, &'static str)> {
    [
        (
            "immutable_local",
            "function invalid() returns nothing:\n    Record record = make()\n    (record).inner.value = 9\n",
            404,
            "cannot reassign `record` because it is not mutable",
        ),
        (
            "owned_parameter",
            "function invalid(record: Record) returns nothing:\n    record.inner.value = 9\n",
            404,
            "cannot reassign `record` because it is not mutable",
        ),
        (
            "mutable_view_parameter",
            "function invalid(view mutable record: Record) returns nothing:\n    record.inner.value = 9\n",
            401,
            "cannot assign through a view because views are read-only",
        ),
        (
            "owned_rebind_from_view",
            "function invalid(view source: Record) returns nothing:\n    mutable Record alias = make()\n    alias = view source\n    alias.inner.value = 9\n",
            401,
            "cannot rebind an owned value from a view; clone the value instead",
        ),
        (
            "parenthesized_owned_rebind_from_view",
            "function invalid(view source: Record) returns nothing:\n    mutable Record alias = make()\n    ((alias)) = view source\n    alias.inner.value = 9\n",
            401,
            "cannot rebind an owned value from a view; clone the value instead",
        ),
        (
            "explicit_view",
            "function invalid() returns nothing:\n    mutable Record record = make()\n    (view record).inner.value = 9\n",
            401,
            "cannot assign through a view because views are read-only",
        ),
        (
            "temporary_call",
            "function invalid() returns nothing:\n    make().inner.value = 9\n",
            404,
            "projected assignment requires an owned mutable binding",
        ),
        (
            "temporary_clone",
            "function invalid() returns nothing:\n    Record record = make()\n    (clone record).inner.value = 9\n",
            404,
            "projected assignment requires an owned mutable binding",
        ),
        (
            "borrowed_loop",
            "function invalid() returns nothing:\n    mutable list[Record] records = list(make())\n    for record in view records:\n        record.inner.value = 9\n",
            401,
            "cannot assign through a view because views are read-only",
        ),
        (
            "borrowed_match",
            "function invalid(view choice: Choice) returns nothing:\n    match choice:\n        wrapped(record):\n            record.inner.value = 9\n",
            404,
            "cannot reassign `record` because it is not mutable",
        ),
        (
            "expired_binding",
            "function invalid() returns nothing:\n    if false:\n        mutable Record record = make()\n        trace record\n    Record record = make()\n    record.inner.value = 9\n",
            404,
            "cannot reassign `record` because it is not mutable",
        ),
        (
            "actor_state",
            "actor Store:\n    app.Record record = make()\n    receive replace:\n        record.inner.value = 9\n",
            404,
            "cannot reassign `record` because it is not mutable",
        ),
        (
            "actor_message_view",
            "actor Store:\n    receive replace(view mutable record: Record):\n        record.inner.value = 9\n",
            401,
            "cannot assign through a view because views are read-only",
        ),
        (
            "actor_owned_rebind_from_view",
            "actor Store:\n    receive replace(view source: Record):\n        mutable Record alias = make()\n        alias = view source\n        alias.inner.value = 9\n",
            401,
            "cannot rebind an owned value from a view; clone the value instead",
        ),
        (
            "explicit_comptime",
            "function invalid() returns int64:\n    Record record = make()\n    record.inner.value = 9\n    return record.inner.value\nfunction requested() returns int64:\n    return comptime invalid()\n",
            404,
            "cannot reassign `record` because it is not mutable",
        ),
        (
            "verify_local",
            "verify invalid:\n    Record record = make()\n    record.inner.value = 9\n    assert true\n",
            404,
            "cannot reassign `record` because it is not mutable",
        ),
        (
            "property_given",
            "property invalid:\n    given record: Record\n    record.inner.value = 9\n    assert true\n",
            404,
            "cannot reassign `record` because it is not mutable",
        ),
    ]
    .into_iter()
    .map(|(name, body, code, message)| (name, format!("{DECLARATIONS}{body}{ENTRIES}"), code, message))
    .collect()
}

fn assert_rejected(built: BuildResult, name: &str, code: u16, message: &str) {
    let errors = built
        .diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.severity == Severity::Error)
        .collect::<Vec<_>>();
    assert!(built.has_errors, "{name}: invalid projected write accepted");
    assert_eq!(errors.len(), 1, "{name}: {:?}", built.diagnostics);
    assert_eq!(
        errors[0].code.code(),
        code,
        "{name}: {:?}",
        built.diagnostics
    );
    assert_eq!(errors[0].message, message, "{name}");
}

#[test]
fn readonly_projected_writes_fail_before_driver_evaluation() {
    for (name, source, code, message) in rejected_cases() {
        assert_rejected(build_source(&source, name), name, code, message);
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("main.jett");
        fs::write(&path, source).unwrap();
        assert_rejected(build_file(&path), name, code, message);
        let failure = run_file_capture_outcome(&path).expect_err("readonly write prevents main");
        assert!(
            failure.message.contains(&format!("E{code:04}")),
            "{name}: {}",
            failure.message
        );
        assert!(
            failure.message.contains(message),
            "{name}: {}",
            failure.message
        );
        assert!(failure.output.stdout.is_empty(), "{name}");
        assert!(failure.output.debug_events.is_empty(), "{name}");
        let failure = test_file(&path)
            .err()
            .expect("readonly write prevents suites");
        assert!(
            failure.contains(&format!("E{code:04}")),
            "{name}: {failure}"
        );
        assert!(failure.contains(message), "{name}: {failure}");
    }
}

#[test]
fn owned_mutable_projected_writes_remain_checked_pending_execution_policy() {
    let source = format!(
        "{DECLARATIONS}function local() returns nothing:\n    mutable Record record = make()\n    (record).inner.value = 9\nfunction parameter(mutable record: Record) returns nothing:\n    record.label = \"after\"\n"
    );
    let built = build_source(&source, "owned_mutable.jett");
    assert!(!built.has_errors, "{:?}", built.diagnostics);
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("main.jett");
    fs::write(&path, source).unwrap();
    let built = build_file(&path);
    assert!(!built.has_errors, "{:?}", built.diagnostics);
}

#[test]
fn projected_reads_and_reconstructed_owners_pass_reference_suites() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/native/projected_read_controls.jett");
    let built = build_file(&path);
    assert!(!built.has_errors, "{:?}", built.diagnostics);
    let output = jett_driver::run_file_capture_output(&path).expect("projected read controls");
    assert_eq!(
        output.stdout,
        "original:1\ncopied:2\noriginal:1\nloop:3\nmatch:2:2\ntemporary:1\nfields:7:1\n"
    );
    assert!(output.debug_events.is_empty());
    let checks = test_file(&path).expect("projected read suites");
    assert_eq!(checks.total, 2);
    assert_eq!(checks.passed, 2);
    assert_eq!(checks.failed, 0);
    assert!(
        checks
            .blocks
            .iter()
            .any(|block| block.is_property && block.iterations.is_some_and(|count| count > 0))
    );
}

#[cfg(all(
    target_arch = "x86_64",
    any(
        all(target_os = "windows", target_env = "msvc"),
        all(target_os = "linux", target_env = "gnu")
    )
))]
#[test]
fn readonly_projected_writes_preserve_existing_native_publications() {
    use jett_driver::native::{
        NativeBuildError, NativeLauncherBundle, build_host_executable_with_options,
        build_host_property_suite_executable, build_host_verify_suite_executable,
    };

    for (name, source, code, message) in rejected_cases().into_iter().filter(|(name, _, _, _)| {
        matches!(
            *name,
            "mutable_view_parameter"
                | "owned_rebind_from_view"
                | "parenthesized_owned_rebind_from_view"
                | "actor_owned_rebind_from_view"
                | "temporary_call"
                | "explicit_comptime"
                | "verify_local"
                | "property_given"
        )
    }) {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("main.jett");
        let output = directory.path().join("preserved.exe");
        let sentinel = b"existing native output";
        fs::write(&path, source).unwrap();
        fs::write(&output, sentinel).unwrap();
        let launcher = if cfg!(windows) {
            NativeLauncherBundle::windows_msvc_static_v1(directory.path().join("unused.lib"))
        } else {
            NativeLauncherBundle::linux_gnu_v1(directory.path().join("unused.a"))
        };
        let check = |error: NativeBuildError| {
            let NativeBuildError::Lowering { source, .. } = error else {
                panic!("{name}: expected frontend rejection before archive lookup: {error}");
            };
            let jett_driver::BackendLoweringError::Build(built) = *source else {
                panic!("{name}: expected preserved frontend diagnostics before HIR");
            };
            assert_rejected(built, name, code, message);
            assert_eq!(fs::read(&output).unwrap(), sentinel, "{name}");
        };
        for release in [false, true] {
            check(
                build_host_executable_with_options(
                    &path,
                    &launcher,
                    &output,
                    jett_driver::BuildOptions { release },
                )
                .expect_err("readonly write prevents native publication"),
            );
        }
        check(
            build_host_verify_suite_executable(&path, &launcher, &output)
                .expect_err("readonly write prevents native verify publication"),
        );
        check(
            build_host_property_suite_executable(&path, &launcher, &output)
                .expect_err("readonly write prevents native property publication"),
        );
    }
}
