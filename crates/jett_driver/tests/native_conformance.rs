//! Run the complete native conformance gates on each supported host.
#![cfg(all(
    target_arch = "x86_64",
    any(
        all(target_os = "windows", target_env = "msvc"),
        all(target_os = "linux", target_env = "gnu")
    )
))]

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
                .join("native-values-launcher");
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
            let archive_name = if cfg!(windows) {
                "jett_native_launcher.lib"
            } else {
                "libjett_native_launcher.a"
            };
            let archive = target.join(&host).join(profile).join(archive_name);
            let directory = tempfile::tempdir().expect("launcher bundle directory");
            let copied = directory.path().join(archive_name);
            std::fs::copy(&archive, &copied).unwrap_or_else(|error| {
                panic!(
                    "cannot copy launcher archive {}: {error}",
                    archive.display()
                )
            });
            Launcher {
                bundle: if cfg!(windows) {
                    NativeLauncherBundle::windows_msvc_static_v1(copied)
                } else {
                    NativeLauncherBundle::linux_gnu_v1(copied)
                },
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
            start.elapsed() < Duration::from_secs(60),
            "native executable exceeded 60 second deadline"
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
fn native_interface_values_match_interpreter() {
    let fixture =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/native/interface_values.jett");
    let expected =
        jett_driver::run_file_capture_output(&fixture).expect("interface interpreter oracle");
    assert!(
        expected
            .debug_output
            .iter()
            .any(|line| line.contains("[redacted]"))
    );
    assert!(
        !expected
            .debug_output
            .iter()
            .any(|line| line.contains("hidden-interface-token"))
    );
    let directory = tempfile::tempdir().unwrap();
    let binary = directory.path().join("interface_values.exe");
    build_host_executable(&fixture, launcher(), &binary).expect("native interface values");
    let actual = run_bounded(&binary, directory.path());
    assert!(actual.status.success(), "{actual:?}");
    assert_eq!(actual.stdout, expected.stdout.as_bytes());
    assert_eq!(
        String::from_utf8_lossy(&actual.stderr),
        format!("{}\n", expected.debug_output.join("\n"))
    );
}

#[test]
fn native_interface_pending_failure_matches_interpreter() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/native/interface_pending_failure.jett");
    let expected = jett_driver::run_file_capture_outcome(&fixture)
        .expect_err("a pending receiver has no concrete dispatch target");
    let directory = tempfile::tempdir().unwrap();
    let binary = directory.path().join("interface_pending_failure.exe");
    build_host_executable(&fixture, launcher(), &binary).expect("native interface failure");
    let actual = run_bounded(&binary, directory.path());
    assert_eq!(actual.status.code(), Some(71), "{actual:?}");
    assert_eq!(actual.stdout, expected.output.stdout.as_bytes());
    assert_eq!(
        String::from_utf8_lossy(&actual.stderr),
        format!(
            "{}\n{}\n",
            expected.output.debug_output.join("\n"),
            expected.message
        )
    );
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
fn native_suites_reject_failing_source_assertions() {
    for (kind, given) in [("verify", ""), ("property", "    given n: int64\n")] {
        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join("failure.jett");
        fs::write(
            &source,
            format!(
                "{kind} first:\n{given}    assert true\n\n{kind} second:\n{given}    assert false \"native suite sentinel\"\n"
            ),
        ).unwrap();
        let oracle = jett_driver::test_file(&source).expect("checked failing suite");
        assert_eq!((oracle.passed, oracle.failed), (1, 1));
        let binary = directory.path().join("failure.exe");
        let build = if kind == "property" {
            build_host_property_suite_executable
        } else {
            build_host_verify_suite_executable
        };
        let error =
            build(&source, launcher(), &binary).expect_err("reject failed source assertion");
        assert!(
            error.to_string().contains("native suite sentinel"),
            "{error}"
        );
        assert!(
            !binary.exists(),
            "a failed suite must not publish an artifact"
        );
    }
}

#[test]
fn native_opaque_capability_values_match_interpreter() {
    for capability in ["Stderr", "Stdin", "Filesystem", "Network", "Process", "Log"] {
        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join("capability.jett");
        fs::write(
            &source,
            format!(
                r#"namespace test
function observe(view value: {capability}) returns nothing:
    trace value
    return nothing
function main(authority: {capability}) returns nothing:
    function(view {capability}) returns nothing callback = observe
    callback(view authority)
    {capability} moved = authority
    {capability} pending = run moved
    {capability} nested = run pending
    trace nested
    {capability} inner = join nested handle error:
        return nothing
    {capability} joined = join inner handle error:
        return nothing
    observe(view joined)
    {capability} cancelled = join joined handle error:
        println(error)
        return nothing
    trace cancelled
    return nothing
"#
            ),
        )
        .unwrap();
        let expected = jett_driver::run_file_capture_output(&source).unwrap();
        assert_eq!(expected.stdout, "task was cancelled\n");
        assert_eq!(
            expected.debug_output,
            [
                format!("trace value: {capability} = nothing"),
                format!("trace nested: {capability} = pending(pending(nothing))"),
                format!("trace value: {capability} = nothing"),
            ]
        );
        let binary = directory.path().join("capability.exe");
        build_host_executable(&source, launcher(), &binary)
            .unwrap_or_else(|error| panic!("{capability}: {error}"));
        let actual = run_bounded(&binary, directory.path());
        assert!(actual.status.success(), "{capability}: {actual:?}");
        assert_eq!(actual.stdout, expected.stdout.as_bytes(), "{capability}");
        assert_eq!(
            String::from_utf8_lossy(&actual.stderr),
            format!("{}\n", expected.debug_output.join("\n")),
            "{capability}"
        );
    }
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
fn native_success_rejects_unconsumed_scripted_provider_inputs() {
    let directory = tempfile::tempdir().expect("isolated scripted provider fixture");
    let fixture = directory.path().join("unconsumed.jett");
    fs::write(
        &fixture,
        "namespace app\nfunction main(rng: Random, clock: Clock, display: Graphics) returns nothing:\n    return nothing\n",
    )
    .expect("write scripted provider fixture");
    let binary = directory.path().join("unconsumed.exe");
    build_host_executable(&fixture, launcher(), &binary).expect("compile provider fixture");

    let random_samples = vec![random::RandomTestSample::Boolean(true)];
    let clock_samples = vec![clock::ClockTestSample::Unavailable];
    let graphics_events = vec![graphics::TestEvent::Close];
    let cases = [
        (
            random::TEST_SCRIPT_ENV,
            random::encode_test_script(&random_samples),
            jett_driver::run_file_capture_outcome_with_random_test_samples(
                &fixture,
                random_samples,
            )
            .expect_err("interpreter must reject unconsumed Random sample"),
        ),
        (
            clock::TEST_SCRIPT_ENV,
            clock::encode_test_script(&clock_samples),
            jett_driver::run_file_capture_outcome_with_clock_test_samples(&fixture, clock_samples)
                .expect_err("interpreter must reject unconsumed Clock sample"),
        ),
        (
            graphics::TEST_SCRIPT_ENV,
            graphics::encode_test_script(&graphics_events),
            jett_driver::run_file_capture_outcome_with_graphics_test_events(
                &fixture,
                graphics_events,
            )
            .expect_err("interpreter must reject unconsumed Graphics event"),
        ),
    ];
    for (environment, script, expected) in cases {
        let actual = run_bounded_with_env(&binary, directory.path(), Some((environment, &script)));
        assert_eq!(actual.status.code(), Some(71), "{environment}: {actual:?}");
        assert_eq!(
            actual.stdout,
            expected.output.stdout.as_bytes(),
            "{environment}"
        );
        assert_eq!(
            actual.stderr,
            format!("{}\n", expected.message).as_bytes(),
            "{environment}"
        );
    }
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
fn native_list_invalid_indices_match_interpreter_errors() {
    for name in ["list_insert_invalid_index", "list_remove_invalid_index"] {
        let fixture =
            Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("../../tests/native/{name}.jett"));
        let expected = jett_driver::run_file_capture_outcome(&fixture)
            .expect_err("interpreter must reject an invalid list index");
        let directory = tempfile::tempdir().expect("isolated list failure directory");
        let binary = directory.path().join(format!("{name}.exe"));
        build_host_executable(&fixture, launcher(), &binary)
            .expect("compile invalid list index fixture");
        let actual = run_bounded(&binary, directory.path());
        assert!(!actual.status.success(), "{name}: {actual:?}");
        assert_eq!(actual.stdout, expected.output.stdout.as_bytes(), "{name}");
        assert_eq!(
            actual.stderr,
            format!("{}\n", expected.message).as_bytes(),
            "{name}"
        );
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
fn native_graphics_pending_scalar_state_matches_interpreter() {
    for (name, events) in [
        (
            "graphics_pending_scalar_state.jett",
            vec![
                graphics::TestEvent::Key(graphics::Key::Right),
                graphics::TestEvent::Key(graphics::Key::Left),
                graphics::TestEvent::Close,
            ],
        ),
        (
            "graphics_pending_scalar_state_types.jett",
            vec![
                graphics::TestEvent::Key(graphics::Key::Right),
                graphics::TestEvent::Close,
                graphics::TestEvent::Key(graphics::Key::Left),
                graphics::TestEvent::Close,
            ],
        ),
    ] {
        let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/native")
            .join(name);
        let expected = jett_driver::run_file_capture_output_with_graphics_test_events(
            &fixture,
            events.clone(),
        )
        .expect("pending Graphics interpreter oracle");
        let script = graphics::encode_test_script(&events);
        let directory = tempfile::tempdir().expect("isolated pending Graphics state directory");
        let binary = directory.path().join("graphics_pending_state.exe");
        build_host_executable(&fixture, launcher(), &binary)
            .expect("compile pending Graphics state");
        let actual = run_bounded_with_env(
            &binary,
            directory.path(),
            Some((graphics::TEST_SCRIPT_ENV, &script)),
        );
        assert!(actual.status.success(), "{name}: {actual:?}");
        assert_eq!(actual.stdout, expected.stdout.as_bytes(), "{name}");
        assert_eq!(
            actual.stderr,
            format!("{}\n", expected.debug_output.join("\n")).as_bytes(),
            "{name}"
        );
    }
}

#[test]
fn native_graphics_callback_runtime_error_is_terminal() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/run_pass/graphics_callback_runtime_error.jett");
    // An unused event must not replace the callback's earlier runtime failure.
    let events = vec![
        graphics::TestEvent::Key(graphics::Key::Right),
        graphics::TestEvent::Close,
    ];
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
fn native_pending_nothing_matches_interpreter() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/native/structured_concurrency_nothing.jett");
    let expected = jett_driver::run_file_capture_output(&fixture).expect("interpreter oracle");
    let directory = tempfile::tempdir().expect("isolated pending nothing directory");
    let binary = directory.path().join("program.exe");
    build_host_executable(&fixture, launcher(), &binary).expect("compile pending nothing");
    let actual = run_bounded(&binary, directory.path());
    assert!(actual.status.success(), "{actual:?}");
    assert_eq!(actual.stdout, expected.stdout.as_bytes());
    assert_eq!(
        actual.stderr,
        format!("{}\n", expected.debug_output.join("\n")).as_bytes()
    );
}

#[test]
fn native_pending_string_values_match_interpreter() {
    let fixture =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/native/pending_string_values.jett");
    let expected = jett_driver::run_file_capture_output(&fixture).expect("interpreter oracle");
    let directory = tempfile::tempdir().expect("isolated pending string directory");
    let binary = directory.path().join("pending_string_values.exe");
    build_host_executable(&fixture, launcher(), &binary).expect("compile pending string values");
    let actual = run_bounded(&binary, directory.path());
    assert!(actual.status.success(), "{actual:?}");
    assert_eq!(actual.stdout, expected.stdout.as_bytes());
    assert_eq!(
        actual.stderr,
        format!("{}\n", expected.debug_output.join("\n")).as_bytes()
    );
}

#[test]
fn native_pending_bytes_values_match_interpreter() {
    let fixture =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/native/pending_bytes_values.jett");
    let expected = jett_driver::run_file_capture_output(&fixture).expect("interpreter oracle");
    let directory = tempfile::tempdir().expect("isolated pending bytes directory");
    let binary = directory.path().join("pending_bytes_values.exe");
    build_host_executable(&fixture, launcher(), &binary).expect("compile pending bytes values");
    let actual = run_bounded(&binary, directory.path());
    assert!(actual.status.success(), "{actual:?}");
    assert_eq!(actual.stdout, expected.stdout.as_bytes());
    assert_eq!(
        actual.stderr,
        format!("{}\n", expected.debug_output.join("\n")).as_bytes()
    );
}

#[test]
fn native_pending_list_values_match_interpreter() {
    let fixture =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/native/pending_list_values.jett");
    let expected = jett_driver::run_file_capture_output(&fixture).expect("interpreter oracle");
    let directory = tempfile::tempdir().expect("isolated pending list directory");
    let binary = directory.path().join("pending_list_values.exe");
    build_host_executable(&fixture, launcher(), &binary).expect("compile pending list values");
    let actual = run_bounded(&binary, directory.path());
    assert!(actual.status.success(), "{actual:?}");
    assert_eq!(actual.stdout, expected.stdout.as_bytes());
    assert_eq!(
        actual.stderr,
        format!("{}\n", expected.debug_output.join("\n")).as_bytes()
    );
}

#[test]
fn native_pending_list_intrinsics_match_interpreter_errors() {
    for name in [
        "pending_list_length_container_failure",
        "pending_list_append_container_failure",
        "pending_list_get_container_failure",
        "pending_list_get_index_failure",
        "pending_list_insert_index_failure",
        "pending_list_remove_index_failure",
        "pending_list_sort_container_failure",
        "pending_list_sort_index_failure",
        "pending_list_is_sorted_container_failure",
        "pending_list_swap_first_failure",
        "pending_list_swap_second_failure",
    ] {
        let fixture =
            Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("../../tests/native/{name}.jett"));
        let expected = jett_driver::run_file_capture_outcome(&fixture)
            .expect_err("interpreter must reject pending list intrinsic operands");
        let directory = tempfile::tempdir().expect("isolated pending list intrinsic directory");
        let binary = directory.path().join(format!("{name}.exe"));
        build_host_executable(&fixture, launcher(), &binary)
            .expect("compile pending list intrinsic");
        let actual = run_bounded(&binary, directory.path());
        assert!(!actual.status.success(), "{name}: {actual:?}");
        assert_eq!(actual.stdout, expected.output.stdout.as_bytes(), "{name}");
        assert_eq!(
            actual.stderr,
            format!("{}\n", expected.message).as_bytes(),
            "{name}"
        );
    }
}

#[test]
fn native_pending_index_and_count_intrinsics_match_interpreter_errors() {
    for name in [
        "pending_range_end_failure",
        "pending_range_start_failure",
        "pending_range_second_failure",
        "pending_range_step_failure",
        "pending_bytes_get_index_failure",
        "pending_bytes_get_container_failure",
        "pending_bytes_slice_start_failure",
        "pending_bytes_slice_end_failure",
        "pending_bytes_slice_container_failure",
        "pending_string_slice_start_failure",
        "pending_string_slice_end_failure",
        "pending_string_slice_container_failure",
        "pending_string_repeat_count_failure",
        "pending_string_repeat_container_failure",
    ] {
        let fixture =
            Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("../../tests/native/{name}.jett"));
        let expected = jett_driver::run_file_capture_outcome(&fixture)
            .expect_err("interpreter must reject pending index or count");
        let directory = tempfile::tempdir().expect("isolated pending intrinsic directory");
        let binary = directory.path().join(format!("{name}.exe"));
        build_host_executable(&fixture, launcher(), &binary)
            .expect("compile pending index or count intrinsic");
        let actual = run_bounded(&binary, directory.path());
        assert!(!actual.status.success(), "{name}: {actual:?}");
        assert_eq!(actual.stdout, expected.output.stdout.as_bytes(), "{name}");
        assert_eq!(
            actual.stderr,
            format!("{}\n", expected.message).as_bytes(),
            "{name}"
        );
    }
}

#[test]
fn native_pending_math_intrinsics_match_interpreter_errors() {
    for name in [
        "pending_math_abs_failure",
        "pending_math_min_failure",
        "pending_math_min_first_failure",
        "pending_math_max_failure",
        "pending_math_sqrt_failure",
        "pending_math_floor_failure",
        "pending_math_ceil_failure",
        "pending_math_round_failure",
        "pending_math_log_failure",
        "pending_math_log2_failure",
        "pending_math_log10_failure",
        "pending_math_sin_failure",
        "pending_math_cos_failure",
        "pending_math_tan_failure",
        "pending_math_pow_failure",
        "pending_math_pow_exponent_failure",
        "pending_math_clamp_failure",
        "pending_math_clamp_upper_failure",
        "pending_math_mod_failure",
        "pending_math_gcd_failure",
        "pending_math_lcm_failure",
        "pending_math_factorial_failure",
    ] {
        let fixture =
            Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("../../tests/native/{name}.jett"));
        let expected = jett_driver::run_file_capture_outcome(&fixture)
            .expect_err("interpreter must reject pending math operands");
        let directory = tempfile::tempdir().expect("isolated pending math directory");
        let binary = directory.path().join(format!("{name}.exe"));
        build_host_executable(&fixture, launcher(), &binary).expect("compile pending math case");
        let actual = run_bounded(&binary, directory.path());
        assert!(!actual.status.success(), "{name}: {actual:?}");
        assert_eq!(actual.stdout, expected.output.stdout.as_bytes(), "{name}");
        assert_eq!(
            actual.stderr,
            format!("{}\n", expected.message).as_bytes(),
            "{name}"
        );
    }
}

