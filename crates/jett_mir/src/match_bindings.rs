//! Exact HIR match payload associations, scoped to their original arm CFG.
use super::*;
use std::collections::BTreeSet;

// These wrappers change checked qualification, not the backing carrier.
// Peel only the actual refinement chain or explicit secret layer relation.
pub(super) fn requires_snapshot(
    types: &TypeInterner,
    mut expression: &Expression,
    view_locals: &[LocalId],
) -> Result<bool, String> {
    loop {
        if expression.ty.index() as usize >= types.len() {
            return Err("match scrutinee has an invalid checked carrier type".into());
        }
        expression = match &expression.kind {
            hir::ExpressionKind::View(value) if value.ty == expression.ty => return Ok(true),
            hir::ExpressionKind::View(_) => {
                return Err("match view changed its exact carrier type".into());
            }
            hir::ExpressionKind::Field { .. } => return Ok(true),
            hir::ExpressionKind::Local(local) => return Ok(view_locals.contains(local)),
            hir::ExpressionKind::Coarsen(value) => {
                let mut ty = value.ty;
                let mut seen = BTreeSet::new();
                let mut matched = false;
                while (ty.index() as usize) < types.len() && seen.insert(ty.index()) {
                    let jett_types::Type::Refinement { base, .. } = types.resolve(ty) else {
                        break;
                    };
                    if *base == expression.ty {
                        matched = true;
                        break;
                    }
                    ty = *base;
                }
                if !matched {
                    return Err(
                        "match coarsen changed its exact refinement carrier relation".into(),
                    );
                }
                value
            }
            hir::ExpressionKind::Declassify(value) => {
                let mut ty = value.ty;
                let mut seen = BTreeSet::new();
                while (ty.index() as usize) < types.len() && seen.insert(ty.index()) {
                    match types.resolve(ty) {
                        jett_types::Type::Refinement { base, .. } => ty = *base,
                        _ => break,
                    }
                }
                if ty.index() as usize >= types.len()
                    || !matches!(types.resolve(ty), jett_types::Type::Secret(inner) if *inner == expression.ty)
                {
                    return Err("match declassify changed its exact secret carrier relation".into());
                }
                value
            }
            _ => return Ok(false),
        };
    }
}

#[derive(Debug, Clone, PartialEq)]
struct Arm {
    entry: BlockId,
    region: BTreeSet<BlockId>,
    bindings: Vec<(Local, hir::MatchBinding)>,
}

#[derive(Debug, Clone, PartialEq)]
struct Match {
    header: BlockId,
    selector: Terminator,
    arms: Vec<Arm>,
}

#[derive(Debug, Clone, PartialEq)]
pub(super) struct Witness {
    function: FunctionId,
    identity: FunctionIdentity,
    matches: Vec<Match>,
}

#[derive(Clone, Default)]
pub(super) struct Capture {
    matches: Vec<Match>,
}

impl Capture {
    // Called only by actual lowering of a HIR Match after the entry validator.
    pub(super) fn record(
        &mut self,
        header: BlockId,
        selector: Terminator,
        match_span: Span,
        regions: Vec<(BlockId, BTreeSet<BlockId>)>,
        locals: &[Local],
        types: &TypeInterner,
    ) -> Result<(), String> {
        let TerminatorKind::Switch {
            scrutinee,
            variants,
            ..
        } = &selector.kind
        else {
            return Err("call ownership match capture is not its lowered Switch".into());
        };
        let mut arms = Vec::new();
        for (variant, entry, ids) in variants {
            let region = regions
                .iter()
                .find(|(block, _)| block == entry)
                .ok_or("call ownership match capture lost its exact arm region")?
                .1
                .clone();
            let mut bindings = Vec::new();
            for (field_index, id) in ids.iter().enumerate() {
                let local = locals
                    .get(id.index() as usize)
                    .filter(|local| local.id == *id)
                    .ok_or("call ownership match capture lost its binder header")?;
                if jett_typecheck::ownership::is_implicitly_copyable(types, local.ty) {
                    continue;
                }
                let proof = hir::checked_match_binding(
                    types,
                    match_span,
                    scrutinee.ty,
                    *variant,
                    field_index,
                    local.ty,
                    local.span,
                )?;
                bindings.push((local.clone(), proof));
            }
            if !bindings.is_empty() {
                arms.push(Arm {
                    entry: *entry,
                    region,
                    bindings,
                });
            }
        }
        if !arms.is_empty() {
            self.matches.push(Match {
                header,
                selector,
                arms,
            });
        }
        Ok(())
    }

