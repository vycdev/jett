//! Logical source views backed by an exact original or prepared sequence loop.
use super::*;
use jett_hir::{BinaryOp, ExpressionKind as E, IterationPart, ViewIterationBinding};
use std::collections::BTreeSet;
#[cfg(test)]
mod tests;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct OriginalIteration {
    function: FunctionId,
    identity: FunctionIdentity,
    header: BlockId,
    body: BlockId,
    exit: BlockId,
    bindings: Vec<(LocalId, ViewIterationBinding)>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct PreparedViewIteration {
    original: OriginalIteration,
    source: SequenceSource,
    cursor: LocalId,
    length: LocalId,
    preheader: BlockId,
    runtime_span: Span,
}

impl PreparedViewIteration {
    pub(super) fn remap_blocks(&mut self, blocks: &[Option<BlockId>]) -> bool {
        let get = |id: BlockId| blocks.get(id.index() as usize).copied().flatten();
        let (Some(header), Some(body), Some(exit), Some(preheader)) = (
            get(self.original.header),
            get(self.original.body),
            get(self.original.exit),
            get(self.preheader),
        ) else {
            return false;
        };
        self.original.header = header;
        self.original.body = body;
        self.original.exit = exit;
        self.preheader = preheader;
        true
    }

    pub(super) fn remap_locals(&mut self, locals: &[Option<LocalId>]) -> bool {
        let get = |id: LocalId| locals.get(id.index() as usize).copied().flatten();
        let (Some(source), Some(cursor), Some(length)) =
            (get(self.source.root()), get(self.cursor), get(self.length))
        else {
            return false;
        };
        let Some(bindings) = self
            .original
            .bindings
            .iter()
            .map(|(local, proof)| get(*local).map(|local| (local, *proof)))
            .collect::<Option<Vec<_>>>()
        else {
            return false;
        };
        match &mut self.source {
            SequenceSource::Local(local) | SequenceSource::Projected { owner: local, .. } => {
                *local = source
            }
        }
        self.cursor = cursor;
        self.length = length;
        self.original.bindings = bindings;
        true
    }
}

/// A finite body region supplies backing at an invocation site only.
pub(super) struct IterationScope {
    bindings: Vec<(LocalId, ViewIterationBinding)>,
    region: BTreeSet<u32>,
}

pub(super) fn apply(
    scopes: &[IterationScope],
    block: BlockId,
    locals: &mut [hir::OwnershipLocalInfo],
    types: &TypeInterner,
) {
    for local in locals.iter_mut() {
        local.view_iteration = None;
    }
    for scope in scopes
        .iter()
        .filter(|scope| scope.region.contains(&block.index()))
    {
        for &(local, proof) in &scope.bindings {
            if !jett_typecheck::ownership::is_implicitly_copyable(types, proof.binder_type()) {
                locals[local.index() as usize].view_iteration = Some(proof);
            }
        }
    }
}

/// Forwarded aliases retain the binder's region instead of escaping it.
pub(super) fn ensure_scope(
    function: &Function,
    scopes: &[IterationScope],
    mut local: LocalId,
    block: BlockId,
) -> Result<(), String> {
    let mut seen = BTreeSet::new();
    loop {
        if !seen.insert(local.index()) {
            return Err("call ownership iteration alias backing is cyclic".into());
        }
        for scope in scopes {
            if scope.bindings.iter().any(|(binder, _)| *binder == local)
                && !scope.region.contains(&block.index())
            {
                return Err(
                    "call ownership iteration binding is used outside its exact body".into(),
                );
            }
        }
        let Some(source) = function.local(local).and_then(|local| local.view_source) else {
            return Ok(());
        };
        local = source;
    }
}

fn read_plan(
    function: &Function,
    header: BlockId,
    types: &TypeInterner,
) -> Result<Option<OriginalIteration>, String> {
    let block = function
        .blocks
        .get(header.index() as usize)
        .ok_or("call ownership iteration header is absent")?;
    let TerminatorKind::ForEach {
        key,
        value,
        by_view,
        iterable,
        body,
        exit,
    } = &block.terminator.kind
    else {
        return Ok(None);
    };
    if !by_view {
        return Ok(None);
    }
    if iterable.ty == TypeInterner::ERROR || iterable.ty.index() as usize >= types.len() {
        return Err("call ownership viewed iterable has an invalid exact type".into());
    }
    let key_part = match types.resolve(iterable.ty) {
        jett_types::Type::Map(..) => IterationPart::Key,
        jett_types::Type::List(_) | jett_types::Type::Set(_) | jett_types::Type::String
            if value.is_none() =>
        {
            IterationPart::Element
        }
        _ => return Err("call ownership viewed iteration has invalid binder roles".into()),
    };
    let mut bindings = Vec::new();
    for (id, part) in
        std::iter::once((*key, key_part)).chain(value.map(|id| (id, IterationPart::Value)))
    {
        if bindings.iter().any(|(existing, _)| *existing == id) {
            return Err("call ownership iteration reuses its binder".into());
        }
        let local = function
            .local(id)
            .ok_or("call ownership iteration binder is absent")?;
        let proof = hir::checked_view_iteration_binding(
            types,
            block.terminator.span,
            iterable.ty,
            local.ty,
            part,
        )?;
        binding_metadata(function, id, proof)?;
        if definitions(function, id) != 1 {
            return Err("call ownership iteration binder has another definition".into());
        }
        bindings.push((id, proof));
    }
    Ok(Some(OriginalIteration {
        function: function.id,
        identity: function.identity.clone(),
        header,
        body: *body,
        exit: *exit,
        bindings,
    }))
}

/// Capture only loops emitted from HIR already validated by the MIR entry gate.
pub(super) fn capture_original(function: &mut Function, types: &TypeInterner) {
    function.original_view_iterations = function
        .blocks
        .iter()
        .filter_map(|block| read_plan(function, block.id, types).ok().flatten())
        .collect();
}

pub(super) fn original_plan(
    function: &Function,
    header: BlockId,
    types: &TypeInterner,
) -> Result<Option<OriginalIteration>, String> {
    let actual = read_plan(function, header, types)?;
    let saved = function
        .original_view_iterations
        .iter()
        .filter(|plan| plan.header == header)
        .collect::<Vec<_>>();
    match (&actual, saved.as_slice()) {
        (None, []) => Ok(None),
        (Some(actual), [saved]) if actual == *saved => Ok(Some(actual.clone())),
        _ => {
            Err("call ownership original viewed loop differs from its validated HIR handoff".into())
        }
    }
}

impl OriginalIteration {
    pub(super) fn remap_blocks(&mut self, blocks: &[Option<BlockId>]) -> bool {
        let get = |id: BlockId| blocks.get(id.index() as usize).copied().flatten();
        let (Some(header), Some(body), Some(exit)) =
            (get(self.header), get(self.body), get(self.exit))
        else {
            return false;
        };
        self.header = header;
        self.body = body;
        self.exit = exit;
        true
    }
    pub(super) fn remap_locals(&mut self, locals: &[Option<LocalId>]) -> bool {
        let Some(bindings) = self
            .bindings
            .iter()
            .map(|(local, proof)| {
                locals
                    .get(local.index() as usize)
                    .copied()
                    .flatten()
                    .map(|local| (local, *proof))
            })
            .collect::<Option<Vec<_>>>()
        else {
            return false;
        };
        self.bindings = bindings;
        true
    }
}

/// Only the canonical sequence rewrite calls this beside its actual mutation.
pub(super) fn retain_prepared(
    function: &mut Function,
    original: OriginalIteration,
    source: SequenceSource,
    cursor: LocalId,
    length: LocalId,
    preheader: BlockId,
    runtime_span: Span,
) {
    function
        .prepared_view_iterations
        .push(PreparedViewIteration {
            original,
            source,
            cursor,
            length,
            preheader,
            runtime_span,
        });
}

pub(super) fn scopes(
    function: &Function,
    types: &TypeInterner,
) -> Result<Vec<IterationScope>, String> {
    let cfg = ControlFlowGraph::analyze(function)
        .map_err(|_| "call ownership iteration CFG is invalid")?;
    let mut result = Vec::new();
    let mut claimed = BTreeSet::new();
    for original in &function.original_view_iterations {
        claim_bindings(original, &mut claimed)?;
        let prepared = function
            .prepared_view_iterations
            .iter()
            .filter(|record| record.original.header == original.header)
            .collect::<Vec<_>>();
        match prepared.as_slice() {
            [] => {
                if original_plan(function, original.header, types)?.as_ref() != Some(original) {
                    return Err("call ownership original viewed loop lost its exact raw or prepared association".into());
                }
                result.push(IterationScope {
                    region: body_region(
                        function,
                        &cfg,
                        original.header,
                        original.body,
                        original.exit,
                        &BTreeSet::new(),
                    )?,
                    bindings: original.bindings.clone(),
                });
            }
            [record] if &record.original == original => {
                result.push(validate_prepared(function, types, &cfg, record)?)
            }
            _ => {
                return Err(
                    "call ownership viewed loop has duplicate or changed prepared association"
                        .into(),
                );
            }
        }
    }
    for block in &function.blocks {
        if matches!(
            block.terminator.kind,
            TerminatorKind::ForEach { by_view: true, .. }
        ) {
            original_plan(function, block.id, types)?
                .ok_or("call ownership viewed loop has no original association")?;
        }
    }
    for record in &function.prepared_view_iterations {
        if !function
            .original_view_iterations
            .iter()
            .any(|original| original == &record.original)
        {
            return Err("call ownership prepared iteration has no validated original loop".into());
        }
    }
    Ok(result)
}

fn claim_bindings(plan: &OriginalIteration, claimed: &mut BTreeSet<u32>) -> Result<(), String> {
    for &(local, _) in &plan.bindings {
        if !claimed.insert(local.index()) {
            return Err("call ownership duplicate iteration binding authority".into());
        }
    }
    Ok(())
}

fn binding_metadata(
    function: &Function,
    id: LocalId,
    proof: ViewIterationBinding,
) -> Result<(), String> {
    let local = function
        .local(id)
        .ok_or("call ownership viewed iteration binder is absent")?;
    let span = proof.loop_span();
    if local.mutable
        || local.view_source.is_some()
        || function.parameter_for_local(id).is_some()
        || local.ty != proof.binder_type()
        || local.debug_ty != local.ty
        || local.span.file != span.file
        || local.span.start < span.start
        || local.span.end > span.end
    {
        return Err(
            "call ownership viewed iteration binder changed its declaration or ABI role".into(),
        );
    }
    Ok(())
}

fn definitions(function: &Function, id: LocalId) -> usize {
    let mut count = 0;
    for block in &function.blocks {
        for statement in &block.statements {
            count += usize::from(match &statement.kind {
                StatementKind::Let { local, .. }
                | StatementKind::BeginCallView { local, .. }
                | StatementKind::CheckRefinement { local, .. } => *local == id,
                StatementKind::SequenceGet { target, .. }
                | StatementKind::SequenceLength { target, .. }
                | StatementKind::SumTake { target, .. }
                | StatementKind::SumTag { target, .. } => *target == id,
                StatementKind::Assign { target, .. } => {
                    matches!(target.kind, E::Local(local) if local == id)
                }
                _ => false,
            });
        }
        match &block.terminator.kind {
            TerminatorKind::ForEach { key, value, .. } => {
                count += usize::from(*key == id) + usize::from(*value == Some(id))
            }
            TerminatorKind::Switch { variants, .. } => {
                count += variants
                    .iter()
                    .map(|(_, _, bindings)| bindings.iter().filter(|&&local| local == id).count())
                    .sum::<usize>()
            }
            _ => {}
        }
    }
    count
}

fn body_region(
    function: &Function,
    cfg: &ControlFlowGraph,
    header: BlockId,
    body: BlockId,
    exit: BlockId,
    ends: &BTreeSet<u32>,
) -> Result<BTreeSet<u32>, String> {
    let valid = |id: BlockId| (id.index() as usize) < function.blocks.len();
    if !valid(header)
        || !valid(body)
        || !valid(exit)
        || header == body
        || body == exit
        || header == exit
    {
        return Err("call ownership iteration has invalid body/exit association".into());
    }
    let mut outside = BTreeSet::new();
    let mut pending = vec![function.entry, exit];
    while let Some(block) = pending.pop() {
        if block == header || !outside.insert(block.index()) {
            continue;
        }
        pending.extend_from_slice(cfg.successors(block));
    }
    let mut region = BTreeSet::new();
    let mut pending = vec![body];
    while let Some(block) = pending.pop() {
        if block == header
            || outside.contains(&block.index())
            || ends.contains(&block.index())
            || !region.insert(block.index())
        {
            continue;
        }
        pending.extend_from_slice(cfg.successors(block));
    }
    if !region.contains(&body.index()) {
        return Err("call ownership viewed iteration body is reachable outside its loop".into());
    }
    for &index in &region {
        if cfg.predecessors(BlockId(index)).iter().any(|id| {
            (*id == header && BlockId(index) != body)
                || (*id != header && !region.contains(&id.index()))
        }) {
            return Err("call ownership iteration body has an external entry".into());
        }
    }
    Ok(region)
}

fn local_expression(value: &Expression, id: LocalId, ty: TypeId, span: Span) -> bool {
    value.ty == ty && value.span == span && matches!(value.kind, E::Local(local) if local == id)
}

fn sequence_part(part: IterationPart) -> SequencePart {
    match part {
        IterationPart::Element => SequencePart::Element,
        IterationPart::Key => SequencePart::Key,
        IterationPart::Value => SequencePart::Value,
    }
}

fn validate_prepared(
    function: &Function,
    types: &TypeInterner,
    cfg: &ControlFlowGraph,
    record: &PreparedViewIteration,
) -> Result<IterationScope, String> {
    let plan = &record.original;
    if plan.function != function.id
        || plan.identity != function.identity
        || plan.bindings.is_empty()
        || plan.bindings.len() > 2
    {
        return Err("call ownership prepared iteration lost its original function/binders".into());
    }
    let iterable_type = plan.bindings[0].1.iterable_type();
    if record.source.ty(function) != Some(iterable_type) {
        return Err("call ownership prepared iteration changed its exact source type".into());
    }
    for &(local, proof) in &plan.bindings {
        let checked = hir::checked_view_iteration_binding(
            types,
            proof.loop_span(),
            iterable_type,
            proof.binder_type(),
            proof.part(),
        )?;
        if checked != proof
            || proof.loop_span() != plan.bindings[0].1.loop_span()
            || proof.iterable_type() != iterable_type
        {
            return Err("call ownership prepared iteration changed its exact endpoint".into());
        }
        binding_metadata(function, local, proof)?;
        if definitions(function, local) != 1 {
            return Err("call ownership prepared iteration binder has another definition".into());
        }
    }
    for id in [record.cursor, record.length] {
        if function.local(id).is_none_or(|local| {
            local.ty != TypeInterner::INT64 || local.debug_ty != TypeInterner::INT64
        }) || function.parameter_for_local(id).is_some()
        {
            return Err("call ownership prepared iterator cursor/length metadata changed".into());
        }
    }
    let header = function
        .blocks
        .get(plan.header.index() as usize)
        .ok_or("call ownership prepared iteration header is absent")?;
    let TerminatorKind::Branch {
        condition,
        then_block,
        else_block,
    } = &header.terminator.kind
    else {
        return Err("call ownership prepared iteration lost its selecting Branch".into());
    };
    let E::Binary {
        left,
        op: BinaryOp::Less,
        right,
    } = &condition.kind
    else {
        return Err("call ownership prepared iteration lost its exact cursor comparison".into());
    };
    if header.terminator.span != plan.bindings[0].1.loop_span()
        || *then_block != plan.body
        || condition.ty != TypeInterner::BOOL
        || condition.span != record.runtime_span
        || !local_expression(
            left,
            record.cursor,
            TypeInterner::INT64,
            record.runtime_span,
        )
        || !local_expression(
            right,
            record.length,
            TypeInterner::INT64,
            record.runtime_span,
        )
    {
        return Err("call ownership prepared iteration changed its cursor or entered body".into());
    }
    let preheader = function
        .blocks
        .get(record.preheader.index() as usize)
        .ok_or("call ownership iterator preheader is absent")?;
    if !matches!(preheader.terminator.kind, TerminatorKind::Goto(target) if target == plan.header) {
        return Err("call ownership iterator preheader bypasses its original header".into());
    }
    let [start, zero, length] = preheader
        .statements
        .get(preheader.statements.len().saturating_sub(3)..)
        .ok_or("call ownership iterator initialization is absent")?
    else {
        return Err("call ownership iterator initialization is incomplete".into());
    };
    if [start, zero, length]
        .iter()
        .any(|statement| statement.span != record.runtime_span)
        || !matches!(&start.kind, StatementKind::IterationBorrow { source, token, start: true }
            if *source == record.source && *token == record.cursor)
        || !matches!(&zero.kind, StatementKind::Let { local, value }
            if *local == record.cursor && value.ty == TypeInterner::INT64 && value.span == record.runtime_span && matches!(value.kind, E::Int(0)))
        || !matches!(&length.kind, StatementKind::SequenceLength { source, target }
            if *source == record.source && *target == record.length)
    {
        return Err("call ownership iterator lost its exact borrow/source initialization".into());
    }
    let body = function
        .blocks
        .get(plan.body.index() as usize)
        .ok_or("call ownership prepared iteration body is absent")?;
    for (index, &(local, proof)) in plan.bindings.iter().enumerate() {
        let statement = body
            .statements
            .get(index)
            .ok_or("call ownership viewed extraction is absent")?;
        if statement.span != record.runtime_span
            || !matches!(&statement.kind, StatementKind::SequenceGet {
            consume: false, source, index, target, part } if *source == record.source && *index == record.cursor
                && *target == local && *part == sequence_part(proof.part()))
        {
            return Err(
                "call ownership viewed extraction changed source/cursor/target/role or consumption"
                    .into(),
            );
        }
    }
    let advance = body
        .statements
        .get(plan.bindings.len())
        .ok_or("call ownership iterator advance is absent")?;
    let StatementKind::Assign { target, value } = &advance.kind else {
        return Err("call ownership iterator advance changed its statement".into());
    };
    let E::Binary {
        left,
        op: BinaryOp::Add,
        right,
    } = &value.kind
    else {
        return Err("call ownership iterator advance lost its checked addition".into());
    };
    if advance.span != record.runtime_span
        || value.ty != TypeInterner::INT64
        || value.span != record.runtime_span
        || !local_expression(
            target,
            record.cursor,
            TypeInterner::INT64,
            record.runtime_span,
        )
        || !local_expression(
            left,
            record.cursor,
            TypeInterner::INT64,
            record.runtime_span,
        )
        || right.ty != TypeInterner::INT64
        || right.span != record.runtime_span
        || !matches!(right.kind, E::Int(1))
    {
        return Err("call ownership iterator advance changed its exact cursor".into());
    }
    let mut starts = 0;
    let mut ends = BTreeSet::new();
    for block in &function.blocks {
        for statement in &block.statements {
            if let StatementKind::IterationBorrow {
                source,
                token,
                start,
            } = &statement.kind
                && *token == record.cursor
            {
                if *source != record.source || statement.span != record.runtime_span {
                    return Err("call ownership iterator borrow changed its exact source".into());
                }
                if *start {
                    starts += 1;
                } else {
                    if block.statements.len() != 1
                        || !matches!(block.terminator.kind, TerminatorKind::Goto(_))
                    {
                        return Err("call ownership iterator end lost its canonical edge".into());
                    }
                    ends.insert(block.id.index());
                }
            }
        }
    }
    if starts != 1 {
        return Err("call ownership iterator has missing/duplicate borrow start".into());
    }
    if !ends.contains(&else_block.index())
        || !matches!(function.blocks[else_block.index() as usize].terminator.kind,
        TerminatorKind::Goto(exit) if exit == plan.exit)
    {
        return Err(
            "call ownership iterator false edge bypasses its exact end and original exit".into(),
        );
    }
    let region = body_region(function, cfg, plan.header, plan.body, plan.exit, &ends)?;
    if ends.iter().any(|&end| {
        matches!(function.blocks[end as usize].terminator.kind,
        TerminatorKind::Goto(target) if target == plan.header || region.contains(&target.index()))
    }) {
        return Err("call ownership iterator end re-enters its completed region".into());
    }
    for block in std::iter::once(plan.header).chain(region.iter().copied().map(BlockId)) {
        if cfg.successors(block).iter().any(|id| {
            *id != plan.header && !region.contains(&id.index()) && !ends.contains(&id.index())
        }) {
            return Err(
                "call ownership iterator leaves its region without the exact borrow end".into(),
            );
        }
    }
    if ends.iter().any(|&end| {
        cfg.predecessors(BlockId(end)).is_empty()
            || cfg
                .predecessors(BlockId(end))
                .iter()
                .any(|id| *id != plan.header && !region.contains(&id.index()))
    }) {
        return Err("call ownership iterator end is outside its exact loop region".into());
    }
    Ok(IterationScope {
        bindings: plan.bindings.clone(),
        region,
    })
}
