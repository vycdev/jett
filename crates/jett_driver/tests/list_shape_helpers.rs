use std::path::PathBuf;

use jett_driver::test_file;

#[test]
fn list_shape_helpers_fixture_passes() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("tests")
        .join("run_pass")
        .join("list_shape_helpers.jett");
    let result = test_file(&path).expect("fixture should type-check and run");
    assert_eq!(result.total, 5);
    assert_eq!(result.passed, result.total);
}