#[test]
fn native_pending_conversion_intrinsics_match_interpreter_errors() {
    for name in [
        "pending_float32_from_float64_failure",
        "pending_float64_from_int64_failure",
        "pending_int64_from_float64_failure",
        "pending_string_from_int64_failure",
        "pending_string_from_uint64_failure",
        "pending_string_from_float64_failure",
        "pending_string_from_bool_failure",
        "pending_int64_from_string_failure",
        "pending_uint64_from_string_failure",
        "pending_float64_from_string_failure",
        "pending_bytes_from_string_failure",
        "pending_bytes_from_hex_failure",
        "pending_bytes_to_string_failure",
        "pending_bytes_to_hex_failure",
    ] {
        let fixture =
            Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("../../tests/native/{name}.jett"));
        let expected = jett_driver::run_file_capture_outcome(&fixture)
            .expect_err("interpreter must reject pending conversion operand");
        let directory = tempfile::tempdir().expect("isolated pending conversion directory");
        let binary = directory.path().join(format!("{name}.exe"));
        build_host_executable(&fixture, launcher(), &binary)
            .expect("compile pending conversion case");
        let actual = run_bounded(&binary, directory.path());
        assert!(!actual.status.success(), "{name}: {actual:?}");
        assert_eq!(actual.stdout, expected.output.stdout.as_bytes(), "{name}");
        assert_eq!(
            actual.stderr,
            format!("{}\n", expected.message).as_bytes(),
            "{name}"
        );
    }
}

#[test]
fn native_pending_string_and_bytes_intrinsics_match_interpreter_errors() {
    for name in [
        "pending_string_upper_failure",
        "pending_string_split_delimiter_failure",
        "pending_string_join_separator_failure",
        "pending_string_join_element_failure",
        "pending_string_join_list_failure",
        "pending_string_replace_third_failure",
        "pending_bytes_length_failure",
        "pending_bytes_concat_first_failure",
        "pending_bytes_concat_second_failure",
    ] {
        let fixture =
            Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("../../tests/native/{name}.jett"));
        let expected = jett_driver::run_file_capture_outcome(&fixture)
            .expect_err("interpreter must reject pending string or bytes operands");
        let directory = tempfile::tempdir().expect("isolated pending text/bytes directory");
        let binary = directory.path().join(format!("{name}.exe"));
        build_host_executable(&fixture, launcher(), &binary)
            .expect("compile pending text/bytes case");
        let actual = run_bounded(&binary, directory.path());
        assert!(!actual.status.success(), "{name}: {actual:?}");
        assert_eq!(actual.stdout, expected.output.stdout.as_bytes(), "{name}");
        assert_eq!(
            actual.stderr,
            format!("{}\n", expected.message).as_bytes(),
            "{name}"
        );
    }
}

#[test]
fn native_pending_encoding_csv_and_crypto_intrinsics_match_interpreter() {
    for name in [
        "pending_encoding_base64_encode_failure",
        "pending_encoding_base64_decode_failure",
        "pending_encoding_hex_decode_failure",
        "pending_encoding_url_encode_failure",
        "pending_encoding_url_decode_failure",
        "pending_encoding_form_encode_failure",
        "pending_encoding_form_decode_failure",
        "pending_csv_parse_failure",
        "pending_csv_parse_with_header_failure",
        "pending_csv_stringify_list_failure",
        "pending_csv_stringify_row_failure",
        "pending_crypto_hmac_contextual_key_failure",
        "pending_crypto_hmac_key_failure",
        "pending_crypto_hmac_message_failure",
    ] {
        let fixture =
            Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("../../tests/native/{name}.jett"));
        let expected = jett_driver::run_file_capture_outcome(&fixture)
            .expect_err("interpreter must reject pending intrinsic operand");
        let directory = tempfile::tempdir().expect("isolated pending intrinsic directory");
        let binary = directory.path().join(format!("{name}.exe"));
        build_host_executable(&fixture, launcher(), &binary)
            .expect("compile pending intrinsic case");
        let actual = run_bounded(&binary, directory.path());
        assert!(!actual.status.success(), "{name}: {actual:?}");
        assert_eq!(actual.stdout, expected.output.stdout.as_bytes(), "{name}");
        assert_eq!(
            actual.stderr,
            format!("{}\n", expected.message).as_bytes(),
            "{name}"
        );
    }

    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/native/pending_csv_stringify_field.jett");
    let expected = jett_driver::run_file_capture_output(&fixture).expect("interpreter oracle");
    let directory = tempfile::tempdir().expect("isolated pending CSV field directory");
    let binary = directory.path().join("pending_csv_stringify_field.exe");
    build_host_executable(&fixture, launcher(), &binary).expect("compile pending CSV field case");
    let actual = run_bounded(&binary, directory.path());
    assert!(actual.status.success(), "{actual:?}");
    assert_eq!(actual.stdout, expected.stdout.as_bytes());
    assert_eq!(
        actual.stderr,
        format!("{}\n", expected.debug_output.join("\n")).as_bytes()
    );
}

#[test]
fn native_pending_secret_task_join_matches_interpreter() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/native/pending_secret_task_join.jett");
    let expected = jett_driver::run_file_capture_output(&fixture).expect("interpreter oracle");
    let directory = tempfile::tempdir().expect("isolated secret task directory");
    let binary = directory.path().join("pending_secret_task_join.exe");
    build_host_executable(&fixture, launcher(), &binary).expect("compile secret task case");
    let actual = run_bounded(&binary, directory.path());
    assert!(actual.status.success(), "{actual:?}");
    assert_eq!(actual.stdout, expected.stdout.as_bytes());
    assert!(actual.stderr.is_empty(), "{actual:?}");
}

#[test]
fn native_pending_capability_intrinsics_match_interpreter() {
    for name in [
        "pending_clock_now_failure",
        "pending_random_float_failure",
        "pending_random_bool_failure",
        "pending_random_int_failure",
        "pending_environment_args_failure",
        "pending_environment_get_authority_failure",
        "pending_environment_get_key_failure",
    ] {
        let fixture =
            Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("../../tests/native/{name}.jett"));
        let expected = jett_driver::run_file_capture_outcome(&fixture)
            .expect_err("interpreter must reject pending capability operand");
        let directory = tempfile::tempdir().expect("isolated pending capability directory");
        let binary = directory.path().join(format!("{name}.exe"));
        build_host_executable(&fixture, launcher(), &binary)
            .expect("compile pending capability case");
        let actual = run_bounded(&binary, directory.path());
        assert!(!actual.status.success(), "{name}: {actual:?}");
        assert_eq!(actual.stdout, expected.output.stdout.as_bytes(), "{name}");
        assert_eq!(
            actual.stderr,
            format!("{}\n", expected.message).as_bytes(),
            "{name}"
        );
    }
    for name in ["pending_stdout_write", "pending_stdout_write_text"] {
        let fixture =
            Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("../../tests/native/{name}.jett"));
        let expected = jett_driver::run_file_capture_output(&fixture).expect("interpreter oracle");
        let directory = tempfile::tempdir().expect("isolated pending stdout directory");
        let binary = directory.path().join(format!("{name}.exe"));
        build_host_executable(&fixture, launcher(), &binary).expect("compile pending stdout case");
        let actual = run_bounded(&binary, directory.path());
        assert!(actual.status.success(), "{name}: {actual:?}");
        assert_eq!(actual.stdout, expected.stdout.as_bytes(), "{name}");
        assert!(actual.stderr.is_empty(), "{name}: {actual:?}");
    }
}

#[test]
fn native_pending_secret_and_bitfield_intrinsics_match_interpreter() {
    for name in [
        "pending_secret_compare_string_first_failure",
        "pending_secret_compare_string_second_failure",
        "pending_secret_compare_bytes_first_failure",
        "pending_secret_compare_bytes_second_failure",
        "pending_bitfield_to_bytes_failure",
        "pending_bitfield_from_bytes_failure",
        "pending_bitfield_payload_element_failure",
        "pending_bitfield_payload_list_failure",
    ] {
        let fixture =
            Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("../../tests/native/{name}.jett"));
        let expected = jett_driver::run_file_capture_outcome(&fixture)
            .expect_err("interpreter must reject pending secret or bitfield operand");
        let directory = tempfile::tempdir().expect("isolated pending intrinsic directory");
        let binary = directory.path().join(format!("{name}.exe"));
        build_host_executable(&fixture, launcher(), &binary)
            .expect("compile pending secret or bitfield case");
        let actual = run_bounded(&binary, directory.path());
        assert!(!actual.status.success(), "{name}: {actual:?}");
        assert_eq!(actual.stdout, expected.output.stdout.as_bytes(), "{name}");
        assert_eq!(
            actual.stderr,
            format!("{}\n", expected.message).as_bytes(),
            "{name}"
        );
    }
    let fixture =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/native/pending_secret_redact.jett");
    let expected = jett_driver::run_file_capture_output(&fixture).expect("interpreter oracle");
    let directory = tempfile::tempdir().expect("isolated pending secret directory");
    let binary = directory.path().join("pending_secret_redact.exe");
    build_host_executable(&fixture, launcher(), &binary).expect("compile pending redaction case");
    let actual = run_bounded(&binary, directory.path());
    assert!(actual.status.success(), "{actual:?}");
    assert_eq!(actual.stdout, expected.stdout.as_bytes());
    assert!(actual.stderr.is_empty(), "{actual:?}");
}

#[test]
fn native_pending_actor_scalar_state_matches_interpreter() {
    for name in [
        "pending_actor_scalar_state",
        "pending_actor_scalar_state_types",
    ] {
        let fixture =
            Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("../../tests/native/{name}.jett"));
        let expected = jett_driver::run_file_capture_output(&fixture).expect("interpreter oracle");
        let directory = tempfile::tempdir().expect("isolated pending actor state directory");
        let binary = directory.path().join(format!("{name}.exe"));
        build_host_executable(&fixture, launcher(), &binary).expect("compile pending actor state");
        let actual = run_bounded(&binary, directory.path());
        assert!(actual.status.success(), "{name}: {actual:?}");
        assert_eq!(actual.stdout, expected.stdout.as_bytes(), "{name}");
        assert_eq!(
            actual.stderr,
            format!("{}\n", expected.debug_output.join("\n")).as_bytes(),
            "{name}"
        );
    }
}

#[test]
fn native_pending_set_map_values_match_interpreter() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/native/pending_set_map_values.jett");
    let expected = jett_driver::run_file_capture_output(&fixture).expect("interpreter oracle");
    let directory = tempfile::tempdir().expect("isolated pending set and map directory");
    let binary = directory.path().join("pending_set_map_values.exe");
    build_host_executable(&fixture, launcher(), &binary)
        .expect("compile pending set and map values");
    let actual = run_bounded(&binary, directory.path());
    assert!(actual.status.success(), "{actual:?}");
    assert_eq!(actual.stdout, expected.stdout.as_bytes());
    assert_eq!(
        actual.stderr,
        format!("{}\n", expected.debug_output.join("\n")).as_bytes()
    );
}

#[test]
fn native_pending_record_values_match_interpreter() {
    let fixture =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/native/pending_record_values.jett");
    let expected = jett_driver::run_file_capture_output(&fixture).expect("interpreter oracle");
    let directory = tempfile::tempdir().expect("isolated pending record directory");
    let binary = directory.path().join("pending_record_values.exe");
    build_host_executable(&fixture, launcher(), &binary).expect("compile pending record values");
    let actual = run_bounded(&binary, directory.path());
    assert!(actual.status.success(), "{actual:?}");
    assert_eq!(actual.stdout, expected.stdout.as_bytes());
    assert_eq!(
        actual.stderr,
        format!("{}\n", expected.debug_output.join("\n")).as_bytes()
    );
}

#[test]
fn native_pending_sum_values_match_interpreter() {
    let fixture =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/native/pending_sum_values.jett");
    let expected = jett_driver::run_file_capture_output(&fixture).expect("interpreter oracle");
    let directory = tempfile::tempdir().expect("isolated pending sum directory");
    let binary = directory.path().join("pending_sum_values.exe");
    build_host_executable(&fixture, launcher(), &binary).expect("compile pending sum values");
    let actual = run_bounded(&binary, directory.path());
    assert!(actual.status.success(), "{actual:?}");
    assert_eq!(actual.stdout, expected.stdout.as_bytes());
    assert_eq!(
        actual.stderr,
        format!("{}\n", expected.debug_output.join("\n")).as_bytes()
    );
}

#[test]
fn native_pending_function_values_match_interpreter() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/native/pending_function_values.jett");
    let expected = jett_driver::run_file_capture_output(&fixture).expect("interpreter oracle");
    let directory = tempfile::tempdir().expect("isolated pending function directory");
    let binary = directory.path().join("pending_function_values.exe");
    build_host_executable(&fixture, launcher(), &binary).expect("compile pending function values");
    let actual = run_bounded(&binary, directory.path());
    assert!(actual.status.success(), "{actual:?}");
    assert_eq!(actual.stdout, expected.stdout.as_bytes());
    assert_eq!(
        actual.stderr,
        format!("{}\n", expected.debug_output.join("\n")).as_bytes()
    );
}

#[test]
fn native_pending_function_call_matches_interpreter_error() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/native/pending_function_call_failure.jett");
    let expected = jett_driver::run_file_capture_outcome(&fixture)
        .expect_err("interpreter must reject a pending function call");
    let directory = tempfile::tempdir().expect("isolated pending function call directory");
    let binary = directory.path().join("pending_function_call_failure.exe");
    build_host_executable(&fixture, launcher(), &binary)
        .expect("compile pending function call failure");
    let actual = run_bounded(&binary, directory.path());
    assert!(!actual.status.success(), "{actual:?}");
    assert_eq!(actual.stdout, expected.output.stdout.as_bytes());
    assert_eq!(actual.stderr, format!("{}\n", expected.message).as_bytes());
}

#[test]
fn native_pending_type_construction_matches_interpreter() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/native/pending_type_construction.jett");
    let expected = jett_driver::run_file_capture_output(&fixture).expect("interpreter oracle");
    let directory = tempfile::tempdir().expect("isolated pending builder directory");
    let binary = directory.path().join("pending_type_construction.exe");
    build_host_executable(&fixture, launcher(), &binary).expect("compile pending builder");
    let actual = run_bounded(&binary, directory.path());
    assert!(actual.status.success(), "{actual:?}");
    assert_eq!(actual.stdout, expected.stdout.as_bytes());
    assert_eq!(
        actual.stderr,
        format!("{}\n", expected.debug_output.join("\n")).as_bytes()
    );
}

#[test]
fn native_pending_reflected_construction_matches_interpreter() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/native/pending_reflected_construction.jett");
    let expected = jett_driver::run_file_capture_output(&fixture).expect("interpreter oracle");
    let directory = tempfile::tempdir().expect("isolated reflected construction directory");
    let binary = directory.path().join("pending_reflected_construction.exe");
    build_host_executable(&fixture, launcher(), &binary).expect("compile reflected construction");
    let actual = run_bounded(&binary, directory.path());
    assert!(actual.status.success(), "{actual:?}");
    assert_eq!(actual.stdout, expected.stdout.as_bytes());
    assert_eq!(
        actual.stderr,
        format!("{}\n", expected.debug_output.join("\n")).as_bytes()
    );
}

