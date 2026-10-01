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

#[path = "support/native_inventory_execution.rs"]
mod inventory_execution;

#[path = "native_conformance/local_view_aliases.rs"]
mod local_view_aliases;

#[path = "native_conformance/pending_graphics.rs"]
mod pending_graphics;

#[path = "native_conformance/graphics_authority.rs"]
mod graphics_authority;

#[path = "native_conformance/display_results.rs"]
mod display_results;

#[path = "native_conformance/equality_results.rs"]
mod equality_results;

#[path = "native_conformance/secret_struct_constructors.rs"]
mod secret_struct_constructors;

#[path = "native_conformance/secret_fields.rs"]
mod secret_fields;

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
fn native_comptime_reflection_callbacks_match_interpreter() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/native/comptime_reflection_callbacks.jett");
    let expected =
        jett_driver::run_file_capture_output(&fixture).expect("reflection callback oracle");
    assert_eq!(
        expected.stdout,
        "list:int64\nalias:int64\ntext:int64\ninteger:int64\nint64:int64\nstring:int64\nlist[int64]:int64\n"
    );
    let directory = tempfile::tempdir().unwrap();
    let binary = directory.path().join("comptime_reflection_callbacks.exe");
    build_host_executable(&fixture, launcher(), &binary).expect("native reflection callbacks");
    let actual = run_bounded(&binary, directory.path());
    assert!(actual.status.success(), "{actual:?}");
    assert_eq!(actual.stdout, expected.stdout.as_bytes());
    assert!(actual.stderr.is_empty(), "{actual:?}");
}

#[test]
fn native_comptime_reflected_callbacks_match_interpreter() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/native/comptime_reflected_callbacks.jett");
    let expected =
        jett_driver::run_file_capture_output(&fixture).expect("reflected loop callback oracle");
    assert_eq!(
        expected.stdout,
        concat!(
            "plain\nnumber:int64\nplain\ntext:string\nplain\nrepeated:int64\n",
            "plain\nnumber:int64\nplain\ntext:string\nplain\nrepeated:int64\n",
            "int64:bool\nstring:bool\nint64:bool\n",
        )
    );
    let directory = tempfile::tempdir().unwrap();
    let binary = directory.path().join("comptime_reflected_callbacks.exe");
    build_host_executable(&fixture, launcher(), &binary).expect("native reflected callbacks");
    let actual = run_bounded(&binary, directory.path());
    assert!(actual.status.success(), "{actual:?}");
    assert_eq!(actual.stdout, expected.stdout.as_bytes());
    assert!(actual.stderr.is_empty(), "{actual:?}");
}

#[test]
fn native_reflected_integer_wrapping_matches_interpreter() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/native/reflected_integer_wrapping.jett");
    let expected =
        jett_driver::run_file_capture_output(&fixture).expect("scoped integer wrapping oracle");
    assert_eq!(
        expected.stdout,
        concat!(
            "-128,128,-128\n-128,128,-128\n-128,128,-128\n-128,128,-128\n",
            "0,0\n0,0\n-128\n128\n-128\n-128\n128\n-128\n0\n0\n0\n0\n",
        )
    );
    let directory = tempfile::tempdir().unwrap();
    let binary = directory.path().join("reflected_integer_wrapping.exe");
    build_host_executable(&fixture, launcher(), &binary).expect("native scoped integer wrapping");
    let actual = run_bounded(&binary, directory.path());
    assert!(actual.status.success(), "{actual:?}");
    assert_eq!(actual.stdout, expected.stdout.as_bytes());
    assert!(actual.stderr.is_empty(), "{actual:?}");
}

#[test]
fn native_reflected_alias_callbacks_match_interpreter() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/native/reflected_alias_callbacks.jett");
    let expected =
        jett_driver::run_file_capture_output(&fixture).expect("reflected alias callback oracle");
    assert_eq!(
        expected.stdout,
        concat!(
            "alias:Label\nprimitive:string\nalias:Other\nalias:Label\nlist:list[Label]\nlist:list[string]\n",
            "alias:Label\nprimitive:string\nalias:Other\nalias:Label\nlist:list[Label]\nlist:list[string]\n",
            "alias:Label\nalias:Label\nalias:Label,primitive:string\nalias:Label,primitive:string\n",
            "primitive:string\nprimitive:string\n",
            "primitive:string,primitive:string,primitive:string,alias:Label,primitive:string\n",
            "primitive:string,primitive:string,primitive:string,alias:Label,primitive:string\n",
            "-128\n-128\n",
        )
    );
    let directory = tempfile::tempdir().unwrap();
    let binary = directory.path().join("reflected_alias_callbacks.exe");
    build_host_executable(&fixture, launcher(), &binary).expect("native reflected alias callbacks");
    let actual = run_bounded(&binary, directory.path());
    assert!(actual.status.success(), "{actual:?}");
    assert_eq!(actual.stdout, expected.stdout.as_bytes());
    assert!(actual.stderr.is_empty(), "{actual:?}");
}

#[test]
fn native_closure_reflection_guards_match_interpreter() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/native/closure_reflection_guards.jett");
    let expected =
        jett_driver::run_file_capture_output(&fixture).expect("closure reflection guard oracle");
    assert_eq!(
        expected.stdout,
        "0\n2\n0\n3\ntype:int64:0\ntype:list[int64]:2\n2\n3\n4\n0\n0\n0\n2\n0\n"
    );
    let directory = tempfile::tempdir().unwrap();
    let binary = directory.path().join("closure_reflection_guards.exe");
    build_host_executable(&fixture, launcher(), &binary).expect("native closure reflection guards");
    let actual = run_bounded(&binary, directory.path());
    assert!(actual.status.success(), "{actual:?}");
    assert_eq!(actual.stdout, expected.stdout.as_bytes());
    assert!(actual.stderr.is_empty(), "{actual:?}");
}

#[test]
fn native_generic_reflection_expressions_match_interpreter() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/native/generic_reflection_expressions.jett");
    let expected =
        jett_driver::run_file_capture_output(&fixture).expect("generic reflection oracle");
    assert_eq!(
        expected.stdout,
        "type:int64\n<string>\nlive:int64\nbaked:string\n"
    );
    let directory = tempfile::tempdir().unwrap();
    let binary = directory.path().join("generic_reflection_expressions.exe");
    build_host_executable(&fixture, launcher(), &binary).expect("native generic reflection");
    let actual = run_bounded(&binary, directory.path());
    assert!(actual.status.success(), "{actual:?}");
    assert_eq!(actual.stdout, expected.stdout.as_bytes());
    assert!(actual.stderr.is_empty(), "{actual:?}");
}

#[test]
fn native_generic_integer_wrapping_matches_interpreter() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/native/generic_integer_wrapping.jett");
    let expected = jett_driver::run_file_capture_output(&fixture).expect("generic wrapping oracle");
    assert_eq!(
        expected.stdout,
        "-128\n128\n0\n0\n-128\n128\n-128\n128\n-128\n128\n"
    );
    let directory = tempfile::tempdir().unwrap();
    let binary = directory.path().join("generic_integer_wrapping.exe");
    build_host_executable(&fixture, launcher(), &binary).expect("native generic wrapping");
    let actual = run_bounded(&binary, directory.path());
    assert!(actual.status.success(), "{actual:?}");
    assert_eq!(actual.stdout, expected.stdout.as_bytes());
    assert!(actual.stderr.is_empty(), "{actual:?}");
}

#[test]
fn native_primitive_list_sums_match_interpreter_in_both_profiles() {
    let fixture =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/native/list_sum_primitives.jett");
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("list_sum.jett");
    fs::copy(&fixture, &source).unwrap();
    let expected =
        jett_driver::run_file_capture_output(&source).expect("primitive list sum oracle");
    let nonempty = "int8:-128;int16:-32768;int32:-2147483648;int64:-9223372036854775808;uint8:1;uint16:1;uint32:1;uint64:1;float32:0;float64:1;\n";
    let empty =
        "int8:0;int16:0;int32:0;int64:0;uint8:0;uint16:0;uint32:0;uint64:0;float32:0;float64:0;\n";
    assert_eq!(
        expected.stdout,
        format!("{nonempty}{nonempty}{empty}{empty}")
    );
    assert!(expected.debug_output.is_empty());
    let mut binaries = Vec::new();
    for release in [false, true] {
        let binary = directory.path().join(format!("list_sum_{release}.exe"));
        jett_driver::native::build_host_executable_with_options(
            &source,
            launcher(),
            &binary,
            jett_driver::BuildOptions { release },
        )
        .expect("native primitive list sum program");
        binaries.push(binary);
    }
    let verify_binary = directory.path().join("list_sum_verify.exe");
    build_host_verify_suite_executable(&source, launcher(), &verify_binary)
        .expect("native primitive list sum verify suite");
    let property_binary = directory.path().join("list_sum_property.exe");
    build_host_property_suite_executable(&source, launcher(), &property_binary)
        .expect("native primitive list sum property suite");
    fs::remove_file(&source).unwrap();
    for binary in binaries {
        let actual = run_bounded(&binary, directory.path());
        assert!(actual.status.success(), "{actual:?}");
        assert_eq!(actual.stdout, expected.stdout.as_bytes());
        assert!(actual.stderr.is_empty(), "{actual:?}");
    }
    for suite in [verify_binary, property_binary] {
        let actual = run_bounded(&suite, directory.path());
        assert!(actual.status.success(), "{actual:?}");
        assert!(actual.stdout.is_empty(), "{actual:?}");
        assert!(actual.stderr.is_empty(), "{actual:?}");
    }
}

#[test]
fn native_primitive_list_sums_reject_pending_in_both_profiles() {
    for (ty, first, second) in [
        ("int8", "2", "3"),
        ("uint64", "2", "3"),
        ("float32", "2.0", "3.0"),
        ("float64", "2.0", "3.0"),
    ] {
        for (name, initializer, message) in [
            (
                "first",
                format!("list(run run {first}, {second})"),
                "list.__sum: list elements must be int64 or float64",
            ),
            (
                "later",
                format!("list({first}, run run {second})"),
                "list.__sum: mixed types",
            ),
            (
                "outer",
                format!("run run list({first}, {second})"),
                "list.__sum: argument must be a list",
            ),
        ] {
            let directory = tempfile::tempdir().unwrap();
            let source = directory.path().join("pending_list_sum.jett");
            fs::write(
                &source,
                format!(
                    "namespace app\nfunction main(stdout: Stdout) returns nothing:\n    list[{ty}] values = {initializer}\n    Stdout.write(view stdout, \"before\\n\")\n    {ty} total = list.sum[{ty}](view values)\n    trace total\n    Stdout.write(view stdout, \"after\\n\")\n"
                ),
            )
            .unwrap();
            let expected = jett_driver::run_file_capture_outcome(&source)
                .expect_err("pending primitive list sum oracle");
            assert_eq!(expected.output.stdout, "before\n", "{ty}/{name}");
            assert!(expected.output.debug_output.is_empty(), "{ty}/{name}");
            assert_eq!(
                expected.message,
                format!("runtime error: {message}"),
                "{ty}/{name}"
            );
            let mut binaries = Vec::new();
            for release in [false, true] {
                let binary = directory.path().join(format!("pending_sum_{release}.exe"));
                jett_driver::native::build_host_executable_with_options(
                    &source,
                    launcher(),
                    &binary,
                    jett_driver::BuildOptions { release },
                )
                .expect("native pending primitive list sum program");
                binaries.push(binary);
            }
            fs::remove_file(&source).unwrap();
            for binary in binaries {
                let actual = run_bounded(&binary, directory.path());
                assert_eq!(actual.status.code(), Some(71), "{ty}/{name}: {actual:?}");
                assert_eq!(
                    actual.stdout,
                    expected.output.stdout.as_bytes(),
                    "{ty}/{name}"
                );
                assert_eq!(
                    actual.stderr,
                    format!("{}\n", expected.message).as_bytes(),
                    "{ty}/{name}: {actual:?}"
                );
            }
        }
    }
}

#[test]
fn native_list_sort_refinements_and_ordering_match_interpreter_in_both_profiles() {
    let refined_stdout = concat!(
        "narrow:-128,-128,0,127,|narrow:-128\n",
        "unsigned:2,9223372036854775808,18446744073709551615,18446744073709551615,\n",
        "decimal:-2.25,0,16777216,16777216,\n",
        "integer:-9223372036854775808,-9223372036854775808,2,9223372036854775807,\n",
        "word:apple,apple,eclair,zebra,|source:4\n",
        "flag:false,false,true,true,\n",
        "nested:-3,-3,2,|nested:-3\n",
        "alias:-2,3,3,\n",
        "empty:0:0;single:5:solo\n",
        "refined:false:true\n",
        "baked:narrow:-128:127:apple:zebra:false\n",
        "pending:zebra:apple\nready:apple:zebra\n",
        "plain-pending:zebra:apple\nplain-ready:apple:zebra;sorted:true:false:true\n",
    );
    let ordering_stdout = concat!(
        "int8:false:2,1,\nuint8:false:2,1,\nfloat32:false:2,1,\nuint64:false:2,1,\n",
        "f32:nan:false:false:true:true;infinity:true:false;sort:true:true\n",
        "f64:nan:false:false:true:true;infinity:true:false;sort:true:true\n",
        "index-string:first:second:zebra\nindex-scalar:11:22:2\nindex-row:first:second:zebra\n",
    );
    let refined_debug = [
        "trace head: app.Word = pending(pending(zebra))",
        "trace once: app.Word = pending(zebra)",
        "trace joined: app.Word = zebra",
        "trace head: string = pending(zebra)",
        "trace joined: string = zebra",
    ];
    let ordering_debug = [
        "trace ordered: list[list[string]] = list(list(pending(pending(zebra)), first), list(apple, second))",
        "trace ordered: list[list[int64]] = list(list(pending(2), 11), list(1, 22))",
        "trace ordered: list[list[string]] = list(pending(pending(list(zebra, first))), list(apple, second))",
    ];
    for (name, stdout, debug) in [
        (
            "list_sort_refinements",
            refined_stdout,
            refined_debug.as_slice(),
        ),
        (
            "list_ordering_controls",
            ordering_stdout,
            ordering_debug.as_slice(),
        ),
    ] {
        let fixture =
            Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("../../tests/native/{name}.jett"));
        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join("main.jett");
        fs::copy(&fixture, &source).unwrap();
        let expected = jett_driver::run_file_capture_output(&source).expect("list ordering oracle");
        assert_eq!(expected.stdout, stdout, "{name}");
        assert_eq!(expected.debug_output, debug, "{name}");
        let mut binaries = Vec::new();
        for release in [false, true] {
            let binary = directory.path().join(format!("ordering_{release}.exe"));
            jett_driver::native::build_host_executable_with_options(
                &source,
                launcher(),
                &binary,
                jett_driver::BuildOptions { release },
            )
            .unwrap_or_else(|error| panic!("{name}, release={release}: {error}"));
            binaries.push((binary, release));
        }
        let verify_binary = directory.path().join("ordering_verify.exe");
        build_host_verify_suite_executable(&source, launcher(), &verify_binary)
            .expect("native list ordering verify suite");
        let property_binary = directory.path().join("ordering_property.exe");
        build_host_property_suite_executable(&source, launcher(), &property_binary)
            .expect("native list ordering property suite");
        fs::remove_file(&source).unwrap();
        for (binary, release) in binaries {
            let actual = run_bounded(&binary, directory.path());
            assert!(
                actual.status.success(),
                "{name}, release={release}: {actual:?}"
            );
            assert_eq!(actual.stdout, expected.stdout.as_bytes(), "{name}");
            let debug = if release {
                String::new()
            } else {
                format!("{}\n", expected.debug_output.join("\n"))
            };
            assert_eq!(actual.stderr, debug.as_bytes(), "{name}: {actual:?}");
        }
        for suite in [verify_binary, property_binary] {
            let actual = run_bounded(&suite, directory.path());
            assert!(actual.status.success(), "{name}: {actual:?}");
            assert!(actual.stdout.is_empty(), "{name}: {actual:?}");
            assert!(actual.stderr.is_empty(), "{name}: {actual:?}");
        }
    }
}

