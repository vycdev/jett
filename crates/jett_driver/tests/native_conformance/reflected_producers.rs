use super::*;

const REFLECTED_CONTROLS: &str = r#"namespace app
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
function sibling_check(value: int64) returns bool:
    string marker = "sibling"
    trace marker
    return value >= 0
type Sibling = int64 where sibling_check(value)
function text_check(value: string) returns bool:
    string marker = "text"
    trace marker
    return string.char_count(value) > 0
type Text = string where text_check(value)
function long_check(value: string) returns bool:
    string marker = "long"
    trace marker
    return string.char_count(value) > 2
type Long = Text where long_check(value)
function nonempty_check(view value: list[int64]) returns bool:
    string marker = "nonempty"
    trace marker
    return list.length(view value) > 0
type NonEmpty = list[int64] where nonempty_check(view value)
struct Record:
    raw: int64
    positive: Positive
    high: High
    sibling: Sibling
enum Event:
    first(raw: int64, positive: Positive, high: High, sibling: Sibling)
    mirror(sibling: Sibling, high: High, positive: Positive, raw: int64)
machine Session:
    states:
        first(raw: int64, positive: Positive, high: High, sibling: Sibling)
        mirror(sibling: Sibling, high: High, positive: Positive, raw: int64)
    transitions:
        first to mirror
struct Parcel:
    raw_text: string
    text: Text
    raw_values: list[int64]
    values: NonEmpty
enum Packet:
    content(values: list[int64])
    empty
machine Store:
    states:
        content(values: list[int64])
        empty
    transitions:
        content to empty
function positive_seed(value: int64) returns Positive:
    Positive ready = value handle error:
        return positive_seed(value)
    return ready
function high_seed(view value: Positive) returns High:
    High ready = clone value handle error:
        return high_seed(view value)
    return ready
function sibling_seed(value: int64) returns Sibling:
    Sibling ready = value handle error:
        return sibling_seed(value)
    return ready
function record_seed(value: int64) returns Record:
    Positive positive = positive_seed(value)
    High high = high_seed(view positive)
    Sibling sibling = sibling_seed(value)
    return Record(raw: value, positive: positive, high: high, sibling: sibling) handle error:
        return record_seed(value)
function event_seed(value: int64, reversed: bool) returns Event:
    Positive positive = positive_seed(value)
    High high = high_seed(view positive)
    Sibling sibling = sibling_seed(value)
    if reversed:
        return Event.mirror(sibling, high, positive, value)
    return Event.first(value, positive, high, sibling)
function session_seed(value: int64, reversed: bool) returns Session:
    Positive positive = positive_seed(value)
    High high = high_seed(view positive)
    Sibling sibling = sibling_seed(value)
    if reversed:
        return Session(mirror, sibling, high, positive, value)
    return Session(first, value, positive, high, sibling)
function record_field(index: int64) returns TypeField:
    return list.get[TypeField](type.fields[Record](), index) handle:
        return record_field(0)
function event_field(view source: Event, index: int64) returns TypeField:
    TypeVariant variant = type.variant_value[Event](view source)
    return list.get[TypeField](variant.fields, index) handle:
        return event_field(view source, 0)
function session_field(view source: Session, index: int64) returns TypeField:
    TypeMachineState state = type.machine_state_value[Session](view source)
    return list.get[TypeField](state.fields, index) handle:
        return session_field(view source, 0)
function record_read(view source: Record, index: int64) returns High:
    TypeField field = record_field(index)
    return type.field_value[Record, High](view source, view field)
function event_read(view source: Event, index: int64) returns High:
    TypeField field = event_field(view source, index)
    return type.variant_field_value[Event, High](view source, view field)
function session_read(view source: Session, index: int64) returns High:
    TypeField field = session_field(view source, index)
    return type.machine_field_value[Session, High](view source, view field)
function observe(view value: Positive) returns int64:
    return coarsen clone value
