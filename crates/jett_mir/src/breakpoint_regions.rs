//! Constructor-owned lexical context for extracted breakpoint conditions.
//! These snapshots are metadata, never execution roots or caller authority.
use super::*;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(super) struct Site {
    block: BlockId,
    index: usize,
}

#[derive(Debug, Clone, PartialEq)]
struct ArchivedCondition(hir::Statement);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct Edge {
    source: BlockId,
    arm: usize,
    target: BlockId,
}

#[derive(Debug, Clone, PartialEq)]
enum Endpoint {
    Live(Site),
    // Minted only by the exact typed Never-side selection below. The old
    // endpoint is archival after compaction; active selected edges are remapped.
    CanonicallyAborted {
        selections: Vec<SelectedEdge>,
        loops: Vec<SelectedLoop>,
    },
}

#[derive(Debug, Clone, PartialEq)]
struct SelectedEdge {
    block: BlockId,
    source: LocalId,
    tag: LocalId,
    source_type: TypeId,
    selected: BlockId,
    span: Span,
}

#[derive(Debug, Clone, PartialEq)]
struct SelectedLoop {
    block: BlockId,
    preheader: BlockId,
    source: SequenceSource,
    source_type: TypeId,
    length: LocalId,
    selected: BlockId,
    span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub(super) struct BreakpointRegion {
    id: usize,
    parent: Option<usize>,
    owner: FunctionId,
    identity: FunctionIdentity,
    debug_kind: hir::FunctionDebugKind,
    entry: BlockId,
    origin: ArchivedCondition,
    start: Site,
    statements: BTreeMap<Site, Statement>,
    terminators: BTreeMap<BlockId, Terminator>,
    blocks: BTreeSet<BlockId>,
    edges: Vec<Edge>,
    endpoint: Endpoint,
    selections: Vec<SelectedEdge>,
    loops: Vec<SelectedLoop>,
    abort_allowed: bool,
}

#[derive(Clone, Default)]
pub(super) struct Capture {
    drafts: Vec<Draft>,
    stack: Vec<usize>,
}

#[derive(Clone)]
struct Draft {
    parent: Option<usize>,
    origin: ArchivedCondition,
    start: Site,
    statements: BTreeMap<Site, Statement>,
    terminators: BTreeMap<BlockId, Terminator>,
    blocks: BTreeSet<BlockId>,
    endpoint: Option<Site>,
}

impl Capture {
    pub(super) fn begin(&mut self, original: &hir::Statement, block: BlockId, index: usize) {
        let id = self.drafts.len();
        self.drafts.push(Draft {
            parent: self.stack.last().copied(),
            origin: ArchivedCondition(original.clone()),
            start: Site { block, index },
            statements: BTreeMap::new(),
            terminators: BTreeMap::new(),
            blocks: BTreeSet::from([block]),
            endpoint: None,
        });
        self.stack.push(id);
    }

    pub(super) fn statement(&mut self, block: BlockId, index: usize, value: &Statement) {
        if let Some(id) = self.stack.last().copied() {
            let draft = &mut self.drafts[id];
            draft.blocks.insert(block);
            draft
                .statements
                .insert(Site { block, index }, value.clone());
        }
    }

    pub(super) fn terminator(&mut self, block: BlockId, value: &Terminator) {
        if let Some(id) = self.stack.last().copied() {
            let draft = &mut self.drafts[id];
            draft.blocks.insert(block);
            draft.terminators.insert(block, value.clone());
        }
    }

    pub(super) fn block(&mut self, block: BlockId) {
        if let Some(id) = self.stack.last().copied() {
            self.drafts[id].blocks.insert(block);
        }
    }

    pub(super) fn end(&mut self, block: BlockId, index: usize) {
        if let Some(id) = self.stack.pop() {
            self.drafts[id].endpoint = Some(Site { block, index });
        }
    }

