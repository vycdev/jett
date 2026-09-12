use std::path::PathBuf;

use jett_diagnostics::Severity;
use jett_driver::{build_file, test_file};

fn fixture(kind: &str, name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests")
        .join(kind)
        .join(name)
}

#[test]
fn unspecialized_generic_values_fail_before_interpretation() {
    let outcome = build_file(&fixture("compile_fail", "generic_function_values.jett"));
    let errors = outcome
        .diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.severity == Severity::Error)
        .collect::<Vec<_>>();
    assert_eq!(errors.len(), 5, "{:?}", outcome.diagnostics);
    for error in errors {
        assert_eq!(error.code.code(), 373, "{error:?}");
        assert!(error.message.contains("concrete named wrapper"));
    }
}

#[test]
fn concrete_wrappers_preserve_named_values_and_generic_call_inference() {
    let outcome = test_file(&fixture("run_pass", "generic_function_value_wrappers.jett"))
        .expect("concrete wrappers and ordinary generic calls should typecheck");
    assert_eq!(outcome.total, 1);
    let failures = outcome
        .blocks
        .iter()
        .filter(|block| !block.passed)
        .map(|block| (&block.name, &block.error))
        .collect::<Vec<_>>();
    assert_eq!(outcome.passed, 1, "{failures:?}");
    assert_eq!(outcome.failed, 0);
}
