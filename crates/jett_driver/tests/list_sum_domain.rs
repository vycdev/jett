use std::fs;
use std::path::Path;

use jett_diagnostics::Severity;
use jett_driver::{BuildOptions, BuildResult, build_file_with_options, build_source, test_file};

const DOMAIN: &str = "numeric primitive for list.sum";

fn assert_rejected(built: &BuildResult) {
    assert!(built.has_errors, "unsupported sum was accepted");
    let errors = built
        .diagnostics
        .iter()
        .filter(|error| error.severity == Severity::Error)
        .collect::<Vec<_>>();
    assert_eq!(errors.len(), 1, "{:?}", built.diagnostics);
    assert_eq!(errors[0].code.code(), 300, "{errors:?}");
    assert!(errors[0].message.contains(DOMAIN), "{errors:?}");
    assert!(built.debug_observations.is_empty());
}

fn reject_source(source: &str) {
    assert_rejected(&build_source(source, "list_sum_domain.jett"));
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("main.jett");
    fs::write(&path, source).unwrap();
    for release in [false, true] {
        assert_rejected(&build_file_with_options(&path, BuildOptions { release }));
    }
}

#[test]
fn list_sum_public_facade_rejects_unsupported_element_domains() {
    let declarations = "namespace app\ntype Positive = int64 where value > 0\ntype PositiveAlias = Positive\ntype Hidden = secret[int64]\nstruct Record:\n    value: int64\n";
    for element in [
        "bool",
        "string",
        "bytes",
        "optional[int64]",
        "result[int64, string]",
        "list[int64]",
        "Record",
        "Positive",
        "PositiveAlias",
        "secret[int64]",
        "Hidden",
        "secret[Positive]",
    ] {
        reject_source(&format!(
            "{declarations}function total(view values: list[{element}]) returns {element}:\n    return list.sum[{element}](view values)\n"
        ));
    }
}

#[test]
fn list_sum_generic_inference_pipelines_and_function_wrappers_keep_domain_policy() {
    let generic = "namespace app\nfunction total[T](values: list[T]) returns T:\n    return list.sum[T](view values)\n";
    for call in [
        "total[bool](values)",
        "total(values)",
        "values into total[bool]()",
        "values into total()",
    ] {
        reject_source(&format!(
            "{generic}function invalid(values: list[bool]) returns bool:\n    return {call}\n"
        ));
    }
    for call in [
        "list.sum(view values)",
        "view values into list.sum[bool]()",
        "view values into list.sum()",
    ] {
        reject_source(&format!(
            "namespace app\nfunction invalid(view values: list[bool]) returns bool:\n    return {call}\n"
        ));
    }
    reject_source(
        "namespace app\nfunction invalid(values: list[bool]) returns bool:\n    return list.sum[bool](view values)\nfunction invoke() returns bool:\n    function(list[bool]) returns bool callback = invalid\n    return callback(list(true))\n",
    );
}

#[test]
fn list_sum_invalid_domain_stops_required_values_and_test_bodies_before_evaluation() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("main.jett");
    for body in [
        "function total() returns bool:\n    list[bool] values = list()\n    return list.sum[bool](view values)\nfunction main() returns nothing:\n    bool baked = comptime total()\n",
        "verify rejected_sum:\n    list[bool] values = list()\n    bool total = list.sum[bool](view values)\n    assert total\n",
        "property rejected_sum:\n    given flag: bool\n    list[bool] values = list(flag)\n    bool total = list.sum[bool](view values)\n    assert total\n",
    ] {
        let source = format!("namespace app\n{body}");
        reject_source(&source);
        fs::write(&path, source).unwrap();
        let failure = test_file(&path).expect_err("unsupported sum prevents suite execution");
        assert!(
            failure.contains("E0300") && failure.contains(DOMAIN),
            "{failure}"
        );
    }
}

#[test]
fn list_sum_numeric_aliases_preserve_generic_and_function_value_routes() {
    let source = r#"namespace app
type Small = int8
type Alias = Small
function total[T](values: list[T]) returns T:
    return list.sum[T](view values)
function wrapped(values: list[Alias]) returns Alias:
    return total(values)
function exercise() returns bool:
    list[Alias] direct_values = list(127, 1)
    list[Alias] inferred_values = list(127, 1)
    Alias direct = list.sum[Alias](view direct_values)
    Alias inferred = list.sum(view inferred_values)
    list[Alias] piped_values = list(127, 1)
    Alias piped = piped_values into total[Alias]()
    function(list[Alias]) returns Alias callback = wrapped
    Alias indirect = callback(list(127, 1))
    Alias expected = -128
    Alias zero = 0
    list[Alias] empty = list()
    Alias empty_total = list.sum[Alias](view empty)
    return direct == expected and inferred == expected and piped == expected and indirect == expected and empty_total == zero
verify numeric_alias_routes:
    assert exercise()
    bool baked = comptime exercise()
    assert baked
property numeric_alias_trials:
    given repeat: bool
    assert exercise()
    if repeat:
        assert exercise()
"#;
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("main.jett");
    fs::write(&path, source).unwrap();
    for release in [false, true] {
        let built = build_file_with_options(&path, BuildOptions { release });
        assert!(!built.has_errors, "{:?}", built.diagnostics);
    }
    let outcome = test_file(&path).expect("numeric alias routes");
    assert_eq!((outcome.total, outcome.passed, outcome.failed), (2, 2, 0));
    let primitives =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/native/list_sum_primitives.jett");
    let outcome = test_file(&primitives).expect("all primitive widths remain supported");
    assert_eq!((outcome.total, outcome.passed, outcome.failed), (2, 2, 0));
}