#[test]
fn native_pending_reflected_scalar_fields_match_interpreter() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/native/pending_reflected_scalar_fields.jett");
    let expected = jett_driver::run_file_capture_output(&fixture).expect("interpreter oracle");
    let directory = tempfile::tempdir().expect("isolated reflected field directory");
    let binary = directory.path().join("pending_reflected_scalar_fields.exe");
    build_host_executable(&fixture, launcher(), &binary).expect("compile reflected fields");
    let actual = run_bounded(&binary, directory.path());
    assert!(actual.status.success(), "{actual:?}");
    assert_eq!(actual.stdout, expected.stdout.as_bytes());
    assert_eq!(
        actual.stderr,
        format!("{}\n", expected.debug_output.join("\n")).as_bytes()
    );
}

#[test]
fn native_pending_reflected_owned_fields_match_interpreter() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/native/pending_reflected_owned_fields.jett");
    let expected = jett_driver::run_file_capture_output(&fixture).expect("interpreter oracle");
    let directory = tempfile::tempdir().expect("isolated reflected field directory");
    let binary = directory.path().join("pending_reflected_owned_fields.exe");
    build_host_executable(&fixture, launcher(), &binary).expect("compile reflected fields");
    let actual = run_bounded(&binary, directory.path());
    assert!(actual.status.success(), "{actual:?}");
    assert_eq!(actual.stdout, expected.stdout.as_bytes());
    assert_eq!(
        actual.stderr,
        format!("{}\n", expected.debug_output.join("\n")).as_bytes()
    );
}

#[test]
fn native_pending_reflected_collections_match_interpreter() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/native/pending_reflected_collections.jett");
    let expected = jett_driver::run_file_capture_output(&fixture).expect("interpreter oracle");
    let directory = tempfile::tempdir().expect("isolated reflected collection directory");
    let binary = directory.path().join("pending_reflected_collections.exe");
    build_host_executable(&fixture, launcher(), &binary)
        .expect("compile reflected pending collections");
    let actual = run_bounded(&binary, directory.path());
    assert!(actual.status.success(), "{actual:?}");
    assert_eq!(actual.stdout, expected.stdout.as_bytes());
    assert_eq!(
        actual.stderr,
        format!("{}\n", expected.debug_output.join("\n")).as_bytes()
    );
}

#[test]
fn native_pending_reflected_sums_match_interpreter() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/native/pending_reflected_sums.jett");
    let expected = jett_driver::run_file_capture_output(&fixture).expect("interpreter oracle");
    let directory = tempfile::tempdir().expect("isolated reflected sum directory");
    let binary = directory.path().join("pending_reflected_sums.exe");
    build_host_executable(&fixture, launcher(), &binary).expect("compile reflected pending sums");
    let actual = run_bounded(&binary, directory.path());
    assert!(actual.status.success(), "{actual:?}");
    assert_eq!(actual.stdout, expected.stdout.as_bytes());
    assert_eq!(
        actual.stderr,
        format!("{}\n", expected.debug_output.join("\n")).as_bytes()
    );
}

#[test]
fn native_pending_contextual_empty_collections_match_interpreter() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/native/pending_contextual_empty_collections.jett");
    let expected = jett_driver::run_file_capture_output(&fixture).expect("interpreter oracle");
    let directory = tempfile::tempdir().expect("isolated pending empty collection directory");
    let binary = directory
        .path()
        .join("pending_contextual_empty_collections.exe");
    build_host_executable(&fixture, launcher(), &binary)
        .expect("compile pending contextual empty collections");
    let actual = run_bounded(&binary, directory.path());
    assert!(actual.status.success(), "{actual:?}");
    assert_eq!(actual.stdout, expected.stdout.as_bytes());
    assert_eq!(
        actual.stderr,
        format!("{}\n", expected.debug_output.join("\n")).as_bytes()
    );
}

#[test]
fn native_pending_reflection_metadata_matches_interpreter() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/native/pending_reflection_metadata.jett");
    let expected = jett_driver::run_file_capture_output(&fixture).expect("interpreter oracle");
    let directory = tempfile::tempdir().expect("isolated pending reflection directory");
    let binary = directory.path().join("pending_reflection_metadata.exe");
    build_host_executable(&fixture, launcher(), &binary)
        .expect("compile pending reflection metadata");
    let actual = run_bounded(&binary, directory.path());
    assert!(actual.status.success(), "{actual:?}");
    assert_eq!(actual.stdout, expected.stdout.as_bytes());
    assert_eq!(
        actual.stderr,
        format!("{}\n", expected.debug_output.join("\n")).as_bytes()
    );
}

#[test]
fn native_pending_reflected_owner_errors_match_interpreter() {
    for name in [
        "pending_variant_value_failure",
        "pending_machine_state_value_failure",
        "pending_struct_field_owner_failure",
        "pending_variant_field_owner_failure",
        "pending_machine_field_owner_failure",
        "pending_bitfield_field_owner_failure",
        "pending_narrowed_machine_field_owner_failure",
        "pending_variant_foreign_field_precedence",
        "pending_machine_foreign_field_precedence",
        "pending_variant_both_metadata_failure",
    ] {
        let fixture =
            Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("../../tests/native/{name}.jett"));
        let expected = jett_driver::run_file_capture_outcome(&fixture)
            .expect_err("interpreter must reject pending reflected owner");
        let directory = tempfile::tempdir().expect("isolated pending reflected owner directory");
        let binary = directory.path().join(format!("{name}.exe"));
        build_host_executable(&fixture, launcher(), &binary)
            .expect("compile pending reflected owner");
        let actual = run_bounded(&binary, directory.path());
        assert!(!actual.status.success(), "{name}: {actual:?}");
        assert_eq!(actual.stdout, expected.output.stdout.as_bytes(), "{name}");
        assert_eq!(
            actual.stderr,
            format!("{}\n", expected.message).as_bytes(),
            "{name}"
        );
    }
}

#[test]
fn native_secret_debug_failure_messages_are_redacted() {
    for name in [
        "secret_pending_struct_field_owner_failure",
        "secret_pending_variant_value_failure",
        "secret_pending_machine_state_value_failure",
        "secret_pending_variant_field_owner_failure",
        "secret_pending_machine_field_owner_failure",
        "secret_pending_builder_put_failure",
        "secret_pending_builder_finish_failure",
    ] {
        let fixture =
            Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("../../tests/native/{name}.jett"));
        let expected = jett_driver::run_file_capture_outcome(&fixture)
            .expect_err("pending reflected value must fail");
        assert!(
            expected.message.contains("[redacted]"),
            "{name}: {}",
            expected.message
        );
        assert!(
            !expected.message.contains("hidden-"),
            "{name}: {}",
            expected.message
        );
        let directory = tempfile::tempdir().unwrap();
        let binary = directory.path().join(format!("{name}.exe"));
        build_host_executable(&fixture, launcher(), &binary).unwrap();
        let actual = run_bounded(&binary, directory.path());
        assert!(!actual.status.success(), "{name}: {actual:?}");
        assert_eq!(actual.stdout, expected.output.stdout.as_bytes(), "{name}");
        assert_eq!(
            actual.stderr,
            format!("{}\n", expected.message).as_bytes(),
            "{name}"
        );
    }
}

#[test]
fn native_json_nested_secrets_match_interpreter() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/native/json_nested_secret_probe.jett");
    let expected = jett_driver::run_file_capture_output(&fixture).expect("interpreter oracle");
    let directory = tempfile::tempdir().expect("isolated nested secret JSON directory");
    let binary = directory.path().join("json_nested_secret_probe.exe");
    build_host_executable(&fixture, launcher(), &binary).expect("compile nested secret JSON");
    let actual = run_bounded(&binary, directory.path());
    assert!(actual.status.success(), "{actual:?}");
    assert_eq!(actual.stdout, expected.stdout.as_bytes());
    assert!(expected.debug_output.is_empty());
    assert!(actual.stderr.is_empty());
}

#[test]
fn native_json_recursive_enum_matches_interpreter() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/native/json_recursive_enum_probe.jett");
    let expected = jett_driver::run_file_capture_output(&fixture).expect("interpreter oracle");
    let directory = tempfile::tempdir().expect("isolated recursive enum JSON directory");
    let binary = directory.path().join("json_recursive_enum_probe.exe");
    build_host_executable(&fixture, launcher(), &binary).expect("compile recursive enum JSON");
    let actual = run_bounded(&binary, directory.path());
    assert!(actual.status.success(), "{actual:?}");
    assert_eq!(actual.stdout, expected.stdout.as_bytes());
    assert!(expected.debug_output.is_empty());
    assert!(actual.stderr.is_empty());
}

#[test]
fn native_pending_json_inputs_match_interpreter_errors() {
    for name in [
        "pending_json_parse_input_failure",
        "pending_json_serialize_string_failure",
    ] {
        let fixture =
            Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("../../tests/native/{name}.jett"));
        let expected = jett_driver::run_file_capture_outcome(&fixture)
            .expect_err("interpreter must reject pending JSON input");
        let directory = tempfile::tempdir().expect("isolated pending JSON directory");
        let binary = directory.path().join(format!("{name}.exe"));
        build_host_executable(&fixture, launcher(), &binary).expect("compile pending JSON input");
        let actual = run_bounded(&binary, directory.path());
        assert!(!actual.status.success(), "{name}: {actual:?}");
        assert_eq!(actual.stdout, expected.output.stdout.as_bytes(), "{name}");
        assert_eq!(
            actual.stderr,
            format!("{}\n", expected.message).as_bytes(),
            "{name}"
        );
    }
}

#[test]
fn native_pending_scalar_formatting_matches_interpreter() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/native/pending_scalar_formatting.jett");
    let expected = jett_driver::run_file_capture_output(&fixture).expect("interpreter oracle");
    let directory = tempfile::tempdir().expect("isolated pending formatting directory");
    let binary = directory.path().join("pending_scalar_formatting.exe");
    build_host_executable(&fixture, launcher(), &binary).expect("compile pending formatting");
    let actual = run_bounded(&binary, directory.path());
    assert!(actual.status.success(), "{actual:?}");
    assert_eq!(actual.stdout, expected.stdout.as_bytes());
    assert_eq!(
        actual.stderr,
        format!("{}\n", expected.debug_output.join("\n")).as_bytes()
    );
}

#[test]
fn native_pending_reflected_field_error_matches_interpreter() {
    for name in [
        "pending_reflected_field_failure",
        "pending_reflected_variant_field_failure",
        "pending_reflected_machine_field_failure",
    ] {
        let fixture =
            Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("../../tests/native/{name}.jett"));
        let expected = jett_driver::run_file_capture_outcome(&fixture)
            .expect_err("interpreter must reject pending reflected field metadata");
        let directory = tempfile::tempdir().expect("isolated pending reflection failure directory");
        let binary = directory.path().join(format!("{name}.exe"));
        build_host_executable(&fixture, launcher(), &binary)
            .expect("compile pending reflected field failure");
        let actual = run_bounded(&binary, directory.path());
        assert!(!actual.status.success(), "{name}: {actual:?}");
        assert_eq!(actual.stdout, expected.output.stdout.as_bytes(), "{name}");
        assert_eq!(
            actual.stderr,
            format!("{}\n", expected.message).as_bytes(),
            "{name}"
        );
    }
}

#[test]
fn native_pending_builder_field_matches_interpreter() {
    let fixture =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/native/pending_builder_field.jett");
    let expected = jett_driver::run_file_capture_output(&fixture).expect("interpreter oracle");
    let directory = tempfile::tempdir().expect("isolated pending builder field directory");
    let binary = directory.path().join("pending_builder_field.exe");
    build_host_executable(&fixture, launcher(), &binary).expect("compile pending builder field");
    let actual = run_bounded(&binary, directory.path());
    assert!(actual.status.success(), "{actual:?}");
    assert_eq!(actual.stdout, expected.stdout.as_bytes());
    assert!(expected.debug_output.is_empty());
    assert!(actual.stderr.is_empty());
}

#[test]
fn native_pending_builder_member_metadata_matches_interpreter() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/native/pending_builder_member_metadata.jett");
    let expected = jett_driver::run_file_capture_output(&fixture).expect("interpreter oracle");
    let directory = tempfile::tempdir().expect("isolated pending builder member directory");
    let binary = directory.path().join("pending_builder_member_metadata.exe");
    build_host_executable(&fixture, launcher(), &binary)
        .expect("compile pending builder member metadata");
    let actual = run_bounded(&binary, directory.path());
    assert!(actual.status.success(), "{actual:?}");
    assert_eq!(actual.stdout, expected.stdout.as_bytes());
    assert!(expected.debug_output.is_empty());
    assert!(actual.stderr.is_empty());
}

#[test]
fn native_reflected_foreign_field_errors_match_interpreter() {
    for name in [
        "reflected_struct_foreign_field_failure",
        "reflected_struct_foreign_incompatible_failure",
        "reflected_enum_foreign_field_failure",
        "reflected_machine_foreign_field_failure",
    ] {
        let fixture =
            Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("../../tests/native/{name}.jett"));
        let expected = jett_driver::run_file_capture_outcome(&fixture)
            .expect_err("interpreter must reject foreign reflected field");
        let directory = tempfile::tempdir().expect("isolated reflected field directory");
        let binary = directory.path().join(format!("{name}.exe"));
        build_host_executable(&fixture, launcher(), &binary)
            .expect("compile foreign reflected field");
        let actual = run_bounded(&binary, directory.path());
        assert!(!actual.status.success(), "{name}: {actual:?}");
        assert_eq!(actual.stdout, expected.output.stdout.as_bytes(), "{name}");
        assert_eq!(
            actual.stderr,
            format!("{}\n", expected.message).as_bytes(),
            "{name}"
        );
    }
}

#[test]
fn native_reflected_requested_type_errors_match_interpreter() {
    for name in [
        "reflected_struct_requested_type_failure",
        "reflected_enum_requested_type_failure",
        "reflected_machine_requested_type_failure",
    ] {
        let fixture =
            Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("../../tests/native/{name}.jett"));
        let expected = jett_driver::run_file_capture_outcome(&fixture)
            .expect_err("interpreter must reject requested reflected field type");
        let directory = tempfile::tempdir().expect("isolated reflected type directory");
        let binary = directory.path().join(format!("{name}.exe"));
        build_host_executable(&fixture, launcher(), &binary)
            .expect("compile requested reflected field type");
        let actual = run_bounded(&binary, directory.path());
        assert!(!actual.status.success(), "{name}: {actual:?}");
        assert_eq!(actual.stdout, expected.output.stdout.as_bytes(), "{name}");
        assert_eq!(
            actual.stderr,
            format!("{}\n", expected.message).as_bytes(),
            "{name}"
        );
    }
}

#[test]
fn native_reflected_missing_candidate_owner_errors_match_interpreter() {
    for name in [
        "reflected_struct_missing_candidate_owner_failure",
        "reflected_enum_missing_candidate_owner_failure",
        "reflected_enum_empty_candidate_owner_failure",
        "reflected_machine_missing_candidate_owner_failure",
        "reflected_machine_empty_candidate_owner_failure",
    ] {
        let fixture =
            Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("../../tests/native/{name}.jett"));
        let expected = jett_driver::run_file_capture_outcome(&fixture)
            .expect_err("interpreter must reject foreign reflected field");
        let directory = tempfile::tempdir().expect("isolated missing field directory");
        let binary = directory.path().join(format!("{name}.exe"));
        build_host_executable(&fixture, launcher(), &binary)
            .expect("compile missing reflected field candidate");
        let actual = run_bounded(&binary, directory.path());
        assert!(!actual.status.success(), "{name}: {actual:?}");
        assert_eq!(actual.stdout, expected.output.stdout.as_bytes(), "{name}");
        assert_eq!(
            actual.stderr,
            format!("{}\n", expected.message).as_bytes(),
            "{name}"
        );
    }
}

