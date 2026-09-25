//! Link and execute real native programs on the Windows MSVC host.
#![cfg(all(target_os = "windows", target_env = "msvc", target_arch = "x86_64"))]

use jett_common::FileId;
use jett_driver::native::{
    NativeLauncherBundle, build_host_executable, build_host_property_suite_executable,
    build_host_verify_suite_executable, host_target,
};
use jett_parser::ast::Item;
use jett_runtime::{clock, environment, graphics, random};
use std::fs;
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
fn native_verify_suite_executes_all_checked_bodies() {
    let fixture =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/run_pass/verify_test.jett");
    let directory = tempfile::tempdir().expect("verify executable directory");
    let executable = directory.path().join("verify_test.exe");
    let artifact = build_host_verify_suite_executable(&fixture, launcher(), &executable)
        .expect("link native verify suite");
    assert_eq!(artifact.path, executable);
    let output = run_bounded(&artifact.path, directory.path());
    assert!(output.status.success(), "native verify failed: {output:?}");
    assert!(
        output.stdout.is_empty(),
        "verify emitted stdout: {output:?}"
    );
    assert!(
        output.stderr.is_empty(),
        "verify emitted stderr: {output:?}"
    );
}

#[test]
fn native_bitfield_constructor_checks_dynamic_widths() {
    let fixture =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/run_pass/bitfield_roundtrip.jett");
    let directory = tempfile::tempdir().expect("bitfield verify directory");
    let executable = directory.path().join("bitfield_widths.exe");
    let artifact = build_host_verify_suite_executable(&fixture, launcher(), &executable)
        .expect("link native bitfield verify suite");
    let output = run_bounded(&artifact.path, directory.path());
    assert!(
        output.status.success(),
        "native bitfield checks failed: {output:?}"
    );
}

#[test]
fn native_property_suite_executes_deterministic_scalar_trials() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/run_pass/namespace_runtime_verify_context.jett");
    let directory = tempfile::tempdir().expect("property executable directory");
    let executable = directory.path().join("scalar_property_suite.exe");
    let artifact = build_host_property_suite_executable(&fixture, launcher(), &executable)
        .expect("link native scalar property suite");
    let output = run_bounded(&artifact.path, directory.path());
    assert!(
        output.status.success(),
        "native properties failed: {output:?}"
    );
}

#[test]
fn native_property_suites_execute_for_all_run_pass_fixtures() {
    let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/run_pass");
    let mut sources = fs::read_dir(&fixtures)
        .expect("run-pass fixture directory")
        .map(|entry| entry.expect("fixture entry").path())
        .filter(|path| {
            path.extension()
                .is_some_and(|extension| extension == "jett")
        })
        .collect::<Vec<_>>();
    sources.sort();
    let directory = tempfile::tempdir().expect("property audit directory");
    let mut attempted = 0;
    let mut property_blocks = 0;
    let mut passed = 0;
    let mut failures = Vec::new();
    for source in sources {
        let text = fs::read_to_string(&source).expect("fixture source");
        let parsed = jett_parser::parse(&text, FileId::new(0));
        let count = parsed
            .module
            .items
            .iter()
            .filter(|item| matches!(item, Item::Property(_)))
            .count();
        if count == 0 {
            continue;
        }
        attempted += 1;
        property_blocks += count;
        let executable = directory.path().join(format!(
            "{}.exe",
            source.file_stem().expect("fixture stem").to_string_lossy()
        ));
        match build_host_property_suite_executable(&source, launcher(), &executable) {
            Ok(artifact) => {
                let output = run_bounded(&artifact.path, directory.path());
                if output.status.success() {
                    passed += 1;
                } else {
                    failures.push(format!("{}: {output:?}", source.display()));
                }
                fs::remove_file(&artifact.path).expect("remove finished property executable");
            }
            Err(error) => failures.push(format!("{}: {error}", source.display())),
        }
    }
    println!("native property suites: {passed}/{attempted}; {property_blocks} blocks");
    for failure in &failures {
        println!("{failure}");
    }
    assert_eq!(attempted, 3, "native property fixture denominator changed");
    assert_eq!(
        property_blocks, 18,
        "native property block denominator changed"
    );
    assert_eq!(passed, attempted, "native property suite failures");
}

