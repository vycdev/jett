//! Remove impossible handler arms without removing runtime sum validation.
use super::*;
use jett_hir::ExpressionKind;
use jett_types::Type;

/// Private preparation authority. The original plan is never a public setter
/// and this record is minted only beside the canonical Branch-to-Goto rewrite.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct PreparedAbsentSuccess {
    plan: call_ownership::AbsentSuccessPlan,
}

impl PreparedAbsentSuccess {
    pub(super) fn plan(&self) -> &call_ownership::AbsentSuccessPlan {
        &self.plan
    }

    pub(super) fn remap_blocks(&mut self, blocks: &[Option<BlockId>]) -> bool {
        let get = |id: BlockId| blocks.get(id.index() as usize).copied().flatten();
        let (Some(selecting), Some(failure)) = (get(self.plan.selecting), get(self.plan.failure))
        else {
            return false;
        };
        self.plan.selecting = selecting;
        self.plan.failure = failure;
        true
    }

    pub(super) fn remap_locals(&mut self, locals: &[Option<LocalId>]) -> bool {
        let get = |id: LocalId| locals.get(id.index() as usize).copied().flatten();
        let (Some(source), Some(tag), Some(output)) = (
            get(self.plan.source),
            get(self.plan.tag),
            get(self.plan.output),
        ) else {
            return false;
        };
        self.plan.source = source;
        self.plan.tag = tag;
        self.plan.output = output;
        true
    }
}

/// Distinct private authority for an inhabited successful payload whose error
/// arm is Never. Original validation proves the removed false edge; this record
/// retains only the actual selected blocks, runtime tag and successful take.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct PreparedPresentSuccess {
    plan: call_ownership::PresentSuccessPlan,
}

impl PreparedPresentSuccess {
    pub(super) fn plan(&self) -> &call_ownership::PresentSuccessPlan {
        &self.plan
    }

    pub(super) fn remap_blocks(&mut self, blocks: &[Option<BlockId>]) -> bool {
        let get = |id: BlockId| blocks.get(id.index() as usize).copied().flatten();
        let (Some(selecting), Some(success)) = (get(self.plan.selecting), get(self.plan.success))
        else {
            return false;
        };
        self.plan.selecting = selecting;
        self.plan.success = success;
        true
    }

    pub(super) fn remap_locals(&mut self, locals: &[Option<LocalId>]) -> bool {
        let get = |id: LocalId| locals.get(id.index() as usize).copied().flatten();
        let (Some(source), Some(tag), Some(output)) = (
            get(self.plan.source),
            get(self.plan.tag),
            get(self.plan.output),
        ) else {
            return false;
        };
        self.plan.source = source;
        self.plan.tag = tag;
        self.plan.output = output;
        true
    }
}

#[cfg(test)]
mod tests;

/// Select the inhabited arm of a checked optional/result handler. The final
/// `SumTag` still evaluates the sum and rejects pending values at runtime.
pub fn prepare_native_uninhabited_sums(program: &mut Program, types: &TypeInterner) {
    // Never hide an invalid ID in an arm that compaction would otherwise erase.
    if validate_call_ownership(program, types).is_err() {
        return;
    }
    // Obtain all plans while every original block and source occurrence exists.
    // Any failure leaves the entire input unchanged, as does the gate above.
    let Ok(plans) = program
        .functions
        .iter()
        .map(|function| {
            call_ownership::validate_function(program, function, types)
                .map(|proof| (proof.absent_successes, proof.present_successes))
        })
        .collect::<Result<Vec<_>, _>>()
    else {
        return;
    };
    for (function, (absent_plans, present_plans)) in program.functions.iter_mut().zip(plans) {
        let before = (!function.breakpoint_regions.is_empty()
            || crate::call_owner_generations::has_records(function))
        .then(|| function.clone());
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
            for plan in absent_plans.iter().filter(|plan| {
                plan.selecting == block.id
                    && plan.failure == selected
                    && plan.source == *source
                    && plan.tag == *target
            }) {
                function
                    .prepared_absent_successes
                    .push(PreparedAbsentSuccess { plan: plan.clone() });
            }
            for plan in present_plans.iter().filter(|plan| {
                plan.selecting == block.id
                    && plan.success == selected
                    && plan.source == *source
                    && plan.tag == *target
            }) {
                function
                    .prepared_present_successes
                    .push(PreparedPresentSuccess { plan: plan.clone() });
            }
            block.terminator.kind = TerminatorKind::Goto(selected);
            changed = true;
        }
        if changed {
            // Every nested tag has its own checked source type, so one scan
            // selects all eligible arms before a single dense compaction.
            let regions_valid = before.as_ref().is_none_or(|before| {
                crate::call_owner_generations::sum_transition(function, before, types).is_ok()
                    && crate::breakpoint_regions::sum_transition(function, before, types).is_ok()
            });
            if !regions_valid || !sequences::prune::unreachable(function) {
                if let Some(before) = before {
                    *function = before;
                }
            }
        }
    }
}