#[test]
fn native_type_arg_index_errors_match_interpreter() {
    for name in [
        "pending_type_arg_failure",
        "pending_type_arg_nested_failure",
        "type_arg_negative_failure",
        "type_arg_out_of_range_failure",
        "type_arg_alias_out_of_range_failure",
    ] {
        let fixture =
            Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("../../tests/native/{name}.jett"));
        let expected = jett_driver::run_file_capture_outcome(&fixture)
            .expect_err("interpreter must reject invalid type.arg index");
        let directory = tempfile::tempdir().expect("isolated type.arg directory");
        let binary = directory.path().join(format!("{name}.exe"));
        build_host_executable(&fixture, launcher(), &binary).expect("compile type.arg failure");
        let actual = run_bounded(&binary, directory.path());
        assert!(!actual.status.success(), "{name}: {actual:?}");
        assert_eq!(actual.stdout, expected.output.stdout.as_bytes(), "{name}");
        assert_eq!(
            actual.stderr,
            format!("{}\n", expected.message).as_bytes(),
            "{name}"
        );
    }
}

#[test]
fn native_pending_type_construction_invalid_use_matches_interpreter() {
    for name in [
        "pending_type_construction_invalid_use",
        "pending_type_construction_put_failure",
    ] {
        let fixture =
            Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("../../tests/native/{name}.jett"));
        let expected = jett_driver::run_file_capture_outcome(&fixture)
            .expect_err("interpreter must reject pending builder use");
        let directory = tempfile::tempdir().expect("isolated pending builder misuse directory");
        let binary = directory.path().join(format!("{name}.exe"));
        build_host_executable(&fixture, launcher(), &binary)
            .expect("compile pending builder misuse");
        let actual = run_bounded(&binary, directory.path());
        assert!(!actual.status.success(), "{name}: {actual:?}");
        assert_eq!(actual.stdout, expected.output.stdout.as_bytes(), "{name}");
        assert_eq!(
            actual.stderr,
            format!("{}\n", expected.message).as_bytes(),
            "{name}"
        );
    }
}

#[test]
fn native_pending_actor_values_match_interpreter() {
    let fixture =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/native/pending_actor_values.jett");
    let expected = jett_driver::run_file_capture_output(&fixture).expect("interpreter oracle");
    let directory = tempfile::tempdir().expect("isolated pending actor directory");
    let binary = directory.path().join("pending_actor_values.exe");
    build_host_executable(&fixture, launcher(), &binary).expect("compile pending actor values");
    let actual = run_bounded(&binary, directory.path());
    assert!(actual.status.success(), "{actual:?}");
    assert_eq!(actual.stdout, expected.stdout.as_bytes());
    assert_eq!(
        actual.stderr,
        format!("{}\n", expected.debug_output.join("\n")).as_bytes()
    );
}

#[test]
fn native_pending_actor_send_matches_interpreter_error() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/native/pending_actor_send_failure.jett");
    let expected = jett_driver::run_file_capture_outcome(&fixture)
        .expect_err("interpreter must reject a pending actor send");
    let directory = tempfile::tempdir().expect("isolated pending actor send directory");
    let binary = directory.path().join("pending_actor_send_failure.exe");
    build_host_executable(&fixture, launcher(), &binary).expect("compile pending actor send");
    let actual = run_bounded(&binary, directory.path());
    assert!(!actual.status.success(), "{actual:?}");
    assert_eq!(actual.stdout, expected.output.stdout.as_bytes());
    assert_eq!(actual.stderr, format!("{}\n", expected.message).as_bytes());
}

#[test]
fn native_pending_capability_values_match_interpreter() {
    for name in [
        "pending_capability_values",
        "pending_capability_cancelled",
        "pending_environment_values",
    ] {
        let fixture =
            Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("../../tests/native/{name}.jett"));
        let expected = jett_driver::run_file_capture_output(&fixture).expect("interpreter oracle");
        let directory = tempfile::tempdir().expect("isolated pending capability directory");
        let binary = directory.path().join(format!("{name}.exe"));
        build_host_executable(&fixture, launcher(), &binary)
            .expect("compile pending capability values");
        let actual = run_bounded(&binary, directory.path());
        assert!(actual.status.success(), "{name}: {actual:?}");
        assert_eq!(actual.stdout, expected.stdout.as_bytes(), "{name}");
        let expected_debug = if expected.debug_output.is_empty() {
            String::new()
        } else {
            format!("{}\n", expected.debug_output.join("\n"))
        };
        assert_eq!(actual.stderr, expected_debug.as_bytes(), "{name}");
    }
}

#[test]
fn native_pending_scalar_values_match_interpreter() {
    let fixture =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/native/pending_scalar_values.jett");
    let expected = jett_driver::run_file_capture_output(&fixture).expect("interpreter oracle");
    let directory = tempfile::tempdir().expect("isolated pending scalar directory");
    let binary = directory.path().join("pending_scalar_values.exe");
    build_host_executable(&fixture, launcher(), &binary).expect("compile pending scalar values");
    let actual = run_bounded(&binary, directory.path());
    assert!(actual.status.success(), "{actual:?}");
    assert_eq!(actual.stdout, expected.stdout.as_bytes());
    assert_eq!(
        actual.stderr,
        format!("{}\n", expected.debug_output.join("\n")).as_bytes()
    );
}

#[test]
fn native_pending_narrow_integers_match_interpreter() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/native/pending_narrow_integers.jett");
    let expected = jett_driver::run_file_capture_output(&fixture).expect("interpreter oracle");
    let directory = tempfile::tempdir().expect("isolated pending narrow integer directory");
    let binary = directory.path().join("pending_narrow_integers.exe");
    build_host_executable(&fixture, launcher(), &binary).expect("compile pending narrow integers");
    let actual = run_bounded(&binary, directory.path());
    assert!(actual.status.success(), "{actual:?}");
    assert_eq!(actual.stdout, expected.stdout.as_bytes());
    assert_eq!(
        actual.stderr,
        format!("{}\n", expected.debug_output.join("\n")).as_bytes()
    );
}

#[test]
fn native_pending_refined_narrow_matches_interpreter() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/native/pending_refined_narrow.jett");
    let expected = jett_driver::run_file_capture_output(&fixture).expect("interpreter oracle");
    let directory = tempfile::tempdir().expect("isolated pending refinement directory");
    let binary = directory.path().join("pending_refined_narrow.exe");
    build_host_executable(&fixture, launcher(), &binary).expect("compile pending refinement");
    let actual = run_bounded(&binary, directory.path());
    assert!(actual.status.success(), "{actual:?}");
    assert_eq!(actual.stdout, expected.stdout.as_bytes());
    assert!(actual.stderr.is_empty(), "{actual:?}");
}

#[test]
fn native_refinement_pending_predicate_result_matches_interpreter() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/native/refinement_pending_predicate_result.jett");
    let expected = jett_driver::run_file_capture_output(&fixture).expect("interpreter oracle");
    let directory = tempfile::tempdir().expect("isolated pending predicate directory");
    let binary = directory
        .path()
        .join("refinement_pending_predicate_result.exe");
    build_host_executable(&fixture, launcher(), &binary).expect("compile pending predicate");
    let actual = run_bounded(&binary, directory.path());
    assert!(actual.status.success(), "{actual:?}");
    assert_eq!(actual.stdout, expected.stdout.as_bytes());
    assert!(actual.stderr.is_empty(), "{actual:?}");
}

#[test]
fn native_refinement_pending_aggregate_builders_match_interpreter() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/native/refinement_pending_aggregate_builders.jett");
    let expected = jett_driver::run_file_capture_output(&fixture).expect("interpreter oracle");
    let message = "refinement constraint for 'app.Positive' must return bool, got pending(true)";
    let evaluation_error = "error evaluating refinement constraint for 'app.StrictPositive': unsupported binary operation: pending(7) Gt 0";
    assert_eq!(
        expected.stdout,
        format!("{message}\n{message}\n{evaluation_error}\n{message}\n{message}\n")
    );
    let directory = tempfile::tempdir().expect("isolated aggregate refinement directory");
    let binary = directory
        .path()
        .join("refinement_pending_aggregate_builders.exe");
    build_host_executable(&fixture, launcher(), &binary)
        .expect("compile aggregate refinement builders");
    let actual = run_bounded(&binary, directory.path());
    assert!(actual.status.success(), "{actual:?}");
    assert_eq!(actual.stdout, expected.stdout.as_bytes());
    assert!(actual.stderr.is_empty(), "{actual:?}");
}

#[test]
fn native_pending_scalar_aggregates_match_interpreter() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/native/pending_scalar_aggregates.jett");
    let expected = jett_driver::run_file_capture_output(&fixture).expect("interpreter oracle");
    let directory = tempfile::tempdir().expect("isolated pending scalar aggregate directory");
    let binary = directory.path().join("pending_scalar_aggregates.exe");
    build_host_executable(&fixture, launcher(), &binary)
        .expect("compile pending scalar aggregates");
    let actual = run_bounded(&binary, directory.path());
    assert!(actual.status.success(), "{actual:?}");
    assert_eq!(actual.stdout, expected.stdout.as_bytes());
    assert_eq!(
        actual.stderr,
        format!("{}\n", expected.debug_output.join("\n")).as_bytes()
    );
}

#[test]
fn native_pending_scalar_operations_match_interpreter_errors() {
    for name in [
        "pending_scalar_add_failure",
        "pending_scalar_nested_add_failure",
        "pending_scalar_comparison_failure",
        "pending_scalar_float_comparison_failure",
        "pending_scalar_negation_failure",
        "pending_scalar_not_failure",
        "pending_scalar_and_failure",
        "pending_scalar_and_right_failure",
        "pending_scalar_or_failure",
        "pending_scalar_or_right_failure",
        "pending_scalar_condition_failure",
        "pending_scalar_breakpoint_failure",
        "pending_random_lower_bound_failure",
        "pending_random_upper_bound_failure",
    ] {
        let fixture =
            Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("../../tests/native/{name}.jett"));
        let expected = jett_driver::run_file_capture_outcome(&fixture)
            .expect_err("interpreter must reject pending scalar operation");
        let directory = tempfile::tempdir().expect("isolated pending scalar operation directory");
        let binary = directory.path().join(format!("{name}.exe"));
        build_host_executable(&fixture, launcher(), &binary)
            .expect("compile pending scalar operation");
        let actual = run_bounded(&binary, directory.path());
        assert!(!actual.status.success(), "{name}: {actual:?}");
        assert_eq!(actual.stdout, expected.output.stdout.as_bytes(), "{name}");
        assert_eq!(
            actual.stderr,
            format!("{}\n", expected.message).as_bytes(),
            "{name}"
        );
    }
}

#[test]
fn native_pending_numeric_aggregates_match_interpreter_errors() {
    for name in [
        "pending_list_sum_first_failure",
        "pending_list_sum_later_failure",
        "pending_list_sum_container_failure",
        "pending_math_average_failure",
        "pending_math_average_container_failure",
        "pending_math_median_failure",
        "pending_math_median_container_failure",
    ] {
        let fixture =
            Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("../../tests/native/{name}.jett"));
        let expected = jett_driver::run_file_capture_outcome(&fixture)
            .expect_err("interpreter must reject pending numeric list element");
        let directory = tempfile::tempdir().expect("isolated pending numeric directory");
        let binary = directory.path().join(format!("{name}.exe"));
        build_host_executable(&fixture, launcher(), &binary)
            .expect("compile pending numeric aggregate");
        let actual = run_bounded(&binary, directory.path());
        assert!(!actual.status.success(), "{name}: {actual:?}");
        assert_eq!(actual.stdout, expected.output.stdout.as_bytes(), "{name}");
        assert_eq!(
            actual.stderr,
            format!("{}\n", expected.message).as_bytes(),
            "{name}"
        );
    }
}

#[test]
fn native_pending_set_elements_match_interpreter() {
    let fixture =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/native/pending_set_elements.jett");
    let expected = jett_driver::run_file_capture_output(&fixture).expect("interpreter oracle");
    let directory = tempfile::tempdir().expect("isolated pending set directory");
    let binary = directory.path().join("pending_set_elements.exe");
    build_host_executable(&fixture, launcher(), &binary).expect("compile pending set elements");
    let actual = run_bounded(&binary, directory.path());
    assert!(actual.status.success(), "{actual:?}");
    assert_eq!(actual.stdout, expected.stdout.as_bytes());
    assert_eq!(
        actual.stderr,
        format!("{}\n", expected.debug_output.join("\n")).as_bytes()
    );
}

#[test]
fn native_pending_map_entries_match_interpreter() {
    let fixture =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/native/pending_map_entries.jett");
    let expected = jett_driver::run_file_capture_output(&fixture).expect("interpreter oracle");
    let directory = tempfile::tempdir().expect("isolated pending map directory");
    let binary = directory.path().join("pending_map_entries.exe");
    build_host_executable(&fixture, launcher(), &binary).expect("compile pending map entries");
    let actual = run_bounded(&binary, directory.path());
    assert!(actual.status.success(), "{actual:?}");
    assert_eq!(actual.stdout, expected.stdout.as_bytes());
    assert_eq!(
        actual.stderr,
        format!("{}\n", expected.debug_output.join("\n")).as_bytes()
    );
}

#[test]
fn native_pending_map_intrinsics_match_interpreter_errors() {
    for name in [
        "pending_map_length_failure",
        "pending_map_has_failure",
        "pending_map_get_failure",
        "pending_map_insert_failure",
        "pending_map_remove_failure",
        "pending_map_from_lists_failure",
    ] {
        let fixture =
            Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("../../tests/native/{name}.jett"));
        let expected = jett_driver::run_file_capture_outcome(&fixture)
            .expect_err("interpreter must reject pending map or input list");
        let directory = tempfile::tempdir().expect("isolated pending map directory");
        let binary = directory.path().join(format!("{name}.exe"));
        build_host_executable(&fixture, launcher(), &binary)
            .expect("compile pending map intrinsic");
        let actual = run_bounded(&binary, directory.path());
        assert!(!actual.status.success(), "{name}: {actual:?}");
        assert_eq!(actual.stdout, expected.output.stdout.as_bytes(), "{name}");
        assert_eq!(
            actual.stderr,
            format!("{}\n", expected.message).as_bytes(),
            "{name}"
        );
    }
}

#[test]
fn native_pending_set_intrinsics_match_interpreter_errors() {
    for name in [
        "pending_set_length_failure",
        "pending_set_contains_failure",
        "pending_set_add_failure",
        "pending_set_remove_failure",
    ] {
        let fixture =
            Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("../../tests/native/{name}.jett"));
        let expected = jett_driver::run_file_capture_outcome(&fixture)
            .expect_err("interpreter must reject pending set container");
        let directory = tempfile::tempdir().expect("isolated pending set directory");
        let binary = directory.path().join(format!("{name}.exe"));
        build_host_executable(&fixture, launcher(), &binary)
            .expect("compile pending set intrinsic");
        let actual = run_bounded(&binary, directory.path());
        assert!(!actual.status.success(), "{name}: {actual:?}");
        assert_eq!(actual.stdout, expected.output.stdout.as_bytes(), "{name}");
        assert_eq!(
            actual.stderr,
            format!("{}\n", expected.message).as_bytes(),
            "{name}"
        );
    }
}

#[test]
fn native_pending_sequence_iteration_matches_interpreter_errors() {
    for name in [
        "pending_list_iteration_failure",
        "pending_string_iteration_failure",
        "pending_map_iteration_failure",
        "pending_set_iteration_failure",
    ] {
        let fixture =
            Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("../../tests/native/{name}.jett"));
        let expected = jett_driver::run_file_capture_outcome(&fixture)
            .expect_err("interpreter must reject pending iterable");
        let directory = tempfile::tempdir().expect("isolated pending iteration directory");
        let binary = directory.path().join(format!("{name}.exe"));
        build_host_executable(&fixture, launcher(), &binary).expect("compile pending iteration");
        let actual = run_bounded(&binary, directory.path());
        assert!(!actual.status.success(), "{name}: {actual:?}");
        assert_eq!(actual.stdout, expected.output.stdout.as_bytes(), "{name}");
        assert_eq!(
            actual.stderr,
            format!("{}\n", expected.message).as_bytes(),
            "{name}"
        );
    }
}

