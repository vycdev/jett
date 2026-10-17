use super::*;

const SUCCESS_CONTROLS: &str = r#"namespace models
export interface Named:
    function name(view self: Named) returns string
implement Named for int8:
    function name(view self: int8) returns string:
        return "small:{self}"
export struct Counter:
    value: int8
implement Named for Counter:
    function name(view self: Counter) returns string:
        return "record:{self.value}"
export type Selected = Named where true
implement Named for Selected:
    function name(view self: Selected) returns string:
        Named base = coarsen clone self
        return "selected:{Named.name(view base)}"
function selected(value: int8) returns Selected:
    Named base = value
    Selected chosen = base handle error:
        return selected(value)
    return chosen
type Callback = function(int8) returns secret[int8]
function classified(value: secret[int8]) returns secret[int8]:
    return value
function captured(offset: int8) returns function(secret[int8]) returns secret[int8]:
    return function(value: secret[int8]) returns secret[int8]: return value + offset
export struct Item:
    count: int8
    labels: list[string]
    owner: Named
    callback: Callback
export struct Box[T]:
    value: T
export function make[T](value: T) returns secret[Box[T]]:
    return Box[T](value: value)
function label(view values: list[string]) returns string:
    return list.get[string](view values, 0) handle: default "missing"
function describe(view source: Item) returns string:
    Callback callback = source.callback
    secret[int8] hidden = callback(4)
    int8 answer = declassify hidden
    return "{source.count}:{list.length(view source.labels)}:{label(view source.labels)}:{Named.name(view source.owner)}:{answer}"
export function read_hidden(view source: secret[Item]) returns string:
    Item visible = declassify clone source
    return describe(view visible)
function public_item(count: int8) returns Item:
    return Item(count: count, labels: list("public", "kept"), owner: Counter(value: count), callback: classified)
function make_item(count: int8) returns secret[Item]:
    return Item(count: count, labels: list("return"), owner: selected(count), callback: classified)
export function direct_report() returns string:
    secret[Item] direct = Item(count: -128, labels: list("direct", "kept"), owner: selected(7), callback: classified)
    secret[Item] parenthesized = (Item(count: 127, labels: list("paren"), owner: Counter(value: 8), callback: classified))
    secret[secret[Item]] nested = Item(count: -1, labels: list("nested"), owner: selected(9), callback: classified)
    secret[Item] once = declassify clone nested
    return "{read_hidden(view direct)}|{read_hidden(view parenthesized)}|{read_hidden(view once)}"
export function producer_report() returns string:
    Item original = public_item(11)
    secret[Item] lifted = clone original
    secret[Item] from_call = public_item(12)
    return "{describe(view original)}|{read_hidden(view lifted)}|{read_hidden(view from_call)}"
export function generic_report() returns string:
    secret[Box[int8]] number = Box[int8](value: -128)
    secret[Box[Named]] owner = Box[Named](value: selected(7))
    secret[Box[int8]] returned = make[int8](127)
    secret[Box[Named]] tagged = make[Named](Counter(value: 8))
    Box[int8] plain_number = declassify clone number
    Box[Named] plain_owner = declassify clone owner
    Box[int8] plain_returned = declassify clone returned
    Box[Named] plain_tagged = declassify clone tagged
    return "{plain_number.value}:{Named.name(view plain_owner.value)}:{plain_returned.value}:{Named.name(view plain_tagged.value)}"
export function return_report() returns string:
    secret[Item] returned = make_item(7)
    int8 offset = 3
    function(int8) returns secret[Item] maker = function(value: int8) returns secret[Item]:
        return Item(count: value, labels: list("inline"), owner: Counter(value: value), callback: captured(offset))
    secret[Item] inline = maker(8)
    return "{read_hidden(view returned)}|{read_hidden(view inline)}"
export function pending_report() returns string:
    secret[Item] pending = run run Item(count: 7, labels: list("pending"), owner: selected(7), callback: classified)
    secret[Item] once = join pending handle error:
        return error
    secret[Item] ready = join once handle error:
        return error
    return read_hidden(view ready)
export function baked_report() returns string:
    secret[Item] hidden = comptime Item(count: -8, labels: list("baked"), owner: selected(9), callback: classified)
    secret[Item] returned = comptime make_item(10)
    return "{read_hidden(view hidden)}|{read_hidden(view returned)}"
