use super::*;

const REUSE_CONTROLS: &str = r#"namespace app
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
function even_check(value: int8) returns bool:
    string marker = "even"
    trace marker
    return value modulo 2 == 0
type Even = High where even_check(value)
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
struct Item:
    value: Positive
struct HighItem:
    value: High
struct EvenItem:
    value: Even
struct Exact:
    values: NonEmpty
    text: Text
struct Strong:
    values: Pair
    text: Long
function exact_numeric() returns string:
    int8 input = 7
    Positive established = input handle error:
        return error
    Positive alias = clone established
    Item ready = Item(value: clone alias) handle error:
        return error
    Positive copied = ready.value
    int8 number = coarsen copied
    return "{number}"
function owned_fields() returns string:
    NonEmpty values = list(7, 9) handle error:
        return error
    Text text = "Agent" handle error:
        return error
    NonEmpty alias_values = clone values
    Text alias_text = clone text
    Exact exact = Exact(values: alias_values, text: alias_text) handle error:
        return error
    NonEmpty copied_values = exact.values
    Text copied_text = exact.text
    Strong strong = Strong(text: text, values: values) handle error:
        return error
    mutable list[int64] independent = coarsen clone copied_values
    independent = list.append[int64](independent, 11)
    list[int64] original = coarsen copied_values
    list[int64] stronger = coarsen strong.values
    string label = coarsen copied_text
    return "{list.length(view independent)}:{list.length(view original)}:{list.length(view stronger)}:{label}"
function local_ancestor() returns string:
    int8 input = 12
    Positive established = input handle error:
        return error
    Even leaf = clone established handle error:
        return error
    High middle = coarsen clone leaf
    EvenItem ready = EvenItem(value: middle) handle error:
        return error
    Even copied = ready.value
    int8 number = coarsen copied
    return "{number}"
function field_ancestor() returns string:
    int8 input = 12
    Positive established = input handle error:
        return error
    EvenItem ready = EvenItem(value: established) handle error:
        return error
    int8 number = coarsen ready.value
    return "{number}"
function rejected_child() returns string:
    int8 input = 7
    Positive established = input handle error:
        return error
    HighItem rejected = HighItem(value: established) handle error:
        return error
    return "must not succeed"
function pending_established() returns string:
    int8 input = 7
    Positive established = input handle error:
        return error
    Positive pending = run run established
    Item ready = Item(value: pending) handle error:
        return error
    Positive child = ready.value
    trace child
    Positive once = join child handle error:
        return error
    trace once
    Positive joined = join once handle error:
        return error
    int8 number = coarsen joined
    return "{number}"
function main(stdout: Stdout) returns nothing:
    Stdout.write(view stdout, "exact:{exact_numeric()}\n")
    Stdout.write(view stdout, "owned:{owned_fields()}\n")
    Stdout.write(view stdout, "local-ancestor:{local_ancestor()}\n")
    Stdout.write(view stdout, "field-ancestor:{field_ancestor()}\n")
    Stdout.write(view stdout, "rejected:{rejected_child()}\n")
    Stdout.write(view stdout, "pending:{pending_established()}\n")
"#;

