use super::*;

const SUCCESS_CONTROLS: &str = r#"namespace models
type Positive = int8 where value > 0
type AboveTen = Positive where value > 10
type NonEmpty = string where string.char_count(value) > 0
type Long = NonEmpty where string.char_count(value) >= 3
type NonEmptyNumbers = list[int8] where list.length[int8](view value) > 0
type PairNumbers = NonEmptyNumbers where list.length[int8](view value) >= 2
struct Item:
    number: AboveTen
    label: Long
    numbers: PairNumbers
struct Box[T]:
    number: Positive
    payload: T
function small(value: int8) returns int8:
    return value
function pair() returns list[int8]:
    return list(-128, 127)
function inspect(candidate: secret[result[Item, string]]) returns string:
    result[Item, string] exposed = declassify candidate
    Item ready = exposed handle error:
        return error
    int8 number = coarsen ready.number
    string label = coarsen ready.label
    list[int8] numbers = coarsen ready.numbers
    int8 first = list.get[int8](view numbers, 0) handle:
        return "missing"
    return "{number}:{label}:{list.length(view numbers)}:{first}"
export function checked(number: int8, label: string, numbers: list[int8]) returns string:
    secret[result[Item, string]] candidate = Item(number: number, label: label, numbers: numbers)
    return inspect(candidate)
export function number_failures() returns string:
    return "{checked(0, "Agent", list(-128, 127))}|{checked(7, "Agent", list(-128, 127))}"
export function owned_failures() returns string:
    return "{checked(12, "", list(-128, 127))}|{checked(12, "Al", list(-128, 127))}|{checked(12, "Agent", list())}|{checked(12, "Agent", list(7))}"
function already_checked() returns string:
    AboveTen number = small(12) handle error:
        return error
    Long label = "Agent" handle error:
        return error
    PairNumbers numbers = pair() handle error:
        return error
    secret[result[Item, string]] candidate = Item(number: number, label: label, numbers: numbers)
    return inspect(candidate)
function public_attempt(number: int8) returns result[Item, string]:
    return Item(number: number, label: "Agent", numbers: pair())
function hidden_attempt(number: int8) returns secret[result[Item, string]]:
    return Item(number: number, label: "Agent", numbers: pair())
function nested_attempt(number: int8) returns string:
    secret[secret[result[Item, string]]] nested = Item(number: number, label: "Agent", numbers: pair())
    secret[result[Item, string]] once = declassify nested
    return inspect(once)
function joined_attempt(number: int8) returns string:
    secret[result[Item, string]] pending = run run Item(number: number, label: "Agent", numbers: pair())
    secret[result[Item, string]] once = join pending handle error:
        return error
    secret[result[Item, string]] ready = join once handle error:
        return error
    return inspect(ready)
export function context_report() returns string:
    secret[result[Item, string]] returned = hidden_attempt(12)
    function(int8) returns secret[result[Item, string]] maker = function(number: int8) returns secret[result[Item, string]]:
        return Item(number: number, label: "Agent", numbers: pair())
    secret[result[Item, string]] inline = maker(12)
    secret[result[Item, string]] parenthesized = ((Item)(number: (small(12)), label: "Agent", numbers: pair()))
    return "{already_checked()}|{inspect(returned)}|{inspect(inline)}|{inspect(parenthesized)}|{nested_attempt(12)}|{joined_attempt(12)}"
export function producer_report() returns string:
    result[Item, string] original = public_attempt(12)
    secret[result[Item, string]] promoted = clone original
    secret[result[Item, string]] producer = public_attempt(12)
    secret[result[Item, string]] original_copy = clone original
    return "{inspect(promoted)}|{inspect(producer)}|{inspect(original_copy)}"
function generic_attempt[T](number: int8, payload: T) returns secret[result[Box[T], string]]:
    return Box[T](number: number, payload: payload)
export function generic_report() returns string:
    secret[result[Box[list[string]], string]] hidden = generic_attempt[list[string]](7, list("kept", "owner"))
    result[Box[list[string]], string] exposed = declassify hidden
    Box[list[string]] ready = exposed handle error:
        return error
    mutable list[string] copied = ready.payload
    copied = list.append[string](copied, "independent")
    int8 number = coarsen ready.number
    string last = list.get[string](view copied, 2) handle:
        return "missing"
    secret[result[Box[string], string]] text_hidden = Box[string](number: small(7), payload: "generic")
    result[Box[string], string] text_exposed = declassify text_hidden
    Box[string] text_ready = text_exposed handle error:
        return error
    return "{number}:{list.length(view ready.payload)}:{list.length(view copied)}:{last}:{text_ready.payload}"
function escaped_numbers() returns list[int8]:
    secret[result[Item, string]] hidden = Item(number: small(12), label: "Agent", numbers: pair())
    result[Item, string] exposed = declassify hidden
    Item ready = exposed handle error:
        return list()
    return coarsen ready.numbers
