use super::*;

const CALL_CONTROLS: &str = r#"
namespace app
function positive_check(value: int8) returns bool:
    string marker = "positive"
    trace marker
    return value > 0
type Positive = int8 where positive_check(value)
function high_check(value: int8) returns bool:
    string marker = "high"
    trace marker
    return value > 10
type High = Positive where high_check(value)
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
function echo_scalar(view value: Positive) returns Positive:
    return clone value
function echo_owned(value: Positive) returns Positive:
    return value
function echo_text(view value: Text) returns Text:
    return clone value
function echo_list(view value: NonEmpty) returns NonEmpty:
    return clone value
function forward[T](view value: T) returns T:
    return clone value
function shadow[Positive](view value: Positive) returns Positive:
    return clone value
struct Reader:
    version: int64
    function echo(view self: Reader, view value: NonEmpty) returns NonEmpty:
        return clone value
function scoped(view value: NonEmpty) returns NonEmpty:
    comptime type Bound = type.info[NonEmpty]():
        function(view Bound) returns Bound callback = echo_list
        return callback(view value)
function scalar_report() returns string:
    int8 input = 7
    Positive source = input handle error:
        return error
    Positive direct = echo_scalar(view source)
    function(view Positive) returns Positive callback = echo_scalar
    Positive named = callback(view source)
    Positive generic = forward[Positive](view source)
    Positive piped = (clone source) into echo_owned
    int8 first = coarsen direct
    int8 second = coarsen named
    int8 third = coarsen generic
    int8 fourth = coarsen piped
    int8 original = coarsen source
    return "{first}:{second}:{third}:{fourth}:{original}"
function owned_report() returns string:
    Text label = "Agent-λ🙂" handle error:
        return error
    NonEmpty values = list(7, 9) handle error:
        return error
    Text returned_text = echo_text(view label)
    Text shadowed = shadow[Text](view label)
    NonEmpty direct = echo_list(view values)
    function(view NonEmpty) returns NonEmpty callback = echo_list
    NonEmpty named = callback(view values)
    NonEmpty generic = forward[NonEmpty](view values)
    NonEmpty reflected = scoped(view values)
    Reader reader = Reader(version: 1)
    NonEmpty method = Reader.echo(view reader, view values)
    function(view Reader, view NonEmpty) returns NonEmpty method_callback = Reader.echo
    NonEmpty method_named = method_callback(view reader, view values)
    mutable list[int64] independent = coarsen direct
    independent = list.append[int64](independent, 11)
    list[int64] original = coarsen clone values
    list[int64] named_values = coarsen named
    list[int64] generic_values = coarsen generic
    list[int64] scoped_values = coarsen reflected
    list[int64] method_values = coarsen method
    list[int64] method_named_values = coarsen method_named
    string first = coarsen returned_text
    string second = coarsen shadowed
    return "{first}:{second}:{list.length(view independent)}:{list.length(view original)}:{list.length(view named_values)}:{list.length(view generic_values)}:{list.length(view scoped_values)}:{list.length(view method_values)}:{list.length(view method_named_values)}"
function pending_report() returns string:
    int8 input = 7
    Positive source = input handle error:
        return error
    Positive pending = run run clone source
    Positive passed = echo_scalar(view pending)
    Positive forwarded = forward[Positive](view passed)
    trace forwarded
    Positive once = join forwarded handle error:
        return error
    trace once
    Positive joined = join once handle error:
        return error
    Positive returned = echo_scalar(view joined)
    int8 ready = coarsen returned
    int8 original = coarsen source
    return "{ready}:{original}"
function new_child(value: int8) returns result[High, string]:
    Positive ancestor = value handle error:
        return fail(error)
    High descendant = ancestor handle error:
        return fail(error)
    return ok(descendant)
function child_report(value: int8) returns string:
    High ready = new_child(value) handle error:
        return error
    int8 number = coarsen ready
    return "{number}"
function handled_return(view source: Positive) returns Positive:
    optional[Positive] absent = none
    return absent handle:
        return clone source
function handled_default(view source: Positive, present: bool) returns Positive:
    mutable optional[Positive] candidate = none
    if present:
        candidate = some(clone source)
    return candidate handle:
        default clone source
function handler_report() returns string:
    int8 input = 7
    Positive source = input handle error:
        return error
    Positive returned = handled_return(view source)
    Positive absent = handled_default(view source, false)
    Positive present = handled_default(view source, true)
    int8 first = coarsen returned
    int8 second = coarsen absent
    int8 third = coarsen present
    return "{first}:{second}:{third}"
