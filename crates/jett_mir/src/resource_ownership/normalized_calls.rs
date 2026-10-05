//! Constructor-only authority for a Source call whose actuals span CFG blocks.
use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ResourceCallRegionId(usize);
impl ResourceCallRegionId {
    pub fn index(self) -> usize {
        self.0
    }
}

/// Public syntax is inert without the exact private constructor witness.
#[derive(Debug, Clone, PartialEq)]
pub enum ResourceCallNode {
    Begin {
        region: ResourceCallRegionId,
    },
    Stage {
        region: ResourceCallRegionId,
        source_index: usize,
        parameter: usize,
        value: Expression,
        ordinary: Option<LocalId>,
    },
    Invoke {
        region: ResourceCallRegionId,
        output: LocalId,
    },
    End {
        region: ResourceCallRegionId,
        outcome: ResourceCompletion,
    },
}
impl ResourceCallNode {
    pub fn region(&self) -> ResourceCallRegionId {
        match self {
            Self::Begin { region }
            | Self::Stage { region, .. }
            | Self::Invoke { region, .. }
            | Self::End { region, .. } => *region,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ResourceCallActual {
    source_index: usize,
    parameter: usize,
    original: Expression,
    endpoint: Expression,
    start: ResourceSite,
    stage: ResourceSite,
    locals: Vec<Local>,
    ordinary: Option<LocalId>,
}
impl ResourceCallActual {
    pub fn source_index(&self) -> usize {
        self.source_index
    }
    pub fn parameter(&self) -> usize {
        self.parameter
    }
    pub fn original(&self) -> &Expression {
        &self.original
    }
    pub fn endpoint(&self) -> &Expression {
        &self.endpoint
    }
    pub fn start(&self) -> ResourceSite {
        self.start
    }
    pub fn stage(&self) -> ResourceSite {
        self.stage
    }
    pub fn locals(&self) -> &[Local] {
        &self.locals
    }
    pub fn ordinary(&self) -> Option<LocalId> {
        self.ordinary
    }
}

/// Archival originals and current emitted endpoints intentionally have separate IDs.
#[derive(Debug, Clone, PartialEq)]
pub struct ResourceCallRegion {
    id: ResourceCallRegionId,
    original: Expression,
    source: hir::SourceCallOwnership,
    function: FunctionId,
    order: Vec<usize>,
    parent: Option<ResourceCallRegionId>,
    begin: ResourceSite,
    actuals: Vec<ResourceCallActual>,
    invocation: Option<(ResourceSite, LocalId)>,
    exits: Vec<(ResourceSite, ResourceCompletion)>,
}
impl ResourceCallRegion {
    pub fn id(&self) -> ResourceCallRegionId {
        self.id
    }
    pub fn original(&self) -> &Expression {
        &self.original
    }
    pub fn source(&self) -> &hir::SourceCallOwnership {
        &self.source
    }
    pub fn function(&self) -> FunctionId {
        self.function
    }
    pub fn evaluation_order(&self) -> &[usize] {
        &self.order
    }
    pub fn parent(&self) -> Option<ResourceCallRegionId> {
        self.parent
    }
    pub fn begin(&self) -> ResourceSite {
        self.begin
    }
    pub fn actuals(&self) -> &[ResourceCallActual] {
        &self.actuals
    }
    pub fn invocation(&self) -> Option<(ResourceSite, LocalId)> {
        self.invocation
    }
    pub fn output(&self) -> Option<LocalId> {
        self.invocation.map(|(_, output)| output)
    }
    pub fn ordinary(&self, parameter: usize) -> Option<LocalId> {
        self.actuals
            .iter()
            .find(|actual| actual.parameter == parameter)
            .and_then(|actual| actual.ordinary)
    }
    pub fn exits(&self) -> &[(ResourceSite, ResourceCompletion)] {
        &self.exits
    }
    pub(super) fn same(&self, other: &Self) -> bool {
        self.id == other.id
            && self.source == other.source
            && self.function == other.function
            && self.order == other.order
            && self.parent == other.parent
            && self.begin == other.begin
            && self.invocation == other.invocation
            && self.exits == other.exits
            && crate::breakpoint_regions::expressions_equal(&self.original, &other.original)
            && self.actuals.len() == other.actuals.len()
            && self.actuals.iter().zip(&other.actuals).all(|(a, b)| {
                a.source_index == b.source_index
                    && a.parameter == b.parameter
                    && a.start == b.start
                    && a.stage == b.stage
                    && a.locals == b.locals
                    && a.ordinary == b.ordinary
                    && crate::breakpoint_regions::expressions_equal(&a.original, &b.original)
                    && crate::breakpoint_regions::expressions_equal(&a.endpoint, &b.endpoint)
            })
    }
    pub(super) fn current(&self, function: &Function) -> Result<(), String> {
        let node = |site: ResourceSite| -> Result<&ResourceCallNode, String> {
            let ResourcePosition::Statement(index) = site.position else {
                return Err("Resource call region site is not a statement".into());
            };
            let statement = function
                .blocks
                .get(site.block.index() as usize)
                .filter(|block| block.id == site.block)
                .and_then(|block| block.statements.get(index))
                .ok_or("Resource call region site is missing")?;
            let StatementKind::ResourceCall(node) = &statement.kind else {
                return Err("Resource call region site changed its typed node".into());
            };
            if site.function != function.id || node.region() != self.id {
                return Err("Resource call region changed its function or opaque reference".into());
            }
            Ok(node)
        };
        if !matches!(node(self.begin)?, ResourceCallNode::Begin { .. })
            || self.order.len() != self.source.arguments.len()
            || self.actuals.len() != self.order.len()
        {
            return Err(
                "Resource call region lost its exact begin or complete Source tuple".into(),
            );
        }
        for (source_index, actual) in self.actuals.iter().enumerate() {
            if actual.source_index != source_index
                || self.order[source_index] != actual.parameter
                || !matches!(node(actual.stage)?, ResourceCallNode::Stage { source_index: index, parameter, value, ordinary, .. }
                    if *index == source_index && *parameter == actual.parameter && *ordinary == actual.ordinary
                        && crate::breakpoint_regions::expressions_equal(value, &actual.endpoint))
                || actual
                    .locals
                    .iter()
                    .any(|header| function.local(header.id) != Some(header))
            {
                return Err(
                    "Resource call actual lost its lexical endpoint or generated headers".into(),
                );
            }
        }
        let (site, output) = self
            .invocation
            .ok_or("Resource call region has no exact invocation")?;
        if !matches!(node(site)?, ResourceCallNode::Invoke { output: found, .. } if *found == output)
            || function.local(output).map(|local| local.ty) != Some(self.original.ty)
        {
            return Err("Resource call region changed its invocation output".into());
        }
        for (site, outcome) in &self.exits {
            if !matches!(node(*site)?, ResourceCallNode::End { outcome: found, .. } if found == outcome)
            {
                return Err("Resource call region lost an exact completion site".into());
            }
        }
        let count = function.blocks.iter().flat_map(|block| &block.statements)
            .filter(|statement| matches!(&statement.kind, StatementKind::ResourceCall(value) if value.region() == self.id)).count();
        if count != 2 + self.actuals.len() + self.exits.len()
            || !self
                .exits
                .iter()
                .any(|(_, outcome)| *outcome == ResourceCompletion::Normal)
        {
            return Err(
                "Resource call region has copied, disconnected or missing typed nodes".into(),
            );
        }
        Ok(())
    }
    pub(super) fn remap_blocks(&mut self, map: &[Option<BlockId>]) -> Result<(), String> {
        let remap = |site: &mut ResourceSite| -> Result<(), String> {
            site.block = map.get(site.block.index() as usize).copied().flatten()
                .ok_or("pending Resource call region: canonical selection removed a sealed evaluation or completion site")?;
            Ok(())
        };
        remap(&mut self.begin)?;
        for actual in &mut self.actuals {
            remap(&mut actual.start)?;
            remap(&mut actual.stage)?;
        }
        if let Some((site, _)) = &mut self.invocation {
            remap(site)?;
        }
        for (site, _) in &mut self.exits {
            remap(site)?;
        }
        Ok(())
    }
    pub(super) fn remap_locals(&mut self, remap: &mut impl FnMut(&mut LocalId)) {
        for actual in &mut self.actuals {
            for header in &mut actual.locals {
                remap(&mut header.id);
                if let Some(source) = &mut header.view_source {
                    remap(source);
                }
            }
            if let Some(local) = &mut actual.ordinary {
                remap(local);
            }
            let mut wrapper = BasicBlock {
                id: BlockId(0),
                statements: vec![Statement {
                    kind: StatementKind::Evaluate(actual.endpoint.clone()),
                    span: actual.endpoint.span,
                }],
                terminator: Terminator {
                    kind: TerminatorKind::Unreachable,
                    span: actual.endpoint.span,
                },
            };
            crate::sequences::prune::block_locals(&mut wrapper, remap, &mut |_| {});
            if let StatementKind::Evaluate(endpoint) = wrapper.statements.remove(0).kind {
                actual.endpoint = endpoint;
            }
        }
        if let Some((_, output)) = &mut self.invocation {
            remap(output);
        }
    }
}

impl Capture {
    pub(crate) fn normalized_call_needed(&self, value: &Expression, types: &TypeInterner) -> bool {
        self.witness.as_ref().is_some_and(|witness| {
            source_custody_call(value, types, &witness.execution) && {
                let mut handled = false;
                walk::expression(value, &mut |node| {
                    handled |= matches!(node.kind, hir::ExpressionKind::Handle { .. })
                });
                handled
            }
        })
    }
    pub(crate) fn begin_normalized_call(
        &mut self,
        original: &Expression,
        parent: Option<ResourceCallRegionId>,
        begin: ResourceSite,
        types: &TypeInterner,
    ) -> Result<ResourceCallRegionId, String> {
        let witness = self
            .witness
            .as_mut()
            .ok_or("Resource call region lacks initial Source authentication")?;
        witness.source.validate_types(types)?;
        let hir::ExpressionKind::Call {
            function,
            args,
            evaluation_order,
            ownership: hir::CallOwnership::Source(source),
        } = &original.kind
        else {
            return Err("pending Resource call region: indirect/hook handled custody calls require their separate invocation proof".into());
        };
        let mut occurrences = 0;
        walk::hir_block(&witness.original.body, &mut |value| {
            occurrences += usize::from(crate::breakpoint_regions::expressions_equal(
                value, original,
            ))
        });
        if occurrences != 1
            || source.arguments.len() != args.len()
            || source
                .arguments
                .iter()
                .any(|fact| fact.staging != hir::ArgumentStaging::Original)
            || evaluation_order.len() != args.len()
            || evaluation_order
                .iter()
                .copied()
                .collect::<BTreeSet<_>>()
                .len()
                != args.len()
            || evaluation_order
                .iter()
                .any(|parameter| *parameter >= args.len())
        {
            return Err(
                "Resource call region differs from its unique complete original Source call".into(),
            );
        }
        let id = ResourceCallRegionId(witness.regions.len());
        if parent.is_some_and(|parent| parent.index() >= id.index()) {
            return Err("Resource call region has no constructor-owned parent".into());
        }
        witness.regions.push(ResourceCallRegion {
            id,
            original: original.clone(),
            source: source.clone(),
            function: *function,
            order: evaluation_order.clone(),
            parent,
            begin,
            actuals: Vec::new(),
            invocation: None,
            exits: Vec::new(),
        });
        Ok(id)
    }
    pub(crate) fn normalized_actual(
        &mut self,
        region: ResourceCallRegionId,
        source_index: usize,
        parameter: usize,
        original: &Expression,
        endpoint: &Expression,
        start: ResourceSite,
        stage: ResourceSite,
        locals: &[Local],
        ordinary: Option<LocalId>,
    ) -> Result<(), String> {
        let record = self
            .witness
            .as_mut()
            .and_then(|witness| witness.regions.get_mut(region.index()))
            .ok_or("Resource actual lost its constructor region")?;
        if record.actuals.len() != source_index
            || record.order.get(source_index) != Some(&parameter)
        {
            return Err("Resource actual changed its exact lexical occurrence".into());
        }
        record.actuals.push(ResourceCallActual {
            source_index,
            parameter,
            original: original.clone(),
            endpoint: endpoint.clone(),
            start,
            stage,
            locals: locals.to_vec(),
            ordinary,
        });
        Ok(())
    }
    pub(crate) fn normalized_node(&mut self, site: ResourceSite, node: &ResourceCallNode) {
        let Some(record) = self
            .witness
            .as_mut()
            .and_then(|witness| witness.regions.get_mut(node.region().index()))
        else {
            return;
        };
        match node {
            ResourceCallNode::Invoke { output, .. } => record.invocation = Some((site, *output)),
            ResourceCallNode::End { outcome, .. } => record.exits.push((site, *outcome)),
            ResourceCallNode::Begin { .. } | ResourceCallNode::Stage { .. } => {}
        }
    }
}

impl Function {
    /// Validity is granted only by fresh whole-program ownership validation.
    pub fn resource_call_region(&self, id: ResourceCallRegionId) -> Option<&ResourceCallRegion> {
        self.resource_lowering
            .as_ref()?
            .regions
            .get(id.index())
            .filter(|region| region.id == id)
    }
}