export function ownership_report() returns string:
    secret[result[Item, string]] hidden = Item(number: small(12), label: "Agent", numbers: pair())
    result[Item, string] exposed = declassify clone hidden
    Item ready = exposed handle error:
        return error
    Item duplicate = clone ready
    mutable list[int8] copied = coarsen ready.numbers
    copied = list.append[int8](copied, 9)
    mutable list[int8] escaped = escaped_numbers()
    escaped = list.append[int8](escaped, 10)
    int8 first = list.get[int8](view escaped, 0) handle:
        return "missing"
    list[int8] original_numbers = coarsen ready.numbers
    list[int8] duplicate_numbers = coarsen duplicate.numbers
    return "{list.length(view copied)}:{list.length(view escaped)}:{first}:{list.length(view original_numbers)}:{list.length(view duplicate_numbers)}|{inspect(hidden)}"
export function baked_report() returns string:
    secret[result[Item, string]] direct = comptime Item(number: small(12), label: "Agent", numbers: pair())
    secret[result[Item, string]] returned = comptime hidden_attempt(12)
    secret[result[Item, string]] failed = comptime Item(number: small(7), label: "Agent", numbers: pair())
    return "{inspect(direct)}|{inspect(returned)}|{inspect(failed)}"
namespace app
function main(stdout: Stdout) returns nothing:
    use models
    Stdout.write(view stdout, "valid:{models.checked(12, "Agent", list(-128, 127))}\n")
    Stdout.write(view stdout, "number-errors:{models.number_failures()}\n")
    Stdout.write(view stdout, "owned-errors:{models.owned_failures()}\n")
    Stdout.write(view stdout, "contexts:{models.context_report()}\n")
    Stdout.write(view stdout, "producers:{models.producer_report()}\n")
    Stdout.write(view stdout, "generic:{models.generic_report()}\n")
    Stdout.write(view stdout, "ownership:{models.ownership_report()}\n")
    Stdout.write(view stdout, "baked:{models.baked_report()}\n")
verify secret_validating_structs:
    use models
    assert models.checked(12, "Agent", list(-128, 127)) == "12:Agent:2:-128"
    assert models.number_failures() == "refinement type constraint failed for 'models.Positive'|refinement type constraint failed for 'models.AboveTen'"
    assert models.owned_failures() == "refinement type constraint failed for 'models.NonEmpty'|refinement type constraint failed for 'models.Long'|refinement type constraint failed for 'models.NonEmptyNumbers'|refinement type constraint failed for 'models.PairNumbers'"
    assert models.context_report() == "12:Agent:2:-128|12:Agent:2:-128|12:Agent:2:-128|12:Agent:2:-128|12:Agent:2:-128|12:Agent:2:-128"
    assert models.producer_report() == "12:Agent:2:-128|12:Agent:2:-128|12:Agent:2:-128"
    assert models.generic_report() == "7:2:3:independent:generic"
    assert models.ownership_report() == "3:3:-128:2:2|12:Agent:2:-128"
    assert models.baked_report() == "12:Agent:2:-128|12:Agent:2:-128|refinement type constraint failed for 'models.AboveTen'"
property secret_validating_struct_trials:
    given high: bool
    use models
    mutable int8 number = 12
    if high:
        number = 127
    assert models.checked(number, "Agent", list(-128, 127)) == "{number}:Agent:2:-128"
    assert models.generic_report() == "7:2:3:independent:generic"
    assert models.ownership_report() == "3:3:-128:2:2|12:Agent:2:-128"
"#;

