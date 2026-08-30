use jett_common::FileId;
use jett_fmt::format_source;

#[test]
fn unary_bang_stays_attached_to_operand() {
    let source = "function negate(value: bool) returns bool:\n    return !value\n";
    let result = format_source(source, FileId::new(0));
    assert!(
        result.errors.is_empty(),
        "format errors: {:?}",
        result.errors
    );
    assert!(result.output.contains("return !value"));
    assert!(!result.output.contains("! value"));
}
