use super::*;

const CONSTRUCTOR_CONTROLS: &str = r#"namespace wire
export bitfield Header:
    version: 4 bits
    length: 8 bits
export type Classified = secret[Header]
type HeaderAlias = Header
bitfield Wide:
    sequence: 64 bits
enum Protocol:
    tcp = 6
    udp = 17
bitfield network NetworkHeader:
    version: 4 bits
    protocol: 8 bits as Protocol
bitfield Packet:
    kind: 8 bits
    payload: list[uint8]
interface Named:
    function name(view self: Named) returns string
struct Counter:
    value: int8
implement Named for Counter:
    function name(view self: Counter) returns string:
        return "record:{self.value}"
type Callback = function(int8) returns int8
function captured(offset: int8) returns Callback:
    return function(value: int8) returns int8: return value + offset
machine Session:
    states:
        idle
        active(label: string, numbers: list[int8], owner: Named, callback: Callback)
    transitions:
        idle to active
        active to idle
export function shown(view source: secret[Header]) returns string:
    int64 version = declassify source.version
    int64 length = declassify source.length
    return "{version}:{length}"
function public_header() returns Header:
    return Header(version: 15, length: 255)
function classified() returns Classified:
    return Header(version: 7, length: 128)
function generic_header[T](unused: T) returns secret[Header]:
    return Header(version: 15, length: 255)
function nested_header() returns string:
    secret[secret[Header]] nested = Header(version: 15, length: 255)
    secret[Header] once = declassify nested
    return shown(view once)
export function constructor_report() returns string:
    secret[Header] zero = Header(version: 0, length: 0)
    secret[HeaderAlias] high = Header(version: 15, length: 255)
    Classified returned = classified()
    secret[Header] generic = generic_header[uint8](7)
    function() returns secret[Header] maker = function() returns secret[Header]: return Header(version: 7, length: 255)
    secret[Header] inline = maker()
    secret[Header] pending = run run (Header(version: 9, length: 255))
    secret[Header] once = join pending handle error:
        return error
    secret[Header] ready = join once handle error:
        return error
    secret[Header] baked = comptime Header(version: 15, length: 255)
    Header original = public_header()
    secret[Header] lifted = clone original
    secret[Header] producer = public_header()
    return "{shown(view zero)}|{shown(view high)}|{shown(view returned)}|{shown(view generic)}|{shown(view inline)}|{shown(view ready)}|{shown(view baked)}|{shown(view lifted)}|{shown(view producer)}|{nested_header()}"
function unwrap_attempt(candidate: secret[result[Header, string]]) returns string:
    result[Header, string] exposed = declassify candidate
    Header ready = exposed handle error:
        return error
    secret[Header] hidden = ready
    return shown(view hidden)
export function checked(version: int64, length: int64) returns string:
    secret[result[Header, string]] attempt = Header(version: version, length: length)
    return unwrap_attempt(attempt)
function contextual_handle(version: int64) returns string:
    secret[Header] hidden = Header(version: version, length: 255) handle error:
        return error
    return shown(view hidden)
function nested_attempt() returns string:
    secret[secret[result[Header, string]]] nested = Header(version: (15), length: (255))
    secret[result[Header, string]] once = declassify nested
    return unwrap_attempt(once)
function baked_attempt() returns string:
    secret[result[Header, string]] attempt = comptime Header(version: (15), length: 255)
    return unwrap_attempt(attempt)
export function validation_report() returns string:
    return "{checked(0, 0)}|{checked(15, 255)}|{checked(16, 255)}|{checked(-1, 255)}|{checked(15, 256)}|{contextual_handle(15)}|{nested_attempt()}|{baked_attempt()}"
function wide_checked(value: uint64) returns string:
    secret[result[Wide, string]] attempt = Wide(sequence: value)
    result[Wide, string] exposed = declassify attempt
    Wide ready = exposed handle error:
        return error
    return "{ready.sequence}"
export function wide_report() returns string:
    secret[Wide] wide = Wide(sequence: 18446744073709551615)
    uint64 sequence = declassify wide.sequence
    secret[NetworkHeader] hidden_network = NetworkHeader(version: 15, protocol: Protocol.tcp)
    Protocol protocol = declassify hidden_network.protocol
    return "{sequence}:{wide_checked(18446744073709551615)}:{protocol == Protocol.tcp}"
function payload_copy() returns secret[list[uint8]]:
    secret[Packet] packet = Packet(kind: 255, payload: list(0, 127, 255))
    return packet.payload
