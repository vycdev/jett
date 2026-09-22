//! Production native execution, without the source tree as runtime cwd.
#![cfg(all(target_os = "linux", target_env = "gnu", target_arch = "x86_64"))]

use jett_driver::native::{NativeLauncherBundle, build_host_executable};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;

fn launcher() -> NativeLauncherBundle {
    static ARCHIVE: OnceLock<PathBuf> = OnceLock::new();
    let archive = ARCHIVE.get_or_init(|| {
        let executable = std::env::current_exe().expect("test executable");
        let profile = executable.parent().unwrap().parent().unwrap();
        let target = profile.parent().unwrap().join("native-values-launcher");
        let debug = target.join("debug");
        let status = Command::new(env!("CARGO"))
            .args(["build", "-q", "-p", "jett_native_launcher", "--target-dir"])
            .arg(&target)
            .current_dir(Path::new(env!("CARGO_MANIFEST_DIR")).join("../.."))
            .env("CARGO_BUILD_JOBS", "2")
            .status()
            .expect("build target-matched launcher");
        assert!(status.success(), "launcher build failed: {status}");
        let archive = debug.join("libjett_native_launcher.a");
        assert!(archive.is_file(), "missing archive: {}", archive.display());
        archive
    });
    NativeLauncherBundle::linux_gnu_v1(archive.clone())
}

fn run_bounded(executable: &Path, directory: &Path) -> std::process::Output {
    use std::process::Stdio;
    use std::time::{Duration, Instant};
    let mut child = Command::new(executable)
        .current_dir(directory)
        .env_clear()
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("execute native artifact");
    // Drain concurrently so output larger than a pipe does not deadlock.
    use std::io::Read;
    let mut stdout = child.stdout.take().unwrap();
    let mut stderr = child.stderr.take().unwrap();
    let out = std::thread::spawn(move || {
        let mut b = Vec::new();
        stdout.read_to_end(&mut b).unwrap();
        b
    });
    let err = std::thread::spawn(move || {
        let mut b = Vec::new();
        stderr.read_to_end(&mut b).unwrap();
        b
    });
    let start = Instant::now();
    let status = loop {
        if let Some(status) = child.try_wait().expect("poll native artifact") {
            break status;
        }
        if start.elapsed() > Duration::from_secs(10) {
            child.kill().expect("terminate stuck native artifact");
            child.wait().expect("reap stuck native artifact");
            panic!("native executable exceeded 10 second deadline");
        }
        std::thread::sleep(Duration::from_millis(10));
    };
    std::process::Output {
        status,
        stdout: out.join().unwrap(),
        stderr: err.join().unwrap(),
    }
}

#[test]
fn native_string_fixtures_match_interpreter_output() {
    for name in ["hello_print", "escape_sequences"] {
        let fixture =
            Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("../../tests/run_pass/{name}.jett"));
        let expected = jett_driver::run_file_capture_output(&fixture).unwrap();
        assert!(!expected.stdout.is_empty());
        let directory = tempfile::tempdir().unwrap();
        let binary = directory.path().join("program");
        build_host_executable(&fixture, &launcher(), &binary)
            .unwrap_or_else(|error| panic!("{name}: {error:?}"));
        let actual = run_bounded(&binary, directory.path());
        assert!(actual.status.success(), "{name}: {actual:?}");
        assert_eq!(actual.stdout, expected.stdout.as_bytes(), "{name}");
        assert!(actual.stderr.is_empty(), "{name}: {actual:?}");
    }
}

fn run_source(source: &str) -> std::process::Output {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("values.jett");
    std::fs::write(&path, source).unwrap();
    let expected = jett_driver::run_file_capture_output(&path).expect("interpreter oracle");
    let binary = directory.path().join("program");
    build_host_executable(&path, &launcher(), &binary).expect("native compilation");
    std::fs::remove_file(path).unwrap();
    let actual = run_bounded(&binary, directory.path());
    assert!(actual.status.success(), "{actual:?}");
    assert_eq!(actual.stdout, expected.stdout.as_bytes());
    assert!(actual.stderr.is_empty(), "{actual:?}");
    actual
}

#[test]
fn native_strings_follow_computed_control_flow_and_release_aliases() {
    let output = run_source(
        r#"
function label(value: int64) returns string:
    string prefix = "item"
    if value == 2:
        return "{prefix}:{value * 3}"
    return "{prefix}{value}"
function main(stdout: Stdout) returns nothing:
    mutable string current = "start"
    mutable int64 index = 0
    while index < 4:
        string alias = current
        current = label(index)
        index = index + 1
        if index == 2:
            continue
        Stdout.write(view stdout, "{alias}->{current};")
    if current == "item3" && label(2) != "wrong":
        println(current, index * 7, true, 18446744073709551615)
    println(1.25, false, nothing)
"#,
    );
    assert_eq!(output.stdout, b"start->item0;item1->item:6;item:6->item3;item3 28 true 18446744073709551615\n1.25 false nothing\n");
}