#[test]
fn native_verify_suite_keeps_inline_callbacks_inside_their_parent_body() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/run_pass/json_public_secret_policy.jett");
    let directory = tempfile::tempdir().expect("verify executable directory");
    let executable = directory.path().join("json_public_secret_policy.exe");
    let artifact = build_host_verify_suite_executable(&fixture, launcher(), &executable)
        .expect("link native verify suite with inline callbacks");
    let output = run_bounded(&artifact.path, directory.path());
    assert!(output.status.success(), "native verify failed: {output:?}");
}

#[test]
fn native_verify_suites_execute_for_all_run_pass_fixtures() {
    let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/run_pass");
    let mut sources = fs::read_dir(&fixtures)
        .expect("run-pass fixture directory")
        .map(|entry| entry.expect("fixture entry").path())
        .filter(|path| {
            path.extension()
                .is_some_and(|extension| extension == "jett")
        })
        .collect::<Vec<_>>();
    sources.sort();
    let directory = tempfile::tempdir().expect("native verify audit directory");
    let mut attempted = 0;
    let mut passed = 0;
    let mut failures = Vec::new();
    for source in sources {
        let text = fs::read_to_string(&source).expect("fixture source");
        let parsed = jett_parser::parse(&text, FileId::new(0));
        if !parsed
            .module
            .items
            .iter()
            .any(|item| matches!(item, Item::Verify(_)))
        {
            continue;
        }
        attempted += 1;
        let executable = directory.path().join(format!(
            "{}.exe",
            source.file_stem().expect("fixture stem").to_string_lossy()
        ));
        match build_host_verify_suite_executable(&source, launcher(), &executable) {
            Ok(artifact) => {
                let output = run_bounded(&artifact.path, directory.path());
                if output.status.success() {
                    passed += 1;
                } else {
                    failures.push(format!("{}: {output:?}", source.display()));
                }
                fs::remove_file(&artifact.path).expect("remove finished verify executable");
            }
            Err(error) => failures.push(format!("{}: {error}", source.display())),
        }
    }
    println!("native verify suites: {passed}/{attempted}");
    for failure in &failures {
        println!("{failure}");
    }
    assert_eq!(attempted, 155, "native verify fixture denominator changed");
    assert_eq!(passed, attempted, "native verify suite failures");
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
            "captured_closure_values",
            "../../tests/native/captured_closure_values.jett",
        ),
        (
            "function_descriptor_ownership",
            "../../tests/native/function_descriptor_ownership.jett",
        ),
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
            "json_machine_parse",
            "../../tests/native/json_machine_parse.jett",
        ),
        (
            "json_machine_serialize",
            "../../tests/native/json_machine_serialize.jett",
        ),
        (
            "json_secret_source",
            "../../tests/native/json_secret_source.jett",
        ),
        (
            "json_enum_source",
            "../../tests/native/json_enum_source.jett",
        ),
        (
            "json_enum_raw_source",
            "../../tests/native/json_enum_raw_source.jett",
        ),
        (
            "json_pipeline_source",
            "../../tests/native/json_pipeline_source.jett",
        ),
        (
            "json_refinement_source",
            "../../tests/native/json_refinement_source.jett",
        ),
        (
            "json_refined_record_source",
            "../../tests/native/json_refined_record_source.jett",
        ),
        (
            "json_bytes_raw_source",
            "../../tests/native/json_bytes_raw_source.jett",
        ),
        (
            "json_bitfield_source",
            "../../tests/native/json_bitfield_source.jett",
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
        (
            "type_construction_machine",
            "../../tests/native/type_construction_machine.jett",
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
        if name == "function_descriptor_ownership" {
            assert_eq!(expected.stdout, "5 8\n10\n12\n");
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
fn native_actor_spawn_releases_state_with_context() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/native/actor_spawn.jett");
    let expected = jett_driver::run_file_capture_output(&fixture).expect("interpreter oracle");
    assert_eq!(expected.stdout, "spawned\n");
    let directory = tempfile::tempdir().expect("isolated execution directory");
    let binary = directory.path().join("program.exe");
    build_host_executable(&fixture, launcher(), &binary).expect("compile actor spawn");
    let actual = run_bounded(&binary, directory.path());
    assert!(actual.status.success(), "{actual:?}");
    assert_eq!(actual.stdout, expected.stdout.as_bytes());
    assert!(actual.stderr.is_empty(), "{actual:?}");
}

#[test]
fn native_actor_messages_match_interpreter() {
    let fixture =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/native/actor_messages.jett");
    let expected = jett_driver::run_file_capture_output(&fixture).expect("interpreter oracle");
    assert_eq!(expected.stdout, "hits: 5\n");
    let directory = tempfile::tempdir().expect("isolated execution directory");
    let binary = directory.path().join("program.exe");
    build_host_executable(&fixture, launcher(), &binary).expect("compile actor messages");
    let actual = run_bounded(&binary, directory.path());
    assert!(actual.status.success(), "{actual:?}");
    assert_eq!(actual.stdout, expected.stdout.as_bytes());
    assert!(actual.stderr.is_empty(), "{actual:?}");
}

#[test]
fn native_graphics_authority_reaches_main() {
    let fixture =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/native/graphics_authority.jett");
    let expected = jett_driver::run_file_capture_output(&fixture).expect("interpreter oracle");
    assert_eq!(expected.stdout, "graphics authority granted\n");
    let directory = tempfile::tempdir().expect("isolated execution directory");
    let binary = directory.path().join("program.exe");
    build_host_executable(&fixture, launcher(), &binary).expect("compile Graphics entry");
    let actual = run_bounded(&binary, directory.path());
    assert!(actual.status.success(), "{actual:?}");
    assert_eq!(actual.stdout, expected.stdout.as_bytes());
    assert!(actual.stderr.is_empty(), "{actual:?}");
}

#[test]
fn native_scripted_graphics_matches_interpreter() {
    for (name, events) in [
        (
            "graphics_scripted.jett",
            vec![
                graphics::TestEvent::Key(graphics::Key::Right),
                graphics::TestEvent::Key(graphics::Key::Right),
                graphics::TestEvent::Key(graphics::Key::Left),
                graphics::TestEvent::Close,
            ],
        ),
        (
            "graphics_pipeline_scripted.jett",
            vec![
                graphics::TestEvent::Key(graphics::Key::Right),
                graphics::TestEvent::Close,
                graphics::TestEvent::Key(graphics::Key::Left),
                graphics::TestEvent::Close,
                graphics::TestEvent::Key(graphics::Key::Right),
                graphics::TestEvent::Close,
            ],
        ),
    ] {
        let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/run_pass")
            .join(name);
        let expected = jett_driver::run_file_capture_output_with_graphics_test_events(
            &fixture,
            events.clone(),
        )
        .expect("scripted Graphics interpreter oracle");
        let script = graphics::encode_test_script(&events);
        assert_scripted_fixture(
            &fixture,
            graphics::TEST_SCRIPT_ENV,
            &script,
            &expected.stdout,
        );
    }
}

#[test]
fn native_graphics_callback_runtime_error_is_terminal() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/run_pass/graphics_callback_runtime_error.jett");
    let events = vec![graphics::TestEvent::Key(graphics::Key::Right)];
    let expected =
        jett_driver::run_file_capture_outcome_with_graphics_test_events(&fixture, events.clone())
            .expect_err("interpreter callback must fail");
    let directory = tempfile::tempdir().expect("isolated execution directory");
    let binary = directory.path().join("program.exe");
    build_host_executable(&fixture, launcher(), &binary)
        .expect("compile Graphics callback fixture");
    let script = graphics::encode_test_script(&events);
    let actual = run_bounded_with_env(
        &binary,
        directory.path(),
        Some((graphics::TEST_SCRIPT_ENV, &script)),
    );
    assert!(!actual.status.success(), "{actual:?}");
    assert_eq!(actual.stdout, expected.output.stdout.as_bytes());
    assert_eq!(
        actual.stderr,
        b"runtime error: string.repeat: requested output is too large\n"
    );
}

#[test]
fn native_graphics_handled_failures_match_interpreter() {
    let fixture =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/run_pass/graphics_scripted.jett");
    let source = std::fs::read_to_string(&fixture).expect("read Graphics fixture");
    let cases = [
        (
            "config",
            source.replacen("width: 32", "width: 0", 1),
            vec![],
        ),
        (
            "scene",
            source.replacen("red: state.moves", "red: 999", 1),
            vec![],
        ),
        (
            "host",
            source.clone(),
            vec![graphics::TestEvent::HostError(
                "injected graphics failure".into(),
            )],
        ),
    ];
    for (name, source, events) in cases {
        let directory = tempfile::tempdir().expect("isolated Graphics fixture");
        let path = directory.path().join(format!("{name}.jett"));
        std::fs::write(&path, source).expect("write Graphics fixture");
        let expected =
            jett_driver::run_file_capture_output_with_graphics_test_events(&path, events.clone())
                .expect("Graphics interpreter oracle");
        let script = graphics::encode_test_script(&events);
        assert_scripted_fixture(&path, graphics::TEST_SCRIPT_ENV, &script, &expected.stdout);
    }
}

#[test]
fn native_actor_fixture_bodies_match_interpreter() {
    for (fixture, main) in [
        (
            "actor_named_arguments.jett",
            "function main() returns nothing:\n    println(actor_results())\n",
        ),
        (
            "namespace_duplicate_leaf_actors.jett",
            "function main() returns nothing:\n    println(direct_duplicate_leaf_actors(), alias_duplicate_leaf_actors())\n",
        ),
        (
            "namespace_qualified_actors.jett",
            "function main() returns nothing:\n    println(direct_actor(), alias_actor())\n",
        ),
        (
            "numeric_literal_contexts.jett",
            "function main() returns nothing:\n    ByteCounter counter = spawn ByteCounter(seed: 4 + 5)\n    send counter.add(10 + 11)\n    println(ask counter.current)\n",
        ),
    ] {
        let source = std::fs::read_to_string(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../tests/run_pass")
                .join(fixture),
        )
        .expect("read actor fixture");
        let directory = tempfile::tempdir().expect("isolated actor execution directory");
        let source_path = directory.path().join(fixture);
        std::fs::write(&source_path, format!("{source}\n{main}"))
            .expect("write executable actor fixture");
        let expected =
            jett_driver::run_file_capture_output(&source_path).expect("interpreter oracle");
        let binary = directory.path().join("program.exe");
        build_host_executable(&source_path, launcher(), &binary)
            .unwrap_or_else(|error| panic!("{fixture}: {error:?}"));
        let actual = run_bounded(&binary, directory.path());
        assert!(actual.status.success(), "{fixture}: {actual:?}");
        assert_eq!(
            actual.stdout,
            expected.stdout.as_bytes(),
            "{fixture}: {actual:?}"
        );
        assert!(actual.stderr.is_empty(), "{fixture}: {actual:?}");
    }
}

#[test]
fn native_structured_concurrency_matches_interpreter() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/native/structured_concurrency.jett");
    let expected = jett_driver::run_file_capture_output(&fixture).expect("interpreter oracle");
    assert_eq!(expected.stdout, "hello:ready:failed\n");
    let directory = tempfile::tempdir().expect("isolated execution directory");
    let binary = directory.path().join("program.exe");
    build_host_executable(&fixture, launcher(), &binary).expect("compile structured concurrency");
    let actual = run_bounded(&binary, directory.path());
    assert!(actual.status.success(), "{actual:?}");
    assert_eq!(actual.stdout, expected.stdout.as_bytes());
    assert!(actual.stderr.is_empty(), "{actual:?}");
}