function record_weaker(view source: Record) returns Positive:
    TypeField field = record_field(2)
    return type.field_value[Record, Positive](view source, view field)
function event_weaker(view source: Event, reversed: bool) returns Positive:
    mutable int64 index = 2
    if reversed:
        index = 1
    TypeField field = event_field(view source, index)
    return type.variant_field_value[Event, Positive](view source, view field)
function session_weaker(view source: Session, reversed: bool) returns Positive:
    mutable int64 index = 2
    if reversed:
        index = 1
    TypeField field = session_field(view source, index)
    return type.machine_field_value[Session, Positive](view source, view field)
function record_report(value: int64) returns string:
    Record source = record_seed(value)
    int64 raw = coarsen record_read(view source, 0)
    int64 ancestor = coarsen record_read(view source, 1)
    int64 exact = coarsen record_read(view source, 2)
    int64 sibling = coarsen record_read(view source, 3)
    int64 original = coarsen source.high
    Positive weaker = record_weaker(view source)
    int64 reused = observe(view weaker)
    return "{raw}:{ancestor}:{exact}:{sibling}:{original}:{reused}"
function event_report(value: int64, reversed: bool) returns string:
    Event source = event_seed(value, reversed)
    TypeVariant variant = type.variant_value[Event](view source)
    int64 first = coarsen event_read(view source, 0)
    int64 second = coarsen event_read(view source, 1)
    int64 third = coarsen event_read(view source, 2)
    int64 fourth = coarsen event_read(view source, 3)
    Positive weaker = event_weaker(view source, reversed)
    int64 reused = observe(view weaker)
    return "{variant.name}:{first}:{second}:{third}:{fourth}:{reused}"
function session_report(value: int64, reversed: bool) returns string:
    Session source = session_seed(value, reversed)
    TypeMachineState state = type.machine_state_value[Session](view source)
    int64 first = coarsen session_read(view source, 0)
    int64 second = coarsen session_read(view source, 1)
    int64 third = coarsen session_read(view source, 2)
    int64 fourth = coarsen session_read(view source, 3)
    Positive weaker = session_weaker(view source, reversed)
    int64 reused = observe(view weaker)
    return "{state.name}:{first}:{second}:{third}:{fourth}:{reused}"
function parcel_field(index: int64) returns TypeField:
    return list.get[TypeField](type.fields[Parcel](), index) handle:
        return parcel_field(0)
function parcel_seed() returns Parcel:
    Text text = "Agent-λ🙂" handle error:
        return parcel_seed()
    NonEmpty values = list(7, 9) handle error:
        return parcel_seed()
    return Parcel(raw_text: "Agent-λ🙂", text: text, raw_values: list(7, 9), values: values) handle error:
        return parcel_seed()
function parcel_text(view source: Parcel, index: int64) returns Long:
    TypeField field = parcel_field(index)
    return type.field_value[Parcel, Long](view source, view field)
function parcel_values(view source: Parcel, index: int64) returns NonEmpty:
    TypeField field = parcel_field(index)
    return type.field_value[Parcel, NonEmpty](view source, view field)
function packet_values(view source: Packet) returns NonEmpty:
    TypeVariant variant = type.variant_value[Packet](view source)
    TypeField field = list.get[TypeField](variant.fields, 0) handle:
        return packet_values(view source)
    return type.variant_field_value[Packet, NonEmpty](view source, view field)
function store_values(view source: Store) returns NonEmpty:
    TypeMachineState state = type.machine_state_value[Store](view source)
    TypeField field = list.get[TypeField](state.fields, 0) handle:
        return store_values(view source)
    return type.machine_field_value[Store, NonEmpty](view source, view field)
function packet_raw(view source: Packet) returns list[int64]:
    TypeVariant variant = type.variant_value[Packet](view source)
    TypeField field = list.get[TypeField](variant.fields, 0) handle:
        return packet_raw(view source)
    return type.variant_field_value[Packet, list[int64]](view source, view field)
