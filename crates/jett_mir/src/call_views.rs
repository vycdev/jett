//! Exact eager value/field borrows whose lifetime belongs to a consuming call.
//! Source local aliases remain persistent; names never authorize scope expiry.
use super::*;
use std::collections::{BTreeMap, BTreeSet, VecDeque};

#[cfg(test)]
mod tests;

pub(crate) fn borrowed_source(
    types: &TypeInterner,
    locals: &[Local],
    expression: &Expression,
) -> Option<LocalId> {
    let hir::ExpressionKind::Local(source) = borrowed_root(expression)?.kind else {
        return None;
    };
    if !call_origin(locals, source) {
        return None;
    }
    let origin = locals.get(source.index() as usize)?;
    hir::validate_local_view_initializer(expression, source, origin.ty, expression.ty, types)
        .ok()?;
    Some(source)
}

/// Persistent intermediate aliases retain their immutable-chain restriction.
/// The terminal owner may be mutable: the active Begin/End loan protects it.
fn call_origin(locals: &[Local], source: LocalId) -> bool {
    let mut current = source;
    for _ in 0..locals.len() {
        let Some(local) = locals.get(current.index() as usize) else {
            return false;
        };
        if local.id != current {
            return false;
        }
        match local.view_source {
            Some(parent) if !local.mutable && parent.index() < current.index() => current = parent,
            Some(_) => return false,
            None => return true,
        }
    }
    false
}

fn borrowed_root(expression: &Expression) -> Option<&Expression> {
    let hir::ExpressionKind::View(value) = &expression.kind else {
        return None;
    };
    let mut current = value.as_ref();
    loop {
        match &current.kind {
            hir::ExpressionKind::Field { base, .. } => {
                current = base;
            }
            hir::ExpressionKind::View(value)
            | hir::ExpressionKind::Coarsen(value)
            | hir::ExpressionKind::Declassify(value) => current = value,
            hir::ExpressionKind::InterfaceCoerce { value, adapters } if adapters.is_empty() => {
                current = value;
            }
            _ => return Some(current),
        }
    }
}

/// Source calls return owned values; constructors and explicit owning operators
/// also produce ordinary storage. A borrowed local, field, allocating conversion
/// or unknown control expression cannot acquire ownership via this predicate.
fn owned_temporary(root: &Expression) -> bool {
    matches!(
        root.kind,
        hir::ExpressionKind::Call { .. }
            | hir::ExpressionKind::IndirectCall { .. }
            | hir::ExpressionKind::Intrinsic { .. }
            | hir::ExpressionKind::StructConstruct { .. }
            | hir::ExpressionKind::BitfieldConstruct { .. }
            | hir::ExpressionKind::MachineConstruct { .. }
            | hir::ExpressionKind::MachineTransition { .. }
            | hir::ExpressionKind::Clone(_)
            | hir::ExpressionKind::Run(_)
            | hir::ExpressionKind::Join(_)
    )
}

pub(crate) fn replace_borrowed_root(
    expression: &Expression,
    replacement: Expression,
) -> Option<Expression> {
    let root = borrowed_root(expression)?;
    if root.ty != replacement.ty {
        return None;
    }
    let mut rewritten = expression.clone();
    replace_root_node(&mut rewritten, replacement);
    Some(rewritten)
}

fn replace_root_node(expression: &mut Expression, replacement: Expression) {
    match &mut expression.kind {
        hir::ExpressionKind::Field { base, .. } => replace_root_node(base, replacement),
        hir::ExpressionKind::View(value)
        | hir::ExpressionKind::Coarsen(value)
        | hir::ExpressionKind::Declassify(value) => replace_root_node(value, replacement),
        hir::ExpressionKind::InterfaceCoerce { value, adapters } if adapters.is_empty() => {
            replace_root_node(value, replacement);
        }
        _ => *expression = replacement,
    }
}

