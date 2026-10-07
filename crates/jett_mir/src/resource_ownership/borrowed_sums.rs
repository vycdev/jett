//! Constructor-owned, nonowning Handle projections. Public sum syntax is inert.
use super::*;
use hir::ExpressionKind as E;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ResourceSumPayloadPath {
    OptionalSome,
    ResultOk,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ResourceBorrowedSumProjection {
    original: Expression,
    original_alias: Local,
    original_backing: Local,
    alias: Local,
    backing: Local,
    source: Local,
    output: Local,
    tag: Local,
    error: Option<Local>,
    initialize: ResourceSite,
    observe: ResourceSite,
    project: ResourceSite,
    failure: Option<ResourceSite>,
    failed: BlockId,
    continuation: BlockId,
    path: ResourceSumPayloadPath,
}
impl ResourceBorrowedSumProjection {
    pub fn original_initializer(&self) -> &Expression {
        &self.original
    }
    pub fn original_handle(&self) -> &Expression {
        let E::View(handle) = &self.original.kind else {
            unreachable!()
        };
        handle
    }
    pub fn alias(&self) -> LocalId {
        self.alias.id
    }
    pub fn backing(&self) -> LocalId {
        self.backing.id
    }
    pub fn source(&self) -> LocalId {
        self.source.id
    }
    pub fn output(&self) -> LocalId {
        self.output.id
    }
    pub fn tag(&self) -> LocalId {
        self.tag.id
    }
    pub fn path(&self) -> ResourceSumPayloadPath {
        self.path
    }
    pub fn success_site(&self) -> ResourceSite {
        self.project
    }
    pub fn failure_site(&self) -> Option<ResourceSite> {
        self.failure
    }

    fn current(&self, function: &Function) -> Result<(), String> {
        let statement = |site: ResourceSite| -> Result<&Statement, String> {
            let ResourcePosition::Statement(index) = site.position else {
                return Err("borrowed Resource Handle site is not a statement".into());
            };
            if site.function != function.id {
                return Err("borrowed Resource Handle has a foreign function".into());
            }
            function
                .blocks
                .get(site.block.index() as usize)
                .filter(|block| block.id == site.block)
                .and_then(|block| block.statements.get(index))
                .ok_or_else(|| "borrowed Resource Handle lost its exact current site".into())
        };
        for header in [
            &self.alias,
            &self.backing,
            &self.source,
            &self.output,
            &self.tag,
        ]
        .into_iter()
        .chain(self.error.iter())
        {
            if function.local(header.id) != Some(header) {
                return Err("borrowed Resource Handle changed its sealed local header".into());
            }
        }
        if self.source.mutable
            || self.output.mutable
            || self.alias.mutable
            || self.source.view_source != Some(self.backing.id)
            || self.output.view_source != Some(self.backing.id)
            || self.alias.view_source != Some(self.backing.id)
            || self.source.ty != self.backing.ty
            || self.output.ty != self.alias.ty
            || self.tag.ty != TypeInterner::BOOL
            || self.tag.view_source.is_some()
        {
            return Err("borrowed Resource Handle lost its immutable nonowning headers".into());
        }
        let initialized = statement(self.initialize)?;
        if !matches!(&initialized.kind, StatementKind::Let { local, value }
            if *local == self.source.id && value.ty == self.source.ty
            && matches!(&value.kind, E::View(inner) if inner.ty == self.source.ty && matches!(inner.kind, E::Local(backing) if backing == self.backing.id)))
            || !matches!(statement(self.observe)?.kind, StatementKind::SumTag { source, target } if source == self.source.id && target == self.tag.id)
            || !matches!(statement(self.project)?.kind, StatementKind::SumTake { source, target, success: true } if source == self.source.id && target == self.output.id)
        {
            return Err(
                "borrowed Resource Handle changed its initializer, tag or projection".into(),
            );
        }
        let branch = &function.blocks[self.observe.block.index() as usize];
        if self.observe.block != self.initialize.block
            || !matches!(&branch.terminator.kind, TerminatorKind::Branch { condition, then_block, else_block }
                if condition.ty == TypeInterner::BOOL && matches!(condition.kind, E::Local(tag) if tag == self.tag.id)
                    && *then_block == self.project.block && *else_block == self.failed && then_block != else_block)
            || !matches!(function.blocks[self.project.block.index() as usize].terminator.kind, TerminatorKind::Goto(target) if target == self.continuation)
        {
            return Err("borrowed Resource Handle lost its exact selecting CFG guard".into());
        }
        match (&self.error, self.failure) {
            (Some(error), Some(site))
                if site.block == self.failed
                    && matches!(statement(site)?.kind, StatementKind::SumTake { source, target, success: false } if source == self.source.id && target == error.id) =>
                {}
            (None, None) => {}
            _ => {
                return Err(
                    "borrowed Resource Handle lost its exact failure companion read".into(),
                );
            }
        }
        let alias_count = function.blocks.iter().flat_map(|block| &block.statements).filter(|statement| {
            matches!(&statement.kind, StatementKind::Let { local, value } if *local == self.alias.id
                && value.ty == self.alias.ty && matches!(&value.kind, E::View(inner) if inner.ty == self.output.ty && matches!(inner.kind, E::Local(output) if output == self.output.id)))
        }).count();
        let projection_count = function.blocks.iter().flat_map(|block| &block.statements).filter(|statement| {
            matches!(statement.kind, StatementKind::SumTake { target, success: true, .. } if target == self.output.id)
        }).count();
        if alias_count != 1 || projection_count != 1 {
            return Err(
                "borrowed Resource Handle has copied or missing current projections".into(),
            );
        }
        let cfg = ControlFlowGraph::analyze(function).map_err(|error| format!("{error:?}"))?;
        if !cfg.reverse_postorder().contains(&self.observe.block)
            || !cfg.reverse_postorder().contains(&self.project.block)
            || cfg.predecessors(self.project.block) != [self.observe.block]
            || cfg.predecessors(self.failed) != [self.observe.block]
        {
            return Err(
                "borrowed Resource Handle is disconnected from its exact selecting edge".into(),
            );
        }
        Ok(())
    }
    fn remap_blocks(&mut self, map: &[Option<BlockId>]) -> Result<(), String> {
        let remap = |id: &mut BlockId| -> Result<(), String> {
            *id =
                map.get(id.index() as usize).copied().flatten().ok_or(
                    "borrowed Resource Handle canonical map removed an authenticated edge",
                )?;
            Ok(())
        };
        for site in [&mut self.initialize, &mut self.observe, &mut self.project]
            .into_iter()
            .chain(self.failure.iter_mut())
        {
            remap(&mut site.block)?;
        }
        remap(&mut self.failed)?;
        remap(&mut self.continuation)
    }
    fn remap_locals(&mut self, remap: &mut impl FnMut(&mut LocalId)) {
        for header in [
            &mut self.alias,
            &mut self.backing,
            &mut self.source,
            &mut self.output,
            &mut self.tag,
        ]
        .into_iter()
        .chain(self.error.iter_mut())
        {
            remap(&mut header.id);
            if let Some(source) = &mut header.view_source {
                remap(source);
            }
        }
    }
}

pub(crate) struct BorrowedSumSeed {
    pub(crate) original: Expression,
    pub(crate) alias: Local,
    pub(crate) backing: Local,
    pub(crate) path: ResourceSumPayloadPath,
}
impl Capture {
    pub(crate) fn borrowed_sum_seed(
        &self,
        expression: &Expression,
        types: &TypeInterner,
    ) -> Result<Option<BorrowedSumSeed>, String> {
        let Some(witness) = &self.witness else {
            return Ok(None);
        };
        if !resource_type_pending(types, expression.ty) {
            return Ok(None);
        }
        let mut found = None;
        for statement in &witness.original.body.statements {
            let hir::StatementKind::Let { local, value } = &statement.kind else {
                continue;
            };
            if !crate::breakpoint_regions::expressions_equal(value, expression) {
                continue;
            }
            let alias = witness
                .original
                .locals
                .get(local.index() as usize)
                .filter(|header| header.id == *local)
                .ok_or("borrowed Resource Handle original alias is absent")?;
            let Some(source) = alias.view_source else {
                continue;
            };
            let backing = witness
                .original
                .locals
                .get(source.index() as usize)
                .filter(|header| header.id == source)
                .ok_or("borrowed Resource Handle original backing is absent")?;
            let Some(handle) =
                hir::borrowed_sum_view_initializer(value, source, backing.ty, alias.ty, types)?
            else {
                continue;
            };
            let E::Handle { kind, .. } = &handle.kind else {
                unreachable!()
            };
            let path = match kind {
                hir::HandleKind::Optional => ResourceSumPayloadPath::OptionalSome,
                hir::HandleKind::Result => ResourceSumPayloadPath::ResultOk,
                _ => return Err("borrowed Resource Handle has an unsupported sum kind".into()),
            };
            if found.is_some() || alias.mutable {
                return Err(
                    "borrowed Resource Handle has no unique immutable original binding".into(),
                );
            }
            found = Some(BorrowedSumSeed {
                original: value.clone(),
                alias: alias.clone(),
                backing: backing.clone(),
                path,
            });
        }
        Ok(found)
    }
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn record_borrowed_sum(
        &mut self,
        seed: BorrowedSumSeed,
        source: Local,
        output: Local,
        tag: Local,
        error: Option<Local>,
        initialize: ResourceSite,
        observe: ResourceSite,
        project: ResourceSite,
        failure: Option<ResourceSite>,
        failed: BlockId,
        continuation: BlockId,
    ) {
        if let Some(witness) = &mut self.witness {
            witness.borrowed_sums.push(ResourceBorrowedSumProjection {
                original: seed.original,
                original_alias: seed.alias.clone(),
                original_backing: seed.backing.clone(),
                alias: seed.alias,
                backing: seed.backing,
                source,
                output,
                tag,
                error,
                initialize,
                observe,
                project,
                failure,
                failed,
                continuation,
                path: seed.path,
            });
        }
    }
}
pub(super) fn current(
    witness: &ResourceLoweringWitness,
    function: &Function,
) -> Result<(), String> {
    let expected = witness.original.body.statements.iter().filter(|statement| {
        matches!(&statement.kind, hir::StatementKind::Let { value, .. }
            if witness.manifest.kind_for_type(value.ty).is_some()
                && matches!(&value.kind, E::View(handle) if matches!(handle.kind, E::Handle { .. })))
    }).count();
    if expected != witness.borrowed_sums.len() {
        return Err(
            "borrowed Resource Handle lost its complete archived projection inventory".into(),
        );
    }
    for (index, row) in witness.borrowed_sums.iter().enumerate() {
        row.current(function)?;
        if witness.borrowed_sums[..index]
            .iter()
            .any(|old| old.original_alias.id == row.original_alias.id)
        {
            return Err("borrowed Resource Handle original binding is duplicated".into());
        }
        let count = witness.original.body.statements.iter().filter(|statement| {
            matches!(&statement.kind, hir::StatementKind::Let { local, value } if *local == row.original_alias.id && crate::breakpoint_regions::expressions_equal(value, &row.original))
        }).count();
        if count != 1
            || witness
                .original
                .locals
                .get(row.original_alias.id.index() as usize)
                != Some(&row.original_alias)
            || witness
                .original
                .locals
                .get(row.original_backing.id.index() as usize)
                != Some(&row.original_backing)
        {
            return Err("borrowed Resource Handle lost its exact archived original binding".into());
        }
    }
    Ok(())
}
pub(super) fn remap_blocks(
    rows: &mut [ResourceBorrowedSumProjection],
    map: &[Option<BlockId>],
) -> Result<(), String> {
    for row in rows {
        row.remap_blocks(map)?;
    }
    Ok(())
}
pub(super) fn remap_locals(
    rows: &mut [ResourceBorrowedSumProjection],
    remap: &mut impl FnMut(&mut LocalId),
) {
    for row in rows {
        row.remap_locals(remap);
    }
}
impl Function {
    /// Readonly exact current statement projection; public copied sum nodes grant nothing.
    pub fn resource_borrowed_sum_projection(
        &self,
        block: BlockId,
        position: ResourcePosition,
    ) -> Result<Option<&ResourceBorrowedSumProjection>, String> {
        let Some(witness) = &self.resource_lowering else {
            return Ok(None);
        };
        witness.current(self)?;
        let site = ResourceSite {
            function: self.id,
            block,
            position,
        };
        Ok(witness
            .borrowed_sums
            .iter()
            .find(|row| row.observe == site || row.project == site || row.failure == Some(site)))
    }
    pub(crate) fn resource_borrowed_sum_alias(
        &self,
        local: LocalId,
        value: &Expression,
    ) -> Result<bool, String> {
        let Some(witness) = &self.resource_lowering else {
            return Ok(false);
        };
        witness.current(self)?;
        Ok(witness.borrowed_sums.iter().any(|row| row.alias.id == local && matches!(&value.kind, E::View(inner) if matches!(inner.kind, E::Local(output) if output == row.output.id))
            && self.blocks.iter().flat_map(|block| &block.statements).any(|statement| matches!(&statement.kind, StatementKind::Let { local: defined, value: current } if *defined == local && std::ptr::eq(current, value)))))
    }
}