export function payload_report() returns string:
    list[uint8] original = list(0, 127, 255)
    secret[Packet] source = Packet(kind: 255, payload: clone original)
    secret[Packet] duplicate = clone source
    secret[list[uint8]] escaped = payload_copy()
    mutable list[uint8] independent = declassify escaped
    independent = list.append[uint8](independent, 9)
    uint8 last = list.get[uint8](view independent, 2) handle:
        return "missing item"
    list[uint8] source_items = declassify source.payload
    list[uint8] duplicate_items = declassify duplicate.payload
    return "{list.length(view original)}:{list.length(view independent)}:{last}:{list.length(view source_items)}:{list.length(view duplicate_items)}"
function machine_summary(view source: secret[Session at active]) returns string:
    string label = declassify source.label
    list[int8] numbers = declassify source.numbers
    Named owner = declassify source.owner
    Callback callback = declassify source.callback
    return "{label}:{list.length(view numbers)}:{Named.name(view owner)}:{callback(4)}"
function machine_return(value: int8) returns secret[Session at active]:
    return Session(active, "Returned", list(-128, 127), Counter(value: value), captured(value))
function generic_machine[T](unused: T) returns secret[Session at active]:
    return Session(active, "Generic", list(-128, 127), Counter(value: 6), captured(6))
function nested_machine() returns string:
    secret[secret[Session at active]] nested = Session(active, "Nested", list(-128, 127), Counter(value: 7), captured(7))
    secret[Session at active] once = declassify nested
    return machine_summary(view once)
function joined_machine() returns string:
    secret[Session at active] pending = run run Session(active, "Joined", list(-128, 127), Counter(value: 10), captured(10))
    secret[Session at active] once = join pending handle error:
        return error
    secret[Session at active] ready = join once handle error:
        return error
    return machine_summary(view ready)
function idle_machine() returns bool:
    secret[Session at idle] hidden = Session(idle)
    Session at idle ready = declassify hidden
    return ready at idle
export function machine_report() returns string:
    secret[Session at active] direct = Session(active, "Ada-λ🙂", list(-128, 127), Counter(value: 7), captured(7))
    secret[Session at active] returned = machine_return(8)
    function() returns secret[Session at active] maker = function() returns secret[Session at active]:
        return Session(active, "Inline", list(-128, 127), Counter(value: 9), captured(9))
    secret[Session at active] inline = maker()
    secret[Session at active] generic = generic_machine[bool](true)
    return "{machine_summary(view direct)}|{machine_summary(view returned)}|{machine_summary(view inline)}|{machine_summary(view generic)}|{nested_machine()}|{joined_machine()}|{idle_machine()}"
export function baked_report() returns string:
    secret[Header] header = comptime Header(version: 15, length: 255)
    secret[Session at active] session = comptime Session(active, "Baked", list(-128, 127), Counter(value: 7), captured(7))
    return "{shown(view header)}|{baked_attempt()}|{machine_summary(view session)}"
namespace app
function alias_report() returns string:
    use wire as w
    secret[w.Header] hidden = w.Header(version: 7, length: 31)
    return w.shown(view hidden)
function main(stdout: Stdout) returns nothing:
    use wire
    Stdout.write(view stdout, "constructors:{wire.constructor_report()}\n")
    Stdout.write(view stdout, "validation:{wire.validation_report()}\n")
    Stdout.write(view stdout, "wide:{wire.wide_report()}\n")
    Stdout.write(view stdout, "payload:{wire.payload_report()}\n")
    Stdout.write(view stdout, "machines:{wire.machine_report()}\n")
    Stdout.write(view stdout, "alias:{alias_report()}\n")
    Stdout.write(view stdout, "comptime:{wire.baked_report()}\n")
verify secret_constructors:
    use wire
    assert wire.constructor_report() == "0:0|15:255|7:128|15:255|7:255|9:255|15:255|15:255|15:255|15:255"
    assert wire.validation_report() == "0:0|15:255|bitfield 'wire.Header' field 'version' is 4 bit(s) wide and cannot hold '16'|bitfield 'wire.Header' field 'version' is 4 bit(s) wide and cannot hold '-1'|bitfield 'wire.Header' field 'length' is 8 bit(s) wide and cannot hold '256'|15:255|15:255|15:255"
    assert wire.wide_report() == "18446744073709551615:18446744073709551615:true"
    assert wire.payload_report() == "3:4:255:3:3"
    assert wire.machine_report() == "Ada-λ🙂:2:record:7:11|Returned:2:record:8:12|Inline:2:record:9:13|Generic:2:record:6:10|Nested:2:record:7:11|Joined:2:record:10:14|true"
    assert alias_report() == "7:31"
    assert wire.baked_report() == "15:255|15:255|Baked:2:record:7:11"