export function clone_report() returns string:
    secret[Item] hidden = Item(count: 7, labels: list("clone", "kept"), owner: Counter(value: 8), callback: captured(3))
    Item visible = declassify clone hidden
    mutable list[string] copied = visible.labels
    copied = list.append[string](copied, "independent")
    return "{list.length(view copied)}:{list.length(view visible.labels)}|{describe(view visible)}|{read_hidden(view hidden)}"
namespace app
function main(stdout: Stdout) returns nothing:
    use models
    Stdout.write(view stdout, "direct:{models.direct_report()}\n")
    Stdout.write(view stdout, "producers:{models.producer_report()}\n")
    Stdout.write(view stdout, "generic:{models.generic_report()}\n")
    Stdout.write(view stdout, "returns:{models.return_report()}\n")
    Stdout.write(view stdout, "pending:{models.pending_report()}\n")
    Stdout.write(view stdout, "baked:{models.baked_report()}\n")
    Stdout.write(view stdout, "clone:{models.clone_report()}\n")
    secret[models.Item] observed = models.Item(count: 7, labels: list("observed"), owner: models.Counter(value: 8), callback: function(value: int8) returns secret[int8]: return value)
    trace observed
    Stdout.write(view stdout, "observed:{models.read_hidden(view observed)}\n")
verify secret_struct_values:
    use models
    assert models.direct_report() == "-128:2:direct:selected:small:7:4|127:1:paren:record:8:4|-1:1:nested:selected:small:9:4"
    assert models.producer_report() == "11:2:public:record:11:4|11:2:public:record:11:4|12:2:public:record:12:4"
    assert models.generic_report() == "-128:selected:small:7:127:record:8"
    assert models.return_report() == "7:1:return:selected:small:7:4|8:1:inline:record:8:7"
    assert models.pending_report() == "7:1:pending:selected:small:7:4"
    assert models.baked_report() == "-8:1:baked:selected:small:9:4|10:1:return:selected:small:10:4"
    assert models.clone_report() == "3:2|7:2:clone:record:8:7|7:2:clone:record:8:7"
property secret_struct_trials:
    given seed: int8
    use models
    secret[models.Box[int8]] hidden = models.Box[int8](value: seed)
    models.Box[int8] visible = declassify clone hidden
    assert visible.value == seed
    secret[models.Box[int8]] returned = models.make[int8](seed)
    models.Box[int8] plain = declassify clone returned
    assert plain.value == seed
    assert models.generic_report() == "-128:selected:small:7:127:record:8"
    assert models.clone_report() == "3:2|7:2:clone:record:8:7|7:2:clone:record:8:7"
"#;

#[test]
fn native_secret_struct_constructors_match_reference_in_both_profiles() {
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("main.jett");
    fs::write(&source, SUCCESS_CONTROLS).unwrap();
    let expected = jett_driver::run_file_capture_output(&source)
        .expect("accepted contextual secret struct constructors");
    assert_eq!(
        expected.stdout,
        concat!(
            "direct:-128:2:direct:selected:small:7:4|127:1:paren:record:8:4|-1:1:nested:selected:small:9:4\n",
            "producers:11:2:public:record:11:4|11:2:public:record:11:4|12:2:public:record:12:4\n",
            "generic:-128:selected:small:7:127:record:8\n",
            "returns:7:1:return:selected:small:7:4|8:1:inline:record:8:7\n",
            "pending:7:1:pending:selected:small:7:4\n",
            "baked:-8:1:baked:selected:small:9:4|10:1:return:selected:small:10:4\n",
            "clone:3:2|7:2:clone:record:8:7|7:2:clone:record:8:7\n",
            "observed:7:1:observed:record:8:4\n",
        )
    );
    assert_eq!(
        debug_trace_lines(&expected.debug_events),
        ["trace observed: secret[models.Item] = [redacted]"]
    );
    let mut binaries = Vec::new();
    for release in [false, true] {
        let binary = directory
            .path()
            .join(format!("secret_struct_{release}.exe"));
        jett_driver::native::build_host_executable_with_options(
            &source,
            launcher(),
            &binary,
            jett_driver::BuildOptions { release },
        )
        .expect("native contextual secret struct constructors");
        binaries.push((binary, release));
    }
    let verify_binary = directory.path().join("secret_struct_verify.exe");
    build_host_verify_suite_executable(&source, launcher(), &verify_binary)
        .expect("compiled secret struct verify suite");
    let property_binary = directory.path().join("secret_struct_property.exe");
    build_host_property_suite_executable(&source, launcher(), &property_binary)
        .expect("compiled secret struct property suite");
    fs::remove_file(&source).unwrap();
    for (binary, release) in binaries {
        let actual = run_bounded(&binary, directory.path());
        assert!(actual.status.success(), "{actual:?}");
        assert_eq!(actual.stdout, expected.stdout.as_bytes());
        let debug = if release {
            String::new()
        } else {
            jett_driver::render_debug_events(&expected.debug_events)
        };
        assert_eq!(actual.stderr, debug.as_bytes(), "{actual:?}");
    }
    for binary in [verify_binary, property_binary] {
        let actual = run_bounded(&binary, directory.path());
        assert!(actual.status.success(), "{actual:?}");
        assert!(actual.stdout.is_empty(), "{actual:?}");
        assert!(actual.stderr.is_empty(), "{actual:?}");
    }
}