#[test]
fn native_pending_refined_list_sort_rejects_outer_pending_in_both_profiles() {
    let fixture =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/native/list_sort_refinements.jett");
    let template = fs::read_to_string(fixture).unwrap();
    let (declarations, _) = template.rsplit_once("function main(").unwrap();
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("pending_list_sort.jett");
    fs::write(
        &source,
        format!(
            "{declarations}function main(stdout: Stdout) returns nothing:\n    pending_outer_failure(view stdout)\n"
        ),
    )
    .unwrap();
    let expected = jett_driver::run_file_capture_outcome(&source).unwrap_err();
    assert_eq!(expected.output.stdout, "before\n");
    assert!(expected.output.debug_output.is_empty());
    assert_eq!(
        expected.message,
        "runtime error: list.__sort expects a list argument"
    );
    let mut binaries = Vec::new();
    for release in [false, true] {
        let binary = directory.path().join(format!("pending_sort_{release}.exe"));
        jett_driver::native::build_host_executable_with_options(
            &source,
            launcher(),
            &binary,
            jett_driver::BuildOptions { release },
        )
        .expect("native pending refined list sort");
        binaries.push(binary);
    }
    fs::remove_file(&source).unwrap();
    for binary in binaries {
        let actual = run_bounded(&binary, directory.path());
        assert_eq!(actual.status.code(), Some(71), "{actual:?}");
        assert_eq!(actual.stdout, expected.output.stdout.as_bytes());
        assert_eq!(
            actual.stderr,
            format!("{}\n", expected.message).as_bytes(),
            "{actual:?}"
        );
    }
}

#[test]
fn native_math_aggregate_primitives_match_interpreter_in_both_profiles() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/native/math_aggregate_primitives.jett");
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("math_aggregate_primitives.jett");
    fs::copy(&fixture, &source).unwrap();
    let expected = jett_driver::run_file_capture_output(&source).expect("primitive math oracle");
    let reports = [
        "int8:-0.5:-0.5",
        "int16:-0.5:-0.5",
        "int32:-0.5:-0.5",
        "int64:0:0",
        "uint8:127.5:127.5",
        "uint16:32767.5:32767.5",
        "uint32:2147483647.5:2147483647.5",
        "uint64:13835058055282164000:13835058055282164000",
        "float32:5592405.333333333:16777216",
        "float64:5592405.666666667:16777216",
    ];
    let mut stdout = String::new();
    for context in ["runtime", "comptime"] {
        for report in reports {
            stdout.push_str(&format!("{context}:{report}\n"));
        }
    }
    stdout.push_str(concat!(
        "ieee32:nan:true:true;infinity:inf:inf;opposite:true:true\n",
        "ieee64:nan:true:true;infinity:inf:inf;opposite:true:true\n",
        "maximum:true:true;residual:true\n",
        "baked:ieee32:nan:true:true;infinity:inf:inf;opposite:true:true\n",
    ));
    assert_eq!(expected.stdout, stdout);
    assert!(expected.debug_output.is_empty());
    let mut binaries = Vec::new();
    for release in [false, true] {
        let binary = directory
            .path()
            .join(format!("math_aggregate_{release}.exe"));
        jett_driver::native::build_host_executable_with_options(
            &source,
            launcher(),
            &binary,
            jett_driver::BuildOptions { release },
        )
        .expect("native primitive math aggregate program");
        binaries.push(binary);
    }
    let verify_binary = directory.path().join("math_aggregate_verify.exe");
    build_host_verify_suite_executable(&source, launcher(), &verify_binary)
        .expect("native primitive math aggregate verify suite");
    let property_binary = directory.path().join("math_aggregate_property.exe");
    build_host_property_suite_executable(&source, launcher(), &property_binary)
        .expect("native primitive math aggregate property suite");
    fs::remove_file(&source).unwrap();
    for binary in binaries {
        let actual = run_bounded(&binary, directory.path());
        assert!(actual.status.success(), "{actual:?}");
        assert_eq!(actual.stdout, expected.stdout.as_bytes());
        assert!(actual.stderr.is_empty(), "{actual:?}");
    }
    for suite in [verify_binary, property_binary] {
        let actual = run_bounded(&suite, directory.path());
        assert!(actual.status.success(), "{actual:?}");
        assert!(actual.stdout.is_empty(), "{actual:?}");
        assert!(actual.stderr.is_empty(), "{actual:?}");
    }
}

#[test]
fn native_math_aggregate_primitives_reject_empty_and_pending_in_both_profiles() {
    let mut cases = Vec::new();
    for operation in ["average", "median"] {
        for ty in [
            "int8", "int16", "int32", "int64", "uint8", "uint16", "uint32", "uint64", "float32",
            "float64",
        ] {
            cases.push((
                format!("{operation}_{ty}_empty"),
                operation,
                ty,
                "list()".to_owned(),
                format!("math.{operation}: list is empty"),
            ));
        }
    }
    for (name, operation, ty, initializer, message) in [
        (
            "average_first",
            "average",
            "int8",
            "list(run run 2, 3)",
            "math.average expects a list of numeric values",
        ),
        (
            "average_later",
            "average",
            "float32",
            "list(2.0, run run 3.0)",
            "math.average expects a list of numeric values",
        ),
        (
            "average_outer",
            "average",
            "uint64",
            "run run list(2, 3)",
            "math.__average expects a list of numbers",
        ),
        (
            "median_first",
            "median",
            "uint64",
            "list(run run 2, 3)",
            "math.median expects a list of numeric values",
        ),
        (
            "median_later",
            "median",
            "int16",
            "list(2, run run 3)",
            "math.median expects a list of numeric values",
        ),
        (
            "median_outer",
            "median",
            "float32",
            "run run list(2.0, 3.0)",
            "math.__median expects a list of numbers",
        ),
    ] {
        cases.push((
            name.to_owned(),
            operation,
            ty,
            initializer.to_owned(),
            message.to_owned(),
        ));
    }
    for (name, operation, ty, initializer, message) in cases {
        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join("math_aggregate_failure.jett");
        fs::write(
            &source,
            format!(
                "namespace app\nfunction aggregate[T](values: list[T]) returns float64:\n    return math.{operation}[T](values)\nfunction main(stdout: Stdout) returns nothing:\n    list[{ty}] values = {initializer}\n    Stdout.write(view stdout, \"before\\n\")\n    float64 value = aggregate[{ty}](values)\n    trace value\n    Stdout.write(view stdout, \"after\\n\")\n"
            ),
        )
        .unwrap();
        let expected = jett_driver::run_file_capture_outcome(&source).unwrap_err();
        assert_eq!(expected.output.stdout, "before\n", "{name}");
        assert!(expected.output.debug_output.is_empty(), "{name}");
        assert_eq!(
            expected.message,
            format!("runtime error: {message}"),
            "{name}"
        );
        let mut binaries = Vec::new();
        for release in [false, true] {
            let binary = directory.path().join(format!("math_failure_{release}.exe"));
            jett_driver::native::build_host_executable_with_options(
                &source,
                launcher(),
                &binary,
                jett_driver::BuildOptions { release },
            )
            .unwrap_or_else(|error| panic!("{name}, release={release}: {error}"));
            binaries.push(binary);
        }
        fs::remove_file(&source).unwrap();
        for binary in binaries {
            let actual = run_bounded(&binary, directory.path());
            assert_eq!(actual.status.code(), Some(71), "{name}: {actual:?}");
            assert_eq!(actual.stdout, expected.output.stdout.as_bytes(), "{name}");
            assert_eq!(
                actual.stderr,
                format!("{}\n", expected.message).as_bytes(),
                "{name}: {actual:?}"
            );
        }
    }
}

#[test]
fn native_empty_collection_iteration_matches_interpreter_in_both_profiles() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/native/empty_collection_iteration.jett");
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("empty_collection_iteration.jett");
    fs::copy(&fixture, &source).unwrap();
    let expected = jett_driver::run_file_capture_output(&source).expect("empty iteration oracle");
    assert_eq!(
        expected.stdout,
        concat!(
            "direct:done\nsource\nargument\neffects:done\n",
            "generic:0:0:live:3\ndead:7:7\n",
            "nested:2:0:borrowed:2:0\n",
            "maps:0:context:0:0:0:0:0\nmapped:0:joined:0\n",
        )
    );
    assert!(expected.debug_output.is_empty());
    let mut binaries = Vec::new();
    for release in [false, true] {
        let binary = directory
            .path()
            .join(format!("empty_iteration_{release}.exe"));
        jett_driver::native::build_host_executable_with_options(
            &source,
            launcher(),
            &binary,
            jett_driver::BuildOptions { release },
        )
        .expect("native empty collection iteration program");
        binaries.push(binary);
    }
    let verify_binary = directory.path().join("empty_iteration_verify.exe");
    build_host_verify_suite_executable(&source, launcher(), &verify_binary)
        .expect("native empty collection iteration verify suite");
    let property_binary = directory.path().join("empty_iteration_property.exe");
    build_host_property_suite_executable(&source, launcher(), &property_binary)
        .expect("native empty collection iteration property suite");
    fs::remove_file(&source).unwrap();
    for binary in binaries {
        let actual = run_bounded(&binary, directory.path());
        assert!(actual.status.success(), "{actual:?}");
        assert_eq!(actual.stdout, expected.stdout.as_bytes());
        assert!(actual.stderr.is_empty(), "{actual:?}");
    }
    for suite in [verify_binary, property_binary] {
        let actual = run_bounded(&suite, directory.path());
        assert!(actual.status.success(), "{actual:?}");
        assert!(actual.stdout.is_empty(), "{actual:?}");
        assert!(actual.stderr.is_empty(), "{actual:?}");
    }
}

#[test]
fn native_empty_collection_iteration_rejects_pending_in_both_profiles() {
    for (collection, binding) in [("list()", "value"), ("map()", "key, value")] {
        for comptime in [false, true] {
            let directory = tempfile::tempdir().unwrap();
            let source = directory.path().join("pending_empty_iteration.jett");
            let initializer = if comptime {
                format!("comptime run run {collection}")
            } else {
                format!("run run {collection}")
            };
            fs::write(
                &source,
                format!(
                    "namespace app\nfunction main(stdout: Stdout) returns nothing:\n    Stdout.write(view stdout, \"before\\n\")\n    for {binding} in {initializer}:\n        trace value\n        Stdout.write(view stdout, \"unexpected body\\n\")\n    Stdout.write(view stdout, \"after\\n\")\n"
                ),
            )
            .unwrap();
            let expected = jett_driver::run_file_capture_outcome(&source).unwrap_err();
            assert_eq!(expected.output.stdout, "before\n");
            assert!(expected.output.debug_output.is_empty());
            assert_eq!(
                expected.message,
                "runtime error: for loop requires a list, string, map, or set value"
            );
            let mut binaries = Vec::new();
            for release in [false, true] {
                let binary = directory
                    .path()
                    .join(format!("pending_empty_{release}.exe"));
                jett_driver::native::build_host_executable_with_options(
                    &source,
                    launcher(),
                    &binary,
                    jett_driver::BuildOptions { release },
                )
                .unwrap_or_else(|error| {
                    panic!("{collection}, comptime={comptime}, release={release}: {error}")
                });
                binaries.push(binary);
            }
            fs::remove_file(&source).unwrap();
            for binary in binaries {
                let actual = run_bounded(&binary, directory.path());
                assert_eq!(actual.status.code(), Some(71), "{actual:?}");
                assert_eq!(actual.stdout, expected.output.stdout.as_bytes());
                assert_eq!(
                    actual.stderr,
                    format!("{}\n", expected.message).as_bytes(),
                    "{actual:?}"
                );
            }
        }
    }
}

#[test]
fn empty_collection_iteration_still_rejects_invalid_dead_bodies() {
    for (body, code, message) in [
        (
            "int64 copied = value + 1",
            301,
            "cannot apply `+` to `<never>` and `int64`",
        ),
        (
            "string rendered = \"{value}\"",
            332,
            "type `<never>` does not implement interface `Displayable`",
        ),
        (
            "missing_function(value)",
            200,
            "undefined name: `missing_function`",
        ),
    ] {
        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join("invalid_dead_body.jett");
        let text = format!(
            "namespace app\nfunction main() returns nothing:\n    for value in list():\n        {body}\n    return nothing\n"
        );
        fs::write(&source, &text).unwrap();
        for checked in [
            jett_driver::build_source(&text, "invalid_dead_body.jett"),
            jett_driver::build_file(&source),
        ] {
            let errors = checked
                .diagnostics
                .iter()
                .filter(|diagnostic| diagnostic.severity == jett_diagnostics::Severity::Error)
                .collect::<Vec<_>>();
            assert_eq!(errors.len(), 1, "{errors:?}");
            assert_eq!(errors[0].code.code(), code);
            assert_eq!(errors[0].message, message);
        }
    }
}

#[test]
fn native_uninhabited_sum_arms_match_interpreter_in_both_profiles() {
    let fixture =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/native/uninhabited_sum_arms.jett");
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("uninhabited_sum_arms.jett");
    fs::copy(&fixture, &source).unwrap();
    let expected = jett_driver::run_file_capture_output(&source).expect("uninhabited sum oracle");
    assert_eq!(
        expected.stdout,
        concat!(
            "handlers:0:0:0\nreuse:0:0:2:0:default:0:0\n",
            "transforms:0:0:0:0:0\noptional source\neffect optional:0\n",
            "result source\neffect failure:0\ncontext:0:0:0:callbacks:0:0:0\n",
            "baked:0:0:0:0:0:2:0:0:0:0:0:0\npayloads:1:1\n",
            "surviving:37:49\n",
            "mixed-sums:7,:7,:8,:8,\n",
            "mixed-nested:9,:9,:10,:10,\n",
            "mixed-callbacks:7,:7,\n",
            "mixed-rows:added:1:0;added:3:2;|added:3:2;added:1:0;\n",
            "mixed-row-callbacks:added:1:0;added:3:2;|added:3:2;added:1:0;\n",
            "boundary:local=7:0;view=7:0;call=7:0;nested=added:1:1:0|added:1:1:0;second=7,:|added:3:2;|added:1:0;|added:3:2;|added:1:0;\n",
            "reflection:outer=list[int64]:list[int64];literal=list[int64]:list[int64];payload=app.Count;root=int64;aliases=app.Count:app.OtherCount:app.Count:app.OtherCount\n",
        )
    );
    assert_eq!(
        expected.debug_output,
        [
            "trace error: string = bad",
            "trace item: int64 = 8",
            "trace error: string = reuse",
            "trace error: string = reuse",
            "trace item: int64 = 8",
            "trace item: int64 = 8",
            "trace item: int64 = 8",
            "trace item: int64 = 8",
            "trace error: string = default",
            "trace error: string = absent",
            "trace error: string = concrete",
            "trace item: app.Count = 8",
            "trace item: app.Named = 8",
            "trace item: function() returns int64 = function(app.callback)",
            "trace item: int64 = pending(pending(6))",
            "trace once: int64 = pending(6)",
            "trace ready: int64 = 6",
            "trace item: int64 = pending(pending(8))",
            "trace once: int64 = pending(8)",
            "trace ready: int64 = 8",
        ]
    );
    let mut binaries = Vec::new();
    for release in [false, true] {
        let binary = directory
            .path()
            .join(format!("uninhabited_sum_{release}.exe"));
        jett_driver::native::build_host_executable_with_options(
            &source,
            launcher(),
            &binary,
            jett_driver::BuildOptions { release },
        )
        .expect("native uninhabited sum program");
        binaries.push((binary, release));
    }
    let verify_binary = directory.path().join("uninhabited_sum_verify.exe");
    build_host_verify_suite_executable(&source, launcher(), &verify_binary)
        .expect("native uninhabited sum verify suite");
    let property_binary = directory.path().join("uninhabited_sum_property.exe");
    build_host_property_suite_executable(&source, launcher(), &property_binary)
        .expect("native uninhabited sum property suite");
    fs::remove_file(&source).unwrap();
    for (binary, release) in binaries {
        let actual = run_bounded(&binary, directory.path());
        assert!(actual.status.success(), "{actual:?}");
        assert_eq!(actual.stdout, expected.stdout.as_bytes());
        let debug = if release {
            String::new()
        } else {
            format!("{}\n", expected.debug_output.join("\n"))
        };
        assert_eq!(actual.stderr, debug.as_bytes(), "{actual:?}");
    }
    let suite_debug = "trace error: string = bad\ntrace item: int64 = 8\n";
    for (suite, repetitions) in [(verify_binary, 1), (property_binary, 100)] {
        let actual = run_bounded(&suite, directory.path());
        assert!(actual.status.success(), "{actual:?}");
        assert!(actual.stdout.is_empty(), "{actual:?}");
        assert_eq!(actual.stderr, suite_debug.repeat(repetitions).as_bytes());
    }
}

