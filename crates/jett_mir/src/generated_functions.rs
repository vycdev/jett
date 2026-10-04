//! Compact local tables shared by enclosing and explicitly lifted inline bodies.
use super::*;

#[cfg(test)]
mod tests;

/// Compact generated inline functions independently of their enclosing function.
/// Enclosing functions lose only unused local slots, preserving every block.
/// Capture and ordinary parameter order remain the existing callable ABI.
pub fn prepare_native_generated_functions(program: &mut Program, types: &TypeInterner) {
    if validate_call_ownership(program, types).is_err() {
        return;
    }
    for function in &mut program.functions {
        let _prepared = if function.debug_kind == hir::FunctionDebugKind::Inline {
            sequences::prune::unreachable(function)
        } else {
            sequences::prune::unused_locals(function)
        };
    }
}