#[test]
fn native_captured_closures_match_interpreter() {
    for (fixture, main) in [
        (
            "closures.jett",
            "function main() returns nothing:\n    println(test_make_adder(), test_linear(), test_count_above(), test_nested_closures())\n",
        ),
        (
            "closures_advanced.jett",
            "function main() returns nothing:\n    println(test_explicit_struct_parameter(), test_closure_as_argument(), test_conditional_closure_add(), test_conditional_closure_mul(), sum_with_closure(), accumulate_squares(), make_inc_and_dec(5), double_all_sum())\n",
        ),
        ("captured_local_function_name.jett", ""),
    ] {
        let source = std::fs::read_to_string(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../tests/run_pass")
                .join(fixture),
        )
        .expect("read closure fixture");
        let directory = tempfile::tempdir().expect("isolated closure execution directory");
        let source_path = directory.path().join(fixture);
        std::fs::write(&source_path, format!("{source}\n{main}"))
            .expect("write closure fixture with an executable entry");
        let expected =
            jett_driver::run_file_capture_output(&source_path).expect("interpreter oracle");
        let binary = directory.path().join("program.exe");
        build_host_executable(&source_path, launcher(), &binary)
            .unwrap_or_else(|error| panic!("{fixture}: {error:?}"));
        let actual = run_bounded(&binary, directory.path());
        assert!(actual.status.success(), "{fixture}: {actual:?}");
        assert_eq!(
            actual.stdout,
            expected.stdout.as_bytes(),
            "{fixture}: {actual:?}"
        );
        assert!(actual.stderr.is_empty(), "{fixture}: {actual:?}");
    }
}

