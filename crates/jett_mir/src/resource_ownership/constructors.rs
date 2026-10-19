//! Constructor-issued struct operation regions, separate from Source call authority.
use super::*;
use jett_hir::ExpressionKind;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ResourceConstructorId(usize);
impl ResourceConstructorId {
    pub fn index(self) -> usize {
        self.0
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum ResourceConstructorNode {
    Begin {
        constructor: ResourceConstructorId,
    },
    Stage {
        constructor: ResourceConstructorId,
        source_index: usize,
        field: usize,
        value: Expression,
    },
    Commit {
        constructor: ResourceConstructorId,
        output: LocalId,
    },
    End {
        constructor: ResourceConstructorId,
        outcome: ResourceCompletion,
    },
}
impl ResourceConstructorNode {
    pub fn constructor(&self) -> ResourceConstructorId {
        match self {
            Self::Begin { constructor }
            | Self::Stage { constructor, .. }
            | Self::Commit { constructor, .. }
            | Self::End { constructor, .. } => *constructor,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ResourceOperationRegion {
    Call(ResourceCallRegionId),
    Constructor(ResourceConstructorId),
}

#[derive(Debug, Clone, PartialEq)]
pub struct ResourceConstructorField {
    source_index: usize,
    field: usize,
    original: Expression,
    endpoint: Expression,
    start: ResourceSite,
    stage: ResourceSite,
    locals: Vec<Local>,
}
impl ResourceConstructorField {
    pub fn source_index(&self) -> usize {
        self.source_index
    }
    pub fn field(&self) -> usize {
        self.field
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
}

#[derive(Debug, Clone, PartialEq)]
pub struct ResourceConstructorRegion {
    id: ResourceConstructorId,
    original: Expression,
    order: Vec<usize>,
    field_types: Vec<TypeId>,
    parent: Option<ResourceOperationRegion>,
    begin: ResourceSite,
    fields: Vec<ResourceConstructorField>,
    commit: Option<(ResourceSite, Local)>,
    exits: Vec<(ResourceSite, ResourceCompletion)>,
}
impl ResourceConstructorRegion {
    pub fn id(&self) -> ResourceConstructorId {
        self.id
    }
    pub fn original(&self) -> &Expression {
        &self.original
    }
    pub fn evaluation_order(&self) -> &[usize] {
        &self.order
    }
    pub fn field_types(&self) -> &[TypeId] {
        &self.field_types
    }
    pub(crate) fn parent(&self) -> Option<ResourceOperationRegion> {
        self.parent
    }
    pub fn begin(&self) -> ResourceSite {
        self.begin
    }
    pub fn fields(&self) -> &[ResourceConstructorField] {
        &self.fields
    }
    pub fn commit(&self) -> Option<(ResourceSite, &Local)> {
        self.commit.as_ref().map(|(s, l)| (*s, l))
    }
    pub fn exits(&self) -> &[(ResourceSite, ResourceCompletion)] {
        &self.exits
    }

    pub(super) fn current(&self, function: &Function) -> Result<(), String> {
        let ExpressionKind::StructConstruct {
            fields,
            evaluation_order,
            struct_type,
            ..
        } = &self.original.kind
        else {
            return Err("Resource constructor lost its original nominal expression".into());
        };
        if *struct_type != self.original.ty
            || *evaluation_order != self.order
            || fields.len() != self.field_types.len()
            || fields
                .iter()
                .zip(&self.field_types)
                .any(|(field, ty)| field.ty != *ty)
            || self.fields.iter().any(|field| {
                fields.get(field.field).is_none_or(|original| {
                    !crate::breakpoint_regions::expressions_equal(original, &field.original)
                })
            })
        {
            return Err("Resource constructor changed its exact original field relation".into());
        }
        let node = |site: ResourceSite| -> Result<&ResourceConstructorNode, String> {
            let ResourcePosition::Statement(index) = site.position else {
                return Err("Resource constructor lost its exact statement site".into());
            };
            let statement = function
                .blocks
                .get(site.block.index() as usize)
                .filter(|b| b.id == site.block)
                .and_then(|b| b.statements.get(index))
                .ok_or("Resource constructor statement disappeared")?;
            let StatementKind::ResourceConstructor(node) = &statement.kind else {
                return Err("Resource constructor statement changed its node family".into());
            };
            if site.function != function.id || node.constructor() != self.id {
                return Err("Resource constructor has a foreign occurrence ID".into());
            }
            Ok(node)
        };
        if !matches!(node(self.begin)?, ResourceConstructorNode::Begin { .. })
            || self.fields.len() != self.order.len()
        {
            return Err("Resource constructor lost its complete original field tuple".into());
        }
        for (source_index, field) in self.fields.iter().enumerate() {
            if field.source_index != source_index
                || self.order.get(source_index) != Some(&field.field)
                || self.field_types.get(field.field) != Some(&field.endpoint.ty)
                || !matches!(node(field.stage)?, ResourceConstructorNode::Stage { source_index:i, field:f, value, .. }
                    if *i == source_index && *f == field.field && crate::breakpoint_regions::expressions_equal(value, &field.endpoint))
                || field.locals.iter().any(|l| function.local(l.id) != Some(l))
            {
                return Err(
                    "Resource constructor changed an exact field endpoint or generated header"
                        .into(),
                );
            }
        }
        let (site, output) = self
            .commit
            .as_ref()
            .ok_or("Resource constructor has no exact Commit")?;
        if function.local(output.id) != Some(output)
            || output.ty != self.original.ty
            || !output.mutable
            || output.view_source.is_some()
            || !matches!(node(*site)?, ResourceConstructorNode::Commit { output:id, .. } if *id == output.id)
        {
            return Err("Resource constructor Commit changed its owned output header".into());
        }
        for (site, outcome) in &self.exits {
            if !matches!(node(*site)?, ResourceConstructorNode::End { outcome:found, .. } if found == outcome)
            {
                return Err("Resource constructor lost its exact completion occurrence".into());
            }
        }
        if self
            .exits
            .iter()
            .filter(|(_, o)| *o == ResourceCompletion::Normal)
            .count()
            != 1
            || function
                .blocks
                .iter()
                .flat_map(|b| &b.statements)
                .filter(|s| {
                    matches!(&s.kind,
                StatementKind::ResourceConstructor(n) if n.constructor() == self.id)
                })
                .count()
                != self.fields.len() + self.exits.len() + 2
        {
            return Err("Resource constructor has missing or copied operation nodes".into());
        }
        Ok(())
    }

    pub(super) fn remap_blocks(&mut self, map: &[Option<BlockId>]) -> Result<(), String> {
        let remap = |site: &mut ResourceSite| -> Result<(), String> {
            site.block = map
                .get(site.block.index() as usize)
                .copied()
                .flatten()
                .ok_or("canonical selection removed a sealed Resource constructor endpoint")?;
            Ok(())
        };
        remap(&mut self.begin)?;
        for field in &mut self.fields {
            remap(&mut field.start)?;
            remap(&mut field.stage)?;
        }
        if let Some((site, _)) = &mut self.commit {
            remap(site)?;
        }
        for (site, _) in &mut self.exits {
            remap(site)?;
        }
        Ok(())
    }

    pub(super) fn remap_locals(&mut self, remap: &mut impl FnMut(&mut LocalId)) {
        for field in &mut self.fields {
            for header in &mut field.locals {
                remap(&mut header.id);
                if let Some(s) = &mut header.view_source {
                    remap(s);
                }
            }
            let mut wrapper = BasicBlock {
                id: BlockId(0),
                statements: vec![Statement {
                    kind: StatementKind::Evaluate(field.endpoint.clone()),
                    span: field.endpoint.span,
                }],
                terminator: Terminator {
                    kind: TerminatorKind::Unreachable,
                    span: field.endpoint.span,
                },
            };
            crate::sequences::prune::block_locals(&mut wrapper, remap, &mut |_| {});
            if let StatementKind::Evaluate(value) = wrapper.statements.remove(0).kind {
                field.endpoint = value;
            }
        }
        if let Some((_, output)) = &mut self.commit {
            remap(&mut output.id);
        }
    }

    pub(super) fn shift_prefix(&mut self, block: BlockId, count: usize) {
        let shift = |site: &mut ResourceSite| {
            if site.block == block
                && let ResourcePosition::Statement(i) = &mut site.position
            {
                *i += count;
            }
        };
        shift(&mut self.begin);
        for f in &mut self.fields {
            shift(&mut f.start);
            shift(&mut f.stage);
        }
        if let Some((site, _)) = &mut self.commit {
            shift(site);
        }
        for (site, _) in &mut self.exits {
            shift(site);
        }
    }
}

pub(super) fn records_equal(
    a: &[ResourceConstructorRegion],
    b: &[ResourceConstructorRegion],
) -> bool {
    a.len() == b.len()
        && a.iter().zip(b).all(|(a, b)| {
            a.id == b.id
                && crate::breakpoint_regions::expressions_equal(&a.original, &b.original)
                && a.order == b.order
                && a.field_types == b.field_types
                && a.parent == b.parent
                && a.begin == b.begin
                && a.commit == b.commit
                && a.exits == b.exits
                && a.fields.len() == b.fields.len()
                && a.fields.iter().zip(&b.fields).all(|(a, b)| {
                    a.source_index == b.source_index
                        && a.field == b.field
                        && a.start == b.start
                        && a.stage == b.stage
                        && a.locals == b.locals
                        && crate::breakpoint_regions::expressions_equal(&a.original, &b.original)
                        && crate::breakpoint_regions::expressions_equal(&a.endpoint, &b.endpoint)
                })
        })
}

impl Capture {
    pub(crate) fn constructor_needed(&self, value: &Expression, types: &TypeInterner) -> bool {
        self.witness.is_some()
            && matches!(&value.kind, ExpressionKind::StructConstruct { fields, .. }
            if fields.iter().any(|v| resource_type_pending(types, v.ty)))
    }

    pub(crate) fn begin_constructor_region(
        &mut self,
        original: &Expression,
        parent: Option<ResourceOperationRegion>,
        begin: ResourceSite,
        types: &TypeInterner,
    ) -> Result<ResourceConstructorId, String> {
        let w = self
            .witness
            .as_mut()
            .ok_or("Resource constructor lacks checked Source authentication")?;
        w.source.validate_types(types)?;
        let ExpressionKind::StructConstruct {
            struct_type,
            fields,
            evaluation_order,
            validates_refinements: false,
            refinement_predicates,
            ..
        } = &original.kind
        else {
            return Err(
                "occupied refined constructor needs its exact checked predicate transition".into(),
            );
        };
        if !refinement_predicates.is_empty() || original.ty != *struct_type {
            return Err("Resource constructor changed its exact nominal type".into());
        }
        let Type::Struct(id) = types.resolve(*struct_type) else {
            return Err("Resource constructor lacks exact struct header".into());
        };
        let field_types = types
            .resolve_struct(*id)
            .fields
            .iter()
            .map(|(_, t)| *t)
            .collect::<Vec<_>>();
        let mut occurrences = 0;
        walk::hir_block(&w.original.body, &mut |v| {
            occurrences += usize::from(crate::breakpoint_regions::expressions_equal(v, original))
        });
        if occurrences != 1
            || fields.len() != field_types.len()
            || fields.iter().zip(&field_types).any(|(f, t)| f.ty != *t)
            || evaluation_order.len() != fields.len()
            || evaluation_order.iter().copied().collect::<BTreeSet<_>>()
                != (0..fields.len()).collect()
            || w.constructors
                .iter()
                .any(|r| crate::breakpoint_regions::expressions_equal(&r.original, original))
        {
            return Err(
                "Resource constructor differs from its unique complete original checked struct"
                    .into(),
            );
        }
        let id = ResourceConstructorId(w.constructors.len());
        if matches!(parent, Some(ResourceOperationRegion::Constructor(p)) if p.index() >= id.index())
        {
            return Err("Resource constructor has no private parent region".into());
        }
        w.constructors.push(ResourceConstructorRegion {
            id,
            original: original.clone(),
            order: evaluation_order.clone(),
            field_types,
            parent,
            begin,
            fields: vec![],
            commit: None,
            exits: vec![],
        });
        Ok(id)
    }

    pub(crate) fn constructor_field(
        &mut self,
        id: ResourceConstructorId,
        source_index: usize,
        field: usize,
        original: &Expression,
        endpoint: &Expression,
        start: ResourceSite,
        stage: ResourceSite,
        locals: &[Local],
    ) -> Result<(), String> {
        let r = self
            .witness
            .as_mut()
            .and_then(|w| w.constructors.get_mut(id.index()))
            .ok_or("Resource field lost its private constructor")?;
        if r.fields.len() != source_index
            || r.order.get(source_index) != Some(&field)
            || r.field_types.get(field) != Some(&endpoint.ty)
        {
            return Err("Resource field changed its original lexical occurrence/type".into());
        }
        r.fields.push(ResourceConstructorField {
            source_index,
            field,
            original: original.clone(),
            endpoint: endpoint.clone(),
            start,
            stage,
            locals: locals.to_vec(),
        });
        Ok(())
    }

    pub(crate) fn constructor_node(
        &mut self,
        site: ResourceSite,
        node: &ResourceConstructorNode,
        locals: &[Local],
    ) {
        let Some(r) = self
            .witness
            .as_mut()
            .and_then(|w| w.constructors.get_mut(node.constructor().index()))
        else {
            return;
        };
        match node {
            ResourceConstructorNode::Commit { output, .. } => {
                if let Some(h) = locals
                    .get(output.index() as usize)
                    .filter(|h| h.id == *output)
                {
                    r.commit = Some((site, h.clone()));
                }
            }
            ResourceConstructorNode::End { outcome, .. } => r.exits.push((site, *outcome)),
            _ => {}
        }
    }
}

impl Function {
    pub fn resource_constructor(
        &self,
        id: ResourceConstructorId,
    ) -> Result<&ResourceConstructorRegion, String> {
        let w = self
            .resource_lowering
            .as_ref()
            .ok_or("Resource constructor lost original checked witness")?;
        w.current(self)?;
        w.constructors
            .get(id.index())
            .filter(|r| r.id == id)
            .ok_or_else(|| "Resource constructor has a foreign private occurrence".into())
    }
}

pub(crate) fn validate_nodes(function: &Function) -> Result<(), String> {
    let has_nodes = function.blocks.iter().any(|block| {
        block
            .statements
            .iter()
            .any(|statement| matches!(statement.kind, StatementKind::ResourceConstructor(_)))
    });
    let has_inventory = function.resource_lowering.as_ref().is_some_and(|witness| {
        !witness.constructors.is_empty() || !witness.constructor_seal.is_empty()
    });
    if !has_nodes && !has_inventory {
        return Ok(());
    }
    function
        .resource_lowering
        .as_ref()
        .ok_or("Resource constructor canonical preparation lost its original checked witness")?
        .current(function)
}

#[cfg(test)]
mod tests;
