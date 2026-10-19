use super::*;

const MINIMAL_PENDING_RESULT: &str = r#"interface Displayable:
    function display(view self: Displayable) returns string
namespace app
struct Message:
    text: string
implement Displayable for Message:
    function display(view self: Message) returns string:
        return run self.text
function main(stdout: Stdout) returns nothing:
    Message value = Message(text: "shown")
    Stdout.write(view stdout, "before\n")
    Stdout.write(view stdout, "value:{value}\n")
"#;

const ORDERED_FAILURE: &str = r#"interface Displayable:
    function display(view self: Displayable) returns string
namespace app
__DECLARATIONS__
function prefix(view stdout: Stdout) returns string:
    list[string] allocated = list("earlier", "owned")
    Stdout.write(view stdout, "prefix\n")
    return "allocated:{list.length(view allocated)}"
function later(view stdout: Stdout) returns optional[int64]:
    Stdout.write(view stdout, "later\n")
    return none
function fallback(view stdout: Stdout) returns int64:
    Stdout.write(view stdout, "handler\n")
    return 7
function main(stdout: Stdout) returns nothing:
    bytes payload = bytes.from_string("retained-payload")
    list[string] retained = list("kept", "owned")
    __VALUE_DECLARATION__
    Stdout.write(view stdout, "before\n")
    string message = "{prefix(view stdout)}:{value}:{later(view stdout) handle: default fallback(view stdout)}"
    Stdout.write(view stdout, "value:{message}\n")
    Stdout.write(view stdout, "after:{bytes.to_hex(view payload)}:{list.length(view retained)}\n")
"#;

fn message_declarations(body: &str) -> String {
    format!(
        "struct Message:\n    text: string\nimplement Displayable for Message:\n    function display(view self: Message) returns string:\n{body}\n"
    )
}

fn ordered_failure_source(declarations: &str, value: &str) -> String {
    ORDERED_FAILURE
        .replace("__DECLARATIONS__", declarations)
        .replace("__VALUE_DECLARATION__", value)
}

fn assert_display_failure_in_both_profiles(
    name: &str,
    text: &str,
    stdout: &str,
    message: &str,
    debug: &[&str],
) {
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("main.jett");
    fs::write(&source, text).unwrap();
    let expected = jett_driver::run_file_capture_outcome(&source)
        .expect_err("implicit display must fail before later interpolation effects");
    assert_eq!(expected.output.stdout, stdout, "{name}");
    assert_eq!(expected.message, message, "{name}");
    assert_eq!(
        debug_trace_lines(&expected.output.debug_events),
        debug,
        "{name}"
    );
    let mut binaries = Vec::new();
    for release in [false, true] {
        let binary = directory.path().join(format!("display_{release}.exe"));
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
            jett_driver::render_debug_events(&expected.output.debug_events)
        };
        stderr.push_str(message);
        stderr.push('\n');
        assert_eq!(actual.stderr, stderr.as_bytes(), "{name}: {actual:?}");
    }
}

#[test]
fn native_pending_display_result_fails_in_both_profiles() {
    assert_display_failure_in_both_profiles(
        "pending_result",
        MINIMAL_PENDING_RESULT,
        "before\n",
        "runtime error: Displayable.display returned pending(shown) instead of string",
        &[],
    );
}

#[test]
fn native_display_result_failures_preserve_depth_order_and_cleanup() {
    let cases = [
        (
            "double_unicode",
            message_declarations(
                "        string shown = self.text\n        trace shown\n        return run run shown",
            ),
            "Message value = Message(text: \"shown-λ🙂\")",
            "runtime error: Displayable.display returned pending(pending(shown-λ🙂)) instead of string",
            vec!["trace shown: string = shown-λ🙂"],
        ),
        (
            "partial_join",
            message_declarations(
                "        string nested = run run self.text\n        return join nested handle error:\n            return error",
            ),
            "Message value = Message(text: \"shown-λ🙂\")",
            "runtime error: Displayable.display returned pending(shown-λ🙂) instead of string",
            vec![],
        ),
        (
            "empty",
            message_declarations("        return run self.text"),
            "Message value = Message(text: \"\")",
            "runtime error: Displayable.display returned pending() instead of string",
            vec![],
        ),
        (
            "primitive_owner",
            "implement Displayable for bool:\n    function display(view self: bool) returns string:\n        return run \"primitive-λ\"\n".to_owned(),
            "bool value = true",
            "runtime error: Displayable.display returned pending(primitive-λ) instead of string",
            vec![],
        ),
        (
            "interface_owner",
            "interface Named:\n    function name(view self: Named) returns string\nstruct Message:\n    text: string\nimplement Named for Message:\n    function name(view self: Message) returns string:\n        return self.text\nimplement Displayable for Named:\n    function display(view self: Named) returns string:\n        string text = Named.name(view self)\n        return run text\n".to_owned(),
            "Named value = Message(text: \"erased\")",
            "runtime error: Displayable.display returned pending(erased) instead of string",
            vec![],
        ),
        (
            "method_failure",
            message_declarations(
                "        list[string] owned = list(\"method-owned\")\n        list[string] removed = list.remove_at[string](owned, -1)\n        return run self.text",
            ),
            "Message value = Message(text: \"unreached\")",
            "runtime error: list.__remove_at: index -1 out of bounds",
            vec![],
        ),
    ];
    for (name, declarations, value, message, debug) in cases {
        assert_display_failure_in_both_profiles(
            name,
            &ordered_failure_source(&declarations, value),
            "before\nprefix\n",
            message,
            &debug,
        );
    }
}

