pub mod checked_types;
pub mod debug;
pub mod explicit;
pub mod interpreter;
mod resource_execution;
pub mod value;
pub mod verify;

/// Shared stack budget for recursive reference and explicit comptime execution.
pub const INTERPRETER_STACK_SIZE: usize = 8 * 1024 * 1024;

pub use debug::{DebugEvent, DebugEventKind, render_debug_events};
pub use explicit::{
    CheckedRequiredOwner, CheckedRequiredResourceHook, CheckedRequiredScope, CheckedRequiredValue,
    ComptimeContext, ExplicitComptimeEvaluation, ExplicitComptimeValues,
    evaluate_explicit_comptime_expressions, evaluate_explicit_comptime_expressions_capture,
};
pub use interpreter::{
    ClockTestSample, EnvironmentTestEntry, EnvironmentTestSnapshot, EnvironmentTestText,
    GraphicsTestEvent, GraphicsTestKey, GraphicsTestObservation, Interpreter, RandomTestSample,
};
pub use resource_execution::ExecutionPurpose;
pub use value::Value;
pub use verify::{
    ComptimeError, eval_assert, eval_function, run_verify_blocks,
    run_verify_blocks_detailed_with_metadata,
    run_verify_blocks_detailed_with_metadata_and_expression_types, run_verify_blocks_with_metadata,
    run_verify_blocks_with_metadata_and_expression_types, verify_results_to_diagnostics,
};
