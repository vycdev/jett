use super::*;

const RETURN_CONTROLS: &str = r#"namespace app
function positive_check(value: int64) returns bool:
    string marker = "positive"
    trace marker
    return value > 0
type Positive = int64 where positive_check(value)
function high_check(value: int64) returns bool:
    string marker = "high"
    trace marker
    return value > 10
type High = Positive where high_check(value)
function text_check(value: string) returns bool:
    string marker = "text"
    trace marker
    return string.char_count(value) > 0
type Text = string where text_check(value)
function long_check(value: string) returns bool:
    string marker = "long"
    trace marker
    return string.char_count(value) >= 3
type Long = Text where long_check(value)
function nonempty_check(view values: list[int64]) returns bool:
    string marker = "nonempty"
    trace marker
    return list.length(view values) > 0
type NonEmpty = list[int64] where nonempty_check(view value)
function pair_check(view values: list[int64]) returns bool:
    string marker = "pair"
    trace marker
    return list.length(view values) >= 2
type Pair = NonEmpty where pair_check(view value)
function raw_scalar(value: int64) returns Positive:
    return value
function elevate(view value: Positive) returns High:
    return clone value
function echo_high(view value: High) returns High:
    return clone value
function identity[T](view value: T) returns T:
    return clone value
function create[T](value: int64) returns T:
    return value
function raw_text(value: string) returns Text:
    return value
function upgrade_text(view value: Text) returns Long:
    return clone value
function raw_values(values: list[int64]) returns NonEmpty:
    return values
function upgrade_values(view values: NonEmpty) returns Pair:
    return clone values
struct Holder:
    text: Long
    values: Pair
function read_text(view holder: Holder) returns Long:
    return holder.text
function read_values(view holder: Holder) returns Pair:
    return holder.values
struct Maker:
    version: int64
    function number(view self: Maker, value: int64) returns Positive:
        return value
function shadow[Positive](view unused: Positive) returns app.Positive:
    return 7
function reflected(value: int64) returns Positive:
    comptime type Bound = type.info[Positive]():
        return value
function scalar_report() returns string:
    Positive source = raw_scalar(12)
    High stronger = elevate(view source)
    High exact = echo_high(view stronger)
    High generic = identity[High](view stronger)
    function(view High) returns High callback = function(view value: High) returns High:
        return clone value
    High inline = callback(view stronger)
    int64 first = coarsen exact
    int64 second = coarsen generic
    int64 third = coarsen inline
    int64 original = coarsen source
    return "{first}:{second}:{third}:{original}"
function pending_report() returns string:
    Positive source = raw_scalar(12)
    High stronger = elevate(view source)
    High pending = run run clone stronger
    High copied = echo_high(view pending)
    trace copied
    High once = join copied handle error:
        return error
    trace once
    High ready = join once handle error:
        return error
    int64 value = coarsen ready
    int64 original = coarsen stronger
    return "{value}:{original}"
function owned_report() returns string:
    string label = "Agent-λ🙂"
    Text source_text = raw_text(label)
    Long longer = upgrade_text(view source_text)
    list[int64] owner = list(7, 9)
    NonEmpty source_values = raw_values(clone owner)
    Pair pair = upgrade_values(view source_values)
    Holder holder = Holder(text: longer, values: pair) handle error:
        return error
    Long copied_text = read_text(view holder)
    Pair copied_values = read_values(view holder)
    mutable list[int64] independent = coarsen copied_values
    independent = list.append[int64](independent, 11)
    list[int64] refined_original = coarsen clone source_values
    list[int64] field_original = coarsen holder.values
    string public_text = coarsen copied_text
    string original_text = coarsen source_text
    return "{public_text}:{original_text}:{list.length(view independent)}:{list.length(view owner)}:{list.length(view refined_original)}:{list.length(view field_original)}"
