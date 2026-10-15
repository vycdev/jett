use crate::{CheckOptions, CheckResult, check_with_options};
use jett_common::FileId;
use jett_diagnostics::Severity;

const RECORDS: &str = "namespace app\nstruct Inner:\n    divisor: int64\nstruct Record:\n    inner: Inner\n    divisor: int64\n    sibling: int64\n";

fn checked(body: &str) -> CheckResult {
    let source = format!("{RECORDS}{body}");
    let parsed = jett_parser::parse(&source, FileId::new(0));
    assert!(parsed.errors.is_empty(), "{source}\n{:?}", parsed.errors);
    let resolved = jett_resolve::resolve(&parsed.module);
    assert!(
        resolved
            .diagnostics
            .iter()
            .all(|diagnostic| diagnostic.severity != Severity::Error),
        "{source}\n{:?}",
        resolved.diagnostics
    );
    check_with_options(&parsed.module, &resolved, CheckOptions { release: false })
}

fn assert_valid(body: &str) {
    let result = checked(body);
    assert!(
        result
            .diagnostics
            .iter()
            .all(|diagnostic| diagnostic.severity != Severity::Error),
        "{body}\n{:?}",
        result.diagnostics
    );
}

fn assert_nonzero_error(body: &str) {
    let result = checked(body);
    let errors = result
        .diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.severity == Severity::Error)
        .collect::<Vec<_>>();
    assert!(!errors.is_empty(), "{body}");
    assert!(
        errors
            .iter()
            .all(|diagnostic| diagnostic.code.code() == 361),
        "{body}\n{errors:?}"
    );
}

#[test]
fn visible_guards_prove_exact_readonly_and_nested_fields() {
    for body in [
        "    if record.divisor != 0:\n        return 8 / record.divisor\n    return 0\n",
        "    if 0 != (record.inner).divisor:\n        return 8 / record.inner.divisor\n    return 0\n",
        "    if record.divisor == 0:\n        return 0\n    return 8 / record.divisor\n",
        "    if record.divisor == 0:\n        return 0\n    else:\n        return 8 / record.divisor\n",
    ] {
        assert_valid(&format!(
            "function guarded(view record: Record) returns int64:\n{body}"
        ));
    }
}

#[test]
fn any_same_root_write_invalidates_nested_field_facts_after_the_rhs() {
    for write in [
        "record.inner.divisor = 0",
        "record.sibling = 2",
        "record.inner = Inner(divisor: 0)",
        "record = Record(inner: Inner(divisor: 0), divisor: 0, sibling: 0)",
    ] {
        assert_nonzero_error(&format!(
            "function invalid(mutable record: Record) returns int64:\n    if record.inner.divisor != 0:\n        int64 before = 8 / record.inner.divisor\n        {write}\n        return 8 / record.inner.divisor\n    return 0\n"
        ));
    }
    assert_valid(
        "function replace(mutable record: Record) returns int64:\n    if record.divisor != 0:\n        record.divisor = 8 / record.divisor\n    return 0\n",
    );
    assert_nonzero_error(
        "function invalid(mutable record: Record) returns int64:\n    if record.divisor != 0:\n        record.divisor = 8 / record.divisor\n        return 8 / record.divisor\n    return 0\n",
    );
}

#[test]
fn field_facts_keep_alias_nominals_root_bindings_and_siblings_distinct() {
    assert_valid(
        "type Alias = Record\nfunction guarded(view record: Alias, mutable other: Record) returns int64:\n    if record.inner.divisor != 0:\n        other.inner.divisor = 0\n        return 8 / record.inner.divisor\n    return 0\n",
    );
    assert_valid(
        "function guarded(view record: Record) returns int64:\n    Record alias = view record\n    if alias.divisor != 0:\n        return 8 / alias.divisor\n    return 0\n",
    );
    for divisor in ["other.divisor", "record.sibling"] {
        assert_nonzero_error(&format!(
            "function invalid(view record: Record, view other: Record) returns int64:\n    if record.divisor != 0:\n        return 8 / {divisor}\n    return 0\n"
        ));
    }
}

