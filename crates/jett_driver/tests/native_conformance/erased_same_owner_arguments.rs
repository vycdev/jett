use super::*;

const ORIGINAL_WRONG: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/native/erased_same_owner/05_erased_other_owner.jett"
));
const ORIGINAL_MATCHING: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/native/erased_same_owner/12_erased_matching_owner.jett"
));
const OWNER_FAILURE: &str = "runtime error: interface dispatch concrete argument owner mismatch";

fn matching_case(name: &str, source_text: &[u8], debug: &str) {
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join(format!("{name}.jett"));
    fs::write(&source, source_text).unwrap();
    let expected = jett_driver::run_file_capture_output(&source).unwrap();
    assert!(expected.stdout.is_empty());
    assert_eq!(
        jett_driver::render_debug_events(&expected.debug_events),
        debug
    );
    let executable = directory.path().join(format!("{name}.exe"));
    build_host_executable(&source, launcher(), &executable).unwrap();
    fs::remove_file(&source).unwrap();
    let actual = run_bounded(&executable, directory.path());
    assert_eq!(actual.status.code(), Some(0), "{actual:?}");
    assert!(actual.stdout.is_empty());
    assert_eq!(actual.stderr, debug.as_bytes());
}

fn mismatch_case(name: &str, source_text: &[u8], prefix: &str) {
    failure_case(name, source_text, prefix, OWNER_FAILURE);
}

fn failure_case(name: &str, source_text: &[u8], prefix: &str, message: &str) {
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join(format!("{name}.jett"));
    fs::write(&source, source_text).unwrap();
    let expected = jett_driver::run_file_capture_outcome(&source).unwrap_err();
    assert_eq!(expected.message, message);
    assert!(expected.output.stdout.is_empty());
    assert_eq!(
        jett_driver::render_debug_events(&expected.output.debug_events),
        prefix
    );
    let executable = directory.path().join(format!("{name}.exe"));
    build_host_executable(&source, launcher(), &executable).unwrap();
    fs::remove_file(&source).unwrap();
    let actual = run_bounded(&executable, directory.path());
    assert_eq!(actual.status.code(), Some(71), "{actual:?}");
    assert!(actual.stdout.is_empty());
    assert_eq!(actual.stderr, format!("{prefix}{message}\n").as_bytes());
}

#[test]
fn native_erased_owner_original_wrong_source_rejects_before_implementation() {
    mismatch_case("original_wrong_owner", ORIGINAL_WRONG, "");
}

#[test]
fn native_erased_owner_original_matching_source_preserves_view_dispatch() {
    matching_case(
        "original_matching_owner",
        ORIGINAL_MATCHING,
        "left:one:two\n",
    );
}

#[test]
fn native_erased_owner_direct_and_callback_preserve_argument_order_before_failure() {
    let original = std::str::from_utf8(ORIGINAL_WRONG)
        .unwrap()
        .replace("\r\n", "\n");
    let source = original.replace(
        "return \"left:{self.name}:{other.name}\"",
        "println(\"entered-left-body\")\n        return \"left:{self.name}:{other.name}\"",
    ).replace(
        "function main() returns nothing:",
        "function receiver() returns Pairable:\n    println(\"receiver\")\n    return Left(name: \"one\")\nfunction other_value() returns Pairable:\n    println(\"other\")\n    return Right(name: \"two\")\nfunction main() returns nothing:",
    ).replace(
        "Pairable first = Left(name: \"one\")\n    Pairable second = Right(name: \"two\")",
        "Pairable first = receiver()\n    Pairable second = other_value()",
    );
    for callback in [false, true] {
        let source = if callback {
            source.replace(
                "println(Pairable.combine(view first, view second))",
                "function(view Pairable, view Pairable) returns string callback = Pairable.combine\n    println(callback(view first, view second))",
            )
        } else {
            source.clone()
        };
        mismatch_case(
            &format!("ordered_callback_{callback}"),
            source.as_bytes(),
            "receiver\nother\n",
        );
    }
}

#[test]
fn native_erased_owner_owned_arguments_reject_before_body() {
    let source = std::str::from_utf8(ORIGINAL_WRONG)
        .unwrap()
        .replace("view self:", "self:")
        .replace("view other:", "other:")
        .replace(
            "Pairable.combine(view first, view second)",
            "Pairable.combine(first, second)",
        )
        .replace(
            "return \"left:{self.name}:{other.name}\"",
            "println(\"entered-left-body\")\n        return \"left:{self.name}:{other.name}\"",
        );
    mismatch_case("owned_wrong_owner", source.as_bytes(), "");
}

const COLLECTION_OWNER: &str = r#"namespace app
interface Pairable:
    function combine(view self: Pairable, view other: Pairable) returns string
struct Left:
    name: string
struct Right:
    name: string
implement Pairable for list[optional[Left]]:
    function combine(view self: list[optional[Left]], view other: list[optional[Left]]) returns string:
        println("entered-left-body")
        return "left"
implement Pairable for list[optional[Right]]:
    function combine(view self: list[optional[Right]], view other: list[optional[Right]]) returns string:
        return "right"
function main() returns nothing:
    Pairable first = list(some(Left(name: "one")))
    Pairable second = list(some(OTHER_OWNER(name: "two")))
    println(Pairable.combine(view first, view second))
    return nothing
"#;

#[test]
fn native_erased_owner_nested_collection_identity_requires_exact_owner() {
    mismatch_case(
        "nested_wrong_owner",
        COLLECTION_OWNER.replace("OTHER_OWNER", "Right").as_bytes(),
        "",
    );
    matching_case(
        "nested_matching_owner",
        COLLECTION_OWNER.replace("OTHER_OWNER", "Left").as_bytes(),
        "entered-left-body\nleft\n",
    );
}