#[test]
fn native_pending_scalar_short_circuit_matches_interpreter() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/native/pending_scalar_short_circuit.jett");
    let expected = jett_driver::run_file_capture_output(&fixture).expect("interpreter oracle");
    let directory = tempfile::tempdir().expect("isolated pending scalar directory");
    let binary = directory.path().join("pending_scalar_short_circuit.exe");
    build_host_executable(&fixture, launcher(), &binary)
        .expect("compile pending scalar short circuit");
    let actual = run_bounded(&binary, directory.path());
    assert!(actual.status.success(), "{actual:?}");
    assert_eq!(actual.stdout, expected.stdout.as_bytes());
    assert_eq!(
        actual.stderr,
        format!("{}\n", expected.debug_output.join("\n")).as_bytes()
    );
}

#[test]
fn native_pending_enum_comparisons_match_interpreter_errors() {
    for name in [
        "pending_enum_equality_failure",
        "pending_enum_inequality_failure",
    ] {
        let fixture =
            Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("../../tests/native/{name}.jett"));
        let expected = jett_driver::run_file_capture_outcome(&fixture)
            .expect_err("interpreter must reject direct pending enum comparison");
        let directory = tempfile::tempdir().expect("isolated pending enum comparison directory");
        let binary = directory.path().join(format!("{name}.exe"));
        build_host_executable(&fixture, launcher(), &binary)
            .expect("compile pending enum comparison");
        let actual = run_bounded(&binary, directory.path());
        assert!(!actual.status.success(), "{name}: {actual:?}");
        assert_eq!(actual.stdout, expected.output.stdout.as_bytes(), "{name}");
        assert_eq!(
            actual.stderr,
            format!("{}\n", expected.message).as_bytes(),
            "{name}"
        );
    }
}

#[test]
fn native_pending_string_comparisons_match_interpreter_errors() {
    for name in [
        "pending_string_equality_failure",
        "pending_string_inequality_failure",
    ] {
        let fixture =
            Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("../../tests/native/{name}.jett"));
        let expected = jett_driver::run_file_capture_outcome(&fixture)
            .expect_err("interpreter must reject direct pending string comparison");
        let directory = tempfile::tempdir().expect("isolated pending string comparison directory");
        let binary = directory.path().join(format!("{name}.exe"));
        build_host_executable(&fixture, launcher(), &binary)
            .expect("compile pending string comparison");
        let actual = run_bounded(&binary, directory.path());
        assert!(!actual.status.success(), "{name}: {actual:?}");
        assert_eq!(actual.stdout, expected.output.stdout.as_bytes(), "{name}");
        assert_eq!(
            actual.stderr,
            format!("{}\n", expected.message).as_bytes(),
            "{name}"
        );
    }
}

#[test]
fn native_pending_nothing_comparisons_match_interpreter_errors() {
    for name in [
        "pending_nothing_equality_failure",
        "pending_nothing_inequality_failure",
    ] {
        let fixture =
            Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("../../tests/native/{name}.jett"));
        let expected = jett_driver::run_file_capture_outcome(&fixture)
            .expect_err("interpreter must reject direct pending nothing comparison");
        let directory = tempfile::tempdir().expect("isolated pending comparison directory");
        let binary = directory.path().join(format!("{name}.exe"));
        build_host_executable(&fixture, launcher(), &binary)
            .expect("compile pending nothing comparison");
        let actual = run_bounded(&binary, directory.path());
        assert!(!actual.status.success(), "{name}: {actual:?}");
        assert_eq!(actual.stdout, expected.output.stdout.as_bytes(), "{name}");
        assert_eq!(
            actual.stderr,
            format!("{}\n", expected.message).as_bytes(),
            "{name}"
        );
    }
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
fn native_reflected_struct_base_values_match_interpreter() {
    let fixture =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/native/refined_builder_base.jett");
    let expected = jett_driver::run_file_capture_output(&fixture).expect("interpreter oracle");
    assert_eq!(
        expected.stdout,
        "Ada\nrefinement type constraint failed for 'app.NonEmpty'\nrefinement type constraint failed for 'app.Long'\n"
    );
    let directory = tempfile::tempdir().expect("isolated execution directory");
    let binary = directory.path().join("program.exe");
    build_host_executable(&fixture, launcher(), &binary)
        .expect("compile reflected base-value builder");
    let actual = run_bounded(&binary, directory.path());
    assert!(actual.status.success(), "{actual:?}");
    assert_eq!(actual.stdout, expected.stdout.as_bytes());
    assert!(actual.stderr.is_empty(), "{actual:?}");
}

#[test]
fn native_reflected_enum_base_values_match_interpreter() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/native/refined_enum_builder_base.jett");
    let expected = jett_driver::run_file_capture_output(&fixture).expect("interpreter oracle");
    assert_eq!(
        expected.stdout,
        "idle\nAda:Agent:ready\nrefinement type constraint failed for 'app.NonEmpty'\nrefinement type constraint failed for 'app.Long'\n"
    );
    let directory = tempfile::tempdir().expect("isolated execution directory");
    let binary = directory.path().join("program.exe");
    build_host_executable(&fixture, launcher(), &binary)
        .expect("compile reflected enum base-value builder");
    let actual = run_bounded(&binary, directory.path());
    assert!(actual.status.success(), "{actual:?}");
    assert_eq!(actual.stdout, expected.stdout.as_bytes());
    assert!(actual.stderr.is_empty(), "{actual:?}");
}

#[test]
fn native_reflected_machine_base_values_match_interpreter() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/native/refined_machine_builder_base.jett");
    let expected = jett_driver::run_file_capture_output(&fixture).expect("interpreter oracle");
    assert_eq!(
        expected.stdout,
        "Ada:Agent:ready\nrefinement type constraint failed for 'app.NonEmpty'\nrefinement type constraint failed for 'app.Long'\nNia\nrefinement type constraint failed for 'app.NonEmpty'\nfilled\nrefinement type constraint failed for 'app.Positive'\nempty\n"
    );
    let directory = tempfile::tempdir().expect("isolated execution directory");
    let binary = directory.path().join("program.exe");
    build_host_executable(&fixture, launcher(), &binary)
        .expect("compile reflected machine base-value builder");
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
fn native_breakpoints_preserve_lexical_frames_and_capture_types() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/native/breakpoint_lexical_frames.jett");
    let expected = jett_driver::run_file_capture_output(&fixture).unwrap();
    assert_eq!(expected.debug_output.len(), 13);
    assert_eq!(
        &expected.debug_output[..3],
        [
            "breakpoint hit: value: int64 = 1",
            "breakpoint hit: nested: string = leaf, value: int64 = 1",
            "breakpoint hit: value: int64 = 1",
        ]
    );
    assert_eq!(
        expected.debug_output[7],
        "breakpoint hit: label: string = captured, seed: uint8 = 7, value: int64 = 3"
    );
    assert_eq!(expected.debug_output[11], expected.debug_output[7]);
    assert_eq!(
        expected
            .debug_output
            .iter()
            .filter(|line| line.contains("caller:"))
            .count(),
        2
    );
    let directory = tempfile::tempdir().unwrap();
    let binary = directory.path().join("breakpoint_frames.exe");
    build_host_executable(&fixture, launcher(), &binary).unwrap();
    let actual = run_bounded(&binary, directory.path());
    assert!(actual.status.success(), "{actual:?}");
    assert_eq!(actual.stdout, expected.stdout.as_bytes());
    assert_eq!(
        String::from_utf8_lossy(&actual.stderr),
        format!("{}\n", expected.debug_output.join("\n"))
    );
}

#[test]
fn native_release_strips_debug_observations_and_their_conditions() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/native/release_debug_observations.jett");
    let directory = tempfile::tempdir().unwrap();
    let expected = jett_driver::run_file_capture_output(&fixture).unwrap();
    for release in [false, true] {
        let binary = directory.path().join(format!("release_{release}.exe"));
        jett_driver::native::build_host_executable_with_options(
            &fixture,
            launcher(),
            &binary,
            jett_driver::BuildOptions { release },
        )
        .unwrap();
        let actual = run_bounded(&binary, directory.path());
        assert!(actual.status.success(), "{actual:?}");
        if release {
            assert_eq!(actual.stdout, b"program output\n");
            assert!(actual.stderr.is_empty(), "{actual:?}");
        } else {
            assert_eq!(actual.stdout, expected.stdout.as_bytes());
            assert_eq!(
                String::from_utf8_lossy(&actual.stderr),
                format!("{}\n", expected.debug_output.join("\n"))
            );
        }
    }
    let source = directory.path().join("debug_print.jett");
    std::fs::write(
        &source,
        "function main() returns nothing:\n    println(7)\n",
    )
    .unwrap();
    let output = directory.path().join("rejected.exe");
    let error = jett_driver::native::build_host_executable_with_options(
        &source,
        launcher(),
        &output,
        jett_driver::BuildOptions { release: true },
    )
    .unwrap_err();
    assert!(error.to_string().contains("E0362"), "{error}");
    assert!(!output.exists());
}

#[test]
fn native_breakpoints_omit_consumed_bindings() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/native/breakpoint_consumed_bindings.jett");
    let expected = jett_driver::run_file_capture_output(&fixture).unwrap();
    let debug = format!("{}\n", expected.debug_output.join("\n"));
    assert!(!debug.contains("initial:"), "{debug}");
    assert!(debug.contains("original_count: app.Count = 7"), "{debug}");
    assert!(debug.contains("copied_count: app.Count = 7"), "{debug}");
    assert!(debug.contains("breakpoint hit: moved: bytes"), "{debug}");
    assert!(debug.contains("breakpoint hit\n"), "{debug}");
    let directory = tempfile::tempdir().unwrap();
    let binary = directory.path().join("breakpoint_consumed.exe");
    build_host_executable(&fixture, launcher(), &binary).unwrap();
    let actual = run_bounded(&binary, directory.path());
    assert!(actual.status.success(), "{actual:?}");
    assert_eq!(actual.stdout, expected.stdout.as_bytes());
    assert_eq!(String::from_utf8_lossy(&actual.stderr), debug);
}

#[test]
fn native_secret_debug_values_are_recursively_redacted() {
    let fixture =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/native/debug_secret_values.jett");
    let expected = jett_driver::run_file_capture_output(&fixture).unwrap();
    let debug = format!("{}\n", expected.debug_output.join("\n"));
    assert!(debug.contains("[redacted]"), "{debug}");
    for secret in ["hidden-", "4321", "7654"] {
        assert!(!debug.contains(secret), "secret leaked: {debug}");
    }
    assert!(
        debug.contains("label: public-label, token: [redacted]"),
        "{debug}"
    );
    assert!(debug.contains("public-key: [redacted]"), "{debug}");
    assert!(
        debug.contains("TypeConstruction[app.Vault](token: [redacted])"),
        "{debug}"
    );
    let directory = tempfile::tempdir().unwrap();
    let binary = directory.path().join("secret_debug.exe");
    build_host_executable(&fixture, launcher(), &binary).unwrap();
    let actual = run_bounded(&binary, directory.path());
    assert!(actual.status.success(), "{actual:?}");
    assert_eq!(actual.stdout, expected.stdout.as_bytes());
    assert_eq!(String::from_utf8_lossy(&actual.stderr), debug);
}

#[test]
fn native_type_construction_debug_matches_interpreter() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/native/debug_type_construction.jett");
    let expected = jett_driver::run_file_capture_output(&fixture).expect("interpreter oracle");
    assert_eq!(expected.stdout, "Ada:7\n4\n3:4\nlogged_in\n");
    assert_eq!(expected.debug_output.len(), 9);
    let directory = tempfile::tempdir().expect("isolated builder debug directory");
    let binary = directory.path().join("debug_type_construction.exe");
    build_host_executable(&fixture, launcher(), &binary)
        .expect("compile partial reflected builders with trace");
    let actual = run_bounded(&binary, directory.path());
    assert!(actual.status.success(), "{actual:?}");
    assert_eq!(actual.stdout, expected.stdout.as_bytes());
    assert_eq!(
        String::from_utf8_lossy(&actual.stderr),
        format!("{}\n", expected.debug_output.join("\n"))
    );
}

#[test]
fn native_collection_callbacks_match_interpreter() {
    let fixture =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/native/collection_callbacks.jett");
    let expected = jett_driver::run_file_capture_output(&fixture).expect("interpreter oracle");
    assert_eq!(expected.stdout, "44:14:true:true:2\nscore-2:score-4\n1:2\n");
    assert_eq!(expected.debug_output, ["trace value: int64 = 2"]);
    let directory = tempfile::tempdir().expect("isolated collection callback directory");
    let binary = directory.path().join("collection_callbacks.exe");
    build_host_executable(&fixture, launcher(), &binary)
        .expect("compile higher-order collection helpers");
    let actual = run_bounded(&binary, directory.path());
    assert!(actual.status.success(), "{actual:?}");
    assert_eq!(actual.stdout, expected.stdout.as_bytes());
    assert_eq!(
        String::from_utf8_lossy(&actual.stderr),
        "trace value: int64 = 2\n"
    );
}

#[test]
fn native_collection_shapes_match_interpreter() {
    let fixture =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/native/collection_shapes.jett");
    let expected = jett_driver::run_file_capture_output(&fixture).expect("interpreter oracle");
    assert_eq!(expected.stdout, "2:1:one:1:1:b:3\n2:2\n2:a:1\n");
    let directory = tempfile::tempdir().expect("isolated collection shape directory");
    let binary = directory.path().join("collection_shapes.exe");
    build_host_executable(&fixture, launcher(), &binary)
        .expect("compile collection shape conversions");
    let actual = run_bounded(&binary, directory.path());
    assert!(actual.status.success(), "{actual:?}");
    assert_eq!(actual.stdout, expected.stdout.as_bytes());
    assert!(actual.stderr.is_empty(), "{actual:?}");
}

#[test]
fn native_json_refined_aggregates_match_interpreter() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/native/json_refined_aggregates.jett");
    let expected = jett_driver::run_file_capture_output(&fixture).expect("interpreter oracle");
    assert_eq!(
        expected.stdout,
        "Ada\nactive.name: refinement type constraint failed for 'app.NonEmpty'\nLin\nnamed.name: refinement type constraint failed for 'app.NonEmpty'\nAda:ready\nLin:ready\nTao\nactive.profile: refinement type constraint failed for 'app.NamedProfile'\nMira\nNia\n"
    );
    let directory = tempfile::tempdir().expect("isolated refined aggregate directory");
    let binary = directory.path().join("json_refined_aggregates.exe");
    build_host_executable(&fixture, launcher(), &binary)
        .expect("compile reflected enum and machine construction with validated fields");
    let actual = run_bounded(&binary, directory.path());
    assert!(actual.status.success(), "{actual:?}");
    assert_eq!(actual.stdout, expected.stdout.as_bytes());
    assert!(actual.stderr.is_empty(), "{actual:?}");
}

#[test]
fn native_function_debug_values_match_interpreter() {
    let fixture =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/native/debug_functions.jett");
    let expected = jett_driver::run_file_capture_output(&fixture).expect("interpreter oracle");
    assert_eq!(expected.stdout, "3 5 9 5 5 9\n3 9 9 9\n4 6 10\n12 kept\n");
    let debug = format!("{}\n", expected.debug_output.join("\n"));
    assert!(debug.contains("function(callback_library.increment)"));
    assert!(debug.contains("function(left, right)"));
    assert!(debug.contains("trace baked_inline: function(int64) returns int64 = function(source)"));
    assert!(!debug.contains("DO_NOT_RENDER_CAPTURE"));
    assert_eq!(debug.matches("breakpoint hit:").count(), 1);
    let directory = tempfile::tempdir().expect("isolated function debug directory");
    let binary = directory.path().join("debug_functions.exe");
    build_host_executable(&fixture, launcher(), &binary)
        .expect("compile function values and nested callback debug layouts");
    let actual = run_bounded(&binary, directory.path());
    assert!(actual.status.success(), "{actual:?}");
    assert_eq!(actual.stdout, expected.stdout.as_bytes());
    assert_eq!(String::from_utf8_lossy(&actual.stderr), debug);

    let verify_binary = directory.path().join("debug_functions_verify.exe");
    build_host_verify_suite_executable(&fixture, launcher(), &verify_binary)
        .expect("compile callback traces in a native verify suite");
    let verified = run_bounded(&verify_binary, directory.path());
    assert!(verified.status.success(), "{verified:?}");
    assert!(verified.stdout.is_empty(), "{verified:?}");
    assert_eq!(
        String::from_utf8_lossy(&verified.stderr),
        "trace verify_named: function(int64) returns int64 = function(test.double)\ntrace verify_captured: function(int64) returns int64 = function(input)\n"
    );

    let property_binary = directory.path().join("debug_functions_property.exe");
    build_host_property_suite_executable(&fixture, launcher(), &property_binary)
        .expect("compile callback traces across generated native property trials");
    let property = run_bounded(&property_binary, directory.path());
    assert!(property.status.success(), "{property:?}");
    assert!(property.stdout.is_empty(), "{property:?}");
    assert_eq!(
        String::from_utf8_lossy(&property.stderr),
        "trace property_callback: function(int64) returns int64 = function(test.double)\n"
            .repeat(100)
    );
}