#[test]
fn native_uninhabited_sum_arms_and_reverse_reject_pending_in_both_profiles() {
    let fixture =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/native/uninhabited_sum_arms.jett");
    let template = fs::read_to_string(&fixture).unwrap();
    let declarations = template.split("function main(").next().unwrap();
    for (name, operation, value, message) in [
        (
            "optional",
            "unwrap_optional",
            "none",
            "handle block requires a result or optional value, got pending(pending(none))",
        ),
        (
            "failure",
            "unwrap_result",
            "fail(\"bad\")",
            "handle block requires a result or optional value, got pending(pending(fail(bad)))",
        ),
        (
            "success",
            "collect_errors",
            "ok(8)",
            "handle block requires a result or optional value, got pending(pending(ok(8)))",
        ),
        (
            "reverse",
            "list.reverse",
            "list()",
            "list.__length expects a list argument",
        ),
    ] {
        for comptime in [false, true] {
            let directory = tempfile::tempdir().unwrap();
            let source = directory.path().join("pending_uninhabited_sum.jett");
            let initializer = if comptime {
                format!("comptime run run {value}")
            } else {
                format!("run run {value}")
            };
            fs::write(
                &source,
                format!(
                    "{declarations}function main(stdout: Stdout) returns nothing:\n    Stdout.write(view stdout, \"before\\n\")\n    int64 size = list.length(view {operation}({initializer}))\n    Stdout.write(view stdout, \"after:{{size}}\\n\")\n"
                ),
            )
            .unwrap();
            let expected = jett_driver::run_file_capture_outcome(&source).unwrap_err();
            assert_eq!(expected.output.stdout, "before\n", "{name}");
            assert!(expected.output.debug_output.is_empty(), "{name}");
            assert_eq!(
                expected.message,
                format!("runtime error: {message}"),
                "{name}"
            );
            let mut binaries = Vec::new();
            for release in [false, true] {
                let binary = directory.path().join(format!("pending_sum_{release}.exe"));
                jett_driver::native::build_host_executable_with_options(
                    &source,
                    launcher(),
                    &binary,
                    jett_driver::BuildOptions { release },
                )
                .unwrap_or_else(|error| {
                    panic!("{name}, comptime={comptime}, release={release}: {error}")
                });
                binaries.push(binary);
            }
            fs::remove_file(&source).unwrap();
            for binary in binaries {
                let actual = run_bounded(&binary, directory.path());
                assert_eq!(actual.status.code(), Some(71), "{name}: {actual:?}");
                assert_eq!(actual.stdout, expected.output.stdout.as_bytes(), "{name}");
                assert_eq!(
                    actual.stderr,
                    format!("{}\n", expected.message).as_bytes(),
                    "{name}: {actual:?}"
                );
            }
        }
    }
}

#[test]
fn uninhabited_sum_handlers_still_reject_invalid_arms() {
    for (declaration, argument) in [
        (
            "function invalid[T](candidate: optional[T]) returns list[T]:\n    T item = candidate handle: return list()\n    int64 copied = item + 1\n    return list(item)\n",
            "none",
        ),
        (
            "function invalid[T, E](candidate: result[T, E]) returns list[E]:\n    T item = candidate handle error:\n        int64 copied = error + 1\n        return list(error)\n    return list()\n",
            "ok(8)",
        ),
    ] {
        let text = format!(
            "namespace app\n{declaration}function main(stdout: Stdout) returns nothing:\n    Stdout.write(view stdout, \"{{list.length(view invalid({argument}))}}\")\n"
        );
        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join("invalid_uninhabited_arm.jett");
        fs::write(&source, &text).unwrap();
        for checked in [
            jett_driver::build_source(&text, "invalid_uninhabited_arm.jett"),
            jett_driver::build_file(&source),
        ] {
            let errors = checked
                .diagnostics
                .iter()
                .filter(|diagnostic| diagnostic.severity == jett_diagnostics::Severity::Error)
                .collect::<Vec<_>>();
            assert_eq!(errors.len(), 1, "{errors:?}");
            assert_eq!(errors[0].code.code(), 301);
            assert_eq!(
                errors[0].message,
                "cannot apply `+` to `<never>` and `int64`"
            );
        }
    }
}

#[test]
fn native_builds_reject_manufactured_never_values_before_publication() {
    let default_forge = "function forge[T](values: list[T]) returns list[T]:\n    T item = none handle: default 7\n    return list(item)\n";
    for (name, declaration, namespace_value, invocation, code) in [
        (
            "default",
            default_forge,
            "",
            "int64 size = list.length(view forge(list()))",
            300,
        ),
        (
            "local",
            "function forge[T](values: list[T]) returns list[T]:\n    T item = 7\n    return list(item)\n",
            "",
            "int64 size = list.length(view forge(list()))",
            311,
        ),
        (
            "return",
            "function forge[T](values: list[T]) returns T:\n    return 7\nfunction invoke[T](values: list[T]) returns list[T]:\n    T item = forge[T](values)\n    return list(item)\n",
            "",
            "int64 size = list.length(view invoke(list()))",
            305,
        ),
        (
            "append",
            "function forge[T](values: list[T]) returns list[T]:\n    return list.append[T](values, 7)\n",
            "",
            "int64 size = list.length(view forge(list()))",
            300,
        ),
        (
            "list_return",
            "function forge[T](values: list[T]) returns list[T]:\n    return list(7)\n",
            "",
            "int64 size = list.length(view forge(list()))",
            300,
        ),
        (
            "namespace_comptime",
            default_forge,
            "int64 forged = comptime list.length(view forge(list()))\n",
            "trace forged",
            300,
        ),
    ] {
        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join("manufactured_never.jett");
        let function_name = format!("manufacture_{name}");
        let declaration = declaration.replace("forge[", &format!("{function_name}["));
        let namespace_value = namespace_value.replace("forge(", &format!("{function_name}("));
        let invocation = invocation.replace("forge(", &format!("{function_name}("));
        let text = format!(
            "namespace app\n{declaration}{namespace_value}function main(stdout: Stdout) returns nothing:\n    {invocation}\n    Stdout.write(view stdout, \"must not execute\\n\")\nverify valid_check:\n    assert true\nproperty valid_trials:\n    given value: int64\n    assert value == value\n"
        );
        fs::write(&source, &text).unwrap();
        let check_diagnostics = |checked: jett_driver::BuildResult| {
            let errors = checked
                .diagnostics
                .iter()
                .filter(|diagnostic| diagnostic.severity == jett_diagnostics::Severity::Error)
                .collect::<Vec<_>>();
            assert!(checked.has_errors, "{name}");
            assert_eq!(errors.len(), 1, "{name}: {errors:?}");
            assert_eq!(errors[0].code.code(), code, "{name}: {errors:?}");
            assert!(errors[0].message.contains("<never>"), "{name}: {errors:?}");
            assert!(errors[0].message.contains("int64"), "{name}: {errors:?}");
        };
        check_diagnostics(jett_driver::build_source(&text, "manufactured_never.jett"));
        check_diagnostics(jett_driver::build_file(&source));
        let expected = jett_driver::run_file_capture_outcome(&source)
            .expect_err("invalid Never values must prevent reference execution");
        assert!(expected.output.stdout.is_empty(), "{name}");
        assert!(expected.output.debug_output.is_empty(), "{name}");
        assert!(
            expected.message.contains(&format!("E{code:04}")),
            "{name}: {}",
            expected.message
        );

        let binary = directory.path().join("preserved.exe");
        let sentinel = b"existing native output";
        fs::write(&binary, sentinel).unwrap();
        // A checked source error must precede opening the launcher archive.
        let unused_launcher = if cfg!(windows) {
            NativeLauncherBundle::windows_msvc_static_v1(directory.path().join("unused.lib"))
        } else {
            NativeLauncherBundle::linux_gnu_v1(directory.path().join("unused.a"))
        };
        let check_native = |error: jett_driver::native::NativeBuildError| {
            let jett_driver::native::NativeBuildError::Lowering {
                source: lowering, ..
            } = error
            else {
                panic!("{name}: expected frontend rejection, got {error}");
            };
            let jett_driver::BackendLoweringError::Build(checked) = *lowering else {
                panic!("{name}: expected preserved diagnostics before HIR");
            };
            check_diagnostics(checked);
            assert_eq!(fs::read(&binary).unwrap(), sentinel, "{name}");
        };
        for release in [false, true] {
            check_native(
                jett_driver::native::build_host_executable_with_options(
                    &source,
                    &unused_launcher,
                    &binary,
                    jett_driver::BuildOptions { release },
                )
                .expect_err("invalid Never values must prevent native program publication"),
            );
        }
        check_native(
            build_host_verify_suite_executable(&source, &unused_launcher, &binary)
                .expect_err("invalid Never values must prevent native verify publication"),
        );
        check_native(
            build_host_property_suite_executable(&source, &unused_launcher, &binary)
                .expect_err("invalid Never values must prevent native property publication"),
        );
    }
}

#[test]
fn native_generic_lexical_type_scope_matches_interpreter() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/native/generic_lexical_type_scope.jett");
    let expected =
        jett_driver::run_file_capture_output(&fixture).expect("lexical type scope oracle");
    assert_eq!(expected.stdout, "T\nT\nT\nT\nT\nint64\nstring\n");
    let directory = tempfile::tempdir().unwrap();
    let binary = directory.path().join("generic_lexical_type_scope.exe");
    build_host_executable(&fixture, launcher(), &binary).expect("native lexical type scope");
    let actual = run_bounded(&binary, directory.path());
    assert!(actual.status.success(), "{actual:?}");
    assert_eq!(actual.stdout, expected.stdout.as_bytes());
    assert!(actual.stderr.is_empty(), "{actual:?}");
}

#[test]
fn native_generic_interface_identity_matches_interpreter() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/native/generic_interface_identity.jett");
    let expected =
        jett_driver::run_file_capture_output(&fixture).expect("generic interface oracle");
    assert_eq!(
        expected.stdout,
        "integer:17\ntext:hello\nsecret\ninteger:23\nsecret\ninteger:31\ninteger:31\n"
    );
    let directory = tempfile::tempdir().unwrap();
    let binary = directory.path().join("generic_interface_identity.exe");
    build_host_executable(&fixture, launcher(), &binary)
        .expect("native generic interface identity");
    let actual = run_bounded(&binary, directory.path());
    assert!(actual.status.success(), "{actual:?}");
    assert_eq!(actual.stdout, expected.stdout.as_bytes());
    assert!(actual.stderr.is_empty(), "{actual:?}");
}

#[test]
fn native_interface_refined_interfaces_match_interpreter() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/native/interface_refined_interfaces.jett");
    let expected = jett_driver::run_file_capture_outcome(&fixture)
        .unwrap_or_else(|error| panic!("refined interface oracle: {error:?}"));
    assert_eq!(
        expected.stdout,
        "selected:text:plain\nselected:small:7\nselected:record:kept\nagain:selected:text:plain\ntext:plain\nselected:text:plain\nselected:result-ok:9\nselected:result-fail:bad\nselected:optional-none\nselected:text:plain\nselected:small:7\nselected:record:kept\nagain:selected:text:plain\ntext:plain\nselected:text:plain\nselected:result-ok:9\nselected:result-fail:bad\nselected:optional-none\nselected:text:direct\nselected:text:direct\nselected:text:direct\nselected:text:direct\nselected:text:direct\nselected:text:direct\nselected:text:callback\nselected:text:callback\nselected:text:direct\nrefinement type constraint failed for 'app.Selected'\nrefinement type constraint failed for 'app.Selected'\nselected:text:fresh\nselected:text:fresh\nrefinement type constraint failed for 'app.Rejected'\n"
    );
    let debug = format!("{}\n", expected.debug_output.join("\n"));
    assert!(debug.contains("[redacted]"));
    assert!(!debug.contains("hidden-refined-interface"));
    let directory = tempfile::tempdir().unwrap();
    let binary = directory.path().join("interface_refined_interfaces.exe");
    build_host_executable(&fixture, launcher(), &binary).expect("native refined interfaces");
    let actual = run_bounded(&binary, directory.path());
    assert!(actual.status.success(), "{actual:?}");
    assert_eq!(actual.stdout, expected.stdout.as_bytes());
    assert_eq!(actual.stderr, debug.as_bytes());
}

#[test]
fn native_interface_method_returns_match_interpreter() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/native/interface_method_returns.jett");
    let expected =
        jett_driver::run_file_capture_output(&fixture).expect("interface method return oracle");
    assert_eq!(
        expected.stdout,
        "number:7\nnumber:7\nnumber:8\nnumber:7\ntext:hello\ntext:hello\ntext:hello!\ntext:hello\ntiny:127\ntiny:127\ntiny:-128\ntiny:127\nnumber:13\ntext:baked\ntiny:127\nnumber:13\ntext:baked\ntiny:127\n"
    );
    let directory = tempfile::tempdir().unwrap();
    let binary = directory.path().join("interface_method_returns.exe");
    build_host_executable(&fixture, launcher(), &binary).expect("native interface method returns");
    let actual = run_bounded(&binary, directory.path());
    assert!(actual.status.success(), "{actual:?}");
    assert_eq!(actual.stdout, expected.stdout.as_bytes());
    assert!(actual.stderr.is_empty(), "{actual:?}");
}

#[test]
fn native_interface_display_contexts_match_interpreter() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/native/interface_display_contexts.jett");
    let expected =
        jett_driver::run_file_capture_output(&fixture).expect("interface display context oracle");
    assert_eq!(
        expected.stdout,
        "erased:number:7\nconcrete:Ada\nerased:record:Ada\nconcrete:Ada\nerased:number:7\nerased:record:Ada\nfield:erased:record:Ada\ncall:erased:record:returned\nevaluated\nonce:erased:record:returned\nerased:record:returned\nerased:record:returned\nscoped:erased:record:returned\nnumber:7 Ada\nenabled\ndisabled\n"
    );
    let directory = tempfile::tempdir().unwrap();
    let binary = directory.path().join("interface_display_contexts.exe");
    build_host_executable(&fixture, launcher(), &binary).expect("native interface display");
    let actual = run_bounded(&binary, directory.path());
    assert!(actual.status.success(), "{actual:?}");
    assert_eq!(actual.stdout, expected.stdout.as_bytes());
    assert!(actual.stderr.is_empty(), "{actual:?}");
}

#[test]
fn native_interface_display_failure_matches_interpreter() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/native/interface_display_failure.jett");
    let expected = jett_driver::run_file_capture_outcome(&fixture)
        .expect_err("pending interface display must fail");
    assert_eq!(expected.output.stdout, "prefix\n");
    assert!(expected.output.debug_output.is_empty());
    assert_eq!(
        expected.message,
        "runtime error: undefined function 'app.Named.name'"
    );
    let directory = tempfile::tempdir().unwrap();
    let binary = directory.path().join("interface_display_failure.exe");
    build_host_executable(&fixture, launcher(), &binary).expect("native interface display failure");
    let actual = run_bounded(&binary, directory.path());
    assert_eq!(actual.status.code(), Some(71), "{actual:?}");
    assert_eq!(actual.stdout, expected.output.stdout.as_bytes());
    assert_eq!(actual.stderr, format!("{}\n", expected.message).as_bytes());
}

#[test]
fn native_enum_struct_equality_matches_interpreter_in_both_profiles() {
    let fixture =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/native/enum_struct_equality.jett");
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("enum_structs.jett");
    fs::copy(&fixture, &source).unwrap();
    let expected =
        jett_driver::run_file_capture_output(&source).expect("explicit payload equality oracle");
    assert_eq!(
        expected.stdout,
        "true true true true true true true true true true\n"
    );
    assert!(expected.debug_output.is_empty());
    let mut binaries = Vec::new();
    for release in [false, true] {
        let binary = directory.path().join(format!("enum_structs_{release}.exe"));
        jett_driver::native::build_host_executable_with_options(
            &source,
            launcher(),
            &binary,
            jett_driver::BuildOptions { release },
        )
        .expect("native explicit payload methods");
        binaries.push(binary);
    }
    fs::remove_file(&source).unwrap();
    for binary in binaries {
        let actual = run_bounded(&binary, directory.path());
        assert!(actual.status.success(), "{actual:?}");
        assert_eq!(actual.stdout, expected.stdout.as_bytes());
        assert!(actual.stderr.is_empty(), "{actual:?}");
    }
}

