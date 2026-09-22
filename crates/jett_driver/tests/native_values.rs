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
            .env("CARGO_BUILD_JOBS", "1")
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

#[test]
fn native_numeric_intrinsics_match_wrapping_and_ieee_contracts() {
    let output = run_source(
        r#"
function main() returns nothing:
    int64 minimum = -9223372036854775807 - 1
    println(math.abs(minimum), math.gcd(minimum, 0), math.lcm(minimum, 1))
    println(math.factorial(21), math.mod(minimum, -1), math.lcm(3037000500, 3037000501))
    println(math.min(4, -3), math.max(-4, 3), math.abs(-2.5))
    println(math.sqrt(9.0), math.pow(2.0, 5.0), math.round(-1.5), math.floor(1.9), math.ceil(1.1))
    println(math.log(1.0), math.log2(8.0), math.log10(100.0), math.clamp(3.0, 0.0, 2.0))
    println(math.sin(0.0), math.cos(0.0), math.tan(0.0), math.pi(), math.e())
    float64 nan = math.sqrt(-1.0)
    println(math.min(nan, 1.0), math.max(1.0, nan), nan == nan)
"#,
    );
    assert!(
        String::from_utf8(output.stdout)
            .unwrap()
            .contains("-9223372036854775808 -9223372036854775808 -9223372036854775808\n")
    );
}

#[test]
fn native_math_runtime_contract_fixtures() {
    for name in [
        "math_factorial_overflow",
        "math_mod_overflow",
        "math_lcm_overflow",
        "math_clamp_nan_bound",
        "math_clamp_nan_upper_bound",
        "math_clamp_reversed_float_bounds",
    ] {
        let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join(format!("../../tests/runtime_fail/{name}.jett"));
        let expected = jett_driver::run_file_capture_outcome(&fixture);
        let directory = tempfile::tempdir().unwrap();
        let binary = directory.path().join("program");
        build_host_executable(&fixture, &launcher(), &binary)
            .unwrap_or_else(|e| panic!("{name}: {e}"));
        let actual = run_bounded(&binary, directory.path());
        match expected {
            Ok(output) => {
                assert!(actual.status.success(), "{name}: {actual:?}");
                assert_eq!(actual.stdout, output.stdout.as_bytes());
                assert!(actual.stderr.is_empty());
            }
            Err(failure) => {
                assert_eq!(actual.status.code(), Some(71), "{name}: {actual:?}");
                assert_eq!(actual.stdout, failure.output.stdout.as_bytes());
                assert_eq!(
                    String::from_utf8(actual.stderr).unwrap(),
                    format!("{}\n", failure.message)
                );
            }
        }
    }
}

#[test]
fn native_adjacent_scalar_interpolation_has_structural_capacity() {
    let output = run_source(
        r#"
function render(value: int64) returns string:
    return "{value}{value}{value}{value}{value}{value}"
function main() returns nothing:
    println(render(7))
"#,
    );
    assert_eq!(output.stdout, b"777777\n");
}

#[test]
fn native_long_interpolation_calls_print_and_cleanup() {
    let scalars = "{value}".repeat(64);
    let strings = "{text}".repeat(64);
    let source = format!(
        r#"
function render(value: int64, text: string) returns string:
    return "{scalars}{strings}"
function nested(value: int64) returns string:
    return "[{{"{{render(value, "x")}}"}}]"
function main() returns nothing:
    mutable int64 index = 0
    while index < 3:
        string text = nested(index)
        string alias = text
        if text == alias && text != "":
            print(text)
        index = index + 1
    println(nested(7), 1, 2, 3, 4, 5, 6, true, nothing)
"#
    );
    let output = run_source(&source);
    let mut expected = String::new();
    for value in [0, 1, 2, 7] {
        expected.push_str(&format!(
            "[{}{}]",
            value.to_string().repeat(64),
            "x".repeat(64)
        ));
    }
    expected.push_str(" 1 2 3 4 5 6 true nothing\n");
    assert_eq!(output.stdout, expected.as_bytes());
}