function store_raw(view source: Store) returns list[int64]:
    TypeMachineState state = type.machine_state_value[Store](view source)
    TypeField field = list.get[TypeField](state.fields, 0) handle:
        return store_raw(view source)
    return type.machine_field_value[Store, list[int64]](view source, view field)
function owned_report() returns string:
    Parcel source = parcel_seed()
    string first = coarsen parcel_text(view source, 0)
    string second = coarsen parcel_text(view source, 1)
    NonEmpty raw = parcel_values(view source, 2)
    NonEmpty exact = parcel_values(view source, 3)
    mutable list[int64] independent = coarsen raw
    independent = list.append[int64](independent, 11)
    list[int64] untouched = coarsen exact
    Packet packet = Packet.content(list(7, 9))
    Store store = Store(content, list(7, 9))
    mutable list[int64] packet_copy = coarsen packet_values(view packet)
    packet_copy = list.append[int64](packet_copy, 11)
    list[int64] store_copy = coarsen store_values(view store)
    list[int64] packet_original = packet_raw(view packet)
    list[int64] store_original = store_raw(view store)
    return "{first}:{second}:{list.length(view independent)}:{list.length(view untouched)}:{list.length(view source.raw_values)}:{list.length(view packet_copy)}:{list.length(view packet_original)}:{list.length(view store_copy)}:{list.length(view store_original)}"
function pending_report() returns string:
    Positive positive = positive_seed(12)
    High high = high_seed(view positive)
    Sibling sibling = sibling_seed(12)
    Record source = Record(raw: 12, positive: positive, high: run run high, sibling: sibling) handle error:
        return error
    High copied = record_read(view source, 2)
    trace copied
    High once = join copied handle error:
        return error
    trace once
    High ready = join once handle error:
        return error
    int64 value = coarsen ready
    High original = source.high
    trace original
    return "{value}"
function event_pending_report() returns string:
    Positive positive = positive_seed(12)
    High high = high_seed(view positive)
    Sibling sibling = sibling_seed(12)
    Event source = Event.first(12, positive, run high, sibling)
    High copied_event = event_read(view source, 2)
    trace copied_event
    High ready = join copied_event handle error:
        return error
    int64 value = coarsen ready
    High original_event = event_read(view source, 2)
    trace original_event
    return "{value}"
function session_pending_report() returns string:
    Positive positive = positive_seed(12)
    High high = high_seed(view positive)
    Sibling sibling = sibling_seed(12)
    Session source = Session(first, 12, positive, run run high, sibling)
    High copied_session = session_read(view source, 2)
    trace copied_session
    High once_session = join copied_session handle error:
        return error
    trace once_session
    High ready = join once_session handle error:
        return error
    int64 value = coarsen ready
    High original_session = session_read(view source, 2)
    trace original_session
    return "{value}"
function source_operand(view stdout: Stdout, view source: Record) returns Record:
    Stdout.write(view stdout, "source\n")
    return clone source
function field_operand(view stdout: Stdout) returns TypeField:
    Stdout.write(view stdout, "field\n")
    return record_field(0)
function operands_report(view stdout: Stdout) returns string:
    Record source = record_seed(12)
    High copied = type.field_value[Record, High](view source_operand(view stdout, view source), view field_operand(view stdout))
    int64 value = coarsen copied
    int64 original = coarsen source.high
    return "{value}:{original}"
function main(stdout: Stdout) returns nothing:
    Stdout.write(view stdout, "record:{record_report(12)}\n")
    Stdout.write(view stdout, "event:{event_report(12, false)}|{event_report(12, true)}\n")
    Stdout.write(view stdout, "machine:{session_report(12, false)}|{session_report(12, true)}\n")
    Stdout.write(view stdout, "owned:{owned_report()}\n")
    Stdout.write(view stdout, "pending:{pending_report()}|{event_pending_report()}|{session_pending_report()}\n")
    Stdout.write(view stdout, "operands\n")
    string operand_report = operands_report(view stdout)
    Stdout.write(view stdout, "operands:{operand_report}\n")