#[test]
fn native_actor_debug_values_match_interpreter() {
    let fixture =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/native/debug_actor_values.jett");
    let expected = jett_driver::run_file_capture_output(&fixture).expect("interpreter oracle");
    assert!(expected.stdout.is_empty());
    assert_eq!(
        expected.debug_output,
        [
            "trace first: test.Holder = actor#0",
            "trace second: test.Holder = actor#1",
            "trace active: list[test.Holder] = list(actor#2)",
            "breakpoint hit: active: list[test.Holder] = list(actor#2), first: test.Holder = actor#0, second: test.Holder = actor#1",
        ]
    );
    let directory = tempfile::tempdir().expect("isolated actor debug directory");
    let binary = directory.path().join("debug_actor_values.exe");
    build_host_executable(&fixture, launcher(), &binary)
        .expect("compile actor traces and breakpoint without consuming handles");
    let actual = run_bounded(&binary, directory.path());
    assert!(actual.status.success(), "{actual:?}");
    assert!(actual.stdout.is_empty(), "{actual:?}");
    assert_eq!(
        String::from_utf8_lossy(&actual.stderr),
        format!("{}\n", expected.debug_output.join("\n"))
    );
}

#[test]
fn native_comptime_namespace_aliases_match_interpreter() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/native/comptime_namespace_aliases.jett");
    let expected = jett_driver::run_file_capture_output(&fixture).expect("interpreter oracle");
    assert_eq!(expected.stdout, "6 6\n105\n6\n");
    let directory = tempfile::tempdir().expect("isolated comptime namespace alias directory");
    let binary = directory.path().join("comptime_namespace_aliases.exe");
    build_host_executable(&fixture, launcher(), &binary)
        .expect("compile baked callbacks through scoped namespace aliases");
    let actual = run_bounded(&binary, directory.path());
    assert!(actual.status.success(), "{actual:?}");
    assert_eq!(actual.stdout, expected.stdout.as_bytes());
    assert!(actual.stderr.is_empty(), "{actual:?}");

    let verify_binary = directory
        .path()
        .join("comptime_namespace_aliases_verify.exe");
    build_host_verify_suite_executable(&fixture, launcher(), &verify_binary)
        .expect("compile alias-aware baked callback in native verify body");
    let verified = run_bounded(&verify_binary, directory.path());
    assert!(verified.status.success(), "{verified:?}");
    assert!(verified.stdout.is_empty(), "{verified:?}");
    assert!(verified.stderr.is_empty(), "{verified:?}");

    let property_binary = directory
        .path()
        .join("comptime_namespace_aliases_property.exe");
    build_host_property_suite_executable(&fixture, launcher(), &property_binary)
        .expect("compile alias-aware baked callback across native property trials");
    let property = run_bounded(&property_binary, directory.path());
    assert!(property.status.success(), "{property:?}");
    assert!(property.stdout.is_empty(), "{property:?}");
    assert!(property.stderr.is_empty(), "{property:?}");
}

#[test]
fn native_method_function_values_match_interpreter() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/native/method_function_values.jett");
    let expected = jett_driver::run_file_capture_output(&fixture).expect("interpreter oracle");
    assert_eq!(
        expected.stdout,
        "10 24 105\n10 10 10 10\n10 10 10\n10 24 10 10\n10 10 12 13\n1010\n"
    );
    let debug = format!("{}\n", expected.debug_output.join("\n"));
    assert!(debug.contains("function(method_library.Point.amount)"));
    assert!(debug.contains("function(method_library.Counter.amount)"));
    assert!(debug.contains("function(alternate_library.Point.amount)"));
    assert!(debug.contains("function(input)"));
    let directory = tempfile::tempdir().expect("isolated method callback directory");
    let binary = directory.path().join("method_function_values.exe");
    build_host_executable(&fixture, launcher(), &binary)
        .expect("compile source method values and baked method callbacks");
    let actual = run_bounded(&binary, directory.path());
    assert!(actual.status.success(), "{actual:?}");
    assert_eq!(actual.stdout, expected.stdout.as_bytes());
    assert_eq!(String::from_utf8_lossy(&actual.stderr), debug);

    let verify_binary = directory.path().join("method_function_values_verify.exe");
    build_host_verify_suite_executable(&fixture, launcher(), &verify_binary)
        .expect("compile method callbacks in native verify bodies");
    let verified = run_bounded(&verify_binary, directory.path());
    assert!(verified.status.success(), "{verified:?}");
    assert!(verified.stdout.is_empty(), "{verified:?}");
    assert!(verified.stderr.is_empty(), "{verified:?}");

    let property_binary = directory.path().join("method_function_values_property.exe");
    build_host_property_suite_executable(&fixture, launcher(), &property_binary)
        .expect("compile method callbacks across native property trials");
    let property = run_bounded(&property_binary, directory.path());
    assert!(property.status.success(), "{property:?}");
    assert!(property.stdout.is_empty(), "{property:?}");
    assert!(property.stderr.is_empty(), "{property:?}");
}

#[test]
fn native_function_debug_values_clean_up_after_failure() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/native/debug_functions_failure.jett");
    let expected = jett_driver::run_file_capture_outcome(&fixture)
        .expect_err("failure after tracing owned function descriptors");
    let debug = format!("{}\n", expected.output.debug_output.join("\n"));
    let directory = tempfile::tempdir().expect("isolated function debug failure directory");
    let binary = directory.path().join("debug_functions_failure.exe");
    build_host_executable(&fixture, launcher(), &binary)
        .expect("compile a terminal failure with live function debug metadata");
    let actual = run_bounded(&binary, directory.path());
    assert_eq!(actual.status.code(), Some(71), "{actual:?}");
    assert_eq!(actual.stdout, expected.output.stdout.as_bytes());
    assert_eq!(
        String::from_utf8_lossy(&actual.stderr),
        format!("{debug}{}\n", expected.message)
    );
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
fn native_enum_struct_payload_matches_interpreter() {
    let fixture =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/native/enum_struct_payload.jett");
    let expected = jett_driver::run_file_capture_output(&fixture).expect("interpreter oracle");
    let directory = tempfile::tempdir().expect("isolated enum payload directory");
    let binary = directory.path().join("enum_struct_payload.exe");
    build_host_executable(&fixture, launcher(), &binary).expect("compile enum struct payload");
    let actual = run_bounded(&binary, directory.path());
    assert!(actual.status.success(), "{actual:?}");
    assert_eq!(actual.stdout, expected.stdout.as_bytes());
    assert_eq!(
        actual.stderr,
        format!("{}\n", expected.debug_output.join("\n")).as_bytes()
    );
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

#[test]
fn native_field_receiver_nested_handle_matches_interpreter() {
    let fixture =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/native/nested_field_handle.jett");
    let expected = jett_driver::run_file_capture_output(&fixture).expect("interpreter oracle");
    assert_eq!(
        expected.stdout,
        "Ada\nfallback\nAda\nmissing\nLin\noptional fallback\nKai\nnested fallback\n"
    );
    let directory = tempfile::tempdir().expect("isolated field receiver directory");
    let binary = directory.path().join("nested_field_handle.exe");
    build_host_executable(&fixture, launcher(), &binary)
        .expect("compile field receiver with nested handlers");
    let actual = run_bounded(&binary, directory.path());
    assert!(actual.status.success(), "{actual:?}");
    assert_eq!(actual.stdout, expected.stdout.as_bytes());
    assert!(actual.stderr.is_empty(), "{actual:?}");
}

#[test]
fn native_state_condition_nested_handle_matches_interpreter() {
    let fixture =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/native/nested_state_handle.jett");
    let expected = jett_driver::run_file_capture_output(&fixture).expect("interpreter oracle");
    assert_eq!(expected.stdout, "active\nguest\n2\n");
    let directory = tempfile::tempdir().expect("isolated state condition directory");
    let binary = directory.path().join("nested_state_handle.exe");
    build_host_executable(&fixture, launcher(), &binary)
        .expect("compile state test with nested handlers");
    let actual = run_bounded(&binary, directory.path());
    assert!(actual.status.success(), "{actual:?}");
    assert_eq!(actual.stdout, expected.stdout.as_bytes());
    assert!(actual.stderr.is_empty(), "{actual:?}");
}

#[test]
fn native_for_iterable_nested_handle_matches_interpreter() {
    let fixture =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/native/nested_for_handle.jett");
    let expected = jett_driver::run_file_capture_output(&fixture).expect("interpreter oracle");
    assert_eq!(
        expected.stdout,
        "source\nAda\nLin\nsource\nfallback\nsource\nmissing\n"
    );
    let directory = tempfile::tempdir().expect("isolated for iterable directory");
    let binary = directory.path().join("nested_for_handle.exe");
    build_host_executable(&fixture, launcher(), &binary)
        .expect("compile for iterable with nested handler");
    let actual = run_bounded(&binary, directory.path());
    assert!(actual.status.success(), "{actual:?}");
    assert_eq!(actual.stdout, expected.stdout.as_bytes());
    assert!(actual.stderr.is_empty(), "{actual:?}");
}

#[test]
fn native_debug_conditions_nested_handle_match_interpreter() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/native/nested_debug_condition_handle.jett");
    let expected = jett_driver::run_file_capture_output(&fixture).expect("interpreter oracle");
    assert_eq!(expected.stdout, "after\n");
    assert_eq!(expected.debug_output, ["breakpoint hit"]);
    let directory = tempfile::tempdir().expect("isolated debug condition directory");
    let binary = directory.path().join("nested_debug_condition_handle.exe");
    build_host_executable(&fixture, launcher(), &binary)
        .expect("compile breakpoint condition with nested handlers");
    let actual = run_bounded(&binary, directory.path());
    assert!(actual.status.success(), "{actual:?}");
    assert_eq!(actual.stdout, expected.stdout.as_bytes());
    assert_eq!(String::from_utf8_lossy(&actual.stderr), "breakpoint hit\n");

    let verify_binary = directory.path().join("nested_debug_condition_verify.exe");
    let artifact = build_host_verify_suite_executable(&fixture, launcher(), &verify_binary)
        .expect("compile assertion condition with nested handler");
    let verified = run_bounded(&artifact.path, directory.path());
    assert!(verified.status.success(), "{verified:?}");
    assert!(verified.stdout.is_empty(), "{verified:?}");
    assert!(verified.stderr.is_empty(), "{verified:?}");
}

#[test]
fn native_machine_transition_nested_handlers_match_interpreter() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/native/nested_transition_handle.jett");
    let expected = jett_driver::run_file_capture_output(&fixture).expect("interpreter oracle");
    assert_eq!(
        expected.stdout,
        "source\npayload\nBefore\nBefore:Ada\nsource\npayload\nChanged\nBefore:Fallback\n"
    );
    let directory = tempfile::tempdir().expect("isolated machine transition directory");
    let binary = directory.path().join("nested_transition_handle.exe");
    build_host_executable(&fixture, launcher(), &binary)
        .expect("compile machine transition with nested handlers");
    let actual = run_bounded(&binary, directory.path());
    assert!(actual.status.success(), "{actual:?}");
    assert_eq!(actual.stdout, expected.stdout.as_bytes());
    assert!(actual.stderr.is_empty(), "{actual:?}");
}

#[test]
fn native_task_operands_nested_handlers_match_interpreter() {
    let fixture =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/native/nested_task_handle.jett");
    let expected = jett_driver::run_file_capture_output(&fixture).expect("interpreter oracle");
    assert_eq!(
        expected.stdout,
        "Ada\nFallback\nsource\nLin\nsource\nFallback\njoined\njoined\n"
    );
    let directory = tempfile::tempdir().expect("isolated task operand directory");
    let binary = directory.path().join("nested_task_handle.exe");
    build_host_executable(&fixture, launcher(), &binary)
        .expect("compile run and join operands with nested handlers");
    let actual = run_bounded(&binary, directory.path());
    assert!(actual.status.success(), "{actual:?}");
    assert_eq!(actual.stdout, expected.stdout.as_bytes());
    assert!(actual.stderr.is_empty(), "{actual:?}");
}

#[test]
fn native_actor_spawn_arguments_nested_handlers_match_interpreter() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/native/nested_actor_spawn_handle.jett");
    let expected = jett_driver::run_file_capture_output(&fixture).expect("interpreter oracle");
    assert_eq!(
        expected.stdout,
        "count\nseed\n7\nAda:3\ncount\nseed\n9\nFallback:7\n"
    );
    let directory = tempfile::tempdir().expect("isolated actor spawn directory");
    let binary = directory.path().join("nested_actor_spawn_handle.exe");
    build_host_executable(&fixture, launcher(), &binary)
        .expect("compile actor spawn arguments with nested handlers");
    let actual = run_bounded(&binary, directory.path());
    assert!(actual.status.success(), "{actual:?}");
    assert_eq!(actual.stdout, expected.stdout.as_bytes());
    assert!(actual.stderr.is_empty(), "{actual:?}");
}

#[test]
fn native_actor_message_arguments_nested_handlers_match_interpreter() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/native/nested_actor_message_handle.jett");
    let expected = jett_driver::run_file_capture_output(&fixture).expect("interpreter oracle");
    assert_eq!(
        expected.stdout,
        "text\ntext\nBefore\nBefore:Ada:Ada\nAda\ntext\ntext\nChanged\nBefore:Fallback:Fallback\nFallback\nAda\nFallback\n"
    );
    let directory = tempfile::tempdir().expect("isolated actor message directory");
    let binary = directory.path().join("nested_actor_message_handle.exe");
    build_host_executable(&fixture, launcher(), &binary)
        .expect("compile actor message arguments with nested handlers");
    let actual = run_bounded(&binary, directory.path());
    assert!(actual.status.success(), "{actual:?}");
    assert_eq!(actual.stdout, expected.stdout.as_bytes());
    assert!(actual.stderr.is_empty(), "{actual:?}");
}

#[test]
fn native_arithmetic_width_matrix_matches_interpreter() {
    let fixture =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/native/arithmetic_matrix.jett");
    let expected = jett_driver::run_file_capture_output(&fixture).expect("interpreter oracle");
    assert_eq!(
        expected.stdout,
        concat!(
            "i8 -128 127 -2 -128 0 true\n",
            "i16 -32768 32767 -2 -32768 0 true\n",
            "i32 -2147483648 2147483647 -2 -2147483648 0 true\n",
            "i64 -9223372036854775808 9223372036854775807 -2 -9223372036854775808 0 true\n",
            "u8 0 255 254 127 1 true\n",
            "u16 0 65535 65534 32767 1 true\n",
            "u32 0 4294967295 4294967294 2147483647 1 true\n",
            "u64 0 18446744073709551615 18446744073709551614 9223372036854775807 1 true\n",
            "f32 -1.25 3.75 -3.125 -0.5 true true false true true\n",
            "f64 -1.25 3.75 -3.125 -0.5 true true false true true\n",
        )
    );
    let directory = tempfile::tempdir().expect("isolated arithmetic matrix directory");
    let binary = directory.path().join("arithmetic_matrix.exe");
    build_host_executable(&fixture, launcher(), &binary).expect("compile arithmetic matrix");
    let actual = run_bounded(&binary, directory.path());
    assert!(actual.status.success(), "{actual:?}");
    assert_eq!(actual.stdout, expected.stdout.as_bytes());
    assert!(actual.stderr.is_empty(), "{actual:?}");
}