function main(stdout: Stdout) returns nothing:
    Stdout.write(view stdout, "scalar:{scalar_report()}\n")
    Stdout.write(view stdout, "owned:{owned_report()}\n")
    Stdout.write(view stdout, "pending:{pending_report()}\n")
    Stdout.write(view stdout, "child:{child_report(12)}|{child_report(7)}\n")
    Stdout.write(view stdout, "handlers:{handler_report()}\n")

"#;

fn run_checked_call_case(
    name: &str,
    source_text: &str,
    expected_stdout: &str,
    expected_debug: &[&str],
    compile_suites: bool,
) {
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("main.jett");
    fs::write(&source, source_text).unwrap();
    let expected = jett_driver::run_file_capture_output(&source)
        .unwrap_or_else(|error| panic!("{name}: {error}"));
    assert_eq!(expected.stdout, expected_stdout, "{name}");
    assert_eq!(
        debug_trace_lines(&expected.debug_events),
        expected_debug,
        "{name}"
    );
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
    let mut suites = Vec::new();
    if compile_suites {
        let verify = directory.path().join(format!("{name}_verify.exe"));
        build_host_verify_suite_executable(&source, launcher(), &verify).unwrap();
        let property = directory.path().join(format!("{name}_property.exe"));
        build_host_property_suite_executable(&source, launcher(), &property).unwrap();
        suites.extend([verify, property]);
    }
    fs::remove_file(&source).unwrap();
    for (binary, release) in binaries {
        let actual = run_bounded(&binary, directory.path());
        assert_eq!(actual.status.code(), Some(0), "{name}: {actual:?}");
        assert_eq!(actual.stdout, expected_stdout.as_bytes(), "{name}");
        let debug = if release || expected_debug.is_empty() {
            String::new()
        } else {
            jett_driver::render_debug_events(&expected.debug_events)
        };
        assert_eq!(actual.stderr, debug.as_bytes(), "{name}: {actual:?}");
    }
    for binary in suites {
        let actual = run_bounded(&binary, directory.path());
        assert_eq!(actual.status.code(), Some(0), "{name}: {actual:?}");
        assert!(actual.stdout.is_empty(), "{name}: {actual:?}");
        assert!(actual.stderr.is_empty(), "{name}: {actual:?}");
    }
}

#[test]
fn native_checked_refinement_calls_preserve_parameter_and_return_identity() {
    run_checked_call_case(
        "checked_refinement_calls",
        CALL_CONTROLS,
        concat!(
            "scalar:7:7:7:7:7\n",
            "owned:Agent-λ🙂:Agent-λ🙂:3:2:2:2:2:2:2\n",
            "pending:7:7\n",
            "child:12|refinement type constraint failed for 'app.High'\n",
            "handlers:7:7:7\n",
        ),
        &[
            "trace marker: string = positive",
            "trace marker: string = text",
            "trace marker: string = nonempty",
            "trace marker: string = positive",
            "trace forwarded: app.Positive = pending(pending(7))",
            "trace once: app.Positive = pending(7)",
            "trace marker: string = positive",
            "trace marker: string = high",
            "trace marker: string = positive",
            "trace marker: string = high",
            "trace marker: string = positive",
        ],
        false,
    );
}

#[test]
fn native_checked_refinement_call_proofs_follow_named_argument_order() {
    run_checked_call_case(
        "checked_refinement_argument_order",
        ORDER_CONTROLS,
        "before\nlabel\nnumber\nchosen:7:72657461696e6564\n",
        &[
            "trace marker: string = text",
            "trace marker: string = positive",
        ],
        false,
    );
}

const PURE_CONTROLS: &str = r#"namespace app
type Positive = int64 where value > 0
type Text = string where string.char_count(value) > 0
type NonEmpty = list[int64] where list.length(view value) > 0
function echo_num(view value: Positive) returns Positive:
    return clone value
function echo_text(view value: Text) returns Text:
    return clone value
function echo_list(view value: NonEmpty) returns NonEmpty:
    return clone value
function forward[T](view value: T) returns T:
    return clone value
function shadow[Positive](view value: Positive) returns Positive:
    return clone value
function scoped(view value: NonEmpty) returns NonEmpty:
    comptime type Bound = type.info[NonEmpty]():
        function(view Bound) returns Bound callback = echo_list
        return callback(view value)
function report(number: int64) returns string:
    Positive established = number handle error:
        return error
    Text label = "Agent" handle error:
        return error
    NonEmpty values = list(7, 9) handle error:
        return error
    function(view Positive) returns Positive callback = echo_num
    Positive returned = callback(view established)
    Positive generic = forward[Positive](view returned)
    Text text = shadow[Text](view label)
    NonEmpty copied = scoped(view values)
    mutable list[int64] independent = coarsen copied
    independent = list.append[int64](independent, 11)
    list[int64] original = coarsen clone values
    int64 value = coarsen generic
    string public_text = coarsen text
    return "{value}:{public_text}:{list.length(view independent)}:{list.length(view original)}"