const FIELD_FAILURE: &str = r#"namespace app
struct Packet:
    first: list[string]
    second: list[string]
function first(view stdout: Stdout) returns list[string]:
    Stdout.write(view stdout, "first\n")
    return list("staged", "owned")
function failing(view stdout: Stdout) returns list[string]:
    Stdout.write(view stdout, "failure\n")
    list[string] owned = list("failure", "owner")
    return list.remove_at[string](owned, -1)
function make(view stdout: Stdout) returns __TYPE__:
    return __CONSTRUCTOR__
function main(stdout: Stdout) returns nothing:
    bytes earlier = bytes.from_string("retained")
    list[string] retained = list("earlier", "owner")
    Stdout.write(view stdout, "before\n")
    __TYPE__ hidden = make(view stdout)
    Stdout.write(view stdout, "must not run:{bytes.to_hex(view earlier)}:{list.length(view retained)}")
"#;

#[test]
fn native_secret_struct_field_failures_preserve_order_and_cleanup_in_both_profiles() {
    for (name, ty, constructor) in [
        (
            "returned",
            "secret[Packet]",
            "Packet(second: first(view stdout), first: failing(view stdout))",
        ),
        (
            "parenthesized",
            "secret[Packet]",
            "(Packet(second: first(view stdout), first: failing(view stdout)))",
        ),
        (
            "nested",
            "secret[secret[Packet]]",
            "Packet(second: first(view stdout), first: failing(view stdout))",
        ),
    ] {
        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join("main.jett");
        fs::write(
            &source,
            FIELD_FAILURE
                .replace("__TYPE__", ty)
                .replace("__CONSTRUCTOR__", constructor),
        )
        .unwrap();
        let expected = jett_driver::run_file_capture_outcome(&source)
            .expect_err("later field failure prevents secret record construction");
        assert_eq!(expected.output.stdout, "before\nfirst\nfailure\n", "{name}");
        assert!(expected.output.debug_events.is_empty(), "{name}");
        assert_eq!(
            expected.message, "runtime error: list.__remove_at: index -1 out of bounds",
            "{name}"
        );
        let mut binaries = Vec::new();
        for release in [false, true] {
            let binary = directory
                .path()
                .join(format!("field_failure_{release}.exe"));
            jett_driver::native::build_host_executable_with_options(
                &source,
                launcher(),
                &binary,
                jett_driver::BuildOptions { release },
            )
            .unwrap_or_else(|error| panic!("{name}: {error}"));
            binaries.push(binary);
        }
        fs::remove_file(&source).unwrap();
        for binary in binaries {
            let actual = run_bounded(&binary, directory.path());
            assert_eq!(actual.status.code(), Some(71), "{name}: {actual:?}");
            assert_eq!(actual.stdout, expected.output.stdout.as_bytes(), "{name}");
            assert_eq!(
                actual.stderr,
                format!("{}\n", expected.message).as_bytes(),
                "{name}: {actual:?}"
            );
        }
    }
}