"#;

const REFLECTED_FAILURE: &str = r#"namespace app
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
function sibling_check(value: int64) returns bool:
    string marker = "sibling"
    trace marker
    return value >= 0
type Sibling = int64 where sibling_check(value)
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
function nonempty_check(view value: list[int64]) returns bool:
    string marker = "nonempty"
    trace marker
    return list.length(view value) > 0
type NonEmpty = list[int64] where nonempty_check(view value)
struct Record:
    raw: int64
    text: string
    values: list[int64]
struct Other:
    raw: int64
enum Event:
    first(value: Positive)
    second(value: int64)
machine Session:
    states:
        first(value: Sibling)
        second(value: int64)
    transitions:
        first to second
function record_field(index: int64) returns TypeField:
    return list.get[TypeField](type.fields[Record](), index) handle:
        return record_field(0)
function event_field(view source: Event) returns TypeField:
    TypeVariant variant = type.variant_value[Event](view source)
    return list.get[TypeField](variant.fields, 0) handle:
        return event_field(view source)
function session_field(view source: Session) returns TypeField:
    TypeMachineState state = type.machine_state_value[Session](view source)
    return list.get[TypeField](state.fields, 0) handle:
        return session_field(view source)
function other_field() returns TypeField:
    return list.get[TypeField](type.fields[Other](), 0) handle:
        return other_field()
function later(view stdout: Stdout) returns optional[string]:
    Stdout.write(view stdout, "later\n")
    return none
function fallback(view stdout: Stdout) returns string:
    Stdout.write(view stdout, "handler\n")
    return "fallback"
function main(stdout: Stdout) returns nothing:
    bytes earlier = bytes.from_string("earlier-owned")
    list[string] retained = list("kept", "owned")
__SETUP__
    Stdout.write(view stdout, "before\n")
__READ__
    string message = "{later(view stdout) handle: default fallback(view stdout)}"
    Stdout.write(view stdout, "after:{message}:{bytes.to_hex(view earlier)}:{list.length(view retained)}\n")
"#;

const PURE_ENTRY: &str = r#"function report(number: int64, reversed: bool) returns string:
    return "{record_report(number)};{event_report(number, reversed)};{session_report(number, reversed)};{owned_report()}"
function expected(number: int64, reversed: bool) returns string:
    mutable string member = "first"
    if reversed:
        member = "mirror"
    return "{number}:{number}:{number}:{number}:{number}:{number};{member}:{number}:{number}:{number}:{number}:{number};{member}:{number}:{number}:{number}:{number}:{number};Agent-λ🙂:Agent-λ🙂:3:2:2:3:2:2:2"
function main(stdout: Stdout) returns nothing:
    string baked = comptime report(12, true)
    Stdout.write(view stdout, "pure:{report(12, false)}|{baked}\n")
verify reflected_producer_values:
    assert report(12, false) == expected(12, false)
    assert report(127, true) == expected(127, true)
property reflected_producer_trials:
    given high: bool
    given reversed: bool
    mutable int64 number = 12
    if high:
        number = 127
    assert report(number, reversed) == expected(number, reversed)
"#;

fn pure_source() -> String {
    let (prefix, _) = REFLECTED_CONTROLS
        .split_once("function pending_report()")
        .unwrap();
    let without_traces = prefix
        .replace("    string marker = \"positive\"\n    trace marker\n", "")
        .replace("    string marker = \"high\"\n    trace marker\n", "")
        .replace("    string marker = \"sibling\"\n    trace marker\n", "")
        .replace("    string marker = \"text\"\n    trace marker\n", "")
        .replace("    string marker = \"long\"\n    trace marker\n", "")
        .replace("    string marker = \"nonempty\"\n    trace marker\n", "");
    format!("{without_traces}{PURE_ENTRY}")
}