#[test]
fn native_float32_formatting_matches_typed_oracle() {
    let output = run_source(
        r#"
function compute(value: float32) returns float32:
    return (value + 1.0) - 1.0
function literal() returns float32:
    return 0.1
function main() returns nothing:
    mutable float32 value = 0.1
    println(value, "{value}")
    value = 0.2
    println(value, compute(0.1), literal())
    float32 baked = comptime compute(0.1)
    float32 computed = compute(0.1)
    string text = "{computed}"
    if text == "{baked}" && text != "0.1":
        println("rounded", text)
    else:
        println("wrong precision")
    float64 wide = 0.1
    println(wide)
"#,
    );
    assert_eq!(output.stdout, b"0.10000000149011612 0.10000000149011612\n0.20000000298023224 0.10000002384185791 0.10000000149011612\nrounded 0.10000002384185791\n0.1\n");
}

#[test]
fn native_float32_arithmetic_and_ieee_edges_match_oracle() {
    let output = run_source(
        r#"
function arithmetic(value: float32) returns float32:
    return (value * value + value) / 0.3
function cancellation(value: float32) returns float32:
    return (value + 1.0) - value
function main() returns nothing:
    println(arithmetic(0.1), cancellation(16777216.0))
    float32 rounded_literal = 16777217.0
    println(rounded_literal - 16777216.0)
    float32 huge = 340282346638528859811704183484516925440.0
    float32 tiny = 0.0000000000000000000000000000000000000000000014
    float32 zero = 0.0
    float32 negative = -zero
    float32 infinity = huge * 2.0
    float32 nan = zero / zero
    println(infinity, -infinity, tiny / 2.0, negative, nan == nan)
    string text = "{infinity} {negative}"
    if text == "inf -0":
        println("ieee")
"#,
    );
    let value = 0.1_f32;
    let arithmetic = ((value * value + value) / 0.3_f32) as f64;
    assert_eq!(
        output.stdout,
        format!("{arithmetic} 0\n0\ninf -inf 0 -0 false\nieee\n").as_bytes()
    );
}

#[test]
fn native_interpolation_ownership_bound_counts_scalar_segments() {
    let output = run_source(
        r#"
function compose(value: int64) returns string:
    return "{value}{value + 1}{value + 2}{value + 3}{value + 4}{value + 5}{value + 6}{value + 7}{value + 8}{value + 9}{value + 10}{value + 11}"
function main() returns nothing:
    println(compose(1))
"#,
    );
    assert_eq!(output.stdout, b"123456789101112\n");
}

#[test]
fn native_owned_bytes_move_borrow_clone_and_loop() {
    let fixture =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/native/bytes_ownership.jett");
    let expected = jett_driver::run_file_capture_output(&fixture).expect("bytes oracle");
    assert_eq!(expected.stdout.as_bytes(), b"2 486921\n787878 2\n");
    let directory = tempfile::tempdir().unwrap();
    let binary = directory.path().join("program");
    build_host_executable(&fixture, &launcher(), &binary).expect("bytes native compilation");
    let actual = run_bounded(&binary, directory.path());
    assert!(actual.status.success(), "{actual:?}");
    assert_eq!(actual.stdout, expected.stdout.as_bytes());
    assert!(actual.stderr.is_empty(), "{actual:?}");
}

#[test]
fn native_bytes_cleanup_across_early_returns_and_short_circuit() {
    let output = run_source(
        r#"
function select(value: bytes, stop: bool) returns bytes:
    bytes unused = bytes.from_string("discard")
    if stop:
        return value
    return bytes.concat(value, bytes.from_string("!"))
function consume(value: bytes) returns bool:
    println(bytes.to_hex(view value))
    return true
function main() returns nothing:
    bytes a = select(bytes.from_string("a"), true)
    bytes b = select(bytes.from_string("b"), false)
    bool no = false && consume(bytes.from_string("no"))
    bool yes = true || consume(bytes.from_string("no"))
    println(bytes.to_hex(a), bytes.to_hex(b), no, yes)
    mutable bytes current = bytes.new()
    mutable int64 index = 0
    while index < 4:
        current = bytes.from_string("{index}")
        index = index + 1
        if index == 2:
            continue
        if index == 3:
            break
    println(bytes.to_hex(current))
"#,
    );
    assert_eq!(output.stdout, b"61 6221 false true\n32\n");
}