#[test]
fn native_enum_struct_equality_failure_cleans_cursor_in_both_profiles() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/native/enum_struct_equality_failure.jett");
    let original = fs::read_to_string(fixture).unwrap().replace("\r\n", "\n");
    let pending = original.replace("string rejected = string.repeat(\"ab\", 9223372036854775807)\n            return string.char_count(rejected) == 0", "return run true");
    assert_ne!(pending, original);
    for (original, message) in [
        (
            &original,
            "runtime error: string.repeat: requested output is too large",
        ),
        (&pending, "runtime error: Equatable.equals must return bool"),
    ] {
        for operator in ["==", "!="] {
            let directory = tempfile::tempdir().unwrap();
            let source = directory.path().join("enum_failure.jett");
            fs::write(
                &source,
                original.replace("\"left\\n\") ==", &format!("\"left\\n\") {operator}")),
            )
            .unwrap();
            let expected = jett_driver::run_file_capture_outcome(&source).unwrap_err();
            assert_eq!(expected.output.stdout, "true\nleft\nright\n");
            assert!(expected.output.debug_output.is_empty());
            assert_eq!(expected.message, message);
            let mut binaries = Vec::new();
            for release in [false, true] {
                let binary = directory.path().join(format!("enum_failure_{release}.exe"));
                jett_driver::native::build_host_executable_with_options(
                    &source,
                    launcher(),
                    &binary,
                    jett_driver::BuildOptions { release },
                )
                .expect("native failing payload method");
                binaries.push(binary);
            }
            fs::remove_file(&source).unwrap();
            for binary in binaries {
                let actual = run_bounded(&binary, directory.path());
                assert_eq!(actual.status.code(), Some(71), "{actual:?}");
                assert_eq!(actual.stdout, expected.output.stdout.as_bytes());
                assert_eq!(actual.stderr, format!("{}\n", expected.message).as_bytes());
            }
        }
    }
}

#[test]
fn native_interface_refined_small_values_match_interpreter_in_both_profiles() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/native/interface_refined_small_values.jett");
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("small_values.jett");
    fs::copy(&fixture, &source).unwrap();
    let expected = jett_driver::run_file_capture_output(&source)
        .expect("refined small-value interface oracle");
    assert_eq!(
        expected.stdout,
        "nothing\nunit\nagain\nbool:false\nenabled:true\nbytes:6f6b\ndata:6f6b\nnothing\nunit\nagain\nbool:false\nenabled:true\nbytes:6f6b\ndata:6f6b\nunit\nunit\n6f6b:6f6b:7368:7368:7368:2:data:6f6b:2:2\n6f6b:6f6b:7368:7368:7368:2:data:6f6b:2:2\n"
    );
    assert!(expected.debug_output.is_empty());
    let mut binaries = Vec::new();
    for release in [false, true] {
        let binary = directory.path().join(format!("small_values_{release}.exe"));
        jett_driver::native::build_host_executable_with_options(
            &source,
            launcher(),
            &binary,
            jett_driver::BuildOptions { release },
        )
        .expect("native refined small-value interfaces");
        binaries.push(binary);
    }
    fs::remove_file(&source).unwrap();
    for binary in binaries {
        let actual = run_bounded(&binary, directory.path());
        assert!(actual.status.success(), "{actual:?}");
        assert_eq!(actual.stdout, expected.stdout.as_bytes());
        assert!(actual.stderr.is_empty(), "{actual:?}");
    }
}

#[test]
fn native_transparent_borrow_argument_failure_cleans_owners_in_both_profiles() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/native/interface_refined_small_values.jett");
    let original = fs::read_to_string(fixture).unwrap();
    let (declarations, _) = original.split_once("function main(").unwrap();
    for callee in ["sized", "callback"] {
        for borrowed in ["view coarsen packet.data", "view declassify packet.hidden"] {
            let directory = tempfile::tempdir().unwrap();
            let source = directory.path().join("borrow_failure.jett");
            fs::write(&source, format!("{declarations}function failed() returns int64:\n    string rejected = string.repeat(\"ab\", 9223372036854775807)\n    return string.char_count(rejected)\nfunction main(stdout: Stdout) returns nothing:\n    Packet packet = Packet(data: bytes.from_string(\"ok\"), hidden: bytes.from_string(\"sh\")) handle error:\n        Stdout.write(view stdout, error)\n        return nothing\n    function(view bytes, int64) returns string callback = sized\n    Stdout.write(view stdout, {callee}({borrowed}, failed()))\n")).unwrap();
            let expected = jett_driver::run_file_capture_outcome(&source).unwrap_err();
            assert!(expected.output.stdout.is_empty(), "{expected:?}");
            assert!(expected.output.debug_output.is_empty(), "{expected:?}");
            assert_eq!(
                expected.message,
                "runtime error: string.repeat: requested output is too large"
            );
            let mut binaries = Vec::new();
            for release in [false, true] {
                let binary = directory
                    .path()
                    .join(format!("borrow_failure_{release}.exe"));
                jett_driver::native::build_host_executable_with_options(
                    &source,
                    launcher(),
                    &binary,
                    jett_driver::BuildOptions { release },
                )
                .expect("native failure following a transparent borrow");
                binaries.push(binary);
            }
            fs::remove_file(&source).unwrap();
            for binary in binaries {
                let actual = run_bounded(&binary, directory.path());
                assert_eq!(
                    actual.status.code(),
                    Some(71),
                    "{callee}({borrowed}): {actual:?}"
                );
                assert!(actual.stdout.is_empty(), "{actual:?}");
                assert_eq!(actual.stderr, format!("{}\n", expected.message).as_bytes());
            }
        }
    }
}

#[test]
fn native_interface_refined_actors_match_interpreter() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/native/interface_refined_actors.jett");
    let expected =
        jett_driver::run_file_capture_output(&fixture).expect("refined actor interface oracle");
    assert_eq!(
        expected.stdout,
        "counter selected again\n3\n3\nselected 5\ncounter\nselected\nagain\nselected\n7\n7\nselected\nrefinement type constraint failed for 'app.Rejected'\n3 3\n"
    );
    let debug = format!("{}\n", expected.debug_output.join("\n"));
    assert_eq!(
        debug,
        "trace copied: app.Holder = app.Holder(worker: pending(pending(actor#0)))\ntrace later: app.Named = pending(pending(actor#0))\n"
    );
    let directory = tempfile::tempdir().unwrap();
    let binary = directory.path().join("interface_refined_actors.exe");
    build_host_executable(&fixture, launcher(), &binary).expect("native refined actors");
    let actual = run_bounded(&binary, directory.path());
    assert!(actual.status.success(), "{actual:?}");
    assert_eq!(actual.stdout, expected.stdout.as_bytes());
    assert_eq!(actual.stderr, debug.as_bytes());
}

#[test]
fn native_refinement_declaration_contexts_match_interpreter() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/native/refinement_declaration_contexts.jett");
    let expected = jett_driver::run_file_capture_output(&fixture)
        .expect("refinement declaration context oracle");
    assert_eq!(
        expected.stdout,
        "selected:text:value\nselected:text:value\nagain:selected:text:value\ndecoy:alias\nstring list[string] 7 false\n7\nbigger:7:string\nrejected:int8:list[int8]:refinement type constraint failed for 'domain.Positive'\nfalse\nbool list[bool] 2 false\n2\nbigger-rejected:bool:list[bool]:refinement type constraint failed for 'domain.Bigger'\nfalse\n"
    );
    let directory = tempfile::tempdir().unwrap();
    let binary = directory.path().join("refinement_declaration_contexts.exe");
    build_host_executable(&fixture, launcher(), &binary)
        .expect("native refinement declaration contexts");
    let actual = run_bounded(&binary, directory.path());
    assert!(actual.status.success(), "{actual:?}");
    assert_eq!(actual.stdout, expected.stdout.as_bytes());
    assert!(actual.stderr.is_empty(), "{actual:?}");
}

#[test]
fn native_contextual_comptime_values_match_interpreter() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/native/contextual_comptime_values.jett");
    let expected =
        jett_driver::run_file_capture_output(&fixture).expect("contextual comptime oracle");
    assert_eq!(
        expected.stdout,
        "int64:primitive\nstring:primitive\napp.Label:alias\nint8\napp.Label\nint8\nstring\napp.Label\n0\n0\nint8\nlist[int8]\napp.Label\nlist[app.Label]\nstring\nlist[string]\nint8\napp.Label\nint8\nstring\nint8\noff\napp.T\napp.T\napp.T\nint16\nstring:primitive\napp.Label:alias\n"
    );
    let directory = tempfile::tempdir().unwrap();
    let binary = directory.path().join("contextual_comptime_values.exe");
    build_host_executable(&fixture, launcher(), &binary).expect("native contextual comptime");
    let actual = run_bounded(&binary, directory.path());
    assert!(actual.status.success(), "{actual:?}");
    assert_eq!(actual.stdout, expected.stdout.as_bytes());
    assert!(actual.stderr.is_empty(), "{actual:?}");
}

#[test]
fn native_reflected_opaque_fields_match_interpreter() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/native/reflected_opaque_fields.jett");
    let expected =
        jett_driver::run_file_capture_output(&fixture).expect("reflected opaque field oracle");
    assert_eq!(expected.stdout, "plain:5\nready\npending:5\nready\n");
    let directory = tempfile::tempdir().unwrap();
    let binary = directory.path().join("reflected_opaque_fields.exe");
    build_host_executable(&fixture, launcher(), &binary).expect("native reflected opaque fields");
    let actual = run_bounded(&binary, directory.path());
    assert!(actual.status.success(), "{actual:?}");
    assert_eq!(actual.stdout, expected.stdout.as_bytes());
    assert_eq!(
        String::from_utf8_lossy(&actual.stderr),
        format!("{}\n", expected.debug_output.join("\n"))
    );
}

#[test]
fn native_comptime_actor_computation_matches_interpreter() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/native/comptime_actor_computation.jett");
    let expected =
        jett_driver::run_file_capture_output(&fixture).expect("comptime actor computation oracle");
    assert_eq!(expected.stdout, "live:5\nbaked:5\n");
    let directory = tempfile::tempdir().unwrap();
    let binary = directory.path().join("comptime_actor_computation.exe");
    build_host_executable(&fixture, launcher(), &binary)
        .expect("native comptime actor computation");
    let actual = run_bounded(&binary, directory.path());
    assert!(actual.status.success(), "{actual:?}");
    assert_eq!(actual.stdout, expected.stdout.as_bytes());
    assert!(actual.stderr.is_empty(), "{actual:?}");
}

#[test]
fn native_indirect_owned_results_match_interpreter_in_both_profiles() {
    for (fixture_name, stdout, failure) in [
        (
            "indirect_owned_results",
            "true true true\n4 4 4 4 4 4 4\n",
            None,
        ),
        (
            "indirect_owned_result_failure",
            "1;2;3;",
            Some("runtime error: string.repeat: requested output is too large"),
        ),
    ] {
        let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join(format!("../../tests/native/{fixture_name}.jett"));
        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join(format!("{fixture_name}.jett"));
        fs::copy(&fixture, &source).unwrap();
        let interpreted = jett_driver::run_file_capture_outcome(&source);
        if let Some(message) = failure {
            let expected = interpreted.unwrap_err();
            assert_eq!(expected.message, message);
            assert_eq!(expected.output.stdout, stdout);
            assert!(expected.output.debug_output.is_empty());
        } else {
            let expected = interpreted.unwrap();
            assert_eq!(expected.stdout, stdout);
            assert!(expected.debug_output.is_empty());
        }
        let mut binaries = Vec::new();
        for release in [false, true] {
            let binary = directory
                .path()
                .join(format!("{fixture_name}_{release}.exe"));
            jett_driver::native::build_host_executable_with_options(
                &source,
                launcher(),
                &binary,
                jett_driver::BuildOptions { release },
            )
            .expect("native owned indirect results");
            binaries.push(binary);
        }
        fs::remove_file(&source).unwrap();
        for binary in binaries {
            let actual = run_bounded(&binary, directory.path());
            assert_eq!(
                actual.status.code(),
                Some(if failure.is_some() { 71 } else { 0 }),
                "{actual:?}"
            );
            assert_eq!(actual.stdout, stdout.as_bytes());
            let stderr = failure
                .map(|message| format!("{message}\n"))
                .unwrap_or_default();
            assert_eq!(actual.stderr, stderr.as_bytes(), "{actual:?}");
        }
    }
}

#[test]
fn native_global_constants_match_interpreter_without_source_in_both_profiles() {
    let fixture =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/native/global_constants.jett");
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("constants.jett");
    fs::copy(fixture, &source).unwrap();
    let expected = jett_driver::run_file_capture_output(&source).expect("constant oracle");
    let line = "-128:18446744073709551615:3.140000104904175:hello:42:true:true:43\n";
    assert_eq!(
        expected.stdout,
        format!("{}hello:42:42:42\n", line.repeat(3))
    );
    assert_eq!(
        expected.debug_output,
        [
            "trace minimum: app.Count = -128",
            "trace label: string = hello:42",
            "trace unit: nothing = nothing",
        ]
    );
    let debug = format!("{}\n", expected.debug_output.join("\n"));
    let mut binaries = Vec::new();
    for release in [false, true] {
        let binary = directory.path().join(format!("constants_{release}.exe"));
        jett_driver::native::build_host_executable_with_options(
            &source,
            launcher(),
            &binary,
            jett_driver::BuildOptions { release },
        )
        .expect("native constants");
        binaries.push((binary, release));
    }
    let verify_binary = directory.path().join("constants_verify.exe");
    build_host_verify_suite_executable(&source, launcher(), &verify_binary)
        .expect("native constant verify suite");
    let property_binary = directory.path().join("constants_property.exe");
    build_host_property_suite_executable(&source, launcher(), &property_binary)
        .expect("native constant property suite");
    fs::remove_file(source).unwrap();
    for (binary, release) in binaries {
        let actual = run_bounded(&binary, directory.path());
        assert!(actual.status.success(), "{actual:?}");
        assert_eq!(actual.stdout, expected.stdout.as_bytes());
        assert_eq!(
            actual.stderr,
            if release {
                b"".as_slice()
            } else {
                debug.as_bytes()
            }
        );
    }
    let verified = run_bounded(&verify_binary, directory.path());
    assert!(verified.status.success(), "{verified:?}");
    assert!(verified.stdout.is_empty(), "{verified:?}");
    assert_eq!(
        String::from_utf8_lossy(&verified.stderr),
        "trace answer: int64 = 42\nbreakpoint hit\n"
    );
    let property = run_bounded(&property_binary, directory.path());
    assert!(property.status.success(), "{property:?}");
    assert!(property.stdout.is_empty(), "{property:?}");
    assert_eq!(
        String::from_utf8_lossy(&property.stderr),
        "trace greeting: string = hello\n".repeat(100)
    );
}

#[test]
fn native_sibling_namespace_constants_stay_distinct_without_source() {
    let directory = tempfile::tempdir().unwrap();
    let sources = directory.path().join("src");
    fs::create_dir(&sources).unwrap();
    let manifest = directory.path().join("jett.proj");
    fs::write(
        &manifest,
        "name: sibling_constants\nversion: 0.1.0\nentry: src/main.jett\n",
    )
    .unwrap();
    let alpha = sources.join("00_alpha.jett");
    let bravo = sources.join("10_bravo.jett");
    // Equal-width namespace and value spellings put the constant declarations
    // at identical byte offsets. Their file identities must keep them distinct.
    for (path, namespace, answer) in [(&alpha, "alpha", 11), (&bravo, "bravo", 29)] {
        fs::write(
            path,
            format!(
                "namespace {namespace}\nint64 answer = {answer}\nexport function read() returns int64:\n    return answer\n"
            ),
        )
        .unwrap();
    }
    let source = sources.join("main.jett");
    fs::write(
        &source,
        r#"namespace app
int64 answer = 80
function combined() returns int64:
    use alpha
    use bravo
    return alpha.read() + bravo.read() + answer
function main(stdout: Stdout) returns nothing:
    use alpha
    use bravo
    Stdout.write(view stdout, "{alpha.read()}:{bravo.read()}:{answer}:{combined()}\n")
verify namespace_constants:
    use alpha
    use bravo
    assert alpha.read() == 11
    assert bravo.read() == 29
    assert answer == 80
    assert combined() == 120
property namespace_constant_reads:
    given value: int64
    use alpha
    use bravo
    assert alpha.read() == 11
    assert bravo.read() == 29
    assert answer == 80
    assert value + combined() - combined() == value
"#,
    )
    .unwrap();
    let expected =
        jett_driver::run_file_capture_output(&source).expect("namespace constant oracle");
    assert_eq!(expected.stdout, "11:29:80:120\n");
    assert!(expected.debug_output.is_empty());
    let mut binaries = Vec::new();
    for release in [false, true] {
        let binary = directory
            .path()
            .join(format!("namespace_constants_{release}.exe"));
        jett_driver::native::build_host_executable_with_options(
            &source,
            launcher(),
            &binary,
            jett_driver::BuildOptions { release },
        )
        .expect("native sibling namespace constants");
        binaries.push((binary, expected.stdout.as_bytes().to_vec()));
    }
    let verify_binary = directory.path().join("namespace_constants_verify.exe");
    build_host_verify_suite_executable(&source, launcher(), &verify_binary)
        .expect("native sibling namespace constant verify suite");
    binaries.push((verify_binary, Vec::new()));
    let property_binary = directory.path().join("namespace_constants_property.exe");
    build_host_property_suite_executable(&source, launcher(), &property_binary)
        .expect("native sibling namespace constant property suite");
    binaries.push((property_binary, Vec::new()));
    for path in [&source, &alpha, &bravo, &manifest] {
        fs::remove_file(path).unwrap();
    }
    for (binary, stdout) in binaries {
        let actual = run_bounded(&binary, directory.path());
        assert!(actual.status.success(), "{actual:?}");
        assert_eq!(actual.stdout, stdout, "{actual:?}");
        assert!(actual.stderr.is_empty(), "{actual:?}");
    }
}