#[test]
fn native_secret_validating_struct_results_match_reference_in_both_profiles() {
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("main.jett");
    fs::write(&source, SUCCESS_CONTROLS).unwrap();
    let expected = jett_driver::run_file_capture_output(&source)
        .expect("accepted secret validating struct constructor results");
    assert_eq!(
        expected.stdout,
        concat!(
            "valid:12:Agent:2:-128\n",
            "number-errors:refinement type constraint failed for 'models.Positive'|refinement type constraint failed for 'models.AboveTen'\n",
            "owned-errors:refinement type constraint failed for 'models.NonEmpty'|refinement type constraint failed for 'models.Long'|refinement type constraint failed for 'models.NonEmptyNumbers'|refinement type constraint failed for 'models.PairNumbers'\n",
            "contexts:12:Agent:2:-128|12:Agent:2:-128|12:Agent:2:-128|12:Agent:2:-128|12:Agent:2:-128|12:Agent:2:-128\n",
            "producers:12:Agent:2:-128|12:Agent:2:-128|12:Agent:2:-128\n",
            "generic:7:2:3:independent:generic\n",
            "ownership:3:3:-128:2:2|12:Agent:2:-128\n",
            "baked:12:Agent:2:-128|12:Agent:2:-128|refinement type constraint failed for 'models.AboveTen'\n",
        )
    );
    assert!(expected.debug_events.is_empty(), "{expected:?}");
    let mut binaries = Vec::new();
    for release in [false, true] {
        let binary = directory
            .path()
            .join(format!("secret_validating_{release}.exe"));
        jett_driver::native::build_host_executable_with_options(
            &source,
            launcher(),
            &binary,
            jett_driver::BuildOptions { release },
        )
        .expect("native secret validating struct results");
        binaries.push(binary);
    }
    let verify_binary = directory.path().join("secret_validating_verify.exe");
    build_host_verify_suite_executable(&source, launcher(), &verify_binary)
        .expect("compiled secret validating struct verify suite");
    let property_binary = directory.path().join("secret_validating_property.exe");
    build_host_property_suite_executable(&source, launcher(), &property_binary)
        .expect("compiled secret validating struct property suite");
    fs::remove_file(&source).unwrap();
    for binary in binaries {
        let actual = run_bounded(&binary, directory.path());
        assert_eq!(actual.status.code(), Some(0), "{actual:?}");
        assert_eq!(actual.stdout, expected.stdout.as_bytes());
        assert!(actual.stderr.is_empty(), "{actual:?}");
    }
    for binary in [verify_binary, property_binary] {
        let actual = run_bounded(&binary, directory.path());
        assert_eq!(actual.status.code(), Some(0), "{actual:?}");
        assert!(actual.stdout.is_empty(), "{actual:?}");
        assert!(actual.stderr.is_empty(), "{actual:?}");
    }
}

const EFFECT_DECLARATIONS: &str = r#"namespace app
function first_check(value: int8) returns bool:
    string marker = "first-predicate"
    trace marker
    return value > 0
type First = int8 where first_check(value)
function second_check(value: int8) returns bool:
    string marker = "second-predicate"
    trace marker
    return value > 0
type Second = int8 where second_check(value)
struct Pair:
    first: First
    second: Second
    payload: list[string]
function throwing_check(value: int8) returns bool:
    string marker = "throwing-predicate"
    trace marker
    list[int8] owned = list(1, 2)
    list[int8] failed = list.remove_at[int8](owned, -1)
    return list.length(view failed) > 0
type Throwing = int8 where throwing_check(value)
function last_check(value: int8) returns bool:
    string marker = "last-predicate"
    trace marker
    return value > 0
type Last = int8 where last_check(value)
struct Broken:
    first: Throwing
    last: Last
    payload: list[string]
function payload(view stdout: Stdout) returns list[string]:
    Stdout.write(view stdout, "payload\n")
    return list("staged", "owned")
function first_input(view stdout: Stdout, value: int8) returns int8:
    Stdout.write(view stdout, "first-input\n")
    return value
function second_input(view stdout: Stdout, value: int8) returns int8:
    Stdout.write(view stdout, "second-input\n")
    return value
function failing_input(view stdout: Stdout) returns int8:
    Stdout.write(view stdout, "failure\n")
    list[int8] owned = list(1, 2)
    list[int8] failed = list.remove_at[int8](owned, -1)
    return list.get[int8](view failed, 0) handle:
        default 0
function inspect_pair(candidate: secret[result[Pair, string]]) returns string:
    result[Pair, string] exposed = declassify candidate
    Pair ready = exposed handle error:
        return error
    int8 first = coarsen ready.first
    int8 second = coarsen ready.second
    return "{first}:{second}:{list.length(view ready.payload)}"
function inspect_broken(candidate: secret[result[Broken, string]]) returns string:
    result[Broken, string] exposed = declassify candidate
    Broken ready = exposed handle error:
        return error
    return "must not succeed"
"#;

