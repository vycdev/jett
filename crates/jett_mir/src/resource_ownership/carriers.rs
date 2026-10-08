//! Constructor-issued typed aggregate transport. No graph row grants leaf custody.
//! Every runtime root is validated to contain zero selected Resource leaves before
//! it is committed; latent occupied shape metadata is retained without execution.
use super::*;
use hir::ExpressionKind as E;
use std::sync::Arc;

mod flow;
mod graph;
mod iterations;
mod planner;

identity!(ResourceCarrierShapeId);
identity!(ResourceCarrierSlotId);
identity!(ResourceCarrierLoanId);
identity!(ResourceCarrierOperationId);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResourceCarrierField {
    pub ordinal: usize,
    pub name: String,
    pub shape: ResourceCarrierShapeId,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResourceCarrierVariant {
    pub ordinal: usize,
    pub name: String,
    pub discriminant: i64,
    pub fields: Vec<ResourceCarrierField>,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResourceCarrierState {
    pub ordinal: usize,
    pub name: String,
    pub fields: Vec<ResourceCarrierField>,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResourceCarrierNode {
    Ordinary,
    Resource {
        kind: hir::ResourceKindRef,
    },
    Optional {
        payload: ResourceCarrierShapeId,
    },
    Result {
        success: ResourceCarrierShapeId,
        failure: ResourceCarrierShapeId,
    },
    List {
        element: ResourceCarrierShapeId,
    },
    Map {
        key: ResourceCarrierShapeId,
        value: ResourceCarrierShapeId,
    },
    Struct {
        owner: jett_types::StructId,
        arguments: Vec<ResourceCarrierShapeId>,
        fields: Vec<ResourceCarrierField>,
    },
    Enum {
        owner: jett_types::EnumId,
        arguments: Vec<ResourceCarrierShapeId>,
        variants: Vec<ResourceCarrierVariant>,
    },
    Machine {
        owner: jett_types::MachineId,
        states: Vec<ResourceCarrierState>,
        transitions: Vec<(usize, usize)>,
    },
    MachineState {
        machine: ResourceCarrierShapeId,
        state: usize,
    },
    Refinement {
        name: String,
        base: ResourceCarrierShapeId,
    },
}
#[derive(Debug, Clone)]
pub struct ResourceCarrierShape {
    id: ResourceCarrierShapeId,
    ty: TypeId,
    node: ResourceCarrierNode,
    contains_resource: bool,
    refinement_predicate: Option<hir::Function>,
}
impl ResourceCarrierShape {
    pub fn id(&self) -> ResourceCarrierShapeId {
        self.id
    }
    pub fn ty(&self) -> TypeId {
        self.ty
    }
    pub fn node(&self) -> &ResourceCarrierNode {
        &self.node
    }
    pub fn contains_resource(&self) -> bool {
        self.contains_resource
    }
    /// Original checked declaration/body association; metadata grants no proof
    /// that a raw base value satisfies the refinement at runtime.
    pub fn refinement_predicate(&self) -> Option<&hir::Function> {
        self.refinement_predicate.as_ref()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResourceCarrierGeneration {
    /// Runtime activation issues a fresh generation on each successful install.
    FreshOnInstall,
}
#[derive(Debug, Clone)]
pub struct ResourceCarrierSlot {
    id: ResourceCarrierSlotId,
    frame: ResourceFrameId,
    shape: ResourceCarrierShapeId,
    storage: ResourceSlotStorage,
}
impl ResourceCarrierSlot {
    pub fn id(&self) -> ResourceCarrierSlotId {
        self.id
    }
    pub fn frame(&self) -> ResourceFrameId {
        self.frame
    }
    pub fn shape(&self) -> ResourceCarrierShapeId {
        self.shape
    }
    pub fn storage(&self) -> &ResourceSlotStorage {
        &self.storage
    }
    pub fn generation(&self) -> ResourceCarrierGeneration {
        ResourceCarrierGeneration::FreshOnInstall
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResourceCarrierIndex {
    Constant(usize),
    Local(LocalId),
    CurrentIteration { header: BlockId },
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResourceCarrierEdge {
    SwitchVariant {
        source: BlockId,
        variant: VariantId,
        target: BlockId,
    },
    SwitchOtherwise {
        source: BlockId,
        target: BlockId,
    },
    IterationBody {
        source: BlockId,
        target: BlockId,
    },
}
#[derive(Debug, Clone)]
pub struct ResourceCarrierIteration {
    pub header: BlockId,
    pub body: BlockId,
    pub exit: BlockId,
    pub source: ResourceCarrierSlotId,
    pub binders: Vec<LocalId>,
    pub cursor: Option<LocalId>,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResourceCarrierProjectionPath {
    Field {
        owner: TypeId,
        field: FieldId,
    },
    Variant {
        owner: TypeId,
        variant: VariantId,
        field: usize,
    },
    State {
        owner: TypeId,
        state: usize,
        field: usize,
    },
    Element {
        index: ResourceCarrierIndex,
    },
    MapKey {
        index: ResourceCarrierIndex,
    },
    MapValue {
        index: ResourceCarrierIndex,
    },
    OptionalSome,
    ResultOk,
    ResultFail,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResourceCarrierLoanSource {
    Slot(ResourceCarrierSlotId),
    Loan(ResourceCarrierLoanId),
    IncomingViewFormal {
        scope: ResourceFrameId,
        parameter: usize,
    },
}
#[derive(Debug, Clone)]
pub struct ResourceCarrierLoan {
    id: ResourceCarrierLoanId,
    frame: ResourceFrameId,
    shape: ResourceCarrierShapeId,
    source: ResourceCarrierLoanSource,
    path: Vec<ResourceCarrierProjectionPath>,
}
impl ResourceCarrierLoan {
    pub fn id(&self) -> ResourceCarrierLoanId {
        self.id
    }
    pub fn frame(&self) -> ResourceFrameId {
        self.frame
    }
    pub fn shape(&self) -> ResourceCarrierShapeId {
        self.shape
    }
    pub fn source(&self) -> ResourceCarrierLoanSource {
        self.source
    }
    pub fn path(&self) -> &[ResourceCarrierProjectionPath] {
        &self.path
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResourceCarrierValue {
    Ordinary { ty: TypeId },
    Owned { slot: ResourceCarrierSlotId },
    Borrowed { loan: ResourceCarrierLoanId },
    LeafOwned { slot: ResourceOwnerSlotId },
    LeafBorrowed { loan: ResourceLoanId },
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResourceCarrierConstructor {
    List,
    Map,
    Struct,
    Enum { variant: VariantId },
    Machine { state: usize },
    OptionalNone,
    OptionalSome,
    ResultOk,
    ResultFail,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResourceCarrierChild {
    pub index: usize,
    pub shape: ResourceCarrierShapeId,
    pub value: ResourceCarrierValue,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResourceCarrierObservation {
    Length,
    Tag,
    State {
        state: usize,
    },
    Ordinary {
        path: Vec<ResourceCarrierProjectionPath>,
        ty: TypeId,
    },
}
#[derive(Debug, Clone)]
pub enum ResourceCarrierOperationRole {
    BeginConstructor {
        destination: ResourceCarrierSlotId,
        constructor: ResourceCarrierConstructor,
    },
    ConstructorChild {
        destination: ResourceCarrierSlotId,
        child: ResourceCarrierChild,
    },
    CommitConstructor {
        destination: ResourceCarrierSlotId,
    },
    Transfer {
        source: ResourceCarrierSlotId,
        destination: ResourceCarrierSlotId,
    },
    QualifyMachine {
        source: ResourceCarrierSlotId,
        destination: ResourceCarrierSlotId,
        machine: ResourceCarrierShapeId,
        state: usize,
    },
    Borrow {
        loan: ResourceCarrierLoanId,
    },
    EndBorrow {
        loan: ResourceCarrierLoanId,
    },
    Observe {
        source: ResourceCarrierLoanId,
        observation: ResourceCarrierObservation,
        target: Option<LocalId>,
    },
    Project {
        source: ResourceCarrierLoanId,
        destination: ResourceCarrierLoanId,
    },
    Extract {
        source: ResourceCarrierLoanId,
        path: Vec<ResourceCarrierProjectionPath>,
        destination: ResourceCarrierSlotId,
    },
    AdaptSum {
        source: ResourceCarrierLoanId,
        path: Vec<ResourceCarrierProjectionPath>,
        destination: ResourceOwnerSlotId,
        loan: Option<ResourceLoanId>,
        lease_frame: ResourceFrameId,
    },
    ExtractOrdinary {
        source: ResourceCarrierLoanId,
        path: Vec<ResourceCarrierProjectionPath>,
        target: LocalId,
        ty: TypeId,
    },
    BeginIteration {
        source: ResourceCarrierValue,
        destination: ResourceCarrierSlotId,
        body: BlockId,
        exit: BlockId,
    },
    RetireIteration {
        source: ResourceCarrierSlotId,
        binders: Vec<LocalId>,
    },
    PublishReturn {
        source: ResourceCarrierSlotId,
    },
    Retire {
        source: ResourceCarrierSlotId,
    },
}
#[derive(Debug, Clone)]
pub struct ResourceCarrierOperation {
    id: ResourceCarrierOperationId,
    site: ResourceSite,
    frame: ResourceFrameId,
    role: ResourceCarrierOperationRole,
    expression: Option<usize>,
    edge: Option<ResourceCarrierEdge>,
}
impl ResourceCarrierOperation {
    pub fn id(&self) -> ResourceCarrierOperationId {
        self.id
    }
    pub fn site(&self) -> ResourceSite {
        self.site
    }
    pub fn frame(&self) -> ResourceFrameId {
        self.frame
    }
    pub fn role(&self) -> &ResourceCarrierOperationRole {
        &self.role
    }
    pub fn edge(&self) -> Option<ResourceCarrierEdge> {
        self.edge
    }
    pub fn is_expression_operation(&self) -> bool {
        self.expression.is_some()
    }
    pub fn is_for_expression(&self, expression: &Expression) -> bool {
        self.expression == Some(std::ptr::from_ref(expression).addr())
    }
}
#[derive(Debug)]
pub struct ResourceCarrierFunctionPlan {
    shapes: Arc<Vec<ResourceCarrierShape>>,
    slots: Vec<ResourceCarrierSlot>,
    loans: Vec<ResourceCarrierLoan>,
    operations: Vec<ResourceCarrierOperation>,
    iterations: Vec<ResourceCarrierIteration>,
}
impl ResourceCarrierFunctionPlan {
    pub fn shapes(&self) -> &[ResourceCarrierShape] {
        &self.shapes
    }
    pub fn shape_for_type(&self, ty: TypeId) -> Option<&ResourceCarrierShape> {
        self.shapes
            .get(usize::try_from(ty.index()).ok()?)
            .filter(|row| row.ty == ty)
    }
    pub fn shape(&self, id: ResourceCarrierShapeId) -> Option<&ResourceCarrierShape> {
        self.shapes.get(id.index()).filter(|row| row.id == id)
    }
    pub fn slots(&self) -> &[ResourceCarrierSlot] {
        &self.slots
    }
    pub fn loans(&self) -> &[ResourceCarrierLoan] {
        &self.loans
    }
    pub fn operations(&self) -> &[ResourceCarrierOperation] {
        &self.operations
    }
    pub fn iterations(&self) -> &[ResourceCarrierIteration] {
        &self.iterations
    }
    pub fn operations_for_expression<'a>(
        &'a self,
        expression: &'a Expression,
    ) -> impl Iterator<Item = &'a ResourceCarrierOperation> {
        self.operations
            .iter()
            .filter(move |operation| operation.is_for_expression(expression))
    }
    pub fn operations_at(
        &self,
        site: ResourceSite,
    ) -> impl Iterator<Item = &ResourceCarrierOperation> {
        self.operations
            .iter()
            .filter(move |operation| operation.site == site && operation.edge.is_none())
    }
    pub fn operations_on_edge(
        &self,
        edge: ResourceCarrierEdge,
    ) -> impl Iterator<Item = &ResourceCarrierOperation> {
        self.operations
            .iter()
            .filter(move |operation| operation.edge == Some(edge))
    }
}

/// Leaf and function descriptor types retain their existing protocol.
pub(super) fn carrier_type(types: &TypeInterner, ty: TypeId) -> bool {
    if !resource_type_pending(types, ty) {
        return false;
    }
    match types.resolve(ty) {
        Type::Resource(_) | Type::Function { .. } => false,
        Type::Optional(inner) if matches!(types.resolve(*inner), Type::Resource(_)) => false,
        Type::Result(ok, fail)
            if matches!(types.resolve(*ok), Type::Resource(_))
                && !resource_type_pending(types, *fail) =>
        {
            false
        }
        _ => true,
    }
}
pub(super) fn required(function: &Function, types: &TypeInterner) -> bool {
    carrier_type(types, function.return_type)
        || function
            .locals
            .iter()
            .any(|local| carrier_type(types, local.ty))
        || function.blocks.iter().any(|block| {
            let mut found = false;
            walk::mir_block(block, &mut |value| found |= carrier_type(types, value.ty));
            found
        })
}
pub(super) fn analyze(
    program: &Program,
    function: &Function,
    types: &TypeInterner,
) -> Result<ResourceFunctionPlan, String> {
    planner::analyze(program, function, types)
}