    pub(super) fn finish(
        self,
        function: &Function,
        types: &TypeInterner,
    ) -> Result<Option<Witness>, String> {
        if self.matches.is_empty() {
            return Ok(None);
        }
        let witness = Witness {
            function: function.id,
            identity: function.identity.clone(),
            matches: self.matches,
        };
        scopes_with(function, types, Some(&witness))?;
        Ok(Some(witness))
    }
}

pub(super) struct Scope {
    region: BTreeSet<BlockId>,
    bindings: Vec<(LocalId, hir::MatchBinding)>,
}

pub(super) fn has_records(function: &Function) -> bool {
    function.match_bindings.is_some()
}

pub(super) fn apply(scopes: &[Scope], block: BlockId, locals: &mut [hir::OwnershipLocalInfo]) {
    for local in locals.iter_mut() {
        local.match_binding = None;
    }
    for scope in scopes.iter().filter(|scope| scope.region.contains(&block)) {
        for &(local, proof) in &scope.bindings {
            locals[local.index() as usize].match_binding = Some(proof);
        }
    }
}

pub(super) fn ensure_scope(
    function: &Function,
    scopes: &[Scope],
    mut local: LocalId,
    block: BlockId,
) -> Result<(), String> {
    let mut seen = BTreeSet::new();
    loop {
        if !seen.insert(local.index()) {
            return Err("call ownership match alias backing is cyclic".into());
        }
        for scope in scopes {
            if scope.bindings.iter().any(|(binder, _)| *binder == local)
                && !scope.region.contains(&block)
            {
                return Err("call ownership match binding is used outside its exact arm".into());
            }
        }
        let Some(source) = function.local(local).and_then(|local| local.view_source) else {
            return Ok(());
        };
        local = source;
    }
}

pub(super) fn scopes(function: &Function, types: &TypeInterner) -> Result<Vec<Scope>, String> {
    scopes_with(function, types, function.match_bindings.as_ref())
}