function contexts_report() returns string:
    function(int64) returns Positive callback = raw_scalar
    Positive named = callback(7)
    Maker maker = Maker(version: 1)
    Positive method = Maker.number(view maker, 9)
    Positive scoped = reflected(11)
    Text label = "Agent" handle error:
        return error
    Positive selected = shadow[Text](view label)
    Positive generated = create[Positive](13)
    High generated_high = create[High](15)
    int64 first = coarsen named
    int64 second = coarsen method
    int64 third = coarsen scoped
    int64 fourth = coarsen selected
    int64 fifth = coarsen generated
    int64 sixth = coarsen generated_high
    string text = coarsen label
    return "{first}:{second}:{third}:{fourth}:{fifth}:{sixth}:{text}"
function main(stdout: Stdout) returns nothing:
    Stdout.write(view stdout, "scalar:{scalar_report()}\n")
    Stdout.write(view stdout, "pending:{pending_report()}\n")
    Stdout.write(view stdout, "owned:{owned_report()}\n")
    Stdout.write(view stdout, "contexts:{contexts_report()}\n")
"#;

const RETURN_FAILURE: &str = r#"namespace app
function positive_check(value: int64) returns bool:
    string marker = "positive"
    trace marker
    return value > 0
type Positive = int64 where positive_check(value)
function high_check(value: int64) returns bool:
    string marker = "high"
    trace marker
    return value > 10
type High = Positive where high_check(value)
function risky_check(value: int64) returns bool:
    string marker = "risky"
    trace marker
    list[string] owned = list("predicate-owned")
    list[string] removed = list.remove_at[string](owned, -1)
    return value > 0
type Risky = int64 where risky_check(value)
function text_check(value: string) returns bool:
    string marker = "text"
    trace marker
    return string.char_count(value) > 0
type Text = string where text_check(value)
function nonempty_check(view values: list[int64]) returns bool:
    string marker = "nonempty"
    trace marker
    return list.length(view values) > 0
type NonEmpty = list[int64] where nonempty_check(view value)
function pair_check(view values: list[int64]) returns bool:
    string marker = "pair"
    trace marker
    return list.length(view values) >= 2
type Pair = NonEmpty where pair_check(view value)
function broken_value() returns int64:
    list[string] owned = list("operand-owned")
    list[string] removed = list.remove_at[string](owned, -1)
    return 7
function operand(view stdout: Stdout, label: string, value: int64) returns int64:
    Stdout.write(view stdout, "{label}\n")
    return value
function later(view stdout: Stdout) returns optional[string]:
    Stdout.write(view stdout, "later\n")
    return none
function fallback(view stdout: Stdout) returns string:
    Stdout.write(view stdout, "handler\n")
    return "fallback"
function attempt(view stdout: Stdout__PARAMETER__) returns __TARGET__:
    list[string] owned = list("return-owned", "payload")
    Stdout.write(view stdout, "operand\n")
__BODY__
function main(stdout: Stdout) returns nothing:
    bytes earlier = bytes.from_string("earlier-owned")
    list[string] retained = list("kept", "owned")
__SETUP__
    Stdout.write(view stdout, "before\n")
    __TARGET__ returned = attempt(view stdout__ARGUMENT__)
    string message = "{later(view stdout) handle: default fallback(view stdout)}"
    Stdout.write(view stdout, "after:{message}:{bytes.to_hex(view earlier)}:{list.length(view retained)}\n")
"#;

fn failure_source(target: &str, body: &str, ancestor: bool) -> String {
    let source_type = if target == "Pair" {
        "NonEmpty"
    } else {
        "Positive"
    };
    let setup = if target == "Pair" {
        "    NonEmpty ancestor = list(7) handle error:\n        return nothing"
    } else {
        "    Positive ancestor = 7 handle error:\n        return nothing"
    };
    let parameter = format!(", view source: {source_type}");
    RETURN_FAILURE
        .replace("__TARGET__", target)
        .replace("__BODY__", body)
        .replace("__PARAMETER__", if ancestor { &parameter } else { "" })
        .replace(
            "__ARGUMENT__",
            if ancestor { ", view ancestor" } else { "" },
        )
        .replace("__SETUP__", if ancestor { setup } else { "" })
}