function baked_callback_report() returns string:
    Positive established = 7 handle error:
        return error
    function(view Positive) returns Positive callback = comptime echo_num
    Positive ready = callback(view established)
    int64 value = coarsen ready
    return "{value}"
function main(stdout: Stdout) returns nothing:
    string baked = comptime report(7)
    Stdout.write(view stdout, "pure:{report(7)}|{baked}|{baked_callback_report()}\n")
verify checked_refinement_call_values:
    assert report(7) == "7:Agent:3:2"
    assert report(0) == "refinement type constraint failed for 'app.Positive'"
    assert baked_callback_report() == "7"
property checked_refinement_call_trials:
    given high: bool
    mutable int64 number = 7
    if high:
        number = 127
    assert report(number) == "{number}:Agent:3:2"
    assert report(0) == "refinement type constraint failed for 'app.Positive'"
    assert baked_callback_report() == "7"
"#;

#[test]
fn native_checked_refinement_calls_support_closed_baking_and_native_suites() {
    run_checked_call_case(
        "checked_refinement_pure_calls",
        PURE_CONTROLS,
        "pure:7:Agent:3:2|7:Agent:3:2|7\n",
        &[],
        true,
    );
}

const BUILDER_CONTROLS: &str = r#"namespace app
function valid(value: int64) returns bool:
    string marker = "positive"
    trace marker
    trace value
    return value > 0
type Positive = int64 where valid(value)
struct Item:
    value: Positive
function main(stdout: Stdout) returns nothing:
    Positive established = 7 handle error:
        return nothing
    mutable TypeConstruction checked = type.construct_start[Item]()
    for field in type.fields[Item]():
        checked = type.construct_put[Item, Positive](checked, view field, clone established) handle error:
            return nothing
    Item ready = type.construct_finish[Item](checked) handle error:
        return nothing
    int64 value = coarsen ready.value
    Stdout.write(view stdout, "ready:{value}\n")
    mutable TypeConstruction unvalidated = type.construct_start[Item]()
    for field in type.fields[Item]():
        unvalidated = type.construct_put[Item, int64](unvalidated, view field, 0) handle error:
            return nothing
    Item rejected = type.construct_finish[Item](unvalidated) handle error:
        Stdout.write(view stdout, "rejected:{error}\n")
        return nothing
    Stdout.write(view stdout, "must not run\n")
"#;

#[test]
fn native_checked_call_proofs_keep_reflected_builder_finish_validation_forced() {
    run_checked_call_case(
        "checked_calls_forced_builder_control",
        BUILDER_CONTROLS,
        "ready:7\nrejected:refinement type constraint failed for 'app.Positive'\n",
        &[
            "trace marker: string = positive",
            "trace value: int64 = 7",
            "trace marker: string = positive",
            "trace value: int64 = 7",
            "trace marker: string = positive",
            "trace value: int64 = 0",
        ],
        false,
    );
}

// Deliberately outside this draft: new raw/ancestor values returned under a
// declared refinement are checker-admitted, but native return lowering lacks
// predicates. Ignored reference probes pin their terminal errors separately.
// Inline closure return annotations and callback lifetime policy are unchanged.

const ORDER_CONTROLS: &str = r#"
namespace app
function positive_check(value: int8) returns bool:
    string marker = "positive"
    trace marker
    return value > 0
type Positive = int8 where positive_check(value)
function text_check(value: string) returns bool:
    string marker = "text"
    trace marker
    return string.char_count(value) > 0
type Text = string where text_check(value)
function number(view stdout: Stdout) returns Positive:
    Stdout.write(view stdout, "number\n")
    int8 input = 7
    Positive ready = input handle error:
        return number(view stdout)
    return ready
function label(view stdout: Stdout) returns Text:
    Stdout.write(view stdout, "label\n")
    Text ready = "Agent" handle error:
        return label(view stdout)
    return ready
function pick(first: Positive, label: Text) returns Positive:
    return first
function main(stdout: Stdout) returns nothing:
    bytes earlier = bytes.from_string("retained")
    Stdout.write(view stdout, "before\n")
    Positive chosen = pick(label: label(view stdout), first: number(view stdout))
    int8 value = coarsen chosen
    Stdout.write(view stdout, "chosen:{value}:{bytes.to_hex(view earlier)}\n")

"#;
