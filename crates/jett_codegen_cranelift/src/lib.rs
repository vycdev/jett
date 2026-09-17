//! Cranelift ahead-of-time object generation for validated Jett MIR.
//!
//! This crate deliberately consumes MIR rather than source or AST nodes. The
//! initial backend slice supports scalar primitives and explicit control flow;
//! every other MIR construct is rejected before object generation.

mod emit;
mod error;
mod mangle;
mod reachability;
mod verify;

pub use emit::{
    JETT_AOT_ENTRY_SUCCESS_V1, JETT_AOT_ENTRY_SYMBOL_V1, ObjectArtifact, emit_host_object,
    emit_host_program_object, emit_object_for_target, emit_program_object_for_target, host_target,
};
pub use error::CodegenError;
pub use mangle::symbol_name;
