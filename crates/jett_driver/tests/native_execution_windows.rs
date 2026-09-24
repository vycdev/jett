//! Link and execute real native programs on the Windows MSVC host.
#![cfg(all(target_os = "windows", target_env = "msvc", target_arch = "x86_64"))]

use jett_driver::native::{NativeLauncherBundle, build_host_executable, host_target};
use jett_runtime::{clock, environment, random};
use std::io::Read;
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::OnceLock;
use std::time::{Duration, Instant};

struct Launcher {
    bundle: NativeLauncherBundle,
    _directory: tempfile::TempDir,
}

fn launcher() -> &'static NativeLauncherBundle {
    static LAUNCHER: OnceLock<Launcher> = OnceLock::new();
    &LAUNCHER
        .get_or_init(|| {
            let executable = std::env::current_exe().expect("test executable");
            let profile_directory = executable.parent().unwrap().parent().unwrap();
            let profile = profile_directory.file_name().unwrap().to_str().unwrap();
            let cargo_profile = if profile == "debug" { "test" } else { profile };
            let target = profile_directory
                .parent()
                .unwrap()
                .join("native-windows-launcher");
            let host = host_target();
            let status = Command::new(env!("CARGO"))
                .args(["build", "-q", "-p", "jett_native_launcher"])
                .args([
                    "--target",
                    &host,
                    "--profile",
                    cargo_profile,
                    "--target-dir",
                ])
                .arg(&target)
                .current_dir(Path::new(env!("CARGO_MANIFEST_DIR")).join("../.."))
                .status()
                .expect("build host- and profile-matched launcher");
            assert!(status.success(), "launcher build failed: {status}");
            let archive = target
                .join(&host)
                .join(profile)
                .join("jett_native_launcher.lib");
            let directory = tempfile::tempdir().expect("launcher bundle directory");
            let copied = directory.path().join("jett_native_launcher.lib");
            std::fs::copy(&archive, &copied).unwrap_or_else(|error| {
                panic!(
                    "cannot copy launcher archive {}: {error}",
                    archive.display()
                )
            });
            Launcher {
                bundle: NativeLauncherBundle::windows_msvc_static_v1(copied),
                _directory: directory,
            }
        })
        .bundle
}

struct ExecutionChild(Child);