#[test]
fn native_nested_json_enums_match_interpreter() {
    let fixture =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/native/json_nested_enum.jett");
    let expected = jett_driver::run_file_capture_output(&fixture).expect("interpreter oracle");
    assert_eq!(
        expected.stdout,
        "[\"north\",\"south\"]\n[{\"point\":[1,2]},\"empty\"]\n{\"a\":\"north\",\"b\":\"south\"}\n[{\"recorded\":[{\"name\":\"Ada\"}]},\"idle\"]\n"
    );
    let directory = tempfile::tempdir().expect("isolated nested JSON enum directory");
    let binary = directory.path().join("json_nested_enum.exe");
    build_host_executable(&fixture, launcher(), &binary).expect("compile nested JSON enums");
    let actual = run_bounded(&binary, directory.path());
    assert!(actual.status.success(), "{actual:?}");
    assert_eq!(actual.stdout, expected.stdout.as_bytes());
    assert!(actual.stderr.is_empty(), "{actual:?}");
}

#[test]
fn native_nested_json_bitfields_match_interpreter() {
    let fixture =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/native/json_nested_bitfield.jett");
    let expected = jett_driver::run_file_capture_output(&fixture).expect("interpreter oracle");
    assert_eq!(
        expected.stdout,
        "{\"title\":\"packet\",\"header\":{\"version\":4,\"flags\":2,\"protocol\":\"tcp\",\"payload\":[1,255]}}\nheader: protocol: expected enum string or object, got number\nheader: payload: 0: expected uint8 in range 0..255\n"
    );
    let directory = tempfile::tempdir().expect("isolated nested bitfield directory");
    let binary = directory.path().join("json_nested_bitfield.exe");
    build_host_executable(&fixture, launcher(), &binary).expect("compile nested bitfield JSON");
    let actual = run_bounded(&binary, directory.path());
    assert!(actual.status.success(), "{actual:?}");
    assert_eq!(actual.stdout, expected.stdout.as_bytes());
    assert!(actual.stderr.is_empty(), "{actual:?}");
}

#[test]
fn native_nested_json_machines_match_interpreter() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/native/json_nested_machine_parse.jett");
    let expected = jett_driver::run_file_capture_output(&fixture).expect("interpreter oracle");
    let directory = tempfile::tempdir().expect("isolated nested machine JSON directory");
    let binary = directory.path().join("json_nested_machine_parse.exe");
    build_host_executable(&fixture, launcher(), &binary).expect("compile nested machine JSON");
    let actual = run_bounded(&binary, directory.path());
    assert!(actual.status.success(), "{actual:?}");
    assert_eq!(actual.stdout, expected.stdout.as_bytes());
    assert!(actual.stderr.is_empty(), "{actual:?}");
}

#[test]
fn native_projected_machine_sequences_match_interpreter() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/native/projected_machine_sequences.jett");
    let expected = jett_driver::run_file_capture_output(&fixture).expect("interpreter oracle");
    assert_eq!(expected.stdout, "a 5\nb 7\n30 2 2 2\n30 30\n");
    let directory = tempfile::tempdir().expect("isolated machine projection directory");
    let binary = directory.path().join("projected_machine_sequences.exe");
    build_host_executable(&fixture, launcher(), &binary)
        .expect("compile iteration through machine fields");
    let actual = run_bounded(&binary, directory.path());
    assert!(actual.status.success(), "{actual:?}");
    assert_eq!(actual.stdout, expected.stdout.as_bytes());
    assert!(actual.stderr.is_empty(), "{actual:?}");
}

#[test]
fn native_projected_nested_collections_match_interpreter() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/native/projected_nested_collections.jett");
    let expected = jett_driver::run_file_capture_output(&fixture).expect("interpreter oracle");
    let directory = tempfile::tempdir().expect("isolated nested projection directory");
    let binary = directory.path().join("projected_nested_collections.exe");
    build_host_executable(&fixture, launcher(), &binary)
        .expect("compile nested collection projection");
    let actual = run_bounded(&binary, directory.path());
    assert!(actual.status.success(), "{actual:?}");
    assert_eq!(actual.stdout, expected.stdout.as_bytes());
    assert!(actual.stderr.is_empty(), "{actual:?}");
}

#[test]
fn native_nested_handle_projected_view_matches_interpreter() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/native/nested_handle_projected_view.jett");
    let expected = jett_driver::run_file_capture_output(&fixture).expect("interpreter oracle");
    assert_eq!(expected.stdout, "3 8\n10 13\n");
    let directory = tempfile::tempdir().expect("isolated projected handler directory");
    let binary = directory.path().join("nested_handle_projected_view.exe");
    build_host_executable(&fixture, launcher(), &binary)
        .expect("compile projected view before handler");
    let actual = run_bounded(&binary, directory.path());
    assert!(actual.status.success(), "{actual:?}");
    assert_eq!(actual.stdout, expected.stdout.as_bytes());
    assert!(actual.stderr.is_empty(), "{actual:?}");
}

#[test]
fn native_reused_handle_locals_match_interpreter() {
    let fixture =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/native/reused_handle_locals.jett");
    let expected = jett_driver::run_file_capture_output(&fixture).expect("interpreter oracle");
    assert_eq!(expected.stdout, "1 2\n7 7\n5 5\n4\n2 2\n");
    let directory = tempfile::tempdir().expect("isolated reused handle directory");
    let binary = directory.path().join("reused_handle_locals.exe");
    build_host_executable(&fixture, launcher(), &binary).expect("compile reused local handles");
    let actual = run_bounded(&binary, directory.path());
    assert!(actual.status.success(), "{actual:?}");
    assert_eq!(actual.stdout, expected.stdout.as_bytes());
    assert!(actual.stderr.is_empty(), "{actual:?}");
}

#[test]
fn native_projected_sum_handles_match_interpreter() {
    let fixture =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/native/projected_sum_handles.jett");
    let expected = jett_driver::run_file_capture_output(&fixture).expect("interpreter oracle");
    assert_eq!(expected.stdout, "18 18\n");
    let directory = tempfile::tempdir().expect("isolated projected sum directory");
    let binary = directory.path().join("projected_sum_handles.exe");
    build_host_executable(&fixture, launcher(), &binary).expect("compile projected sum handles");
    let actual = run_bounded(&binary, directory.path());
    assert!(actual.status.success(), "{actual:?}");
    assert_eq!(actual.stdout, expected.stdout.as_bytes());
    assert!(actual.stderr.is_empty(), "{actual:?}");
}

#[test]
fn native_join_local_reuse_matches_interpreter() {
    let fixture =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/native/join_local_reuse.jett");
    let expected = jett_driver::run_file_capture_output(&fixture).expect("interpreter oracle");
    let directory = tempfile::tempdir().expect("isolated join reuse directory");
    let binary = directory.path().join("join_local_reuse.exe");
    build_host_executable(&fixture, launcher(), &binary).expect("compile reusable join local");
    let actual = run_bounded(&binary, directory.path());
    assert!(actual.status.success(), "{actual:?}");
    assert_eq!(actual.stdout, expected.stdout.as_bytes());
    assert_eq!(
        actual.stderr,
        format!("{}\n", expected.debug_output.join("\n")).as_bytes()
    );
}

#[test]
fn native_run_local_aggregate_reuse_matches_interpreter() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/native/run_local_aggregate_reuse.jett");
    let expected = jett_driver::run_file_capture_output(&fixture).expect("interpreter oracle");
    let directory = tempfile::tempdir().expect("isolated run-local directory");
    let binary = directory.path().join("run_local_aggregate_reuse.exe");
    build_host_executable(&fixture, launcher(), &binary).expect("compile local aggregate run");
    let actual = run_bounded(&binary, directory.path());
    assert!(actual.status.success(), "{actual:?}");
    assert_eq!(actual.stdout, expected.stdout.as_bytes());
    assert_eq!(
        actual.stderr,
        format!("{}\n", expected.debug_output.join("\n")).as_bytes()
    );
}

#[test]
fn native_run_local_builder_reuse_matches_interpreter() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/native/run_local_builder_reuse.jett");
    let expected = jett_driver::run_file_capture_output(&fixture).expect("interpreter oracle");
    let directory = tempfile::tempdir().expect("isolated run-local builder directory");
    let binary = directory.path().join("run_local_builder_reuse.exe");
    build_host_executable(&fixture, launcher(), &binary).expect("compile local builder run");
    let actual = run_bounded(&binary, directory.path());
    assert!(actual.status.success(), "{actual:?}");
    assert_eq!(actual.stdout, expected.stdout.as_bytes());
    assert_eq!(
        actual.stderr,
        format!("{}\n", expected.debug_output.join("\n")).as_bytes()
    );
}

#[test]
fn native_transparent_local_reuse_matches_interpreter() {
    for name in [
        "coarsen_local_refinement_reuse",
        "declassify_local_secret_reuse",
    ] {
        let fixture =
            Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("../../tests/native/{name}.jett"));
        let expected = jett_driver::run_file_capture_output(&fixture).expect("interpreter oracle");
        let directory = tempfile::tempdir().expect("isolated transparent conversion directory");
        let binary = directory.path().join(format!("{name}.exe"));
        build_host_executable(&fixture, launcher(), &binary)
            .expect("compile transparent local conversion");
        let actual = run_bounded(&binary, directory.path());
        assert!(actual.status.success(), "{name}: {actual:?}");
        assert_eq!(actual.stdout, expected.stdout.as_bytes(), "{name}");
        let debug = if expected.debug_output.is_empty() {
            String::new()
        } else {
            format!("{}\n", expected.debug_output.join("\n"))
        };
        assert_eq!(actual.stderr, debug.as_bytes(), "{name}");
    }
}

#[test]
fn native_projected_string_iteration_matches_interpreter() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/native/projected_string_iteration.jett");
    let expected = jett_driver::run_file_capture_output(&fixture).expect("interpreter oracle");
    assert_eq!(expected.stdout, "4 4\n");
    let directory = tempfile::tempdir().expect("isolated projected string directory");
    let binary = directory.path().join("projected_string_iteration.exe");
    build_host_executable(&fixture, launcher(), &binary)
        .expect("compile projected string iteration");
    let actual = run_bounded(&binary, directory.path());
    assert!(actual.status.success(), "{actual:?}");
    assert_eq!(actual.stdout, expected.stdout.as_bytes());
    assert!(actual.stderr.is_empty(), "{actual:?}");
}

#[test]
fn native_nested_borrowed_collections_match_interpreter() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/native/nested_borrowed_collections.jett");
    let expected = jett_driver::run_file_capture_output(&fixture).expect("interpreter oracle");
    assert!(expected.stdout.ends_with("11 11\n2 2\n"), "{expected:?}");
    let directory = tempfile::tempdir().expect("isolated nested collection directory");
    let binary = directory.path().join("nested_borrowed_collections.exe");
    build_host_executable(&fixture, launcher(), &binary)
        .expect("compile nested borrowed collection iteration");
    let actual = run_bounded(&binary, directory.path());
    assert!(actual.status.success(), "{actual:?}");
    assert_eq!(actual.stdout, expected.stdout.as_bytes());
    assert!(actual.stderr.is_empty(), "{actual:?}");
}

#[test]
fn native_nested_machine_borrowed_collections_match_interpreter() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/native/nested_machine_borrowed_collections.jett");
    let expected = jett_driver::run_file_capture_output(&fixture).expect("interpreter oracle");
    assert!(expected.stdout.ends_with("6 6\n"), "{expected:?}");
    let directory = tempfile::tempdir().expect("isolated nested machine collection directory");
    let binary = directory
        .path()
        .join("nested_machine_borrowed_collections.exe");
    build_host_executable(&fixture, launcher(), &binary)
        .expect("compile nested borrowed machine collection iteration");
    let actual = run_bounded(&binary, directory.path());
    assert!(actual.status.success(), "{actual:?}");
    assert_eq!(actual.stdout, expected.stdout.as_bytes());
    assert!(actual.stderr.is_empty(), "{actual:?}");
}

#[test]
fn native_generic_empty_collections_match_interpreter() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/native/generic_empty_collections.jett");
    let expected = jett_driver::run_file_capture_output(&fixture).expect("interpreter oracle");
    assert_eq!(expected.stdout, "0 0 0\n0 0\n1 1\nAda true\n");
    let directory = tempfile::tempdir().expect("isolated generic collection directory");
    let binary = directory.path().join("generic_empty_collections.exe");
    build_host_executable(&fixture, launcher(), &binary)
        .expect("compile generic empty collections");
    let actual = run_bounded(&binary, directory.path());
    assert!(actual.status.success(), "{actual:?}");
    assert_eq!(actual.stdout, expected.stdout.as_bytes());
    assert!(actual.stderr.is_empty(), "{actual:?}");
}

#[test]
fn native_generic_map_aggregates_match_interpreter() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/native/generic_map_aggregates.jett");
    let expected = jett_driver::run_file_capture_output(&fixture).expect("interpreter oracle");
    assert_eq!(expected.stdout, "0 0\nAda true 2\n");
    let directory = tempfile::tempdir().expect("isolated generic map directory");
    let binary = directory.path().join("generic_map_aggregates.exe");
    build_host_executable(&fixture, launcher(), &binary)
        .expect("compile generic maps of aggregates");
    let actual = run_bounded(&binary, directory.path());
    assert!(actual.status.success(), "{actual:?}");
    assert_eq!(actual.stdout, expected.stdout.as_bytes());
    assert!(actual.stderr.is_empty(), "{actual:?}");
}

#[test]
fn native_displayable_interpolation_matches_interpreter() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/native/displayable_interpolation.jett");
    let expected = jett_driver::run_file_capture_output(&fixture).expect("interpreter oracle");
    assert_eq!(expected.stdout, "Hello user:Ada!\nResult user:Grace\n");
    let directory = tempfile::tempdir().expect("isolated displayable directory");
    let binary = directory.path().join("displayable_interpolation.exe");
    build_host_executable(&fixture, launcher(), &binary)
        .expect("compile displayable interpolation");
    let actual = run_bounded(&binary, directory.path());
    assert!(actual.status.success(), "{actual:?}");
    assert_eq!(actual.stdout, expected.stdout.as_bytes());
    assert!(actual.stderr.is_empty(), "{actual:?}");
}

#[test]
fn native_handled_interpolation_matches_interpreter() {
    let fixture =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/native/handled_interpolation.jett");
    let expected = jett_driver::run_file_capture_output(&fixture).expect("interpreter oracle");
    assert_eq!(
        expected.stdout,
        "value 3\nfirst\nfallback\nlast\nordered first 7 last\n"
    );
    let directory = tempfile::tempdir().expect("isolated handled interpolation directory");
    let binary = directory.path().join("handled_interpolation.exe");
    build_host_executable(&fixture, launcher(), &binary).expect("compile handled interpolation");
    let actual = run_bounded(&binary, directory.path());
    assert!(actual.status.success(), "{actual:?}");
    assert_eq!(actual.stdout, expected.stdout.as_bytes());
    assert!(actual.stderr.is_empty(), "{actual:?}");
}

#[test]
fn native_match_scrutinee_nested_handle_matches_interpreter() {
    let fixture =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/native/nested_match_handle.jett");
    let expected = jett_driver::run_file_capture_output(&fixture).expect("interpreter oracle");
    assert_eq!(expected.stdout, "hello\nempty\n");
    let directory = tempfile::tempdir().expect("isolated match scrutinee directory");
    let binary = directory.path().join("nested_match_handle.exe");
    build_host_executable(&fixture, launcher(), &binary)
        .expect("compile match scrutinee with nested handler");
    let actual = run_bounded(&binary, directory.path());
    assert!(actual.status.success(), "{actual:?}");
    assert_eq!(actual.stdout, expected.stdout.as_bytes());
    assert!(actual.stderr.is_empty(), "{actual:?}");
}

#[test]
fn native_clone_operand_nested_handle_matches_interpreter() {
    let fixture =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/native/nested_clone_handle.jett");
    let expected = jett_driver::run_file_capture_output(&fixture).expect("interpreter oracle");
    assert_eq!(expected.stdout, "Ada\nfallback\n");
    let directory = tempfile::tempdir().expect("isolated clone operand directory");
    let binary = directory.path().join("nested_clone_handle.exe");
    build_host_executable(&fixture, launcher(), &binary)
        .expect("compile clone operand with nested handler");
    let actual = run_bounded(&binary, directory.path());
    assert!(actual.status.success(), "{actual:?}");
    assert_eq!(actual.stdout, expected.stdout.as_bytes());
    assert!(actual.stderr.is_empty(), "{actual:?}");
}

