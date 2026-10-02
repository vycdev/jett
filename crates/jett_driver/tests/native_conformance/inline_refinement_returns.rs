use super::*;

const INLINE_SUCCESS: &str = r#"namespace models
export type Positive = int64 where value > 0
export type High = Positive where value > 5
export type Text = string where string.char_count(value) > 0
export type NonEmpty = list[int64] where list.length(view value) > 0
export function make[T]() returns function(int64) returns T:
    return function(value: int64) returns T: return value
export function reflected() returns function(int64) returns Positive:
    comptime type Field = type.info[Positive]():
        return function(value: int64) returns Field: return value
namespace shadow
export type Positive = int64 where value < 0
namespace app
function report(number: int64) returns string:
    use models
    function(int64) returns models.Positive scalar = function(value: int64) returns models.Positive:
        return value
    models.Positive initial = scalar(number)
    function(view models.Positive) returns models.High stronger = function(view value: models.Positive) returns models.High:
        return clone value
    models.High raised = stronger(view initial)
    function(view models.High) returns models.High exact = function(view value: models.High) returns models.High:
        return clone value
    models.High copied = exact(view raised)
    function(int64) returns models.Positive generic = models.make[models.Positive]()
    models.Positive specialized = generic(number)
    function(int64) returns models.Positive reflected = models.reflected()
    models.Positive scoped = reflected(number)
    function(string) returns models.Text text = function(value: string) returns models.Text: return value
    models.Text label = text("Agent-λ🙂")
    function(view list[int64]) returns models.NonEmpty values = function(view source: list[int64]) returns models.NonEmpty:
        return clone source
    list[int64] owned = list(7, 9)
    models.NonEmpty kept = values(view owned)
    function(view models.NonEmpty) returns models.NonEmpty echo = function(view source: models.NonEmpty) returns models.NonEmpty:
        return clone source
    models.NonEmpty copied_values = echo(view kept)
    list[int64] public_values = coarsen copied_values
    int64 first = coarsen initial
    int64 second = coarsen copied
    int64 third = coarsen specialized
    int64 fourth = coarsen scoped
    string public_text = coarsen label
    return "{first}:{second}:{third}:{fourth}:{public_text}:{list.length(view public_values)}:{list.length(view owned)}"
function alias_report(number: int64) returns string:
    use models as selected
    use shadow as models
    function(int64) returns selected.Positive callback = function(value: int64) returns selected.Positive:
        return value
    return "{coarsen callback(number)}"
function main(stdout: Stdout) returns nothing:
    use models
    string baked = comptime report(12)
    function(int64) returns models.Positive callback = comptime models.make[models.Positive]()
    models.Positive produced = callback(127)
    int64 ready = coarsen produced
    string alias_baked = comptime alias_report(7)
    Stdout.write(view stdout, "{report(12)}|{baked}|{ready}|{alias_report(7)}:{alias_baked}\n")
verify inline_return_contracts:
    assert report(12) == "12:12:12:12:Agent-λ🙂:2:2"
    assert alias_report(7) == "7"
property inline_return_trials:
    given high: bool
    int64 number = 12
    if high:
        assert report(number) == "12:12:12:12:Agent-λ🙂:2:2"
        assert alias_report(number) == "12"
    else:
        assert report(127) == "127:127:127:127:Agent-λ🙂:2:2"
        assert alias_report(127) == "127"
"#;

const INLINE_FAILURE: &str = r#"namespace app
function positive_check(value: int64) returns bool:
    string marker = "positive"
    trace marker
    return value > 0
type Positive = int64 where positive_check(value)
function echo(value: Positive) returns Positive:
    return value
function operand(view stdout: Stdout) returns int64:
    Stdout.write(view stdout, "operand\n")
    return -1
function relay(view stdout: Stdout, callback: function(int64) returns Positive) returns Positive:
    return callback(operand(view stdout))
function main(stdout: Stdout) returns nothing:
    bytes earlier = bytes.from_string("earlier-owned")
    list[string] kept = list("kept", "owned")
    function(int64) returns Positive callback = function(value: int64) returns Positive:
        list[string] closure_owned = list("closure-owned", "payload")
        return value
    Stdout.write(view stdout, "before\n")