#[test]
fn native_erased_owner_interface_children_keep_their_declared_erased_contract() {
    let source = r#"namespace app
interface Pairable:
    function combine(view self: Pairable, view others: list[Pairable]) returns string
struct Left:
    name: string
struct Right:
    name: string
implement Pairable for Left:
    function combine(view self: Left, view others: list[Pairable]) returns string:
        return self.name
implement Pairable for Right:
    function combine(view self: Right, view others: list[Pairable]) returns string:
        return self.name
function main() returns nothing:
    Pairable first = Left(name: "one")
    Pairable second = Right(name: "two")
    list[Pairable] others = list(second)
    println(Pairable.combine(view first, view others))
    return nothing
"#;
    matching_case("retained_erased_children", source.as_bytes(), "one\n");
}

#[test]
fn native_erased_owner_prior_argument_failure_keeps_its_message_and_order() {
    let original = std::str::from_utf8(ORIGINAL_WRONG)
        .unwrap()
        .replace("\r\n", "\n");
    let source = original.replace(
        "function main() returns nothing:",
        "function other_value() returns Pairable:\n    println(\"other\")\n    string rejected = string.repeat(\"ab\", 9223372036854775807)\n    return Right(name: rejected)\nfunction main() returns nothing:",
    ).replace("Pairable second = Right(name: \"two\")", "Pairable second = other_value()");
    failure_case(
        "prior_argument_failure",
        source.as_bytes(),
        "other\n",
        "runtime error: string.repeat: requested output is too large",
    );
}

const PRIMITIVE_OWNER: &str = r#"namespace app
interface Pairable:
    function combine(view self: Pairable, view other: Pairable) returns string
implement Pairable for int8:
    function combine(view self: int8, view other: int8) returns string:
        println("entered-int8-body")
        return "int8"
implement Pairable for int64:
    function combine(view self: int64, view other: int64) returns string:
        return "int64"
function main() returns nothing:
    int8 narrow = 7
    OTHER_TYPE second_value = 9
    Pairable first = narrow
    Pairable second = second_value
    println(Pairable.combine(view first, view second))
    return nothing
"#;

#[test]
fn native_erased_owner_equal_primitive_carriers_keep_checked_widths() {
    mismatch_case(
        "wrong_primitive_width",
        PRIMITIVE_OWNER.replace("OTHER_TYPE", "int64").as_bytes(),
        "",
    );
    matching_case(
        "matching_primitive_width",
        PRIMITIVE_OWNER.replace("OTHER_TYPE", "int8").as_bytes(),
        "entered-int8-body\nint8\n",
    );
}

const GENERIC_OWNER: &str = r#"namespace app
interface Pairable:
    function combine(view self: Pairable, view other: Pairable) returns string
struct Box[T]:
    item: T
implement Pairable for Box[int64]:
    function combine(view self: Box[int64], view other: Box[int64]) returns string:
        println("entered-int-box-body")
        return "integer"
implement Pairable for Box[string]:
    function combine(view self: Box[string], view other: Box[string]) returns string:
        return "text"
function main() returns nothing:
    Pairable first = Box[int64](item: 7)
    Pairable second = OTHER_BOX
    println(Pairable.combine(view first, view second))
    return nothing
"#;

#[test]
fn native_erased_owner_nominal_generic_arguments_are_not_layout_casts() {
    mismatch_case(
        "wrong_generic_owner",
        GENERIC_OWNER
            .replace("OTHER_BOX", "Box[string](item: \"nine\")")
            .as_bytes(),
        "",
    );
    matching_case(
        "matching_generic_owner",
        GENERIC_OWNER
            .replace("OTHER_BOX", "Box[int64](item: 9)")
            .as_bytes(),
        "entered-int-box-body\ninteger\n",
    );
}

#[test]
fn native_erased_owner_production_dispatch_matches_with_optimized_objects_without_source() {
    for (name, original, status, output) in [
        ("matching", ORIGINAL_MATCHING, 0, b"left:one:two".as_slice()),
        ("mismatched", ORIGINAL_WRONG, 71, b"".as_slice()),
    ] {
        let original = std::str::from_utf8(original).unwrap().replace("\r\n", "\n");
        assert!(original.contains("function main() returns nothing:"));
        assert!(original.contains("println(Pairable.combine(view first, view second))"));
        let production = original
            .replace(
                "function main() returns nothing:",
                "function main(stdout: Stdout) returns nothing:",
            )
            .replace(
                "println(Pairable.combine(view first, view second))",
                "Stdout.write(view stdout, Pairable.combine(view first, view second))",
            );
        let directory = tempfile::tempdir().unwrap();
        let source = directory
            .path()
            .join(format!("production_owner_{name}.jett"));
        fs::write(&source, production).unwrap();
        let mut binaries = Vec::new();
        for release in [false, true] {
            let binary = directory
                .path()
                .join(format!("production_owner_{name}_{release}.exe"));
            jett_driver::native::build_host_executable_with_options(
                &source,
                launcher(),
                &binary,
                jett_driver::BuildOptions { release },
            )
            .unwrap();
            binaries.push(binary);
        }
        fs::remove_file(&source).unwrap();
        assert!(!source.exists());
        for binary in binaries {
            let actual = run_bounded(&binary, directory.path());
            assert_eq!(actual.status.code(), Some(status), "{actual:?}");
            assert_eq!(actual.stdout, output, "{actual:?}");
            let error = if status == 0 {
                String::new()
            } else {
                format!("{OWNER_FAILURE}\n")
            };
            assert_eq!(actual.stderr, error.as_bytes(), "{actual:?}");
        }
    }
}