#[test]
fn native_wrapped_enum_equality_matches_interpreter_in_both_profiles() {
    let fixture =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/native/wrapped_enum_equality.jett");
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("wrapped_enum.jett");
    fs::copy(&fixture, &source).unwrap();
    let expected = jett_driver::run_file_capture_output(&source).expect("wrapped equality oracle");
    assert_eq!(
        expected.stdout,
        "true true true true true true true true true true true true\ntrue true true true true true\n"
    );
    assert!(expected.debug_output.is_empty());
    let mut binaries = Vec::new();
    for release in [false, true] {
        let binary = directory.path().join(format!("wrapped_enum_{release}.exe"));
        jett_driver::native::build_host_executable_with_options(
            &source,
            launcher(),
            &binary,
            jett_driver::BuildOptions { release },
        )
        .expect("native wrapped enum equality");
        binaries.push(binary);
    }
    fs::remove_file(&source).unwrap();
    for binary in binaries {
        let actual = run_bounded(&binary, directory.path());
        assert!(actual.status.success(), "{actual:?}");
        assert_eq!(actual.stdout, expected.stdout.as_bytes());
        assert!(actual.stderr.is_empty(), "{actual:?}");
    }
}

#[test]
fn native_pending_sum_handles_fail_before_extraction_in_both_profiles() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/native/pending_sum_handle_failure.jett");
    let template = fs::read_to_string(fixture).unwrap();
    for (ty, value, payload, rendered, handle) in [
        (
            "optional[string]",
            "some(\"payload\")",
            "string",
            "some(payload)",
            "handle:",
        ),
        ("optional[string]", "none", "string", "none", "handle:"),
        (
            "result[string, int64]",
            "ok(\"payload\")",
            "string",
            "ok(payload)",
            "handle error:",
        ),
        (
            "result[string, int64]",
            "fail(17)",
            "string",
            "fail(17)",
            "handle error:",
        ),
        (
            "optional[Record]",
            "some(Record(label: \"payload\"))",
            "Record",
            "some(app.Record(label: payload))",
            "handle:",
        ),
    ] {
        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join("pending_sum.jett");
        let text = template
            .replace("optional[string]", ty)
            .replace("some(\"payload\")", value)
            .replace("string found", &format!("{payload} found"))
            .replace("handle:", handle);
        fs::write(&source, text).unwrap();
        let expected = jett_driver::run_file_capture_outcome(&source).unwrap_err();
        assert_eq!(expected.output.stdout, "before;");
        assert_eq!(
            expected.message,
            format!(
                "runtime error: handle block requires a result or optional value, got pending(pending({rendered}))"
            )
        );
        assert!(expected.output.debug_output.is_empty());
        let mut binaries = Vec::new();
        for release in [false, true] {
            let binary = directory.path().join(format!("pending_sum_{release}.exe"));
            jett_driver::native::build_host_executable_with_options(
                &source,
                launcher(),
                &binary,
                jett_driver::BuildOptions { release },
            )
            .expect("native pending sum handle");
            binaries.push(binary);
        }
        fs::remove_file(&source).unwrap();
        for binary in binaries {
            let actual = run_bounded(&binary, directory.path());
            assert_eq!(actual.status.code(), Some(71), "{actual:?}");
            assert_eq!(actual.stdout, expected.output.stdout.as_bytes());
            assert_eq!(
                actual.stderr,
                format!("{}\n", expected.message).as_bytes(),
                "{actual:?}"
            );
        }
    }
}

#[test]
fn native_pending_aggregate_access_fails_before_observation_in_both_profiles() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/native/pending_aggregate_access.jett");
    let template = fs::read_to_string(&fixture).unwrap();
    let (declarations, _) = template.rsplit_once("function main(").unwrap();
    for (scenario, message) in [
        (
            "direct_record_failure",
            "field access is not supported on pending(app.Record(label: Ada, items: list(7)))",
        ),
        (
            "nested_record_failure",
            "field access is not supported on pending(pending(app.Record(label: Ada, items: list(7))))",
        ),
        (
            "cloned_record_failure",
            "field access is not supported on pending(pending(app.Record(label: Ada, items: list(7))))",
        ),
        (
            "machine_field_failure",
            "field access is not supported on pending(pending(app.Session@active(Ada, 7)))",
        ),
        (
            "machine_state_failure",
            "'at' requires a machine value, got pending(pending(app.Session@active(Ada, 7)))",
        ),
        (
            "broad_machine_state_failure",
            "'at' requires a machine value, got pending(pending(app.Session@active(Ada, 7)))",
        ),
        (
            "machine_transition_failure",
            "app.Session.transition: first argument must be a machine value",
        ),
        (
            "enum_match_failure",
            "match requires an enum value, got pending(pending(app.Choice.rows(list(7))))",
        ),
        (
            "enum_payload_failure",
            "unsupported binary operation: pending(7) Add 1",
        ),
        (
            "projected_iteration_failure",
            "field access is not supported on pending(pending(app.Record(label: Ada, items: list(7))))",
        ),
    ] {
        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join("pending_aggregate.jett");
        fs::write(
            &source,
            format!(
                "{declarations}function main(stdout: Stdout) returns nothing:\n    {scenario}(view stdout)\n"
            ),
        )
        .unwrap();
        let expected = jett_driver::run_file_capture_outcome(&source).unwrap_err();
        assert_eq!(expected.output.stdout, "before\n", "{scenario}");
        assert_eq!(
            expected.message,
            format!("runtime error: {message}"),
            "{scenario}"
        );
        let debug = if scenario == "enum_payload_failure" {
            assert_eq!(
                expected.output.debug_output,
                [
                    "trace value: int64 = pending(pending(7))",
                    "trace joined: int64 = pending(7)",
                ],
                "{scenario}"
            );
            format!("{}\n", expected.output.debug_output.join("\n"))
        } else {
            assert!(
                expected.output.debug_output.is_empty(),
                "{scenario}: {expected:?}"
            );
            String::new()
        };
        let mut binaries = Vec::new();
        for release in [false, true] {
            let binary = directory
                .path()
                .join(format!("pending_aggregate_{release}.exe"));
            jett_driver::native::build_host_executable_with_options(
                &source,
                launcher(),
                &binary,
                jett_driver::BuildOptions { release },
            )
            .unwrap_or_else(|error| panic!("{scenario}, release={release}: {error}"));
            binaries.push((binary, release));
        }
        fs::remove_file(&source).unwrap();
        for (binary, release) in binaries {
            let actual = run_bounded(&binary, directory.path());
            assert_eq!(actual.status.code(), Some(71), "{scenario}: {actual:?}");
            assert_eq!(
                actual.stdout,
                expected.output.stdout.as_bytes(),
                "{scenario}"
            );
            let debug = if release { "" } else { debug.as_str() };
            assert_eq!(
                actual.stderr,
                format!("{debug}{}\n", expected.message).as_bytes(),
                "{scenario}, release={release}: {actual:?}"
            );
        }
    }
}

#[test]
fn native_ready_and_joined_aggregate_access_preserves_owners_in_both_profiles() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/native/pending_aggregate_access.jett");
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("ready_aggregate.jett");
    fs::copy(&fixture, &source).unwrap();
    let expected = jett_driver::run_file_capture_output(&source).expect("ready aggregate oracle");
    assert_eq!(
        expected.stdout,
        "record:Ada:1\nrecord:Ada:1\nrecord:Ada:1\n7\ntrue:Ada:7\ntrue:Ada:7\n1:1\n1:1\n7\nint8:8\nfloat32:1.5\nbool:false\nnothing joined\n"
    );
    assert_eq!(
        expected.debug_output,
        [
            "trace value: int8 = pending(pending(7))",
            "trace once: int8 = pending(7)",
            "trace ready: int8 = 7",
            "trace value: float32 = pending(pending(1.25))",
            "trace once: float32 = pending(1.25)",
            "trace ready: float32 = 1.25",
            "trace value: bool = pending(pending(true))",
            "trace once: bool = pending(true)",
            "trace ready: bool = true",
            "trace value: nothing = pending(pending(nothing))",
            "trace once: nothing = pending(nothing)",
            "trace ready: nothing = nothing",
        ]
    );
    let mut binaries = Vec::new();
    for release in [false, true] {
        let binary = directory
            .path()
            .join(format!("ready_aggregate_{release}.exe"));
        jett_driver::native::build_host_executable_with_options(
            &source,
            launcher(),
            &binary,
            jett_driver::BuildOptions { release },
        )
        .expect("native ready and joined aggregate control");
        binaries.push((binary, release));
    }
    fs::remove_file(&source).unwrap();
    for (binary, release) in binaries {
        let actual = run_bounded(&binary, directory.path());
        assert!(actual.status.success(), "{actual:?}");
        assert_eq!(actual.stdout, expected.stdout.as_bytes());
        let debug = if release {
            String::new()
        } else {
            format!("{}\n", expected.debug_output.join("\n"))
        };
        assert_eq!(actual.stderr, debug.as_bytes(), "{actual:?}");
    }
}

#[test]
fn native_projected_reads_and_reconstructed_owners_match_interpreter_in_both_profiles() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/native/projected_read_controls.jett");
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("projected_read_controls.jett");
    fs::copy(&fixture, &source).unwrap();
    let expected = jett_driver::run_file_capture_output(&source).expect("projected read oracle");
    assert_eq!(
        expected.stdout,
        "original:1\ncopied:2\noriginal:1\nloop:3\nmatch:2:2\ntemporary:1\nfields:7:1\n"
    );
    assert!(expected.debug_output.is_empty());
    let mut binaries = Vec::new();
    for release in [false, true] {
        let binary = directory
            .path()
            .join(format!("projected_read_{release}.exe"));
        jett_driver::native::build_host_executable_with_options(
            &source,
            launcher(),
            &binary,
            jett_driver::BuildOptions { release },
        )
        .expect("native projected read control");
        binaries.push(binary);
    }
    let verify_binary = directory.path().join("projected_read_verify.exe");
    build_host_verify_suite_executable(&source, launcher(), &verify_binary)
        .expect("native projected read verify suite");
    let property_binary = directory.path().join("projected_read_property.exe");
    build_host_property_suite_executable(&source, launcher(), &property_binary)
        .expect("native projected read property suite");
    fs::remove_file(&source).unwrap();
    for binary in binaries {
        let actual = run_bounded(&binary, directory.path());
        assert!(actual.status.success(), "{actual:?}");
        assert_eq!(actual.stdout, expected.stdout.as_bytes());
        assert!(actual.stderr.is_empty(), "{actual:?}");
    }
    for suite in [verify_binary, property_binary] {
        let actual = run_bounded(&suite, directory.path());
        assert!(actual.status.success(), "{actual:?}");
        assert!(actual.stdout.is_empty(), "{actual:?}");
        assert!(actual.stderr.is_empty(), "{actual:?}");
    }
}

#[test]
fn native_interface_refined_sums_match_interpreter_in_both_profiles() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/native/interface_refined_sums.jett");
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("refined_sums.jett");
    fs::copy(&fixture, &source).unwrap();
    let expected =
        jett_driver::run_file_capture_output(&source).expect("refined interface sums oracle");
    let values = "choice:small:7;again:choice:small:7;outcome:text;outcome-fail:kept;choice:small:7;outcome:text;\n";
    assert_eq!(
        expected.stdout,
        format!(
            "{values}{values}choice rejected\nchoice rejected\noutcome rejected\noutcome rejected\nchoice:small:11;choice:small:11;\nchoice:small:11\nabsent rejected\nabsent rejected\npending rejected\nchoice:small:11\n17:true\n17:true\n"
        )
    );
    assert!(expected.debug_output.is_empty());
    let mut binaries = Vec::new();
    for release in [false, true] {
        let binary = directory.path().join(format!("refined_sums_{release}.exe"));
        jett_driver::native::build_host_executable_with_options(
            &source,
            launcher(),
            &binary,
            jett_driver::BuildOptions { release },
        )
        .expect("native refined interface sums");
        binaries.push(binary);
    }
    fs::remove_file(&source).unwrap();
    for binary in binaries {
        let actual = run_bounded(&binary, directory.path());
        assert!(actual.status.success(), "{actual:?}");
        assert_eq!(actual.stdout, expected.stdout.as_bytes());
        assert!(actual.stderr.is_empty(), "{actual:?}");
    }
}

#[test]
fn native_interface_math_facade_contexts_match_interpreter_in_both_profiles() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/native/interface_math_facade_contexts.jett");
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("math_facades.jett");
    fs::copy(&fixture, &source).unwrap();
    let expected =
        jett_driver::run_file_capture_output(&source).expect("math facade context oracle");
    let primitives = "integer:3;fraction:1.25;integer:1;fraction:1.25;integer:2;fraction:2.5;\n";
    let hidden = "hidden-integer;hidden-fraction;hidden-integer;hidden-fraction;hidden-integer;hidden-fraction;\n";
    let callbacks = "integer:3;fraction:1.25;hidden-integer;hidden-fraction;\n";
    assert_eq!(
        expected.stdout,
        format!("{primitives}{hidden}{callbacks}{primitives}{hidden}{callbacks}{callbacks}")
    );
    assert!(expected.debug_output.is_empty());
    let mut binaries = Vec::new();
    for release in [false, true] {
        let binary = directory.path().join(format!("math_facades_{release}.exe"));
        jett_driver::native::build_host_executable_with_options(
            &source,
            launcher(),
            &binary,
            jett_driver::BuildOptions { release },
        )
        .expect("native math facade contexts");
        binaries.push(binary);
    }
    fs::remove_file(&source).unwrap();
    for binary in binaries {
        let actual = run_bounded(&binary, directory.path());
        assert!(actual.status.success(), "{actual:?}");
        assert_eq!(actual.stdout, expected.stdout.as_bytes());
        assert!(actual.stderr.is_empty(), "{actual:?}");
    }
}

#[test]
fn native_interface_facade_results_match_interpreter() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/native/interface_facade_results.jett");
    let expected = jett_driver::run_file_capture_output(&fixture).expect("facade result oracle");
    let typed = "small:ok\nsmall:fail:expected int8 in range -128..127\nwide:ok\nwide:fail:expected int64, got null\nfloat32:ok\nfloat32:fail:expected float64, got bool\npositive:ok\npositive:fail:refinement type constraint failed for 'app.Positive'\nrecord:ok\nrecord:fail:count: expected int8 in range -128..127\nrecord:fail:unknown field 'extra' for app.Record\n";
    let raw = "tree:fail:unterminated JSON array\nwide:fail:expected int64, got string\ntree:fail:missing array item 0\nmaybe-tree:ok\nmaybe-tree:fail:expected object, got array\n";
    assert_eq!(expected.stdout, format!("{typed}{typed}{raw}{raw}"));
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
            .any(|line| line.contains("hidden-facade-result"))
    );
    let directory = tempfile::tempdir().unwrap();
    let binary = directory.path().join("interface_facade_results.exe");
    build_host_executable(&fixture, launcher(), &binary).expect("native facade results");
    let actual = run_bounded(&binary, directory.path());
    assert!(actual.status.success(), "{actual:?}");
    assert_eq!(actual.stdout, expected.stdout.as_bytes());
    assert_eq!(
        String::from_utf8_lossy(&actual.stderr),
        format!("{}\n", expected.debug_output.join("\n"))
    );
}

#[test]
fn native_interface_refined_containers_match_interpreter() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/native/interface_refined_containers.jett");
    let expected =
        jett_driver::run_file_capture_output(&fixture).expect("refined container oracle");
    let mixed = "mapping:1\nmembers:1\nsome:9\nnone\nok:11\nfail:problem\ncallback:5\ncallback:7\ncallback:7\nfunction:5\n";
    assert_eq!(expected.stdout, mixed.repeat(2));
    let directory = tempfile::tempdir().unwrap();
    let binary = directory.path().join("interface_refined_containers.exe");
    build_host_executable(&fixture, launcher(), &binary).expect("native refined containers");
    let actual = run_bounded(&binary, directory.path());
    assert!(actual.status.success(), "{actual:?}");
    assert_eq!(actual.stdout, expected.stdout.as_bytes());
    assert_eq!(
        String::from_utf8_lossy(&actual.stderr),
        format!("{}\n", expected.debug_output.join("\n"))
    );
}