fn execute_effect_case(
    name: &str,
    statements: &str,
    expected_stdout: &str,
    expected_debug: &[&str],
    terminal_error: Option<&str>,
) {
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("main.jett");
    fs::write(
        &source,
        format!(
            "{EFFECT_DECLARATIONS}function main(stdout: Stdout) returns nothing:\n    bytes earlier = bytes.from_string(\"retained\")\n    list[string] retained = list(\"earlier\", \"owner\")\n    Stdout.write(view stdout, \"before\\n\")\n{statements}    Stdout.write(view stdout, \"after:{{bytes.to_hex(view earlier)}}:{{list.length(view retained)}}\\n\")\n"
        ),
    )
    .unwrap();
    let runtime_debug_events = match (
        jett_driver::run_file_capture_outcome(&source),
        terminal_error,
    ) {
        (Ok(expected), None) => {
            assert_eq!(expected.stdout, expected_stdout, "{name}");
            assert_eq!(
                debug_trace_lines(&expected.debug_events),
                expected_debug,
                "{name}"
            );
            expected.debug_events
        }
        (Err(expected), Some(message)) => {
            assert_eq!(expected.message, message, "{name}");
            assert_eq!(expected.output.stdout, expected_stdout, "{name}");
            assert_eq!(
                debug_trace_lines(&expected.output.debug_events),
                expected_debug,
                "{name}"
            );
            expected.output.debug_events
        }
        (expected, error) => panic!("{name}: expected error {error:?}, got {expected:?}"),
    };
    let mut binaries = Vec::new();
    for release in [false, true] {
        let binary = directory.path().join(format!("{name}_{release}.exe"));
        jett_driver::native::build_host_executable_with_options(
            &source,
            launcher(),
            &binary,
            jett_driver::BuildOptions { release },
        )
        .unwrap_or_else(|error| panic!("{name}, release={release}: {error}"));
        binaries.push((binary, release));
    }
    fs::remove_file(&source).unwrap();
    for (binary, release) in binaries {
        let actual = run_bounded(&binary, directory.path());
        assert_eq!(
            actual.status.code(),
            Some(if terminal_error.is_some() { 71 } else { 0 }),
            "{name}: {actual:?}"
        );
        assert_eq!(actual.stdout, expected_stdout.as_bytes(), "{name}");
        let mut stderr = if release {
            String::new()
        } else {
            jett_driver::render_debug_events(&runtime_debug_events)
        };
        if let Some(error) = terminal_error {
            stderr.push_str(error);
            stderr.push('\n');
        }
        assert_eq!(actual.stderr, stderr.as_bytes(), "{name}: {actual:?}");
    }
}

#[test]
fn native_secret_struct_validation_preserves_field_then_predicate_order_and_result_data() {
    for (name, first, second, outcome, debug) in [
        (
            "reverse_fields_ready",
            7,
            7,
            "7:7:2",
            &[
                "trace marker: string = second-predicate",
                "trace marker: string = first-predicate",
            ][..],
        ),
        (
            "second_predicate_fails",
            7,
            0,
            "refinement type constraint failed for 'app.Second'",
            &["trace marker: string = second-predicate"][..],
        ),
        (
            "first_predicate_fails",
            0,
            7,
            "refinement type constraint failed for 'app.First'",
            &[
                "trace marker: string = second-predicate",
                "trace marker: string = first-predicate",
            ][..],
        ),
    ] {
        let statements = format!(
            "    secret[result[Pair, string]] candidate = Pair(payload: payload(view stdout), second: second_input(view stdout, {second}), first: first_input(view stdout, {first}))\n    Stdout.write(view stdout, \"outcome:{{inspect_pair(candidate)}}\\n\")\n"
        );
        let stdout = format!(
            "before\npayload\nsecond-input\nfirst-input\noutcome:{outcome}\nafter:72657461696e6564:2\n"
        );
        execute_effect_case(name, &statements, &stdout, debug, None);
    }
    // A predicate's terminal operation is captured as validation failure data.
    // All field expressions run before it; the following predicate is skipped.
    execute_effect_case(
        "predicate_operation_becomes_result_data",
        "    secret[result[Broken, string]] candidate = Broken(payload: payload(view stdout), first: first_input(view stdout, 7), last: second_input(view stdout, 7))\n    Stdout.write(view stdout, \"outcome:{inspect_broken(candidate)}\\n\")\n",
        "before\npayload\nfirst-input\nsecond-input\noutcome:error evaluating refinement constraint for 'app.Throwing': list.__remove_at: index -1 out of bounds\nafter:72657461696e6564:2\n",
        &["trace marker: string = throwing-predicate"],
        None,
    );
}

#[test]
fn native_secret_validating_struct_field_failures_precede_predicates_and_handlers() {
    for (name, constructor, stdout) in [
        (
            "later_field_failure",
            "Pair(payload: payload(view stdout), first: failing_input(view stdout), second: second_input(view stdout, 7))",
            "before\npayload\nfailure\n",
        ),
        (
            "first_field_failure",
            "Pair(first: failing_input(view stdout), payload: payload(view stdout), second: second_input(view stdout, 7))",
            "before\nfailure\n",
        ),
    ] {
        let statements = format!(
            "    secret[result[Pair, string]] candidate = {constructor}\n    result[Pair, string] exposed = declassify candidate\n    Pair ready = exposed handle error:\n        Stdout.write(view stdout, \"must not handle\\n\")\n        return nothing\n    Stdout.write(view stdout, \"must not run:{{list.length(view ready.payload)}}\\n\")\n"
        );
        execute_effect_case(
            name,
            &statements,
            stdout,
            &[],
            Some("runtime error: list.__remove_at: index -1 out of bounds"),
        );
    }
}