#[test]
fn native_coarsen_operand_nested_handle_matches_interpreter() {
    let fixture =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/native/nested_coarsen_handle.jett");
    let expected = jett_driver::run_file_capture_output(&fixture).expect("interpreter oracle");
    assert_eq!(expected.stdout, "Ada\nFallback\n");
    let directory = tempfile::tempdir().expect("isolated coarsen operand directory");
    let binary = directory.path().join("nested_coarsen_handle.exe");
    build_host_executable(&fixture, launcher(), &binary)
        .expect("compile coarsen operand with nested handler");
    let actual = run_bounded(&binary, directory.path());
    assert!(actual.status.success(), "{actual:?}");
    assert_eq!(actual.stdout, expected.stdout.as_bytes());
    assert!(actual.stderr.is_empty(), "{actual:?}");
}

#[test]
fn native_declassify_operand_nested_handle_matches_interpreter() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/native/nested_declassify_handle.jett");
    let expected = jett_driver::run_file_capture_output(&fixture).expect("interpreter oracle");
    assert_eq!(expected.stdout, "Ada\nFallback\n");
    let directory = tempfile::tempdir().expect("isolated declassify operand directory");
    let binary = directory.path().join("nested_declassify_handle.exe");
    build_host_executable(&fixture, launcher(), &binary)
        .expect("compile declassify operand with nested handler");
    let actual = run_bounded(&binary, directory.path());
    assert!(actual.status.success(), "{actual:?}");
    assert_eq!(actual.stdout, expected.stdout.as_bytes());
    assert!(actual.stderr.is_empty(), "{actual:?}");
}

#[test]
fn native_enum_binary_nested_handle_matches_interpreter() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/native/nested_handle_enum_binary.jett");
    let expected = jett_driver::run_file_capture_output(&fixture).expect("interpreter oracle");
    let directory = tempfile::tempdir().expect("isolated enum binary directory");
    let binary = directory.path().join("nested_handle_enum_binary.exe");
    build_host_executable(&fixture, launcher(), &binary)
        .expect("compile enum equality with a nested handler");
    let actual = run_bounded(&binary, directory.path());
    assert!(actual.status.success(), "{actual:?}");
    assert_eq!(actual.stdout, expected.stdout.as_bytes());
    assert!(actual.stderr.is_empty(), "{actual:?}");
}

#[test]
fn native_refinement_local_source_reuse_matches_interpreter() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/native/refinement_local_source_reuse.jett");
    let expected = jett_driver::run_file_capture_output(&fixture).expect("interpreter oracle");
    let directory = tempfile::tempdir().expect("isolated refinement source directory");
    let binary = directory.path().join("refinement_local_source_reuse.exe");
    build_host_executable(&fixture, launcher(), &binary)
        .expect("compile refinement from reusable local");
    let actual = run_bounded(&binary, directory.path());
    assert!(actual.status.success(), "{actual:?}");
    assert_eq!(actual.stdout, expected.stdout.as_bytes());
    assert_eq!(
        actual.stderr,
        format!("{}\n", expected.debug_output.join("\n")).as_bytes()
    );
}

#[test]
fn native_nested_refinement_handle_matches_interpreter() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/native/nested_refinement_handle.jett");
    let expected = jett_driver::run_file_capture_output(&fixture).expect("interpreter oracle");
    let directory = tempfile::tempdir().expect("isolated refinement handler directory");
    let binary = directory.path().join("nested_refinement_handle.exe");
    build_host_executable(&fixture, launcher(), &binary)
        .expect("compile refinement candidate with a nested handler");
    let actual = run_bounded(&binary, directory.path());
    assert!(actual.status.success(), "{actual:?}");
    assert_eq!(actual.stdout, expected.stdout.as_bytes());
    assert!(actual.stderr.is_empty(), "{actual:?}");
}

#[test]
fn native_refined_binary_nested_handle_matches_interpreter() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/native/nested_handle_refined_binary.jett");
    let expected = jett_driver::run_file_capture_output(&fixture).expect("interpreter oracle");
    let directory = tempfile::tempdir().expect("isolated refined binary directory");
    let binary = directory.path().join("nested_handle_refined_binary.exe");
    build_host_executable(&fixture, launcher(), &binary)
        .expect("compile refined equality with a nested handler");
    let actual = run_bounded(&binary, directory.path());
    assert!(actual.status.success(), "{actual:?}");
    assert_eq!(actual.stdout, expected.stdout.as_bytes());
    assert!(actual.stderr.is_empty(), "{actual:?}");
}

#[test]
fn native_comptime_composites_match_interpreter() {
    let fixture =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/native/comptime_composites.jett");
    let expected = jett_driver::run_file_capture_output(&fixture).expect("interpreter oracle");
    let directory = tempfile::tempdir().expect("isolated comptime composite directory");
    let binary = directory.path().join("comptime_composites.exe");
    build_host_executable(&fixture, launcher(), &binary)
        .expect("compile a baked composite without running its source expression");
    let actual = run_bounded(&binary, directory.path());
    assert!(actual.status.success(), "{actual:?}");
    assert_eq!(actual.stdout, expected.stdout.as_bytes());
    assert!(actual.stderr.is_empty(), "{actual:?}");
}

#[test]
fn native_comptime_function_values_match_interpreter() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/native/comptime_function_values.jett");
    let expected = jett_driver::run_file_capture_output(&fixture).expect("interpreter oracle");
    let directory = tempfile::tempdir().expect("isolated comptime function directory");
    let binary = directory.path().join("comptime_function_values.exe");
    build_host_executable(&fixture, launcher(), &binary)
        .expect("compile a baked function value without resolving its source expression");
    let actual = run_bounded(&binary, directory.path());
    assert!(actual.status.success(), "{actual:?}");
    assert_eq!(actual.stdout, expected.stdout.as_bytes());
    assert!(actual.stderr.is_empty(), "{actual:?}");
}

#[test]
fn native_comptime_captured_functions_match_interpreter() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/native/comptime_captured_functions.jett");
    let expected = jett_driver::run_file_capture_output(&fixture).expect("interpreter oracle");
    let directory = tempfile::tempdir().expect("isolated comptime closure directory");
    let binary = directory.path().join("comptime_captured_functions.exe");
    build_host_executable(&fixture, launcher(), &binary)
        .expect("compile a baked closure with an evaluated capture environment");
    let actual = run_bounded(&binary, directory.path());
    assert!(actual.status.success(), "{actual:?}");
    assert_eq!(actual.stdout, expected.stdout.as_bytes());
    assert!(actual.stderr.is_empty(), "{actual:?}");
}

#[test]
fn native_comptime_function_values_execute_in_verify_suite() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/native/comptime_captured_functions.jett");
    let directory = tempfile::tempdir().expect("isolated comptime verify directory");
    let binary = directory.path().join("comptime_captured_verify.exe");
    let artifact = build_host_verify_suite_executable(&fixture, launcher(), &binary)
        .expect("compile baked function values inside a verify body");
    let actual = run_bounded(&artifact.path, directory.path());
    assert!(actual.status.success(), "{actual:?}");
    assert!(actual.stdout.is_empty(), "{actual:?}");
    assert!(actual.stderr.is_empty(), "{actual:?}");
}

#[test]
fn native_view_function_values_match_interpreter() {
    let fixture =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/native/view_function_values.jett");
    let expected = jett_driver::run_file_capture_output(&fixture).expect("interpreter oracle");
    let directory = tempfile::tempdir().expect("isolated view callback directory");
    let binary = directory.path().join("view_function_values.exe");
    build_host_executable(&fixture, launcher(), &binary)
        .expect("compile an indirect call through a view-parameter function value");
    let actual = run_bounded(&binary, directory.path());
    assert!(actual.status.success(), "{actual:?}");
    assert_eq!(actual.stdout, expected.stdout.as_bytes());
    assert!(actual.stderr.is_empty(), "{actual:?}");
}

#[test]
fn native_source_function_named_like_builtin_prefix_matches_interpreter() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/native/source_function_builtin_prefix_name.jett");
    let expected = jett_driver::run_file_capture_output(&fixture).expect("interpreter oracle");
    let directory = tempfile::tempdir().expect("isolated source function directory");
    let binary = directory
        .path()
        .join("source_function_builtin_prefix_name.exe");
    build_host_executable(&fixture, launcher(), &binary)
        .expect("compile source function named like a builtin prefix");
    let actual = run_bounded(&binary, directory.path());
    assert!(actual.status.success(), "{actual:?}");
    assert_eq!(actual.stdout, expected.stdout.as_bytes());
    assert!(actual.stderr.is_empty(), "{actual:?}");
}

#[test]
fn native_function_expression_calls_match_interpreter() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/native/function_expression_calls.jett");
    let expected = jett_driver::run_file_capture_output(&fixture).expect("interpreter oracle");
    let directory = tempfile::tempdir().expect("isolated expression callback directory");
    let binary = directory.path().join("function_expression_calls.exe");
    build_host_executable(&fixture, launcher(), &binary)
        .expect("compile calls through function expressions");
    let actual = run_bounded(&binary, directory.path());
    assert!(actual.status.success(), "{actual:?}");
    assert_eq!(actual.stdout, expected.stdout.as_bytes());
    assert!(actual.stderr.is_empty(), "{actual:?}");
}

#[test]
fn native_function_expression_callee_failure_cleans_owned_arguments() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/native/function_expression_callee_failure.jett");
    let expected = jett_driver::run_file_capture_outcome(&fixture)
        .expect_err("callee selection must fail after evaluating owned arguments");
    let directory = tempfile::tempdir().expect("isolated callee failure directory");
    let binary = directory
        .path()
        .join("function_expression_callee_failure.exe");
    build_host_executable(&fixture, launcher(), &binary).expect("compile failing callee");
    let actual = run_bounded(&binary, directory.path());
    assert_eq!(actual.status.code(), Some(71), "{actual:?}");
    assert_eq!(actual.stdout, expected.output.stdout.as_bytes());
    assert_eq!(actual.stderr, format!("{}\n", expected.message).as_bytes());
}

#[test]
fn native_named_enum_arguments_preserve_source_order() {
    let fixture =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/native/named_enum_arguments.jett");
    let expected = jett_driver::run_file_capture_output(&fixture).expect("interpreter oracle");
    assert_eq!(
        expected.stdout,
        "amount\nlabel\nnumbers\nlabel 2 7\nfirst\nhandler\nafter\nafter 2 9\nallocated\nreturned 1 11\npiped\npiped amount\npiped numbers\npiped 2 12\nbaked 3 8\nmixed amount\nmixed label\nmixed numbers\nmixed label 2 13\nmixed piped\nmixed piped amount\nmixed piped numbers\nmixed piped 2 14\nmixed baked 1 15\nmixed function 2 16\nmixed function piped 1 17\n"
    );
    let directory = tempfile::tempdir().expect("isolated named enum directory");
    let binary = directory.path().join("named_enum_arguments.exe");
    build_host_executable(&fixture, launcher(), &binary).expect("compile named enum payloads");
    let actual = run_bounded(&binary, directory.path());
    assert!(actual.status.success(), "{actual:?}");
    assert_eq!(actual.stdout, expected.stdout.as_bytes());
    assert!(actual.stderr.is_empty(), "{actual:?}");
}

#[test]
fn native_constructor_nested_handles_match_interpreter() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/native/nested_handle_constructors.jett");
    let expected = jett_driver::run_file_capture_output(&fixture).expect("interpreter oracle");
    let directory = tempfile::tempdir().expect("isolated constructor handle directory");
    let binary = directory.path().join("nested_handle_constructors.exe");
    build_host_executable(&fixture, launcher(), &binary)
        .expect("compile constructors with nested handlers");
    let actual = run_bounded(&binary, directory.path());
    assert!(actual.status.success(), "{actual:?}");
    assert_eq!(actual.stdout, expected.stdout.as_bytes());
    let debug = format!("{}\n", expected.debug_output.join("\n"));
    assert_eq!(String::from_utf8_lossy(&actual.stderr), debug);
}

#[test]
fn native_indirect_call_nested_handle_matches_interpreter() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/native/nested_handle_indirect_call.jett");
    let expected = jett_driver::run_file_capture_output(&fixture).expect("interpreter oracle");
    let directory = tempfile::tempdir().expect("isolated indirect handle directory");
    let binary = directory.path().join("nested_handle_indirect_call.exe");
    build_host_executable(&fixture, launcher(), &binary)
        .expect("compile indirect call with a nested handler");
    let actual = run_bounded(&binary, directory.path());
    assert!(actual.status.success(), "{actual:?}");
    assert_eq!(actual.stdout, expected.stdout.as_bytes());
    assert!(actual.stderr.is_empty(), "{actual:?}");
}

#[test]
fn native_indirect_view_argument_before_handler_matches_interpreter() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/native/nested_handle_indirect_view.jett");
    let expected = jett_driver::run_file_capture_output(&fixture).expect("interpreter oracle");
    let directory = tempfile::tempdir().expect("isolated indirect view directory");
    let binary = directory.path().join("nested_handle_indirect_view.exe");
    build_host_executable(&fixture, launcher(), &binary)
        .expect("compile indirect view argument before a handler");
    let actual = run_bounded(&binary, directory.path());
    assert!(actual.status.success(), "{actual:?}");
    assert_eq!(actual.stdout, expected.stdout.as_bytes());
    assert!(actual.stderr.is_empty(), "{actual:?}");
}

#[test]
fn native_intrinsic_nested_handle_matches_interpreter() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/native/nested_handle_intrinsic.jett");
    let expected = jett_driver::run_file_capture_output(&fixture).expect("interpreter oracle");
    let directory = tempfile::tempdir().expect("isolated intrinsic handle directory");
    let binary = directory.path().join("nested_handle_intrinsic.exe");
    build_host_executable(&fixture, launcher(), &binary)
        .expect("compile intrinsic with a nested handler");
    let actual = run_bounded(&binary, directory.path());
    assert!(actual.status.success(), "{actual:?}");
    assert_eq!(actual.stdout, expected.stdout.as_bytes());
    assert!(actual.stderr.is_empty(), "{actual:?}");
}

#[test]
fn native_indirect_aggregate_view_before_handler_matches_interpreter() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/native/nested_handle_indirect_aggregate_view.jett");
    let expected = jett_driver::run_file_capture_output(&fixture).expect("interpreter oracle");
    let directory = tempfile::tempdir().expect("isolated aggregate view directory");
    let binary = directory
        .path()
        .join("nested_handle_indirect_aggregate_view.exe");
    build_host_executable(&fixture, launcher(), &binary)
        .expect("compile indirect aggregate view before a handler");
    let actual = run_bounded(&binary, directory.path());
    assert!(actual.status.success(), "{actual:?}");
    assert_eq!(actual.stdout, expected.stdout.as_bytes());
    assert!(actual.stderr.is_empty(), "{actual:?}");
}

#[test]
fn native_borrowed_stdlib_call_before_handler_matches_interpreter() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/native/nested_handle_borrowed_stdlib_call.jett");
    let expected = jett_driver::run_file_capture_output(&fixture).expect("interpreter oracle");
    let directory = tempfile::tempdir().expect("isolated borrowed stdlib directory");
    let binary = directory
        .path()
        .join("nested_handle_borrowed_stdlib_call.exe");
    build_host_executable(&fixture, launcher(), &binary)
        .expect("compile borrowed stdlib call before a handler");
    let actual = run_bounded(&binary, directory.path());
    assert!(actual.status.success(), "{actual:?}");
    assert_eq!(actual.stdout, expected.stdout.as_bytes());
    assert!(actual.stderr.is_empty(), "{actual:?}");
}

#[test]
fn native_capability_view_before_handler_matches_interpreter() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/native/nested_handle_capability_view.jett");
    let expected = jett_driver::run_file_capture_output(&fixture).expect("interpreter oracle");
    let directory = tempfile::tempdir().expect("isolated capability view directory");
    let binary = directory.path().join("nested_handle_capability_view.exe");
    build_host_executable(&fixture, launcher(), &binary)
        .expect("compile capability view before a handler");
    let actual = run_bounded(&binary, directory.path());
    assert!(actual.status.success(), "{actual:?}");
    assert_eq!(actual.stdout, expected.stdout.as_bytes());
    assert!(actual.stderr.is_empty(), "{actual:?}");
}