#[test]
fn native_interface_reflected_fields_match_interpreter() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/native/interface_reflected_fields.jett");
    let expected =
        jett_driver::run_file_capture_output(&fixture).expect("reflected interface oracle");
    assert_eq!(
        expected.stdout,
        "small:7\nsmall:7\nwide:9\nsecret\n".repeat(9)
    );
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
            .any(|line| line.contains("hidden-reflected-interface"))
    );
    let directory = tempfile::tempdir().unwrap();
    let binary = directory.path().join("interface_reflected_fields.exe");
    build_host_executable(&fixture, launcher(), &binary).expect("native reflected interfaces");
    let actual = run_bounded(&binary, directory.path());
    assert!(actual.status.success(), "{actual:?}");
    assert_eq!(actual.stdout, expected.stdout.as_bytes());
    assert_eq!(
        String::from_utf8_lossy(&actual.stderr),
        format!("{}\n", expected.debug_output.join("\n"))
    );
}

#[test]
fn native_interface_opaque_identity_matches_interpreter() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/native/interface_opaque_identity.jett");
    let expected = jett_driver::run_file_capture_output(&fixture).expect("opaque interface oracle");
    assert_eq!(
        expected.stdout,
        "first\n5\nfirst\n5\nfirst\n7\n2\nfirst\nsecond\nfirst\nchecked-builder\nchecked-builder\nrecord:7\nrecord:9\nbuilder\nchecked-builder\nrecord:9\nchecked-builder\nrecord:9\nchecked-builder\nchecked-builder\nbuilder\nchecked-builder\nbuilder\noutput\noutput\nreadyoutput\noutput\nstdout\n"
    );
    let directory = tempfile::tempdir().unwrap();
    let binary = directory.path().join("interface_opaque_identity.exe");
    build_host_executable(&fixture, launcher(), &binary).expect("native opaque interfaces");
    let actual = run_bounded(&binary, directory.path());
    assert!(actual.status.success(), "{actual:?}");
    assert_eq!(actual.stdout, expected.stdout.as_bytes());
    assert!(actual.stderr.is_empty(), "{actual:?}");
}

#[test]
fn native_interface_facade_identity_matches_interpreter() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/native/interface_facade_identity.jett");
    let expected = jett_driver::run_file_capture_output(&fixture).expect("facade interface oracle");
    let mixed = "small:7\nwide:7\nfloat32:1.5\nfloat64:1.5\nsmall-list:2\nwide-list:2\npositive:9\nsmall-box:11\nsecret-box\nevent:13\nheader:15\nsession:17\nactive:19\n";
    let math = "wide:3\nfloat64:3.5\nwide:5\nfloat64:6\n";
    assert_eq!(
        expected.stdout,
        format!("{mixed}{mixed}small:21\nsmall:23\n{math}{math}")
    );
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
            .any(|line| line.contains("hidden-json-interface"))
    );
    let directory = tempfile::tempdir().unwrap();
    let binary = directory.path().join("interface_facade_identity.exe");
    build_host_executable(&fixture, launcher(), &binary).expect("native facade interfaces");
    let actual = run_bounded(&binary, directory.path());
    assert!(actual.status.success(), "{actual:?}");
    assert_eq!(actual.stdout, expected.stdout.as_bytes());
    assert_eq!(
        String::from_utf8_lossy(&actual.stderr),
        format!("{}\n", expected.debug_output.join("\n"))
    );
}

#[test]
fn native_interface_nominal_identity_matches_interpreter() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/native/interface_nominal_identity.jett");
    let expected =
        jett_driver::run_file_capture_output(&fixture).expect("nominal interface oracle");
    let mixed = "active:7\nsession:7\nempty-state\nrefined-session:7\nidle\nevent:normal\ntagged:chosen\nheader:2\nnonzero-header:3\n";
    assert_eq!(
        expected.stdout,
        format!(
            "{mixed}{mixed}empty-state\nactive:9\nsession:9\nactive:9\nactive:7\nactive:7\nactive:7\nactive:7\n"
        )
    );
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
            .any(|line| line.contains("hidden-nominal-secret"))
    );
    let directory = tempfile::tempdir().unwrap();
    let binary = directory.path().join("interface_nominal_identity.exe");
    build_host_executable(&fixture, launcher(), &binary).expect("native nominal interfaces");
    let actual = run_bounded(&binary, directory.path());
    assert!(actual.status.success(), "{actual:?}");
    assert_eq!(actual.stdout, expected.stdout.as_bytes());
    assert_eq!(
        String::from_utf8_lossy(&actual.stderr),
        format!("{}\n", expected.debug_output.join("\n"))
    );
}

#[test]
fn native_interface_function_identity_matches_interpreter() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/native/interface_function_identity.jett");
    let expected =
        jett_driver::run_file_capture_output(&fixture).expect("function interface oracle");
    let mixed = "reader:7\nreader:11\nwords:hello\nitems:3\nborrowed\nowned\nnested:8\nsmall:17\nreader:19\n";
    assert_eq!(
        expected.stdout,
        format!(
            "{mixed}{mixed}reader:11\n11\n11\nreader:11\nreader:11\nreader:11\nbroad:13\nnarrow:13\nnarrow:13\ncalled:13\ncalled:13\nnarrow:13\nlabel-factory:made\nnamed-factory:made\nnamed-factory:made\nnamed-factory:made\nmade\n"
        )
    );
    let directory = tempfile::tempdir().unwrap();
    let binary = directory.path().join("interface_function_identity.exe");
    build_host_executable(&fixture, launcher(), &binary).expect("native function interfaces");
    let actual = run_bounded(&binary, directory.path());
    assert!(actual.status.success(), "{actual:?}");
    assert_eq!(actual.stdout, expected.stdout.as_bytes());
    assert_eq!(
        String::from_utf8_lossy(&actual.stderr),
        format!("{}\n", expected.debug_output.join("\n"))
    );
}

#[test]
fn native_interface_collection_identity_matches_interpreter() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/native/interface_collection_identity.jett");
    let expected =
        jett_driver::run_file_capture_output(&fixture).expect("collection interface oracle");
    let mixed = "numbers:2\nwords:2\nsecrets:1\nnonempty:3\nnumber-map:1\nword-map:2\nnumber-set:1\nword-set:1\nmaybe-number:7\nmaybe-word:hello\nno-number\nno-word\nnumber-ok:9\nword-ok:yes\nnumber-fail:bad\nword-fail:11\n";
    assert_eq!(
        expected.stdout,
        format!(
            "{mixed}{mixed}numbers:2\nnumbers:2\nnumbers:2\nnumbers:2\nnumbers:2\ninterfaces:3\nnumbers:1\nnumbers:2\nwords:1\ninterfaces:3\ncounted:4\ncounted:4\nnumbers:3\nnumbers:3\nnumbers:4\nnumbers:4\n6\n2\n9\n17\n"
        )
    );
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
            .any(|line| line.contains("hidden-collection-secret"))
    );
    let directory = tempfile::tempdir().unwrap();
    let binary = directory.path().join("interface_collection_identity.exe");
    build_host_executable(&fixture, launcher(), &binary).expect("native collection interfaces");
    let actual = run_bounded(&binary, directory.path());
    assert!(actual.status.success(), "{actual:?}");
    assert_eq!(actual.stdout, expected.stdout.as_bytes());
    assert_eq!(
        String::from_utf8_lossy(&actual.stderr),
        format!("{}\n", expected.debug_output.join("\n"))
    );
}

#[test]
fn native_interface_primitive_identity_matches_interpreter() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/native/interface_primitive_identity.jett");
    let expected =
        jett_driver::run_file_capture_output(&fixture).expect("primitive interface oracle");
    let mixed = "int8:7\nint64:7\nuint8:9\nuint64:9\nfloat32\nfloat64\nint16:15\nint32:31\nuint16:16\nuint32:32\npositive:7\nnonempty:yes\nstring:yes\n";
    assert_eq!(
        expected.stdout,
        format!(
            "{mixed}{mixed}int8:7\nint8:7\npositive:7\nint8:8\nint8:8\nint8:7\nint8:7\nint8:11\nint8:11\nint8:11\nint8:11\nint8:12\nint8:13\nint8:7\nint8:7\n"
        )
    );
    let directory = tempfile::tempdir().unwrap();
    let binary = directory.path().join("interface_primitive_identity.exe");
    build_host_executable(&fixture, launcher(), &binary).expect("native primitive interfaces");
    let actual = run_bounded(&binary, directory.path());
    assert!(actual.status.success(), "{actual:?}");
    assert_eq!(actual.stdout, expected.stdout.as_bytes());
    assert_eq!(
        String::from_utf8_lossy(&actual.stderr),
        format!("{}\n", expected.debug_output.join("\n"))
    );
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
            .any(|line| line.contains("hidden-interface-token")
                || line.contains("hidden-generic-interface")
                || line.contains("reflection-failed"))
    );
    assert!(
        expected
            .debug_output
            .iter()
            .any(|line| line.contains("visible-generic-interface"))
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
fn native_builds_reject_mutable_globals_before_publication() {
    let fixture =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/compile_fail/mutable_global.jett");
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("mutable_global.jett");
    fs::copy(fixture, &source).unwrap();
    let reference = jett_driver::run_file_capture_output(&source).unwrap_err();
    assert!(reference.contains("E0377"), "{reference}");
    for release in [false, true] {
        let binary = directory.path().join("preserved.exe");
        fs::write(&binary, b"existing output").unwrap();
        let error = jett_driver::native::build_host_executable_with_options(
            &source,
            launcher(),
            &binary,
            jett_driver::BuildOptions { release },
        )
        .unwrap_err();
        let jett_driver::native::NativeBuildError::Lowering {
            source: lowering, ..
        } = error
        else {
            panic!("expected frontend rejection, got {error}");
        };
        let jett_driver::BackendLoweringError::Build(checked) = *lowering else {
            panic!("expected diagnostics before HIR");
        };
        let errors = checked
            .diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.severity == jett_diagnostics::Severity::Error)
            .collect::<Vec<_>>();
        assert_eq!(errors.len(), 2, "{errors:?}");
        assert!(
            errors
                .iter()
                .all(|diagnostic| diagnostic.code.code() == 377)
        );
        assert_eq!(fs::read(&binary).unwrap(), b"existing output");
    }
}

#[test]
fn native_builds_reject_direct_collection_equality_before_publication() {
    for ty in [
        "bytes",
        "list[int64]",
        "map[string, int64]",
        "set[int64]",
        "optional[int64]",
        "result[int64, string]",
    ] {
        for op in ["==", "!="] {
            let directory = tempfile::tempdir().unwrap();
            let source = directory.path().join("comparison.jett");
            fs::write(&source, format!("namespace app\nfunction compare(view left: {ty}, view right: {ty}) returns bool:\n    return left {op} right\nfunction main() returns nothing:\n    return nothing\n")).unwrap();
            for release in [false, true] {
                let binary = directory.path().join("preserved.exe");
                fs::write(&binary, b"existing output").unwrap();
                let error = jett_driver::native::build_host_executable_with_options(
                    &source,
                    launcher(),
                    &binary,
                    jett_driver::BuildOptions { release },
                )
                .unwrap_err();
                let jett_driver::native::NativeBuildError::Lowering {
                    source: lowering, ..
                } = error
                else {
                    panic!("{ty} {op}: expected frontend rejection, got {error}");
                };
                let jett_driver::BackendLoweringError::Build(checked) = *lowering else {
                    panic!("{ty} {op}: expected diagnostics before HIR");
                };
                let errors = checked
                    .diagnostics
                    .iter()
                    .filter(|diagnostic| diagnostic.severity == jett_diagnostics::Severity::Error)
                    .collect::<Vec<_>>();
                assert_eq!(errors.len(), 1, "{ty} {op}: {errors:?}");
                assert_eq!(errors[0].code.code(), 376);
                assert_eq!(
                    errors[0].message,
                    format!(
                        "operator `{op}` is not defined for `{ty}`; compare its contents explicitly"
                    )
                );
                assert_eq!(fs::read(&binary).unwrap(), b"existing output");
            }
        }
    }
}