property secret_constructor_trials:
    given high: bool
    use wire
    mutable int64 version = 0
    if high:
        version = 15
    assert wire.checked(version, 255) == "{version}:255"
    assert wire.payload_report() == "3:4:255:3:3"
    assert wire.machine_report() == "Ada-λ🙂:2:record:7:11|Returned:2:record:8:12|Inline:2:record:9:13|Generic:2:record:6:10|Nested:2:record:7:11|Joined:2:record:10:14|true"
"#;

#[test]
fn native_secret_bitfield_and_machine_constructors_match_reference_in_both_profiles() {
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("main.jett");
    fs::write(&source, CONSTRUCTOR_CONTROLS).unwrap();
    let expected = jett_driver::run_file_capture_output(&source)
        .expect("accepted secret bitfield values, validating results, and exact machine states");
    assert_eq!(
        expected.stdout,
        concat!(
            "constructors:0:0|15:255|7:128|15:255|7:255|9:255|15:255|15:255|15:255|15:255\n",
            "validation:0:0|15:255|bitfield 'wire.Header' field 'version' is 4 bit(s) wide and cannot hold '16'|bitfield 'wire.Header' field 'version' is 4 bit(s) wide and cannot hold '-1'|bitfield 'wire.Header' field 'length' is 8 bit(s) wide and cannot hold '256'|15:255|15:255|15:255\n",
            "wide:18446744073709551615:18446744073709551615:true\n",
            "payload:3:4:255:3:3\n",
            "machines:Ada-λ🙂:2:record:7:11|Returned:2:record:8:12|Inline:2:record:9:13|Generic:2:record:6:10|Nested:2:record:7:11|Joined:2:record:10:14|true\n",
            "alias:7:31\n",
            "comptime:15:255|15:255|Baked:2:record:7:11\n",
        )
    );
    assert!(expected.debug_events.is_empty(), "{expected:?}");
    let mut binaries = Vec::new();
    for release in [false, true] {
        let binary = directory
            .path()
            .join(format!("secret_constructors_{release}.exe"));
        jett_driver::native::build_host_executable_with_options(
            &source,
            launcher(),
            &binary,
            jett_driver::BuildOptions { release },
        )
        .expect("native qualified nominal constructors");
        binaries.push(binary);
    }
    let verify_binary = directory.path().join("secret_constructors_verify.exe");
    build_host_verify_suite_executable(&source, launcher(), &verify_binary)
        .expect("compiled secret nominal constructor verify suite");
    let property_binary = directory.path().join("secret_constructors_property.exe");
    build_host_property_suite_executable(&source, launcher(), &property_binary)
        .expect("compiled secret nominal constructor property suite");
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

const EFFECT_DECLARATIONS: &str = r#"namespace app
bitfield Packet:
    kind: 8 bits
    payload: list[uint8]
machine Snapshot:
    states:
        active(label: string, payload: list[uint8], count: int64)
        closed
    transitions:
        active to closed
function payload(view stdout: Stdout) returns list[uint8]:
    Stdout.write(view stdout, "payload\n")
    return list(0, 127, 255)
function width(view stdout: Stdout) returns int64:
    Stdout.write(view stdout, "width\n")
    return 256
function failure(view stdout: Stdout) returns int64:
    Stdout.write(view stdout, "failure\n")
    list[int64] owned = list(1, 2)
    list[int64] failed = list.remove_at[int64](owned, -1)
    return list.length(view failed)
function failing_payload(view stdout: Stdout) returns list[uint8]:
    Stdout.write(view stdout, "failure\n")
    list[uint8] owned = list(1, 2)
    return list.remove_at[uint8](owned, -1)
function label(view stdout: Stdout) returns string:
    Stdout.write(view stdout, "label\n")
    return "Ada"
function later(view stdout: Stdout) returns int64:
    Stdout.write(view stdout, "must not run\n")
    return 7
function handled_count(view stdout: Stdout, candidate: optional[int64]) returns int64:
    return candidate handle:
        Stdout.write(view stdout, "fallback\n")
        default 7
"#;

fn execute_effect_case(
    name: &str,
    statements: &str,
    expected_stdout: &str,
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
    let expected = jett_driver::run_file_capture_outcome(&source);
    match (expected, terminal_error) {
        (Ok(expected), None) => {
            assert_eq!(expected.stdout, expected_stdout, "{name}");
            assert!(expected.debug_events.is_empty(), "{name}");
        }
        (Err(expected), Some(message)) => {
            assert_eq!(expected.message, message, "{name}");
            assert_eq!(expected.output.stdout, expected_stdout, "{name}");
            assert!(expected.output.debug_events.is_empty(), "{name}");
        }
        (expected, error) => panic!("{name}: expected error {error:?}, got {expected:?}"),
    }
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
        binaries.push(binary);
    }
    fs::remove_file(&source).unwrap();
    for binary in binaries {
        let actual = run_bounded(&binary, directory.path());
        assert_eq!(
            actual.status.code(),
            Some(if terminal_error.is_some() { 71 } else { 0 }),
            "{name}: {actual:?}"
        );
        assert_eq!(actual.stdout, expected_stdout.as_bytes(), "{name}");
        let stderr = terminal_error.map_or(String::new(), |error| format!("{error}\n"));
        assert_eq!(actual.stderr, stderr.as_bytes(), "{name}: {actual:?}");
    }
}