#[test]
fn native_nested_json_shapes_match_interpreter() {
    for (fixture, main) in [
        (
            "namespace_use_alias.jett",
            "function main() returns nothing:\n    println(namespace_alias_summary())\n",
        ),
        (
            "json_shape_matrix.jett",
            "function main() returns nothing:\n    println(parse_matrix_summary(), parse_matrix_result_fail(), serialize_matrix_shape_checks(), parse_exact_accepts_matrix_shape(), parse_exact_rejects_nested_shape_extra())\n",
        ),
        (
            "json_tree_reflection_parse_wrapper.jett",
            "function main() returns nothing:\n    println(tree_reflected_parse_summary(), tree_reflected_missing_error(), tree_reflected_wrong_item_error(), tree_reflected_full_summary(), tree_reflected_result_failure(), tree_reflected_invalid_refinement_message(), tree_reflected_invalid_bitfield_message())\n",
        ),
    ] {
        let source = std::fs::read_to_string(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../tests/run_pass")
                .join(fixture),
        )
        .expect("read nested JSON fixture");
        let directory = tempfile::tempdir().expect("isolated nested JSON execution directory");
        let source_path = directory.path().join(fixture);
        std::fs::write(&source_path, format!("{source}\n{main}"))
            .expect("write nested JSON fixture with an executable entry");
        let expected =
            jett_driver::run_file_capture_output(&source_path).expect("interpreter oracle");
        let binary = directory.path().join("program.exe");
        build_host_executable(&source_path, launcher(), &binary)
            .unwrap_or_else(|error| panic!("{fixture}: {error:?}"));
        let actual = run_bounded(&binary, directory.path());
        assert!(actual.status.success(), "{fixture}: {actual:?}");
        assert_eq!(
            actual.stdout,
            expected.stdout.as_bytes(),
            "{fixture}: {actual:?}"
        );
        assert!(actual.stderr.is_empty(), "{fixture}: {actual:?}");
    }
}