fn scopes_with(
    function: &Function,
    types: &TypeInterner,
    witness: Option<&Witness>,
) -> Result<Vec<Scope>, String> {
    let Some(witness) = witness else {
        return Ok(Vec::new());
    };
    if witness.function != function.id || witness.identity != function.identity {
        return Err("call ownership match proof belongs to another function".into());
    }
    let cfg =
        ControlFlowGraph::analyze(function).map_err(|_| "call ownership match CFG is invalid")?;
    let mut claimed = BTreeSet::new();
    let mut result = Vec::new();
    for original in &witness.matches {
        let block = function
            .blocks
            .get(original.header.index() as usize)
            .filter(|block| block.id == original.header)
            .ok_or("call ownership match selector is absent")?;
        if !crate::breakpoint_regions::terminator_equal(&block.terminator, &original.selector) {
            return Err("call ownership match changed its exact selecting Switch".into());
        }
        let TerminatorKind::Switch {
            scrutinee,
            variants,
            ..
        } = &block.terminator.kind
        else {
            return Err("call ownership match lost its selecting Switch".into());
        };
        for arm in &original.arms {
            if !arm.region.contains(&arm.entry)
                || arm.region.contains(&original.header)
                || arm.region.contains(&function.entry)
            {
                return Err("call ownership match changed its finite arm boundary".into());
            }
            for &id in &arm.region {
                if function
                    .blocks
                    .get(id.index() as usize)
                    .is_none_or(|block| block.id != id)
                    || cfg.predecessors(id).iter().any(|source| {
                        !arm.region.contains(source)
                            && !(*source == original.header && id == arm.entry)
                    })
                {
                    return Err("call ownership match arm has an external entry".into());
                }
            }
            let mut bindings = Vec::new();
            for (saved, proof) in &arm.bindings {
                let current = function
                    .local(saved.id)
                    .ok_or("call ownership match binder is absent")?;
                if current != saved
                    || current.mutable
                    || current.view_source.is_some()
                    || function.parameter_for_local(current.id).is_some()
                    || crate::iteration_views::definitions(function, current.id) != 1
                    || !claimed.insert(current.id.index())
                {
                    return Err(
                        "call ownership match binder changed its declaration or defining edge"
                            .into(),
                    );
                }
                let actual = hir::checked_match_binding(
                    types,
                    proof.match_span(),
                    scrutinee.ty,
                    proof.variant(),
                    proof.field_index(),
                    current.ty,
                    current.span,
                )?;
                if actual != *proof
                    || !variants.iter().any(|(variant, entry, fields)| {
                        *variant == proof.variant()
                            && *entry == arm.entry
                            && fields.get(proof.field_index()) == Some(&current.id)
                    })
                {
                    return Err(
                        "call ownership match changed its exact enum/tag/field association".into(),
                    );
                }
                bindings.push((current.id, *proof));
            }
            result.push(Scope {
                region: arm.region.clone(),
                bindings,
            });
        }
    }
    Ok(result)
}

pub(super) fn remap_blocks(function: &mut Function, map: &[Option<BlockId>]) -> Result<(), String> {
    let Some(mut witness) = function.match_bindings.clone() else {
        return Ok(());
    };
    let get = |id: BlockId| map.get(id.index() as usize).copied().flatten();
    let mut matches = Vec::new();
    for mut record in witness.matches {
        let Some(header) = get(record.header) else {
            if record
                .arms
                .iter()
                .any(|arm| arm.region.iter().any(|id| get(*id).is_some()))
            {
                return Err("call ownership match selector removal left an arm fragment".into());
            }
            continue;
        };
        record.header = header;
        let missing = std::cell::Cell::new(false);
        crate::sequences::prune::block_targets(&mut record.selector.kind, &|id| match get(*id) {
            Some(new) => *id = new,
            None => missing.set(true),
        });
        if missing.get() {
            return Err("call ownership match remap removed a selected arm".into());
        }
        for arm in &mut record.arms {
            arm.entry = get(arm.entry).ok_or("call ownership match remap lost its binder entry")?;
            arm.region = arm.region.iter().filter_map(|id| get(*id)).collect();
        }
        matches.push(record);
    }
    witness.matches = matches;
    function.match_bindings = Some(witness);
    Ok(())
}

pub(super) fn remap_locals(function: &mut Function, map: &[Option<LocalId>]) -> Result<(), String> {
    let Some(mut witness) = function.match_bindings.clone() else {
        return Ok(());
    };
    let get = |id: LocalId| map.get(id.index() as usize).copied().flatten();
    for record in &mut witness.matches {
        let mut block = BasicBlock {
            id: record.header,
            statements: Vec::new(),
            terminator: record.selector.clone(),
        };
        let missing = std::cell::Cell::new(false);
        crate::sequences::prune::block_locals(
            &mut block,
            &mut |id| match get(*id) {
                Some(new) => *id = new,
                None => missing.set(true),
            },
            &mut |_| {},
        );
        if missing.get() {
            return Err("call ownership match selector lost a retained local".into());
        }
        record.selector = block.terminator;
        for arm in &mut record.arms {
            for (local, _) in &mut arm.bindings {
                local.id =
                    get(local.id).ok_or("call ownership match binder lost its local remap")?;
            }
        }
    }
    function.match_bindings = Some(witness);
    Ok(())
}