#[test]
fn loop_backedges_kill_inherited_field_and_scalar_facts() {
    for (parameters, divisor, write) in [
        (
            "mutable record: Record, keep: bool",
            "record.divisor",
            "record.divisor = 0",
        ),
        (
            "mutable divisor: int64, keep: bool",
            "divisor",
            "divisor = 0",
        ),
    ] {
        for header in ["for item in list(1, 2)", "while keep"] {
            assert_nonzero_error(&format!(
                "function invalid({parameters}) returns int64:\n    if {divisor} != 0:\n        {header}:\n            int64 quotient = 8 / {divisor}\n            {write}\n    return 0\n"
            ));
        }
    }
    assert_valid(
        "function guarded(mutable record: Record) returns int64:\n    while record.divisor != 0:\n        int64 quotient = 8 / record.divisor\n        record.divisor = 0\n    return 0\n",
    );
    assert_valid(
        "function guarded(mutable divisor: int64) returns int64:\n    for item in list(1, 2):\n        if divisor != 0:\n            int64 quotient = 8 / divisor\n            divisor = 0\n    return 0\n",
    );
}

#[test]
fn condition_handler_writes_cannot_remint_field_or_scalar_facts() {
    for (parameters, divisor, write) in [
        (
            "mutable record: Record",
            "record.divisor",
            "record.divisor = 0",
        ),
        ("mutable divisor: int64", "divisor", "divisor = 0"),
    ] {
        assert_nonzero_error(&format!(
            "function invalid({parameters}) returns int64:\n    optional[bool] absent = none\n    if {divisor} != 0 and (absent handle:\n        {write}\n        default true\n    ):\n        return 8 / {divisor}\n    return 0\n"
        ));
        assert_nonzero_error(&format!(
            "function invalid({parameters}) returns int64:\n    optional[bool] absent = none\n    if {divisor} == 0:\n        return 0\n    else if (absent handle:\n        {write}\n        default true\n    ):\n        return 8 / {divisor}\n    return 0\n"
        ));
    }
}

#[test]
fn guarded_field_keys_remain_concrete_inside_generic_bodies() {
    assert_valid(
        "struct GenericRecord[T]:\n    divisor: int64\n    item: T\nfunction guarded[T](view record: GenericRecord[T]) returns int64:\n    if record.divisor != 0:\n        return 8 / record.divisor\n    return 0\nfunction main() returns int64:\n    int64 first = guarded[int64](view GenericRecord[int64](divisor: 2, item: 1))\n    return first + guarded[string](view GenericRecord[string](divisor: 2, item: \"two\"))\n",
    );
}

#[test]
fn exact_machine_state_field_guards_use_the_checked_owner() {
    assert_valid(
        "machine Counter:\n    states:\n        ready(divisor: int64)\nfunction guarded(view counter: Counter at ready) returns int64:\n    if counter.divisor != 0:\n        return 8 / counter.divisor\n    return 0\n",
    );
}

#[test]
fn a_conditional_write_and_repeated_condition_cannot_keep_inherited_facts() {
    assert_nonzero_error(
        "function invalid(mutable record: Record, replace: bool) returns int64:\n    if record.divisor != 0:\n        if replace:\n            record.divisor = 0\n        return 8 / record.divisor\n    return 0\n",
    );
    for (parameters, divisor, write) in [
        (
            "mutable record: Record",
            "record.divisor",
            "record.divisor = 0",
        ),
        ("mutable divisor: int64", "divisor", "divisor = 0"),
    ] {
        assert_nonzero_error(&format!(
            "function invalid({parameters}) returns int64:\n    optional[bool] absent = none\n    if {divisor} != 0:\n        while 8 / {divisor} != 0 and (absent handle:\n            {write}\n            default true\n        ):\n            int64 marker = 1\n    return 0\n"
        ));
    }
}
