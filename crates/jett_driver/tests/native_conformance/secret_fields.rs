use super::*;

const FIELD_CONTROLS: &str = r#"namespace app
interface Named:
    function name(view self: Named) returns string
implement Named for int8:
    function name(view self: int8) returns string:
        return "small:{self}"
type Selected = Named where true
implement Named for Selected:
    function name(view self: Selected) returns string:
        Named base = coarsen clone self
        return "selected:{Named.name(view base)}"
function selected(value: int8) returns Selected:
    Named base = value
    Selected chosen = base handle error:
        return selected(value)
    return chosen
type Callback = function(int8) returns int8
function captured(offset: int8) returns Callback:
    return function(value: int8) returns int8: return value + offset
struct Item:
    number: int8
    text: string
    items: list[int8]
    owner: Named
    callback: Callback
    token: secret[string]
    marker: nothing
struct Box[T]:
    value: T
type Positive = secret[int8] where value > 0
struct Nominal:
    amount: Positive
bitfield Header:
    version: 4 bits
    length: 8 bits
machine Session:
    states:
        active(label: string, number: int8)
        closed
    transitions:
        active to closed
function seed() returns Item:
    return Item(number: -128, text: "Ada-λ🙂", items: list(-128, 127), owner: selected(7), callback: captured(3), token: "token", marker: nothing)
function marker_name(value: nothing) returns string:
    return "nothing"
function plain_summary(view source: Item) returns string:
    list[int8] items = source.items
    Named owner = source.owner
    Callback callback = source.callback
    string token = declassify source.token
    nothing marker = source.marker
    return "{source.number}:{source.text}:{list.length(view items)}:{Named.name(view owner)}:{callback(4)}:{token}:{marker_name(marker)}"
function secret_summary(view source: secret[Item]) returns string:
    int8 number = declassify source.number
    string text = declassify source.text
    list[int8] items = declassify source.items
    Named owner = declassify source.owner
    Callback callback = declassify source.callback
    string token = declassify source.token
    nothing marker = source.marker
    return "{number}:{text}:{list.length(view items)}:{Named.name(view owner)}:{callback(4)}:{token}:{marker_name(marker)}"
function direct_report() returns string:
    Item original = seed()
    secret[Item] hidden = clone original
    secret[Item] copied = clone hidden
    return "{plain_summary(view original)}|{secret_summary(view hidden)}|{secret_summary(view copied)}"
function generic_field[T](view source: secret[Box[T]]) returns secret[T]:
    return source.value
function generic_report() returns string:
    Box[int8] plain_number = Box[int8](value: -128)
    secret[Box[int8]] number = plain_number
    Box[string] plain_text = Box[string](value: "generic-λ")
    secret[Box[string]] text = plain_text
    int8 answer = declassify generic_field[int8](view number)
    string label = declassify generic_field[string](view text)
    return "{answer}:{label}"
function reflected_report() returns string:
    Item source = seed()
    TypeField field = list.get[TypeField](type.fields[Item](), 5) handle:
        return "missing metadata"
    secret[string] token = type.field_value[Item, secret[string]](view source, view field)
    string visible = declassify token
    secret[Item] hidden = clone source
    string direct = declassify hidden.token
    return "{visible}:{direct}:{plain_summary(view source)}"
function nominal_report() returns string:
    secret[int8] positive = 7
    Nominal source = Nominal(amount: positive) handle error:
        return error
    Positive ordinary = source.amount
    secret[Nominal] hidden = clone source
    Positive copied = hidden.amount
    secret[int8] first = coarsen ordinary
    secret[int8] second = coarsen copied
    int8 first_value = declassify first
    int8 second_value = declassify second
    return "{first_value}:{second_value}"
function other_owners_report() returns string:
    Header header = Header(version: 5, length: 127)
    secret[Header] hidden_header = clone header
    int64 version = declassify hidden_header.version
    int64 length = declassify hidden_header.length
    Session at active session = Session(active, "session-λ", 7)
    secret[Session at active] hidden_session = clone session
    string label = declassify hidden_session.label
    int8 number = declassify hidden_session.number
    return "{header.version}:{header.length}:{version}:{length}|{session.label}:{session.number}:{label}:{number}"
function copy_items() returns secret[list[int8]]:
    Item source = seed()
    secret[Item] hidden = clone source
    return hidden.items
function copy_owner() returns secret[Named]:
    Item source = seed()
    secret[Item] hidden = clone source
    return hidden.owner
function copy_callback() returns secret[Callback]:
    Item source = seed()
    secret[Item] hidden = clone source
    return hidden.callback
function clone_report() returns string:
    Item source = seed()
    secret[Item] hidden = clone source
    secret[list[int8]] escaped = copy_items()
    mutable list[int8] independent = declassify escaped
    independent = list.append[int8](independent, 9)
    Named owner = declassify copy_owner()
    Callback callback = declassify copy_callback()
    return "{list.length(view independent)}:{list.length(view source.items)}:{Named.name(view owner)}:{callback(4)}|{secret_summary(view hidden)}|{plain_summary(view source)}"