__INVOCATION__
    Stdout.write(view stdout, "after:{bytes.to_hex(view earlier)}:{list.length(view kept)}\n")
"#;

fn run_inline_refinement_case(
    name: &str,
    text: &str,
    stdout: &str,
    message: Option<&str>,
    debug: &[&str],
    suites: bool,
) {
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("main.jett");
    fs::write(&source, text).unwrap();
    match (jett_driver::run_file_capture_outcome(&source), message) {
        (Ok(expected), None) => {
            assert_eq!(expected.stdout, stdout, "{name}");
            assert_eq!(expected.debug_output, debug, "{name}");
        }
        (Err(expected), Some(message)) => {
            assert_eq!(expected.message, message, "{name}");
            assert_eq!(expected.output.stdout, stdout, "{name}");
            assert_eq!(expected.output.debug_output, debug, "{name}");
        }
        (actual, message) => panic!("{name}: outcome {actual:?}, error {message:?}"),
    }
    let mut binaries = Vec::new();
    for release in [false, true] {
        let binary = directory.path().join(format!("inline_{release}.exe"));
        jett_driver::native::build_host_executable_with_options(
            &source,
            launcher(),
            &binary,
            jett_driver::BuildOptions { release },
        )
        .unwrap_or_else(|error| panic!("{name}: {error}"));
        binaries.push((binary, release));
    }
    let mut suite_binaries = Vec::new();
    if suites {
        let verify = directory.path().join("inline_verify.exe");
        build_host_verify_suite_executable(&source, launcher(), &verify).unwrap();
        let property = directory.path().join("inline_property.exe");
        build_host_property_suite_executable(&source, launcher(), &property).unwrap();
        suite_binaries.extend([verify, property]);
    }
    fs::remove_file(&source).unwrap();
    for (binary, release) in binaries {
        let actual = run_bounded(&binary, directory.path());
        assert_eq!(
            actual.status.code(),
            Some(if message.is_some() { 71 } else { 0 }),
            "{name}: {actual:?}"
        );
        assert_eq!(actual.stdout, stdout.as_bytes(), "{name}");
        let mut stderr = if release || debug.is_empty() {
            String::new()
        } else {
            format!("{}\n", debug.join("\n"))
        };
        if let Some(message) = message {
            stderr.push_str(message);
            stderr.push('\n');
        }
        assert_eq!(actual.stderr, stderr.as_bytes(), "{name}: {actual:?}");
    }
    for binary in suite_binaries {
        let actual = run_bounded(&binary, directory.path());
        assert_eq!(actual.status.code(), Some(0), "{name}: {actual:?}");
        assert!(actual.stdout.is_empty(), "{name}: {actual:?}");
        assert!(actual.stderr.is_empty(), "{name}: {actual:?}");
    }
}

#[test]
fn native_inline_refinement_returns_keep_captured_contracts_and_owned_values() {
    run_inline_refinement_case(
        "inline_return_contracts",
        INLINE_SUCCESS,
        "12:12:12:12:Agent-λ🙂:2:2|12:12:12:12:Agent-λ🙂:2:2|127|7:7\n",
        None,
        &[],
        true,
    );
}

#[test]
fn native_inline_refinement_return_failures_precede_checked_consumers_and_cleanup() {
    for (name, invocation) in [
        (
            "local",
            "    Positive rejected = callback(operand(view stdout))",
        ),
        (
            "argument",
            "    Positive rejected = echo(callback(operand(view stdout)))",
        ),
        (
            "return",
            "    Positive rejected = relay(view stdout, callback)",
        ),
        (
            "higher_order_map",
            "    list[int64] inputs = list(operand(view stdout))\n    list[Positive] rejected = list.map[int64, Positive](inputs, callback)",
        ),
    ] {
        let source = INLINE_FAILURE.replace("__INVOCATION__", invocation);
        run_inline_refinement_case(
            name,
            &source,
            "before\noperand\n",
            Some("runtime error: refinement type constraint failed for 'app.Positive'"),
            &["trace marker: string = positive"],
            false,
        );
    }
}