#[test]
fn native_terminal_failure_drops_bytes_in_nested_frames_and_arguments() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("failure.jett");
    std::fs::write(
        &path,
        r#"
function explode(value: bytes) returns string:
    bytes other = clone value
    print(bytes.to_hex(view value))
    return string.repeat("ab", 9223372036854775807)
function relay(value: bytes) returns string:
    bytes local = bytes.from_string("held")
    return explode(value)
function later(first: bytes, second: string) returns nothing:
    println("unreachable")
function main() returns nothing:
    bytes owner = bytes.from_string("x")
    later(bytes.from_string("temporary"), relay(owner))
"#,
    )
    .unwrap();
    let expected = jett_driver::run_file_capture_outcome(&path).expect_err("terminal oracle");
    let binary = directory.path().join("program");
    build_host_executable(&path, &launcher(), &binary).expect("compile byte cleanup");
    let actual = run_bounded(&binary, directory.path());
    assert_eq!(actual.status.code(), Some(71), "{actual:?}");
    assert_eq!(actual.stdout, expected.output.stdout.as_bytes());
    assert_eq!(actual.stderr, format!("{}\n", expected.message).as_bytes());
}

#[test]
fn native_bytes_result_optional_fixture_functions_execute() {
    let fixture =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/run_pass/bytes_operations.jett");
    let mut source = std::fs::read_to_string(fixture).unwrap();
    source.push_str(
        r#"
function main() returns nothing:
    println(new_bytes_test(), from_string_length_test(), to_string_test())
    println(get_valid_test(), get_oob_test(), get_negative_test())
    println(slice_test(), slice_content_test(), concat_test(), concat_length_test())
    println(slice_clamp_test(), roundtrip_test(), to_hex_test())
    println(from_hex_length_test(), from_hex_prefix_test())
    println(from_hex_invalid_message(), from_hex_odd_message(), concat_empty_test())
    println(observer_views_preserve_owner(), explicit_clone_preserves_original())
"#,
    );
    let actual = run_source(&source);
    assert!(actual.stdout.starts_with(b"0 5 hello\n65 -1 -1\n"));
}

#[test]
fn native_nested_owned_sum_payloads_and_handler_exits() {
    let actual = run_source(
        r#"
function wrap(value: bytes) returns result[optional[bytes], string]:
    return ok(some(value))
function unwrap(value: result[optional[bytes], string]) returns bytes:
    optional[bytes] inner = value handle error:
        return bytes.from_string(error)
    bytes output = inner handle:
        default bytes.from_string("none")
    return output
function main() returns nothing:
    result[optional[bytes], string] nested = wrap(bytes.from_string("yes"))
    result[optional[bytes], string] duplicate = clone nested
    bytes first = unwrap(nested)
    bytes second = unwrap(duplicate)
    bytes failed = unwrap(fail("bad"))
    bytes absent = unwrap(ok(none))
    println(bytes.to_hex(first), bytes.to_hex(second), bytes.to_hex(failed), bytes.to_hex(absent))
"#,
    );
    assert_eq!(actual.stdout, b"796573 796573 626164 6e6f6e65\n");
}

#[test]
fn native_handler_backedges_defaults_and_binary_byte_errors() {
    let actual = run_source(
        r#"
function classify(raw: string) returns string:
    bytes data = bytes.from_hex(raw) handle error:
        return error
    string decoded = bytes.to_string(data) handle error:
        return error
    return decoded
function main() returns nothing:
    println(classify("0X41"), classify("+1"), classify("ff"), classify("00"))
    mutable int64 index = 0
    mutable optional[bytes] held = some(bytes.from_string("old"))
    while index < 4:
        index = index + 1
        held = some(bytes.from_string("{index}"))
        bytes data = bytes.from_hex("zz") handle error:
            if index == 2:
                continue
            if index == 3:
                break
            default bytes.from_string(error)
        println(bytes.length(data))
    bytes last = held handle:
        default bytes.new()
    println(bytes.to_hex(last))
"#,
    );
    assert!(actual.stdout.ends_with(b"35\n33\n"));
}