fn run_return_case(
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
    let outcome = jett_driver::run_file_capture_outcome(&source);
    match (outcome, message) {
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
        let binary = directory.path().join(format!("return_{release}.exe"));
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
        let verify = directory.path().join("return_verify.exe");
        build_host_verify_suite_executable(&source, launcher(), &verify).unwrap();
        let property = directory.path().join("return_property.exe");
        build_host_property_suite_executable(&source, launcher(), &property).unwrap();
        suite_binaries.extend([verify, property]);
    }
    fs::remove_file(&source).unwrap();
    for (binary, release) in binaries {
        let actual = run_bounded(&binary, directory.path());
        let code = if message.is_some() { 71 } else { 0 };
        assert_eq!(actual.status.code(), Some(code), "{name}: {actual:?}");
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
fn native_refined_returns_validate_new_suffixes_and_preserve_exact_owned_values() {
    run_return_case(
        "refined_return_values",
        RETURN_CONTROLS,
        concat!(
            "scalar:12:12:12:12\n",
            "pending:12:12\n",
            "owned:Agent-λ🙂:Agent-λ🙂:3:2:2:2\n",
            "contexts:7:9:11:7:13:15:Agent\n",
        ),
        None,
        &[
            "trace marker: string = positive",
            "trace marker: string = high",
            "trace marker: string = positive",
            "trace marker: string = high",
            "trace copied: app.High = pending(pending(12))",
            "trace once: app.High = pending(12)",
            "trace marker: string = text",
            "trace marker: string = long",
            "trace marker: string = nonempty",
            "trace marker: string = pair",
            "trace marker: string = positive",
            "trace marker: string = positive",
            "trace marker: string = positive",
            "trace marker: string = text",
            "trace marker: string = positive",
            "trace marker: string = positive",
            "trace marker: string = positive",
            "trace marker: string = high",
        ],
        false,
    );
}

#[test]
fn native_refined_return_failures_preserve_first_error_depth_order_and_cleanup() {
    let cases: &[(&str, &str, &str, bool, &str, &str, &[&str])] = &[
        (
            "base_false",
            "Positive",
            "    return operand(view stdout, \"left\", -1) + operand(view stdout, \"right\", 0)",
            false,
            "before\noperand\nleft\nright\n",
            "runtime error: refinement type constraint failed for 'app.Positive'",
            &["trace marker: string = positive"],
        ),
        (
            "ancestor_false",
            "High",
            "    return clone source",
            true,
            "before\noperand\n",
            "runtime error: refinement type constraint failed for 'app.High'",
            &[
                "trace marker: string = positive",
                "trace marker: string = high",
            ],
        ),
        (
            "generic_ancestor_false",
            "High",
            "    return clone source",
            true,
            "before\noperand\n",
            "runtime error: refinement type constraint failed for 'app.High'",
            &[
                "trace marker: string = positive",
                "trace marker: string = high",
            ],
        ),
        (
            "reflected_ancestor_false",
            "High",
            "    comptime type Bound = type.info[High]():\n        return clone source",
            true,
            "before\noperand\n",
            "runtime error: refinement type constraint failed for 'app.High'",
            &[
                "trace marker: string = positive",
                "trace marker: string = high",
            ],
        ),
        (
            "predicate_error",
            "Risky",
            "    return 7",
            false,
            "before\noperand\n",
            "runtime error: error evaluating refinement constraint for 'app.Risky': list.__remove_at: index -1 out of bounds",
            &["trace marker: string = risky"],
        ),
        (
            "pending_one",
            "Positive",
            "    return run 7",
            false,
            "before\noperand\n",
            "runtime error: error evaluating refinement constraint for 'app.Positive': unsupported binary operation: pending(7) Gt 0",
            &["trace marker: string = positive"],
        ),
        (
            "pending_two",
            "Positive",
            "    return run run 7",
            false,
            "before\noperand\n",
            "runtime error: error evaluating refinement constraint for 'app.Positive': unsupported binary operation: pending(pending(7)) Gt 0",
            &["trace marker: string = positive"],
        ),
        (
            "empty_string",
            "Text",
            "    return \"\"",
            false,
            "before\noperand\n",
            "runtime error: refinement type constraint failed for 'app.Text'",
            &["trace marker: string = text"],
        ),
        (
            "empty_list",
            "NonEmpty",
            "    return list.new[int64]()",
            false,
            "before\noperand\n",
            "runtime error: refinement type constraint failed for 'app.NonEmpty'",
            &["trace marker: string = nonempty"],
        ),
        (
            "owned_ancestor_false",
            "Pair",
            "    return clone source",
            true,
            "before\noperand\n",
            "runtime error: refinement type constraint failed for 'app.Pair'",
            &[
                "trace marker: string = nonempty",
                "trace marker: string = pair",
            ],
        ),
        (
            "operand_error",
            "Positive",
            "    return broken_value()",
            false,
            "before\noperand\n",
            "runtime error: list.__remove_at: index -1 out of bounds",
            &[],
        ),
    ];
    for (name, target, body, ancestor, stdout, message, debug) in cases {
        let mut source = failure_source(target, body, *ancestor);
        if name.starts_with("generic_") {
            source = source
                .replace("function attempt(", "function attempt[T](")
                .replace(&format!(") returns {target}:"), ") returns T:")
                .replace("= attempt(", &format!("= attempt[{target}]("));
        }
        run_return_case(name, &source, stdout, Some(message), debug, false);
    }
}

const PURE_RETURN_CONTROLS: &str = r#"namespace app
type Positive = int64 where value > 0
type High = Positive where value > 10
type Text = string where string.char_count(value) > 0
type NonEmpty = list[int64] where list.length(view value) > 0
function scalar(value: int64) returns Positive:
    return value
function elevate(view value: Positive) returns High:
    return clone value
function text(value: string) returns Text:
    return value
function values(owned: list[int64]) returns NonEmpty:
    return owned
function identity[T](view value: T) returns T:
    return clone value
function create[T](value: int64) returns T:
    return value
function reflected(value: int64) returns Positive:
    comptime type Bound = type.info[Positive]():
        return value
function report(number: int64) returns string:
    Positive initial = scalar(number)
    High stronger = elevate(view initial)
    High copied = identity[High](view stronger)
    High generated = create[High](number)
    Text label = text("Agent")
    NonEmpty nonempty = values(list(7, 9))
    Positive scoped = reflected(number)
    int64 ready = coarsen copied
    int64 generic = coarsen generated
    int64 observed = coarsen scoped
    string public_text = coarsen label
    list[int64] public_values = coarsen nonempty
    return "{ready}:{generic}:{observed}:{public_text}:{list.length(view public_values)}"
function main(stdout: Stdout) returns nothing:
    string baked = comptime report(12)
    function(int64) returns Positive callback = comptime scalar
    Positive returned = callback(127)
    int64 ready = coarsen returned
    Stdout.write(view stdout, "pure:{report(12)}|{baked}|{ready}\n")
verify refined_return_values:
    assert report(12) == "12:12:12:Agent:2"
    assert report(127) == "127:127:127:Agent:2"
property refined_return_trials:
    given high: bool
    mutable int64 number = 12
    if high:
        number = 127
    assert report(number) == "{number}:{number}:{number}:Agent:2"
"#;

#[test]
fn native_refined_returns_support_closed_comptime_and_native_suites() {
    run_return_case(
        "refined_return_pure_values",
        PURE_RETURN_CONTROLS,
        "pure:12:12:12:Agent:2|12:12:12:Agent:2|127\n",
        None,
        &[],
        true,
    );
}

// Inline descriptors do not retain or validate their declared result annotation
// in the current interpreter. The success control uses only an already-checked
// exact refinement; invalid inline returns are a separate residual, not a parity
// claim. Public/raw interpreter APIs continue to force refinement validation.
