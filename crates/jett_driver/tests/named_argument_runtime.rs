use std::path::PathBuf;

use jett_driver::run_file_capture_stdout;

#[test]
fn named_direct_and_pipeline_calls_preserve_source_evaluation_order() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/run_pass/named_argument_runtime_order.jett");
    let output = run_file_capture_stdout(&path).expect("named source calls should run");
    assert_eq!(output, "abcpqr123:456\n");
}

#[test]
fn captured_locals_take_precedence_over_registered_function_names() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/run_pass/captured_local_function_name.jett");
    let output = run_file_capture_stdout(&path).expect("captured-local inference should run");
    assert_eq!(output, "int64");
}