#[test]
fn native_numeric_result_parsing_and_original_minimum_contracts() {
    for name in [
        "math_abs_int64_min",
        "math_gcd_int64_min",
        "math_lcm_int64_min",
    ] {
        let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join(format!("../../tests/runtime_fail/{name}.jett"));
        let expected = jett_driver::run_file_capture_output(&fixture).unwrap();
        let directory = tempfile::tempdir().unwrap();
        let binary = directory.path().join("program");
        build_host_executable(&fixture, &launcher(), &binary).expect("numeric result fixture");
        let actual = run_bounded(&binary, directory.path());
        assert!(actual.status.success(), "{actual:?}");
        assert_eq!(actual.stdout, expected.stdout.as_bytes());
        assert!(actual.stderr.is_empty());
    }
    run_source(
        r#"
function number(raw: string) returns string:
    int64 parsed = int64.from_string(raw) handle error:
        return error
    return "{parsed}"
function main() returns nothing:
    println(number("-9223372036854775808"), number("wrong"))
    uint64 large = uint64.from_string("18446744073709551615") handle error:
        default 0
    float64 real = float64.from_string("-0") handle error:
        default 1.0
    println(large, real)
"#,
    );
}

#[test]
fn native_terminal_failure_bypasses_result_handler_with_live_nested_owners() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("failure.jett");
    std::fs::write(
        &path,
        r#"
function explode(value: bytes) returns result[bytes, string]:
    string doomed = string.repeat("ab", 9223372036854775807)
    return ok(bytes.concat(value, bytes.from_string(doomed)))
function relay(value: bytes) returns result[bytes, string]:
    optional[bytes] held = some(bytes.from_string("held"))
    result[bytes, string] outcome = explode(value)
    bytes after = held handle:
        default bytes.new()
    print(bytes.length(after))
    return outcome
function main() returns nothing:
    result[optional[bytes], string] held = ok(some(bytes.from_string("outer")))
    bytes output = relay(bytes.from_string("input")) handle error:
        println("must not catch terminal failure")
        default bytes.from_string(error)
    optional[bytes] nested = held handle error:
        return nothing
    print(bytes.length(output))
"#,
    )
    .unwrap();
    let expected = jett_driver::run_file_capture_outcome(&path).expect_err("terminal oracle");
    let binary = directory.path().join("program");
    build_host_executable(&path, &launcher(), &binary).expect("sum cleanup compilation");
    let actual = run_bounded(&binary, directory.path());
    assert_eq!(actual.status.code(), Some(71), "{actual:?}");
    assert_eq!(actual.stdout, expected.output.stdout.as_bytes());
    assert_eq!(actual.stderr, format!("{}\n", expected.message).as_bytes());
}

#[test]
fn native_linear_argument_order_and_conditional_moves() {
    let actual = run_source(
        r#"
function observed(view value: bytes) returns int64:
    print("observe:", bytes.to_hex(view value), ";")
    return bytes.length(view value)
function marked(value: bytes) returns bytes:
    print("mark:", bytes.to_hex(view value), ";")
    return value
function pair(first: int64, second: bytes) returns nothing:
    println(first, bytes.to_hex(view second))
function shared(view first: bytes, second: bytes) returns nothing:
    println(bytes.to_hex(view first), bytes.to_hex(view second))
function consumed(value: bytes) returns bool:
    println("consume", bytes.to_hex(view value))
    return true
function branch(flag: bool) returns nothing:
    bytes owner = bytes.from_string("x")
    bool used = flag && consumed(owner)
    println(used)
function main() returns nothing:
    bytes owner = bytes.from_string("ab")
    shared(second: marked(clone owner), first: view owner)
    pair(first: observed(view owner), second: owner)
    bytes a = bytes.from_string("a")
    bytes b = bytes.from_string("b")
    pair(second: marked(b), first: observed(view a))
    branch(false)
    branch(true)
"#,
    );
    assert_eq!(actual.stdout, b"mark: 6162 ;6162 6162\nobserve: 6162 ;2 6162\nmark: 62 ;observe: 61 ;1 62\nfalse\nconsume 78\ntrue\n");
}

