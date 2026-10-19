//! Ordinary owned metadata rooted in protected checked actor graphs.
//! This proof selects storage only; invocation and Scope remain separate proofs.
use super::*;
use hir::ExpressionKind as E;

mod current;
mod original;

#[derive(Debug, Clone)]
enum Shape {
    Plain {
        plan: hir::ActorGraphPlan,
        value: Expression,
    },
    Hook(hir::ActorGraphInstruction),
    HiddenRootToken {
        fill: hir::ActorGraphInstruction,
        root: Box<GraphStorageProof>,
    },
    Actor {
        plan: hir::ActorGraphPlan,
        node: usize,
    },
    Fields(Vec<Option<GraphStorageProof>>),
    Items(Vec<Option<GraphStorageProof>>),
    Entries(Vec<(Option<GraphStorageProof>, Option<GraphStorageProof>)>),
    Some(Box<GraphStorageProof>),
    Ok(Box<GraphStorageProof>),
    Fail(Box<GraphStorageProof>),
    Alternatives(Vec<GraphStorageProof>),
}
impl PartialEq for Shape {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (
                Self::Plain {
                    plan: left,
                    value: a,
                },
                Self::Plain {
                    plan: right,
                    value: b,
                },
            ) => left == right && equal(a, b),
            (Self::Hook(a), Self::Hook(b)) => a == b,
            (
                Self::HiddenRootToken {
                    fill: a,
                    root: left,
                },
                Self::HiddenRootToken {
                    fill: b,
                    root: right,
                },
            ) => a == b && left == right,
            (
                Self::Actor {
                    plan: a,
                    node: left,
                },
                Self::Actor {
                    plan: b,
                    node: right,
                },
            ) => a == b && left == right,
            (Self::Fields(a), Self::Fields(b)) | (Self::Items(a), Self::Items(b)) => a == b,
            (Self::Entries(a), Self::Entries(b)) => a == b,
            (Self::Some(a), Self::Some(b))
            | (Self::Ok(a), Self::Ok(b))
            | (Self::Fail(a), Self::Fail(b)) => a == b,
            (Self::Alternatives(a), Self::Alternatives(b)) => a == b,
            _ => false,
        }
    }
}

/// No public constructor. The type is a projection of a protected graph row.
#[derive(Debug, Clone, PartialEq)]
pub struct GraphStorageProof {
    ty: TypeId,
    shape: Shape,
}
impl GraphStorageProof {
    pub fn ty(&self) -> TypeId {
        self.ty
    }
    fn transferable(&self) -> bool {
        !matches!(self.shape, Shape::HiddenRootToken { .. })
    }
}

/// An owned graph hook box, never a scalar returned descriptor or Scope grant.
#[derive(Debug, Clone, PartialEq)]
pub struct GraphHookProjectionProof {
    ty: TypeId,
    hook: hir::ResourceHookRef,
    producers: Vec<hir::ActorGraphInstruction>,
}
impl GraphHookProjectionProof {
    pub fn ty(&self) -> TypeId {
        self.ty
    }
    pub fn hook(&self) -> &hir::ResourceHookRef {
        &self.hook
    }
    pub fn producers(&self) -> &[hir::ActorGraphInstruction] {
        &self.producers
    }
}

#[derive(Debug, Clone)]
struct ValueRow {
    original: Expression,
    current: Expression,
    proof: GraphStorageProof,
}
impl PartialEq for ValueRow {
    fn eq(&self, other: &Self) -> bool {
        equal(&self.original, &other.original)
            && equal(&self.current, &other.current)
            && self.proof == other.proof
    }
}
#[derive(Debug, Clone, PartialEq)]
struct HeaderRow {
    original: Local,
    current: Local,
    proof: GraphStorageProof,
}
#[derive(Debug, Clone, PartialEq)]
struct FormalRow {
    original: Param,
    current: Param,
    proof: GraphStorageProof,
}
/// Full original/current Source call and exact formal header; no tuple projection
/// is accepted as a replacement for either whole call.
#[derive(Debug, Clone)]
pub struct GraphSourceEdgeProof {
    original_call: Expression,
    current_call: Expression,
    parameter: usize,
    formal: Param,
    actual: Expression,
    proof: GraphStorageProof,
}
impl PartialEq for GraphSourceEdgeProof {
    fn eq(&self, other: &Self) -> bool {
        equal(&self.original_call, &other.original_call)
            && equal(&self.current_call, &other.current_call)
            && self.parameter == other.parameter
            && self.formal == other.formal
            && equal(&self.actual, &other.actual)
            && self.proof == other.proof
    }
}
impl GraphSourceEdgeProof {
    pub fn original_call(&self) -> &Expression {
        &self.original_call
    }
    pub fn current_call(&self) -> &Expression {
        &self.current_call
    }
    pub fn parameter_index(&self) -> usize {
        self.parameter
    }
    pub fn original_formal(&self) -> &Param {
        &self.formal
    }
    pub fn original_actual(&self) -> &Expression {
        &self.actual
    }
    pub fn storage(&self) -> &GraphStorageProof {
        &self.proof
    }
}