fn marker_lines(markers: &[&str]) -> Vec<String> {
    markers
        .iter()
        .map(|marker| format!("trace marker: string = {marker}"))
        .collect()
}

fn control_debug() -> Vec<String> {
    let read_group = [
        "positive", "high", "sibling", "positive", "high", "high", "positive", "high",
    ];
    let mut expected = Vec::new();
    // Record, both enum variants, and both machine states each seed three
    // invariants, then read raw/full, ancestor/suffix, exact, and sibling/full.
    for _ in 0..5 {
        expected.extend(marker_lines(&read_group));
    }
    expected.extend(marker_lines(&[
        "text", "nonempty", "text", "long", "long", "nonempty", "nonempty", "nonempty",
    ]));
    expected.extend(marker_lines(&["positive", "high", "sibling"]));
    expected.extend([
        "trace copied: app.High = pending(pending(12))".into(),
        "trace once: app.High = pending(12)".into(),
        "trace original: app.High = pending(pending(12))".into(),
    ]);
    expected.extend(marker_lines(&["positive", "high", "sibling"]));
    expected.extend([
        "trace copied_event: app.High = pending(12)".into(),
        "trace original_event: app.High = pending(12)".into(),
    ]);
    expected.extend(marker_lines(&["positive", "high", "sibling"]));
    expected.extend([
        "trace copied_session: app.High = pending(pending(12))".into(),
        "trace once_session: app.High = pending(12)".into(),
        "trace original_session: app.High = pending(pending(12))".into(),
    ]);
    expected.extend(marker_lines(&[
        "positive", "high", "sibling", "positive", "high",
    ]));
    expected
}

fn run_reflected_case(
    name: &str,
    text: &str,
    stdout: &str,
    message: Option<&str>,
    debug: &[String],
    suites: bool,
) {
    run_reflected_form(name, text, stdout, message, debug, suites);
    let piped = piped_getters(text);
    assert_ne!(
        piped, text,
        "{name}: pipeline fixture must exercise a getter"
    );
    run_reflected_form(
        &format!("{name}_pipeline"),
        &piped,
        stdout,
        message,
        debug,
        suites,
    );
}

fn piped_getters(text: &str) -> String {
    let mut piped = text.to_owned();
    for getter in [
        "type.field_value[Record, High]",
        "type.variant_field_value[Event, High]",
        "type.machine_field_value[Session, High]",
        "type.field_value[Record, Positive]",
        "type.variant_field_value[Event, Positive]",
        "type.machine_field_value[Session, Positive]",
        "type.field_value[Parcel, Long]",
        "type.field_value[Parcel, NonEmpty]",
        "type.variant_field_value[Packet, NonEmpty]",
        "type.machine_field_value[Store, NonEmpty]",
        "type.variant_field_value[Packet, list[int64]]",
        "type.machine_field_value[Store, list[int64]]",
        "type.field_value[Record, Risky]",
        "type.field_value[Record, Text]",
        "type.field_value[Record, NonEmpty]",
    ] {
        piped = piped.replace(
            &format!("{getter}(view source, view field)"),
            &format!("view source into view {getter}(view field)"),
        );
    }
    piped.replace(
        "type.field_value[Record, High](view source_operand(view stdout, view source), view field_operand(view stdout))",
        "source_operand(view stdout, view source) into view type.field_value[Record, High](view field_operand(view stdout))",
    )
}

