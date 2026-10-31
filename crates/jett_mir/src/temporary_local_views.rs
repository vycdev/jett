//! Source-authenticated whole owners for function-body temporary local views.
//! The current graph is sealed separately from the immutable HIR occurrence.
//! Pins are cleanup obligations; they never establish availability or reads.

use super::*;
use hir::{CallOwnership, ExpressionKind as E};
use std::collections::BTreeSet;
use std::sync::Arc;

struct Original {
    // Retains the exact physical HIR input, including mutable_sum_backings and
    // all independent opaque callable/source receipts. No HIR field is added.
    source: hir::Function,
    inventory: Vec<hir::TemporaryLocalBacking>,
}
impl std::fmt::Debug for Original {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Original")
            .field("source", &self.source)
            .field("occurrences", &self.inventory.len())
            .finish()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Site {
    block: BlockId,
    statement: usize,
}
impl Site {
    fn remap_blocks(&mut self, map: &[Option<BlockId>]) -> Result<(), String> {
        self.block = map
            .get(self.block.index() as usize)
            .copied()
            .flatten()
            .ok_or("temporary view compaction removed its owning declaration")?;
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Acquisition {
    Direct,
    // The physical Call must complete before call loans end. The whole result
    // then moves to the hidden owner; its ordinary intermediate slot is cleared.
    Staged {
        local: LocalId,
        call: Site,
        ends: Vec<Site>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Row {
    occurrence: usize,
    owner: LocalId,
    alias: LocalId,
    initialize: Site,
    publish: Site,
    acquisition: Acquisition,
}
impl Row {
    fn remap_blocks(&mut self, map: &[Option<BlockId>]) -> Result<(), String> {
        self.initialize.remap_blocks(map)?;
        self.publish.remap_blocks(map)?;
        if let Acquisition::Staged { call, ends, .. } = &mut self.acquisition {
            call.remap_blocks(map)?;
            for site in ends {
                site.remap_blocks(map)?;
            }
        }
        Ok(())
    }
    fn remap_locals(&mut self, map: &[Option<LocalId>]) -> Result<(), String> {
        self.owner = mapped_local(map, self.owner)?;
        self.alias = mapped_local(map, self.alias)?;
        if let Acquisition::Staged { local, .. } = &mut self.acquisition {
            *local = mapped_local(map, *local)?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone)]
struct Graph {
    id: FunctionId,
    identity: FunctionIdentity,
    debug_kind: hir::FunctionDebugKind,
    params: Vec<Param>,
    capture_count: usize,
    return_type: TypeId,
    locals: Vec<Local>,
    entry: BlockId,
    blocks: Vec<BasicBlock>,
    span: Span,
    actor_state_views: Vec<hir::ActorStateView>,
    actor_message_source: bool,
}
impl Graph {
    fn new(function: &Function) -> Self {
        Self {
            id: function.id,
            identity: function.identity.clone(),
            debug_kind: function.debug_kind.clone(),
            params: function.params.clone(),
            capture_count: function.capture_count,
            return_type: function.return_type,
            locals: function.locals.clone(),
            entry: function.entry,
            blocks: function.blocks.clone(),
            span: function.span,
            actor_state_views: function.actor_state_views.clone(),
            actor_message_source: function.actor_message_source,
        }
    }
    fn matches(&self, function: &Function) -> bool {
        self.id == function.id
            && self.identity == function.identity
            && self.debug_kind == function.debug_kind
            && self.params == function.params
            && self.capture_count == function.capture_count
            && self.return_type == function.return_type
            && self.locals == function.locals
            && self.entry == function.entry
            && self.span == function.span
            && self.actor_state_views == function.actor_state_views
            && self.actor_message_source == function.actor_message_source
            // This comparison retains f64 bits and private callable identities.
            && breakpoint_regions::blocks_equal(&self.blocks, &function.blocks)
    }
}

#[derive(Debug, Clone)]
struct Current {
    graph: Graph,
    rows: Vec<Row>,
}

#[derive(Debug, Clone)]
pub(super) struct Witness {
    original: Arc<Original>,
    rows: Vec<Row>,
    current: Arc<Current>,
}
impl PartialEq for Witness {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.original, &other.original)
            && Arc::ptr_eq(&self.current, &other.current)
            && self.rows == other.rows
    }
}
impl Witness {
    fn current(&self, function: &Function) -> Result<(), String> {
        if !self.current.graph.matches(function)
            || self.current.rows != self.rows
            || self.original.source.id != function.id
            || self.original.source.identity != function.identity
            || self.rows.len() != self.original.inventory.len()
        {
            return Err("temporary local view changed its exact original inventory or current semantic graph".into());
        }
        for (index, row) in self.rows.iter().enumerate() {
            if row.occurrence != index {
                return Err("temporary local view reordered its original occurrences".into());
            }
            let proof = &self.original.inventory[index];
            if proof.original_scope().body_span() != self.original.source.body.span {
                return Err(
                    "temporary local view is outside the supported original function scope".into(),
                );
            }
            validate_row(function, row)?;
        }
        Ok(())
    }
}

pub(super) fn has_records(function: &Function) -> bool {
    function.temporary_local_views.is_some()
}

fn has_receipts(function: &Function) -> bool {
    let mut found = false;
    for block in &function.blocks {
        resource_ownership::walk::mir_block(block, &mut |value| {
            let packet = match &value.kind {
                E::Call { ownership, .. }
                | E::IndirectCall { ownership, .. }
                | E::Intrinsic { ownership, .. }
                | E::ResourceInvoke { ownership, .. } => Some(ownership),
                _ => None,
            };
            found |= packet.is_some_and(CallOwnership::has_temporary_local_receipts);
        });
    }
    found
}

pub(super) fn validate_current(function: &Function) -> Result<(), String> {
    if let Some(witness) = &function.temporary_local_views {
        witness.current(function)?;
    } else if has_receipts(function) {
        return Err("temporary local view invocation has no original private MIR witness".into());
    }
    Ok(())
}

pub(super) fn validate(function: &Function, types: &TypeInterner) -> Result<(), String> {
    validate_current(function)?;
    let Some(witness) = &function.temporary_local_views else {
        return Ok(());
    };
    for row in &witness.rows {
        let owner = function
            .local(row.owner)
            .ok_or("temporary owner header is absent")?;
        let alias = function
            .local(row.alias)
            .ok_or("temporary alias header is absent")?;
        let (_, value) = let_at(function, row.publish)?;
        if owner.mutable
            || owner.view_source.is_some()
            || alias.mutable
            || alias.view_source != Some(row.owner)
            || !move_values::is_linear(types, owner.ty)
            || resource_type_pending(types, owner.ty)
            || function.is_view_local(row.owner)
            || function.parameter_for_local(row.owner).is_some()
            || function.parameter_for_local(row.alias).is_some()
        {
            return Err("temporary local view requires one ordinary whole owning slot and one nonowning alias".into());
        }
        hir::validate_local_view_initializer(value, row.owner, owner.ty, alias.ty, types)?;
    }
    Ok(())
}

/// The whole-program lowering entry point alone may mint initial MIR witnesses.
/// HIR joins its retained expected archive even when a call marker is missing.
/// Do not replace this call with an optional-marker scan of current expressions.
pub(super) fn capture(
    program: &mut Program,
    source: &hir::Program,
    types: &TypeInterner,
) -> Result<(), String> {
    if program.functions.len() != source.functions.len() {
        return Err("temporary local view capture changed the original function inventory".into());
    }
    for (function, original) in program.functions.iter_mut().zip(&source.functions) {
        if function.id != original.id
            || function.identity != original.identity
            || function.debug_kind != original.debug_kind
            || function.params != original.params
            || function.capture_count != original.capture_count
            || function.return_type != original.return_type
            || function.span != original.span
            || !function.locals.starts_with(&original.locals)
        {
            return Err(
                "temporary local view capture changed the original function identity".into(),
            );
        }
        let inventory = hir::temporary_local_backings(source, original, types)?;
        if inventory.is_empty() {
            continue;
        }
        let mut rows = Vec::with_capacity(inventory.len());
        for (occurrence, proof) in inventory.iter().enumerate() {
            let (source_owner, source_alias) = source_pair(original, proof)?;
            if !proof.matches_owner_initializer(source_owner)
                || !breakpoint_regions::expressions_equal(
                    source_alias,
                    proof.rewritten_initializer(),
                )
            {
                return Err(
                    "temporary local view changed the exact physical HIR initializer pair".into(),
                );
            }
            let (initialize, owner_value) = unique_let(function, proof.owner())?;
            let (publish, alias_value) = unique_let(function, proof.alias())?;
            if !breakpoint_regions::expressions_equal(alias_value, source_alias) {
                return Err(
                    "temporary local view changed its original typed alias projection".into(),
                );
            }
            let acquisition = match owner_value.kind {
                E::Call { .. } => {
                    same_source_producer(source_owner, owner_value)?;
                    Acquisition::Direct
                }
                E::Local(local) => {
                    let (call, value) = unique_let(function, local)?;
                    same_source_producer(source_owner, value)?;
                    if call.block != initialize.block || call.statement >= initialize.statement {
                        return Err("temporary acquisition crossed a block or control-flow merge".into());
                    }
                    let staged = function.local(local).ok_or("temporary acquisition header is absent")?;
                    // finish_call_view_scope acquires its complete result with
                    // Builder::temporary. Its mutable flag describes generated
                    // storage, not a mutable Source binding. Require that exact
                    // appended constructor header; the persistent owner/alias
                    // still retain their original immutable HIR headers.
                    if (local.index() as usize) < original.locals.len()
                        || function.parameter_for_local(local).is_some()
                        || staged.name != format!("$native{}", local.index())
                        || staged.ty != owner_value.ty
                        || staged.ty != value.ty
                        || staged.debug_ty != staged.ty
                        || staged.debug_type_name.is_some()
                        || staged.span != value.span
                        || staged.span != owner_value.span
                        || staged.view_source.is_some()
                        || !staged.mutable
                    {
                        return Err("temporary acquisition is not a generated whole owning result".into());
                    }
                    let ends = ((call.statement + 1)..initialize.statement)
                        .map(|statement| Site { block: call.block, statement })
                        .collect::<Vec<_>>();
                    if ends.is_empty()
                        || ends.iter().any(|site| !matches!(
                            function.blocks[site.block.index() as usize].statements[site.statement].kind,
                            StatementKind::EndCallView { .. }
                        ))
                    {
                        return Err("temporary acquisition changed its exact call-loan end chain".into());
                    }
                    Acquisition::Staged { local, call, ends }
                }
                _ => return Err("pending temporary local view acquisition: expected a named whole-owner Call or its exact staged move".into()),
            };
            let row = Row {
                occurrence,
                owner: proof.owner(),
                alias: proof.alias(),
                initialize,
                publish,
                acquisition,
            };
            validate_row(function, &row)?;
            rows.push(row);
        }
        let witness = Witness {
            original: Arc::new(Original {
                source: original.clone(),
                inventory,
            }),
            current: Arc::new(Current {
                graph: Graph::new(function),
                rows: rows.clone(),
            }),
            rows,
        };
        witness.current(function)?;
        function.temporary_local_views = Some(witness);
        validate(function, types)?;
    }
    Ok(())
}

fn source_pair<'a>(
    function: &'a hir::Function,
    proof: &hir::TemporaryLocalBacking,
) -> Result<(&'a Expression, &'a Expression), String> {
    // The API authenticated the original occurrence and root scope. Check the
    // actual physical input again; a matching public Local header is not proof.
    let mut owner = None;
    let mut alias = None;
    for (index, statement) in function.body.statements.iter().enumerate() {
        if let hir::StatementKind::Let { local, value } = &statement.kind {
            let target = if *local == proof.owner() {
                &mut owner
            } else if *local == proof.alias() {
                &mut alias
            } else {
                continue;
            };
            if target.replace((index, value)).is_some() {
                return Err(
                    "temporary local view duplicated its original physical HIR declaration".into(),
                );
            }
        }
    }
    let (owner_index, owner) =
        owner.ok_or("temporary local view has no physical HIR owner initialization")?;
    let (alias_index, alias) =
        alias.ok_or("temporary local view has no physical HIR alias initialization")?;
    if alias_index != owner_index + 1 {
        return Err("temporary local view moved its physical HIR alias publication".into());
    }
    Ok((owner, alias))
}

/// Operand extraction changes only the packet's per-argument staging. Retain
/// every independent private marker and opaque callable token in the comparison.
/// call_ownership's complete gate validates the actual argument stage operations
/// and their lexical source order after this initial association is captured.
fn same_source_producer(original: &Expression, physical: &Expression) -> Result<(), String> {
    let (
        E::Call {
            function: original_target,
            args: original_args,
            evaluation_order: original_order,
            ownership: original_ownership,
        },
        E::Call {
            function: physical_target,
            args: physical_args,
            evaluation_order: physical_order,
            ownership: CallOwnership::Source(physical_packet),
        },
    ) = (&original.kind, &physical.kind)
    else {
        return Err("temporary producer requires its exact original named Source Call".into());
    };
    let CallOwnership::Source(original_packet) = original_ownership else {
        return Err("temporary producer has no original Source acquisition receipt".into());
    };
    if !original_packet.generated_operands.is_empty()
        || !physical_packet.generated_operands.is_empty()
        || original_target != physical_target
        || original.ty != physical.ty
        || original.span != physical.span
        || original_order != physical_order
        || original_args.len() != physical_args.len()
        || original_packet.arguments.len() != physical_packet.arguments.len()
    {
        return Err(
            "pending temporary producer composition or changed original producer identity".into(),
        );
    }
    let mut restored = physical_packet.clone();
    for (current, previous) in restored
        .arguments
        .iter_mut()
        .zip(&original_packet.arguments)
    {
        current.staging = previous.staging;
    }
    if restored != *original_packet {
        return Err("temporary producer changed immutable Source call authority".into());
    }
    for argument in &physical_packet.arguments {
        if argument.staging == hir::ArgumentStaging::Original {
            let index = argument.parameter_index;
            if !original_args
                .get(index)
                .zip(physical_args.get(index))
                .is_some_and(|(a, b)| breakpoint_regions::expressions_equal(a, b))
            {
                return Err("temporary producer changed an unstaged original operand".into());
            }
        }
    }
    Ok(())
}

fn unique_let(function: &Function, local: LocalId) -> Result<(Site, &Expression), String> {
    let mut found = None;
    for block in &function.blocks {
        for (statement, value) in block.statements.iter().enumerate() {
            if let StatementKind::Let {
                local: target,
                value,
            } = &value.kind
                && *target == local
                && found
                    .replace((
                        Site {
                            block: block.id,
                            statement,
                        },
                        value,
                    ))
                    .is_some()
            {
                return Err(
                    "temporary local view has duplicate owning or alias initialization".into(),
                );
            }
        }
    }
    found.ok_or_else(|| "temporary local view lost its complete initialization".into())
}

fn let_at(function: &Function, site: Site) -> Result<(LocalId, &Expression), String> {
    match function
        .blocks
        .get(site.block.index() as usize)
        .filter(|block| block.id == site.block)
        .and_then(|block| block.statements.get(site.statement))
        .map(|statement| &statement.kind)
    {
        Some(StatementKind::Let { local, value }) => Ok((*local, value)),
        _ => Err("temporary local view lost its exact initialization site".into()),
    }
}

fn runtime_occurrences(function: &Function, local: LocalId) -> usize {
    let mut count = 0;
    for mut block in function.blocks.clone() {
        sequences::prune::block_runtime_locals(
            &mut block,
            &mut |id| {
                count += usize::from(*id == local);
            },
            &mut |_| {},
        );
    }
    count
}

fn validate_row(function: &Function, row: &Row) -> Result<(), String> {
    let (owner_site, owner_value) = unique_let(function, row.owner)?;
    let (alias_site, alias_value) = unique_let(function, row.alias)?;
    let owner = function
        .local(row.owner)
        .ok_or("temporary owner header is absent")?;
    let alias = function
        .local(row.alias)
        .ok_or("temporary alias header is absent")?;
    if owner_site != row.initialize
        || alias_site != row.publish
        || owner_value.ty != owner.ty
        || alias_value.ty != alias.ty
        || row.publish.block != row.initialize.block
        || row.publish.statement != row.initialize.statement + 1
        || runtime_occurrences(function, row.owner) != 2
    {
        return Err(
            "temporary whole owner was duplicated, consumed, moved or published at another site"
                .into(),
        );
    }
    match &row.acquisition {
        Acquisition::Direct if matches!(owner_value.kind, E::Call { .. }) => {}
        Acquisition::Staged { local, call, ends } => {
            let (actual, value) = unique_let(function, *local)?;
            if actual != *call
                || !matches!(value.kind, E::Call { .. })
                || !matches!(owner_value.kind, E::Local(id) if id == *local)
                || runtime_occurrences(function, *local) != 2
                || call.block != row.initialize.block
                || call.statement >= row.initialize.statement
                || ends
                    != &((call.statement + 1)..row.initialize.statement)
                        .map(|statement| Site {
                            block: call.block,
                            statement,
                        })
                        .collect::<Vec<_>>()
                || ends.iter().any(|site| {
                    !matches!(
                        function.blocks[site.block.index() as usize].statements[site.statement]
                            .kind,
                        StatementKind::EndCallView { .. }
                    )
                })
            {
                return Err(
                    "temporary whole-owner staging no longer matches its exact acquisition chain"
                        .into(),
                );
            }
        }
        _ => return Err("temporary whole-owner initializer changed acquisition shape".into()),
    }
    let cfg = ControlFlowGraph::analyze(function).map_err(|errors| format!("{errors:?}"))?;
    let mut pending = cfg.successors(row.initialize.block).to_vec();
    let mut seen = BTreeSet::new();
    while let Some(block) = pending.pop() {
        if block == row.initialize.block {
            return Err(
                "pending temporary local view activation: backing is acquired on a cycle".into(),
            );
        }
        if seen.insert(block) {
            pending.extend_from_slice(cfg.successors(block));
        }
    }
    Ok(())
}

/// Called after definite initialization and ordinary liveness have succeeded.
/// No owner is inserted into initialization, alias facts or execution reads.
pub(super) fn pin_scope(
    function: &Function,
    types: &TypeInterner,
    cfg: &ControlFlowGraph,
    live_in: &mut [BTreeSet<usize>],
    live_out: &mut [BTreeSet<usize>],
    after: &mut [Vec<BTreeSet<usize>>],
) -> Result<(), String> {
    validate(function, types)?;
    let Some(witness) = &function.temporary_local_views else {
        return Ok(());
    };
    let owners = witness
        .rows
        .iter()
        .map(|row| row.owner.index() as usize)
        .collect::<BTreeSet<_>>();
    for id in cfg.reverse_postorder() {
        let index = id.index() as usize;
        live_in[index].extend(&owners);
        live_out[index].extend(&owners);
        for live in &mut after[index] {
            live.extend(&owners);
        }
    }
    Ok(())
}

fn mapped_local(map: &[Option<LocalId>], local: LocalId) -> Result<LocalId, String> {
    map.get(local.index() as usize)
        .copied()
        .flatten()
        .ok_or_else(|| {
            "temporary view compaction removed a live owner, alias or acquisition".into()
        })
}

fn transition(
    function: &mut Function,
    before: &Function,
    expected: &Function,
    rows: Vec<Row>,
) -> Result<(), String> {
    let witness = before
        .temporary_local_views
        .as_ref()
        .ok_or("temporary transition has no before-witness")?;
    witness.current(before)?;
    if function.temporary_local_views.as_ref() != Some(witness)
        || !Graph::new(expected).matches(function)
    {
        return Err("temporary preparation differs from exact canonical replay".into());
    }
    if witness.current.graph.matches(function) && rows == witness.rows {
        return witness.current(function);
    }
    let mut next = witness.clone();
    next.current = Arc::new(Current {
        graph: Graph::new(expected),
        rows: rows.clone(),
    });
    next.rows = rows;
    next.current(function)?;
    function.temporary_local_views = Some(next);
    Ok(())
}

pub(super) fn remap_blocks(
    function: &mut Function,
    before: &Function,
    map: &[Option<BlockId>],
) -> Result<(), String> {
    let Some(witness) = &before.temporary_local_views else {
        return Ok(());
    };
    witness.current(before)?;
    let cfg = ControlFlowGraph::analyze(before).map_err(|errors| format!("{errors:?}"))?;
    let mut canonical = vec![None; before.blocks.len()];
    let mut next = 0_u32;
    for block in &before.blocks {
        if cfg.reverse_postorder().contains(&block.id) {
            canonical[block.id.index() as usize] = Some(BlockId(next));
            next += 1;
        }
    }
    if map != canonical {
        return Err("temporary block map is not exact reachable dense compaction".into());
    }
    let mut expected = before.clone();
    expected
        .blocks
        .retain(|block| map[block.id.index() as usize].is_some());
    let remap = |id: &mut BlockId| {
        *id = map[id.index() as usize].expect("canonical reachable target has a map");
    };
    remap(&mut expected.entry);
    for block in &mut expected.blocks {
        remap(&mut block.id);
        sequences::prune::block_targets(&mut block.terminator.kind, &remap);
    }
    let mut rows = witness.rows.clone();
    for row in &mut rows {
        row.remap_blocks(map)?;
    }
    transition(function, before, &expected, rows)
}

fn canonical_local_map(function: &Function) -> Result<Vec<Option<LocalId>>, String> {
    let mut used = vec![false; function.locals.len()];
    let mut valid = true;
    for parameter in &function.params {
        if let Some(slot) = used.get_mut(parameter.local.index() as usize) {
            *slot = true;
        } else {
            valid = false;
        }
    }
    for mut block in function.blocks.clone() {
        sequences::prune::block_locals(
            &mut block,
            &mut |id| {
                if let Some(slot) = used.get_mut(id.index() as usize) {
                    *slot = true;
                } else {
                    valid = false;
                }
            },
            &mut |_| {},
        );
    }
    if !valid {
        return Err("temporary local compaction input has an invalid local reference".into());
    }
    loop {
        let mut changed = false;
        for local in &function.locals {
            if used[local.id.index() as usize]
                && let Some(source) = local.view_source
            {
                let slot = used
                    .get_mut(source.index() as usize)
                    .ok_or("temporary view has a foreign source local")?;
                changed |= !*slot;
                *slot = true;
            }
        }
        if !changed {
            break;
        }
    }
    let mut next = 0_u32;
    Ok(used
        .into_iter()
        .map(|used| {
            used.then(|| {
                let id = LocalId::new(next);
                next += 1;
                id
            })
        })
        .collect())
}

pub(super) fn remap_locals(
    function: &mut Function,
    before: &Function,
    map: &[Option<LocalId>],
) -> Result<(), String> {
    let Some(witness) = &before.temporary_local_views else {
        return Ok(());
    };
    witness.current(before)?;
    if map != canonical_local_map(before)? {
        return Err("temporary local map is not exact unused-local dense compaction".into());
    }
    let mut expected = before.clone();
    expected
        .locals
        .retain(|local| map[local.id.index() as usize].is_some());
    for parameter in &mut expected.params {
        parameter.local = mapped_local(map, parameter.local)?;
    }
    for local in &mut expected.locals {
        local.id = mapped_local(map, local.id)?;
        if let Some(source) = &mut local.view_source {
            *source = mapped_local(map, *source)?;
        }
    }
    let mut valid = true;
    for block in &mut expected.blocks {
        sequences::prune::block_locals(
            block,
            &mut |id| {
                if let Ok(mapped) = mapped_local(map, *id) {
                    *id = mapped;
                } else {
                    valid = false;
                }
            },
            &mut |floor| {
                if let Some(prefix) = map.get(..*floor as usize) {
                    *floor = u32::try_from(prefix.iter().filter(|id| id.is_some()).count())
                        .expect("canonical map width fits LocalId");
                }
            },
        );
    }
    if !valid {
        return Err("temporary local replay removed an active reference".into());
    }
    let mut rows = witness.rows.clone();
    for row in &mut rows {
        row.remap_locals(map)?;
    }
    transition(function, before, &expected, rows)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    const SOURCE09: &str = include_str!(
        "../../jett_driver/tests/native_conformance/local_view_aliases/09_temporary_root_boundary.jett"
    );
    const FACTORY: &str = r#"namespace app
struct Packet:
    data: bytes
    sibling: bytes
function make_packet(view seed: bytes) returns Packet:
    return Packet(data: clone seed, sibling: bytes.from_string("sibling"))
function exercise(view seed: bytes) returns bytes:
    bytes borrowed = view make_packet(view seed).data
    bytes forwarded = view borrowed
    print("after alias creation")
    return clone forwarded
"#;
    const COMPACT: &str = r#"namespace app
struct Packet:
    data: bytes
function make_packet() returns Packet:
    return Packet(data: bytes.from_string("AB"))
function main() returns nothing:
    function() returns int64 callback = function() returns int64:
        int64 child_local = 7
        return child_local
    bytes borrowed = view make_packet().data
"#;
    fn lower_source(source: &str) -> (Program, Arc<jett_typecheck::CheckedResourceProgram>) {
        let project = jett_common::FileId::new(0);
        let list = jett_common::FileId::new(jett_common::STDLIB_FILE_ID_START);
        let bytes = jett_common::FileId::new(jett_common::STDLIB_FILE_ID_START + 1);
        // Keep the original project text and FileId. Load the shipped facades
        // in dependency order, as the other MIR Source fixture helpers do.
        let mut parsed = jett_parser::parse(include_str!("../../../stdlib/list.jett"), list);
        let mut byte_module = jett_parser::parse(include_str!("../../../stdlib/bytes.jett"), bytes);
        let mut primary = jett_parser::parse(source, project);
        assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
        assert!(byte_module.errors.is_empty(), "{:?}", byte_module.errors);
        assert!(primary.errors.is_empty(), "{:?}", primary.errors);
        parsed.module.items.append(&mut byte_module.module.items);
        parsed.module.items.append(&mut primary.module.items);
        let checked = Arc::new(
            jett_typecheck::CheckedResourceProgram::prepare(
                parsed,
                HashMap::from([
                    (project, jett_common::SourceOrigin::Project),
                    (list, jett_common::SourceOrigin::Stdlib),
                    (bytes, jett_common::SourceOrigin::Stdlib),
                ]),
                &[],
                jett_typecheck::CheckOptions { release: false },
            )
            .expect("genuine checked Source program with shipped stdlib"),
        );
        let source = hir::lower_checked_ordinary_program(&checked).expect("genuine checked HIR");
        let types = &checked.checked().interner;
        let program = lower(&source, types).expect("genuine checked MIR");
        (program, checked)
    }
    fn named<'a>(program: &'a Program, name: &str) -> &'a Function {
        program
            .functions
            .iter()
            .find(|function| function.identity.declaration.name == name)
            .unwrap()
    }
    fn named_mut<'a>(program: &'a mut Program, name: &str) -> &'a mut Function {
        program
            .functions
            .iter_mut()
            .find(|function| function.identity.declaration.name == name)
            .unwrap()
    }
    fn first(function: &Function) -> Row {
        function.temporary_local_views.as_ref().unwrap().rows[0].clone()
    }
    fn prepare(program: &mut Program, types: &TypeInterner) {
        crate::prepare_native_sequences(program, types);
        crate::prepare_native_uninhabited_sums(program, types);
        crate::prepare_native_generated_functions(program, types);
    }

    /// A direct private replay fixture, deliberately not Source-generation
    /// evidence. Genuine Source09 supplies the immutable HIR receipt and exact
    /// original operations. Insert one empty disconnected block at index zero,
    /// relocate every original block/edge/row by one, and install the matching
    /// test-only temporary current seal. Independent Resource receipts stay
    /// unchanged and correctly reject this expanded graph. This supplies only
    /// a preimage for direct temporary-replay tests, never real-pruner or
    /// compiler-generated disconnected-Source acceptance evidence.
    ///
    /// Ordinary lower_if creates no dead join after two returning arms, so that
    /// tempting Source fixture cannot establish a removable disconnected block.
    fn synthetic_block_preimage(
        owner_reachable: bool,
    ) -> (
        Program,
        Arc<jett_typecheck::CheckedResourceProgram>,
        Function,
    ) {
        let (mut program, checked) = lower_source(SOURCE09);
        let types = &checked.checked().interner;
        let original = named(&program, "main").clone();
        validate(&original, types).unwrap();
        crate::validate_call_ownership(&program, types).unwrap();
        let mut expanded = original.clone();
        let shift = |id: &mut BlockId| {
            *id = BlockId(id.index().checked_add(1).expect("small Source09 test CFG"));
        };
        shift(&mut expanded.entry);
        for block in &mut expanded.blocks {
            shift(&mut block.id);
            sequences::prune::block_targets(&mut block.terminator.kind, &shift);
        }
        expanded.blocks.insert(
            0,
            BasicBlock {
                id: BlockId(0),
                statements: Vec::new(),
                terminator: Terminator {
                    kind: TerminatorKind::Unreachable,
                    span: original.span,
                },
            },
        );
        if !owner_reachable {
            expanded.entry = BlockId(0);
        }
        let map = original
            .blocks
            .iter()
            .map(|block| Some(BlockId(block.id.index().checked_add(1).unwrap())))
            .collect::<Vec<_>>();
        let mut witness = original.temporary_local_views.as_ref().unwrap().clone();
        for row in &mut witness.rows {
            row.remap_blocks(&map).unwrap();
        }
        witness.current = Arc::new(Current {
            graph: Graph::new(&expanded),
            rows: witness.rows.clone(),
        });
        expanded.temporary_local_views = Some(witness);
        validate(&expanded, types).unwrap();
        let cfg = ControlFlowGraph::analyze(&expanded).unwrap();
        assert_eq!(
            cfg.reverse_postorder()
                .contains(&first(&expanded).initialize.block),
            owner_reachable
        );
        assert_eq!(
            cfg.reverse_postorder().contains(&BlockId(0)),
            !owner_reachable
        );
        *named_mut(&mut program, "main") = expanded;
        let errors = crate::validate_call_ownership(&program, types).unwrap_err();
        assert!(errors.iter().any(|error| error.message.contains(
            "Resource ownership differs from its initially authenticated Source or constructor-emitted graph"
        )));
        (program, checked, original)
    }

    #[test]
    fn unchanged_unused_source09_has_one_owned_slot_pinned_without_alias_reads() {
        let (mut program, checked) = lower_source(SOURCE09);
        let types = &checked.checked().interner;
        let function = named(&program, "main");
        let row = first(function);
        let plan = move_values::MoveValuePlan::analyze(&program, function, types).unwrap();
        let owner = row.owner.index() as usize;
        assert!(plan.owned_locals.contains(&owner));
        assert!(!plan.owned_locals.contains(&(row.alias.index() as usize)));
        let cfg = ControlFlowGraph::analyze(function).unwrap();
        for block in cfg.reverse_postorder() {
            let index = block.index() as usize;
            assert!(plan.live_in[index].contains(&owner));
            assert!(plan.live_out[index].contains(&owner));
            assert!(
                plan.live_after_statement[index]
                    .iter()
                    .all(|set| set.contains(&owner))
            );
        }
        let before = program.clone();
        prepare(&mut program, types);
        assert_eq!(
            program, before,
            "identity preparation preserves witness identity"
        );
        crate::validate_call_ownership(&program, types).unwrap();
    }

    #[test]
    fn stable_view_factory_actual_has_one_direct_whole_result_acquisition() {
        let (program, checked) = lower_source(FACTORY);
        let types = &checked.checked().interner;
        let function = named(&program, "exercise");
        let row = first(function);
        assert_eq!(row.acquisition, Acquisition::Direct);
        let (owner, initializer) = let_at(function, row.initialize).unwrap();
        assert_eq!(owner, row.owner);
        let E::Call {
            function: target,
            args,
            evaluation_order,
            ownership: CallOwnership::Source(packet),
        } = &initializer.kind
        else {
            panic!("the whole producer result initializes its owning slot directly");
        };
        let factory = named(&program, "make_packet");
        assert_eq!(*target, factory.id);
        assert_eq!(factory.params[0].mode, ParamMode::View);
        assert_eq!(evaluation_order, &[0]);
        let E::View(endpoint) = &args[0].kind else {
            panic!("written stable view actual");
        };
        let E::Local(seed) = endpoint.kind else {
            panic!("original view parameter");
        };
        assert_eq!(
            function.parameter_for_local(seed).unwrap().mode,
            ParamMode::View
        );
        assert_eq!(
            packet.arguments[0].syntax,
            jett_typecheck::CheckedCallerSyntax::WrittenView
        );
        assert_eq!(
            packet.arguments[0].effect,
            jett_typecheck::CheckedCallerEffect::RetainBorrow
        );
        assert_eq!(
            packet.arguments[0].physical_access,
            jett_typecheck::CheckedCalleeAccess::View
        );
        assert_eq!(packet.arguments[0].staging, hir::ArgumentStaging::Original);
        // No eager operand or ownership transfer requires a call-loan scope:
        // the checked view parameter remains an original borrowed operand.
        let mut calls = 0;
        for block in &function.blocks {
            resource_ownership::walk::mir_block(block, &mut |value| {
                calls += usize::from(matches!(
                    value.kind,
                    E::Call { function, .. } if function == factory.id
                ));
            });
        }
        assert_eq!(calls, 1);
        assert_eq!(runtime_occurrences(function, row.owner), 2);
        assert_eq!(row.publish.block, row.initialize.block);
        assert_eq!(row.publish.statement, row.initialize.statement + 1);
        crate::validate_call_ownership(&program, types).unwrap();
        let plan = move_values::MoveValuePlan::analyze(&program, function, types).unwrap();
        let return_block = function
            .blocks
            .iter()
            .find(|block| {
                matches!(
                    block.terminator.kind,
                    TerminatorKind::Return(Some(Expression {
                        kind: E::Clone(_),
                        ..
                    }))
                )
            })
            .unwrap();
        assert!(
            plan.live_out[return_block.id.index() as usize].contains(&(row.owner.index() as usize))
        );
        // The unviewed sibling belongs to Packet's whole owning slot. No leaf
        // owner, runtime retain, emitted cleanup or measurement is invented here.
        assert!(matches!(
            types.resolve(function.local(row.owner).unwrap().ty),
            Type::Struct(_)
        ));
    }

    #[test]
    fn temporary_factory_actual_with_later_handle_stages_call_before_lexical_owner() {
        // A written temporary alone need not force MIR staging. This genuine
        // later handled actual requires lexical evaluation before the handler,
        // preserving both the inner call loan and the outer whole Packet owner.
        let source = FACTORY
            .replace(
                "function make_packet(view seed: bytes) returns Packet:",
                "function make_packet(view seed: bytes, marker: int64) returns Packet:",
            )
            .replace(
                "function exercise(view seed: bytes) returns bytes:",
                "function exercise(incoming: optional[int64]) returns bytes:",
            )
            .replace(
                "view make_packet(view seed).data",
                "view make_packet(view bytes.from_string(\"AB\"), incoming handle:\n        default 3\n    ).data",
            );
        let (program, checked) = lower_source(&source);
        let types = &checked.checked().interner;
        let function = named(&program, "exercise");
        let row = first(function);
        let Acquisition::Staged {
            local,
            call,
            ref ends,
        } = row.acquisition
        else {
            panic!("the handled later actual requires the complete call before its loan ends");
        };
        assert_eq!(runtime_occurrences(function, local), 2);
        assert_eq!(runtime_occurrences(function, row.owner), 2);
        assert_eq!(call.block, row.initialize.block);
        let (actual_call, value) = unique_let(function, local).unwrap();
        assert_eq!(actual_call, call);
        let E::Call {
            function: target,
            evaluation_order,
            ownership: CallOwnership::Source(packet),
            ..
        } = &value.kind
        else {
            panic!("genuine named factory acquisition");
        };
        assert_eq!(*target, named(&program, "make_packet").id);
        assert_eq!(evaluation_order, &[0, 1]);
        let hir::ArgumentStaging::Borrowed { loan } = packet.arguments[0].staging else {
            panic!("the exact first temporary actual retains its checked loan");
        };
        assert_eq!(
            packet.arguments[0].syntax,
            jett_typecheck::CheckedCallerSyntax::WrittenView
        );
        assert_eq!(
            packet.arguments[0].effect,
            jett_typecheck::CheckedCallerEffect::RetainBorrow
        );
        let actual_owner = function.local(loan).unwrap().view_source.unwrap();
        let (actual_site, actual_value) = unique_let(function, actual_owner).unwrap();
        assert!(matches!(actual_value.kind, E::Call { function, .. }
            if function == named(&program, "from_string").id));
        let begins = function
            .blocks
            .iter()
            .flat_map(|block| {
                block.statements.iter().enumerate().filter_map(move |(statement, value)| {
                matches!(value.kind, StatementKind::BeginCallView { local, .. } if local == loan)
                    .then_some(Site { block: block.id, statement })
            })
            })
            .collect::<Vec<_>>();
        let [begin] = begins.as_slice() else {
            panic!("one loan for the first actual");
        };
        assert_eq!(actual_site.block, begin.block);
        assert!(actual_site.statement < begin.statement);
        let later_sum = function.blocks[begin.block.index() as usize]
            .statements
            .iter()
            .position(|statement| matches!(statement.kind, StatementKind::SumTag { .. }))
            .unwrap();
        assert!(
            begin.statement < later_sum,
            "the first temporary actual is acquired before handling the second actual"
        );
        let mut factory_calls = 0;
        let mut actual_calls = 0;
        for block in &function.blocks {
            resource_ownership::walk::mir_block(block, &mut |value| {
                if let E::Call {
                    function: target, ..
                } = value.kind
                {
                    factory_calls += usize::from(target == named(&program, "make_packet").id);
                    actual_calls += usize::from(target == named(&program, "from_string").id);
                }
            });
        }
        assert_eq!(factory_calls, 1);
        assert_eq!(actual_calls, 1);
        assert_eq!(
            function.local(local).unwrap().ty,
            named(&program, "make_packet").return_type
        );
        assert!(function.local(local).unwrap().view_source.is_none());
        assert_eq!(ends.len(), 1);
        assert!(ends.iter().all(|site| matches!(
            function.blocks[site.block.index() as usize].statements[site.statement].kind,
            StatementKind::EndCallView { local } if local == loan
        )));
        assert!(call.statement < ends[0].statement);
        assert!(ends[0].statement < row.initialize.statement);
        assert_eq!(row.publish.statement, row.initialize.statement + 1);
        let (_, owner_value) = let_at(function, row.initialize).unwrap();
        assert!(matches!(owner_value.kind, E::Local(id) if id == local));
        crate::validate_call_ownership(&program, types).unwrap();
        let plan = move_values::MoveValuePlan::analyze(&program, function, types).unwrap();
        assert!(plan.owned_locals.contains(&(row.owner.index() as usize)));
        assert!(!plan.owned_locals.contains(&(row.alias.index() as usize)));
        assert!(!plan.owned_locals.contains(&(loan.index() as usize)));
        let returned = function
            .blocks
            .iter()
            .find(|block| {
                matches!(
                    block.terminator.kind,
                    TerminatorKind::Return(Some(Expression {
                        kind: E::Clone(_),
                        ..
                    }))
                )
            })
            .unwrap();
        assert!(
            plan.live_out[returned.id.index() as usize].contains(&(row.owner.index() as usize))
        );
    }

    #[test]
    fn genuine_child_local_compaction_remaps_owner_alias_and_all_source_metadata() {
        let (mut program, checked) = lower_source(COMPACT);
        let types = &checked.checked().interner;
        let before = named(&program, "main").clone();
        let old = first(&before);
        let original = before
            .temporary_local_views
            .as_ref()
            .unwrap()
            .original
            .clone();
        crate::prepare_native_generated_functions(&mut program, types);
        let after = named(&program, "main");
        let new = first(after);
        assert!(after.locals.len() < before.locals.len());
        assert!(
            new.owner.index() < old.owner.index(),
            "requires a genuine nonidentity map before the owner"
        );
        assert!(new.alias.index() < old.alias.index());
        assert!(Arc::ptr_eq(
            &original,
            &after.temporary_local_views.as_ref().unwrap().original
        ));
        assert_eq!(
            original.source.locals,
            before
                .temporary_local_views
                .as_ref()
                .unwrap()
                .original
                .source
                .locals
        );
        crate::validate_call_ownership(&program, types).unwrap();
        move_values::MoveValuePlan::analyze(&program, after, types).unwrap();
    }

    #[test]
    fn identity_map_cannot_reseal_a_function_that_requires_nonidentity_compaction() {
        let (program, _) = lower_source(COMPACT);
        let before = named(&program, "main");
        let map = before
            .locals
            .iter()
            .map(|local| Some(local.id))
            .collect::<Vec<_>>();
        assert_ne!(map, canonical_local_map(before).unwrap());
        let mut unchanged = before.clone();
        assert!(remap_locals(&mut unchanged, before, &map).is_err());
        assert_eq!(&unchanged, before);
    }

    #[test]
    fn isolated_private_block_replay_restores_the_original_graph_and_independent_receipts() {
        let (mut program, checked, original) = synthetic_block_preimage(true);
        let types = &checked.checked().interner;
        let before = named(&program, "main").clone();
        let old = first(&before);
        let original_receipt = before
            .temporary_local_views
            .as_ref()
            .unwrap()
            .original
            .clone();
        assert_eq!(before.blocks.len(), original.blocks.len() + 1);
        assert!(before.entry.index() > original.entry.index());
        // The artificial preimage has no matching Resource Source graph. The
        // public pruner must refuse it and preserve every independent receipt.
        let mut refused = before.clone();
        assert!(!sequences::prune::unreachable(&mut refused));
        assert_eq!(refused, before);

        // Directly exercise only temporary replay using the known exact dense
        // map of this synthetic one-gap preimage. Restoring the original graph
        // also restores compatibility with its unchanged genuine Resource seal.
        let map = std::iter::once(None)
            .chain(original.blocks.iter().map(|block| Some(block.id)))
            .collect::<Vec<_>>();
        let mut candidate = before.clone();
        candidate
            .blocks
            .retain(|block| map[block.id.index() as usize].is_some());
        let relocate = |id: &mut BlockId| {
            *id = map[id.index() as usize].unwrap();
        };
        relocate(&mut candidate.entry);
        for block in &mut candidate.blocks {
            relocate(&mut block.id);
            sequences::prune::block_targets(&mut block.terminator.kind, &relocate);
        }
        remap_blocks(&mut candidate, &before, &map).unwrap();
        validate(&candidate, types).unwrap();
        assert_eq!(candidate.resource_lowering, original.resource_lowering);
        *named_mut(&mut program, "main") = candidate;
        let compacted = named(&program, "main");
        let new = first(compacted);
        assert_eq!(compacted.blocks.len() + 1, before.blocks.len());
        assert_eq!(
            new.initialize.block.index() + 1,
            old.initialize.block.index()
        );
        assert_eq!(new.publish.block.index() + 1, old.publish.block.index());
        assert!(
            Graph::new(&original).matches(compacted),
            "all original operations and headers survive exact replay"
        );
        assert!(Arc::ptr_eq(
            &original_receipt,
            &compacted.temporary_local_views.as_ref().unwrap().original
        ));
        assert_eq!(
            compacted.temporary_local_views.as_ref().unwrap().rows,
            original.temporary_local_views.as_ref().unwrap().rows
        );
        crate::validate_call_ownership(&program, types).unwrap();
        move_values::MoveValuePlan::analyze(&program, compacted, types).unwrap();
    }

    #[test]
    fn private_block_replay_refuses_altered_dense_or_reachability_maps_without_mutation() {
        let (program, _, original) = synthetic_block_preimage(true);
        let before = named(&program, "main");
        // The fixture is the exact inverse of a one-gap canonical compaction.
        // This is a direct temporary-replay control, not a valid whole-program
        // preimage for the public pruner or a Source-generated dead block.
        let canonical = std::iter::once(None)
            .chain(original.blocks.iter().map(|block| Some(block.id)))
            .collect::<Vec<_>>();
        let mut wrong_target = canonical.clone();
        wrong_target[before.entry.index() as usize] = Some(before.entry);
        let mut keep_disconnected = canonical.clone();
        keep_disconnected[0] = Some(BlockId(0));
        for altered in [wrong_target, keep_disconnected] {
            assert_ne!(altered, canonical);
            let mut unchanged = before.clone();
            assert!(remap_blocks(&mut unchanged, before, &altered).is_err());
            assert_eq!(
                &unchanged, before,
                "refused maps cannot change the graph or renew its seal"
            );
        }
    }

    #[test]
    fn public_marked_call_transplant_cannot_lose_the_private_function_witness() {
        let source = SOURCE09.replace(
            "function main()",
            concat!(
                "function inspect(view data: bytes) returns nothing:\n    return nothing\n",
                "function empty() returns nothing:\n    return nothing\nfunction main()",
            ),
        ) + "    inspect(view borrowed)\n";
        let (original, checked) = lower_source(&source);
        let types = &checked.checked().interner;
        for target_name in ["make_packet", "inspect"] {
            let target = named(&original, target_name).id;
            let donor = named(&original, "main");
            let mut marked = Vec::new();
            for block in &donor.blocks {
                resource_ownership::walk::mir_block(block, &mut |value| {
                    if matches!(&value.kind, E::Call { function, ownership, .. }
                        if *function == target && ownership.has_temporary_local_receipts())
                    {
                        marked.push(value.clone());
                    }
                });
            }
            let [call] = marked.as_slice() else {
                panic!("one genuine marked {target_name} invocation");
            };
            // make_packet carries the producer marker; inspect carries the
            // argument's temporary-origin receipt. Neither can mint a witness.
            for disconnected in [false, true] {
                let mut program = original.clone();
                let mut transplanted = named(&program, "empty").clone();
                assert!(!has_records(&transplanted));
                // Only public fields are copied into a genuine unmarked
                // Function. External callers cannot set its private witness.
                transplanted.locals = donor.locals.clone();
                let statement = Statement {
                    kind: StatementKind::Evaluate(call.clone()),
                    span: call.span,
                };
                if disconnected {
                    let block = BlockId(u32::try_from(transplanted.blocks.len()).unwrap());
                    transplanted.blocks.push(BasicBlock {
                        id: block,
                        statements: vec![statement],
                        terminator: Terminator {
                            kind: TerminatorKind::Return(None),
                            span: call.span,
                        },
                    });
                    assert!(
                        !ControlFlowGraph::analyze(&transplanted)
                            .unwrap()
                            .reverse_postorder()
                            .contains(&block)
                    );
                } else {
                    transplanted.blocks[transplanted.entry.index() as usize]
                        .statements
                        .push(statement);
                }
                assert!(has_receipts(&transplanted));
                assert!(
                    validate_current(&transplanted)
                        .unwrap_err()
                        .contains("no original private MIR witness")
                );
                let index = transplanted.id.index() as usize;
                program.functions[index] = transplanted;
                let unchanged = program.clone();
                assert!(crate::validate_call_ownership(&program, types).is_err());
                prepare(&mut program, types);
                assert_eq!(program, unchanged);
            }
        }
    }

    #[test]
    fn corrupt_unused_records_are_refused_by_all_preparation_and_standalone_planners() {
        let (original, checked) = lower_source(SOURCE09);
        let types = &checked.checked().interner;
        for mutation in 0..5 {
            let mut program = original.clone();
            let function = named_mut(&mut program, "main");
            let row = first(function);
            match mutation {
                0 => {
                    function.blocks[row.initialize.block.index() as usize]
                        .statements
                        .remove(row.initialize.statement);
                }
                1 => {
                    let value = function.blocks[row.initialize.block.index() as usize].statements
                        [row.initialize.statement]
                        .clone();
                    function.blocks[row.initialize.block.index() as usize]
                        .statements
                        .insert(row.initialize.statement, value);
                }
                2 => {
                    function.locals[row.owner.index() as usize].ty = TypeInterner::STRING;
                }
                3 => {
                    function.blocks[row.publish.block.index() as usize]
                        .terminator
                        .kind = TerminatorKind::Unreachable;
                }
                4 => {
                    function.temporary_local_views.as_mut().unwrap().rows[0].owner = row.alias;
                }
                _ => unreachable!(),
            }
            let unchanged = program.clone();
            assert!(crate::validate_call_ownership(&program, types).is_err());
            assert!(copy_values::CopyValuePlan::analyze(named(&program, "main"), types).is_err());
            assert!(
                move_values::MoveValuePlan::analyze(&program, named(&program, "main"), types)
                    .is_err()
            );
            prepare(&mut program, types);
            assert_eq!(
                program, unchanged,
                "preparation must refuse before hiding mutation {mutation}"
            );
        }
    }

    #[test]
    fn isolated_private_disconnected_initialization_corruption_is_not_pruned_away() {
        let (mut program, checked, _) = synthetic_block_preimage(false);
        let types = &checked.checked().interner;
        let function = named_mut(&mut program, "main");
        validate(function, types).unwrap();
        let row = first(function);
        let cfg = ControlFlowGraph::analyze(function).unwrap();
        assert!(!cfg.reverse_postorder().contains(&row.initialize.block));
        function.blocks[row.initialize.block.index() as usize]
            .statements
            .remove(row.initialize.statement);
        assert!(
            validate_current(function).is_err(),
            "the temporary witness itself detects the missing initialization"
        );
        let unchanged = program.clone();
        assert!(crate::validate_call_ownership(&program, types).is_err());
        prepare(&mut program, types);
        assert_eq!(program, unchanged);
        let function = named_mut(&mut program, "main");
        assert!(!sequences::prune::unreachable(function));
        assert!(!sequences::prune::unused_locals(function));
        assert_eq!(program, unchanged);
    }
}