#[test]
fn native_nested_json_public_serializer_omits_secret_collections() {
    let fixture = "json_reflection_nested_serializer.jett";
    let source = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/run_pass")
            .join(fixture),
    )
    .expect("read nested serializer fixture");
    let directory = tempfile::tempdir().expect("isolated nested serializer directory");
    let source_path = directory.path().join(fixture);
    std::fs::write(
        &source_path,
        format!(
            "{source}\nfunction main() returns nothing:\n    println(reflected_user_json(), reflected_alias_refinement_json(), reflected_enum_json(), reflected_bitfield_json(), reflected_control_string_json(), reflected_all_control_string_json(), reflected_unicode_roundtrip_hex())\n"
        ),
    )
    .expect("write nested serializer executable");
    let expected = jett_driver::run_file_capture_output(&source_path).expect("interpreter oracle");
    let binary = directory.path().join("program.exe");
    build_host_executable(&source_path, launcher(), &binary)
        .unwrap_or_else(|error| panic!("{fixture}: {error:?}"));
    let actual = run_bounded(&binary, directory.path());
    assert!(actual.status.success(), "{fixture}: {actual:?}");
    assert_eq!(
        actual.stdout,
        expected.stdout.as_bytes(),
        "{fixture}: {actual:?}"
    );
    assert!(actual.stderr.is_empty(), "{fixture}: {actual:?}");
}

