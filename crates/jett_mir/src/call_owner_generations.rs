//! Unique ownership of a captured mutable TypeConstruction generation.
//! Escrows are internal native storage, never Jett value locals or Source roots.
use super::*;
use jett_typecheck::{CheckedCalleeAccess, CheckedCallerEffect, CheckedCallerSyntax};
use std::collections::{BTreeMap, BTreeSet, HashSet, VecDeque};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct CallOwnerGenerationId(usize);
impl CallOwnerGenerationId {
    pub fn index(self) -> usize {
        self.0
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct CallOwnerEscrowId(usize);
impl CallOwnerEscrowId {
    pub fn index(self) -> usize {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CallGenerationSlot {
    generation: CallOwnerGenerationId,
    escrow: CallOwnerEscrowId,
    root: LocalId,
    ty: TypeId,
}
impl CallGenerationSlot {
    pub fn generation(&self) -> CallOwnerGenerationId {
        self.generation
    }
    pub fn escrow(&self) -> CallOwnerEscrowId {
        self.escrow
    }
    pub fn root(&self) -> LocalId {
        self.root
    }
    pub fn ty(&self) -> TypeId {
        self.ty
    }
}

#[derive(Debug, Clone)]
pub struct CallGenerationStoragePlan {
    slots: Vec<CallGenerationSlot>,
    loans: BTreeMap<CallOwnerGenerationId, HashSet<LocalId>>,
}
impl CallGenerationStoragePlan {
    pub(crate) fn empty() -> Self {
        Self {
            slots: Vec::new(),
            loans: BTreeMap::new(),
        }
    }
    pub fn slots(&self) -> &[CallGenerationSlot] {
        &self.slots
    }
    pub fn slot(&self, generation: CallOwnerGenerationId) -> Option<&CallGenerationSlot> {
        self.slots
            .get(generation.index())
            .filter(|slot| slot.generation == generation)
    }
    pub(crate) fn covers_loan(&self, generation: CallOwnerGenerationId, loan: LocalId) -> bool {
        self.loans
            .get(&generation)
            .is_some_and(|loans| loans.contains(&loan))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct Site {
    block: BlockId,
    statement: usize,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Edge {
    source: BlockId,
    arm: usize,
    target: BlockId,
}
#[derive(Debug, Clone, PartialEq)]
struct CaptureSite {
    loan: LocalId,
    parameter: usize,
    original_argument: hir::ArgumentOwnership,
    begin: Site,
}
#[derive(Debug, Clone, PartialEq)]
struct ReplacementSite {
    original: hir::Statement,
    rhs: LocalId,
    acquired: Site,
    replacement: Site,
}

/// The original invocation/assignment archive is immutable in original IDs.
/// Only live root/loan/RHS/sites and exact current snapshots are remapped.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct CallOwnerGeneration {
    id: CallOwnerGenerationId,
    slot: CallOwnerEscrowId,
    owner: FunctionId,
    identity: FunctionIdentity,
    entry: BlockId,
    original_call: Expression,
    original_root: hir::CallerBindingFact,
    // Exact original HIR local header. Its full declaration span differs
    // from the immutable Source fact's identifier span and never remaps.
    original_root_header: Local,
    current_source: hir::SourceCallOwnership,
    consumer: Option<Site>,
    root: LocalId,
    captures: Vec<CaptureSite>,
    replacements: Vec<ReplacementSite>,
    start: Site,
    statements: BTreeMap<Site, Statement>,
    terminators: BTreeMap<BlockId, Terminator>,
    blocks: BTreeSet<BlockId>,
    edges: Vec<Edge>,
}

#[derive(Clone)]
struct Scope {
    original: Option<Expression>,
    generations: Vec<CallOwnerGenerationId>,
}
#[derive(Clone)]
struct Draft {
    original_call: Expression,
    original_root: hir::CallerBindingFact,
    // Exact original HIR local header. Its full declaration span differs
    // from the immutable Source fact's identifier span and never remaps.
    original_root_header: Local,
    root: LocalId,
    captures: Vec<CaptureSite>,
    replacements: Vec<ReplacementSite>,
    start: Site,
    statements: BTreeMap<Site, Statement>,
    terminators: BTreeMap<BlockId, Terminator>,
    blocks: BTreeSet<BlockId>,
}
#[derive(Clone, Default)]
pub(super) struct Capture {
    scopes: Vec<Scope>,
    drafts: Vec<Draft>,
    recording: BTreeSet<CallOwnerGenerationId>,
}
impl Capture {
    pub(super) fn begin_scope(&mut self, original: &Expression, types: &TypeInterner) {
        let selected = source_packet(original).is_some_and(|source| source.arguments.iter().any(|argument|
            argument.syntax == CheckedCallerSyntax::WrittenView
                && argument.effect == CheckedCallerEffect::RetainBorrow
                && argument.callee_access == CheckedCalleeAccess::View
                && argument.physical_access == CheckedCalleeAccess::View
                && matches!(&argument.origin, hir::CallerOrigin::Binding(fact) if fact.mode == hir::CallerBindingMode::Owned && fact.mutable && (fact.ty.index() as usize) < types.len() && matches!(types.resolve(fact.ty), jett_types::Type::TypeConstruction))));
        self.scopes.push(Scope {
            original: selected.then(|| original.clone()),
            generations: Vec::new(),
        });
    }
    pub(super) fn end_scope(&mut self) -> Vec<CallOwnerGenerationId> {
        self.scopes
            .pop()
            .map(|scope| scope.generations)
            .unwrap_or_default()
    }
    pub(super) fn abandoned(&self, depth: usize) -> Vec<CallOwnerGenerationId> {
        self.scopes
            .iter()
            .skip(depth)
            .rev()
            .flat_map(|scope| scope.generations.iter().rev().copied())
            .collect()
    }
    pub(super) fn stop(&mut self, id: CallOwnerGenerationId) {
        self.recording.remove(&id);
    }
    pub(super) fn suspend(&mut self) -> Vec<(Option<Expression>, Vec<CallOwnerGenerationId>)> {
        std::mem::take(&mut self.scopes)
            .into_iter()
            .map(|scope| (scope.original, scope.generations))
            .collect()
    }
    pub(super) fn restore(
        &mut self,
        scopes: Vec<(Option<Expression>, Vec<CallOwnerGenerationId>)>,
    ) {
        self.scopes = scopes
            .into_iter()
            .map(|(original, generations)| Scope {
                original,
                generations,
            })
            .collect();
    }
    pub(super) fn protected(&self, root: LocalId) -> Option<CallOwnerGenerationId> {
        self.scopes
            .iter()
            .rev()
            .flat_map(|scope| scope.generations.iter().rev())
            .find(|id| self.drafts[id.index()].root == root)
            .copied()
    }
    pub(super) fn candidate(
        &mut self,
        parameter: usize,
        root: LocalId,
        projection: &Expression,
        block: BlockId,
        statement: usize,
        locals: &[Local],
        types: &TypeInterner,
    ) -> Result<Option<(CallOwnerGenerationId, bool)>, String> {
        let Some(scope) = self.scopes.last() else {
            return Ok(None);
        };
        let Some(original) = &scope.original else {
            return Ok(None);
        };
        let Some(source) = source_packet(original) else {
            return Ok(None);
        };
        let Some(argument) = source.arguments.get(parameter) else {
            return Ok(None);
        };
        let hir::CallerOrigin::Binding(fact) = &argument.origin else {
            return Ok(None);
        };
        let fact = *fact;
        if argument.syntax != CheckedCallerSyntax::WrittenView
            || argument.effect != CheckedCallerEffect::RetainBorrow
            || argument.callee_access != CheckedCalleeAccess::View
            || argument.physical_access != CheckedCalleeAccess::View
            || fact.mode != hir::CallerBindingMode::Owned
            || !fact.mutable
            || fact.local != root
            || fact.ty.index() as usize >= types.len()
            || !matches!(types.resolve(fact.ty), jett_types::Type::TypeConstruction)
        {
            return Ok(None);
        }
        let Some(local) = locals
            .get(root.index() as usize)
            .filter(|local| local.id == root)
        else {
            return Err("call owner generation source root is outside its function".into());
        };
        let E::View(raw) = &projection.kind else {
            return Err("call owner generation needs its original explicit view".into());
        };
        if !matches!(raw.kind, E::Local(id) if id == root)
            || local.ty != fact.ty
            || local.debug_ty != fact.ty
            || !local.mutable
            || local.view_source.is_some()
            || local.span.file != fact.declaration_span.file
            || fact.declaration_span.start < local.span.start
            || fact.declaration_span.end > local.span.end
            || argument.actual_type != fact.ty
            || argument.parameter_type != fact.ty
            || argument.source_witness().occurrence_type() != fact.ty
            || raw.ty != fact.ty
            || projection.ty != fact.ty
            || source_operands(original)
                .and_then(|args| args.get(parameter))
                .is_none_or(|original| {
                    !crate::breakpoint_regions::expression_equal(original, projection)
                })
        {
            return Err("call owner generation changes its exact plain source binding".into());
        }
        if let Some(id) = self.protected(root) {
            if !self
                .scopes
                .last()
                .is_some_and(|scope| scope.generations.contains(&id))
                || !self.drafts[id.index()].replacements.is_empty()
            {
                return Err(
                    "call owner generation needs a separate overlapping cohort proof".into(),
                );
            }
            return Ok(Some((id, false)));
        }
        let id = CallOwnerGenerationId(self.drafts.len());
        let draft = Draft {
            original_call: original.clone(),
            original_root: fact,
            original_root_header: local.clone(),
            root,
            captures: Vec::new(),
            replacements: Vec::new(),
            start: Site { block, statement },
            statements: BTreeMap::new(),
            terminators: BTreeMap::new(),
            blocks: BTreeSet::from([block]),
        };
        self.drafts.push(draft);
        self.scopes
            .last_mut()
            .ok_or("call owner generation has no current scope")?
            .generations
            .push(id);
        self.recording.insert(id);
        Ok(Some((id, true)))
    }
    pub(super) fn capture_loan(
        &mut self,
        id: CallOwnerGenerationId,
        loan: LocalId,
        parameter: usize,
        block: BlockId,
        statement: usize,
    ) -> Result<(), String> {
        let draft = self
            .drafts
            .get_mut(id.index())
            .ok_or("call owner generation capture is absent")?;
        let argument = source_packet(&draft.original_call)
            .and_then(|source| source.arguments.get(parameter))
            .ok_or("call owner generation original source argument is absent")?
            .clone();
        if draft
            .captures
            .iter()
            .any(|capture| capture.loan == loan || capture.parameter == parameter)
        {
            return Err("call owner generation duplicates a capture".into());
        }
        draft.captures.push(CaptureSite {
            loan,
            parameter,
            original_argument: argument,
            begin: Site { block, statement },
        });
        Ok(())
    }
    pub(super) fn replacement(
        &mut self,
        id: CallOwnerGenerationId,
        original: &hir::Statement,
        rhs: LocalId,
        acquired: (BlockId, usize),
        replacement: (BlockId, usize),
    ) -> Result<(), String> {
        let draft = self
            .drafts
            .get_mut(id.index())
            .ok_or("call owner generation replacement is absent")?;
        let hir::StatementKind::Assign { target, value } = &original.kind else {
            return Err("call owner generation replacement is not its source Assign".into());
        };
        if !matches!(target.kind, E::Local(root) if root == draft.root)
            || target.ty != draft.original_root.ty
            || value.ty != target.ty
            || rhs == draft.root
        {
            return Err(
                "call owner generation replacement changes its root or acquired type".into(),
            );
        }
        draft.replacements.push(ReplacementSite {
            original: original.clone(),
            rhs,
            acquired: Site {
                block: acquired.0,
                statement: acquired.1,
            },
            replacement: Site {
                block: replacement.0,
                statement: replacement.1,
            },
        });
        Ok(())
    }
    pub(super) fn statement(&mut self, block: BlockId, statement: usize, value: &Statement) {
        for id in &self.recording {
            let draft = &mut self.drafts[id.index()];
            draft.blocks.insert(block);
            draft
                .statements
                .insert(Site { block, statement }, value.clone());
        }
    }
    pub(super) fn terminator(&mut self, block: BlockId, value: &Terminator) {
        for id in &self.recording {
            let draft = &mut self.drafts[id.index()];
            draft.blocks.insert(block);
            draft.terminators.insert(block, value.clone());
        }
    }
    pub(super) fn block(&mut self, block: BlockId) {
        for id in &self.recording {
            self.drafts[id.index()].blocks.insert(block);
        }
    }
    pub(super) fn seal(self, function: &Function) -> Result<Vec<CallOwnerGeneration>, String> {
        if !self.scopes.is_empty() || !self.recording.is_empty() {
            return Err("call owner generation constructor left an open source scope".into());
        }
        let edges = all_edges(function);
        let records = self.drafts.into_iter().enumerate().map(|(index, draft)| CallOwnerGeneration {
            id: CallOwnerGenerationId(index), slot: CallOwnerEscrowId(index), owner: function.id, identity: function.identity.clone(), entry: function.entry,
            current_source: draft.statements.values().find_map(|statement| {
                statement_source(statement).filter(|source| draft.captures.iter().all(|capture|
                    source.arguments.get(capture.parameter).is_some_and(|argument|
                        matches!(argument.staging, hir::ArgumentStaging::Borrowed { loan } if loan == capture.loan))))
                    .cloned()
            }).expect("constructor-selected consuming Source invocation"),
            consumer: draft.statements.iter().find_map(|(site, statement)| {
                statement_source(statement).filter(|source| draft.captures.iter().all(|capture|
                    source.arguments.get(capture.parameter).is_some_and(|argument|
                        matches!(argument.staging, hir::ArgumentStaging::Borrowed { loan } if loan == capture.loan))))
                    .map(|_| *site)
            }),
            original_call: draft.original_call, original_root: draft.original_root,
            original_root_header: draft.original_root_header, root: draft.root,
            captures: draft.captures, replacements: draft.replacements, start: draft.start,
            statements: draft.statements, terminators: draft.terminators,
            edges: touching(&edges, &draft.blocks), blocks: draft.blocks,
        }).collect::<Vec<_>>();
        if records.iter().any(|record| record.consumer.is_none()) {
            return Err("call owner generation has no constructor-owned Source consumer".into());
        }
        Ok(records)
    }
}

fn statement_source(statement: &Statement) -> Option<&hir::SourceCallOwnership> {
    match &statement.kind {
        StatementKind::Let { value, .. } | StatementKind::Evaluate(value) => source_packet(value),
        _ => None,
    }
}

pub(super) fn has_records(function: &Function) -> bool {
    !function.call_owner_generations.is_empty()
}

fn source_operands(value: &Expression) -> Option<&[Expression]> {
    match &value.kind {
        E::Call { args, .. } | E::IndirectCall { args, .. } | E::Intrinsic { args, .. } => {
            Some(args)
        }
        _ => None,
    }
}

fn source_packet(value: &Expression) -> Option<&hir::SourceCallOwnership> {
    match &value.kind {
        E::Call {
            ownership: hir::CallOwnership::Source(source),
            ..
        }
        | E::IndirectCall {
            ownership: hir::CallOwnership::Source(source),
            ..
        }
        | E::Intrinsic {
            ownership: hir::CallOwnership::Source(source),
            ..
        } => Some(source),
        _ => None,
    }
}
use hir::ExpressionKind as E;
fn all_edges(function: &Function) -> Vec<Edge> {
    let mut result = Vec::new();
    for block in &function.blocks {
        let targets = match &block.terminator.kind {
            TerminatorKind::Goto(target) => vec![*target],
            TerminatorKind::Branch {
                then_block,
                else_block,
                ..
            } => vec![*then_block, *else_block],
            TerminatorKind::Switch {
                variants,
                otherwise,
                ..
            } => variants
                .iter()
                .map(|(_, target, _)| *target)
                .chain(*otherwise)
                .collect(),
            TerminatorKind::ForEach { body, exit, .. } => vec![*body, *exit],
            TerminatorKind::ReflectedTypeDispatch {
                arms, otherwise, ..
            } => arms
                .iter()
                .map(|arm| arm.target)
                .chain(std::iter::once(*otherwise))
                .collect(),
            TerminatorKind::Return(_)
            | TerminatorKind::Respond(_)
            | TerminatorKind::Unreachable => Vec::new(),
        };
        result.extend(targets.into_iter().enumerate().map(|(arm, target)| Edge {
            source: block.id,
            arm,
            target,
        }));
    }
    result
}
fn touching(edges: &[Edge], blocks: &BTreeSet<BlockId>) -> Vec<Edge> {
    edges
        .iter()
        .copied()
        .filter(|edge| blocks.contains(&edge.source) || blocks.contains(&edge.target))
        .collect()
}
fn operation(statement: &StatementKind) -> Option<CallOwnerGenerationId> {
    match statement {
        StatementKind::OpenCallOwnerGeneration { generation, .. }
        | StatementKind::ReplaceCallOwnerGeneration { generation, .. }
        | StatementKind::CloseCallOwnerGeneration { generation } => Some(*generation),
        _ => None,
    }
}

pub(super) fn validate(
    function: &Function,
    types: &TypeInterner,
) -> Result<CallGenerationStoragePlan, String> {
    let records = &function.call_owner_generations;
    if records.is_empty() {
        return if function
            .blocks
            .iter()
            .flat_map(|block| &block.statements)
            .any(|statement| operation(&statement.kind).is_some())
        {
            Err("call owner generation operation has no constructor-owned record".into())
        } else {
            Ok(CallGenerationStoragePlan::empty())
        };
    }
    let mut plan = CallGenerationStoragePlan::empty();
    let mut occurrences = BTreeMap::new();
    let edges = all_edges(function);
    for (index, record) in records.iter().enumerate() {
        if record.id.index() != index
            || record.slot.index() != index
            || record.owner != function.id
            || record.identity != function.identity
            || record.entry != function.entry
        {
            return Err("call owner generation has duplicate or foreign ownership".into());
        }
        let root = function
            .local(record.root)
            .ok_or("call owner generation root is absent")?;
        let header = &record.original_root_header;
        if header.id != record.original_root.local
            || header.ty != record.original_root.ty
            || header.debug_ty != header.ty
            || !header.mutable
            || header.view_source.is_some()
            || header.span.file != record.original_root.declaration_span.file
            || record.original_root.declaration_span.start < header.span.start
            || record.original_root.declaration_span.end > header.span.end
        {
            return Err(
                "call owner generation changes its sealed original source or local header".into(),
            );
        }
        if root.name != header.name
            || root.ty != header.ty
            || root.debug_ty != header.debug_ty
            || root.debug_type_name != header.debug_type_name
            || root.mutable != header.mutable
            || root.view_source != header.view_source
            || root.span != header.span
        {
            return Err("call owner generation changes its sealed local declaration header".into());
        }
        if root.ty != record.original_root.ty
            || root.debug_ty != root.ty
            || !root.mutable
            || root.view_source.is_some()
            || function.parameter_for_local(root.id).is_some()
            || root.ty.index() as usize >= types.len()
            || !matches!(types.resolve(root.ty), jett_types::Type::TypeConstruction)
            || record.original_root.mode != hir::CallerBindingMode::Owned
            || !record.original_root.mutable
            || record.captures.is_empty()
            || !record.blocks.contains(&record.start.block)
            || touching(&edges, &record.blocks) != record.edges
        {
            return Err(
                "call owner generation loses its owning mutable plain source root or CFG".into(),
            );
        }
        for (&site, snapshot) in &record.statements {
            let actual = function
                .blocks
                .get(site.block.index() as usize)
                .filter(|block| block.id == site.block)
                .and_then(|block| block.statements.get(site.statement));
            if !record.blocks.contains(&site.block)
                || actual.is_none_or(|actual| {
                    !crate::breakpoint_regions::statement_equal(actual, snapshot)
                })
            {
                return Err(
                    "call owner generation changes an exact constructor-owned statement".into(),
                );
            }
            if operation(&snapshot.kind) == Some(record.id)
                && occurrences.insert(site, record.id).is_some()
            {
                return Err("call owner generation duplicates its operation occurrence".into());
            }
        }
        for (&block, snapshot) in &record.terminators {
            if function
                .blocks
                .get(block.index() as usize)
                .filter(|value| value.id == block)
                .is_none_or(|value| {
                    !crate::breakpoint_regions::terminator_equal(&value.terminator, snapshot)
                })
            {
                return Err(
                    "call owner generation changes an exact constructor-owned terminator".into(),
                );
            }
        }
        if !matches!(record.statements.get(&record.start).map(|s| &s.kind), Some(StatementKind::OpenCallOwnerGeneration { generation, root }) if *generation == record.id && *root == record.root)
        {
            return Err("call owner generation loses its unique original Open".into());
        }
        let mut loans = HashSet::new();
        for capture in &record.captures {
            let loan = function
                .local(capture.loan)
                .ok_or("call owner generation loan is absent")?;
            if !loans.insert(capture.loan)
                || loan.ty != root.ty
                || loan.view_source != Some(root.id)
                || function.parameter_for_local(loan.id).is_some()
            {
                return Err(
                    "call owner generation duplicates or changes its nonowning capture".into(),
                );
            }
            let Some(Statement {
                kind: StatementKind::BeginCallView { local, value },
                ..
            }) = record.statements.get(&capture.begin)
            else {
                return Err("call owner generation capture loses its exact Begin".into());
            };
            let E::View(raw) = &value.kind else {
                return Err(
                    "call owner generation capture needs its original explicit view".into(),
                );
            };
            let argument = &capture.original_argument;
            if source_packet(&record.original_call)
                .and_then(|source| source.arguments.get(capture.parameter))
                != Some(argument)
                || !matches!(&argument.origin, hir::CallerOrigin::Binding(fact) if *fact == record.original_root)
            {
                return Err(
                    "call owner generation loses its immutable original source witness".into(),
                );
            }
            if *local != loan.id
                || raw.ty != root.ty
                || value.ty != root.ty
                || !matches!(raw.kind, E::Local(id) if id == root.id)
                || source_operands(&record.original_call)
                    .and_then(|args| args.get(capture.parameter))
                    .is_none_or(|original| original.span != value.span)
                || argument.actual_type != root.ty
                || argument.parameter_type != root.ty
                || argument.source_witness().occurrence_type() != root.ty
                || argument.syntax != CheckedCallerSyntax::WrittenView
                || argument.effect != CheckedCallerEffect::RetainBorrow
                || argument.callee_access != CheckedCalleeAccess::View
                || argument.physical_access != CheckedCalleeAccess::View
            {
                return Err(
                    "call owner generation capture changes its complete source contract".into(),
                );
            }
        }
        for replacement in &record.replacements {
            let rhs = function
                .local(replacement.rhs)
                .ok_or("call owner generation acquired RHS is absent")?;
            if rhs.id == root.id
                || rhs.ty != root.ty
                || rhs.view_source.is_some()
                || function.parameter_for_local(rhs.id).is_some()
                || !matches!(record.statements.get(&replacement.acquired).map(|s| &s.kind), Some(StatementKind::Let { local, value }) if *local == rhs.id && value.ty == root.ty)
                || !matches!(record.statements.get(&replacement.replacement).map(|s| &s.kind), Some(StatementKind::ReplaceCallOwnerGeneration { generation, root: actual, rhs_owner }) if *generation == record.id && *actual == root.id && *rhs_owner == rhs.id)
            {
                return Err(
                    "call owner generation replacement loses its prior independent owning RHS"
                        .into(),
                );
            }
            dominates(function, replacement.acquired, replacement.replacement)?;
            if function.blocks.iter().flat_map(|block| &block.statements).filter(|statement| matches!(statement.kind, StatementKind::Let { local, .. } | StatementKind::BeginCallView { local, .. } if local == rhs.id)).count() != 1 {
                return Err("call owner generation RHS does not have a unique acquisition".into());
            }
        }
        if let Some(consumer) = record.consumer {
            let source = record
                .statements
                .get(&consumer)
                .and_then(statement_source)
                .ok_or("call owner generation loses its exact Source consumer")?;
            for capture in &record.captures {
                let argument = source
                    .arguments
                    .get(capture.parameter)
                    .ok_or("call owner generation consumer loses its formal")?;
                if !matches!(argument.staging, hir::ArgumentStaging::Borrowed { loan } if loan == capture.loan)
                {
                    return Err("call owner generation consumer changes its nonowning loan".into());
                }
                dominates(function, capture.begin, consumer)?;
            }
            if *source != record.current_source {
                return Err(
                    "call owner generation consumer changes its original closed Source witness"
                        .into(),
                );
            }
        }
        plan.slots.push(CallGenerationSlot {
            generation: record.id,
            escrow: record.slot,
            root: root.id,
            ty: root.ty,
        });
        plan.loans.insert(record.id, loans);
    }
    for block in &function.blocks {
        for (statement, value) in block.statements.iter().enumerate() {
            if let Some(generation) = operation(&value.kind) {
                if occurrences.get(&Site {
                    block: block.id,
                    statement,
                }) != Some(&generation)
                {
                    return Err(
                        "call owner generation operation has no exact constructor-owned occurrence"
                            .into(),
                    );
                }
            }
        }
    }
    if !records.is_empty() {
        validate_protocol(function, &plan)?;
    }
    Ok(plan)
}

fn dominates(function: &Function, acquired: Site, used: Site) -> Result<(), String> {
    let cfg = ControlFlowGraph::analyze(function).map_err(|errors| format!("{errors:?}"))?;
    let mut queue = vec![used];
    let mut seen = BTreeSet::new();
    let mut reached = false;
    while let Some(site) = queue.pop() {
        if site.block == acquired.block && acquired.statement < site.statement {
            reached = true;
            continue;
        }
        if !seen.insert(site.block) {
            continue;
        }
        if site.block == function.entry || cfg.predecessors(site.block).is_empty() {
            return Err("call owner generation replacement precedes RHS acquisition".into());
        }
        for &block in cfg.predecessors(site.block) {
            queue.push(Site {
                block,
                statement: function.blocks[block.index() as usize].statements.len(),
            });
        }
    }
    if !reached {
        return Err("call owner generation RHS has an uninitialized cycle".into());
    }
    Ok(())
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum StorageState {
    Closed,
    Attached,
    Retired,
    MaybeRetired,
}
#[derive(Clone, PartialEq, Eq)]
struct ProtocolState {
    storage: StorageState,
    loans: HashSet<LocalId>,
}
impl ProtocolState {
    fn closed() -> Self {
        Self {
            storage: StorageState::Closed,
            loans: HashSet::new(),
        }
    }
}
fn merge(left: &ProtocolState, right: &ProtocolState) -> Result<ProtocolState, String> {
    let storage = match (left.storage, right.storage) {
        (StorageState::Closed, StorageState::Closed) => StorageState::Closed,
        (StorageState::Closed, _) | (_, StorageState::Closed) => {
            return Err("call owner generation has a path bypassing its active scope".into());
        }
        (left, right) if left == right => left,
        _ => StorageState::MaybeRetired,
    };
    if left.loans != right.loans {
        return Err("call owner generation joins different active capture cohorts".into());
    }
    Ok(ProtocolState {
        storage,
        loans: left.loans.clone(),
    })
}
fn validate_protocol(function: &Function, plan: &CallGenerationStoragePlan) -> Result<(), String> {
    let cfg = ControlFlowGraph::analyze(function).map_err(|errors| format!("{errors:?}"))?;
    let initial = vec![ProtocolState::closed(); plan.slots.len()];
    let mut outgoing: Vec<Option<Vec<ProtocolState>>> = vec![None; function.blocks.len()];
    let mut queue = VecDeque::from([function.entry]);
    let mut pending = BTreeSet::from([function.entry]);
    while let Some(block) = queue.pop_front() {
        pending.remove(&block);
        let mut incoming: Option<Vec<ProtocolState>> =
            (block == function.entry).then(|| initial.clone());
        for predecessor in cfg.predecessors(block) {
            if let Some(states) = &outgoing[predecessor.index() as usize] {
                if let Some(current) = &mut incoming {
                    for (current, state) in current.iter_mut().zip(states) {
                        *current = merge(current, state)?;
                    }
                } else {
                    incoming = Some(states.clone());
                }
            }
        }
        let Some(mut states) = incoming else {
            continue;
        };
        transfer(function, block, plan, &mut states)?;
        if outgoing[block.index() as usize].as_ref() != Some(&states) {
            outgoing[block.index() as usize] = Some(states);
            for &successor in cfg.successors(block) {
                if pending.insert(successor) {
                    queue.push_back(successor);
                }
            }
        }
    }
    // Disconnected blocks remain an integrity obligation, never a permission.
    for block in &function.blocks {
        if outgoing[block.id.index() as usize].is_none() {
            let mut states = initial.clone();
            transfer(function, block.id, plan, &mut states)?;
        }
    }
    Ok(())
}
fn transfer(
    function: &Function,
    block: BlockId,
    plan: &CallGenerationStoragePlan,
    states: &mut [ProtocolState],
) -> Result<(), String> {
    for statement in &function.blocks[block.index() as usize].statements {
        match &statement.kind {
            StatementKind::OpenCallOwnerGeneration { generation, root } => {
                let slot = plan
                    .slot(*generation)
                    .ok_or("call owner generation Open has no storage")?;
                let state = &mut states[generation.index()];
                if *root != slot.root
                    || state.storage != StorageState::Closed
                    || !state.loans.is_empty()
                {
                    return Err("call owner generation opens an active or foreign escrow".into());
                }
                state.storage = StorageState::Attached;
            }
            StatementKind::ReplaceCallOwnerGeneration {
                generation,
                root,
                rhs_owner,
            } => {
                let slot = plan
                    .slot(*generation)
                    .ok_or("call owner generation Replace has no storage")?;
                let state = &mut states[generation.index()];
                if *root != slot.root
                    || *rhs_owner == *root
                    || state.storage == StorageState::Closed
                    || state.loans.is_empty()
                {
                    return Err(
                        "call owner generation replacement has no captured active owner".into(),
                    );
                }
                state.storage = StorageState::Retired;
            }
            StatementKind::CloseCallOwnerGeneration { generation } => {
                let state = states
                    .get_mut(generation.index())
                    .ok_or("call owner generation Close has no storage")?;
                if state.storage == StorageState::Closed || !state.loans.is_empty() {
                    return Err(
                        "call owner generation closes an active loan or already closed escrow"
                            .into(),
                    );
                }
                state.storage = StorageState::Closed;
            }
            StatementKind::BeginCallView { local, .. } => {
                for (id, loans) in &plan.loans {
                    if loans.contains(local) {
                        let state = &mut states[id.index()];
                        if state.storage != StorageState::Attached || !state.loans.insert(*local) {
                            return Err(
                                "call owner generation recaptures a retired or active cohort"
                                    .into(),
                            );
                        }
                    }
                }
            }
            StatementKind::EndCallView { local } => {
                for (id, loans) in &plan.loans {
                    if loans.contains(local) && !states[id.index()].loans.remove(local) {
                        return Err("call owner generation End has no active capture".into());
                    }
                }
            }
            StatementKind::Assign { target, .. } => {
                if let E::Local(root) = target.kind {
                    if plan.slots.iter().any(|slot| {
                        slot.root == root
                            && states[slot.generation.index()].storage != StorageState::Closed
                    }) {
                        return Err(
                            "call owner generation protected root still uses ordinary Assign"
                                .into(),
                        );
                    }
                }
            }
            _ => {}
        }
    }
    if matches!(
        function.blocks[block.index() as usize].terminator.kind,
        TerminatorKind::Return(_) | TerminatorKind::Respond(_)
    ) && states
        .iter()
        .any(|state| state.storage != StorageState::Closed || !state.loans.is_empty())
    {
        return Err("call owner generation exits before exact loan and escrow cleanup".into());
    }
    Ok(())
}

// Only finite canonical preparation calls these transitions. Original archives
// never change and are never marked as runtime or liveness roots.
fn shift(record: &mut CallOwnerGeneration, block: BlockId, amount: usize) {
    let shift_site = |site: &mut Site| {
        if site.block == block {
            site.statement += amount;
        }
    };
    shift_site(&mut record.start);
    if let Some(consumer) = &mut record.consumer {
        shift_site(consumer);
    }
    for capture in &mut record.captures {
        shift_site(&mut capture.begin);
    }
    for replacement in &mut record.replacements {
        shift_site(&mut replacement.acquired);
        shift_site(&mut replacement.replacement);
    }
    record.statements = std::mem::take(&mut record.statements)
        .into_iter()
        .map(|(mut site, value)| {
            shift_site(&mut site);
            (site, value)
        })
        .collect();
}
pub(super) fn sequence_transition(
    function: &mut Function,
    before: &Function,
    edit: &crate::breakpoint_regions::SequenceEdit,
    types: &TypeInterner,
) -> Result<(), String> {
    if !has_records(before) {
        return Ok(());
    }
    validate(before, types)?;
    // Reuse the identical complete finite before/after edit validation. This
    // shadow cannot mint a generation or change the actual breakpoint records.
    let mut shadow = function.clone();
    crate::breakpoint_regions::sequence_transition(&mut shadow, before, edit.clone(), types)?;
    let mut records = before.call_owner_generations.clone();
    for record in &mut records {
        if let Some((block, prefix)) = &edit.prefix {
            shift(record, *block, prefix.len());
        }
        let owns_header = record.terminators.contains_key(&edit.header);
        if owns_header {
            record.terminators.insert(edit.header, edit.after.clone());
        }
        for (block, _, new) in &edit.redirects {
            if record.terminators.contains_key(block) {
                record.terminators.insert(*block, new.clone());
            }
        }
        if owns_header {
            for (block, values) in &edit.append {
                record.blocks.insert(*block);
                let offset = before.blocks[block.index() as usize].statements.len();
                let offset = offset
                    + edit
                        .prefix
                        .as_ref()
                        .filter(|(id, _)| id == block)
                        .map_or(0, |(_, values)| values.len());
                for (index, value) in values.iter().enumerate() {
                    record.statements.insert(
                        Site {
                            block: *block,
                            statement: offset + index,
                        },
                        value.clone(),
                    );
                }
            }
            if let Some((block, values)) = &edit.prefix {
                record.blocks.insert(*block);
                for (index, value) in values.iter().enumerate() {
                    record.statements.insert(
                        Site {
                            block: *block,
                            statement: index,
                        },
                        value.clone(),
                    );
                }
            }
            for block in &edit.blocks {
                record.blocks.insert(block.id);
                record
                    .terminators
                    .insert(block.id, block.terminator.clone());
                for (index, value) in block.statements.iter().enumerate() {
                    record.statements.insert(
                        Site {
                            block: block.id,
                            statement: index,
                        },
                        value.clone(),
                    );
                }
            }
        }
        record.edges = touching(&all_edges(function), &record.blocks);
    }
    function.call_owner_generations = records;
    Ok(())
}
pub(super) fn sum_transition(
    function: &mut Function,
    before: &Function,
    types: &TypeInterner,
) -> Result<(), String> {
    if !has_records(before) {
        return Ok(());
    }
    validate(before, types)?;
    let mut shadow = function.clone();
    crate::breakpoint_regions::sum_transition(&mut shadow, before, types)?;
    for record in &mut function.call_owner_generations {
        for (block, snapshot) in &mut record.terminators {
            let actual = &function.blocks[block.index() as usize].terminator;
            if !crate::breakpoint_regions::terminator_equal(snapshot, actual) {
                *snapshot = actual.clone();
            }
        }
        record.edges = touching(&all_edges_from_blocks(&function.blocks), &record.blocks);
    }
    Ok(())
}
fn all_edges_from_blocks(blocks: &[BasicBlock]) -> Vec<Edge> {
    let mut edges = Vec::new();
    for block in blocks {
        let targets = match &block.terminator.kind {
            TerminatorKind::Goto(target) => vec![*target],
            TerminatorKind::Branch {
                then_block,
                else_block,
                ..
            } => vec![*then_block, *else_block],
            TerminatorKind::Switch {
                variants,
                otherwise,
                ..
            } => variants
                .iter()
                .map(|(_, target, _)| *target)
                .chain(*otherwise)
                .collect(),
            TerminatorKind::ForEach { body, exit, .. } => vec![*body, *exit],
            TerminatorKind::ReflectedTypeDispatch {
                arms, otherwise, ..
            } => arms
                .iter()
                .map(|arm| arm.target)
                .chain(std::iter::once(*otherwise))
                .collect(),
            TerminatorKind::Return(_)
            | TerminatorKind::Respond(_)
            | TerminatorKind::Unreachable => Vec::new(),
        };
        edges.extend(targets.into_iter().enumerate().map(|(arm, target)| Edge {
            source: block.id,
            arm,
            target,
        }));
    }
    edges
}
fn remap_statement(value: &mut Statement, map: &[Option<LocalId>]) -> Result<(), String> {
    let mut block = BasicBlock {
        id: BlockId(0),
        statements: vec![value.clone()],
        terminator: Terminator {
            kind: TerminatorKind::Unreachable,
            span: value.span,
        },
    };
    crate::breakpoint_regions::remap_snapshot(&mut block, map)?;
    *value = block.statements.remove(0);
    Ok(())
}
pub(super) fn remap_locals(function: &mut Function, map: &[Option<LocalId>]) -> Result<(), String> {
    let get = |id: LocalId| {
        map.get(id.index() as usize)
            .copied()
            .flatten()
            .ok_or("generation surviving operand has no local remap")
    };
    for record in &mut function.call_owner_generations {
        record.root = get(record.root)?;
        for capture in &mut record.captures {
            capture.loan = get(capture.loan)?;
        }
        for replacement in &mut record.replacements {
            replacement.rhs = get(replacement.rhs)?;
        }
        for value in record.statements.values_mut() {
            remap_statement(value, map)?;
        }
        for value in record.terminators.values_mut() {
            let mut block = BasicBlock {
                id: BlockId(0),
                statements: Vec::new(),
                terminator: value.clone(),
            };
            crate::breakpoint_regions::remap_snapshot(&mut block, map)?;
            *value = block.terminator;
        }
        if record.consumer.is_some() {
            let mut ownership = hir::CallOwnership::Source(record.current_source.clone());
            ownership.remap_metadata_locals(|id| get(id).map_err(str::to_owned))?;
            let hir::CallOwnership::Source(source) = ownership else {
                unreachable!()
            };
            record.current_source = source;
        }
    }
    Ok(())
}
pub(super) fn remap_blocks(
    function: &mut Function,
    before: &Function,
    map: &[Option<BlockId>],
) -> Result<(), String> {
    if !has_records(before) {
        return Ok(());
    }
    // Exactly the reachable compaction, never a caller-supplied subset/range.
    let cfg = ControlFlowGraph::analyze(before).map_err(|errors| format!("{errors:?}"))?;
    let reachable = cfg
        .reverse_postorder()
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    if map.len() != before.blocks.len()
        || before
            .blocks
            .iter()
            .any(|block| map[block.id.index() as usize].is_some() != reachable.contains(&block.id))
    {
        return Err("generation block remap is not exact reachable compaction".into());
    }
    let get = |id: BlockId| {
        map.get(id.index() as usize)
            .copied()
            .flatten()
            .ok_or("generation surviving site has no block remap")
    };
    let mut expected = before
        .blocks
        .iter()
        .filter(|block| reachable.contains(&block.id))
        .cloned()
        .collect::<Vec<_>>();
    for block in &mut expected {
        block.id = get(block.id)?;
        crate::sequences::prune::block_targets(&mut block.terminator.kind, &|target| {
            // Reachability includes every actual target, so this cannot be absent.
            *target = map[target.index() as usize].expect("reachable edge target remains");
        });
    }
    if !crate::breakpoint_regions::blocks_equal(&expected, &function.blocks)
        || function.entry != get(before.entry)?
    {
        return Err("generation compaction changes surviving original blocks".into());
    }
    let mut records = Vec::new();
    let mut ids = vec![None; before.call_owner_generations.len()];
    for mut record in before.call_owner_generations.clone() {
        if get(record.start.block).is_err() {
            if record.statements.keys().any(|site| get(site.block).is_ok()) {
                return Err("generation surviving operations lose their Open".into());
            }
            continue;
        }
        let new_id = CallOwnerGenerationId(records.len());
        ids[record.id.index()] = Some(new_id);
        record.id = new_id;
        record.slot = CallOwnerEscrowId(new_id.index());
        record.entry = get(record.entry)?;
        record.start.block = get(record.start.block)?;
        record.consumer = record.consumer.and_then(|mut site| {
            site.block = get(site.block).ok()?;
            Some(site)
        });
        record.captures.retain_mut(|capture| {
            if let Ok(block) = get(capture.begin.block) {
                capture.begin.block = block;
                true
            } else {
                false
            }
        });
        record.replacements.retain_mut(|replacement| {
            if let (Ok(acquired), Ok(replaced)) = (
                get(replacement.acquired.block),
                get(replacement.replacement.block),
            ) {
                replacement.acquired.block = acquired;
                replacement.replacement.block = replaced;
                true
            } else {
                false
            }
        });
        record.statements = record
            .statements
            .into_iter()
            .filter_map(|(mut site, value)| {
                site.block = get(site.block).ok()?;
                Some((site, value))
            })
            .collect();
        record.terminators = record
            .terminators
            .into_iter()
            .filter_map(|(block, mut value)| {
                let block = get(block).ok()?;
                crate::sequences::prune::block_targets(&mut value.kind, &|target| {
                    *target = map[target.index() as usize].expect("reachable snapshot edge")
                });
                Some((block, value))
            })
            .collect();
        record.blocks = record
            .blocks
            .into_iter()
            .filter_map(|block| get(block).ok())
            .collect();
        records.push(record);
    }
    let remap_operation = |statement: &mut Statement| -> Result<(), String> {
        match &mut statement.kind {
            StatementKind::OpenCallOwnerGeneration { generation, .. }
            | StatementKind::ReplaceCallOwnerGeneration { generation, .. }
            | StatementKind::CloseCallOwnerGeneration { generation } => {
                *generation = ids
                    .get(generation.index())
                    .copied()
                    .flatten()
                    .ok_or("generation operation survives its removed record")?;
            }
            _ => {}
        }
        Ok(())
    };
    for block in &mut function.blocks {
        for statement in &mut block.statements {
            remap_operation(statement)?;
        }
    }
    for record in &mut records {
        for value in record.statements.values_mut() {
            remap_operation(value)?;
        }
        record.edges = touching(&all_edges(function), &record.blocks);
    }
    function.call_owner_generations = records;
    Ok(())
}

#[cfg(test)]
mod tests;