impl Drop for ExecutionChild {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn run_bounded(executable: &Path, directory: &Path) -> std::process::Output {
    run_bounded_with_env(executable, directory, None)
}

fn run_bounded_with_env(
    executable: &Path,
    directory: &Path,
    scripted_provider: Option<(&str, &str)>,
) -> std::process::Output {
    // File-backed output also bounds the wait if a child process inherits a handle.
    let mut stdout = tempfile::NamedTempFile::new().unwrap();
    let mut stderr = tempfile::NamedTempFile::new().unwrap();
    let mut command = Command::new(executable);
    command
        .current_dir(directory)
        .env_clear()
        .stdin(Stdio::null())
        .stdout(stdout.reopen().unwrap())
        .stderr(stderr.reopen().unwrap());
    if let Some((name, value)) = scripted_provider {
        command.env(name, value);
    }
    let mut child = ExecutionChild(command.spawn().expect("execute native artifact"));
    let start = Instant::now();
    let status = loop {
        if let Some(status) = child.0.try_wait().expect("poll native artifact") {
            break status;
        }
        assert!(
            start.elapsed() < Duration::from_secs(10),
            "native executable exceeded 10 second deadline"
        );
        std::thread::sleep(Duration::from_millis(10));
    };
    let snapshot = |file: &mut std::fs::File| {
        let mut bytes = Vec::new();
        file.take(file.metadata().unwrap().len())
            .read_to_end(&mut bytes)
            .unwrap();
        bytes
    };
    std::process::Output {
        status,
        stdout: snapshot(stdout.as_file_mut()),
        stderr: snapshot(stderr.as_file_mut()),
    }
}

#[test]
fn native_scripted_capabilities_match_interpreter() {
    let clock_samples = vec![
        clock::ClockTestSample::Wall {
            unix_seconds: 0,
            subsecond_nanoseconds: 0,
        },
        clock::ClockTestSample::Wall {
            unix_seconds: 0,
            subsecond_nanoseconds: 0,
        },
        clock::ClockTestSample::Wall {
            unix_seconds: -1,
            subsecond_nanoseconds: 999_999_999,
        },
        clock::ClockTestSample::Wall {
            unix_seconds: 42,
            subsecond_nanoseconds: 123_456_789,
        },
        clock::ClockTestSample::Wall {
            unix_seconds: 40,
            subsecond_nanoseconds: 0,
        },
    ];
    let clock_fixture =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/run_pass/clock_scripted.jett");
    let clock_expected = jett_driver::run_file_capture_stdout_with_clock_test_samples(
        &clock_fixture,
        clock_samples.clone(),
    )
    .expect("scripted Clock interpreter oracle");
    let clock_script = clock::encode_test_script(&clock_samples);
    assert_scripted_fixture(
        &clock_fixture,
        clock::TEST_SCRIPT_ENV,
        &clock_script,
        &clock_expected,
    );

    let random_samples = vec![
        random::RandomTestSample::Bounded(0),
        random::RandomTestSample::Bounded(u64::MAX - 1),
        random::RandomTestSample::Unit53(0),
        random::RandomTestSample::Unit53((1_u64 << 53) - 1),
        random::RandomTestSample::Boolean(false),
        random::RandomTestSample::Boolean(true),
        random::RandomTestSample::Bounded(0),
        random::RandomTestSample::Bounded(2),
        random::RandomTestSample::Bounded(0),
        random::RandomTestSample::Bounded(1),
        random::RandomTestSample::Bounded(0),
    ];
    let random_fixture =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/run_pass/random_scripted.jett");
    let random_expected = jett_driver::run_file_capture_stdout_with_random_test_samples(
        &random_fixture,
        random_samples.clone(),
    )
    .expect("scripted Random interpreter oracle");
    let random_script = random::encode_test_script(&random_samples);
    assert_scripted_fixture(
        &random_fixture,
        random::TEST_SCRIPT_ENV,
        &random_script,
        &random_expected,
    );

    let text = |value: &str| environment::EnvironmentTestText::Unicode(value.to_owned());
    let environment_snapshot = environment::EnvironmentTestSnapshot {
        arguments: vec![text("first"), text(""), text("third")],
        entries: vec![
            environment::EnvironmentTestEntry {
                name: text("PRESENT"),
                value: text("value"),
            },
            environment::EnvironmentTestEntry {
                name: text("DUPLICATE"),
                value: text("first"),
            },
            environment::EnvironmentTestEntry {
                name: text("DUPLICATE"),
                value: text("second"),
            },
            environment::EnvironmentTestEntry {
                name: text("BROKEN"),
                value: environment::EnvironmentTestText::InvalidUnicode,
            },
            environment::EnvironmentTestEntry {
                name: environment::EnvironmentTestText::InvalidUnicode,
                value: text("ignored"),
            },
        ],
    };
    let environment_fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/run_pass/environment_snapshot.jett");
    let environment_expected = jett_driver::run_file_capture_stdout_with_environment_test_snapshot(
        &environment_fixture,
        environment_snapshot.clone(),
    )
    .expect("injected Environment interpreter oracle");
    let environment_script = environment::encode_test_snapshot(&environment_snapshot);
    assert_scripted_fixture(
        &environment_fixture,
        environment::TEST_SNAPSHOT_ENV,
        &environment_script,
        &environment_expected,
    );
}

fn assert_scripted_fixture(fixture: &Path, env: &str, script: &str, expected: &str) {
    let directory = tempfile::tempdir().expect("isolated execution directory");
    let binary = directory.path().join("program.exe");
    build_host_executable(fixture, launcher(), &binary).expect("compile scripted fixture");
    let actual = run_bounded_with_env(&binary, directory.path(), Some((env, script)));
    assert!(actual.status.success(), "{}: {actual:?}", fixture.display());
    assert_eq!(actual.stdout, expected.as_bytes(), "{}", fixture.display());
    assert!(
        actual.stderr.is_empty(),
        "{}: {actual:?}",
        fixture.display()
    );
}

#[test]
fn native_scalar_stdout_and_owned_bytes_match_interpreter() {
    for (name, relative_path) in [
        (
            "native_scalar_entry",
            "../../tests/run_pass/native_scalar_entry.jett",
        ),
        ("hello_print", "../../tests/run_pass/hello_print.jett"),
        ("bytes_ownership", "../../tests/native/bytes_ownership.jett"),
        ("string_search", "../../tests/native/string_search.jett"),
        (
            "string_intrinsics",
            "../../tests/native/string_intrinsics.jett",
        ),
        ("unit_enum", "../../tests/native/unit_enum.jett"),
        ("payload_enum", "../../tests/native/payload_enum.jett"),
        (
            "payload_enum_equality",
            "../../tests/native/payload_enum_equality.jett",
        ),
        ("bitfield_values", "../../tests/native/bitfield_values.jett"),
        (
            "contextual_generic_empty_list",
            "../../tests/native/contextual_generic_empty_list.jett",
        ),
        ("machine_values", "../../tests/native/machine_values.jett"),
        ("uuid_values", "../../tests/native/uuid_values.jett"),
        ("function_values", "../../tests/native/function_values.jett"),
        (
            "inline_functions",
            "../../tests/native/inline_functions.jett",
        ),
        (
            "list_source_values",
            "../../tests/native/list_source_values.jett",
        ),
        ("list_sort", "../../tests/native/list_sort.jett"),
        (
            "list_insert_remove",
            "../../tests/native/list_insert_remove.jett",
        ),
        ("set_values", "../../tests/native/set_values.jett"),
        ("map_values", "../../tests/native/map_values.jett"),
        (
            "json_generic_raw_bridge",
            "../../tests/native/json_generic_raw_bridge.jett",
        ),
        (
            "json_generic_primitives",
            "../../tests/native/json_generic_primitives.jett",
        ),
        (
            "json_struct_serialize",
            "../../tests/native/json_struct_serialize.jett",
        ),
        (
            "json_aggregate_parse",
            "../../tests/native/json_aggregate_parse.jett",
        ),
        (
            "json_result_parse",
            "../../tests/native/json_result_parse.jett",
        ),
        ("json_set_parse", "../../tests/native/json_set_parse.jett"),
        (
            "json_struct_parse",
            "../../tests/native/json_struct_parse.jett",
        ),
        (
            "json_generic_parse_primitives",
            "../../tests/native/json_generic_parse_primitives.jett",
        ),
        ("encoding", "../../tests/native/encoding.jett"),
        ("csv", "../../tests/native/csv.jett"),
        ("secret_values", "../../tests/native/secret_values.jett"),
        (
            "reflection_scalars",
            "../../tests/native/reflection_scalars.jett",
        ),
        ("type_info", "../../tests/native/type_info.jett"),
        ("type_fields", "../../tests/native/type_fields.jett"),
        (
            "type_construction_struct",
            "../../tests/native/type_construction_struct.jett",
        ),
        (
            "type_construction_bitfield",
            "../../tests/native/type_construction_bitfield.jett",
        ),
        (
            "type_construction_enum",
            "../../tests/native/type_construction_enum.jett",
        ),
        ("type_variants", "../../tests/native/type_variants.jett"),
        (
            "reflected_type_dispatch",
            "../../tests/native/reflected_type_dispatch.jett",
        ),
        (
            "projected_sequences",
            "../../tests/native/projected_sequences.jett",
        ),
        (
            "bitfield_reflection",
            "../../tests/native/bitfield_reflection.jett",
        ),
        ("crypto", "../../tests/native/crypto.jett"),
        ("math_aggregate", "../../tests/native/math_aggregate.jett"),
        (
            "clock_production",
            "../../tests/native/clock_production.jett",
        ),
        (
            "string_scalar_iteration",
            "../../tests/native/string_scalar_iteration.jett",
        ),
        (
            "nested_handle_view_call",
            "../../tests/run_pass/uint64_checked_expression_runtime_main.jett",
        ),
        (
            "nested_handler_call",
            "../../tests/native/nested_handler_call.jett",
        ),
        (
            "nested_handler_order",
            "../../tests/native/nested_handler_order.jett",
        ),
        (
            "numeric_conversions",
            "../../tests/native/numeric_conversions.jett",
        ),
        (
            "refinement_validation",
            "../../tests/native/refinement_validation.jett",
        ),
        (
            "refinement_collections",
            "../../tests/native/refinement_collections.jett",
        ),
        (
            "empty_never_list",
            "../../tests/native/empty_never_list.jett",
        ),
    ] {
        let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join(relative_path);
        let expected = jett_driver::run_file_capture_output(&fixture).expect("interpreter oracle");
        if name == "uuid_values" {
            assert_eq!(expected.stdout, "true true true true\n");
        }
        if name == "function_values" {
            assert_eq!(expected.stdout, "8 14 4 3 z a b\n");
        }
        if name == "list_source_values" {
            assert_eq!(expected.stdout, "a true true true true 2 true true true\n");
        }
        let directory = tempfile::tempdir().expect("isolated execution directory");
        let binary = directory.path().join("program.exe");
        build_host_executable(&fixture, launcher(), &binary)
            .unwrap_or_else(|error| panic!("{name}: {error:?}"));
        let actual = run_bounded(&binary, directory.path());
        assert!(actual.status.success(), "{name}: {actual:?}");
        assert_eq!(
            actual.stdout,
            expected.stdout.as_bytes(),
            "{name}: {actual:?}"
        );
        assert!(actual.stderr.is_empty(), "{name}: {actual:?}");
    }
}

#[test]
fn native_generic_struct_fields_match_interpreter() {
    let source = include_str!("../../../tests/run_pass/generic_struct.jett");
    let directory = tempfile::tempdir().expect("isolated execution directory");
    let source_path = directory.path().join("generic_struct.jett");
    std::fs::write(
        &source_path,
        format!(
            "{source}\nfunction main() returns nothing:\n    println(pair_first_test(), pair_second_test(), box_value_test())\n"
        ),
    )
    .expect("write struct fixture with an executable entry");
    let expected = jett_driver::run_file_capture_output(&source_path).expect("interpreter oracle");
    let binary = directory.path().join("program.exe");
    build_host_executable(&source_path, launcher(), &binary)
        .expect("compile generic struct fixture");
    std::fs::remove_file(&source_path).expect("source is unnecessary at runtime");
    let actual = run_bounded(&binary, directory.path());
    assert!(actual.status.success(), "{actual:?}");
    assert_eq!(actual.stdout, expected.stdout.as_bytes());
    assert!(actual.stderr.is_empty(), "{actual:?}");
}

#[test]
fn native_uint64_checked_expression_dispatch_matches_interpreter() {
    let source =
        include_str!("../../../tests/run_pass/uint64_checked_expression_runtime_types.jett");
    let directory = tempfile::tempdir().expect("isolated execution directory");
    let source_path = directory.path().join("uint64_dispatch.jett");
    std::fs::write(
        &source_path,
        format!(
            "{source}\nfunction main() returns nothing:\n    println(constructed_list_uint64_dispatch(), appended_list_uint64_dispatch(), map_value_uint64_dispatch(), set_to_list_uint64_dispatch(), optional_uint64_dispatch(), result_uint64_dispatch())\n"
        ),
    )
    .expect("write uint64 dispatch fixture with an executable entry");
    let expected = jett_driver::run_file_capture_output(&source_path).expect("interpreter oracle");
    let binary = directory.path().join("program.exe");
    build_host_executable(&source_path, launcher(), &binary)
        .expect("compile uint64 dispatch fixture");
    let actual = run_bounded(&binary, directory.path());
    assert!(actual.status.success(), "{actual:?}");
    assert_eq!(actual.stdout, expected.stdout.as_bytes());
    assert!(actual.stderr.is_empty(), "{actual:?}");
}

#[test]
fn native_math_aggregates_preserve_interpreter_extremes() {
    let source = include_str!("../../../tests/run_pass/math_extra.jett");
    let directory = tempfile::tempdir().expect("isolated execution directory");
    let source_path = directory.path().join("math_extra.jett");
    std::fs::write(
        &source_path,
        format!(
            "{source}\nfunction main() returns nothing:\n    println(average_test(), average_finite_extremes_test(), average_preserves_small_residual_test(), median_odd_test(), median_even_test(), median_finite_extremes_test())\n"
        ),
    )
    .expect("write math aggregate fixture with an executable entry");
    let expected = jett_driver::run_file_capture_output(&source_path).expect("interpreter oracle");
    let binary = directory.path().join("program.exe");
    build_host_executable(&source_path, launcher(), &binary)
        .expect("compile math aggregate fixture");
    let actual = run_bounded(&binary, directory.path());
    assert!(actual.status.success(), "{actual:?}");
    assert_eq!(actual.stdout, expected.stdout.as_bytes());
    assert!(actual.stderr.is_empty(), "{actual:?}");
}

#[test]
fn native_int64_debug_statements_match_interpreter_output() {
    for relative_path in [
        "../../tests/run_pass/trace_basic.jett",
        "../../tests/native/trace_int64.jett",
        "../../tests/run_pass/breakpoint_basic.jett",
        "../../tests/native/breakpoint_int64.jett",
    ] {
        let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join(relative_path);
        let expected = jett_driver::run_file_capture_output(&fixture).expect("interpreter oracle");
        let directory = tempfile::tempdir().expect("isolated execution directory");
        let binary = directory.path().join("program.exe");
        build_host_executable(&fixture, launcher(), &binary).expect("compile int64 trace fixture");
        let actual = run_bounded(&binary, directory.path());
        assert!(actual.status.success(), "{relative_path}: {actual:?}");
        assert_eq!(actual.stdout, expected.stdout.as_bytes(), "{relative_path}");
        let debug = format!("{}\n", expected.debug_output.join("\n"));
        assert_eq!(actual.stderr, debug.as_bytes(), "{relative_path}");
    }
}