function double_joined[T](candidate: T) returns result[T, string]:
    T once = join candidate handle error:
        return fail(error)
    T ready = join once handle error:
        return fail(error)
    return ok(ready)
function pending_report() returns string:
    Item source = Item(number: run run 7, text: run run "λ🙂", items: run run list(-128, 127), owner: run run selected(7), callback: run run captured(3), token: run run "token", marker: nothing)
    secret[Item] hidden = source
    int8 pending_number = declassify hidden.number
    int8 once_number = join pending_number handle error:
        return error
    int8 number = join once_number handle error:
        return error
    string pending_text = declassify hidden.text
    string once_text = join pending_text handle error:
        return error
    string text = join once_text handle error:
        return error
    list[int8] pending_items = declassify hidden.items
    list[int8] items = double_joined[list[int8]](pending_items) handle error:
        return error
    Named pending_owner = declassify hidden.owner
    Named owner = double_joined[Named](pending_owner) handle error:
        return error
    Callback pending_callback = declassify hidden.callback
    Callback callback = double_joined[Callback](pending_callback) handle error:
        return error
    string pending_token = declassify hidden.token
    string token = double_joined[string](pending_token) handle error:
        return error
    return "{pending_number}:{once_number}:{number}|{pending_text}:{once_text}:{text}|{list.length(view items)}:{Named.name(view owner)}:{callback(4)}:{token}"
function main(stdout: Stdout) returns nothing:
    Stdout.write(view stdout, "direct:{direct_report()}\n")
    Stdout.write(view stdout, "generic:{generic_report()}\n")
    Stdout.write(view stdout, "reflected:{reflected_report()}\n")
    Stdout.write(view stdout, "nominal:{nominal_report()}\n")
    Stdout.write(view stdout, "owners:{other_owners_report()}\n")
    Stdout.write(view stdout, "clone:{clone_report()}\n")
    Stdout.write(view stdout, "pending:{pending_report()}\n")
    string baked = comptime direct_report()
    Stdout.write(view stdout, "comptime:{baked}\n")
verify secret_field_values:
    assert direct_report() == "-128:Ada-λ🙂:2:selected:small:7:7:token:nothing|-128:Ada-λ🙂:2:selected:small:7:7:token:nothing|-128:Ada-λ🙂:2:selected:small:7:7:token:nothing"
    assert generic_report() == "-128:generic-λ"
    assert reflected_report() == "token:token:-128:Ada-λ🙂:2:selected:small:7:7:token:nothing"
    assert nominal_report() == "7:7"
    assert other_owners_report() == "5:127:5:127|session-λ:7:session-λ:7"
    assert pending_report() == "pending(pending(7)):pending(7):7|pending(pending(λ🙂)):pending(λ🙂):λ🙂|2:selected:small:7:7:token"
    assert clone_report() == "3:2:selected:small:7:7|-128:Ada-λ🙂:2:selected:small:7:7:token:nothing|-128:Ada-λ🙂:2:selected:small:7:7:token:nothing"
    string baked = comptime direct_report()
    assert baked == direct_report()
property secret_field_trials:
    given value: int8
    Box[int8] source = Box[int8](value: value)
    secret[Box[int8]] hidden = source
    int8 copied = declassify generic_field[int8](view hidden)
    assert copied == value
    assert clone_report() == "3:2:selected:small:7:7|-128:Ada-λ🙂:2:selected:small:7:7:token:nothing|-128:Ada-λ🙂:2:selected:small:7:7:token:nothing"
"#;