#[test]
fn native_nested_json_decoder_matches_interpreter() {
    let fixture = "json_reflection_nested_decoder.jett";
    let source = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/run_pass")
            .join(fixture),
    )
    .expect("read nested decoder fixture");
    let directory = tempfile::tempdir().expect("isolated nested decoder directory");
    let source_path = directory.path().join(fixture);
    std::fs::write(
        &source_path,
        format!(
            "{source}\nfunction main() returns nothing:\n    println(decoded_nested_summary(), decoded_result_failure(), invalid_refinement_message(), missing_nested_message(), wrong_struct_shape_message(), wrong_list_item_message(), invalid_bitfield_message(), invalid_enum_payload_message(), invalid_bitfield_enum_message(), decoded_top_level_refinement(), invalid_top_level_refinement_message(), decoded_refined_list_count(), invalid_refined_list_message(), decoded_top_level_float(), decoded_null_as_nothing(), invalid_nothing_message(), decoded_top_level_bytes_length(), decoded_blob_bytes_first(), invalid_bytes_message(), decoded_top_level_secret_redaction(), decoded_secret_field_summary(), invalid_secret_field_message(), missing_secret_field_message())\n"
        ),
    )
    .expect("write nested decoder executable");
    let expected = jett_driver::run_file_capture_output(&source_path).expect("interpreter oracle");
    let binary = directory.path().join("program.exe");
    build_host_executable(&source_path, launcher(), &binary)
        .unwrap_or_else(|error| panic!("{fixture}: {error:?}"));
    let actual = run_bounded(&binary, directory.path());
    assert!(actual.status.success(), "{fixture}: {actual:?}");
    assert_eq!(
        actual.stdout,
        expected.stdout.as_bytes(),
        "{fixture}: {actual:?}"
    );
    assert!(actual.stderr.is_empty(), "{fixture}: {actual:?}");
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
fn native_alias_construction_matches_interpreter() {
    let fixture =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/native/alias_construction.jett");
    let expected = jett_driver::run_file_capture_output(&fixture).expect("interpreter oracle");
    let directory = tempfile::tempdir().expect("isolated execution directory");
    let binary = directory.path().join("program.exe");
    build_host_executable(&fixture, launcher(), &binary).expect("compile alias construction");
    let actual = run_bounded(&binary, directory.path());
    assert!(actual.status.success(), "{actual:?}");
    assert_eq!(actual.stdout, expected.stdout.as_bytes());
    assert!(actual.stderr.is_empty(), "{actual:?}");
}

#[test]
fn native_refined_struct_builder_matches_interpreter() {
    let source = include_str!("../../../tests/run_pass/type_construction_builder.jett");
    let directory = tempfile::tempdir().expect("isolated execution directory");
    let source_path = directory.path().join("refined_struct_builder.jett");
    std::fs::write(
        &source_path,
        format!(
            "{source}\nfunction main() returns nothing:\n    println(clone_user_by_reflection(), clone_generic_box_by_reflection(), clone_refined_user_by_reflection(), missing_field_message(), duplicate_field_message(), mismatched_metadata_message(), mismatched_finish_target_message())\n"
        ),
    )
    .expect("write reflected builder executable");
    let expected = jett_driver::run_file_capture_output(&source_path).expect("interpreter oracle");
    let binary = directory.path().join("program.exe");
    build_host_executable(&source_path, launcher(), &binary)
        .expect("compile reflected builder executable");
    let actual = run_bounded(&binary, directory.path());
    assert!(actual.status.success(), "{actual:?}");
    assert_eq!(actual.stdout, expected.stdout.as_bytes());
    assert!(actual.stderr.is_empty(), "{actual:?}");
}

#[test]
fn native_refined_struct_base_values_match_interpreter() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/native/refined_struct_base_values.jett");
    let expected = jett_driver::run_file_capture_output(&fixture).expect("interpreter oracle");
    assert_eq!(
        expected.stdout,
        "Ada:Agent\nrefinement type constraint failed for 'test.NonEmpty'\nrefinement type constraint failed for 'test.Long'\nAda\nrefinement type constraint failed for 'test.NonEmpty'\n7\nrefinement type constraint failed for 'test.Positive'\n"
    );
    let directory = tempfile::tempdir().expect("isolated execution directory");
    let binary = directory.path().join("program.exe");
    build_host_executable(&fixture, launcher(), &binary)
        .expect("compile base-value refinement constructor");
    let actual = run_bounded(&binary, directory.path());
    assert!(actual.status.success(), "{actual:?}");
    assert_eq!(actual.stdout, expected.stdout.as_bytes());
    assert!(actual.stderr.is_empty(), "{actual:?}");
}