pub(super) fn sequence_transition(
    function: &mut Function,
    before: &Function,
    edit: &crate::breakpoint_regions::SequenceEdit,
    types: &TypeInterner,
) -> Result<(), String> {
    let Some(mut witness) = before.match_bindings.clone() else {
        return Ok(());
    };
    scopes(before, types)?;
    let mut shadow = function.clone();
    shadow.breakpoint_regions = before.breakpoint_regions.clone();
    crate::breakpoint_regions::sequence_transition(&mut shadow, before, edit.clone(), types)?;
    for record in &mut witness.matches {
        // Sequence preparation never stages a new scrutinee: lower_match
        // already lowered calls/handlers before Capture::record. Its only
        // possible selector edit is an authenticated edge-cleanup split.
        for (source, old, new) in &edit.redirects {
            if *source != record.header {
                continue;
            }
            if !crate::breakpoint_regions::terminator_equal(&record.selector, old) {
                return Err(
                    "call ownership match redirect lost its exact selecting before-edge".into(),
                );
            }
            let TerminatorKind::Switch {
                variants: old_variants,
                otherwise: old_otherwise,
                ..
            } = &old.kind
            else {
                return Err("call ownership match redirect changed its selector kind".into());
            };
            let TerminatorKind::Switch {
                variants: new_variants,
                otherwise: new_otherwise,
                ..
            } = &new.kind
            else {
                return Err("call ownership match redirect is no longer a Switch".into());
            };
            if old_variants.len() != new_variants.len()
                || old_otherwise.is_some() != new_otherwise.is_some()
                || old_variants
                    .iter()
                    .zip(new_variants)
                    .any(|(old, new)| old.0 != new.0 || old.2 != new.2)
            {
                return Err("call ownership match redirect changed tag or payload binders".into());
            }
            let mut expected = old.clone();
            let mut position = 0;
            let targets = new_variants
                .iter()
                .map(|(_, target, _)| *target)
                .chain(new_otherwise.iter().copied())
                .collect::<Vec<_>>();
            if let TerminatorKind::Switch {
                variants,
                otherwise,
                ..
            } = &mut expected.kind
            {
                for (_, target, _) in variants {
                    *target = targets[position];
                    position += 1;
                }
                if let Some(target) = otherwise {
                    *target = targets[position];
                }
            }
            if !crate::breakpoint_regions::terminator_equal(&expected, new) {
                return Err(
                    "call ownership match redirect changed its scrutinee or Source span".into(),
                );
            }
            for arm in &mut record.arms {
                if let Some((old, new)) = old_variants
                    .iter()
                    .zip(new_variants)
                    .find(|(old, _)| old.1 == arm.entry)
                {
                    if old.1 != new.1 {
                        if !edit.blocks.iter().any(|block| block.id == new.1
                            && matches!(block.terminator.kind, TerminatorKind::Goto(target) if target == old.1)) {
                            return Err("call ownership match redirect lacks its exact cleanup edge block".into());
                        }
                        arm.entry = new.1;
                        arm.region.insert(new.1);
                    }
                }
            }
            record.selector = new.clone();
        }
        for arm in &mut record.arms {
            for block in &edit.blocks {
                if edit.redirects.iter().any(|(source, _, term)| {
                    if !arm.region.contains(source) {
                        return false;
                    }
                    let found = std::cell::Cell::new(false);
                    let mut kind = term.kind.clone();
                    crate::sequences::prune::block_targets(&mut kind, &|target| {
                        if *target == block.id {
                            found.set(true);
                        }
                    });
                    found.get()
                }) {
                    arm.region.insert(block.id);
                }
            }
        }
    }
    scopes_with(function, types, Some(&witness))?;
    function.match_bindings = Some(witness);
    Ok(())
}

#[cfg(test)]
mod tests;
