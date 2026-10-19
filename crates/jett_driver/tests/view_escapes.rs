use std::fs;

use jett_diagnostics::Severity;
use jett_driver::{BuildResult, build_source, run_file_capture_outcome, test_file};

const DECLARATIONS: &str = "namespace app\nfunction consume(values: list[int64]) returns int64:\n    return list.length(view values)\nfunction generic_consume[T](values: list[T]) returns int64:\n    return list.length[T](view values)\n";
const ENTRIES: &str = "function main(stdout: Stdout) returns nothing:\n    Stdout.write(view stdout, \"must not run\")\nverify valid_check:\n    assert true\nproperty valid_trials:\n    given input: int64\n    assert input == input\n";

fn rejected_cases() -> Vec<(&'static str, String)> {
    [
        (
            "borrowed_return",
            "function invalid(view values: list[int64]) returns list[int64]:\n    return values\n",
        ),
        (
            "forwarded_return",
            "function invalid(view values: list[int64]) returns list[int64]:\n    list[int64] first = view values\n    list[int64] second = first\n    return (second)\n",
        ),
        (
            "owned_call_alias",
            "function invalid(view values: list[int64]) returns int64:\n    list[int64] borrowed = view values\n    return consume(borrowed)\n",
        ),
        (
            "generic_call_alias",
            "function invalid(view values: list[int64]) returns int64:\n    list[int64] borrowed = view values\n    return generic_consume[int64](borrowed)\n",
        ),
        (
            "indirect_call_alias",
            "function invalid(view values: list[int64]) returns int64:\n    list[int64] borrowed = view values\n    function(list[int64]) returns int64 callback = consume\n    return callback(borrowed)\n",
        ),
        (
            "pipeline_alias",
            "function invalid(view values: list[int64]) returns int64:\n    list[int64] borrowed = view values\n    return borrowed into consume\n",
        ),
        (
            "coarsened_return",
            "type Data = list[int64] where true\nfunction invalid(view values: Data) returns list[int64]:\n    return coarsen values\n",
        ),
        (
            "declassified_return",
            "function invalid(view values: secret[list[int64]]) returns list[int64]:\n    return declassify values\n",
        ),
        (
            "return_annotation",
            "function invalid(view values: list[int64]) returns view list[int64]:\n    return clone values\n",
        ),
        (
            "unused_generic_return_annotation",
            "function invalid[T](view value: T) returns view T:\n    return clone value\n",
        ),
        (
            "before_explicit_comptime",
            "function invalid(view values: list[int64]) returns list[int64]:\n    return values\nfunction baking() returns string:\n    return comptime string.repeat(\"a\", 9223372036854775807)\n",
        ),
        (
            "verify_alias",
            "function invalid(view values: list[int64]) returns int64:\n    list[int64] borrowed = view values\n    return consume(borrowed)\nverify invalid_check:\n    list[int64] values = list(7)\n    int64 observed = invalid(view values)\n    assert observed == 1\n",
        ),
        (
            "property_alias",
            "function invalid(view values: list[int64]) returns int64:\n    list[int64] borrowed = view values\n    return consume(borrowed)\nproperty invalid_trials:\n    given input: int64\n    list[int64] values = list(input)\n    int64 observed = invalid(view values)\n    assert observed == 1\n",
        ),
    ]
    .into_iter()
    .map(|(name, body)| (name, format!("{DECLARATIONS}{body}{ENTRIES}")))
    .collect()
}

fn assert_rejected(built: BuildResult, name: &str) {
    let errors = built
        .diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.severity == Severity::Error)
        .collect::<Vec<_>>();
    assert!(built.has_errors, "{name}");
    assert_eq!(errors.len(), 1, "{name}: {errors:?}");
    assert_eq!(errors[0].code.code(), 401, "{name}: {errors:?}");
}

#[test]
fn borrowed_values_cannot_escape_before_program_or_suite_execution() {
    for (name, source) in rejected_cases() {
        assert_rejected(build_source(&source, name), name);
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("main.jett");
        fs::write(&path, source).unwrap();
        let failure =
            run_file_capture_outcome(&path).expect_err("view escape prevents main from executing");
        assert!(failure.message.contains("E0401"), "{name}: {failure:?}");
        assert!(failure.output.stdout.is_empty(), "{name}: {failure:?}");
        assert!(
            failure.output.debug_events.is_empty(),
            "{name}: {failure:?}"
        );
        let failure = test_file(&path)
            .err()
            .expect("view escape prevents suite execution");
        assert!(failure.contains("E0401"), "{name}: {failure}");
    }
}

#[test]
fn lexical_verify_and_property_observations_preserve_borrowed_aliases() {
    let cases = [
        (
            "verify_alias",
            "verify observed_check:\n    list[int64] values = list(7)\n    list[int64] borrowed = view values\n    int64 observed = consume(borrowed)\n    assert observed == 1\n    assert list.length(view borrowed) == 1\n    assert list.length(view values) == 1\n",
            "observed_check",
            false,
        ),
        (
            "property_alias",
            "property observed_trials:\n    given input: int64\n    list[int64] values = list(input)\n    list[int64] borrowed = view values\n    int64 observed = consume(borrowed)\n    assert observed == 1\n    assert list.length(view borrowed) == 1\n    assert list.length(view values) == 1\n",
            "observed_trials",
            true,
        ),
    ];
    for (name, body, block_name, is_property) in cases {
        let source = format!("{DECLARATIONS}{body}");
        let built = build_source(&source, name);
        assert!(!built.has_errors, "{name}: {:?}", built.diagnostics);
        assert!(built.debug_observations.is_empty(), "{name}");
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("main.jett");
        fs::write(&path, source).unwrap();
        let outcome = test_file(&path).expect("lexical observation suites should execute");
        assert_eq!(
            (outcome.total, outcome.passed, outcome.failed),
            (1, 1, 0),
            "{name}"
        );
        assert!(outcome.debug_observations.is_empty(), "{name}");
        assert_eq!(outcome.blocks.len(), 1, "{name}");
        let block = &outcome.blocks[0];
        assert_eq!(block.name, block_name, "{name}");
        assert!(block.passed, "{name}: {:?}", block.error);
        assert!(block.error.is_none(), "{name}");
        assert_eq!(block.is_property, is_property, "{name}");
        assert_eq!(
            block.iterations,
            if is_property { Some(100) } else { None },
            "{name}"
        );
        assert!(block.debug_events.is_empty(), "{name}");
    }
}

#[cfg(all(
    target_arch = "x86_64",
    any(
        all(target_os = "windows", target_env = "msvc"),
        all(target_os = "linux", target_env = "gnu")
    )
))]
#[test]
fn borrowed_value_escapes_preserve_existing_native_publications() {
    use jett_driver::native::{
        NativeBuildError, NativeLauncherBundle, build_host_executable_with_options,
        build_host_property_suite_executable, build_host_verify_suite_executable,
    };

    for (name, source) in rejected_cases() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("main.jett");
        let output = directory.path().join("preserved.exe");
        let sentinel = b"existing native publication";
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
                panic!("{name}: expected frontend diagnostics before HIR");
            };
            assert_rejected(built, name);
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
                .expect_err("view escape prevents native publication"),
            );
        }
        check(
            build_host_verify_suite_executable(&path, &launcher, &output)
                .expect_err("view escape prevents native verify publication"),
        );
        check(
            build_host_property_suite_executable(&path, &launcher, &output)
                .expect_err("view escape prevents native property publication"),
        );
    }
}
