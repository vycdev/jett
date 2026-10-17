//! Independent original import and current CFG seals. Public edits cannot mint them.
use super::*;
use std::sync::Arc;
#[derive(Debug, Clone, PartialEq)]
pub(super) struct Witness {
    original: Vec<hir::ActorGraphPlan>,
    current: Arc<Snapshot>,
}
#[derive(Debug, Clone)]
struct Snapshot {
    id: FunctionId,
    debug_kind: hir::FunctionDebugKind,
    capture_count: usize,
    span: Span,
    identity: FunctionIdentity,
    params: Vec<Param>,
    locals: Vec<Local>,
    blocks: Vec<BasicBlock>,
    entry: BlockId,
    return_type: TypeId,
}
impl PartialEq for Snapshot {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
            && self.debug_kind == other.debug_kind
            && self.capture_count == other.capture_count
            && self.span == other.span
            && self.identity == other.identity
            && self.params == other.params
            && self.locals == other.locals
            && self.entry == other.entry
            && self.return_type == other.return_type
            && breakpoint_regions::blocks_equal(&self.blocks, &other.blocks)
    }
}
fn snapshot(function: &Function) -> Snapshot {
    Snapshot {
        id: function.id,
        debug_kind: function.debug_kind.clone(),
        capture_count: function.capture_count,
        span: function.span,
        identity: function.identity.clone(),
        params: function.params.clone(),
        locals: function.locals.clone(),
        blocks: function.blocks.clone(),
        entry: function.entry,
        return_type: function.return_type,
    }
}
pub(super) fn has_records(function: &Function) -> bool {
    function.actor_graph.is_some()
}
pub(super) fn seal(
    original: &hir::Function,
    lowered: &Function,
    types: &TypeInterner,
) -> Result<Option<Witness>, String> {
    let mut body = original.body.clone();
    let mut plans = Vec::new();
    hir::visit_actor_graph_plans(&mut body, |plan| plans.push(plan.clone()))?;
    if plans.is_empty() {
        let mut forged = false;
        for block in &lowered.blocks {
            crate::resource_ownership::visit_block(block, &mut |value| {
                forged |= matches!(value.kind, hir::ExpressionKind::ActorGraphStep { .. })
            });
        }
        if forged {
            return Err(
                "raw MIR import instruction has no original checked graph occurrence".into(),
            );
        }
        return Ok(None);
    }
    for plan in &plans {
        plan.validate(original, types)?;
    }
    let mut actual = Vec::new();
    for block in &lowered.blocks {
        crate::resource_ownership::visit_block(block, &mut |value| {
            if let hir::ExpressionKind::ActorGraphStep { instruction, args } = &value.kind {
                actual.push((instruction.clone(), args.clone(), value.ty));
            }
        });
    }
    let expected = plans
        .iter()
        .flat_map(|plan| plan.instructions())
        .collect::<Vec<_>>();
    if actual.len() != expected.len() {
        return Err("actor graph lowering changed instruction cardinality".into());
    }
    // Blocks are allocated in lexical lowering order; each private instruction
    // retains its required occurrence, exact local operands and operation index.
    let mut seen = Vec::new();
    for (instruction, args, result) in &actual {
        if !expected.iter().any(|expected| instruction == *expected)
            || seen.iter().any(|other| instruction == other)
        {
            return Err(
                "actor graph lowering changed or duplicated an original instruction".into(),
            );
        }
        instruction.validate(lowered.id, &lowered.identity, args, *result, types)?;
        seen.push(instruction.clone());
    }
    Ok(Some(Witness {
        original: plans,
        current: Arc::new(snapshot(lowered)),
    }))
}
pub(super) fn validate(function: &Function) -> Result<(), String> {
    if let Some(witness) = &function.actor_graph {
        if *witness.current != snapshot(function) {
            return Err(
                "actor graph current CFG/local seal changed outside canonical preparation".into(),
            );
        }
    } else {
        let mut forged = false;
        for block in &function.blocks {
            crate::resource_ownership::visit_block(block, &mut |value| {
                forged |= matches!(
                    value.kind,
                    hir::ExpressionKind::ActorGraphStep { .. }
                        | hir::ExpressionKind::ActorGraphImport(_)
                )
            });
        }
        if forged {
            return Err("actor graph execution has no original/current private seal".into());
        }
    }
    Ok(())
}
pub(super) fn refresh(function: &mut Function) {
    let current = Arc::new(snapshot(function));
    if let Some(witness) = &mut function.actor_graph {
        witness.current = current;
    }
}
pub fn validate_native_actor_graphs(program: &Program) -> Result<(), String> {
    for function in &program.functions {
        validate(function)?;
    }
    Ok(())
}

/// A function signature is metadata, not an occupied Resource leaf. This
/// classification is usable only for an exact factory expression/header retained
/// by the private graph plan and its independently validated current CFG seal.
pub(super) fn metadata_type(types: &TypeInterner, ty: TypeId) -> bool {
    fn occupied(
        types: &TypeInterner,
        ty: TypeId,
        seen: &mut std::collections::HashSet<TypeId>,
    ) -> bool {
        if ty.index() as usize >= types.len() {
            return true;
        }
        if !seen.insert(ty) {
            return false;
        }
        match types.resolve(ty) {
            Type::Resource(_) => true,
            Type::Function { .. } => false,
            Type::List(child)
            | Type::Set(child)
            | Type::Optional(child)
            | Type::Secret(child)
            | Type::Refinement { base: child, .. } => occupied(types, *child, seen),
            Type::Map(a, b) | Type::Result(a, b) => {
                occupied(types, *a, seen) || occupied(types, *b, seen)
            }
            Type::Struct(id) => types
                .resolve_struct(*id)
                .fields
                .iter()
                .any(|(_, child)| occupied(types, *child, seen)),
            Type::Enum(id) => types.resolve_enum(*id).variants.iter().any(|variant| {
                variant
                    .fields
                    .iter()
                    .any(|(_, child)| occupied(types, *child, seen))
            }),
            Type::Machine(id) | Type::MachineState { machine: id, .. } => {
                types.resolve_machine(*id).states.iter().any(|state| {
                    state
                        .fields
                        .iter()
                        .any(|(_, child)| occupied(types, *child, seen))
                })
            }
            _ => false,
        }
    }
    resource_type_pending(types, ty) && !occupied(types, ty, &mut std::collections::HashSet::new())
}

pub fn native_graph_metadata_expression(
    function: &Function,
    types: &TypeInterner,
    value: &Expression,
) -> bool {
    let Some(witness) = &function.actor_graph else {
        return false;
    };
    if validate(function).is_err() || !metadata_type(types, value.ty) {
        return false;
    }
    witness.original.iter().any(|plan| {
        let mut found = false;
        let mut body = plan.body().clone();
        let result = hir::visit_actor_graph_factory_values(&mut body, |original| {
            found |= breakpoint_regions::expressions_equal(original, value);
        });
        result.is_ok() && found
    })
}

pub fn native_graph_metadata_local(
    function: &Function,
    types: &TypeInterner,
    local: &Local,
) -> bool {
    let Some(witness) = &function.actor_graph else {
        return false;
    };
    if validate(function).is_err() || !metadata_type(types, local.ty) {
        return false;
    }
    witness
        .original
        .iter()
        .any(|plan| plan.factory_metadata_local(local, types))
}

#[cfg(test)]
mod tests;