#[derive(Debug, Clone)]
struct SequenceOutput {
    current: Local,
    original_iterable: Expression,
    proof: GraphStorageProof,
}
impl PartialEq for SequenceOutput {
    fn eq(&self, other: &Self) -> bool {
        self.current == other.current
            && equal(&self.original_iterable, &other.original_iterable)
            && self.proof == other.proof
    }
}

#[derive(Debug, Clone, Default)]
pub(super) struct Witness {
    owner: Option<hir::Function>,
    local_map: Vec<(Local, Local)>,
    values: Vec<ValueRow>,
    headers: Vec<HeaderRow>,
    formals: Vec<FormalRow>,
    returns: Vec<ValueRow>,
    returned: Option<GraphStorageProof>,
    calls: Vec<Expression>,
    edges: Vec<GraphSourceEdgeProof>,
    graph: Option<current::Snapshot>,
    constructor_outputs: Vec<current::ConstructorOutput>,
    sequence_outputs: Vec<SequenceOutput>,
}
impl PartialEq for Witness {
    fn eq(&self, other: &Self) -> bool {
        match (&self.owner, &other.owner) {
            (Some(a), Some(b)) if original_function_equal(a, b) => {}
            (None, None) => {}
            _ => return false,
        }
        self.local_map == other.local_map
            && self.values == other.values
            && self.headers == other.headers
            && self.formals == other.formals
            && self.returns == other.returns
            && self.returned == other.returned
            && expression_rows_equal(&self.calls, &other.calls)
            && self.edges == other.edges
            && self.graph == other.graph
            && self.constructor_outputs == other.constructor_outputs
            && self.sequence_outputs == other.sequence_outputs
    }
}
impl Witness {
    pub(super) fn needed(&self) -> bool {
        !self.values.is_empty()
            || !self.headers.is_empty()
            || !self.formals.is_empty()
            || self.returned.is_some()
            || !self.calls.is_empty()
    }
    pub(super) fn original_storage_for_expression(
        &self,
        expression: &Expression,
    ) -> Option<&GraphStorageProof> {
        self.values
            .iter()
            .find(|row| equal(&row.original, expression))
            .map(|row| &row.proof)
    }
    pub(super) fn original_storage_for_local(&self, local: &Local) -> Option<&GraphStorageProof> {
        self.headers
            .iter()
            .find(|row| row.original == *local)
            .map(|row| &row.proof)
    }
    pub(super) fn original_storage_for_formal(
        &self,
        parameter: &Param,
    ) -> Option<&GraphStorageProof> {
        self.formals
            .iter()
            .find(|row| row.original == *parameter)
            .map(|row| &row.proof)
    }
    pub(super) fn original_storage_for_return(&self) -> Option<&GraphStorageProof> {
        self.returned.as_ref()
    }
    pub(super) fn has_source_call(&self, expression: &Expression) -> bool {
        self.calls
            .iter()
            .any(|original| equal(original, expression))
    }
    pub(super) fn original_source_edge(
        &self,
        expression: &Expression,
        parameter: usize,
    ) -> Option<&GraphSourceEdgeProof> {
        self.edges
            .iter()
            .find(|edge| edge.parameter == parameter && equal(&edge.original_call, expression))
    }
}