    pub(super) fn seal(self, function: &Function) -> Result<Vec<BreakpointRegion>, String> {
        if !self.stack.is_empty() {
            return Err("breakpoint constructor has an unfinished condition capture".into());
        }
        let mut records = Vec::new();
        for (id, draft) in self.drafts.into_iter().enumerate() {
            let endpoint = draft
                .endpoint
                .ok_or("breakpoint constructor lacks an exact final endpoint")?;
            records.push(BreakpointRegion {
                id,
                parent: draft.parent,
                owner: function.id,
                identity: function.identity.clone(),
                debug_kind: function.debug_kind.clone(),
                entry: function.entry,
                origin: draft.origin,
                start: draft.start,
                statements: draft.statements,
                terminators: draft.terminators,
                blocks: draft.blocks,
                edges: Vec::new(),
                endpoint: Endpoint::Live(endpoint),
                selections: Vec::new(),
                loops: Vec::new(),
                abort_allowed: false,
            });
        }
        for id in (0..records.len()).rev() {
            if let Some(parent) = records[id].parent {
                let child_blocks = records[id].blocks.clone();
                records[parent].blocks.extend(child_blocks);
            }
        }
        let all = edges(&function.blocks);
        for record in &mut records {
            record.edges = touching(&all, &record.blocks);
        }
        Ok(records)
    }
}

pub(super) struct VerifiedSites {
    statements: BTreeSet<Site>,
    terminators: BTreeSet<BlockId>,
}

impl VerifiedSites {
    pub(super) fn statement(&self, block: BlockId, index: usize) -> bool {
        self.statements.contains(&Site { block, index })
    }
    pub(super) fn terminator(&self, block: BlockId) -> bool {
        self.terminators.contains(&block)
    }
}

pub(super) fn validate(function: &Function, types: &TypeInterner) -> Result<VerifiedSites, String> {
    let mut verified = VerifiedSites {
        statements: BTreeSet::new(),
        terminators: BTreeSet::new(),
    };
    if function.breakpoint_regions.is_empty() {
        if function.blocks.iter().any(|block| {
            block.statements.iter().any(|statement| {
                matches!(
                    statement.kind,
                    StatementKind::Breakpoint {
                        condition: Some(_),
                        ..
                    }
                )
            })
        }) {
            return Err("breakpoint condition has no exact constructor-owned endpoint".into());
        }
        return Ok(verified);
    }
    let mut ids = BTreeSet::new();
    let mut endpoints = BTreeSet::new();
    let all = edges(&function.blocks);
    for record in &function.breakpoint_regions {
        if !ids.insert(record.id)
            || record.owner != function.id
            || record.identity != function.identity
            || record.debug_kind != function.debug_kind
            || record.entry != function.entry
        {
            return Err("breakpoint extraction has duplicate or foreign function ownership".into());
        }
        if let Some(parent) = record.parent
            && (parent >= record.id || !function.breakpoint_regions.iter().any(|r| r.id == parent))
        {
            return Err("breakpoint extraction has an orphan lexical parent".into());
        }
        let hir::StatementKind::Breakpoint {
            condition: Some(condition),
            ..
        } = &record.origin.0.kind
        else {
            return Err("breakpoint extraction lacks its archived checked condition".into());
        };
        if condition.ty != TypeInterner::BOOL
            || record.blocks.is_empty()
            || !record.blocks.contains(&record.start.block)
            || function
                .blocks
                .get(record.start.block.index() as usize)
                .is_none_or(|block| record.start.index > block.statements.len())
            || touching(&all, &record.blocks) != record.edges
        {
            return Err(
                "breakpoint extraction changes its condition or exact CFG boundaries".into(),
            );
        }
        for (&site, expected) in &record.statements {
            let actual = function
                .blocks
                .get(site.block.index() as usize)
                .filter(|block| block.id == site.block)
                .and_then(|block| block.statements.get(site.index));
            if !record.blocks.contains(&site.block)
                || !verified.statements.insert(site)
                || actual.is_none_or(|actual| !statement_equal(actual, expected))
            {
                return Err(
                    "breakpoint extraction changes or duplicates an exact statement occurrence"
                        .into(),
                );
            }
        }
        for (&block, expected) in &record.terminators {
            let actual = function
                .blocks
                .get(block.index() as usize)
                .filter(|value| value.id == block);
            if !record.blocks.contains(&block)
                || !verified.terminators.insert(block)
                || actual.is_none_or(|value| !terminator_equal(&value.terminator, expected))
            {
                return Err(
                    "breakpoint extraction changes or duplicates an exact terminator occurrence"
                        .into(),
                );
            }
        }
        match &record.endpoint {
            Endpoint::Live(site) => {
                if !endpoints.insert(*site) {
                    return Err("breakpoint extraction duplicates its endpoint".into());
                }
                if !matches!(record.statements.get(site).map(|s| &s.kind), Some(StatementKind::Breakpoint { condition: Some(value), .. }) if value.ty == TypeInterner::BOOL)
                {
                    return Err("breakpoint extraction loses its exact final endpoint".into());
                }
            }
            Endpoint::CanonicallyAborted { selections, loops } => {
                if !record.abort_allowed
                    || (selections.is_empty() && loops.is_empty())
                    || selections != &record.selections
                    || loops != &record.loops
                {
                    return Err(
                        "breakpoint extraction has no canonical aborted-edge evidence".into(),
                    );
                }
            }
        }
        for selected in &record.selections {
            validate_selection(function, selected, types)?;
        }
        for selected in &record.loops {
            validate_loop_selection(function, selected, types)?;
        }
    }
    for block in &function.blocks {
        for (index, statement) in block.statements.iter().enumerate() {
            if matches!(
                statement.kind,
                StatementKind::Breakpoint {
                    condition: Some(_),
                    ..
                }
            ) && !endpoints.contains(&Site {
                block: block.id,
                index,
            }) {
                return Err("breakpoint condition has no exact constructor-owned endpoint".into());
            }
        }
    }
    Ok(verified)
}

fn edges(blocks: &[BasicBlock]) -> Vec<Edge> {
    let mut result = Vec::new();
    for block in blocks {
        let mut targets = Vec::new();
        match &block.terminator.kind {
            TerminatorKind::Goto(target) => targets.push(*target),
            TerminatorKind::Branch {
                then_block,
                else_block,
                ..
            } => targets.extend([*then_block, *else_block]),
            TerminatorKind::Switch {
                variants,
                otherwise,
                ..
            } => {
                targets.extend(variants.iter().map(|(_, target, _)| *target));
                targets.extend(*otherwise);
            }
            TerminatorKind::ForEach { body, exit, .. } => targets.extend([*body, *exit]),
            TerminatorKind::ReflectedTypeDispatch {
                arms, otherwise, ..
            } => {
                targets.extend(arms.iter().map(|arm| arm.target));
                targets.push(*otherwise);
            }
            TerminatorKind::Return(_)
            | TerminatorKind::Respond(_)
            | TerminatorKind::Unreachable => {}
        }
        result.extend(targets.into_iter().enumerate().map(|(arm, target)| Edge {
            source: block.id,
            arm,
            target,
        }));
    }
    result
}

fn touching(all: &[Edge], blocks: &BTreeSet<BlockId>) -> Vec<Edge> {
    all.iter()
        .copied()
        .filter(|edge| blocks.contains(&edge.source) || blocks.contains(&edge.target))
        .collect()
}

// Only the canonical sequence pass constructs this finite edit description.
// Before is already authenticated; additions come from that exact ForEach,
// never from current call context, source spans or a post-mutation search.
#[derive(Clone)]
pub(super) struct SequenceEdit {
    pub header: BlockId,
    pub before: Terminator,
    pub after: Terminator,
    pub append: Vec<(BlockId, Vec<Statement>)>,
    pub prefix: Option<(BlockId, Vec<Statement>)>,
    pub redirects: Vec<(BlockId, Terminator, Terminator)>,
    pub blocks: Vec<BasicBlock>,
}

pub(super) fn sequence_transition(
    function: &mut Function,
    before: &Function,
    edit: SequenceEdit,
    types: &TypeInterner,
) -> Result<(), String> {
    validate(before, types)?;
    let Some(header) = before
        .blocks
        .get(edit.header.index() as usize)
        .filter(|block| block.id == edit.header)
    else {
        return Err("breakpoint sequence transition has no exact header".into());
    };
    let existing = |id: BlockId| {
        before
            .blocks
            .get(id.index() as usize)
            .is_some_and(|block| block.id == id)
    };
    if edit.append.iter().any(|(block, _)| !existing(*block))
        || edit
            .prefix
            .as_ref()
            .is_some_and(|(block, _)| !existing(*block))
        || edit.redirects.iter().any(|(block, _, _)| !existing(*block))
        || edit
            .blocks
            .iter()
            .enumerate()
            .any(|(index, block)| block.id.index() as usize != before.blocks.len() + index)
    {
        return Err("breakpoint sequence edit has a foreign or nondense occurrence".into());
    }
    if !terminator_equal(&header.terminator, &edit.before)
        || !matches!(edit.before.kind, TerminatorKind::ForEach { .. })
    {
        return Err("breakpoint sequence transition differs from its original ForEach".into());
    }
    let mut expected = before.clone();
    expected.blocks[edit.header.index() as usize].terminator = edit.after.clone();
    for (block, values) in &edit.append {
        expected.blocks[block.index() as usize]
            .statements
            .extend(values.clone());
    }
    if let Some((block, prefix)) = &edit.prefix {
        let statements = &mut expected.blocks[block.index() as usize].statements;
        let mut values = prefix.clone();
        values.append(statements);
        *statements = values;
    }
    for (block, old, new) in &edit.redirects {
        if !terminator_equal(&expected.blocks[block.index() as usize].terminator, old) {
            return Err("breakpoint sequence redirect has a foreign before-edge".into());
        }
        expected.blocks[block.index() as usize].terminator = new.clone();
    }
    expected.blocks.extend(edit.blocks.clone());
    if !blocks_equal(&expected.blocks, &function.blocks) {
        return Err("breakpoint sequence transition has changes outside its canonical edit".into());
    }
    let owners = before
        .breakpoint_regions
        .iter()
        .filter(|record| record.terminators.contains_key(&edit.header))
        .map(|record| record.id)
        .collect::<BTreeSet<_>>();
    let mut records = before.breakpoint_regions.clone();
    for record in &mut records {
        if let Some((block, prefix)) = &edit.prefix {
            shift(record, *block, 0, prefix.len());
        }
        if record.terminators.contains_key(&edit.header) {
            record.terminators.insert(edit.header, edit.after.clone());
        }
        for (block, _, new) in &edit.redirects {
            if record.terminators.contains_key(block) {
                record.terminators.insert(*block, new.clone());
            }
        }
        if owners.contains(&record.id) {
            if let (
                TerminatorKind::ForEach { iterable, exit, .. },
                TerminatorKind::Goto(selected),
            ) = (&edit.before.kind, &edit.after.kind)
            {
                let Some((preheader, source, length)) =
                    edit.append.iter().find_map(|(block, values)| {
                        values.iter().find_map(|statement| {
                            if let StatementKind::SequenceLength { source, target } =
                                &statement.kind
                            {
                                Some((*block, source.clone(), *target))
                            } else {
                                None
                            }
                        })
                    })
                else {
                    return Err(
                        "breakpoint empty sequence lacks canonical length validation".into(),
                    );
                };
                if selected != exit || !empty_sequence(types, iterable.ty) {
                    return Err("breakpoint sequence selected an inhabited body".into());
                }
                record.loops.push(SelectedLoop {
                    block: edit.header,
                    preheader,
                    source,
                    source_type: iterable.ty,
                    length,
                    selected: *selected,
                    span: edit.after.span,
                });
            }
            for (block, values) in &edit.append {
                let start = before.blocks[block.index() as usize].statements.len();
                for (index, value) in values.iter().enumerate() {
                    record.statements.insert(
                        Site {
                            block: *block,
                            index: start + index,
                        },
                        value.clone(),
                    );
                }
                record.blocks.insert(*block);
            }
            if let Some((block, values)) = &edit.prefix {
                for (index, value) in values.iter().enumerate() {
                    record.statements.insert(
                        Site {
                            block: *block,
                            index,
                        },
                        value.clone(),
                    );
                }
                record.blocks.insert(*block);
            }
            for block in &edit.blocks {
                record.blocks.insert(block.id);
                for (index, value) in block.statements.iter().enumerate() {
                    record.statements.insert(
                        Site {
                            block: block.id,
                            index,
                        },
                        value.clone(),
                    );
                }
                record
                    .terminators
                    .insert(block.id, block.terminator.clone());
            }
        }
    }
    // Ancestors include child CFG boundaries without claiming child sites.
    for id in (0..records.len()).rev() {
        if let Some(parent) = records[id].parent {
            let blocks = records[id].blocks.clone();
            let parent_record = records
                .iter_mut()
                .find(|record| record.id == parent)
                .ok_or("breakpoint preparation loses a parent")?;
            parent_record.blocks.extend(blocks);
        }
    }
    for record in &mut records {
        record.edges = touching(&edges(&expected.blocks), &record.blocks);
        if let Endpoint::Live(endpoint) = &record.endpoint {
            record.abort_allowed |= reachable(&before.blocks, before.entry, endpoint.block)
                && !reachable(&expected.blocks, expected.entry, endpoint.block)
                && (!record.selections.is_empty() || !record.loops.is_empty());
        }
    }
    function.breakpoint_regions = records;
    Ok(())
}

fn shift(record: &mut BreakpointRegion, block: BlockId, start: usize, count: usize) {
    record.statements = std::mem::take(&mut record.statements)
        .into_iter()
        .map(|(mut site, statement)| {
            if site.block == block && site.index >= start {
                site.index += count;
            }
            (site, statement)
        })
        .collect();
    if record.start.block == block && record.start.index >= start {
        record.start.index += count;
    }
    if let Endpoint::Live(site) = &mut record.endpoint
        && site.block == block
        && site.index >= start
    {
        site.index += count;
    }
}

pub(super) fn sum_transition(
    function: &mut Function,
    before: &Function,
    types: &TypeInterner,
) -> Result<(), String> {
    validate(before, types)?;
    let mut expected = before.clone();
    let mut changed = Vec::new();
    for block in &mut expected.blocks {
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
        let Some(source_local) = before.local(*source) else {
            continue;
        };
        if condition.ty != TypeInterner::BOOL
            || !matches!(condition.kind, hir::ExpressionKind::Local(local) if local == *target)
            || before
                .local(*target)
                .is_none_or(|local| local.ty != TypeInterner::BOOL)
            || source_local.ty.index() as usize >= types.len()
        {
            continue;
        }
        let selected = match types.resolve(source_local.ty) {
            jett_types::Type::Optional(inner) if *inner == TypeInterner::NEVER => *else_block,
            jett_types::Type::Result(ok, error)
                if *ok == TypeInterner::NEVER
                    && *error != TypeInterner::NEVER
                    && (error.index() as usize) < types.len() =>
            {
                *else_block
            }
            jett_types::Type::Result(ok, error)
                if *ok != TypeInterner::NEVER
                    && *error == TypeInterner::NEVER
                    && (ok.index() as usize) < types.len() =>
            {
                *then_block
            }
            _ => continue,
        };
        changed.push(SelectedEdge {
            block: block.id,
            source: *source,
            tag: *target,
            source_type: source_local.ty,
            selected,
            span: block.terminator.span,
        });
        block.terminator.kind = TerminatorKind::Goto(selected);
    }
    if !blocks_equal(&expected.blocks, &function.blocks) {
        return Err("breakpoint sum transition changes more than an exact Never-side edge".into());
    }
    for record in &mut function.breakpoint_regions {
        for selected in &changed {
            if record.blocks.contains(&selected.block) {
                if record.terminators.contains_key(&selected.block) {
                    record.terminators.insert(
                        selected.block,
                        expected.blocks[selected.block.index() as usize]
                            .terminator
                            .clone(),
                    );
                }
                record.selections.push(selected.clone());
            }
        }
        record.edges = touching(&edges(&expected.blocks), &record.blocks);
        if let Endpoint::Live(endpoint) = &record.endpoint {
            record.abort_allowed |= reachable(&before.blocks, before.entry, endpoint.block)
                && !reachable(&expected.blocks, expected.entry, endpoint.block)
                && !record.selections.is_empty();
        }
    }
    Ok(())
}

fn validate_selection(
    function: &Function,
    selected: &SelectedEdge,
    types: &TypeInterner,
) -> Result<(), String> {
    let Some(block) = function
        .blocks
        .get(selected.block.index() as usize)
        .filter(|block| block.id == selected.block)
    else {
        return Err("breakpoint selected edge has no retained block".into());
    };
    let exact_never = (selected.source_type.index() as usize) < types.len()
        && match types.resolve(selected.source_type) {
            jett_types::Type::Optional(inner) => *inner == TypeInterner::NEVER,
            jett_types::Type::Result(ok, error) => {
                (*ok == TypeInterner::NEVER) != (*error == TypeInterner::NEVER)
            }
            _ => false,
        };
    if !exact_never
        || function
            .local(selected.source)
            .is_none_or(|local| local.ty != selected.source_type)
        || function
            .local(selected.tag)
            .is_none_or(|local| local.ty != TypeInterner::BOOL)
        || !matches!(block.terminator.kind, TerminatorKind::Goto(target) if target == selected.selected)
        || block.terminator.span != selected.span
        || !matches!(block.statements.last().map(|s| &s.kind), Some(StatementKind::SumTag { source, target }) if *source == selected.source && *target == selected.tag)
    {
        return Err("breakpoint selected edge loses its exact runtime sum/tag proof".into());
    }
    Ok(())
}

pub(super) fn remap_blocks(function: &mut Function, map: &[Option<BlockId>]) -> Result<(), String> {
    let get = |id: BlockId| {
        map.get(id.index() as usize)
            .copied()
            .flatten()
            .ok_or_else(|| "breakpoint surviving CFG reference has no block remap".to_string())
    };
    let old = std::mem::take(&mut function.breakpoint_regions);
    let mut records = Vec::new();
    for mut record in old.clone() {
        let surviving = old.iter().any(|child| {
            (child.id == record.id || descendant(child, record.id, &old))
                && (child.statements.keys().any(|site| get(site.block).is_ok())
                    || child.terminators.keys().any(|block| get(*block).is_ok()))
        });
        if !surviving {
            continue;
        }
        if let Endpoint::Live(site) = record.endpoint.clone() {
            if get(site.block).is_err() {
                if !record.abort_allowed
                    || (record.selections.is_empty() && record.loops.is_empty())
                {
                    return Err(
                        "breakpoint endpoint removal lacks canonical selected-edge proof".into(),
                    );
                }
                record.endpoint = Endpoint::CanonicallyAborted {
                    selections: record.selections.clone(),
                    loops: record.loops.clone(),
                };
            }
        }
        record.entry = get(record.entry)?;
        // The entry prefix can itself be unreachable only when the whole region
        // is removed. A surviving fragment never invents a replacement entry.
        record.start.block = get(record.start.block)?;
        record.statements = record
            .statements
            .into_iter()
            .filter_map(|(mut site, value)| {
                let block = get(site.block).ok()?;
                site.block = block;
                Some((site, value))
            })
            .collect();
        let mut terms = BTreeMap::new();
        for (block, mut value) in record.terminators {
            let Ok(block) = get(block) else {
                continue;
            };
            let missing = std::cell::Cell::new(false);
            sequences::prune::block_targets(&mut value.kind, &|id| match get(*id) {
                Ok(new) => *id = new,
                Err(_) => missing.set(true),
            });
            if missing.get() {
                return Err("breakpoint retained terminator targets a removed block".into());
            }
            terms.insert(block, value);
        }
        record.terminators = terms;
        record.blocks = record
            .blocks
            .into_iter()
            .filter_map(|id| get(id).ok())
            .collect();
        record.edges = record
            .edges
            .into_iter()
            .filter_map(|mut edge| {
                edge.source = get(edge.source).ok()?;
                edge.target = get(edge.target).ok()?;
                Some(edge)
            })
            .collect();
        record.selections.retain_mut(|selected| {
            let (Ok(block), Ok(target)) = (get(selected.block), get(selected.selected)) else {
                return false;
            };
            selected.block = block;
            selected.selected = target;
            true
        });
        record.loops.retain_mut(|selected| {
            let (Ok(block), Ok(preheader), Ok(target)) = (
                get(selected.block),
                get(selected.preheader),
                get(selected.selected),
            ) else {
                return false;
            };
            selected.block = block;
            selected.preheader = preheader;
            selected.selected = target;
            true
        });
        match &mut record.endpoint {
            Endpoint::Live(site) => site.block = get(site.block)?,
            Endpoint::CanonicallyAborted { selections, loops } => {
                *selections = record.selections.clone();
                *loops = record.loops.clone();
            }
        }
        records.push(record);
    }
    function.breakpoint_regions = records;
    Ok(())
}

pub(super) fn remap_locals(function: &mut Function, map: &[Option<LocalId>]) -> Result<(), String> {
    for record in &mut function.breakpoint_regions {
        for value in record.statements.values_mut() {
            let mut block = BasicBlock {
                id: BlockId(0),
                statements: vec![value.clone()],
                terminator: Terminator {
                    kind: TerminatorKind::Unreachable,
                    span: value.span,
                },
            };
            remap_snapshot(&mut block, map)?;
            *value = block.statements.remove(0);
        }
        for value in record.terminators.values_mut() {
            let mut block = BasicBlock {
                id: BlockId(0),
                statements: Vec::new(),
                terminator: value.clone(),
            };
            remap_snapshot(&mut block, map)?;
            *value = block.terminator;
        }
        for selected in &mut record.selections {
            selected.source = map
                .get(selected.source.index() as usize)
                .copied()
                .flatten()
                .ok_or("breakpoint selected source has no local remap")?;
            selected.tag = map
                .get(selected.tag.index() as usize)
                .copied()
                .flatten()
                .ok_or("breakpoint selected tag has no local remap")?;
        }
        for selected in &mut record.loops {
            selected.length = map
                .get(selected.length.index() as usize)
                .copied()
                .flatten()
                .ok_or("breakpoint sequence length has no local remap")?;
            let root = match &mut selected.source {
                SequenceSource::Local(root) | SequenceSource::Projected { owner: root, .. } => root,
            };
            *root = map
                .get(root.index() as usize)
                .copied()
                .flatten()
                .ok_or("breakpoint sequence source has no local remap")?;
        }
        if let Endpoint::CanonicallyAborted { selections, loops } = &mut record.endpoint {
            *selections = record.selections.clone();
            *loops = record.loops.clone();
        }
    }
    Ok(())
}

fn descendant(child: &BreakpointRegion, ancestor: usize, records: &[BreakpointRegion]) -> bool {
    let mut parent = child.parent;
    for _ in 0..records.len() {
        let Some(id) = parent else {
            return false;
        };
        if id == ancestor {
            return true;
        }
        parent = records
            .iter()
            .find(|record| record.id == id)
            .and_then(|record| record.parent);
    }
    false
}

fn reachable(blocks: &[BasicBlock], entry: BlockId, target: BlockId) -> bool {
    let all = edges(blocks);
    let mut seen = BTreeSet::new();
    let mut pending = vec![entry];
    while let Some(block) = pending.pop() {
        if !seen.insert(block) {
            continue;
        }
        if block == target {
            return true;
        }
        pending.extend(
            all.iter()
                .filter(|edge| edge.source == block)
                .map(|edge| edge.target),
        );
    }
    false
}

fn empty_sequence(types: &TypeInterner, ty: TypeId) -> bool {
    if ty.index() as usize >= types.len() {
        return false;
    }
    match types.resolve(ty) {
        jett_types::Type::List(element) | jett_types::Type::Set(element) => {
            *element == TypeInterner::NEVER
        }
        jett_types::Type::Map(key, value) => {
            *key == TypeInterner::NEVER || *value == TypeInterner::NEVER
        }
        _ => false,
    }
}

fn validate_loop_selection(
    function: &Function,
    selected: &SelectedLoop,
    types: &TypeInterner,
) -> Result<(), String> {
    if !empty_sequence(types, selected.source_type)
        || selected.source.ty(function) != Some(selected.source_type)
        || function.local(selected.length).is_none_or(|local| local.ty != TypeInterner::INT64)
        || !function.blocks.get(selected.preheader.index() as usize).is_some_and(|block| block.statements.iter().any(|statement| matches!(&statement.kind, StatementKind::SequenceLength { source, target } if source == &selected.source && *target == selected.length)))
        || !function.blocks.get(selected.block.index() as usize).is_some_and(|block| block.terminator.span == selected.span && matches!(block.terminator.kind, TerminatorKind::Goto(target) if target == selected.selected)) {
        return Err("breakpoint empty sequence loses its exact validation or selected edge".into());
    }
    Ok(())
}

pub(super) fn remap_snapshot(
    block: &mut BasicBlock,
    map: &[Option<LocalId>],
) -> Result<(), String> {
    let mut missing = false;
    sequences::prune::block_locals(
        block,
        &mut |id| {
            if let Some(new) = map.get(id.index() as usize).copied().flatten() {
                *id = new;
            } else {
                missing = true;
            }
        },
        &mut |floor| {
            if (*floor as usize) <= map.len() {
                *floor = map[..*floor as usize]
                    .iter()
                    .filter(|id| id.is_some())
                    .count() as u32;
            }
        },
    );
    if missing {
        Err("breakpoint surviving snapshot has no local remap".into())
    } else {
        Ok(())
    }
}

// Typed normalization changes only f64 payloads for equality. Their exact bits
// are compared separately, so NaN and signed zero never weaken a snapshot.
pub(super) fn statement_equal(left: &Statement, right: &Statement) -> bool {
    let span = left.span;
    let left = BasicBlock {
        id: BlockId(0),
        statements: vec![left.clone()],
        terminator: Terminator {
            kind: TerminatorKind::Unreachable,
            span,
        },
    };
    let right = BasicBlock {
        id: BlockId(0),
        statements: vec![right.clone()],
        terminator: Terminator {
            kind: TerminatorKind::Unreachable,
            span,
        },
    };
    block_equal(&left, &right)
}
pub(super) fn terminator_equal(left: &Terminator, right: &Terminator) -> bool {
    block_equal(
        &BasicBlock {
            id: BlockId(0),
            statements: Vec::new(),
            terminator: left.clone(),
        },
        &BasicBlock {
            id: BlockId(0),
            statements: Vec::new(),
            terminator: right.clone(),
        },
    )
}
pub(super) fn blocks_equal(left: &[BasicBlock], right: &[BasicBlock]) -> bool {
    left.len() == right.len()
        && left
            .iter()
            .zip(right)
            .all(|(left, right)| block_equal(left, right))
}
fn block_equal(left: &BasicBlock, right: &BasicBlock) -> bool {
    let (mut left, mut right) = (left.clone(), right.clone());
    let (mut left_bits, mut right_bits) = (Vec::new(), Vec::new());
    float_block(&mut left, &mut left_bits);
    float_block(&mut right, &mut right_bits);
    left == right && left_bits == right_bits
}

fn float_block(block: &mut BasicBlock, bits: &mut Vec<u64>) {
    for statement in &mut block.statements {
        match &mut statement.kind {
            StatementKind::Let { value, .. }
            | StatementKind::BeginCallView { value, .. }
            | StatementKind::CheckRefinement { call: value, .. }
            | StatementKind::Evaluate(value)
            | StatementKind::HandleDefault(value) => float_expression(value, bits),
            StatementKind::Assign { target, value } => {
                float_expression(target, bits);
                float_expression(value, bits);
            }
            StatementKind::Assert { condition, message } => {
                float_expression(condition, bits);
                if let Some(value) = message {
                    float_expression(value, bits);
                }
            }
            StatementKind::Breakpoint { condition, .. } => {
                if let Some(value) = condition {
                    float_expression(value, bits);
                }
            }
            StatementKind::OpenCallOwnerGeneration { .. }
            | StatementKind::ReplaceCallOwnerGeneration { .. }
            | StatementKind::CloseCallOwnerGeneration { .. }
            | StatementKind::EndCallView { .. }
            | StatementKind::ReflectedContainerReady { .. }
            | StatementKind::SequenceLength { .. }
            | StatementKind::SequenceGet { .. }
            | StatementKind::IterationBorrow { .. }
            | StatementKind::SumTag { .. }
            | StatementKind::SumTake { .. }
            | StatementKind::Trace(_) => {}
        }
    }
    match &mut block.terminator.kind {
        TerminatorKind::Return(value) => {
            if let Some(value) = value {
                float_expression(value, bits);
            }
        }
        TerminatorKind::Respond(value)
        | TerminatorKind::Branch {
            condition: value, ..
        }
        | TerminatorKind::Switch {
            scrutinee: value, ..
        }
        | TerminatorKind::ForEach {
            iterable: value, ..
        }
        | TerminatorKind::ReflectedTypeDispatch {
            type_info: value, ..
        } => float_expression(value, bits),
        TerminatorKind::Goto(_) | TerminatorKind::Unreachable => {}
    }
}

fn float_expression(value: &mut Expression, bits: &mut Vec<u64>) {
    use hir::ExpressionKind as E;
    match &mut value.kind {
        E::Float(value) => {
            bits.push(value.to_bits());
            *value = 0.0;
        }
        E::Binary { left, right, .. } => {
            float_expression(left, bits);
            float_expression(right, bits);
        }
        E::Unary { value, .. }
        | E::ResultOk(value)
        | E::ResultFail(value)
        | E::OptionalSome(value)
        | E::Declassify(value)
        | E::Coarsen(value)
        | E::RefinementValidated(value)
        | E::DisplayResult(value)
        | E::EquatableResult(value)
        | E::RuntimeFailureMessage(value)
        | E::InterfaceCoerce { value, .. }
        | E::FunctionAdapter { value, .. }
        | E::InterfaceType(value)
        | E::Comptime { value, .. }
        | E::StateIs { value, .. }
        | E::Run(value)
        | E::Join(value)
        | E::Cancel(value)
        | E::Field { base: value, .. }
        | E::View(value)
        | E::Clone(value) => float_expression(value, bits),
        E::Call { args, .. }
        | E::Intrinsic { args, .. }
        | E::ActorSpawn { args, .. }
        | E::StructConstruct { fields: args, .. }
        | E::BitfieldConstruct { fields: args, .. }
        | E::MachineConstruct { payloads: args, .. }
        | E::EnumConstruct { payloads: args, .. }
        | E::ListConstruct { elements: args } => {
            for arg in args {
                float_expression(arg, bits);
            }
        }
        E::IndirectCall { callee, args, .. }
        | E::ActorMessage {
            actor: callee,
            args,
            ..
        }
        | E::MachineTransition {
            source: callee,
            payloads: args,
            ..
        } => {
            float_expression(callee, bits);
            for arg in args {
                float_expression(arg, bits);
            }
        }
        E::MapConstruct { entries } => {
            for entry in entries {
                float_expression(&mut entry.key, bits);
                float_expression(&mut entry.value, bits);
            }
        }
        E::Handle {
            target, failure, ..
        } => {
            float_expression(target, bits);
            float_hir_block(failure, bits);
        }
        E::StringInterpolation(parts) => {
            for part in parts {
                if let hir::StringSegment::Value(value) = part {
                    float_expression(value, bits);
                }
            }
        }
        E::InlineFunction { body, .. } => float_hir_block(body, bits),
        E::Int(_)
        | E::String(_)
        | E::Bool(_)
        | E::Nothing
        | E::Local(_)
        | E::Constant { .. }
        | E::FunctionRef(_)
        | E::ClosureRef { .. }
        | E::OptionalNone
        | E::RuntimeFailure(_)
        | E::PropertyCaseContext(_) => {}
    }
}

fn float_hir_block(block: &mut hir::Block, bits: &mut Vec<u64>) {
    use hir::StatementKind as H;
    for statement in &mut block.statements {
        match &mut statement.kind {
            H::Let { value, .. }
            | H::HandleDefault(value)
            | H::Expression(value)
            | H::Respond(value) => float_expression(value, bits),
            H::Assign { target, value } => {
                float_expression(target, bits);
                float_expression(value, bits);
            }
            H::Return(value) => {
                if let Some(value) = value {
                    float_expression(value, bits);
                }
            }
            H::If {
                condition,
                then_block,
                else_block,
            } => {
                float_expression(condition, bits);
                float_hir_block(then_block, bits);
                if let Some(block) = else_block {
                    float_hir_block(block, bits);
                }
            }
            H::While { condition, body } => {
                float_expression(condition, bits);
                float_hir_block(body, bits);
            }
            H::For { iterable, body, .. } => {
                float_expression(iterable, bits);
                float_hir_block(body, bits);
            }
            H::Match { scrutinee, arms } => {
                float_expression(scrutinee, bits);
                for arm in arms {
                    float_hir_block(&mut arm.body, bits);
                }
            }
            H::Assert { condition, message } => {
                float_expression(condition, bits);
                if let Some(message) = message {
                    float_expression(message, bits);
                }
            }
            H::Breakpoint { condition, .. } => {
                if let Some(condition) = condition {
                    float_expression(condition, bits);
                }
            }
            H::Scope(block) => float_hir_block(block, bits),
            H::ReflectedTypeDispatch { type_info, arms } => {
                float_expression(type_info, bits);
                for arm in arms {
                    float_hir_block(&mut arm.body, bits);
                }
            }
            H::Trace(_) | H::Break | H::Continue => {}
        }
    }
}

#[cfg(test)]
mod tests;

// Read-only bit-exact expression comparison, sharing the constructor snapshot rule.
pub(super) fn expression_equal(left: &Expression, right: &Expression) -> bool {
    statement_equal(
        &Statement {
            kind: StatementKind::Evaluate(left.clone()),
            span: left.span,
        },
        &Statement {
            kind: StatementKind::Evaluate(right.clone()),
            span: right.span,
        },
    )
}
