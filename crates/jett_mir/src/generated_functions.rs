//! Remove unused parent-local slots copied into explicitly lifted inline bodies.
use super::*;

#[cfg(test)]
mod tests;

/// Compact generated inline functions independently of their enclosing function.
/// Capture and ordinary parameter order remain the existing callable ABI.
pub fn prepare_native_generated_functions(program: &mut Program) {
    if validate(program).is_err() {
        return;
    }
    for function in &mut program.functions {
        if function.debug_kind == hir::FunctionDebugKind::Inline {
            sequences::prune::unreachable(function);
        }
    }
}