fn run_reflected_form(
    name: &str,
    text: &str,
    stdout: &str,
    message: Option<&str>,
    debug: &[String],
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
        let binary = directory.path().join(format!("reflected_{release}.exe"));
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
        let verify = directory.path().join("reflected_verify.exe");
        build_host_verify_suite_executable(&source, launcher(), &verify).unwrap();
        let property = directory.path().join("reflected_property.exe");
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
fn native_reflected_root_refinements_validate_selected_suffixes_and_preserve_owned_sources() {
    run_reflected_case(
        "reflected_root_values",
        REFLECTED_CONTROLS,
        concat!(
            "record:12:12:12:12:12:12\n",
            "event:first:12:12:12:12:12|mirror:12:12:12:12:12\n",
            "machine:first:12:12:12:12:12|mirror:12:12:12:12:12\n",
            "owned:Agent-λ🙂:Agent-λ🙂:3:2:2:3:2:2:2\n",
            "pending:12|12|12\n",
            "operands\nsource\nfield\noperands:12:12\n",
        ),
        None,
        &control_debug(),
        false,
    );
}

#[test]
fn native_reflected_root_refinements_support_closed_comptime_and_native_suites() {
    run_reflected_case(
        "reflected_root_pure_values",
        &pure_source(),
        concat!(
            "pure:12:12:12:12:12:12;first:12:12:12:12:12;first:12:12:12:12:12;",
            "Agent-λ🙂:Agent-λ🙂:3:2:2:3:2:2:2|",
            "12:12:12:12:12:12;mirror:12:12:12:12:12;mirror:12:12:12:12:12;",
            "Agent-λ🙂:Agent-λ🙂:3:2:2:3:2:2:2\n",
        ),
        None,
        &[],
        true,
    );
}

struct FailureCase {
    name: &'static str,
    setup: &'static str,
    read: &'static str,
    message: &'static str,
    markers: &'static [&'static str],
}