const READY_AND_EXPLICIT_CONTROLS: &str = r#"interface Displayable:
    function display(view self: Displayable) returns string
namespace messages
export struct Pending:
    text: string
implement Displayable for Pending:
    function display(view self: Pending) returns string:
        return run run self.text
export struct Joined:
    text: string
implement Displayable for Joined:
    function display(view self: Joined) returns string:
        string nested = run run self.text
        string once = join nested handle error:
            return error
        return join once handle error:
            return error
export function format[T](view value: T) returns string:
    return "{value}"
export function baked() returns string:
    Joined value = Joined(text: "shown-λ🙂")
    return "{value}"
namespace app
function main(stdout: Stdout) returns nothing:
    use messages
    messages.Pending pending_owner = messages.Pending(text: "shown-λ🙂")
    string pending = messages.Pending.display(view pending_owner)
    trace pending
    Stdout.write(view stdout, "explicit-pending:{pending}\n")
    string once = join pending handle error:
        Stdout.write(view stdout, "unexpected:{error}\n")
        return nothing
    trace once
    Stdout.write(view stdout, "explicit-partial:{once}\n")
    string ready = join once handle error:
        Stdout.write(view stdout, "unexpected:{error}\n")
        return nothing
    trace ready
    Stdout.write(view stdout, "explicit:{ready}\n")
    messages.Joined joined_owner = messages.Joined(text: "shown-λ🙂")
    Stdout.write(view stdout, "implicit:{joined_owner}\n")
    string generic = messages.format[messages.Joined](view joined_owner)
    Stdout.write(view stdout, "generic:{generic}\n")
    Stdout.write(view stdout, "owners:{pending_owner.text}:{joined_owner.text}\n")
    string baked = comptime messages.baked()
    Stdout.write(view stdout, "comptime:{baked}\n")
verify joined_display:
    use messages
    assert messages.baked() == "shown-λ🙂"
    string baked = comptime messages.baked()
    assert baked == "shown-λ🙂"
property joined_display_repeats:
    given seed: int64
    use messages
    assert messages.baked() == "shown-λ🙂"
    assert seed == seed
"#;

#[test]
fn native_joined_display_and_explicit_pending_calls_match_reference_in_both_profiles() {
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("main.jett");
    fs::write(&source, READY_AND_EXPLICIT_CONTROLS).unwrap();
    let expected = jett_driver::run_file_capture_output(&source)
        .expect("fully joined display and ordinary explicit calls remain valid");
    assert_eq!(
        expected.stdout,
        concat!(
            "explicit-pending:pending(pending(shown-λ🙂))\n",
            "explicit-partial:pending(shown-λ🙂)\n",
            "explicit:shown-λ🙂\n",
            "implicit:shown-λ🙂\n",
            "generic:shown-λ🙂\n",
            "owners:shown-λ🙂:shown-λ🙂\n",
            "comptime:shown-λ🙂\n",
        )
    );
    assert_eq!(
        debug_trace_lines(&expected.debug_events),
        [
            "trace pending: string = pending(pending(shown-λ🙂))",
            "trace once: string = pending(shown-λ🙂)",
            "trace ready: string = shown-λ🙂",
        ]
    );
    let mut binaries = Vec::new();
    for release in [false, true] {
        let binary = directory
            .path()
            .join(format!("joined_display_{release}.exe"));
        jett_driver::native::build_host_executable_with_options(
            &source,
            launcher(),
            &binary,
            jett_driver::BuildOptions { release },
        )
        .expect("native implicit joined display and explicit pending-string call");
        binaries.push((binary, release));
    }
    let verify_binary = directory.path().join("joined_display_verify.exe");
    build_host_verify_suite_executable(&source, launcher(), &verify_binary)
        .expect("compiled joined display verify suite");
    let property_binary = directory.path().join("joined_display_property.exe");
    build_host_property_suite_executable(&source, launcher(), &property_binary)
        .expect("compiled joined display property suite");
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
