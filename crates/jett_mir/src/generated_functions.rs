//! Compact local tables shared by enclosing and explicitly lifted inline bodies.
use super::*;

#[cfg(test)]
mod tests;

/// Compact generated inline functions independently of their enclosing function.
/// Enclosing functions lose only unused local slots, preserving every block.
/// Capture and ordinary parameter order remain the existing callable ABI.
pub fn prepare_native_generated_functions(program: &mut Program, types: &TypeInterner) {
    if crate::actor_graph::validate_native_actor_graphs(program).is_err() {
        return;
    }
    if validate_call_ownership(program, types).is_err() {
        return;
    }
    for function in &mut program.functions {
        let before = function.clone();
        let _prepared = if function.debug_kind == hir::FunctionDebugKind::Inline {
            sequences::prune::unreachable(function)
        } else {
            sequences::prune::unused_locals(function)
        };
        if crate::reflected_callable::prepared(function, &before).is_err() {
            *function = before;
        }
    }
    for function in &mut program.functions {
        crate::actor_graph::refresh(function);
    }
}