#[test]
fn native_secret_field_reads_preserve_qualified_values_and_owners_in_both_profiles() {
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("main.jett");
    fs::write(&source, FIELD_CONTROLS).unwrap();
    let expected = jett_driver::run_file_capture_output(&source)
        .expect("secret field reads and owned field copies remain accepted");
    assert_eq!(
        expected.stdout,
        concat!(
            "direct:-128:Ada-λ🙂:2:selected:small:7:7:token:nothing|-128:Ada-λ🙂:2:selected:small:7:7:token:nothing|-128:Ada-λ🙂:2:selected:small:7:7:token:nothing\n",
            "generic:-128:generic-λ\n",
            "reflected:token:token:-128:Ada-λ🙂:2:selected:small:7:7:token:nothing\n",
            "nominal:7:7\n",
            "owners:5:127:5:127|session-λ:7:session-λ:7\n",
            "clone:3:2:selected:small:7:7|-128:Ada-λ🙂:2:selected:small:7:7:token:nothing|-128:Ada-λ🙂:2:selected:small:7:7:token:nothing\n",
            "pending:pending(pending(7)):pending(7):7|pending(pending(λ🙂)):pending(λ🙂):λ🙂|2:selected:small:7:7:token\n",
            "comptime:-128:Ada-λ🙂:2:selected:small:7:7:token:nothing|-128:Ada-λ🙂:2:selected:small:7:7:token:nothing|-128:Ada-λ🙂:2:selected:small:7:7:token:nothing\n",
        )
    );
    assert!(expected.debug_output.is_empty(), "{expected:?}");
    let mut binaries = Vec::new();
    for release in [false, true] {
        let binary = directory
            .path()
            .join(format!("secret_fields_{release}.exe"));
        jett_driver::native::build_host_executable_with_options(
            &source,
            launcher(),
            &binary,
            jett_driver::BuildOptions { release },
        )
        .expect("native exact field qualification");
        binaries.push(binary);
    }
    let verify_binary = directory.path().join("secret_fields_verify.exe");
    build_host_verify_suite_executable(&source, launcher(), &verify_binary)
        .expect("compiled secret field verify suite");
    let property_binary = directory.path().join("secret_fields_property.exe");
    build_host_property_suite_executable(&source, launcher(), &property_binary)
        .expect("compiled secret field property suite");
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

const FAILURE_DECLARATIONS: &str = r#"namespace app
struct Item:
    value: int64
function failing_owner(view stdout: Stdout) returns secret[Item]:
    Stdout.write(view stdout, "receiver\n")
    list[int64] owned = list(1, 2)
    list[int64] failed = list.remove_at[int64](owned, -1)
    return Item(value: list.length(view failed))
"#;

fn assert_failure(
    name: &str,
    body: &str,
    stdout: &str,
    reference_message: &str,
    native_message: &str,
) {
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("main.jett");
    fs::write(
        &source,
        format!(
            "{FAILURE_DECLARATIONS}function main(stdout: Stdout) returns nothing:\n    bytes earlier = bytes.from_string(\"retained\")\n    list[string] retained = list(\"earlier\", \"owner\")\n    Stdout.write(view stdout, \"before\\n\")\n{body}    Stdout.write(view stdout, \"must not run:{{bytes.to_hex(view earlier)}}:{{list.length(view retained)}}\")\n"
        ),
    )
    .unwrap();
    let expected = jett_driver::run_file_capture_outcome(&source)
        .expect_err("secret field access must preserve the first runtime failure");
    assert_eq!(expected.message, reference_message, "{name}");
    assert_eq!(expected.output.stdout, stdout, "{name}");
    assert!(expected.output.debug_output.is_empty(), "{name}");
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
        assert_eq!(actual.status.code(), Some(71), "{name}: {actual:?}");
        assert_eq!(actual.stdout, stdout.as_bytes(), "{name}");
        assert_eq!(
            actual.stderr,
            format!("{native_message}\n").as_bytes(),
            "{name}: {actual:?}"
        );
    }
}

#[test]
fn native_pending_secret_field_children_preserve_depth_and_first_error_in_both_profiles() {
    for (name, pending, join_body, stdout, message) in [
        (
            "pending_child",
            "run 7",
            "    Stdout.write(view stdout, \"copied:{copied}\\n\")\n    int64 answer = copied + 1\n",
            "before\ncopied:pending(7)\n",
            "runtime error: unsupported binary operation: pending(7) Add 1",
        ),
        (
            "partial_child",
            "run run 7",
            "    Stdout.write(view stdout, \"copied:{copied}\\n\")\n    int64 once = join copied handle error:\n        return nothing\n    Stdout.write(view stdout, \"joined:{once}\\n\")\n    int64 answer = once + 1\n",
            "before\ncopied:pending(pending(7))\njoined:pending(7)\n",
            "runtime error: unsupported binary operation: pending(7) Add 1",
        ),
    ] {
        let body = format!(
            "    Item original = Item(value: {pending})\n    secret[Item] hidden = original\n    int64 copied = declassify hidden.value\n{join_body}"
        );
        assert_failure(name, &body, stdout, message, message);
    }
    let message = "runtime error: list.__remove_at: index -1 out of bounds";
    assert_failure(
        "receiver_failure",
        "    int64 copied = declassify failing_owner(view stdout).value\n",
        "before\nreceiver\n",
        message,
        message,
    );
}

#[test]
fn native_pending_secret_field_owners_keep_typed_redaction_in_both_profiles() {
    // Reference field-access errors currently disclose the owner payload. The
    // hidden-secret observation policy remains open; native typed redaction is
    // deliberate and must not be weakened to reproduce that disclosure.
    for (name, setup, pending, reference_message) in [
        (
            "pending_owner",
            "",
            "run clone original",
            "runtime error: field access is not supported on pending(app.Item(value: 7))",
        ),
        (
            "nested_pending_owner",
            "",
            "run run clone original",
            "runtime error: field access is not supported on pending(pending(app.Item(value: 7)))",
        ),
        (
            "partial_pending_owner",
            "    secret[Item] twice = run run clone original\n",
            "join twice handle error:\n        return nothing",
            "runtime error: field access is not supported on pending(app.Item(value: 7))",
        ),
    ] {
        let body = format!(
            "    Item original = Item(value: 7)\n{setup}    secret[Item] hidden = {pending}\n    int64 copied = declassify hidden.value\n"
        );
        assert_failure(
            name,
            &body,
            "before\n",
            reference_message,
            "runtime error: field access is not supported on [redacted]",
        );
    }
}