fn equal(left: &Expression, right: &Expression) -> bool {
    crate::breakpoint_regions::expressions_equal(left, right)
}
fn expression_rows_equal(left: &[Expression], right: &[Expression]) -> bool {
    left.len() == right.len() && left.iter().zip(right).all(|(a, b)| equal(a, b))
}
fn merge(facts: impl IntoIterator<Item = GraphStorageProof>) -> Option<GraphStorageProof> {
    let mut alternatives = Vec::new();
    for fact in facts {
        if alternatives
            .first()
            .is_some_and(|old: &GraphStorageProof| old.ty != fact.ty)
        {
            return None;
        }
        let rows = match fact.shape {
            Shape::Alternatives(rows) => rows,
            _ => vec![fact],
        };
        for row in rows {
            if !alternatives.contains(&row) {
                alternatives.push(row);
            }
        }
    }
    match alternatives.len() {
        0 => None,
        1 => alternatives.pop(),
        _ => Some(GraphStorageProof {
            ty: alternatives[0].ty,
            shape: Shape::Alternatives(alternatives),
        }),
    }
}
fn hook_projection(proof: &GraphStorageProof) -> Option<GraphHookProjectionProof> {
    let mut producers = Vec::new();
    fn collect(
        proof: &GraphStorageProof,
        producers: &mut Vec<hir::ActorGraphInstruction>,
    ) -> Option<()> {
        match &proof.shape {
            Shape::Hook(instruction) => {
                if !producers.contains(instruction) {
                    producers.push(instruction.clone());
                }
            }
            Shape::Alternatives(rows) => {
                for row in rows {
                    collect(row, producers)?;
                }
            }
            _ => return None,
        }
        Some(())
    }
    collect(proof, &mut producers)?;
    let hir::ActorGraphOperation::HookBox { hook, .. } = producers.first()?.operation() else {
        return None;
    };
    if hook.function_type() != proof.ty || producers.iter().any(|instruction|
        !matches!(instruction.operation(),hir::ActorGraphOperation::HookBox {hook:other,..} if other==hook)
        || instruction.projected_hook().is_none()) { return None; }
    Some(GraphHookProjectionProof {
        ty: proof.ty,
        hook: hook.clone(),
        producers,
    })
}

/// Function signature types are not occupied values here. Only this private
/// rooted proof permits that distinction; callers cannot query by TypeId.
fn storage_safe(types: &TypeInterner, ty: TypeId) -> bool {
    fn visit(types: &TypeInterner, ty: TypeId, seen: &mut BTreeSet<u32>) -> bool {
        if ty.index() as usize >= types.len() {
            return false;
        }
        if !seen.insert(ty.index()) {
            return true;
        }
        match types.resolve(ty) {
            Type::Resource(_) | Type::Capability(_) | Type::TypeConstruction | Type::Error => false,
            Type::Function { .. } => true,
            Type::List(inner)
            | Type::Set(inner)
            | Type::Optional(inner)
            | Type::Secret(inner)
            | Type::Refinement { base: inner, .. } => visit(types, *inner, seen),
            Type::Map(key, value) | Type::Result(key, value) => {
                visit(types, *key, seen) && visit(types, *value, seen)
            }
            Type::Struct(id) => types
                .resolve_struct(*id)
                .fields
                .iter()
                .all(|(_, field)| visit(types, *field, seen)),
            Type::Actor(id) => {
                let actor = types.resolve_actor(*id);
                actor
                    .capability_params
                    .iter()
                    .chain(&actor.state_fields)
                    .all(|(_, field)| visit(types, *field, seen))
            }
            // These shapes need a protected constructor/variant path in a later
            // packet. The absence of a row is a proof obligation, not a ban.
            Type::Interface(_) | Type::Enum(_) | Type::Machine(_) | Type::MachineState { .. } => {
                false
            }
            _ => true,
        }
    }
    visit(types, ty, &mut BTreeSet::new())
}

pub(super) use original::OriginalGraphAnalysis;

#[cfg(test)]
fn capture(
    original: &hir::Function,
    archive: &hir::ResourceSourceArchive,
    types: &TypeInterner,
) -> Result<Witness, String> {
    OriginalGraphAnalysis::prepare(archive, types)?
        .capture(original, archive, types)
        .cloned()
}
pub(super) fn preserve_original_call(witness: &Witness, expression: &Expression) -> bool {
    witness.has_source_call(expression)
}
pub(super) use current::{
    current, remap_blocks, remap_locals, seal, sequence_transition, validate,
};
pub(super) fn has_records(
    parent: &ResourceLoweringWitness,
    function: &Function,
    types: &TypeInterner,
) -> bool {
    parent.graph_metadata.needed()
        && parent.current(function).is_ok()
        && validate(parent, function, types).is_ok()
}