/// Prove the exact path against a hypothetical ordinary owning local before
/// operand lowering mutates CFG/locals. The original root is materialized once.
pub(crate) fn temporary_root(
    types: &TypeInterner,
    locals: &[Local],
    expression: &Expression,
) -> Option<Expression> {
    let root = borrowed_root(expression)?;
    if !owned_temporary(root) {
        return None;
    }
    let source = LocalId::new(u32::try_from(locals.len()).ok()?);
    let rewritten = replace_borrowed_root(
        expression,
        Expression {
            kind: hir::ExpressionKind::Local(source),
            ty: root.ty,
            span: root.span,
        },
    )?;
    hir::validate_local_view_initializer(&rewritten, source, root.ty, expression.ty, types).ok()?;
    Some(root.clone())
}

/// Validate unused and unreachable initializers and scope paths before pruning.
/// The returned map records only typed Begin authority, not synthesized names.
pub(crate) fn validate(
    function: &Function,
    types: &TypeInterner,
) -> Result<BTreeMap<usize, usize>, String> {
    let mut stages = BTreeMap::new();
    let mut starts = BTreeMap::new();
    for block in &function.blocks {
        for statement in &block.statements {
            if let StatementKind::BeginCallView { local, value } = &statement.kind {
                let definition = function.local(*local).ok_or("call view target is absent")?;
                let source = definition
                    .view_source
                    .ok_or("call view has no borrowed origin")?;
                if definition.id != *local
                    || definition.mutable
                    || definition.ty != value.ty
                    || definition.debug_ty != value.ty
                    || source.index() >= local.index()
                    || function.params.iter().any(|param| param.local == *local)
                    || borrowed_source(types, &function.locals, value) != Some(source)
                {
                    return Err(
                        "call view requires its exact borrowed initializer and stable origin"
                            .into(),
                    );
                }
                let root = function
                    .view_root(source)
                    .ok_or("call view origin is cyclic")?;
                if stages
                    .insert(local.index() as usize, root.index() as usize)
                    .is_some()
                {
                    return Err("call view has more than one initializer".into());
                }
                starts.insert(local.index() as usize, block.id);
            }
        }
    }
    // An internal stage cannot escape by becoming another local's origin. Its
    // only uses belong to the operation guarded by its own typed Begin/End.
    if function.locals.iter().any(|local| {
        local
            .view_source
            .is_some_and(|source| stages.contains_key(&(source.index() as usize)))
    }) {
        return Err("call view cannot escape into another local alias origin".into());
    }
    let mut ends = BTreeSet::new();
    for statement in function.blocks.iter().flat_map(|block| &block.statements) {
        let target = match &statement.kind {
            StatementKind::EndCallView { local } => {
                if !stages.contains_key(&(local.index() as usize)) {
                    return Err("call view end does not identify an internal stage".into());
                }
                ends.insert(local.index() as usize);
                continue;
            }
            StatementKind::Let { local, .. }
            | StatementKind::CheckRefinement { local, .. }
            | StatementKind::SequenceLength { target: local, .. }
            | StatementKind::SequenceGet { target: local, .. }
            | StatementKind::SumTag { target: local, .. }
            | StatementKind::SumTake { target: local, .. } => Some(*local),
            StatementKind::Assign { target, .. } => match target.kind {
                hir::ExpressionKind::Local(local) => Some(local),
                _ => None,
            },
            _ => None,
        };
        if target.is_some_and(|local| stages.contains_key(&(local.index() as usize))) {
            return Err("call view cannot acquire a source or owning initializer".into());
        }
    }
    if stages.keys().any(|stage| !ends.contains(stage)) {
        return Err("call view has no typed end".into());
    }
    if !stages.is_empty() {
        let cfg = ControlFlowGraph::analyze(function).map_err(|error| format!("{error:?}"))?;
        let facts = scope_facts(function);
        for (&stage, &start) in &starts {
            validate_stage(function, &cfg, &facts, stage, start)?;
        }
    }
    Ok(stages)
}

