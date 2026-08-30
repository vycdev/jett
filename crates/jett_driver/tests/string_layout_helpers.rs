use std::path::PathBuf;

use jett_driver::test_file;

#[test]
fn string_layout_helpers_fixture_compiles() {
    let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..");
    let path = workspace_root
        .join("tests")
        .join("run_pass")
        .join("string_layout_helpers.jett");
    let result = test_file(&path).expect("fixture should type-check and run");
    assert_eq!(result.total, 6);
    assert_eq!(result.passed, result.total, "verify block count mismatch");
}