// Only a completed private analysis for this exact archive can establish
// absence. Empty current rows are required too; malformed positive records
// continue through the independent original/current validation below.
fn no_original_or_current_records(parent: &ResourceLoweringWitness) -> bool {
    !parent.graph_metadata.needed() && parent.execution.has_no_original_graph_rows(&parent.source)
}

pub(super) fn storage_for_expression(
    parent: &ResourceLoweringWitness,
    function: &Function,
    types: &TypeInterner,
    expression: &Expression,
) -> Option<GraphStorageProof> {
    if no_original_or_current_records(parent) {
        return None;
    }
    current::selection(parent, function, types, expression)
}
pub(super) fn storage_for_local(
    parent: &ResourceLoweringWitness,
    function: &Function,
    types: &TypeInterner,
    local: &Local,
) -> Option<GraphStorageProof> {
    if no_original_or_current_records(parent) {
        return None;
    }
    parent.current(function).ok()?;
    validate(parent, function, types).ok()?;
    (function
        .local(local.id)
        .is_some_and(|header| std::ptr::eq(header, local)))
    .then_some(())?;
    parent
        .graph_metadata
        .headers
        .iter()
        .find(|row| row.current == *local)
        .map(|row| row.proof.clone())
        .or_else(|| {
            parent
                .graph_metadata
                .constructor_outputs
                .iter()
                .find(|row| row.current == *local)
                .map(|row| row.proof.clone())
        })
        .or_else(|| {
            parent
                .graph_metadata
                .sequence_outputs
                .iter()
                .find(|row| row.current == *local)
                .map(|row| row.proof.clone())
        })
}
pub(super) fn storage_for_constructor(
    parent: &ResourceLoweringWitness,
    function: &Function,
    types: &TypeInterner,
    node: &ResourceConstructorNode,
) -> Option<GraphStorageProof> {
    if no_original_or_current_records(parent) {
        return None;
    }
    parent.current(function).ok()?;
    validate(parent, function, types).ok()?;
    let occurrences = function.blocks.iter().flat_map(|block| &block.statements)
        .filter(|statement| matches!(&statement.kind, StatementKind::ResourceConstructor(actual) if std::ptr::eq(actual, node)))
        .count();
    (occurrences == 1).then_some(())?;
    parent
        .graph_metadata
        .constructor_outputs
        .iter()
        .find(|row| row.constructor == node.constructor())
        .map(|row| row.proof.clone())
}
pub(super) fn storage_for_formal(
    parent: &ResourceLoweringWitness,
    function: &Function,
    types: &TypeInterner,
    parameter: &Param,
) -> Option<GraphStorageProof> {
    if no_original_or_current_records(parent) {
        return None;
    }
    parent.current(function).ok()?;
    validate(parent, function, types).ok()?;
    function
        .params
        .iter()
        .any(|header| std::ptr::eq(header, parameter))
        .then_some(())?;
    parent
        .graph_metadata
        .formals
        .iter()
        .find(|row| row.current == *parameter)
        .map(|row| row.proof.clone())
}
pub(super) fn storage_for_return(
    parent: &ResourceLoweringWitness,
    function: &Function,
    types: &TypeInterner,
) -> Option<GraphStorageProof> {
    if no_original_or_current_records(parent) {
        return None;
    }
    parent.current(function).ok()?;
    validate(parent, function, types).ok()?;
    parent.graph_metadata.returned.clone()
}
pub(super) fn projection_for_expression(
    parent: &ResourceLoweringWitness,
    function: &Function,
    types: &TypeInterner,
    expression: &Expression,
) -> Option<GraphHookProjectionProof> {
    hook_projection(&storage_for_expression(
        parent, function, types, expression,
    )?)
}
pub(super) fn source_edge(
    parent: &ResourceLoweringWitness,
    function: &Function,
    types: &TypeInterner,
    expression: &Expression,
    parameter: usize,
) -> Option<GraphSourceEdgeProof> {
    if no_original_or_current_records(parent) {
        return None;
    }
    parent.current(function).ok()?;
    validate(parent, function, types).ok()?;
    current::contains(function, expression).then_some(())?;
    parent
        .graph_metadata
        .edges
        .iter()
        .find(|edge| edge.parameter == parameter && equal(&edge.current_call, expression))
        .cloned()
}

#[cfg(test)]
mod tests;