struct ScopeFacts {
    statements: Vec<BTreeSet<usize>>,
    terminator: BTreeSet<usize>,
}

fn referenced_locals(block: &BasicBlock) -> BTreeSet<usize> {
    let mut block = block.clone();
    let mut locals = BTreeSet::new();
    crate::sequences::prune::block_runtime_locals(
        &mut block,
        &mut |local| {
            locals.insert(local.index() as usize);
        },
        &mut |_| {},
    );
    locals
}

fn scope_facts(function: &Function) -> Vec<ScopeFacts> {
    function
        .blocks
        .iter()
        .map(|block| {
            let statements = block
                .statements
                .iter()
                .map(|statement| {
                    referenced_locals(&BasicBlock {
                        id: block.id,
                        statements: vec![statement.clone()],
                        terminator: Terminator {
                            kind: TerminatorKind::Unreachable,
                            span: statement.span,
                        },
                    })
                })
                .collect();
            let terminator = referenced_locals(&BasicBlock {
                id: block.id,
                statements: Vec::new(),
                terminator: block.terminator.clone(),
            });
            ScopeFacts {
                statements,
                terminator,
            }
        })
        .collect()
}

fn validate_stage(
    function: &Function,
    cfg: &ControlFlowGraph,
    facts: &[ScopeFacts],
    stage: usize,
    start: BlockId,
) -> Result<(), String> {
    let mut seen = vec![[false; 2]; function.blocks.len()];
    walk_stage(function, cfg, facts, stage, function.entry, &mut seen)?;
    // Unreachable compiler metadata is still a proof obligation. Start at its
    // initializer before examining any disconnected end/use component.
    if !seen[start.index() as usize].iter().any(|seen| *seen) {
        walk_stage(function, cfg, facts, stage, start, &mut seen)?;
    }
    for block in &function.blocks {
        let id = block.id.index() as usize;
        if !seen[id].iter().any(|seen| *seen)
            && (facts[id]
                .statements
                .iter()
                .any(|reads| reads.contains(&stage))
                || facts[id].terminator.contains(&stage))
        {
            walk_stage(function, cfg, facts, stage, block.id, &mut seen)?;
        }
    }
    Ok(())
}

fn walk_stage(
    function: &Function,
    cfg: &ControlFlowGraph,
    facts: &[ScopeFacts],
    stage: usize,
    start: BlockId,
    seen: &mut [[bool; 2]],
) -> Result<(), String> {
    let mut pending = VecDeque::from([(start, false)]);
    while let Some((id, mut active)) = pending.pop_front() {
        let index = id.index() as usize;
        if std::mem::replace(&mut seen[index][usize::from(active)], true) {
            continue;
        }
        let block = &function.blocks[index];
        for (statement, reads) in block.statements.iter().zip(&facts[index].statements) {
            match &statement.kind {
                StatementKind::BeginCallView { local, .. } if local.index() as usize == stage => {
                    if active {
                        return Err("call view begins while its previous scope is active".into());
                    }
                    active = true;
                }
                StatementKind::EndCallView { local } if local.index() as usize == stage => {
                    if !active {
                        return Err("call view ends without an active scope".into());
                    }
                    active = false;
                }
                _ if !active && reads.contains(&stage) => {
                    return Err("call view is read outside its internal scope".into());
                }
                _ => {}
            }
        }
        if !active && facts[index].terminator.contains(&stage) {
            return Err("call view is read outside its internal scope".into());
        }
        if active
            && matches!(
                block.terminator.kind,
                TerminatorKind::Return(_) | TerminatorKind::Respond(_)
            )
        {
            return Err("call view remains active at a successful exit".into());
        }
        pending.extend(cfg.successors(id).iter().map(|next| (*next, active)));
    }
    Ok(())
}