#[test]
fn native_reflected_root_refinement_failures_preserve_selector_precedence_and_cleanup() {
    let cases: &[FailureCase] = &[
        FailureCase {
            name: "record_base",
            setup: r#"    Record source = Record(raw: -1, text: "", values: list.new[int64]())
    TypeField field = record_field(0)"#,
            read: r#"    High returned = type.field_value[Record, High](view source, view field)"#,
            message: "runtime error: refinement type constraint failed for 'app.Positive'",
            markers: &["positive"],
        },
        FailureCase {
            name: "enum_ancestor",
            setup: r#"    Positive positive = 7 handle error:
        return nothing
    Event source = Event.first(positive)
    TypeField field = event_field(view source)"#,
            read: r#"    High returned = type.variant_field_value[Event, High](view source, view field)"#,
            message: "runtime error: refinement type constraint failed for 'app.High'",
            markers: &["positive", "high"],
        },
        FailureCase {
            name: "machine_sibling",
            setup: r#"    Sibling sibling = 7 handle error:
        return nothing
    Session source = Session(first, sibling)
    TypeField field = session_field(view source)"#,
            read: r#"    High returned = type.machine_field_value[Session, High](view source, view field)"#,
            message: "runtime error: refinement type constraint failed for 'app.High'",
            markers: &["sibling", "positive", "high"],
        },
        FailureCase {
            name: "predicate_error",
            setup: r#"    Record source = Record(raw: -1, text: "", values: list.new[int64]())
    TypeField field = record_field(0)"#,
            read: r#"    Risky returned = type.field_value[Record, Risky](view source, view field)"#,
            message: "runtime error: error evaluating refinement constraint for 'app.Risky': list.__remove_at: index -1 out of bounds",
            markers: &["risky"],
        },
        FailureCase {
            name: "empty_text",
            setup: r#"    Record source = Record(raw: 7, text: "", values: list.new[int64]())
    TypeField field = record_field(1)"#,
            read: r#"    Text returned = type.field_value[Record, Text](view source, view field)"#,
            message: "runtime error: refinement type constraint failed for 'app.Text'",
            markers: &["text"],
        },
        FailureCase {
            name: "empty_list",
            setup: r#"    Record source = Record(raw: 7, text: "", values: list.new[int64]())
    TypeField field = record_field(2)"#,
            read: r#"    NonEmpty returned = type.field_value[Record, NonEmpty](view source, view field)"#,
            message: "runtime error: refinement type constraint failed for 'app.NonEmpty'",
            markers: &["nonempty"],
        },
        FailureCase {
            name: "pending_one",
            setup: r#"    Record source = Record(raw: run 7, text: "", values: list.new[int64]())
    TypeField field = record_field(0)"#,
            read: r#"    Positive returned = type.field_value[Record, Positive](view source, view field)"#,
            message: "runtime error: error evaluating refinement constraint for 'app.Positive': unsupported binary operation: pending(7) Gt 0",
            markers: &["positive"],
        },
        FailureCase {
            name: "pending_two",
            setup: r#"    Session source = Session(second, run run 7)
    TypeField field = session_field(view source)"#,
            read: r#"    Positive returned = type.machine_field_value[Session, Positive](view source, view field)"#,
            message: "runtime error: error evaluating refinement constraint for 'app.Positive': unsupported binary operation: pending(pending(7)) Gt 0",
            markers: &["positive"],
        },
        FailureCase {
            name: "wrong_owner",
            setup: r#"    Record source = Record(raw: 7, text: "", values: list.new[int64]())
    TypeField field = other_field()"#,
            read: r#"    Positive returned = type.field_value[Record, Positive](view source, view field)"#,
            message: "runtime error: type.field_value: field metadata belongs to 'app.Other', expected 'app.Record'",
            markers: &[],
        },
        FailureCase {
            name: "wrong_request",
            setup: r#"    Record source = Record(raw: 7, text: "", values: list.new[int64]())
    TypeField field = record_field(1)"#,
            read: r#"    Positive returned = type.field_value[Record, Positive](view source, view field)"#,
            message: "runtime error: type.field_value: field 'text' has type 'string', requested 'app.Positive'",
            markers: &[],
        },
        FailureCase {
            name: "wrong_enum_member",
            setup: r#"    Positive positive = 7 handle error:
        return nothing
    Event prior = Event.first(positive)
    Event source = Event.second(7)
    TypeField field = event_field(view prior)"#,
            read: r#"    High returned = type.variant_field_value[Event, High](view source, view field)"#,
            message: "runtime error: type.variant_field_value: field metadata belongs to 'app.Event.first', expected 'app.Event.second'",
            markers: &["positive"],
        },
        FailureCase {
            name: "wrong_machine_member",
            setup: r#"    Sibling sibling = 7 handle error:
        return nothing
    Session prior = Session(first, sibling)
    Session source = Session(second, 7)
    TypeField field = session_field(view prior)"#,
            read: r#"    High returned = type.machine_field_value[Session, High](view source, view field)"#,
            message: "runtime error: type.machine_field_value: field metadata belongs to 'app.Session.first', expected 'app.Session.second'",
            markers: &["sibling"],
        },
        FailureCase {
            name: "pending_selector",
            setup: r#"    Session source = Session(second, 7)
    TypeField field = run run session_field(view source)"#,
            read: r#"    High returned = type.machine_field_value[Session, High](view source, view field)"#,
            message: "runtime error: type.machine_field_value: second argument must be TypeField, got pending(pending(TypeField(index: 0, owner_type: app.Session, owner_member: some(second), name: value, type_name: int64, kind: primitive, kind_tag: TypeKind.primitive_type, serialize_name: value, has_secret: false, type_info: TypeInfo(type_name: int64, kind: primitive, kind_tag: TypeKind.primitive_type, primitive_tag: some(TypePrimitive.int64_type), has_secret: false, args: list()))))",
            markers: &[],
        },
    ];
    for case in cases {
        let source = REFLECTED_FAILURE
            .replace("__SETUP__", case.setup)
            .replace("__READ__", case.read);
        run_reflected_case(
            case.name,
            &source,
            "before\n",
            Some(case.message),
            &marker_lines(case.markers),
            false,
        );
    }
}

// TypeField is compiler-owned: source cannot forge a metadata index, field
// name, or declared type. Corrupt-plan coverage belongs in HIR/MIR tests.
// These linked cases preserve the original compatibility admission domain;
// changed nested-container requests remain outside the native parity claim.