#[test]
fn native_builds_preserve_shrunk_property_diagnostics_and_existing_artifacts() {
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("property_failure.jett");
    let text = r#"namespace app
function main() returns nothing:
    return nothing
verify first:
    assert true
property first_property:
    given n: int8
    assert n == n
property shrunk_failure:
    given number: int8
    given items: list[int64]
    assert number < 2 "native property sentinel"
"#;
    fs::write(&source, text).unwrap();
    let tested = jett_driver::test_file(&source).expect("checked property failure");
    assert_eq!((tested.total, tested.passed, tested.failed), (3, 2, 1));
    let failure = tested.blocks.iter().find(|block| !block.passed).unwrap();
    assert_eq!(failure.name, "shrunk_failure");
    assert!(failure.is_property);
    assert_eq!(failure.iterations, Some(4));
    assert_eq!(failure.line, 9);
    assert_eq!(
        failure.error.as_deref(),
        Some("native property sentinel (counterexample: number = 2, items = list())")
    );

    let checked = jett_driver::build_file(&source);
    assert!(checked.has_errors);
    let diagnostics = |result: &jett_driver::BuildResult| {
        result
            .diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.severity == jett_diagnostics::Severity::Error)
            .map(|diagnostic| {
                (
                    diagnostic.code.code(),
                    diagnostic.span,
                    diagnostic.message.clone(),
                )
            })
            .collect::<Vec<_>>()
    };
    let expected = diagnostics(&checked);
    assert_eq!(expected.len(), 1);
    assert_eq!(expected[0].0, 9000);
    assert_eq!(
        &text[expected[0].1.start as usize..expected[0].1.end as usize],
        "shrunk_failure"
    );
    assert_eq!(
        expected[0].2,
        format!(
            "comptime verify failed in 'shrunk_failure': {}",
            failure.error.as_ref().unwrap()
        )
    );

    for mode in ["program", "verify", "property"] {
        let binary = directory.path().join(format!("{mode}.exe"));
        fs::write(&binary, b"existing output sentinel").unwrap();
        let result = match mode {
            "program" => build_host_executable(&source, launcher(), &binary),
            "verify" => build_host_verify_suite_executable(&source, launcher(), &binary),
            "property" => build_host_property_suite_executable(&source, launcher(), &binary),
            _ => unreachable!(),
        };
        let error = result.expect_err("a failed property must prevent native publication");
        let jett_driver::native::NativeBuildError::Lowering {
            source: lowering, ..
        } = error
        else {
            panic!("{mode}: expected frontend rejection, got {error}");
        };
        let jett_driver::BackendLoweringError::Build(native) = *lowering else {
            panic!("{mode}: expected property diagnostics before HIR");
        };
        assert_eq!(diagnostics(&native), expected, "{mode}");
        assert_eq!(
            fs::read(&binary).unwrap(),
            b"existing output sentinel",
            "{mode}"
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
        match jett_driver::native::run_host_property_suite(
            &source,
            launcher(),
            jett_driver::native::NativePropertyOptions::default(),
        ) {
            Ok(result) => {
                if result.failure.is_none() {
                    assert_eq!(
                        result.trials,
                        count * jett_comptime::verify::PROPERTY_DEFAULT_ITERATIONS
                    );
                    passed += 1;
                } else {
                    failures.push(format!("{}: {result:?}", source.display()));
                }
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
fn native_deep_interface_conversion_matches_interpreter_in_both_profiles() {
    // Keep frontend recursion separate from the linked program's runtime stack.
    std::thread::Builder::new().stack_size(8 * 1024 * 1024).spawn(|| {
        let concrete = format!("{}User{}", "list[".repeat(130), "]".repeat(130));
        let element = format!("{}Named{}", "list[".repeat(129), "]".repeat(129));
        let source_text = format!("namespace app\ninterface Named:\n    function name(view self: Named) returns string\nstruct User:\n    value: string\nimplement Named for User:\n    function name(view self: User) returns string:\n        return self.value\ntype Concrete = {concrete}\ntype Element = {element}\ntype Erased = list[Element]\nfunction main(stdout: Stdout) returns nothing:\n    Concrete values = list()\n    Erased converted = values\n    Stdout.write(view stdout, \"{{list.length[Element](view converted)}}\\n\")\n");
        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join("deep_conversion.jett");
        fs::write(&source, source_text).unwrap();
        let expected = jett_driver::run_file_capture_output(&source).unwrap();
        assert_eq!(expected.stdout, "0\n");
        let mut binaries = Vec::new();
        for release in [false, true] {
            let binary = directory.path().join(format!("conversion_{release}.exe"));
            jett_driver::native::build_host_executable_with_options(
                &source, launcher(), &binary, jett_driver::BuildOptions { release },
            ).unwrap();
            binaries.push(binary);
        }
        fs::remove_file(&source).unwrap();
        for binary in binaries {
            let actual = run_bounded(&binary, directory.path());
            assert!(actual.status.success(), "{actual:?}");
            assert_eq!(actual.stdout, expected.stdout.as_bytes());
            assert!(actual.stderr.is_empty(), "{actual:?}");
        }
    }).unwrap().join().unwrap();
}

#[test]
fn native_deep_reflection_dispatch_matches_interpreter_in_both_profiles() {
    // Give the frontend's recursive type walks enough stack for this source.
    // Linked programs still run in separate processes with ordinary stacks.
    std::thread::Builder::new()
        .stack_size(8 * 1024 * 1024)
        .spawn(|| {
    let nested = format!("{}int64{}", "list[".repeat(70), "]".repeat(70));
    let source_text = format!(
        "namespace app\ntype Deep = {nested}\nstruct Record:\n    direct: {nested}\n    aliased: Deep\nfunction main(stdout: Stdout) returns nothing:\n    for field in type.fields[Record]():\n        comptime type Field = field.type_info:\n            Stdout.write(view stdout, \"{{type.kind[Field]()}}:{{type.name[Field]()}}\\n\")\n"
    );
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("deep_reflection.jett");
    fs::write(&source, source_text).unwrap();
    let expected = jett_driver::run_file_capture_output(&source).unwrap();
    assert_eq!(expected.stdout, format!("list:{nested}\nalias:app.Deep\n"));
    let mut binaries = Vec::new();
    for release in [false, true] {
        let binary = directory.path().join(format!("reflection_{release}.exe"));
        jett_driver::native::build_host_executable_with_options(
            &source,
            launcher(),
            &binary,
            jett_driver::BuildOptions { release },
        )
        .unwrap();
        binaries.push(binary);
    }
    fs::remove_file(&source).unwrap();
    for binary in binaries {
        let actual = run_bounded(&binary, directory.path());
        assert!(actual.status.success(), "{actual:?}");
        assert_eq!(actual.stdout, expected.stdout.as_bytes());
        assert!(actual.stderr.is_empty(), "{actual:?}");
    }
        })
        .unwrap()
        .join()
        .unwrap();
}

#[test]
fn native_enum_deep_equality_matches_interpreter_in_both_profiles() {
    let fixture =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/native/enum_deep_equality.jett");
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("deep_equality.jett");
    fs::copy(&fixture, &source).unwrap();
    let expected = jett_driver::run_file_capture_output(&source).unwrap();
    assert_eq!(expected.stdout, "true true false false true false\n");
    let mut binaries = Vec::new();
    for release in [false, true] {
        let binary = directory.path().join(format!("equality_{release}.exe"));
        jett_driver::native::build_host_executable_with_options(
            &source,
            launcher(),
            &binary,
            jett_driver::BuildOptions { release },
        )
        .unwrap();
        binaries.push(binary);
    }
    fs::remove_file(&source).unwrap();
    for binary in binaries {
        let actual = run_bounded(&binary, directory.path());
        assert!(actual.status.success(), "{actual:?}");
        assert_eq!(actual.stdout, expected.stdout.as_bytes());
        assert!(actual.stderr.is_empty(), "{actual:?}");
    }
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
fn native_deep_debug_values_match_interpreter_without_a_fixed_depth_limit() {
    let fixture =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/native/debug_deep_values.jett");
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("deep.jett");
    fs::copy(&fixture, &source).unwrap();
    let expected = jett_driver::run_file_capture_output(&source).unwrap();
    let debug = format!("{}\n", expected.debug_output.join("\n"));
    assert!(expected.stdout.contains("app.Chain.end"));
    assert!(debug.contains("[redacted]"));
    assert!(!debug.contains("deep-secret-token"));
    let binary = directory.path().join("deep.exe");
    build_host_executable(&source, launcher(), &binary).unwrap();
    fs::remove_file(&source).unwrap();
    let actual = run_bounded(&binary, directory.path());
    assert!(actual.status.success(), "{actual:?}");
    assert_eq!(actual.stdout, expected.stdout.as_bytes());
    assert_eq!(actual.stderr, debug.as_bytes());
}

#[test]
fn native_debug_print_argument_failure_produces_no_partial_output() {
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("print_failure.jett");
    fs::write(&source, "namespace app\nfunction failed() returns int64:\n    string rejected = string.repeat(\"ab\", 9223372036854775807)\n    return string.char_count(rejected)\nfunction main() returns nothing:\n    println(list(\"held\"), failed())\n").unwrap();
    let expected = jett_driver::run_file_capture_outcome(&source).unwrap_err();
    assert!(expected.output.stdout.is_empty());
    assert!(expected.message.contains("string.repeat"), "{expected:?}");
    let binary = directory.path().join("print_failure.exe");
    build_host_executable(&source, launcher(), &binary).unwrap();
    fs::remove_file(&source).unwrap();
    let actual = run_bounded(&binary, directory.path());
    assert_eq!(actual.status.code(), Some(71), "{actual:?}");
    assert!(actual.stdout.is_empty());
    assert_eq!(actual.stderr, format!("{}\n", expected.message).as_bytes());
}

#[test]
fn native_debug_prints_aggregate_values_without_consuming_views() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/native");
    for name in [
        "debug_print_values",
        "debug_aggregates",
        "debug_functions",
        "debug_actor_values",
        "debug_type_construction",
    ] {
        let original = fs::read_to_string(root.join(format!("{name}.jett"))).unwrap();
        let source_text = original
            .lines()
            .filter_map(|line| {
                let content = line.trim_start();
                if content.starts_with("breakpoint ") {
                    return None;
                }
                Some(if let Some(value) = content.strip_prefix("trace ") {
                    format!(
                        "{}println(view {value})",
                        &line[..line.len() - content.len()]
                    )
                } else {
                    line.to_owned()
                })
            })
            .collect::<Vec<_>>()
            .join("\n");
        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join(format!("{name}.jett"));
        fs::write(&source, format!("{source_text}\n")).unwrap();
        let expected = jett_driver::run_file_capture_output(&source).unwrap();
        assert!(!expected.stdout.is_empty(), "{name}");
        assert!(expected.debug_output.is_empty(), "{name}");
        let binary = directory.path().join("print.exe");
        build_host_executable(&source, launcher(), &binary).unwrap();
        fs::remove_file(&source).unwrap();
        let actual = run_bounded(&binary, directory.path());
        assert!(actual.status.success(), "{name}: {actual:?}");
        assert_eq!(actual.stdout, expected.stdout.as_bytes(), "{name}");
        assert!(actual.stderr.is_empty(), "{name}: {actual:?}");
    }
}

#[test]
fn native_float_remainder_matches_interpreter_in_both_profiles() {
    let fixture =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/native/float_remainder.jett");
    let expected = jett_driver::run_file_capture_output(&fixture).unwrap();
    assert_eq!(
        expected.stdout,
        concat!(
            "1.5 -1.5 1.5 -1.5\n1.5\n",
            "1.5 -1.5 1.5 -1.5\n1.5\n",
            "true true true true true true\n",
            "true true true true true true\n",
            "1.5 -1.5\ntrue\n",
        )
    );
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("remainder.jett");
    fs::copy(&fixture, &source).unwrap();
    let mut executables = Vec::new();
    for release in [false, true] {
        let binary = directory.path().join(format!("remainder_{release}.exe"));
        jett_driver::native::build_host_executable_with_options(
            &source,
            launcher(),
            &binary,
            jett_driver::BuildOptions { release },
        )
        .unwrap();
        executables.push(binary);
    }
    fs::remove_file(&source).unwrap();
    for executable in executables {
        let actual = run_bounded(&executable, directory.path());
        assert!(actual.status.success(), "{actual:?}");
        assert_eq!(actual.stdout, expected.stdout.as_bytes());
        assert!(actual.stderr.is_empty(), "{actual:?}");
    }
}

#[test]
fn native_float_remainder_rejects_pending_operands_and_cleans_owners() {
    for width in ["float32", "float64"] {
        for (left, right) in [("run run 5.5", "2.0"), ("5.5", "run run 2.0")] {
            let directory = tempfile::tempdir().unwrap();
            let source = directory.path().join("pending_remainder.jett");
            fs::write(&source, format!(
                "namespace app\nfunction main() returns nothing:\n    list[string] owners = list(\"left\", \"right\")\n    {width} left = {left}\n    {width} right = {right}\n    {width} remainder = left modulo right\n"
            )).unwrap();
            let expected = jett_driver::run_file_capture_outcome(&source).unwrap_err();
            assert!(
                expected.message.contains("unsupported binary operation:"),
                "{expected:?}"
            );
            assert!(expected.message.contains("Modulo"), "{expected:?}");
            for release in [false, true] {
                let binary = directory.path().join(format!("pending_{release}.exe"));
                jett_driver::native::build_host_executable_with_options(
                    &source,
                    launcher(),
                    &binary,
                    jett_driver::BuildOptions { release },
                )
                .unwrap();
                let actual = run_bounded(&binary, directory.path());
                assert_eq!(actual.status.code(), Some(71), "{actual:?}");
                assert_eq!(actual.stdout, expected.output.stdout.as_bytes());
                assert_eq!(actual.stderr, format!("{}\n", expected.message).as_bytes());
            }
        }
    }
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
fn native_temporary_projected_views_match_interpreter() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/native/temporary_projected_views.jett");
    let expected = jett_driver::run_file_capture_output(&fixture).expect("temporary view oracle");
    assert_eq!(
        expected.stdout,
        "make:list\nAda\nLin\nmake:set\ntag\nmake:map\nrow owned 7\nmake:string\n🚀\ne\n\u{301}\nmake:record\nAda\nLin\nrow owned\n11\nmake:direct\nmissing:direct\nAda:fallback:2\nmake:indirect\nmissing:indirect\nAda:fallback:2\nmake:early\nmissing:early\nreturned\nmake:break\nAda\nmake:continue\nLin\n"
    );
    let directory = tempfile::tempdir().unwrap();
    let binary = directory.path().join("temporary_projected_views.exe");
    build_host_executable(&fixture, launcher(), &binary).expect("native temporary views");
    let actual = run_bounded(&binary, directory.path());
    assert!(actual.status.success(), "{actual:?}");
    assert_eq!(actual.stdout, expected.stdout.as_bytes());
    assert!(actual.stderr.is_empty(), "{actual:?}");
}

#[test]
fn native_temporary_projected_view_failure_cleans_owners() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/native/temporary_projected_view_failure.jett");
    let expected = jett_driver::run_file_capture_outcome(&fixture)
        .expect_err("later argument fails after borrowing the temporary owner");
    assert_eq!(expected.output.stdout, "owner\nargument\n");
    let directory = tempfile::tempdir().unwrap();
    let binary = directory
        .path()
        .join("temporary_projected_view_failure.exe");
    build_host_executable(&fixture, launcher(), &binary).expect("native temporary view failure");
    let actual = run_bounded(&binary, directory.path());
    assert_eq!(actual.status.code(), Some(71), "{actual:?}");
    assert_eq!(actual.stdout, expected.output.stdout.as_bytes());
    assert_eq!(actual.stderr, format!("{}\n", expected.message).as_bytes());
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
fn native_comptime_machine_values_can_be_evaluated_repeatedly() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/native/comptime_machine_reuse.jett");
    let expected =
        jett_driver::run_file_capture_output(&fixture).expect("repeated comptime oracle");
    assert_eq!(expected.stdout, "active:7\n".repeat(12));
    let directory = tempfile::tempdir().unwrap();
    let binary = directory.path().join("comptime_machine_reuse.exe");
    build_host_executable(&fixture, launcher(), &binary)
        .expect("native repeated comptime machines");
    let actual = run_bounded(&binary, directory.path());
    assert!(actual.status.success(), "{actual:?}");
    assert_eq!(actual.stdout, expected.stdout.as_bytes());
    assert!(actual.stderr.is_empty(), "{actual:?}");
}

#[test]
fn native_comptime_builders_match_interpreter() {
    let fixture =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/native/comptime_builders.jett");
    let expected = jett_driver::run_file_capture_output(&fixture).expect("comptime builder oracle");
    assert_eq!(
        expected.stdout,
        concat!(
            "type.construct_finish: 'app.Record' is missing required field 'count'\n",
            "record:5:runtime\n",
            "type.construct_finish: 'app.Record' is missing required field 'label'\n",
            "refinement type constraint failed for 'app.Positive'\n",
            "record:8:runtime\nrecord:1:runtime\nrecord:2:runtime\nrecord:9:runtime\nbuilder\n",
            "127\nalias\ndeclared-alias\nrecord:12:runtime\nrecord:14:runtime\n7\nheader:3\n",
            "bitfield 'app.Header' field 'kind' is 8 bit(s) wide and cannot hold '256'\n",
        )
    );
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
            .any(|line| line.contains("hidden-baked-builder"))
    );
    let directory = tempfile::tempdir().unwrap();
    let binary = directory.path().join("comptime_builders.exe");
    build_host_executable(&fixture, launcher(), &binary).expect("native comptime builders");
    let actual = run_bounded(&binary, directory.path());
    assert!(actual.status.success(), "{actual:?}");
    assert_eq!(actual.stdout, expected.stdout.as_bytes());
    assert_eq!(
        String::from_utf8_lossy(&actual.stderr),
        format!("{}\n", expected.debug_output.join("\n"))
    );
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
fn native_comptime_generic_closures_match_interpreter() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/native/comptime_generic_closures.jett");
    let expected = jett_driver::run_file_capture_output(&fixture).expect("generic closure oracle");
    assert_eq!(
        expected.stdout,
        "7:2\nname:3\nalias:4\n17\nkept\n29\nnested:5\n23:6\nint64\nLabel\n"
    );
    let directory = tempfile::tempdir().unwrap();
    let binary = directory.path().join("comptime_generic_closures.exe");
    build_host_executable(&fixture, launcher(), &binary).expect("native generic comptime closures");
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

#[test]
fn native_reflected_interface_field_owners_match_interpreter_in_both_profiles() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/native/reflected_interface_field_owners.jett");
    let directory = tempfile::tempdir().unwrap();
    let source = directory
        .path()
        .join("reflected_interface_field_owners.jett");
    fs::copy(&fixture, &source).unwrap();
    let expected = jett_driver::run_file_capture_output(&source)
        .expect("reflected interface field owner oracle");
    assert_eq!(
        expected.stdout,
        concat!(
            "direct:selected:small:7|nested:selected:small:7\n",
            "record:selected:small:7|selected:small:7|small:7;nested:selected:small:7|selected:small:7|nested:selected:small:7|selected:small:7;record:18446744073709551615\n",
            "event:active:selected:small:7|selected:small:7|small:7;nested:selected:small:7|selected:small:7|nested:selected:small:7|selected:small:7\n",
            "mirror-event:mirror:selected:small:9|selected:small:9|small:9;nested:selected:small:9|selected:small:9|nested:selected:small:9|selected:small:9\n",
            "state:active:selected:small:7|selected:small:7|small:7;nested:selected:small:7|selected:small:7|nested:selected:small:7|selected:small:7\n",
            "mirror-state:mirror:selected:small:9|selected:small:9|small:9;nested:selected:small:9|selected:small:9|nested:selected:small:9|selected:small:9\n",
            "baked:\n",
            "record:selected:small:7|selected:small:7|small:7;nested:selected:small:7|selected:small:7|nested:selected:small:7|selected:small:7;record:18446744073709551615\n",
            "event:active:selected:small:7|selected:small:7|small:7;nested:selected:small:7|selected:small:7|nested:selected:small:7|selected:small:7\n",
            "mirror-event:mirror:selected:small:9|selected:small:9|small:9;nested:selected:small:9|selected:small:9|nested:selected:small:9|selected:small:9\n",
            "state:active:selected:small:7|selected:small:7|small:7;nested:selected:small:7|selected:small:7|nested:selected:small:7|selected:small:7\n",
            "mirror-state:mirror:selected:small:9|selected:small:9|small:9;nested:selected:small:9|selected:small:9|nested:selected:small:9|selected:small:9\n",
            "reuse:selected:small:7|selected:small:7|small:7;nested:selected:small:7|selected:small:7|nested:selected:small:7|selected:small:7;record:18446744073709551615=selected:small:7|selected:small:7|small:7;nested:selected:small:7|selected:small:7|nested:selected:small:7|selected:small:7;record:18446744073709551615=selected:small:7|selected:small:7|small:7;nested:selected:small:7|selected:small:7|nested:selected:small:7|selected:small:7;record:18446744073709551615\n",
            "pending:selected:small:7|nested:selected:small:9|nested:selected:small:9\n",
            "secret-direct:selected:small:7\n",
            "secret-reflected:selected:small:7\n",
            "secret-reuse:selected:small:7\n",
        )
    );
    assert_eq!(
        expected.debug_output,
        [
            "trace copied: app.Named = pending(pending(7))",
            "trace once: app.Named = pending(7)",
            "trace copied: app.Named = pending(pending(9))",
            "trace once: app.Named = pending(9)",
            "trace copied: app.Named = pending(pending(9))",
            "trace once: app.Named = pending(9)",
            "trace ordinary: secret[app.Named] = [redacted]",
            "trace reflected: secret[app.Named] = [redacted]",
        ]
    );
    let mut binaries = Vec::new();
    for release in [false, true] {
        let binary = directory.path().join(format!("field_owners_{release}.exe"));
        jett_driver::native::build_host_executable_with_options(
            &source,
            launcher(),
            &binary,
            jett_driver::BuildOptions { release },
        )
        .expect("native reflected interface field owners");
        binaries.push((binary, release));
    }
    let verify_binary = directory.path().join("field_owners_verify.exe");
    build_host_verify_suite_executable(&source, launcher(), &verify_binary)
        .expect("native reflected interface field verify suite");
    let property_binary = directory.path().join("field_owners_property.exe");
    build_host_property_suite_executable(&source, launcher(), &property_binary)
        .expect("native reflected interface field property suite");
    fs::remove_file(&source).unwrap();
    for (binary, release) in binaries {
        let actual = run_bounded(&binary, directory.path());
        assert!(actual.status.success(), "{actual:?}");
        assert_eq!(actual.stdout, expected.stdout.as_bytes());
        let debug = if release {
            String::new()
        } else {
            format!("{}\n", expected.debug_output.join("\n"))
        };
        assert_eq!(actual.stderr, debug.as_bytes(), "{actual:?}");
    }
    for suite in [verify_binary, property_binary] {
        let actual = run_bounded(&suite, directory.path());
        assert!(actual.status.success(), "{actual:?}");
        assert!(actual.stdout.is_empty(), "{actual:?}");
        assert!(actual.stderr.is_empty(), "{actual:?}");
    }
}

#[test]
fn native_reflected_interface_field_failures_match_interpreter_in_both_profiles() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/native/reflected_interface_field_owners.jett");
    let template = fs::read_to_string(&fixture).unwrap();
    let declarations = template.split("function main(").next().unwrap();
    for (operation, message) in [
        (
            "wrong_record_request",
            "type.field_value: field 'item' has type 'app.Selected', requested 'int64'",
        ),
        (
            "wrong_event_member",
            "type.variant_field_value: field metadata belongs to 'app.Event.active', expected 'app.Event.mirror'",
        ),
        (
            "pending_session_metadata",
            concat!(
                "type.machine_field_value: second argument must be TypeField, got pending(pending(",
                "TypeField(index: 1, owner_type: app.Session, owner_member: some(active), name: item, ",
                "type_name: app.Selected, kind: refinement, kind_tag: TypeKind.refinement_type, ",
                "serialize_name: item, has_secret: false, ",
                "type_info: TypeInfo(type_name: app.Selected, kind: refinement, ",
                "kind_tag: TypeKind.refinement_type, primitive_tag: none, has_secret: false, ",
                "args: list(TypeInfo(type_name: app.Named, kind: unknown, kind_tag: TypeKind.unknown_type, ",
                "primitive_tag: none, has_secret: false, args: list()))))))",
            ),
        ),
    ] {
        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join("reflected_interface_failure.jett");
        fs::write(
            &source,
            format!(
                "{declarations}function main(stdout: Stdout) returns nothing:\n    Stdout.write(view stdout, \"before\\n\")\n    string value = {operation}()\n    Stdout.write(view stdout, value)\n    return nothing\n"
            ),
        )
        .unwrap();
        let expected = jett_driver::run_file_capture_outcome(&source)
            .expect_err("invalid reflected request or metadata must fail before dispatch");
        assert_eq!(expected.output.stdout, "before\n", "{operation}");
        assert!(expected.output.debug_output.is_empty(), "{operation}");
        assert_eq!(expected.message, format!("runtime error: {message}"));
        let mut binaries = Vec::new();
        for release in [false, true] {
            let binary = directory
                .path()
                .join(format!("field_failure_{release}.exe"));
            jett_driver::native::build_host_executable_with_options(
                &source,
                launcher(),
                &binary,
                jett_driver::BuildOptions { release },
            )
            .unwrap_or_else(|error| panic!("{operation}, release={release}: {error}"));
            binaries.push(binary);
        }
        fs::remove_file(&source).unwrap();
        for binary in binaries {
            let actual = run_bounded(&binary, directory.path());
            assert_eq!(actual.status.code(), Some(71), "{operation}: {actual:?}");
            assert_eq!(actual.stdout, expected.output.stdout.as_bytes());
            assert_eq!(
                actual.stderr,
                format!("{}\n", expected.message).as_bytes(),
                "{operation}: {actual:?}"
            );
        }
    }
}

#[test]
fn native_interface_method_values_match_interpreter_in_both_profiles() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/native/interface_method_values.jett");
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("interface_method_values.jett");
    fs::copy(&fixture, &source).unwrap();
    let expected =
        jett_driver::run_file_capture_output(&source).expect("interface method value oracle");
    assert_eq!(
        expected.stdout,
        concat!(
            "owners:8:107:18:1008\n",
            "slots:13:112:23:1013\n",
            "coarsen:8\n",
            "routes:8:8:8:8:8:8:8:8\n",
            "adapted:18:18\n",
            "alias:18\n",
            "baked:8:8:8:8:8\n",
            "receiver\nfactory\nordered:8\n",
            "capability:9\n",
            "reuse:9:8\n",
            "collision:8:8:8\n",
        )
    );
    assert_eq!(expected.debug_output.len(), 4);
    for (event, method) in expected.debug_output.iter().zip([
        "callback_library.Reader.read",
        "callback_library.Reader.offset",
        "callback_library.Reader.read",
        "callback_library.Absent.read",
    ]) {
        assert!(event.ends_with(&format!("= function({method})")), "{event}");
    }
    let mut binaries = Vec::new();
    for release in [false, true] {
        let binary = directory
            .path()
            .join(format!("method_values_{release}.exe"));
        jett_driver::native::build_host_executable_with_options(
            &source,
            launcher(),
            &binary,
            jett_driver::BuildOptions { release },
        )
        .expect("native interface method values");
        binaries.push((binary, release));
    }
    let verify_binary = directory.path().join("method_values_verify.exe");
    build_host_verify_suite_executable(&source, launcher(), &verify_binary)
        .expect("native interface method value verify suite");
    let property_binary = directory.path().join("method_values_property.exe");
    build_host_property_suite_executable(&source, launcher(), &property_binary)
        .expect("native interface method value property suite");
    fs::remove_file(&source).unwrap();
    for (binary, release) in binaries {
        let actual = run_bounded(&binary, directory.path());
        assert!(actual.status.success(), "{actual:?}");
        assert_eq!(actual.stdout, expected.stdout.as_bytes());
        let debug = if release {
            String::new()
        } else {
            format!("{}\n", expected.debug_output.join("\n"))
        };
        assert_eq!(actual.stderr, debug.as_bytes(), "{actual:?}");
    }
    for binary in [verify_binary, property_binary] {
        let actual = run_bounded(&binary, directory.path());
        assert!(actual.status.success(), "{actual:?}");
        assert!(actual.stdout.is_empty(), "{actual:?}");
        assert!(actual.stderr.is_empty(), "{actual:?}");
    }
}