#[test]
fn native_print_preserves_lexical_argument_effects_and_short_circuiting() {
    let output = run_source(
        r#"
function marked(value: int64) returns string:
    print(value)
    return "v{value}"
function unused() returns bool:
    println("must not execute")
    return true
function main() returns nothing:
    bool first = false && unused()
    bool second = true || unused()
    println(marked(1), marked(2), first, second)
"#,
    );
    assert_eq!(output.stdout, b"12v1 v2 false true\n");
}

#[test]
fn native_terminal_failure_unwinds_compiled_string_frames() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/runtime_fail/string_repeat_capacity_overflow.jett");
    let directory = tempfile::tempdir().unwrap();
    let nested = directory.path().join("nested.jett");
    std::fs::write(
        &nested,
        r#"
function explode(value: string) returns string:
    string alias = value
    print("before:", alias)
    return "left{string.repeat(value, 9223372036854775807)}right"
function relay(value: string) returns string:
    return explode(value)
function main() returns nothing:
    string keep = "keep"
    println("left", relay("ab"), keep)
    println("must not execute")
"#,
    )
    .unwrap();
    for path in [&fixture, &nested] {
        let expected =
            jett_driver::run_file_capture_outcome(path).expect_err("terminal failure oracle");
        let binary = directory.path().join("program");
        build_host_executable(path, &launcher(), &binary).expect("compile terminal failure");
        let actual = run_bounded(&binary, directory.path());
        assert_eq!(actual.status.code(), Some(71), "{actual:?}");
        assert_eq!(actual.stdout, expected.output.stdout.as_bytes());
        assert_eq!(
            String::from_utf8(actual.stderr).unwrap(),
            format!("{}\n", expected.message)
        );
    }
}

#[test]
fn native_unicode_leaf_operations_preserve_graphemes() {
    let output = run_source(
        r#"
function main() returns nothing:
    string text = "é🇷🇴x"
    println(string.char_count(text), string.slice(text, 0, 1), string.slice(text, 1, 2))
    println(string.slice(text, -10, 20), string.slice(text, 2, 1))
    println(string.upper("Straße"), string.lower("İ"), string.trim("  hi  "))
    println(string.trim_start("  a  "), string.trim_end("  b  "))
    println(string.repeat("é", 3), string.repeat("x", -1), string.repeat("", 9223372036854775807))
    println(string.is_alpha("é"), string.is_numeric("１２"), string.is_numeric("123"))
    println(string.center("hi", 7), string.zfill("-42", 6))
"#,
    );
    assert!(
        String::from_utf8(output.stdout)
            .unwrap()
            .starts_with("3 é 🇷🇴\n")
    );
}

#[test]
fn native_proven_fixture_outputs_remain_monotonic() {
    for name in [
        "tests/run_pass/escape_sequences.jett",
        "tests/run_pass/explicit_comptime_expression.jett",
        "tests/run_pass/hello_print.jett",
        "tests/run_pass/named_argument_runtime_order.jett",
        "tests/run_pass/namespace_runtime_main_context.jett",
        "tests/run_pass/native_scalar_entry.jett",
        "tests/run_pass/stdlib_loading.jett",
        "tests/run_pass/string_interpolation.jett",
        "tests/runtime_fail/int16_return_overflow.jett",
        "tests/runtime_fail/int32_assignment_underflow.jett",
        "tests/runtime_fail/int8_expression_underflow.jett",
        "tests/runtime_fail/uint16_parameter_overflow.jett",
        "tests/runtime_fail/uint32_multiplication_overflow.jett",
        "tests/runtime_fail/uint32_nested_expression_overflow.jett",
        "tests/runtime_fail/uint8_expression_overflow.jett",
    ] {
        let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join(name);
        let expected = jett_driver::run_file_capture_output(&fixture).unwrap();
        let directory = tempfile::tempdir().unwrap();
        let binary = directory.path().join("program");
        build_host_executable(&fixture, &launcher(), &binary)
            .unwrap_or_else(|error| panic!("{name}: {error}"));
        let actual = run_bounded(&binary, directory.path());
        assert!(actual.status.success(), "{name}: {actual:?}");
        assert_eq!(actual.stdout, expected.stdout.as_bytes(), "{name}");
        assert!(actual.stderr.is_empty(), "{name}: {actual:?}");
    }
}