#[test]
fn native_list_access_fixture_executes_compiled_stdlib_control_flow() {
    let fixture =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/run_pass/list_access_source.jett");
    let mut source = std::fs::read_to_string(fixture).unwrap();
    source.push_str(r#"
function main() returns nothing:
    println(inferred_first_reuses_list(), inferred_first_view_parameter(), inferred_pipeline_first_reuses_list())
    println(explicit_last_reuses_list(), empty_first_and_last(), is_empty_outcomes_reuse_lists())
"#);
    let actual = run_source(&source);
    assert_eq!(actual.stdout, b"13 42 62\n33 true true\n");
}

#[test]
fn native_nested_lists_clone_owned_elements_and_run_stdlib_loops() {
    let actual = run_source(
        r#"
function main() returns nothing:
    bytes data = bytes.from_string("x")
    list[bytes] originals = list(data)
    list[bytes] copies = clone originals
    list[bytes] extended = list.append[bytes](copies, bytes.from_string("y"))
    list[bytes] reversed = list.reverse[bytes](extended)
    bytes first = list.first[bytes](view reversed) handle:
        default bytes.new()
    bytes original = list.first[bytes](view originals) handle:
        default bytes.new()
    println(bytes.to_hex(first), bytes.to_hex(original), list.length[bytes](view reversed))
    list[list[bytes]] nested = list(originals, reversed)
    list[list[bytes]] duplicate = clone nested
    list[bytes] inner = list.last[list[bytes]](view duplicate) handle:
        default list.new[bytes]()
    bytes last = list.last[bytes](view inner) handle:
        default bytes.new()
    println(bytes.to_hex(last), list.length[list[bytes]](view nested))
    list[string] repeated = list.repeat[string]("held", 3)
    string text = list.last[string](view repeated) handle:
        default "empty"
    float64 number = list.first[float64](list(-0.0, 2.5)) handle:
        default 1.0
    println(text, list.length[string](view repeated), number)
"#,
    );
    assert_eq!(actual.stdout, b"79 78 2\n78 2\nheld 3 -0\n");
}
#[test]
fn native_partial_list_construction_and_nested_owner_failure_cleanup() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("failure.jett");
    std::fs::write(&path, r#"
function explode(view held: list[optional[bytes]]) returns list[bytes]:
    return list(bytes.from_string("first"), bytes.from_string(string.repeat("ab", 9223372036854775807)), bytes.from_string("{list.length[optional[bytes]](view held)}"))
function main() returns nothing:
    list[optional[bytes]] held = list(some(bytes.from_string("held")))
    list[bytes] output = explode(view held)
    println(list.length[bytes](view output), list.length[optional[bytes]](view held))
"#).unwrap();
    let expected = jett_driver::run_file_capture_outcome(&path).expect_err("terminal oracle");
    let binary = directory.path().join("program");
    build_host_executable(&path, &launcher(), &binary).expect("partial list compilation");
    let actual = run_bounded(&binary, directory.path());
    assert_eq!(actual.status.code(), Some(71), "{actual:?}");
    assert_eq!(actual.stdout, expected.output.stdout.as_bytes());
    assert_eq!(actual.stderr, format!("{}\n", expected.message).as_bytes());
}

#[test]
fn native_list_sum_wrapping_contracts_and_observable_results() {
    for name in ["list_sum_overflow", "math_sum_overflow"] {
        let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join(format!("../../tests/runtime_fail/{name}.jett"));
        let expected = jett_driver::run_file_capture_output(&fixture).unwrap();
        let directory = tempfile::tempdir().unwrap();
        let binary = directory.path().join("program");
        build_host_executable(&fixture, &launcher(), &binary).expect("sum fixture compilation");
        let actual = run_bounded(&binary, directory.path());
        assert!(actual.status.success(), "{actual:?}");
        assert_eq!(actual.stdout, expected.stdout.as_bytes());
        assert!(actual.stderr.is_empty());
    }
    let actual = run_source(
        r#"
function main() returns nothing:
    list[int64] items = list(9223372036854775807, 1)
    println(list.sum[int64](view items), list.length[int64](view items))
    println(math.sum(list(9223372036854775807, 2)))
"#,
    );
    assert_eq!(
        actual.stdout,
        b"-9223372036854775808 2\n-9223372036854775807\n"
    );
}

#[test]
fn native_list_iteration_evaluates_once_and_bounds_nested_loans() {
    let actual = run_source(
        r#"
function produced() returns list[string]:
    print("once;")
    return list("a", "b", "c")
function main() returns nothing:
    for value in produced():
        if value == "b":
            continue
        print(value)
    println(";")
    list[int64] original = list(1, 2, 3)
    mutable int64 total = 0
    for item in view original:
        for inner in view original:
            total = total + item * inner
            if inner == 2:
                break
    list[int64] moved = original
    println(total, list.length[int64](view moved))
"#,
    );
    assert_eq!(actual.stdout, b"once;ac;\n18 3\n");
}

#[test]
fn native_consuming_iteration_moves_nested_elements_and_drops_remainders() {
    let actual = run_source(
        r#"
function first(items: list[bytes]) returns bytes:
    for item in items:
        return item
    return bytes.new()
function main() returns nothing:
    list[list[bytes]] nested = list(list(bytes.from_string("a"), bytes.from_string("b")), list(bytes.from_string("c")))
    list[bytes] flat = list.flatten[bytes](nested)
    list[bytes] copied = clone flat
    println(bytes.to_hex(first(flat)))
    mutable int64 index = 0
    for item in copied:
        index = index + 1
        if index == 1:
            continue
        println(bytes.to_hex(item))
        break
    list[list[int64]] chunks = list.chunk[int64](list(1, 2, 3, 4, 5), 2)
    for inner in chunks:
        println(list.sum[int64](view inner))
"#,
    );
    assert_eq!(actual.stdout, b"61\n62\n3\n7\n5\n");
}

#[test]
fn native_consuming_iteration_failure_drops_yield_and_unvisited_elements() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("failure.jett");
    std::fs::write(&path, r#"
function explode(value: bytes) returns string:
    print(bytes.to_hex(view value))
    return string.repeat("ab", 9223372036854775807)
function main() returns nothing:
    list[result[bytes, string]] items = list(ok(bytes.from_string("first")), ok(bytes.from_string("unvisited")), fail("last"))
    for item in items:
        bytes payload = item handle error:
            return nothing
        println(explode(payload))
"#).unwrap();
    let expected = jett_driver::run_file_capture_outcome(&path).expect_err("terminal oracle");
    let binary = directory.path().join("program");
    build_host_executable(&path, &launcher(), &binary).expect("consuming loop cleanup compilation");
    let actual = run_bounded(&binary, directory.path());
    assert_eq!(actual.status.code(), Some(71), "{actual:?}");
    assert_eq!(actual.stdout, expected.output.stdout.as_bytes());
    assert_eq!(actual.stderr, format!("{}\n", expected.message).as_bytes());
}

#[test]
fn native_iteration_reentry_empty_handlers_and_payload_widths() {
    run_source(
        r#"
function values(index: int64) returns list[string]:
    print("build", index, ";")
    if index == 0:
        return list.new[string]()
    return list("a", "b", "c")
function reentry() returns nothing:
    mutable int64 count = 0
    while count < 3:
        for item in values(count):
            if item == "b":
                continue
            print(item)
            optional[string] missing = none
            string value = missing handle:
                if count >= 0:
                    break
                default "unused"
        count = count + 1
    println("done")
function payloads() returns nothing:
    list[int8] narrow = list(127, -128)
    for value in view narrow:
        println(value + 1)
    list[uint8] unsigned = list(255, 0)
    for value in unsigned:
        println(value + 1)
    list[float32] floats = list(1.25, -0.0)
    for value in view floats:
        println(value)
function main() returns nothing:
    reentry()
    payloads()
    list[string] words = list("x", "y", "z")
    for outer in view words:
        for inner in view words:
            if inner == "x":
                continue
            print(outer, inner)
            break
        if outer == "y":
            break
    println(list.length[string](view words))
"#,
    );
}

#[test]
fn native_range_boundaries_and_terminal_capacity_contract() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/run_pass/range_step_boundaries.jett");
    let mut source = std::fs::read_to_string(fixture).unwrap();
    source.push_str(
        r#"
function main() returns nothing:
    println(positive_step_boundary(), negative_step_boundary())
    for item in range(5):
        print(item)
    for item in range(5, -1, -2):
        print(item)
    println(list.length[int64](range(4, 2)), list.sum[int64](range(2, 6)))
"#,
    );
    let actual = run_source(&source);
    assert_eq!(
        actual.stdout,
        b"9223372036854775806 -9223372036854775807\n012345310 14\n"
    );
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/runtime_fail/range_capacity_overflow.jett");
    let expected =
        jett_driver::run_file_capture_outcome(&fixture).expect_err("terminal range contract");
    let directory = tempfile::tempdir().unwrap();
    let binary = directory.path().join("program");
    build_host_executable(&fixture, &launcher(), &binary).expect("range fixture compilation");
    let actual = run_bounded(&binary, directory.path());
    assert_eq!(actual.status.code(), Some(71), "{actual:?}");
    assert_eq!(actual.stdout, expected.output.stdout.as_bytes());
    assert_eq!(actual.stderr, format!("{}\n", expected.message).as_bytes());
}