#[test]
fn native_interface_method_value_pending_receiver_matches_qualified_call() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/native/interface_method_values.jett");
    let template = fs::read_to_string(&fixture).unwrap();
    let declarations = template.split("function main(").next().unwrap();
    for callee in ["callback", "library.Reader.read"] {
        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join("pending_method_receiver.jett");
        fs::write(
            &source,
            format!(
                "{declarations}function main(stdout: Stdout) returns nothing:\n    use callback_library as library\n    library.Reader source = run 7\n    function(view library.Reader) returns int64 callback = library.Reader.read\n    Stdout.write(view stdout, \"before\\n\")\n    int64 value = {callee}(view source)\n    Stdout.write(view stdout, \"{{value}}\\n\")\n    return nothing\n"
            ),
        )
        .unwrap();
        let expected = jett_driver::run_file_capture_outcome(&source)
            .expect_err("pending receiver must fail before implementation dispatch");
        assert_eq!(expected.output.stdout, "before\n");
        assert!(expected.output.debug_output.is_empty());
        assert_eq!(
            expected.message,
            "runtime error: undefined function 'callback_library.Reader.read'"
        );
        let mut binaries = Vec::new();
        for release in [false, true] {
            let binary = directory
                .path()
                .join(format!("pending_method_{release}.exe"));
            jett_driver::native::build_host_executable_with_options(
                &source,
                launcher(),
                &binary,
                jett_driver::BuildOptions { release },
            )
            .expect("native interface method pending receiver");
            binaries.push(binary);
        }
        fs::remove_file(&source).unwrap();
        for binary in binaries {
            let actual = run_bounded(&binary, directory.path());
            assert_eq!(actual.status.code(), Some(71), "{callee}: {actual:?}");
            assert_eq!(actual.stdout, expected.output.stdout.as_bytes());
            assert_eq!(
                actual.stderr,
                format!("{}\n", expected.message).as_bytes(),
                "{callee}: {actual:?}"
            );
        }
    }
}

#[test]
fn native_borrowed_return_clone_controls_match_interpreter_in_both_profiles() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/native/borrowed_return_clone_controls.jett");
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("borrowed_return_clone_controls.jett");
    fs::copy(&fixture, &source).unwrap();
    let expected = jett_driver::run_file_capture_output(&source)
        .expect("owned clone and implicit-copy return oracle");
    assert_eq!(
        expected.stdout,
        concat!(
            "copies:3:4:3:3:before\n",
            "owned:3:3:3:3:4:3\n",
            "scalars:7:before\n",
            "wrapped:2:2:2:2\n",
            "baked:3:3:3:3:3:4:3:7:seed\n",
        )
    );
    assert!(expected.debug_output.is_empty());
    let mut binaries = Vec::new();
    for release in [false, true] {
        let binary = directory
            .path()
            .join(format!("borrowed_clones_{release}.exe"));
        jett_driver::native::build_host_executable_with_options(
            &source,
            launcher(),
            &binary,
            jett_driver::BuildOptions { release },
        )
        .expect("native owned clone and implicit-copy returns");
        binaries.push(binary);
    }
    let verify_binary = directory.path().join("borrowed_clones_verify.exe");
    build_host_verify_suite_executable(&source, launcher(), &verify_binary)
        .expect("native borrowed clone verify suite");
    let property_binary = directory.path().join("borrowed_clones_property.exe");
    build_host_property_suite_executable(&source, launcher(), &property_binary)
        .expect("native borrowed clone property suite");
    fs::remove_file(&source).unwrap();
    for binary in binaries {
        let actual = run_bounded(&binary, directory.path());
        assert!(actual.status.success(), "{actual:?}");
        assert_eq!(actual.stdout, expected.stdout.as_bytes());
        assert!(actual.stderr.is_empty(), "{actual:?}");
    }
    for binary in [verify_binary, property_binary] {
        let actual = run_bounded(&binary, directory.path());
        assert!(actual.status.success(), "{actual:?}");
        assert!(actual.stdout.is_empty(), "{actual:?}");
        assert!(actual.stderr.is_empty(), "{actual:?}");
    }
}

#[test]
fn native_contextual_secret_constructor_failures_preserve_order_and_cleanup() {
    let declarations = r#"namespace app
function first(view stdout: Stdout) returns list[string]:
    Stdout.write(view stdout, "first\n")
    return list("owned")
function failing(view stdout: Stdout) returns list[string]:
    Stdout.write(view stdout, "failure\n")
    return list.remove_at[string](list("other", "owner"), -1)
"#;
    for (name, ty, constructor, output) in [
        (
            "list",
            "secret[list[list[string]]]",
            "list(first(view stdout), failing(view stdout))",
            "before\nfirst\nfailure\n",
        ),
        (
            "map",
            "secret[map[string, list[string]]]",
            "map(\"first\": first(view stdout), \"second\": failing(view stdout))",
            "before\nfirst\nfailure\n",
        ),
        (
            "some",
            "secret[optional[list[string]]]",
            "some(failing(view stdout))",
            "before\nfailure\n",
        ),
        (
            "ok",
            "secret[result[list[string], string]]",
            "ok(failing(view stdout))",
            "before\nfailure\n",
        ),
        (
            "fail",
            "secret[result[int64, list[string]]]",
            "fail(failing(view stdout))",
            "before\nfailure\n",
        ),
    ] {
        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join("main.jett");
        fs::write(
            &source,
            format!(
                "{declarations}function main(stdout: Stdout) returns nothing:\n    Stdout.write(view stdout, \"before\\n\")\n    {ty} hidden = {constructor}\n    Stdout.write(view stdout, \"must not run\")\n"
            ),
        )
        .unwrap();
        let expected = jett_driver::run_file_capture_outcome(&source)
            .expect_err("payload fails before the secret constructor completes");
        assert_eq!(expected.output.stdout, output, "{name}: {expected:?}");
        assert!(expected.output.debug_output.is_empty());
        let mut binaries = Vec::new();
        for release in [false, true] {
            let binary = directory
                .path()
                .join(format!("secret_{name}_{release}.exe"));
            jett_driver::native::build_host_executable_with_options(
                &source,
                launcher(),
                &binary,
                jett_driver::BuildOptions { release },
            )
            .expect("native contextual secret constructor with terminal payload failure");
            binaries.push(binary);
        }
        fs::remove_file(&source).unwrap();
        for binary in binaries {
            let actual = run_bounded(&binary, directory.path());
            assert_eq!(actual.status.code(), Some(71), "{name}: {actual:?}");
            assert_eq!(actual.stdout, output.as_bytes(), "{name}: {actual:?}");
            assert_eq!(
                actual.stderr,
                format!("{}\n", expected.message).as_bytes(),
                "{name}: {actual:?}"
            );
        }
    }
}

#[test]
fn native_contextual_secret_constructors_match_interpreter_in_both_profiles() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/native/contextual_secret_constructors.jett");
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("main.jett");
    fs::copy(&fixture, &source).unwrap();
    let expected = jett_driver::run_file_capture_output(&source)
        .expect("contextual secret constructor reference oracle");
    assert_eq!(
        expected.stdout,
        concat!(
            "lists:list=3:-128:127:-1;empty=0\n",
            "maps:map=2:127:-8;empty=0\n",
            "sums:some:-128:none:ok:127:fail:-128\n",
            "nested:twice=2:7:-8;children=7,-8,\n",
            "owners:selected:small:7;record:8;|some:selected:small:9|ok:record:10|fail:selected:small:11|selected:small:12:record:13\n",
            "children:refined=7,-8,;callbacks=-128:7\n",
            "contexts:list=2:-8:7;empty=0;some:-128;fail:-8;map=-8;inline=some:127\n",
            "baked:list=2:-128:127;some:-8;owners=selected:small:7;record:8;\n",
            "observed:2:7:-8\n",
        )
    );
    assert_eq!(
        expected.debug_output,
        ["trace observed: secret[list[int8]] = [redacted]"]
    );
    let mut binaries = Vec::new();
    for release in [false, true] {
        let binary = directory
            .path()
            .join(format!("secret_values_{release}.exe"));
        jett_driver::native::build_host_executable_with_options(
            &source,
            launcher(),
            &binary,
            jett_driver::BuildOptions { release },
        )
        .expect("native contextual secret constructors");
        binaries.push((binary, release));
    }
    let verify_binary = directory.path().join("secret_values_verify.exe");
    build_host_verify_suite_executable(&source, launcher(), &verify_binary)
        .expect("compiled secret constructor verify suite");
    let property_binary = directory.path().join("secret_values_property.exe");
    build_host_property_suite_executable(&source, launcher(), &property_binary)
        .expect("compiled secret constructor property suite");
    fs::remove_file(&source).unwrap();
    for (binary, release) in binaries {
        let actual = run_bounded(&binary, directory.path());
        assert!(actual.status.success(), "{actual:?}");
        assert_eq!(actual.stdout, expected.stdout.as_bytes());
        let debug = if release {
            String::new()
        } else {
            format!("{}\n", expected.debug_output.join("\n"))
        };
        assert_eq!(actual.stderr, debug.as_bytes());
    }
    for binary in [verify_binary, property_binary] {
        let actual = run_bounded(&binary, directory.path());
        assert!(actual.status.success(), "{actual:?}");
        assert!(actual.stdout.is_empty(), "{actual:?}");
        assert!(actual.stderr.is_empty(), "{actual:?}");
    }
}