#[test]
fn native_secret_constructor_validation_observes_all_fields_in_lexical_order() {
    for (name, constructor, ordering) in [
        (
            "payload_first",
            "Packet(payload: payload(view stdout), kind: width(view stdout))",
            "payload\nwidth\n",
        ),
        (
            "width_first",
            "Packet(kind: width(view stdout), payload: payload(view stdout))",
            "width\npayload\n",
        ),
    ] {
        let statements = format!(
            "    secret[result[Packet, string]] attempt = {constructor}\n    result[Packet, string] exposed = declassify attempt\n    Packet checked = exposed handle error:\n        Stdout.write(view stdout, \"handled:{{error}}\\n\")\n        default Packet(kind: 255, payload: list(9))\n    Stdout.write(view stdout, \"ready:{{checked.kind}}:{{list.length(view checked.payload)}}\\n\")\n"
        );
        let stdout = format!(
            "before\n{ordering}handled:bitfield 'app.Packet' field 'kind' is 8 bit(s) wide and cannot hold '256'\nready:255:1\nafter:72657461696e6564:2\n"
        );
        execute_effect_case(name, &statements, &stdout, None);
    }
    execute_effect_case(
        "machine_handled_payload",
        "    optional[int64] candidate = none\n    secret[Snapshot at active] hidden = Snapshot(active, label(view stdout), payload(view stdout), handled_count(view stdout, candidate))\n    string text = declassify hidden.label\n    list[uint8] values = declassify hidden.payload\n    int64 count = declassify hidden.count\n    Stdout.write(view stdout, \"ready:{text}:{list.length(view values)}:{count}\\n\")\n",
        "before\nlabel\npayload\nfallback\nready:Ada:3:7\nafter:72657461696e6564:2\n",
        None,
    );
}

#[test]
fn native_secret_constructor_field_failures_suppress_later_effects_and_clean_up() {
    let message = "runtime error: list.__remove_at: index -1 out of bounds";
    for (name, statements, stdout) in [
        (
            "bitfield_field_failure",
            "    secret[result[Packet, string]] attempt = Packet(payload: payload(view stdout), kind: failure(view stdout))\n    result[Packet, string] exposed = declassify attempt\n    Packet checked = exposed handle error:\n        Stdout.write(view stdout, \"must not handle\\n\")\n        default Packet(kind: 0, payload: list())\n",
            "before\npayload\nfailure\n",
        ),
        (
            "bitfield_first_field_failure",
            "    secret[result[Packet, string]] attempt = Packet(kind: failure(view stdout), payload: payload(view stdout))\n    result[Packet, string] exposed = declassify attempt\n    Packet checked = exposed handle error:\n        Stdout.write(view stdout, \"must not handle\\n\")\n        default Packet(kind: 0, payload: list())\n",
            "before\nfailure\n",
        ),
        (
            "machine_field_failure",
            "    secret[Snapshot at active] hidden = Snapshot(active, label(view stdout), failing_payload(view stdout), later(view stdout))\n",
            "before\nlabel\nfailure\n",
        ),
    ] {
        execute_effect_case(name, statements, stdout, Some(message));
    }
}