#[test]
fn native_range_failure_cleans_nested_live_owners_without_entering_handler() {
    for step in ["0", "1"] {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("range_failure.jett");
        let source = r#"
function explode(value: list[bytes], step: int64) returns result[list[int64], string]:
    print(list.length[bytes](view value))
    return ok(range(0, 9223372036854775807, step))
function main() returns nothing:
    list[result[bytes, string]] outer = list(ok(bytes.from_string("held")))
    list[int64] output = explode(list(bytes.from_string("argument")), STEP) handle error:
        println("not a result failure")
        return nothing
    println(list.length[int64](view output), list.length[result[bytes, string]](view outer))
"#
        .replace("STEP", step);
        std::fs::write(&path, source).unwrap();
        let expected = jett_driver::run_file_capture_outcome(&path).expect_err("terminal oracle");
        let binary = directory.path().join("program");
        build_host_executable(&path, &launcher(), &binary).expect("range cleanup compilation");
        let actual = run_bounded(&binary, directory.path());
        assert_eq!(actual.status.code(), Some(71), "{actual:?}");
        assert_eq!(actual.stdout, expected.output.stdout.as_bytes());
        assert_eq!(actual.stderr, format!("{}\n", expected.message).as_bytes());
    }
}

#[test]
fn native_string_list_kernels_run_real_split_and_line_fixtures() {
    for (name, call) in [
        (
            "string_split_grapheme_boundaries",
            "split_ignores_partial_grapheme_delimiters()",
        ),
        ("string_lines_mixed_endings", "lines_mixed_endings_test()"),
        (
            "string_split_max",
            "limited_split_preserves_remainder(), one_part_keeps_original_value(), limited_split_ignores_partial_grapheme_delimiter()",
        ),
    ] {
        let fixture =
            Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("../../tests/run_pass/{name}.jett"));
        let mut source = std::fs::read_to_string(fixture).unwrap();
        source.push_str(&format!(
            "\nfunction main() returns nothing:\n    println({call})\n"
        ));
        run_source(&source);
    }
    run_source(
        r#"
function main() returns nothing:
    bytes data = bytes.from_hex("65cc81f09f91a8e2808df09f91a9e2808df09f91a7e2808df09f91a6f09f87baf09f87b8") handle error:
        return nothing
    string text = bytes.to_string(data) handle error:
        return nothing
    println(string.reverse(text), string.join(string.chars(text), "|"))
    println(string.join(string.words("  one\t two\nthree  "), ","))
    println(string.join(string.split("ab", ""), "|"))
    println(list.length[string](string.lines("")), string.join(string.lines("\r\n\nend\r"), "|"))
"#,
    );
}
