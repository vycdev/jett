//! Remove impossible handler arms without removing runtime sum validation.
use super::*;
use jett_hir::ExpressionKind;
use jett_types::Type;

#[cfg(test)]
mod tests;

/// Select the inhabited arm of a checked optional/result handler. The final
/// `SumTag` still evaluates the sum and rejects pending values at runtime.
pub fn prepare_native_uninhabited_sums(program: &mut Program, types: &TypeInterner) {
    // Never hide an invalid ID in an arm that compaction would otherwise erase.
    if validate(program).is_err() {
        return;
    }
    for function in &mut program.functions {
        let mut changed = false;
        for block in &mut function.blocks {
            let Some(Statement {
                kind: StatementKind::SumTag { source, target },
                ..
            }) = block.statements.last()
            else {
                continue;
            };
            let TerminatorKind::Branch {
                condition,
                then_block,
                else_block,
            } = &block.terminator.kind
            else {
                continue;
            };
            if condition.ty != TypeInterner::BOOL
                || !matches!(condition.kind, ExpressionKind::Local(local) if local == *target)
                || function.locals[target.index() as usize].ty != TypeInterner::BOOL
            {
                continue;
            }
            let ty = function.locals[source.index() as usize].ty;
            if ty.index() as usize >= types.len() {
                continue;
            }
            let selected = match types.resolve(ty) {
                Type::Optional(inner) if *inner == TypeInterner::NEVER => *else_block,
                Type::Result(ok, error)
                    if *ok == TypeInterner::NEVER
                        && *error != TypeInterner::NEVER
                        && (error.index() as usize) < types.len() =>
                {
                    *else_block
                }
                Type::Result(ok, error)
                    if *ok != TypeInterner::NEVER
                        && *error == TypeInterner::NEVER
                        && (ok.index() as usize) < types.len() =>
                {
                    *then_block
                }
                _ => continue,
            };
            // Retain every statement, including the source snapshot and SumTag.
            // Replacing only this edge preserves evaluation and failure order.
            block.terminator.kind = TerminatorKind::Goto(selected);
            changed = true;
        }
        if changed {
            // Every nested tag has its own checked source type, so one scan
            // selects all eligible arms before a single dense compaction.
            sequences::prune::unreachable(function);
        }
    }
}