#[test]
fn native_refined_struct_derived_inputs_match_interpreter() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/native/refined_struct_derived_inputs.jett");
    let expected = jett_driver::run_file_capture_output(&fixture).expect("interpreter oracle");
    assert_eq!(
        expected.stdout,
        "Agent\nrefinement type constraint failed for 'test.Long'\naccepted\nrefinement type constraint failed for 'test.SecretPositive'\nAgent\nrefinement type constraint failed for 'test.Long'\naccepted\nrefinement type constraint failed for 'test.SecretPositive'\nAgent\nrefinement type constraint failed for 'test.SecretLong'\nrefinement type constraint failed for 'test.SecretNonEmpty'\n"
    );
    let directory = tempfile::tempdir().expect("isolated execution directory");
    let binary = directory.path().join("program.exe");
    build_host_executable(&fixture, launcher(), &binary)
        .expect("compile derived-input refinement constructor");
    let actual = run_bounded(&binary, directory.path());
    assert!(actual.status.success(), "{actual:?}");
    assert_eq!(actual.stdout, expected.stdout.as_bytes());
    assert!(actual.stderr.is_empty(), "{actual:?}");
}

#[test]
fn native_json_narrow_integers_match_interpreter() {
    let fixture =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/native/json_narrow_integers.jett");
    let expected = jett_driver::run_file_capture_output(&fixture).expect("interpreter oracle");
    let directory = tempfile::tempdir().expect("isolated execution directory");
    let binary = directory.path().join("program.exe");
    build_host_executable(&fixture, launcher(), &binary).expect("compile narrow JSON decoders");
    let actual = run_bounded(&binary, directory.path());
    assert!(actual.status.success(), "{actual:?}");
    assert_eq!(actual.stdout, expected.stdout.as_bytes());
    assert!(actual.stderr.is_empty(), "{actual:?}");
}

#[test]
fn native_json_float32_matches_interpreter() {
    let fixture =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/native/json_float32.jett");
    let expected = jett_driver::run_file_capture_output(&fixture).expect("interpreter oracle");
    let directory = tempfile::tempdir().expect("isolated execution directory");
    let binary = directory.path().join("program.exe");
    build_host_executable(&fixture, launcher(), &binary).expect("compile float32 JSON decoder");
    let actual = run_bounded(&binary, directory.path());
    assert!(actual.status.success(), "{actual:?}");
    assert_eq!(actual.stdout, expected.stdout.as_bytes());
    assert!(actual.stderr.is_empty(), "{actual:?}");
}

#[test]
fn native_json_set_serialize_matches_interpreter() {
    let fixture =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/native/json_set_serialize.jett");
    let expected = jett_driver::run_file_capture_output(&fixture).expect("interpreter oracle");
    let directory = tempfile::tempdir().expect("isolated execution directory");
    let binary = directory.path().join("program.exe");
    build_host_executable(&fixture, launcher(), &binary).expect("compile set JSON serializer");
    let actual = run_bounded(&binary, directory.path());
    assert!(actual.status.success(), "{actual:?}");
    assert_eq!(actual.stdout, expected.stdout.as_bytes());
    assert!(actual.stderr.is_empty(), "{actual:?}");
}

#[test]
fn native_json_extended_set_parse_matches_interpreter() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/native/json_set_parse_extended.jett");
    let expected = jett_driver::run_file_capture_output(&fixture).expect("interpreter oracle");
    let directory = tempfile::tempdir().expect("isolated execution directory");
    let binary = directory.path().join("program.exe");
    build_host_executable(&fixture, launcher(), &binary).expect("compile extended set JSON parser");
    let actual = run_bounded(&binary, directory.path());
    assert!(actual.status.success(), "{actual:?}");
    assert_eq!(actual.stdout, expected.stdout.as_bytes());
    assert!(actual.stderr.is_empty(), "{actual:?}");
}

#[test]
fn native_alias_json_parse_matches_interpreter() {
    let source = include_str!("../../../tests/run_pass/reflection_type_id_duplicate_aliases.jett");
    let directory = tempfile::tempdir().expect("isolated execution directory");
    let source_path = directory.path().join("alias_json.jett");
    std::fs::write(
        &source_path,
        format!(
            "{source}\nfunction main() returns nothing:\n    println(alias_reflection_summary(), account_alias_json_summary(), audit_alias_json_summary())\n"
        ),
    )
    .expect("write alias JSON executable");
    let expected = jett_driver::run_file_capture_output(&source_path).expect("interpreter oracle");
    let binary = directory.path().join("program.exe");
    build_host_executable(&source_path, launcher(), &binary).expect("compile alias JSON");
    let actual = run_bounded(&binary, directory.path());
    assert!(actual.status.success(), "{actual:?}");
    assert_eq!(actual.stdout, expected.stdout.as_bytes());
    assert!(actual.stderr.is_empty(), "{actual:?}");
}

