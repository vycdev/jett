use super::*;

const EQUALITY_ERROR: &str = "runtime error: Equatable.equals must return bool";

const MINIMAL_PENDING_RESULT: &str = r#"namespace app
struct Item:
    id: int64
implement Equatable for Item:
    function equals(view self: Item, view other: Item) returns bool:
        return run true
function main(stdout: Stdout) returns nothing:
    Item first = Item(id: 7)
    Item second = Item(id: 7)
    Stdout.write(view stdout, "before\n")
    bool same = first __OPERATOR__ second
    Stdout.write(view stdout, "value:{same}\n")
"#;

const ORDERED_FAILURE: &str = r#"namespace app
struct Item:
    answer: bool
    label: string
implement Equatable for Item:
    function equals(view self: Item, view other: Item) returns bool:
__METHOD_BODY__
function operand(view stdout: Stdout, label: string, answer: bool) returns Item:
    list[string] allocated = list(label, "owned")
    Stdout.write(view stdout, "{label}\n")
    return Item(answer: answer, label: "{label}:{list.length(view allocated)}")
function later(view stdout: Stdout) returns optional[bool]:
    Stdout.write(view stdout, "later\n")
    return none
function fallback(view stdout: Stdout) returns bool:
    Stdout.write(view stdout, "handler\n")
    return true
function short_effect(view stdout: Stdout) returns bool:
    Stdout.write(view stdout, "short\n")
    return true
function main(stdout: Stdout) returns nothing:
    bytes payload = bytes.from_string("earlier-owned-payload")
    list[string] retained = list("kept", "owned")
    Stdout.write(view stdout, "before\n")
__COMPARISON__
    Stdout.write(view stdout, "after:{bytes.to_hex(view payload)}:{list.length(view retained)}\n")
"#;

fn ordered_failure_source(body: &str, comparison: &str) -> String {
    ORDERED_FAILURE
        .replace("__METHOD_BODY__", body)
        .replace("__COMPARISON__", comparison)
}

fn interpolation_comparison(operator: &str, answer: &str) -> String {
    r#"    string report = "{operand(view stdout, "left", __ANSWER__) __OPERATOR__ operand(view stdout, "right", true)}:{later(view stdout) handle: default fallback(view stdout)}"
    Stdout.write(view stdout, "value:{report}\n")"#
        .replace("__ANSWER__", answer)
        .replace("__OPERATOR__", operator)
}

fn assert_equality_failure_in_both_profiles(
    name: &str,
    text: &str,
    stdout: &str,
    message: &str,
    debug: &[&str],
) {
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("main.jett");
    fs::write(&source, text).unwrap();
    let expected = jett_driver::run_file_capture_outcome(&source).expect_err(
        "implicit equality rejects a pending method result or preserves an earlier error",
    );
    assert_eq!(expected.output.stdout, stdout, "{name}");
    assert_eq!(expected.message, message, "{name}");
    assert_eq!(expected.output.debug_output, debug, "{name}");
    let mut binaries = Vec::new();
    for release in [false, true] {
        let binary = directory.path().join(format!("equality_{release}.exe"));
        jett_driver::native::build_host_executable_with_options(
            &source,
            launcher(),
            &binary,
            jett_driver::BuildOptions { release },
        )
        .unwrap_or_else(|error| panic!("{name}: {error}"));
        binaries.push((binary, release));
    }
    fs::remove_file(&source).unwrap();
    for (binary, release) in binaries {
        let actual = run_bounded(&binary, directory.path());
        assert_eq!(actual.status.code(), Some(71), "{name}: {actual:?}");
        assert_eq!(actual.stdout, stdout.as_bytes(), "{name}");
        let mut stderr = if release || debug.is_empty() {
            String::new()
        } else {
            format!("{}\n", debug.join("\n"))
        };
        stderr.push_str(message);
        stderr.push('\n');
        assert_eq!(actual.stderr, stderr.as_bytes(), "{name}: {actual:?}");
    }
}

#[test]
fn native_pending_equality_results_fail_for_both_operators_in_both_profiles() {
    for operator in ["==", "!="] {
        assert_equality_failure_in_both_profiles(
            operator,
            &MINIMAL_PENDING_RESULT.replace("__OPERATOR__", operator),
            "before\n",
            EQUALITY_ERROR,
            &[],
        );
    }
}

