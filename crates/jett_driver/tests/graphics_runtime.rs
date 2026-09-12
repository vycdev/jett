use std::path::PathBuf;

use jett_driver::{
    GraphicsTestEvent, GraphicsTestKey, run_file_capture_output_with_graphics_test_events,
};

fn fixture() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/run_pass/graphics_scripted.jett")
}

#[test]
fn graphics_main_receives_authority_and_runs_checked_callbacks_without_a_window() {
    let output = run_file_capture_output_with_graphics_test_events(
        &fixture(),
        vec![
            GraphicsTestEvent::Key(GraphicsTestKey::Right),
            GraphicsTestEvent::Key(GraphicsTestKey::Right),
            GraphicsTestEvent::Key(GraphicsTestKey::Left),
            GraphicsTestEvent::Close,
        ],
    )
    .expect("checked graphics callbacks should run through the scripted provider");
    assert_eq!(output.stdout, "closed\n");
    assert!(output.debug_output.is_empty());
}

#[test]
fn graphics_driver_requires_consumption_of_the_scripted_input() {
    let error = run_file_capture_output_with_graphics_test_events(
        &fixture(),
        vec![
            GraphicsTestEvent::Close,
            GraphicsTestEvent::Key(GraphicsTestKey::Right),
        ],
    )
    .expect_err("unused scripted input must not silently pass");
    assert_eq!(
        error,
        "runtime error: Graphics: test provider has 1 unconsumed sample"
    );
}

#[test]
fn graphics_pipelines_preserve_generic_calls_aliases_and_argument_order() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/run_pass/graphics_pipeline_scripted.jett");
    let output = run_file_capture_output_with_graphics_test_events(
        &path,
        vec![
            GraphicsTestEvent::Key(GraphicsTestKey::Right),
            GraphicsTestEvent::Close,
            GraphicsTestEvent::Key(GraphicsTestKey::Left),
            GraphicsTestEvent::Close,
            GraphicsTestEvent::Key(GraphicsTestKey::Right),
            GraphicsTestEvent::Close,
        ],
    )
    .expect("explicit and inferred graphics pipelines should run through the scripted provider");
    assert_eq!(output.stdout, "closed three times\n");
    assert!(output.debug_output.is_empty());
}

#[test]
fn graphics_expression_statements_propagate_callback_runtime_errors() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/run_pass/graphics_callback_runtime_error.jett");
    let error = run_file_capture_output_with_graphics_test_events(
        &path,
        vec![GraphicsTestEvent::Key(GraphicsTestKey::Right)],
    )
    .expect_err(
        "callback errors must not be swallowed by expression statements or handled as data",
    );
    assert_eq!(
        error,
        "runtime error: string.repeat: requested output is too large"
    );
}