#[test]
fn native_recursive_struct_json_matches_interpreter() {
    let source = include_str!("../../../tests/run_pass/recursive_owned_values.jett");
    let directory = tempfile::tempdir().expect("isolated execution directory");
    let source_path = directory.path().join("recursive_json.jett");
    std::fs::write(
        &source_path,
        format!(
            "{source}\nfunction recursive_json_summary() returns string:\n    Node tail = Node(value: 1, next: none)\n    Node original = Node(value: 2, next: some(tail))\n    string encoded = json.serialize[Node](view original)\n    Node decoded = json.parse_exact[Node](encoded) handle error:\n        return error\n    Node next = decoded.next handle:\n        return \"missing next\"\n    return \"{{decoded.value}}:{{next.value}}:{{encoded}}\"\nfunction main() returns nothing:\n    println(recursive_json_summary())\n"
        ),
    )
    .expect("write recursive JSON executable");
    let expected = jett_driver::run_file_capture_output(&source_path).expect("interpreter oracle");
    assert_eq!(
        expected.stdout,
        "2:1:{\"value\":2,\"next\":{\"value\":1,\"next\":null}}\n"
    );
    let binary = directory.path().join("program.exe");
    build_host_executable(&source_path, launcher(), &binary).expect("compile recursive JSON");
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
fn native_debug_statements_match_interpreter_output() {
    for relative_path in [
        "../../tests/run_pass/trace_basic.jett",
        "../../tests/native/trace_int64.jett",
        "../../tests/run_pass/breakpoint_basic.jett",
        "../../tests/native/breakpoint_int64.jett",
        "../../tests/native/debug_primitives.jett",
        "../../tests/native/debug_aggregates.jett",
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
        assert_eq!(
            String::from_utf8_lossy(&actual.stderr),
            debug,
            "{relative_path}"
        );
    }
}

#[test]
fn native_assertion_compiles_interpolated_failure_message() {
    let fixture =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/native/assert_messages.jett");
    let directory = tempfile::tempdir().expect("isolated verify directory");
    let binary = directory.path().join("assert_messages.exe");
    let artifact = build_host_verify_suite_executable(&fixture, launcher(), &binary)
        .expect("compile native verify assertion");
    let actual = run_bounded(&artifact.path, directory.path());
    assert!(actual.status.success(), "{actual:?}");
    assert!(actual.stdout.is_empty(), "{actual:?}");
    assert!(actual.stderr.is_empty(), "{actual:?}");
    let property_binary = directory.path().join("assert_property_messages.exe");
    let property = build_host_property_suite_executable(&fixture, launcher(), &property_binary)
        .expect("compile native property assertion");
    let property_output = run_bounded(&property.path, directory.path());
    assert!(property_output.status.success(), "{property_output:?}");
    assert!(property_output.stdout.is_empty(), "{property_output:?}");
    assert!(property_output.stderr.is_empty(), "{property_output:?}");
}

#[test]
fn native_enum_aggregate_equality_matches_interpreter() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/native/enum_aggregate_equality.jett");
    let expected = jett_driver::run_file_capture_output(&fixture).expect("interpreter oracle");
    let directory = tempfile::tempdir().expect("isolated enum equality directory");
    let binary = directory.path().join("enum_aggregate_equality.exe");
    build_host_executable(&fixture, launcher(), &binary).expect("compile aggregate enum equality");
    let actual = run_bounded(&binary, directory.path());
    assert!(actual.status.success(), "{actual:?}");
    assert_eq!(actual.stdout, expected.stdout.as_bytes());
    assert!(actual.stderr.is_empty(), "{actual:?}");
}

#[test]
fn native_binary_nested_handle_matches_interpreter() {
    let fixture =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/native/nested_handle_binary.jett");
    let expected = jett_driver::run_file_capture_output(&fixture).expect("interpreter oracle");
    let directory = tempfile::tempdir().expect("isolated nested handle directory");
    let binary = directory.path().join("nested_handle_binary.exe");
    build_host_executable(&fixture, launcher(), &binary)
        .expect("compile binary expression with a nested handle");
    let actual = run_bounded(&binary, directory.path());
    assert!(actual.status.success(), "{actual:?}");
    assert_eq!(actual.stdout, expected.stdout.as_bytes());
    assert!(actual.stderr.is_empty(), "{actual:?}");
}