#[test]
fn native_equality_result_failures_preserve_depth_order_and_cleanup() {
    for operator in ["==", "!="] {
        for (depth, body) in [
            (
                "one",
                "        bool answer = self.answer\n        trace answer\n        return run answer",
            ),
            (
                "two",
                "        bool answer = self.answer\n        trace answer\n        return run run answer",
            ),
        ] {
            for answer in ["true", "false"] {
                let name = format!("{operator}_{depth}_{answer}");
                let trace = if answer == "true" {
                    "trace answer: bool = true"
                } else {
                    "trace answer: bool = false"
                };
                assert_equality_failure_in_both_profiles(
                    &name,
                    &ordered_failure_source(body, &interpolation_comparison(operator, answer)),
                    "before\nleft\nright\n",
                    EQUALITY_ERROR,
                    &[trace],
                );
            }
        }
        assert_equality_failure_in_both_profiles(
            &format!("{operator}_partial_join"),
            &ordered_failure_source(
                "        bool nested = run run self.answer\n        return join nested handle error:\n            return false",
                &interpolation_comparison(operator, "true"),
            ),
            "before\nleft\nright\n",
            EQUALITY_ERROR,
            &[],
        );
        assert_equality_failure_in_both_profiles(
            &format!("{operator}_method_failure"),
            &ordered_failure_source(
                "        list[string] owned = list(\"method-owned\")\n        list[string] removed = list.remove_at[string](owned, -1)\n        return run self.answer",
                &interpolation_comparison(operator, "true"),
            ),
            "before\nleft\nright\n",
            "runtime error: list.__remove_at: index -1 out of bounds",
            &[],
        );
    }

    for (operator, answer, short_operator) in [("==", "true", "and"), ("!=", "false", "or")] {
        let comparison = r#"    bool value = (operand(view stdout, "left", __ANSWER__) __OPERATOR__ operand(view stdout, "right", true)) __SHORT_OPERATOR__ short_effect(view stdout)
    Stdout.write(view stdout, "value:{value}\n")"#
            .replace("__ANSWER__", answer)
            .replace("__OPERATOR__", operator)
            .replace("__SHORT_OPERATOR__", short_operator);
        assert_equality_failure_in_both_profiles(
            &format!("{operator}_short_circuit"),
            &ordered_failure_source("        return run self.answer", &comparison),
            "before\nleft\nright\n",
            EQUALITY_ERROR,
            &[],
        );
    }

    for (operator, answer) in [("==", "true"), ("!=", "false")] {
        let comparison = r#"    Item first_source = operand(view stdout, "left", true)
    secret[Item] first = first_source
    Item second_source = operand(view stdout, "right", true)
    secret[Item] second = second_source
    bool value = declassify(first __OPERATOR__ second)
    Stdout.write(view stdout, "value:{value}:{later(view stdout) handle: default fallback(view stdout)}\n")"#
            .replace("__OPERATOR__", operator);
        let body = format!("        return run {answer}");
        assert_equality_failure_in_both_profiles(
            &format!("{operator}_secret_operands"),
            &ordered_failure_source(&body, &comparison),
            "before\nleft\nright\n",
            EQUALITY_ERROR,
            &[],
        );
    }
}

const READY_AND_EXPLICIT_CONTROLS: &str = r#"namespace models
export struct Item:
    id: int64
    label: string
implement Equatable for Item:
    function equals(view self: Item, view other: Item) returns bool:
        return run run(self.id == other.id)
export struct Ready:
    id: int64
implement Equatable for Ready:
    function equals(view self: Ready, view other: Ready) returns bool:
        return self.id == other.id
export struct Joined:
    id: int64
implement Equatable for Joined:
    function equals(view self: Joined, view other: Joined) returns bool:
        bool nested = run run(self.id == other.id)
        bool once = join nested handle error:
            return false
        return join once handle error:
            return false
export struct Holder:
    item: Ready
export function equal[T](view first: T, view second: T) returns bool:
    return first == second
export function reflected(view first: Holder, view second: Holder) returns bool:
    mutable bool same = true
    for field in type.fields[Holder]():
        comptime type Field = field.type_info:
            Field left = type.field_value[Holder, Field](view first, view field)
            Field right = type.field_value[Holder, Field](view second, view field)
            same = same and(left == right)
    return same
export function ready_report() returns string:
    Ready first = Ready(id: 7)
    Ready same = Ready(id: 7)
    Ready other = Ready(id: 9)
    return "{first == same}:{first == other}:{first != other}:{first != same}"
export function joined_report() returns string:
    Joined first = Joined(id: 7)
    Joined same = Joined(id: 7)
    Joined other = Joined(id: 9)
    return "{first == same}:{first == other}:{first != other}:{first != same}"
export function generic_report() returns string:
    Ready first = Ready(id: 7)
    Ready same = Ready(id: 7)
    Joined joined = Joined(id: 7)
    Joined matching = Joined(id: 7)
    return "{equal[Ready](view first, view same)}:{equal[Joined](view joined, view matching)}"
export function reflection_report() returns string:
    Holder first = Holder(item: Ready(id: 7))
    Holder same = Holder(item: Ready(id: 7))
    Holder other = Holder(item: Ready(id: 9))
    return "{reflected(view first, view same)}:{reflected(view first, view other)}"
