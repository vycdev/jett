//! Initial Source-authenticated custody analysis, separate from ordinary Move/Copy.
//! A successful plan is metadata proof only; the Resource ABI remains mandatory.
use super::*;
use std::collections::{BTreeMap, BTreeSet};

mod execution_closure;
#[cfg(test)]
mod execution_closure_tests;
mod flow;
pub(super) use execution_closure::ResourceExecutionClosure;
#[cfg(test)]
mod original_calls_tests;
#[cfg(test)]
mod tests;
mod walk;

macro_rules! identity {
    ($name:ident) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name(usize);
        impl $name {
            pub fn index(self) -> usize {
                self.0
            }
        }
    };
}
identity!(ResourceFrameId);
identity!(ResourceOwnerSlotId);
identity!(ResourceLoanId);
identity!(ResourceOperationId);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResourcePosition {
    Statement(usize),
    Terminator,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResourceSite {
    function: FunctionId,
    block: BlockId,
    position: ResourcePosition,
}
impl ResourceSite {
    pub fn function(self) -> FunctionId {
        self.function
    }
    pub fn block(self) -> BlockId {
        self.block
    }
    pub fn position(self) -> ResourcePosition {
        self.position
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResourceShape {
    Plain {
        kind: hir::ResourceKindRef,
    },
    Optional {
        kind: hir::ResourceKindRef,
    },
    Result {
        kind: hir::ResourceKindRef,
        failure: TypeId,
    },
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResourcePath {
    Some,
    Ok,
}
impl ResourceShape {
    pub fn kind(&self) -> &hir::ResourceKindRef {
        match self {
            Self::Plain { kind } | Self::Optional { kind } | Self::Result { kind, .. } => kind,
        }
    }
    pub fn active_path(&self) -> &[ResourcePath] {
        match self {
            Self::Plain { .. } => &[],
            Self::Optional { .. } => &[ResourcePath::Some],
            Self::Result { .. } => &[ResourcePath::Ok],
        }
    }
    fn conditional(&self) -> bool {
        !matches!(self, Self::Plain { .. })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResourceFrameRole {
    Scope,
    Operation,
    Return,
}
#[derive(Debug, Clone)]
pub struct ResourceFrame {
    id: ResourceFrameId,
    role: ResourceFrameRole,
    site: ResourceSite,
    parent: Option<ResourceFrameId>,
    ordinal: usize,
}
impl ResourceFrame {
    pub fn id(&self) -> ResourceFrameId {
        self.id
    }
    pub fn role(&self) -> ResourceFrameRole {
        self.role
    }
    pub fn site(&self) -> ResourceSite {
        self.site
    }
    pub fn parent(&self) -> Option<ResourceFrameId> {
        self.parent
    }
    /// Expression occurrence within the exact containing-function statement or terminator.
    pub fn ordinal(&self) -> usize {
        self.ordinal
    }
}

#[derive(Debug, Clone)]
pub enum ResourceSlotStorage {
    Local {
        header: Local,
    },
    Expression {
        site: ResourceSite,
        ordinal: usize,
    },
    Argument {
        site: ResourceSite,
        call_ordinal: usize,
        parameter: usize,
    },
    Return {
        site: ResourceSite,
    },
}
#[derive(Debug, Clone)]
pub struct ResourceOwnerSlot {
    id: ResourceOwnerSlotId,
    frame: ResourceFrameId,
    shape: ResourceShape,
    storage: ResourceSlotStorage,
}
impl ResourceOwnerSlot {
    pub fn id(&self) -> ResourceOwnerSlotId {
        self.id
    }
    pub fn frame(&self) -> ResourceFrameId {
        self.frame
    }
    pub fn shape(&self) -> &ResourceShape {
        &self.shape
    }
    pub fn storage(&self) -> &ResourceSlotStorage {
        &self.storage
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ResourceLoanSource {
    Owner(ResourceOwnerSlotId),
    IncomingViewFormal {
        scope: ResourceFrameId,
        parameter: usize,
    },
}
#[derive(Debug, Clone)]
pub struct ResourceLoan {
    id: ResourceLoanId,
    frame: ResourceFrameId,
    source: ResourceLoanSource,
}
impl ResourceLoan {
    pub fn id(&self) -> ResourceLoanId {
        self.id
    }
    pub fn frame(&self) -> ResourceFrameId {
        self.frame
    }
    pub fn source(&self) -> ResourceLoanSource {
        self.source
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResourceOccupancy {
    Occupied,
    Conditional,
    Empty,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResourceCompletion {
    Normal,
    Return,
    Abort,
}
#[derive(Debug, Clone)]
pub enum ResourceCallOperand {
    Ordinary {
        parameter: usize,
        ty: TypeId,
    },
    Owned {
        parameter: usize,
        slot: ResourceOwnerSlotId,
    },
    Borrowed {
        parameter: usize,
        loan: ResourceLoanId,
    },
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResourceCallResult {
    Ordinary { ty: TypeId },
    Owned { slot: ResourceOwnerSlotId },
}

/// Readonly projection of the original checked Source tuple, not a minting API.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResourceArgumentSyntax {
    Bare,
    WrittenView,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResourceArgumentEffect {
    Copy,
    TransferOwned,
    RelinquishOwned,
    RetainBorrow,
    ObserveData,
}
#[derive(Debug, Clone)]
pub struct ResourceCallFormal {
    parameter: usize,
    source_index: usize,
    actual_type: TypeId,
    parameter_type: TypeId,
    syntax: ResourceArgumentSyntax,
    effect: ResourceArgumentEffect,
    access: ParamMode,
}
impl ResourceCallFormal {
    pub fn parameter(&self) -> usize {
        self.parameter
    }
    pub fn source_index(&self) -> usize {
        self.source_index
    }
    pub fn actual_type(&self) -> TypeId {
        self.actual_type
    }
    pub fn parameter_type(&self) -> TypeId {
        self.parameter_type
    }
    pub fn syntax(&self) -> ResourceArgumentSyntax {
        self.syntax
    }
    pub fn effect(&self) -> ResourceArgumentEffect {
        self.effect
    }
    pub fn access(&self) -> ParamMode {
        self.access
    }
    fn original(argument: &hir::ArgumentOwnership) -> Self {
        use jett_typecheck::{
            CheckedCalleeAccess as A, CheckedCallerEffect as E, CheckedCallerSyntax as S,
        };
        Self {
            parameter: argument.parameter_index,
            source_index: argument.source_index,
            actual_type: argument.actual_type,
            parameter_type: argument.parameter_type,
            syntax: match argument.syntax {
                S::Bare => ResourceArgumentSyntax::Bare,
                S::WrittenView => ResourceArgumentSyntax::WrittenView,
            },
            effect: match argument.effect {
                E::Copy => ResourceArgumentEffect::Copy,
                E::TransferOwned => ResourceArgumentEffect::TransferOwned,
                E::RelinquishOwned => ResourceArgumentEffect::RelinquishOwned,
                E::RetainBorrow => ResourceArgumentEffect::RetainBorrow,
                E::ObserveData => ResourceArgumentEffect::ObserveData,
            },
            access: match argument.callee_access {
                A::Owned => ParamMode::Owned,
                A::View => ParamMode::View,
            },
        }
    }
}

/// Closed metadata roles. None is a public constructor for a validated plan.
#[derive(Debug, Clone)]
pub enum ResourceOperationRole {
    Acquire {
        hook: hir::ResourceHookRef,
        destination: ResourceOwnerSlotId,
    },
    Transfer {
        source: ResourceOwnerSlotId,
        destination: ResourceOwnerSlotId,
    },
    Borrow {
        loan: ResourceLoanId,
    },
    BoundedBorrowUse {
        loan: ResourceLoanId,
        parameter: usize,
    },
    EndBorrow {
        loan: ResourceLoanId,
    },
    InvokeHook {
        hook: hir::ResourceHookRef,
        source: hir::SourceCallOwnership,
        evaluation_order: Vec<usize>,
        operands: Vec<ResourceCallOperand>,
        result: ResourceCallResult,
    },
    InvokeSourceFunction {
        function: FunctionId,
        formals: Vec<ResourceCallFormal>,
        source: hir::SourceCallOwnership,
        evaluation_order: Vec<usize>,
        operands: Vec<ResourceCallOperand>,
        result: ResourceCallResult,
    },
    /// Move a sole plain payload into a fresh conditional shell.
    SumAdopt {
        source: ResourceOwnerSlotId,
        destination: ResourceOwnerSlotId,
    },
    CreateAbsentSum {
        destination: ResourceOwnerSlotId,
    },
    CreateFailureSum {
        destination: ResourceOwnerSlotId,
        failure: TypeId,
    },
    TakeFailureCompanion {
        source: ResourceOwnerSlotId,
        target: Local,
        failure: TypeId,
        tag: LocalId,
    },
    SumTake {
        source: ResourceOwnerSlotId,
        destination: ResourceOwnerSlotId,
        tag: LocalId,
        success: bool,
    },
    Close {
        hook: hir::ResourceHookRef,
        source: ResourceOwnerSlotId,
    },
    Replace {
        destination: ResourceOwnerSlotId,
        old: ResourceOccupancy,
    },
    /// Scope completion obligations are retired by the dynamic core log, never local-number order.
    Drop {
        source: ResourceOwnerSlotId,
        occupancy: ResourceOccupancy,
    },
    Complete {
        outcome: ResourceCompletion,
    },
    CompleteReturnAfterCleanup {
        source: ResourceOwnerSlotId,
    },
    Descriptor {
        hook: hir::ResourceHookRef,
    },
}
#[derive(Debug, Clone)]
pub struct ResourceOperation {
    id: ResourceOperationId,
    frame: ResourceFrameId,
    site: ResourceSite,
    ordinal: usize,
    role: ResourceOperationRole,
}
impl ResourceOperation {
    pub fn id(&self) -> ResourceOperationId {
        self.id
    }
    pub fn frame(&self) -> ResourceFrameId {
        self.frame
    }
    pub fn site(&self) -> ResourceSite {
        self.site
    }
    pub fn ordinal(&self) -> usize {
        self.ordinal
    }
    pub fn role(&self) -> &ResourceOperationRole {
        &self.role
    }
}

#[derive(Debug)]
pub struct ResourceFunctionPlan {
    function: FunctionId,
    identity: FunctionIdentity,
    parameters: Vec<Param>,
    return_type: TypeId,
    frames: Vec<ResourceFrame>,
    slots: Vec<ResourceOwnerSlot>,
    loans: Vec<ResourceLoan>,
    operations: Vec<ResourceOperation>,
}
impl ResourceFunctionPlan {
    pub fn function(&self) -> FunctionId {
        self.function
    }
    pub fn identity(&self) -> &FunctionIdentity {
        &self.identity
    }
    pub fn parameters(&self) -> &[Param] {
        &self.parameters
    }
    pub fn return_type(&self) -> TypeId {
        self.return_type
    }
    pub fn root_scope(&self) -> &ResourceFrame {
        &self.frames[0]
    }
    pub fn provisional_return(&self) -> Option<&ResourceFrame> {
        self.root_scope()
            .parent
            .and_then(|id| self.frames.get(id.index()))
            .filter(|frame| frame.role == ResourceFrameRole::Return)
    }
    pub fn entry(&self) -> BlockId {
        self.frames[0].site.block
    }
    pub fn frames(&self) -> &[ResourceFrame] {
        &self.frames
    }
    pub fn owner_slots(&self) -> &[ResourceOwnerSlot] {
        &self.slots
    }
    pub fn loans(&self) -> &[ResourceLoan] {
        &self.loans
    }
    pub fn operations(&self) -> &[ResourceOperation] {
        &self.operations
    }
}
#[derive(Debug)]
pub struct ResourceOwnershipPlan<'p> {
    program: &'p Program,
    types: &'p TypeInterner,
    functions: Vec<ResourceFunctionPlan>,
}
impl<'p> ResourceOwnershipPlan<'p> {
    pub fn program(&self) -> &'p Program {
        self.program
    }
    pub fn types(&self) -> &'p TypeInterner {
        self.types
    }
    pub fn functions(&self) -> &[ResourceFunctionPlan] {
        &self.functions
    }
    pub fn original_source(&self, id: FunctionId) -> Option<&'p hir::Function> {
        self.program
            .functions
            .get(id.index() as usize)
            .filter(|function| function.id == id)
            .and_then(|function| function.resource_lowering.as_ref())
            .map(|witness| &witness.original)
    }
    pub fn function(&self, id: FunctionId) -> Option<&ResourceFunctionPlan> {
        self.functions
            .iter()
            .find(|function| function.function == id)
    }
}

#[derive(Debug, Clone, PartialEq)]
struct OriginalCallAssociation {
    original: Expression,
    current: Expression,
}
impl OriginalCallAssociation {
    fn same(&self, other: &Self) -> bool {
        crate::breakpoint_regions::expressions_equal(&self.original, &other.original)
            && crate::breakpoint_regions::expressions_equal(&self.current, &other.current)
    }
}
#[derive(Debug, Clone, PartialEq)]
pub(super) struct ResourceLoweringWitness {
    original: hir::Function,
    calls: Vec<OriginalCallAssociation>,
    source: hir::ResourceSourceArchive,
    execution: ResourceExecutionClosure,
    manifest: hir::ResourceManifest,
    parameters: Vec<Param>,
    locals: Vec<Local>,
    blocks: Vec<BasicBlock>,
    entry: BlockId,
}

#[derive(Clone, Default)]
pub(super) struct Capture {
    witness: Option<ResourceLoweringWitness>,
}
impl Capture {
    /// Only the checked lowering entry calls this, before any operand extraction.
    pub(super) fn authenticated(
        function: &hir::Function,
        manifest: &hir::ResourceManifest,
        source: &hir::ResourceSourceArchive,
        types: &TypeInterner,
        execution: &ResourceExecutionClosure,
    ) -> Self {
        let mut needed = execution.contains(function.id)
            || resource_type_pending(types, function.return_type)
            || function
                .locals
                .iter()
                .any(|local| resource_type_pending(types, local.ty));
        walk::hir_block(&function.body, &mut |value| {
            needed |= matches!(
                value.kind,
                hir::ExpressionKind::ResourceHookValue { .. }
                    | hir::ExpressionKind::ResourceInvoke { .. }
            ) || custody_type(types, value.ty);
        });
        Self {
            witness: needed.then(|| ResourceLoweringWitness {
                original: function.clone(),
                calls: Vec::new(),
                source: source.clone(),
                execution: execution.clone(),
                manifest: manifest.clone(),
                parameters: function.params.clone(),
                locals: function.locals.clone(),
                blocks: vec![BasicBlock {
                    id: BlockId(0),
                    statements: Vec::new(),
                    terminator: Terminator {
                        kind: TerminatorKind::Unreachable,
                        span: function.body.span,
                    },
                }],
                entry: BlockId(0),
            }),
        }
    }
    pub(super) fn preserve_original_call(
        &mut self,
        expression: &Expression,
        types: &TypeInterner,
    ) -> Result<bool, String> {
        let Some(witness) = &self.witness else {
            return Ok(false);
        };
        let (args, ownership) = match &expression.kind {
            hir::ExpressionKind::Call {
                args, ownership, ..
            }
            | hir::ExpressionKind::IndirectCall {
                args, ownership, ..
            }
            | hir::ExpressionKind::ResourceInvoke {
                args, ownership, ..
            } => (args, ownership),
            _ => return Ok(false),
        };
        let occupied = |ty| {
            resource_type_pending(types, ty) && !matches!(types.resolve(ty), Type::Function { .. })
        };
        if !occupied(expression.ty)
            && !args.iter().any(|value| occupied(value.ty))
            && !matches!(expression.kind, hir::ExpressionKind::ResourceInvoke { .. })
            && !witness.execution.direct_call(expression)
        {
            return Ok(false);
        }
        let hir::CallOwnership::Source(source) = ownership else {
            return Err(
                "Resource original call preservation requires exact Source authority".into(),
            );
        };
        if source
            .arguments
            .iter()
            .any(|argument| argument.staging != hir::ArgumentStaging::Original)
        {
            return Err("Resource original call preservation changed its initial staging".into());
        }
        let mut handled = false;
        for value in args {
            walk::expression(value, &mut |value| {
                handled |= matches!(value.kind, hir::ExpressionKind::Handle { .. })
            });
        }
        if let hir::ExpressionKind::IndirectCall { callee, .. } = &expression.kind {
            walk::expression(callee, &mut |value| {
                handled |= matches!(value.kind, hir::ExpressionKind::Handle { .. })
            });
        }
        if handled {
            return Err("pending ResourceOwnershipPlan: eager handled actuals need the dedicated custody CFG normalizer".into());
        }
        let mut occurrences = 0usize;
        walk::hir_block(&witness.original.body, &mut |candidate| {
            occurrences += usize::from(crate::breakpoint_regions::expressions_equal(
                candidate, expression,
            ));
        });
        if occurrences != 1 {
            return Err(
                "Resource preservation has no unique exact original Source expression".into(),
            );
        }
        let mut candidates = Vec::new();
        walk::expression(expression, &mut |candidate| {
            if source_custody_call(candidate, types, &witness.execution) {
                candidates.push(candidate.clone());
            }
        });
        let witness = self
            .witness
            .as_mut()
            .ok_or("Resource association lost its initial witness")?;
        for candidate in candidates {
            let mut original_count = 0;
            walk::hir_block(&witness.original.body, &mut |original| {
                original_count += usize::from(crate::breakpoint_regions::expressions_equal(
                    original, &candidate,
                ));
            });
            if original_count != 1 {
                return Err("Resource call association has no unique original Source tuple".into());
            }
            if !witness.calls.iter().any(|record| {
                crate::breakpoint_regions::expressions_equal(&record.current, &candidate)
            }) {
                witness.calls.push(OriginalCallAssociation {
                    original: candidate.clone(),
                    current: candidate,
                });
            }
        }
        Ok(true)
    }
    pub(super) fn block(&mut self, block: BlockId, span: Span) {
        if let Some(witness) = &mut self.witness {
            witness.blocks.push(BasicBlock {
                id: block,
                statements: Vec::new(),
                terminator: Terminator {
                    kind: TerminatorKind::Unreachable,
                    span,
                },
            });
        }
    }
    pub(super) fn local(&mut self, local: &Local) {
        if let Some(witness) = &mut self.witness {
            witness.locals.push(local.clone());
        }
    }
    pub(super) fn statement(&mut self, block: BlockId, index: usize, value: &Statement) {
        if let Some(witness) = &mut self.witness {
            if let Some(destination) = witness.blocks.get_mut(block.index() as usize) {
                // Sequential constructor writes only; a mismatch remains unsealable.
                if destination.id == block && destination.statements.len() == index {
                    destination.statements.push(value.clone());
                }
            }
        }
    }
    pub(super) fn terminator(&mut self, block: BlockId, value: &Terminator) {
        if let Some(witness) = &mut self.witness {
            if let Some(destination) = witness
                .blocks
                .get_mut(block.index() as usize)
                .filter(|destination| destination.id == block)
            {
                destination.terminator = value.clone();
            }
        }
    }
    pub(super) fn finish(
        self,
        function: &Function,
    ) -> Result<Option<ResourceLoweringWitness>, String> {
        if let Some(witness) = &self.witness {
            witness.current(function)?;
        }
        Ok(self.witness)
    }
}
impl ResourceLoweringWitness {
    fn current(&self, function: &Function) -> Result<(), String> {
        if function.id != self.original.id
            || function.identity != self.original.identity
            || function.debug_kind != self.original.debug_kind
            || function.params != self.parameters
            || function.capture_count != self.original.capture_count
            || function.return_type != self.original.return_type
            || function.span != self.original.span
            || function.entry != self.entry
            || function.locals != self.locals
            || !crate::breakpoint_regions::blocks_equal(&function.blocks, &self.blocks)
        {
            return Err("Resource ownership differs from its initially authenticated Source or constructor-emitted graph".into());
        }
        for (index, record) in self.calls.iter().enumerate() {
            if self.calls[..index].iter().any(|earlier| {
                crate::breakpoint_regions::expressions_equal(&earlier.original, &record.original)
                    || crate::breakpoint_regions::expressions_equal(
                        &earlier.current,
                        &record.current,
                    )
            }) {
                return Err("Resource original call association is duplicated".into());
            }
            let (mut original_count, mut current_count) = (0usize, 0usize);
            walk::hir_block(&self.original.body, &mut |value| {
                original_count += usize::from(crate::breakpoint_regions::expressions_equal(
                    value,
                    &record.original,
                ))
            });
            for block in &self.blocks {
                walk::mir_block(block, &mut |value| {
                    current_count += usize::from(crate::breakpoint_regions::expressions_equal(
                        value,
                        &record.current,
                    ))
                });
            }
            if original_count != 1 || current_count != 1 {
                return Err("Resource original call association lost its unique source or current occurrence".into());
            }
        }
        Ok(())
    }
    fn same(&self, other: &Self) -> bool {
        self.original.id == other.original.id
            && self.original.identity == other.original.identity
            && self.original.source_definition == other.original.source_definition
            && self.original.params == other.original.params
            && self.original.locals == other.original.locals
            && self.original.return_type == other.original.return_type
            && self.original.capture_count == other.original.capture_count
            && self.original.span == other.original.span
            && self.original.debug_kind == other.original.debug_kind
            && crate::breakpoint_regions::hir_blocks_equal(
                &self.original.body,
                &other.original.body,
            )
            && self.calls.len() == other.calls.len()
            && self
                .calls
                .iter()
                .zip(&other.calls)
                .all(|(left, right)| left.same(right))
            && self.source == other.source
            && self.execution == other.execution
            && self.manifest == other.manifest
            && self.parameters == other.parameters
            && self.locals == other.locals
            && self.entry == other.entry
            && crate::breakpoint_regions::blocks_equal(&self.blocks, &other.blocks)
    }
}

fn original_function_equal(left: &hir::Function, right: &hir::Function) -> bool {
    left.id == right.id
        && left.identity == right.identity
        && left.debug_kind == right.debug_kind
        && left.source_definition == right.source_definition
        && left.params == right.params
        && left.capture_count == right.capture_count
        && left.return_type == right.return_type
        && left.locals == right.locals
        && left.span == right.span
        && crate::breakpoint_regions::hir_blocks_equal(&left.body, &right.body)
}
/// Executed before constructor capture; edited public HIR can never be re-sealed.
pub(super) fn authenticate_original(
    program: &hir::Program,
    types: &TypeInterner,
) -> Result<ResourceExecutionClosure, String> {
    let archive = &program.resource_source;
    if archive.manifest().is_none() {
        if !program.resource_manifest.kinds().is_empty()
            || program.resource_manifest.hooks().next().is_some()
            || program.functions.iter().any(|function| {
                resource_type_pending(types, function.return_type)
                    || function
                        .locals
                        .iter()
                        .any(|local| resource_type_pending(types, local.ty))
            })
        {
            return Err("Resource lowering has no original checked HIR archive".into());
        }
        return Ok(ResourceExecutionClosure::default());
    }
    archive.validate_types(types)?;
    if archive.manifest() != Some(&program.resource_manifest)
        || archive.equality_methods() != Some(&program.equality_methods)
        || archive.functions().len() != program.functions.len()
        || !archive
            .functions()
            .iter()
            .zip(&program.functions)
            .all(|(left, right)| original_function_equal(left, right))
    {
        return Err("Resource lowering differs from its original checked HIR archive".into());
    }
    ResourceExecutionClosure::from_original(archive.functions(), types)
}

fn custody_type(types: &TypeInterner, ty: TypeId) -> bool {
    resource_type_pending(types, ty) && !matches!(types.resolve(ty), Type::Function { .. })
}
fn source_custody_call(
    value: &Expression,
    types: &TypeInterner,
    execution: &ResourceExecutionClosure,
) -> bool {
    let (args, ownership) = match &value.kind {
        hir::ExpressionKind::Call {
            args, ownership, ..
        }
        | hir::ExpressionKind::IndirectCall {
            args, ownership, ..
        }
        | hir::ExpressionKind::ResourceInvoke {
            args, ownership, ..
        } => (args, ownership),
        _ => return false,
    };
    matches!(ownership, hir::CallOwnership::Source(source) if source.arguments.iter().all(|argument| argument.staging == hir::ArgumentStaging::Original))
        && (custody_type(types, value.ty)
            || args.iter().any(|arg| custody_type(types, arg.ty))
            || matches!(value.kind, hir::ExpressionKind::ResourceInvoke { .. })
            || execution.direct_call(value))
}
/// A readonly association lookup, never a constructor or ordinary owner claim.
pub(super) fn original_call_at(
    function: &Function,
    manifest: &hir::ResourceManifest,
    types: &TypeInterner,
    expression: &Expression,
    block: BlockId,
    statement: usize,
) -> Result<bool, String> {
    let Some(witness) = function.resource_lowering.as_ref() else {
        return Ok(false);
    };
    if !witness
        .calls
        .iter()
        .any(|record| crate::breakpoint_regions::expressions_equal(&record.current, expression))
    {
        return Ok(false);
    }
    witness.source.validate_types(types)?;
    if witness.execution
        != ResourceExecutionClosure::from_original(witness.source.functions(), types)?
    {
        return Err(
            "Resource call execution closure differs from its original checked bodies".into(),
        );
    }
    witness.manifest.validate(types)?;
    witness.current(function)?;
    if &witness.manifest != manifest
        || witness.source.manifest() != Some(manifest)
        || !witness
            .source
            .functions()
            .iter()
            .any(|original| original_function_equal(original, &witness.original))
    {
        return Err(
            "Resource call association differs from its original checked function or manifest"
                .into(),
        );
    }
    let current = function
        .blocks
        .get(block.index() as usize)
        .filter(|value| value.id == block)
        .ok_or("Resource call association has no exact current block")?;
    let mut pointers = 0;
    walk::mir_site(current, statement, &mut |value| {
        pointers += usize::from(std::ptr::eq(value, expression))
    });
    if pointers != 1 {
        return Err(
            "Resource call association differs from its exact current expression site".into(),
        );
    }
    Ok(true)
}
/// The finite connected actual shapes; descriptors and ordinary data remain ordinary.
pub(super) fn original_resource_actual(
    manifest: &hir::ResourceManifest,
    types: &TypeInterner,
    ty: TypeId,
) -> bool {
    match types.resolve(ty) {
        Type::Resource(_) => manifest.contains_type(ty),
        Type::Optional(inner) => {
            matches!(types.resolve(*inner), Type::Resource(_)) && manifest.contains_type(*inner)
        }
        Type::Result(ok, fail) => {
            matches!(types.resolve(*ok), Type::Resource(_))
                && manifest.contains_type(*ok)
                && !resource_type_pending(types, *fail)
        }
        _ => false,
    }
}

pub(super) fn validate_witnesses(
    program: &Program,
    types: &TypeInterner,
) -> Result<(), Vec<ValidationError>> {
    let mut errors = Vec::new();
    let execution = program
        .functions
        .iter()
        .find_map(|function| function.resource_lowering.as_ref())
        .map(|witness| {
            witness.source.validate_types(types)?;
            ResourceExecutionClosure::from_original(witness.source.functions(), types)
        })
        .transpose()
        .map_err(|message| {
            vec![ValidationError {
                span: program
                    .functions
                    .first()
                    .map_or(Span::new(jett_common::FileId::new(0), 0, 0), |function| {
                        function.span
                    }),
                message,
            }]
        })?
        .unwrap_or_default();
    for function in &program.functions {
        let needed = execution.contains(function.id)
            || resource_type_pending(types, function.return_type)
            || function
                .locals
                .iter()
                .any(|local| resource_type_pending(types, local.ty))
            || function.blocks.iter().any(|block| {
                walk::mir_block_has_resource(block) || walk::mir_block_has_custody(block, types)
            });
        match &function.resource_lowering {
            Some(witness) => {
                let result = witness
                    .source
                    .validate_types(types)
                    .and_then(|()| witness.manifest.validate(types))
                    .and_then(|()| {
                        if witness.execution != execution {
                            return Err("Resource execution closure differs from the original checked call graph".into());
                        }
                        if witness.source.manifest() != Some(&witness.manifest)
                            || !witness.source.functions().iter().any(|original| {
                                original_function_equal(original, &witness.original)
                            })
                        {
                            return Err(
                                "Resource ownership lost its original checked HIR archive join"
                                    .into(),
                            );
                        }
                        if witness.manifest != program.resource_manifest {
                            return Err(
                                "Resource ownership witness belongs to another checked manifest"
                                    .into(),
                            );
                        }
                        witness.current(function)
                    });
                if let Err(message) = result {
                    errors.push(ValidationError {
                        span: function.span,
                        message,
                    });
                }
            }
            None if needed => errors.push(ValidationError {
                span: function.span,
                message:
                    "Resource ownership has no initially authenticated Source constructor witness"
                        .into(),
            }),
            None => {}
        }
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

/// Fresh typed/CFG proof. This is not a runtime registration or ABI admission.
pub fn validate_resource_ownership<'p>(
    program: &'p Program,
    types: &'p TypeInterner,
) -> Result<ResourceOwnershipPlan<'p>, Vec<ValidationError>> {
    validate_call_ownership(program, types)?;
    validate_witnesses(program, types)?;
    let mut functions = Vec::new();
    let mut errors = Vec::new();
    for function in &program.functions {
        if function.resource_lowering.is_none() {
            continue;
        }
        match flow::analyze(program, function, types) {
            Ok(plan) => functions.push(plan),
            Err(message) => errors.push(ValidationError {
                span: function.span,
                message,
            }),
        }
    }
    if errors.is_empty() {
        Ok(ResourceOwnershipPlan {
            program,
            types,
            functions,
        })
    } else {
        Err(errors)
    }
}

fn shape(
    manifest: &hir::ResourceManifest,
    types: &TypeInterner,
    ty: TypeId,
) -> Result<Option<ResourceShape>, String> {
    if ty.index() as usize >= types.len() {
        return Err("Resource ownership type is outside the exact interner".into());
    }
    let kind = |ty| {
        manifest.kind_for_type(ty).ok_or_else(|| {
            "Resource ownership kind has no original manifest declaration".to_string()
        })
    };
    Ok(match types.resolve(ty) {
        Type::Resource(_) => Some(ResourceShape::Plain { kind: kind(ty)? }),
        Type::Optional(inner) if matches!(types.resolve(*inner), Type::Resource(_)) => Some(ResourceShape::Optional { kind: kind(*inner)? }),
        Type::Result(ok, fail) if matches!(types.resolve(*ok), Type::Resource(_)) && !resource_type_pending(types, *fail) => Some(ResourceShape::Result { kind: kind(*ok)?, failure: *fail }),
        Type::Function { .. } => None, // A signature is descriptor data, never occupied custody.
        _ if resource_type_pending(types, ty) => return Err("pending ResourceOwnershipPlan: occupied Resource layout needs its dedicated aggregate or qualification transport".into()),
        _ => None,
    })
}

pub(super) fn has_records(function: &Function) -> bool {
    function.resource_lowering.is_some()
}
pub(super) fn validate_current(function: &Function) -> Result<(), String> {
    function
        .resource_lowering
        .as_ref()
        .map_or(Ok(()), |witness| witness.current(function))
}

/// Only the canonical unreachable-block deletion supplies this exact map.
pub(super) fn remap_blocks(
    function: &mut Function,
    before: &Function,
    map: &[Option<BlockId>],
) -> Result<(), String> {
    let Some(original) = before.resource_lowering.as_ref() else {
        return Ok(());
    };
    original.current(before)?;
    let witness = function
        .resource_lowering
        .as_mut()
        .ok_or("Resource canonical remap lost its constructor witness")?;
    if !witness.same(original) {
        return Err("Resource canonical remap changed its before-witness".into());
    }
    witness.blocks.retain(|block| {
        map.get(block.id.index() as usize)
            .copied()
            .flatten()
            .is_some()
    });
    let remap = |id: &mut BlockId| {
        if let Some(mapped) = map.get(id.index() as usize).copied().flatten() {
            *id = mapped;
        }
    };
    remap(&mut witness.entry);
    for block in &mut witness.blocks {
        remap(&mut block.id);
        crate::sequences::prune::block_targets(&mut block.terminator.kind, &remap);
    }
    function
        .resource_lowering
        .as_ref()
        .ok_or("Resource canonical remap lost its witness")?
        .current(function)
}

pub(super) fn remap_locals(
    function: &mut Function,
    before: &Function,
    map: &[Option<LocalId>],
) -> Result<(), String> {
    let Some(original) = before.resource_lowering.as_ref() else {
        return Ok(());
    };
    original.current(before)?;
    let witness = function
        .resource_lowering
        .as_mut()
        .ok_or("Resource canonical local remap lost its constructor witness")?;
    if !witness.same(original) {
        return Err("Resource canonical local remap changed its before-witness".into());
    }
    // Source IDs in original are archival. Only the independently captured current graph is remapped.
    let mut valid = true;
    let mut remap = |id: &mut LocalId| {
        if let Some(mapped) = map.get(id.index() as usize).copied().flatten() {
            *id = mapped;
        } else {
            valid = false;
        }
    };
    for parameter in &mut witness.parameters {
        remap(&mut parameter.local);
    }
    witness.locals.retain(|local| {
        map.get(local.id.index() as usize)
            .copied()
            .flatten()
            .is_some()
    });
    for local in &mut witness.locals {
        remap(&mut local.id);
        if let Some(source) = &mut local.view_source {
            remap(source);
        }
    }
    for block in &mut witness.blocks {
        crate::sequences::prune::block_locals(block, &mut remap, &mut |_| {});
    }
    for record in &mut witness.calls {
        let mut block = BasicBlock {
            id: BlockId(0),
            statements: vec![Statement {
                kind: StatementKind::Evaluate(record.current.clone()),
                span: record.current.span,
            }],
            terminator: Terminator {
                kind: TerminatorKind::Unreachable,
                span: record.current.span,
            },
        };
        crate::sequences::prune::block_locals(&mut block, &mut remap, &mut |_| {});
        let StatementKind::Evaluate(value) = block.statements.remove(0).kind else {
            return Err("Resource association metadata remap changed its wrapper".into());
        };
        record.current = value;
    }
    if !valid {
        return Err("Resource canonical local remap lost an active current reference".into());
    }
    function
        .resource_lowering
        .as_ref()
        .ok_or("Resource canonical remap lost its witness")?
        .current(function)
}