fn execute_reuse_case(
    name: &str,
    source_text: &str,
    stdout: &str,
    debug: &[&str],
    compile_suites: bool,
) {
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("main.jett");
    fs::write(&source, source_text).unwrap();
    let expected = jett_driver::run_file_capture_output(&source)
        .unwrap_or_else(|error| panic!("{name}: {error}"));
    assert_eq!(expected.stdout, stdout, "{name}");
    assert_eq!(debug_trace_lines(&expected.debug_events), debug, "{name}");
    let mut programs = Vec::new();
    for release in [false, true] {
        let binary = directory.path().join(format!("{name}_{release}.exe"));
        jett_driver::native::build_host_executable_with_options(
            &source,
            launcher(),
            &binary,
            jett_driver::BuildOptions { release },
        )
        .unwrap_or_else(|error| panic!("{name}, release={release}: {error}"));
        programs.push((binary, release));
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
    for (binary, release) in programs {
        let actual = run_bounded(&binary, directory.path());
        assert_eq!(actual.status.code(), Some(0), "{name}: {actual:?}");
        assert_eq!(actual.stdout, stdout.as_bytes(), "{name}");
        let stderr = if release || debug.is_empty() {
            String::new()
        } else {
            jett_driver::render_debug_events(&expected.debug_events)
        };
        assert_eq!(actual.stderr, stderr.as_bytes(), "{name}: {actual:?}");
    }
    for binary in suites {
        let actual = run_bounded(&binary, directory.path());
        assert_eq!(actual.status.code(), Some(0), "{name}: {actual:?}");
        assert!(actual.stdout.is_empty(), "{name}: {actual:?}");
        assert!(actual.stderr.is_empty(), "{name}: {actual:?}");
    }
}

#[test]
fn native_direct_refinement_reuse_skips_established_local_and_field_predicates() {
    execute_reuse_case(
        "direct_refinement_reuse",
        REUSE_CONTROLS,
        "exact:7\nowned:3:2:2:Agent\nlocal-ancestor:12\nfield-ancestor:12\nrejected:refinement type constraint failed for 'app.High'\npending:7\n",
        &[
            "trace marker: string = positive",
            "trace marker: string = nonempty",
            "trace marker: string = text",
            "trace marker: string = long",
            "trace marker: string = pair",
            "trace marker: string = positive",
            "trace marker: string = high",
            "trace marker: string = even",
            "trace marker: string = even",
            "trace marker: string = positive",
            "trace marker: string = high",
            "trace marker: string = even",
            "trace marker: string = positive",
            "trace marker: string = high",
            "trace marker: string = positive",
            "trace child: app.Positive = pending(pending(7))",
            "trace once: app.Positive = pending(7)",
        ],
        false,
    );
}

const ORDER_DECLARATIONS: &str = r#"namespace app
function positive_check(value: int8) returns bool:
    string marker = "positive"
    trace marker
    trace value
    return value > 0
type Positive = int8 where positive_check(value)
function high_check(value: int8) returns bool:
    string marker = "high"
    trace marker
    trace value
    return value > 10
type High = Positive where high_check(value)
function text_check(value: string) returns bool:
    string marker = "text"
    trace marker
    return string.char_count(value) > 0
type Text = string where text_check(value)
struct Mixed:
    first: High
    exact: Text
    last: High
    payload: list[string]
function input(view stdout: Stdout) returns int8:
    Stdout.write(view stdout, "input\n")
    return 12
function payload(view stdout: Stdout) returns list[string]:
    Stdout.write(view stdout, "payload\n")
    return list("staged", "owner")
function main(stdout: Stdout) returns nothing:
    int8 first_input = __FIRST__
    Positive first = first_input handle error:
        return nothing
    Text label = "Agent" handle error:
        return nothing
    bytes earlier = bytes.from_string("retained")
    Stdout.write(view stdout, "before\n")
    Mixed ready = Mixed(last: input(view stdout), payload: payload(view stdout), first: first, exact: label) handle error:
        Stdout.write(view stdout, "outcome:{error}\n")
        Stdout.write(view stdout, "after:{bytes.to_hex(view earlier)}\n")
        return nothing
    int8 first_value = coarsen ready.first
    int8 last_value = coarsen ready.last
    Stdout.write(view stdout, "ready:{first_value}:{last_value}:{list.length(view ready.payload)}\n")
    Stdout.write(view stdout, "after:{bytes.to_hex(view earlier)}\n")
"#;

const PENDING_NEW: &str = r#"namespace app
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
struct Item:
    value: High
function main(stdout: Stdout) returns nothing:
    int8 input = 12
    Positive established = input handle error:
        return nothing
    Positive pending = run established
    Item ready = Item(value: pending) handle error:
        Stdout.write(view stdout, "field-error:{error}\n")
        return nothing
    Stdout.write(view stdout, "must not run\n")
"#;

#[test]
fn native_direct_refinement_reuse_keeps_suffix_order_and_new_pending_rejection() {
    for (name, first, outcome) in [
        ("reversed_ready", "12", "ready:12:12:2"),
        (
            "reversed_rejected",
            "7",
            "outcome:refinement type constraint failed for 'app.High'",
        ),
    ] {
        let source = ORDER_DECLARATIONS.replace("__FIRST__", first);
        let stdout = format!("before\ninput\npayload\n{outcome}\nafter:72657461696e6564\n");
        let first_trace = format!("trace value: int8 = {first}");
        execute_reuse_case(
            name,
            &source,
            &stdout,
            &[
                "trace marker: string = positive",
                &first_trace,
                "trace marker: string = text",
                "trace marker: string = positive",
                "trace value: int8 = 12",
                "trace marker: string = high",
                "trace value: int8 = 12",
                "trace marker: string = high",
                &first_trace,
            ],
            false,
        );
    }
    // Pending data entering a NEW refinement is not an established proof.
    execute_reuse_case(
        "pending_new_descendant",
        PENDING_NEW,
        "field-error:error evaluating refinement constraint for 'app.High': unsupported binary operation: pending(12) Gt 10\n",
        &[
            "trace marker: string = positive",
            "trace marker: string = high",
        ],
        false,
    );
}

const PURE_CONTROLS: &str = r#"namespace app
type Positive = int64 where value > 0
type High = Positive where value > 10
type NonEmpty = list[int64] where list.length(view value) > 0
struct Item:
    number: Positive
    values: NonEmpty
struct Box[T]:
    number: Positive
    payload: T
function scalar_copy(view source: Positive) returns int64:
    return coarsen clone source
function report(number: int64) returns string:
    Positive established = number handle error:
        return error
    Positive alias = clone established
    NonEmpty values = list(7, 9) handle error:
        return error
    NonEmpty copied = clone values
    Item ready = Item(number: alias, values: copied) handle error:
        return error
    Positive field = ready.number
    NonEmpty field_values = ready.values
    list[int64] public = coarsen field_values
    return "{scalar_copy(view field)}:{list.length(view public)}"
function generic_report[T](number: int64, payload: T) returns string:
    Positive established = number handle error:
        return error
    Box[T] ready = Box[T](number: established, payload: payload) handle error:
        return error
    int64 value = coarsen ready.number
    return "{value}"
function builder_report(number: int64) returns string:
    mutable TypeConstruction builder = type.construct_start[Box[string]]()
    for field in type.fields[Box[string]]():
        if field.name == "number":
            builder = type.construct_put[Box[string], int64](builder, view field, number) handle error:
                return error
        if field.name == "payload":
            builder = type.construct_put[Box[string], string](builder, view field, "kept") handle error:
                return error
    Box[string] ready = type.construct_finish[Box[string]](builder) handle error:
        return error
    int64 value = coarsen ready.number
    return "{value}:{ready.payload}"
function main(stdout: Stdout) returns nothing:
    string baked = comptime report(12)
    Stdout.write(view stdout, "pure:{report(12)}|{generic_report[list[string]](12, list("kept"))}|{baked}\n")
    Stdout.write(view stdout, "builder:{builder_report(7)}|{builder_report(0)}\n")
verify established_refinement_reuse:
    assert report(12) == "12:2"
    assert generic_report[string](12, "kept") == "12"
    assert builder_report(7) == "7:kept"
    assert builder_report(0) == "refinement type constraint failed for 'app.Positive'"
property established_refinement_trials:
    given high: bool
    mutable int64 number = 12
    if high:
        number = 127
    assert report(number) == "{number}:2"
    assert generic_report[list[string]](number, list("kept")) == "{number}"
    assert builder_report(0) == "refinement type constraint failed for 'app.Positive'"
"#;

#[test]
fn native_direct_refinement_reuse_supports_pure_baking_and_suites_without_changing_builders() {
    execute_reuse_case(
        "pure_refinement_reuse",
        PURE_CONTROLS,
        "pure:12:2|12|12:2\nbuilder:7:kept|refinement type constraint failed for 'app.Positive'\n",
        &[],
        true,
    );
}

const GENERIC_CONSTRUCTOR_CONTROLS: &str = r#"namespace app
type Positive = int64 where value > 0
struct Box[T]:
    value: T
function make[T](view ignored: T) returns Box[bool]:
    return Box[bool](value: true)
function refined_report(number: int64) returns string:
    Positive established = number handle error:
        return error
    result[Box[Positive], string] candidate = Box[Positive](value: established)
    Box[Positive] ready = candidate handle error:
        return error
    Positive copied = ready.value
    int64 observed = coarsen copied
    return "refined:{observed}"
function plain_report() returns string:
    Positive established = 7 handle error:
        return error
    Box[bool] ready = make[Positive](view established)
    return "plain:{ready.value}"
function main(stdout: Stdout) returns nothing:
    string baked = comptime refined_report(7)
    Stdout.write(view stdout, "generic:{refined_report(7)}|{plain_report()}|{baked}\n")
verify concrete_constructor_type_arguments:
    assert refined_report(7) == "refined:7"
    assert plain_report() == "plain:true"
property concrete_constructor_trials:
    given high: bool
    mutable int64 number = 7
    if high:
        number = 127
    assert refined_report(number) == "refined:{number}"
    assert plain_report() == "plain:true"
"#;

#[test]
fn native_generic_constructor_refinement_shape_uses_the_selected_type_arguments() {
    // Pure predicates isolate constructor type selection from the separately
    // scoped reference parameter-validation event behavior.
    execute_reuse_case(
        "generic_constructor_refinement_shape",
        GENERIC_CONSTRUCTOR_CONTROLS,
        "generic:refined:7|plain:true|refined:7\n",
        &[],
        true,
    );
}

// Scope note: traced refined parameters/returns and builder finish are excluded
// from direct reuse oracles. The unchanged pure parameter/builder controls above
// do not claim parity for their preexisting extra reference predicate events.
// Sibling/cross-namespace source comparisons are not manufactured here: checked
// nominal admission must be established before adding any such linked control.