export function baked() returns bool:
    Ready first = Ready(id: 7)
    Ready same = Ready(id: 7)
    Joined joined = Joined(id: 7)
    Joined matching = Joined(id: 7)
    return first == same and joined == matching
namespace app
function explicit_calls(view stdout: Stdout) returns nothing:
    use models
    models.Item first = models.Item(id: 7, label: "first")
    models.Item same = models.Item(id: 7, label: "same")
    models.Item other = models.Item(id: 9, label: "other")
    bool pending_same = models.Item.equals(view first, view same)
    bool pending_other = models.Item.equals(view first, view other)
    trace pending_same
    trace pending_other
    Stdout.write(view stdout, "explicit-pending:{pending_same}:{pending_other}\n")
    bool once_same = join pending_same handle error:
        return nothing
    bool once_other = join pending_other handle error:
        return nothing
    trace once_same
    trace once_other
    Stdout.write(view stdout, "explicit-partial:{once_same}:{once_other}\n")
    bool ready_same = join once_same handle error:
        return nothing
    bool ready_other = join once_other handle error:
        return nothing
    trace ready_same
    trace ready_other
    Stdout.write(view stdout, "explicit-ready:{ready_same}:{ready_other}\n")
    Stdout.write(view stdout, "owners:{first.id}:{same.id}:{other.id}:{first.label}:{same.label}:{other.label}\n")
function main(stdout: Stdout) returns nothing:
    use models
    explicit_calls(view stdout)
    Stdout.write(view stdout, "ready:{models.ready_report()}\n")
    Stdout.write(view stdout, "joined:{models.joined_report()}\n")
    Stdout.write(view stdout, "generic:{models.generic_report()}\n")
    Stdout.write(view stdout, "reflected:{models.reflection_report()}\n")
    bool baked = comptime models.baked()
    Stdout.write(view stdout, "comptime:{baked}\n")
verify equality_controls:
    use models
    assert models.ready_report() == "true:false:true:false"
    assert models.joined_report() == "true:false:true:false"
    assert models.generic_report() == "true:true"
    assert models.reflection_report() == "true:false"
    bool baked = comptime models.baked()
    assert baked
property equality_controls_repeat:
    given seed: int64
    use models
    assert models.ready_report() == "true:false:true:false"
    assert models.joined_report() == "true:false:true:false"
    assert models.generic_report() == "true:true"
    assert models.reflection_report() == "true:false"
    assert models.baked()
    assert seed == seed
"#;

#[test]
fn native_ready_equality_and_explicit_pending_calls_match_reference_in_both_profiles() {
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("main.jett");
    fs::write(&source, READY_AND_EXPLICIT_CONTROLS).unwrap();
    let expected = jett_driver::run_file_capture_output(&source)
        .expect("ready implicit equality and ordinary explicit pending-bool calls remain valid");
    assert_eq!(
        expected.stdout,
        concat!(
            "explicit-pending:pending(pending(true)):pending(pending(false))\n",
            "explicit-partial:pending(true):pending(false)\n",
            "explicit-ready:true:false\n",
            "owners:7:7:9:first:same:other\n",
            "ready:true:false:true:false\n",
            "joined:true:false:true:false\n",
            "generic:true:true\n",
            "reflected:true:false\n",
            "comptime:true\n",
        )
    );
    assert_eq!(
        expected.debug_output,
        [
            "trace pending_same: bool = pending(pending(true))",
            "trace pending_other: bool = pending(pending(false))",
            "trace once_same: bool = pending(true)",
            "trace once_other: bool = pending(false)",
            "trace ready_same: bool = true",
            "trace ready_other: bool = false",
        ]
    );
    let mut binaries = Vec::new();
    for release in [false, true] {
        let binary = directory
            .path()
            .join(format!("ready_equality_{release}.exe"));
        jett_driver::native::build_host_executable_with_options(
            &source,
            launcher(),
            &binary,
            jett_driver::BuildOptions { release },
        )
        .expect("native implicit ready equality and explicit pending-bool calls");
        binaries.push((binary, release));
    }
    let verify_binary = directory.path().join("equality_verify.exe");
    build_host_verify_suite_executable(&source, launcher(), &verify_binary)
        .expect("compiled equality verify suite");
    let property_binary = directory.path().join("equality_property.exe");
    build_host_property_suite_executable(&source, launcher(), &property_binary)
        .expect("compiled equality property suite");
    fs::remove_file(&source).unwrap();
    for (binary, release) in binaries {
        let actual = run_bounded(&binary, directory.path());
        assert!(actual.status.success(), "{actual:?}");
        assert_eq!(actual.stdout, expected.stdout.as_bytes());
        let debug = if release {
            String::new()
        } else {
            format!("{}\n", expected.debug_output.join("\n"))
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
