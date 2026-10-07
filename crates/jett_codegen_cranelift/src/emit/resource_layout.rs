//! Constructor-owned projection of a fresh Source custody proof into immutable wire rows.
//! Bytes and ordinals are consistency metadata; only the runtime issuer creates custody.
use super::*;
use jett_mir::{
    self as custody, ResourceCallOperand as Operand, ResourceCallResult as CallResult,
    ResourceOperationRole as Role,
};
use std::collections::BTreeMap;

const LIMIT: usize = 4096;
const BYTE_LIMIT: usize = 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct SelectedEntry {
    pub(super) function: u32,
    pub(super) signature: u32,
    pub(super) scope: u32,
}
#[derive(Debug)]
pub(super) struct EmittedResourceLayout<'p> {
    plan: custody::ResourceOwnershipPlan<'p>,
    entry: SelectedEntry,
    wire: Vec<u8>,
    functions: BTreeMap<u32, u32>,
    frames: BTreeMap<(u32, usize), u32>,
    slots: BTreeMap<(u32, usize), u32>,
    operations: BTreeMap<(u32, usize), u32>,
}
impl<'p> EmittedResourceLayout<'p> {
    /// The driver-selected identity is an input; no function name or block chooses entry.
    pub(super) fn from_program(
        program: &'p Program,
        types: &'p TypeInterner,
        entry: FunctionId,
    ) -> Result<Self, CodegenError> {
        let plan = custody::validate_resource_ownership(program, types)
            .map_err(CodegenError::InvalidMir)?;
        let selected = program
            .functions
            .get(entry.index() as usize)
            .filter(|function| function.id == entry)
            .ok_or_else(|| pending("driver entry has no exact current function"))?;
        let original = plan
            .original_source(entry)
            .ok_or_else(|| pending("driver entry has no original Source execution closure"))?;
        let headers_match = selected.params.len() == original.params.len()
            && selected
                .params
                .iter()
                .zip(&original.params)
                .all(|(current, archived)| {
                    current.name == archived.name
                        && current.ty == archived.ty
                        && current.mode == archived.mode
                        && current.mutable == archived.mutable
                        && current.span == archived.span
                });
        // Current local IDs are independently authenticated by the constructor witness;
        // the immutable original IDs stay archival through exact canonical compaction.
        if selected.identity != original.identity
            || !headers_match
            || selected.return_type != original.return_type
            || selected.capture_count != 0
            || selected.return_type != TypeInterner::NOTHING
            || selected
                .params
                .iter()
                .any(|param| param.ty != TypeInterner::NETWORK)
        {
            return Err(pending(
                "driver entry does not have the original closed Network/Nothing header",
            ));
        }
        let mut rows = Rows::new(&plan)?;
        rows.populate()?;
        let signature = rows.function_signature(entry)?;
        let function = plan
            .function(entry)
            .ok_or_else(|| pending("entry has no fresh custody function plan"))?;
        let scope = rows.frame(entry, function.root_scope().id())?;
        let frame = &mut rows.frames[scope as usize];
        if function.provisional_return().is_some() || !frame.parents.is_empty() {
            return Err(pending(
                "entry Scope cannot also be activated as a Source callee",
            ));
        }
        frame.parents.push(Parent::Root);
        let wire = rows.encode()?;
        let entry = SelectedEntry {
            function: entry.index(),
            signature,
            scope,
        };
        let functions = rows.functions.clone();
        let frames = rows.frame_ids.clone();
        let slots = rows.slot_ids.clone();
        let operations = rows.operation_ids.clone();
        drop(rows);
        Ok(Self {
            plan,
            entry,
            wire,
            functions,
            frames,
            slots,
            operations,
        })
    }
    pub(super) fn entry(&self) -> SelectedEntry {
        self.entry
    }
    pub(super) fn bytes(&self) -> &[u8] {
        &self.wire
    }
    pub(super) fn plan(&self) -> &custody::ResourceOwnershipPlan<'p> {
        &self.plan
    }
    pub(super) fn function_signature(&self, function: FunctionId) -> Option<u32> {
        self.functions.get(&function.index()).copied()
    }
    pub(super) fn frame(
        &self,
        function: FunctionId,
        frame: custody::ResourceFrameId,
    ) -> Option<u32> {
        self.frames.get(&(function.index(), frame.index())).copied()
    }
    pub(super) fn slot(
        &self,
        function: FunctionId,
        slot: custody::ResourceOwnerSlotId,
    ) -> Option<u32> {
        self.slots.get(&(function.index(), slot.index())).copied()
    }
    pub(super) fn operation(
        &self,
        function: FunctionId,
        operation: custody::ResourceOperationId,
    ) -> Option<u32> {
        self.operations
            .get(&(function.index(), operation.index()))
            .copied()
    }

    /// Emits only an immutable accessor; this does not emit or admit a Source body.
    pub(super) fn define_accessor(&self, module: &mut ObjectModule) -> Result<(), CodegenError> {
        const SYMBOL: &str = "jett_aot_resource_v1_manifest";
        if module.target_config().pointer_type() != ir::types::I64 {
            return Err(pending(
                "Resource manifest accessor requires the selected 64-bit layout",
            ));
        }
        if module.get_name(SYMBOL).is_some() {
            return Err(CodegenError::DuplicateSymbol(SYMBOL.into()));
        }
        let data_id = module
            .declare_data("__jett_resource_layout_v2", Linkage::Local, false, false)
            .map_err(|error| {
                CodegenError::Backend(format!("Resource layout data declaration: {error}"))
            })?;
        let mut data = cranelift_module::DataDescription::new();
        data.define(self.wire.clone().into_boxed_slice());
        module.define_data(data_id, &data).map_err(|error| {
            CodegenError::Backend(format!("Resource layout data definition: {error}"))
        })?;
        let mut signature = module.make_signature();
        signature.params.push(AbiParam::new(ir::types::I64));
        signature.returns.push(AbiParam::new(ir::types::I32));
        let id = module
            .declare_function(SYMBOL, Linkage::Export, &signature)
            .map_err(|error| {
                CodegenError::Backend(format!("Resource manifest accessor declaration: {error}"))
            })?;
        let mut context = module.make_context();
        context.func.signature = signature;
        let mut builder_context = FunctionBuilderContext::new();
        let mut builder = FunctionBuilder::new(&mut context.func, &mut builder_context);
        let entry = builder.create_block();
        let valid = builder.create_block();
        let refused = builder.create_block();
        builder.append_block_params_for_function_params(entry);
        builder.switch_to_block(entry);
        let output = builder.block_params(entry)[0];
        let global = module.declare_data_in_func(data_id, builder.func);
        let address = builder.ins().global_value(ir::types::I64, global);
        let not_null = builder.ins().icmp_imm(IntCC::NotEqual, output, 0);
        let low = builder.ins().band_imm(output, 7);
        let aligned = builder.ins().icmp_imm(IntCC::Equal, low, 0);
        let output_end = builder.ins().iadd_imm(output, 32);
        let no_overflow = builder
            .ins()
            .icmp(IntCC::UnsignedGreaterThan, output_end, output);
        let data_end = builder.ins().iadd_imm(
            address,
            i64::try_from(self.wire.len())
                .map_err(|_| pending("manifest data length exceeds selected address width"))?,
        );
        let before = builder
            .ins()
            .icmp(IntCC::UnsignedLessThanOrEqual, output_end, address);
        let after = builder
            .ins()
            .icmp(IntCC::UnsignedGreaterThanOrEqual, output, data_end);
        let separate = builder.ins().bor(before, after);
        let valid_alignment = builder.ins().band(not_null, aligned);
        let valid_range = builder.ins().band(no_overflow, separate);
        let accepted = builder.ins().band(valid_alignment, valid_range);
        builder.ins().brif(accepted, valid, &[], refused, &[]);
        builder.switch_to_block(valid);
        let flags = ir::MemFlags::new();
        builder.ins().store(flags, address, output, 0);
        let length = builder.ins().iconst(
            ir::types::I64,
            i64::try_from(self.wire.len())
                .map_err(|_| pending("manifest data length exceeds selected address width"))?,
        );
        builder.ins().store(flags, length, output, 8);
        for (offset, value) in [
            (16, self.entry.function),
            (20, self.entry.signature),
            (24, self.entry.scope),
            (28, 0),
        ] {
            let value = builder.ins().iconst(ir::types::I32, i64::from(value));
            builder.ins().store(flags, value, output, offset);
        }
        let zero = builder.ins().iconst(ir::types::I32, 0);
        builder.ins().return_(&[zero]);
        builder.switch_to_block(refused);
        let refusal = builder.ins().iconst(ir::types::I32, 1);
        builder.ins().return_(&[refusal]);
        builder.seal_all_blocks();
        builder.finalize();
        module.define_function(id, &mut context).map_err(|error| {
            CodegenError::Backend(format!("Resource manifest accessor definition: {error}"))
        })?;
        Ok(())
    }
}
pub(super) fn pending(message: &str) -> CodegenError {
    CodegenError::Backend(format!("pending native Resource execution: {message}"))
}
fn ordinal(value: usize) -> Result<u32, CodegenError> {
    if value >= LIMIT {
        return Err(pending(
            "layout row capacity exceeds the bounded wire domain",
        ));
    }
    u32::try_from(value).map_err(|_| pending("layout ordinal exceeds u32"))
}
fn count(value: usize) -> Result<u32, CodegenError> {
    if value > LIMIT {
        return Err(pending("layout vector exceeds the bounded wire domain"));
    }
    u32::try_from(value).map_err(|_| pending("layout vector exceeds u32"))
}
#[derive(Debug, Clone, PartialEq, Eq)]
struct Signature {
    result: u32,
    parameters: Vec<(u32, u32)>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Parent {
    Root,
    Frame(u32),
}
#[derive(Debug)]
struct Frame {
    role: u32,
    site: custody::ResourceSite,
    signature: u32,
    parents: Vec<Parent>,
}
struct Rows<'a, 'p> {
    plan: &'a custody::ResourceOwnershipPlan<'p>,
    hooks: Vec<(u32, u32, u32)>,
    hook_ids: Vec<jett_hir::ResourceHookRef>,
    signatures: Vec<Signature>,
    shapes: Vec<Vec<u32>>,
    shape_ids: BTreeMap<u32, u32>,
    functions: BTreeMap<u32, u32>,
    frames: Vec<Frame>,
    frame_ids: BTreeMap<(u32, usize), u32>,
    slots: Vec<Vec<u32>>,
    slot_ids: BTreeMap<(u32, usize), u32>,
    operations: Vec<(custody::ResourceSite, Vec<u32>)>,
    operation_ids: BTreeMap<(u32, usize), u32>,
    borrow_ids: BTreeMap<(u32, usize), u32>,
    descriptor_borrow_targets: BTreeMap<(u32, usize), u32>,
}
impl<'a, 'p> Rows<'a, 'p> {
    fn new(plan: &'a custody::ResourceOwnershipPlan<'p>) -> Result<Self, CodegenError> {
        count(plan.program().resource_manifest.kinds().len())?;
        Ok(Self {
            plan,
            hooks: Vec::new(),
            hook_ids: Vec::new(),
            signatures: Vec::new(),
            shapes: Vec::new(),
            shape_ids: BTreeMap::new(),
            functions: BTreeMap::new(),
            frames: Vec::new(),
            frame_ids: BTreeMap::new(),
            slots: Vec::new(),
            slot_ids: BTreeMap::new(),
            operations: Vec::new(),
            operation_ids: BTreeMap::new(),
            borrow_ids: BTreeMap::new(),
            descriptor_borrow_targets: BTreeMap::new(),
        })
    }
    fn shape(&mut self, ty: TypeId) -> Result<u32, CodegenError> {
        if let Some(id) = self.shape_ids.get(&ty.index()) {
            return Ok(*id);
        }
        let words = match self.plan.types().resolve(ty) {
            Type::Int8 => vec![1, 8, 1],
            Type::Int16 => vec![1, 16, 1],
            Type::Int32 => vec![1, 32, 1],
            Type::Int64 => vec![1, 64, 1],
            Type::Uint8 => vec![1, 8, 0],
            Type::Uint16 => vec![1, 16, 0],
            Type::Uint32 => vec![1, 32, 0],
            Type::Uint64 => vec![1, 64, 0],
            Type::Float32 => vec![2, 32],
            Type::Float64 => vec![2, 64],
            Type::Bool => vec![3],
            Type::String => vec![4],
            Type::Nothing => vec![5],
            Type::Capability(jett_types::CapabilityKind::Network) => vec![6],
            Type::Resource(_) => vec![
                7,
                self.plan
                    .program()
                    .resource_manifest
                    .kind_for_type(ty)
                    .ok_or_else(|| pending("shape has no original manifest kind"))?
                    .id()
                    .index(),
            ],
            Type::Optional(child) => {
                let child = *child;
                vec![8, self.shape(child)?]
            }
            Type::Result(ok, fail) => {
                let (ok, fail) = (*ok, *fail);
                vec![9, self.shape(ok)?, self.shape(fail)?]
            }
            _ => {
                return Err(pending(
                    "exact shape needs its dedicated native qualification, aggregate or callable transport",
                ));
            }
        };
        let id = if let Some(index) = self.shapes.iter().position(|shape| shape == &words) {
            ordinal(index)?
        } else {
            let id = ordinal(self.shapes.len())?;
            self.shapes.push(words);
            id
        };
        self.shape_ids.insert(ty.index(), id);
        Ok(id)
    }
    fn custody_shape(&mut self, shape: &custody::ResourceShape) -> Result<u32, CodegenError> {
        let plain = self.shape(shape.kind().ty())?;
        let words = match shape {
            custody::ResourceShape::Plain { .. } => return Ok(plain),
            custody::ResourceShape::Optional { .. } => vec![8, plain],
            custody::ResourceShape::Result { failure, .. } => vec![9, plain, self.shape(*failure)?],
        };
        if let Some(index) = self.shapes.iter().position(|shape| shape == &words) {
            return ordinal(index);
        }
        let id = ordinal(self.shapes.len())?;
        self.shapes.push(words);
        Ok(id)
    }
    fn signature(
        &mut self,
        params: &[(TypeId, jett_mir::ParamMode)],
        result: TypeId,
    ) -> Result<u32, CodegenError> {
        let result = self.shape(result)?;
        self.signature_result(params, result)
    }
    fn signature_result(
        &mut self,
        params: &[(TypeId, jett_mir::ParamMode)],
        result: u32,
    ) -> Result<u32, CodegenError> {
        let mut parameters = Vec::new();
        for (ty, mode) in params {
            parameters.push((
                self.shape(*ty)?,
                match mode {
                    jett_mir::ParamMode::Owned => 1,
                    jett_mir::ParamMode::View => 2,
                },
            ));
        }
        let signature = Signature { result, parameters };
        if let Some(index) = self
            .signatures
            .iter()
            .position(|existing| existing == &signature)
        {
            return ordinal(index);
        }
        let id = ordinal(self.signatures.len())?;
        self.signatures.push(signature);
        Ok(id)
    }
    fn function_signature(&self, function: FunctionId) -> Result<u32, CodegenError> {
        self.functions
            .get(&function.index())
            .copied()
            .ok_or_else(|| pending("callee has no fresh execution-family signature"))
    }
    fn frame(
        &self,
        function: FunctionId,
        frame: custody::ResourceFrameId,
    ) -> Result<u32, CodegenError> {
        self.frame_ids
            .get(&(function.index(), frame.index()))
            .copied()
            .ok_or_else(|| pending("frame is outside its exact function plan"))
    }
    fn slot(
        &self,
        function: FunctionId,
        slot: custody::ResourceOwnerSlotId,
    ) -> Result<u32, CodegenError> {
        self.slot_ids
            .get(&(function.index(), slot.index()))
            .copied()
            .ok_or_else(|| pending("slot is outside its exact function plan"))
    }
    fn hook(&self, hook: &jett_hir::ResourceHookRef) -> Result<u32, CodegenError> {
        self.hook_ids
            .iter()
            .position(|candidate| candidate == hook)
            .map(ordinal)
            .transpose()?
            .ok_or_else(|| pending("operation has a foreign manifest hook"))
    }
    fn loan(
        &self,
        function: &custody::ResourceFunctionPlan,
        loan: custody::ResourceLoanId,
    ) -> Result<Vec<u32>, CodegenError> {
        let source = function
            .loans()
            .get(loan.index())
            .filter(|record| record.id() == loan)
            .ok_or_else(|| pending("formal has no exact plan loan"))?;
        Ok(match source.source() {
            custody::ResourceLoanSource::Owner(_) => vec![
                1,
                *self
                    .borrow_ids
                    .get(&(function.function().index(), loan.index()))
                    .ok_or_else(|| pending("loan has no unique actual Borrow operation"))?,
            ],
            custody::ResourceLoanSource::IncomingViewFormal { scope, parameter } => vec![
                2,
                self.frame(function.function(), scope)?,
                count(parameter)?,
            ],
            custody::ResourceLoanSource::ProjectedSumPayload { .. } => vec![
                3,
                *self
                    .borrow_ids
                    .get(&(function.function().index(), loan.index()))
                    .ok_or_else(|| {
                        pending("projected payload has no unique constructor projection operation")
                    })?,
            ],
        })
    }
    fn populate(&mut self) -> Result<(), CodegenError> {
        let plan = self.plan;
        // Every original hook is retained, including unused descriptors.
        for hook in plan.program().resource_manifest.hooks() {
            let Type::Function {
                params,
                view_params,
                return_type,
            } = self.plan.types().resolve(hook.function_type())
            else {
                return Err(pending("manifest hook lost its exact function signature"));
            };
            if params.len() != view_params.len() {
                return Err(pending("hook access vector is malformed"));
            }
            let parameters = params
                .iter()
                .zip(view_params)
                .map(|(ty, view)| {
                    (
                        *ty,
                        if *view {
                            jett_mir::ParamMode::View
                        } else {
                            jett_mir::ParamMode::Owned
                        },
                    )
                })
                .collect::<Vec<_>>();
            let result = *return_type;
            let signature = self.signature(&parameters, result)?;
            let recipe = match hook.recipe() {
                jett_types::ResourceKernelRecipe::NetworkFactory => 1,
                jett_types::ResourceKernelRecipe::NetworkBorrow => 2,
                jett_types::ResourceKernelRecipe::Finalize => 3,
            };
            ordinal(self.hooks.len())?;
            self.hooks.push((hook.kind().index(), recipe, signature));
            self.hook_ids.push(hook);
        }
        for function in plan.functions() {
            let params = function
                .parameters()
                .iter()
                .map(|parameter| (parameter.ty, parameter.mode))
                .collect::<Vec<_>>();
            let result = if matches!(
                plan.types().resolve(function.return_type()),
                Type::Function { .. }
            ) {
                let hook = function.descriptor_return().ok_or_else(|| {
                    pending("descriptor result has no exact original/current Return proof")
                })?;
                if hook.function_type() != function.return_type() {
                    return Err(pending(
                        "descriptor Return proof changes its function signature",
                    ));
                }
                let words = vec![10, self.hook(&hook)?];
                if let Some(index) = self.shapes.iter().position(|shape| shape == &words) {
                    ordinal(index)?
                } else {
                    let id = ordinal(self.shapes.len())?;
                    self.shapes.push(words);
                    id
                }
            } else {
                self.shape(function.return_type())?
            };
            let signature = self.signature_result(&params, result)?;
            self.functions
                .insert(function.function().index(), signature);
            for frame in function.frames() {
                let id = ordinal(self.frames.len())?;
                self.frame_ids
                    .insert((function.function().index(), frame.id().index()), id);
                self.frames.push(Frame {
                    role: match frame.role() {
                        custody::ResourceFrameRole::Scope => 1,
                        custody::ResourceFrameRole::Operation => 2,
                        custody::ResourceFrameRole::Return => 3,
                    },
                    site: frame.site(),
                    signature,
                    parents: Vec::new(),
                });
            }
        }
        for function in plan.functions() {
            for frame in function.frames() {
                if let Some(parent) = frame.parent() {
                    let (id, parent) = (
                        self.frame(function.function(), frame.id())?,
                        self.frame(function.function(), parent)?,
                    );
                    self.frames[id as usize].parents.push(Parent::Frame(parent));
                }
            }
            for slot in function.owner_slots() {
                let id = ordinal(self.slots.len())?;
                let frame = self.frame(function.function(), slot.frame())?;
                let shape = self.custody_shape(slot.shape())?;
                if matches!(slot.storage(), custody::ResourceSlotStorage::Return { .. })
                    && shape
                        != self.signatures[self.function_signature(function.function())? as usize]
                            .result
                {
                    return Err(pending(
                        "Return slot differs from its containing function signature result",
                    ));
                }
                let mut row = vec![frame, shape, count(slot.shape().active_path().len())?];
                row.extend(slot.shape().active_path().iter().map(|step| match step {
                    custody::ResourcePath::Some => 1,
                    custody::ResourcePath::Ok => 2,
                }));
                self.slot_ids
                    .insert((function.function().index(), slot.id().index()), id);
                self.slots.push(row);
            }
        }
        // Fix all operation ordinals before projecting references to Borrow rows.
        for function in plan.functions() {
            for operation in function.operations() {
                if (!emitted(operation.role()) && operation.indirect_hook_target().is_none())
                    || staged_alias(function, operation)?.is_some()
                {
                    continue;
                }
                let id = ordinal(self.operations.len())?;
                self.operation_ids
                    .insert((function.function().index(), operation.id().index()), id);
                if let Role::Borrow { loan }
                | Role::PrepareSourceBorrow { loan, .. }
                | Role::BorrowSum { loan }
                | Role::PrepareSourceSumBorrow { loan, .. }
                | Role::ProjectSumView {
                    destination: loan, ..
                } = operation.role()
                {
                    if self
                        .borrow_ids
                        .insert((function.function().index(), loan.index()), id)
                        .is_some()
                    {
                        return Err(pending("loan has duplicate constructor Borrow operations"));
                    }
                }
                self.operations.push((operation.site(), Vec::new()));
                if operation.indirect_hook_target().is_some()
                    && matches!(operation.role(), Role::InvokeHook { hook, .. }
                        if hook.recipe() == jett_types::ResourceKernelRecipe::NetworkBorrow)
                {
                    let target = ordinal(self.operations.len())?;
                    self.descriptor_borrow_targets.insert(
                        (function.function().index(), operation.id().index()),
                        target,
                    );
                    self.operations.push((operation.site(), Vec::new()));
                }
            }
        }
        for function in plan.functions() {
            for operation in function.operations() {
                let Some(id) = self
                    .operation_ids
                    .get(&(function.function().index(), operation.id().index()))
                    .copied()
                else {
                    continue;
                };
                let row = self.operation_row(function, operation)?;
                self.operations[id as usize].1 = row;
                if let Some(target) = self
                    .descriptor_borrow_targets
                    .get(&(function.function().index(), operation.id().index()))
                    .copied()
                {
                    self.operations[target as usize].1 =
                        self.borrow_hook_row(function, operation)?;
                }
            }
        }
        // Prepared metadata owns the single wire row. Reached activations use its
        // exact ordinal; the private region witness owns their cross-block sites.
        for function in plan.functions() {
            for operation in function.operations() {
                let Some(prepared) = staged_alias(function, operation)? else {
                    continue;
                };
                let id = *self
                    .operation_ids
                    .get(&(function.function().index(), prepared.id().index()))
                    .ok_or_else(|| pending("staged activation lost its prepared row"))?;
                if self
                    .operation_ids
                    .insert((function.function().index(), operation.id().index()), id)
                    .is_some()
                {
                    return Err(pending("staged activation already owns another wire row"));
                }
            }
        }
        for frame in &mut self.frames {
            frame.parents.sort();
            frame.parents.dedup();
            count(frame.parents.len())?;
        }
        Ok(())
    }
    fn operation_row(
        &mut self,
        function: &custody::ResourceFunctionPlan,
        operation: &custody::ResourceOperation,
    ) -> Result<Vec<u32>, CodegenError> {
        let f = function.function();
        let frame = self.frame(f, operation.frame())?;
        if let Some(proof) = operation.indirect_hook_target() {
            let Role::InvokeHook { hook, source, .. } = operation.role() else {
                return Err(pending(
                    "descriptor target proof is not an exact hook invocation",
                ));
            };
            if hook != proof
                || !matches!(source.target, jett_hir::CallTarget::Indirect { signature_type }
                    if signature_type == hook.function_type())
                || source.bridge != jett_hir::CallBridge::Direct
                || !source.generated_operands.is_empty()
            {
                return Err(pending(
                    "descriptor wire projection changes its exact original hook tuple",
                ));
            }
            let target = if hook.recipe() == jett_types::ResourceKernelRecipe::NetworkBorrow {
                *self
                    .descriptor_borrow_targets
                    .get(&(f.index(), operation.id().index()))
                    .ok_or_else(|| pending("indirect Borrow has no separate physical target row"))?
            } else {
                let mut targets = function.operations().iter().filter(|physical| {
                    physical.frame() == operation.frame()
                        && match physical.role() {
                            Role::Acquire { hook: current, .. } => {
                                current == hook
                                    && hook.recipe()
                                        == jett_types::ResourceKernelRecipe::NetworkFactory
                            }
                            Role::Close { hook: current, .. } => {
                                current == hook
                                    && hook.recipe() == jett_types::ResourceKernelRecipe::Finalize
                            }
                            _ => false,
                        }
                });
                let target = targets
                    .next()
                    .ok_or_else(|| pending("indirect hook has no exact physical target"))?;
                if targets.next().is_some() {
                    return Err(pending("indirect hook has multiple physical targets"));
                }
                *self
                    .operation_ids
                    .get(&(f.index(), target.id().index()))
                    .ok_or_else(|| pending("indirect hook physical target has no fixed row"))?
            };
            let hook = self.hook(hook)?;
            return Ok(vec![15, hook, self.hooks[hook as usize].2, target]);
        }
        Ok(match operation.role() {
            Role::Acquire { hook, destination } => {
                vec![1, frame, self.hook(hook)?, self.slot(f, *destination)?]
            }
            Role::Transfer {
                source,
                destination,
            } => {
                let slot = function
                    .owner_slots()
                    .get(destination.index())
                    .ok_or_else(|| {
                        pending("Transfer destination is outside its fresh function plan")
                    })?;
                let execution_frame =
                    if matches!(slot.storage(), custody::ResourceSlotStorage::Return { .. }) {
                        let returning = function.provisional_return().ok_or_else(|| {
                            pending("Return transfer has no exact provisional frame")
                        })?;
                        if operation.frame() != returning.id() || slot.frame() != returning.id() {
                            return Err(pending(
                                "Return transfer differs from its exact provisional storage",
                            ));
                        }
                        // Execution precedes Scope cleanup; storage remains parent-lived Return.
                        self.frame(f, function.root_scope().id())?
                    } else {
                        frame
                    };
                vec![
                    2,
                    execution_frame,
                    self.slot(f, *source)?,
                    self.slot(f, *destination)?,
                ]
            }
            Role::Borrow { loan } | Role::PrepareSourceBorrow { loan, .. } => {
                let record = function
                    .loans()
                    .get(loan.index())
                    .ok_or_else(|| pending("Borrow loan is absent"))?;
                let custody::ResourceLoanSource::Owner(source) = record.source() else {
                    return Err(pending("resident formals cannot mint child core loans"));
                };
                vec![
                    3,
                    frame,
                    self.slot(f, source)?,
                    self.frame(f, record.frame())?,
                ]
            }
            Role::EndBorrow { loan } => {
                let source = self.loan(function, *loan)?;
                if !matches!(source[0], 1 | 3) {
                    return Err(pending(
                        "resident formal cannot retire the caller's core loan",
                    ));
                }
                vec![5, frame, source[1]]
            }
            Role::BorrowSum { loan } | Role::PrepareSourceSumBorrow { loan, .. } => {
                let record = function
                    .loans()
                    .get(loan.index())
                    .ok_or_else(|| pending("sum borrow has no exact loan"))?;
                let custody::ResourceLoanSource::Owner(source) = record.source() else {
                    return Err(pending(
                        "sum borrow cannot mint authority from an incoming formal",
                    ));
                };
                if matches!(record.shape(), custody::ResourceShape::Plain { .. }) {
                    return Err(pending("sum borrow requires an exact conditional shape"));
                }
                vec![
                    21,
                    frame,
                    self.slot(f, source)?,
                    self.frame(f, record.frame())?,
                ]
            }
            Role::ObserveSumView { source, .. } => {
                let mut row = vec![22, frame];
                row.extend(self.loan(function, *source)?);
                row
            }
            Role::ProjectSumView {
                source,
                destination,
                path,
                ..
            } => {
                let record = function
                    .loans()
                    .get(destination.index())
                    .ok_or_else(|| pending("sum projection has no exact child loan"))?;
                if !matches!(record.shape(), custody::ResourceShape::Plain { .. }) {
                    return Err(pending("sum projection child is not a plain Resource loan"));
                }
                let mut row = vec![23, frame];
                row.extend(self.loan(function, *source)?);
                row.push(match path {
                    custody::ResourceSumPayloadPath::OptionalSome => 1,
                    custody::ResourceSumPayloadPath::ResultOk => 2,
                });
                row.push(self.frame(f, record.frame())?);
                row
            }
            Role::ReadFailureCompanion {
                source, failure, ..
            } => {
                let mut row = vec![24, frame];
                row.extend(self.loan(function, *source)?);
                row.push(self.shape(*failure)?);
                row
            }
            Role::EndSumBorrow { loan } => {
                let source = self.loan(function, *loan)?;
                if source[0] != 1 {
                    return Err(pending(
                        "incoming sum formal cannot end its caller's shell lease",
                    ));
                }
                vec![25, frame, source[1]]
            }
            Role::InvokeHook { hook, .. } => {
                if hook.recipe() != jett_types::ResourceKernelRecipe::NetworkBorrow {
                    return Err(pending("unexpected emitted hook metadata role"));
                }
                self.borrow_hook_row(function, operation)?
            }
            Role::Close { hook, source } => {
                vec![7, frame, self.hook(hook)?, self.slot(f, *source)?]
            }
            Role::Drop { source, .. } => {
                let slot = function
                    .owner_slots()
                    .get(source.index())
                    .ok_or_else(|| pending("Drop slot is absent"))?;
                vec![
                    if matches!(slot.shape(), custody::ResourceShape::Plain { .. }) {
                        8
                    } else {
                        11
                    },
                    frame,
                    self.slot(f, *source)?,
                ]
            }
            Role::SumAdopt {
                source,
                destination,
            } => vec![
                9,
                frame,
                self.slot(f, *source)?,
                self.slot(f, *destination)?,
            ],
            Role::SumTake {
                source,
                destination,
                success,
                ..
            } => {
                if !*success {
                    return Err(pending("failure extraction cannot adopt a Resource owner"));
                }
                vec![
                    10,
                    frame,
                    self.slot(f, *source)?,
                    self.slot(f, *destination)?,
                ]
            }
            Role::Replace {
                destination,
                replacement,
                old,
            } => {
                let old_slot = function
                    .owner_slots()
                    .get(destination.index())
                    .ok_or_else(|| pending("Replace destination is outside its exact plan"))?;
                let rhs_slot = function
                    .owner_slots()
                    .get(replacement.index())
                    .ok_or_else(|| pending("Replace RHS is outside its exact plan"))?;
                if destination == replacement
                    || *old != custody::ResourceOccupancy::Occupied
                    || !matches!(old_slot.shape(), custody::ResourceShape::Plain { .. })
                    || old_slot.shape() != rhs_slot.shape()
                    || !matches!(old_slot.storage(), custody::ResourceSlotStorage::Local { header } if header.mutable)
                    || old_slot.frame() != operation.frame()
                    || rhs_slot.frame() != operation.frame()
                    || function.frames()[operation.frame().index()].role()
                        != custody::ResourceFrameRole::Scope
                {
                    return Err(pending(
                        "mutable replacement requires distinct occupied plain owners in the exact Scope",
                    ));
                }
                vec![
                    12,
                    frame,
                    self.slot(f, *destination)?,
                    self.slot(f, *replacement)?,
                ]
            }
            Role::SelfRebind { .. } => {
                return Err(pending("SelfRebind has no runtime operation row"));
            }
            Role::Complete { .. } => vec![13, frame],
            Role::Descriptor { hook } => vec![14, self.hook(hook)?],
            Role::BeginSourceFunction {
                function: callee,
                formals,
                evaluation_order,
                operands,
                result,
                ..
            }
            | Role::InvokeSourceFunction {
                function: callee,
                formals,
                evaluation_order,
                operands,
                result,
                ..
            } => self.source_row(
                function,
                frame,
                *callee,
                formals,
                evaluation_order,
                operands,
                *result,
            )?,
            Role::TakeFailureCompanion {
                source, failure, ..
            } => vec![17, frame, self.slot(f, *source)?, self.shape(*failure)?],
            Role::CompleteReturnAfterCleanup { source } => vec![
                18,
                self.frame(f, function.root_scope().id())?,
                self.slot(f, *source)?,
            ],
            Role::CreateAbsentSum { destination } => vec![19, frame, self.slot(f, *destination)?],
            Role::CreateFailureSum {
                destination,
                failure,
            } => vec![
                20,
                frame,
                self.slot(f, *destination)?,
                self.shape(*failure)?,
            ],
            Role::StageSourceActual { .. } | Role::BoundedBorrowUse { .. } => {
                return Err(pending(
                    "bounded use is joined by the actual invocation row",
                ));
            }
        })
    }
    fn borrow_hook_row(
        &mut self,
        function: &custody::ResourceFunctionPlan,
        operation: &custody::ResourceOperation,
    ) -> Result<Vec<u32>, CodegenError> {
        let Role::InvokeHook { hook, operands, .. } = operation.role() else {
            return Err(pending(
                "physical Borrow target is not an exact hook operation",
            ));
        };
        if hook.recipe() != jett_types::ResourceKernelRecipe::NetworkBorrow {
            return Err(pending("physical Borrow target changes its hook recipe"));
        }
        let mut loans = operands.iter().filter_map(|operand| {
            if let Operand::Borrowed { loan, .. } = operand {
                Some(*loan)
            } else {
                None
            }
        });
        let loan = loans
            .next()
            .ok_or_else(|| pending("Borrow hook has no exact resource loan"))?;
        if loans.next().is_some() {
            return Err(pending("Borrow hook has multiple resource loan operands"));
        }
        let mut row = vec![
            6,
            self.frame(function.function(), operation.frame())?,
            self.hook(hook)?,
        ];
        row.extend(self.loan(function, loan)?);
        Ok(row)
    }
    fn source_row(
        &mut self,
        caller: &custody::ResourceFunctionPlan,
        frame: u32,
        target: FunctionId,
        formals: &[custody::ResourceCallFormal],
        order: &[usize],
        operands: &[Operand],
        result: CallResult,
    ) -> Result<Vec<u32>, CodegenError> {
        let plan = self.plan;
        let callee = plan
            .function(target)
            .ok_or_else(|| pending("Source target is outside the fresh execution closure"))?;
        if formals.len() != callee.parameters().len()
            || order.len() != formals.len()
            || operands.len() != formals.len()
        {
            return Err(pending("Source formal tuple is incomplete"));
        }
        let scope = self.frame(target, callee.root_scope().id())?;
        let returning = callee
            .provisional_return()
            .map(|record| self.frame(target, record.id()))
            .transpose()?;
        let parent = returning.unwrap_or(scope);
        self.frames[parent as usize]
            .parents
            .push(Parent::Frame(frame));
        let mut row = vec![
            16,
            frame,
            target.index(),
            self.function_signature(target)?,
            scope,
        ];
        if let Some(returning) = returning {
            row.extend([1, returning]);
        } else {
            row.push(0);
        }
        row.push(count(order.len())?);
        for parameter in order {
            row.push(count(*parameter)?);
        }
        row.push(count(formals.len())?);
        for (parameter, param) in callee.parameters().iter().enumerate() {
            let fact = formals
                .iter()
                .find(|fact| fact.parameter() == parameter)
                .ok_or_else(|| pending("Source parameter has no original formal fact"))?;
            if fact.parameter_type() != param.ty
                || fact.access() != param.mode
                || order.get(fact.source_index()) != Some(&parameter)
            {
                return Err(pending(
                    "Source target header/permutation differs from its sealed formal",
                ));
            }
            let syntax = match fact.syntax() {
                custody::ResourceArgumentSyntax::Bare => 1,
                custody::ResourceArgumentSyntax::WrittenView => 2,
            };
            let effect = match fact.effect() {
                custody::ResourceArgumentEffect::Copy => 1,
                custody::ResourceArgumentEffect::TransferOwned => 2,
                custody::ResourceArgumentEffect::RelinquishOwned => 3,
                custody::ResourceArgumentEffect::RetainBorrow => 4,
                custody::ResourceArgumentEffect::ObserveData => 5,
            };
            row.extend([
                count(parameter)?,
                count(fact.source_index())?,
                self.shape(fact.actual_type())?,
                self.shape(param.ty)?,
                syntax,
                effect,
                if param.mode == jett_mir::ParamMode::View {
                    2
                } else {
                    1
                },
            ]);
            let operand = operands
                .iter()
                .find(|operand| match operand {
                    Operand::Ordinary { parameter: p, .. }
                    | Operand::Owned { parameter: p, .. }
                    | Operand::Borrowed { parameter: p, .. } => *p == parameter,
                })
                .ok_or_else(|| pending("Source formal has no exact evaluated operand"))?;
            match operand {
                Operand::Ordinary { .. } => row.push(1),
                Operand::Owned { slot, .. } => {
                    let destinations=callee.owner_slots().iter().filter(|slot| matches!(slot.storage(),custody::ResourceSlotStorage::Local {header} if header.id==param.local)).collect::<Vec<_>>();
                    if destinations.len() != 1
                        || destinations[0].frame() != callee.root_scope().id()
                    {
                        return Err(pending("owned formal has no unique exact parameter slot"));
                    }
                    row.extend([
                        2,
                        self.slot(caller.function(), *slot)?,
                        self.slot(target, destinations[0].id())?,
                    ]);
                }
                Operand::Borrowed { loan, .. } => {
                    let record = caller
                        .loans()
                        .get(loan.index())
                        .ok_or_else(|| pending("Source formal has no exact borrowed shape"))?;
                    row.push(
                        if matches!(record.shape(), custody::ResourceShape::Plain { .. }) {
                            3
                        } else {
                            4
                        },
                    );
                    row.extend(self.loan(caller, *loan)?);
                }
            }
        }
        match result {
            CallResult::Ordinary { ty } => {
                if ty != callee.return_type() || returning.is_some() {
                    return Err(pending(
                        "ordinary Source result changes the callee return contract",
                    ));
                }
                row.extend([
                    1,
                    self.signatures[self.function_signature(target)? as usize].result,
                ]);
            }
            CallResult::Owned { slot } => {
                let returning = returning.ok_or_else(|| {
                    pending("owned Source result has no callee provisional Return")
                })?;
                let destination = caller
                    .owner_slots()
                    .get(slot.index())
                    .ok_or_else(|| pending("caller result slot is absent"))?;
                let mut returns = callee
                    .owner_slots()
                    .iter()
                    .filter(|slot| {
                        matches!(slot.storage(), custody::ResourceSlotStorage::Return { .. })
                    })
                    .map(|slot| self.slot(target, slot.id()))
                    .collect::<Result<Vec<_>, _>>()?;
                returns.sort();
                returns.dedup();
                if returns.is_empty() {
                    return Err(pending("callee has no actual Return publication site"));
                }
                row.extend([
                    2,
                    self.custody_shape(destination.shape())?,
                    self.frame(caller.function(), destination.frame())?,
                    self.slot(caller.function(), slot)?,
                    returning,
                    count(returns.len())?,
                ]);
                row.extend(returns);
            }
        }
        Ok(row)
    }
    fn encode(&self) -> Result<Vec<u8>, CodegenError> {
        let mut output = Vec::new();
        output.extend_from_slice(b"JTRSC001");
        word(&mut output, 2);
        word(&mut output, 0);
        output.extend_from_slice(&0u64.to_le_bytes());
        for size in [
            self.plan.program().resource_manifest.kinds().len(),
            self.hooks.len(),
            self.signatures.len(),
            self.shapes.len(),
            self.frames.len(),
            self.slots.len(),
            self.operations.len(),
        ] {
            word(&mut output, count(size)?);
        }
        word(&mut output, 0);
        for i in 0..self.plan.program().resource_manifest.kinds().len() {
            word(&mut output, ordinal(i)?);
        }
        for (i, (kind, recipe, signature)) in self.hooks.iter().enumerate() {
            words(&mut output, &[ordinal(i)?, *kind, *recipe, *signature]);
        }
        for (i, signature) in self.signatures.iter().enumerate() {
            words(
                &mut output,
                &[
                    ordinal(i)?,
                    count(signature.parameters.len())?,
                    signature.result,
                ],
            );
            for (shape, access) in &signature.parameters {
                words(&mut output, &[*shape, *access]);
            }
        }
        for (i, shape) in self.shapes.iter().enumerate() {
            word(&mut output, ordinal(i)?);
            words(&mut output, shape);
        }
        for (i, frame) in self.frames.iter().enumerate() {
            words(&mut output, &[ordinal(i)?, frame.role]);
            site(&mut output, frame.site)?;
            words(&mut output, &[frame.signature, count(frame.parents.len())?]);
            for parent in &frame.parents {
                match parent {
                    Parent::Root => words(&mut output, &[0, 0]),
                    Parent::Frame(index) => words(&mut output, &[1, *index]),
                }
            }
        }
        for (i, slot) in self.slots.iter().enumerate() {
            word(&mut output, ordinal(i)?);
            words(&mut output, slot);
        }
        for (i, (at, operation)) in self.operations.iter().enumerate() {
            word(&mut output, ordinal(i)?);
            site(&mut output, *at)?;
            words(&mut output, operation);
        }
        if output.len() > BYTE_LIMIT {
            return Err(pending(
                "layout byte capacity exceeds the bounded wire domain",
            ));
        }
        let length =
            u64::try_from(output.len()).map_err(|_| pending("layout length exceeds u64"))?;
        output[16..24].copy_from_slice(&length.to_le_bytes());
        Ok(output)
    }
}
fn staged_alias<'a>(
    function: &'a custody::ResourceFunctionPlan,
    operation: &custody::ResourceOperation,
) -> Result<Option<&'a custody::ResourceOperation>, CodegenError> {
    let mut prepared = function.operations().iter().filter(|candidate| {
        match (operation.role(), candidate.role()) {
            (Role::Borrow { loan }, Role::PrepareSourceBorrow { loan: expected, .. })
            | (Role::BorrowSum { loan }, Role::PrepareSourceSumBorrow { loan: expected, .. }) => {
                loan == expected
            }
            (Role::InvokeSourceFunction { .. }, Role::BeginSourceFunction { .. }) => {
                operation.frame() == candidate.frame()
            }
            _ => false,
        }
    });
    let Some(selected) = prepared.next() else {
        return Ok(None);
    };
    if prepared.next().is_some() || selected.frame() != operation.frame() {
        return Err(pending(
            "staged activation has ambiguous or foreign prepared metadata",
        ));
    }
    match (operation.role(), selected.role()) {
        (Role::Borrow { loan }, Role::PrepareSourceBorrow { parameter, .. })
        | (Role::BorrowSum { loan }, Role::PrepareSourceSumBorrow { parameter, .. }) => {
            let record = function
                .loans()
                .get(loan.index())
                .ok_or_else(|| pending("staged activation loan is absent"))?;
            if record.frame() != operation.frame() || record.parameter() != Some(*parameter) {
                return Err(pending("staged activation changes its sealed loan/formal"));
            }
        }
        (
            Role::InvokeSourceFunction {
                function: current,
                formals,
                evaluation_order,
                operands,
                result,
                ..
            },
            Role::BeginSourceFunction {
                function: expected,
                formals: originals,
                evaluation_order: order,
                operands: planned,
                result: output,
                ..
            },
        ) => {
            if current != expected
                || evaluation_order != order
                || result != output
                || formals.len() != originals.len()
                || operands.len() != planned.len()
                || !formals.iter().zip(originals).all(|(a, b)| {
                    a.parameter() == b.parameter()
                        && a.source_index() == b.source_index()
                        && a.actual_type() == b.actual_type()
                        && a.parameter_type() == b.parameter_type()
                        && a.syntax() == b.syntax()
                        && a.effect() == b.effect()
                        && a.access() == b.access()
                })
                || !operands
                    .iter()
                    .zip(planned)
                    .all(|(a, b)| same_operand(a, b))
            {
                return Err(pending(
                    "staged Invoke changes its complete prepared Source tuple",
                ));
            }
        }
        _ => return Err(pending("staged activation selected another prepared role")),
    }
    Ok(Some(selected))
}

fn same_operand(a: &Operand, b: &Operand) -> bool {
    match (a, b) {
        (
            Operand::Ordinary {
                parameter: p,
                ty: a,
            },
            Operand::Ordinary {
                parameter: q,
                ty: b,
            },
        ) => p == q && a == b,
        (
            Operand::Owned {
                parameter: p,
                slot: a,
            },
            Operand::Owned {
                parameter: q,
                slot: b,
            },
        ) => p == q && a == b,
        (
            Operand::Borrowed {
                parameter: p,
                loan: a,
            },
            Operand::Borrowed {
                parameter: q,
                loan: b,
            },
        ) => p == q && a == b,
        _ => false,
    }
}

fn emitted(role: &Role) -> bool {
    match role {
        Role::StageSourceActual { .. }
        | Role::BoundedBorrowUse { .. }
        | Role::SelfRebind { .. } => false,
        Role::InvokeHook { hook, .. } => {
            hook.recipe() == jett_types::ResourceKernelRecipe::NetworkBorrow
        }
        _ => true,
    }
}
fn word(output: &mut Vec<u8>, value: u32) {
    output.extend_from_slice(&value.to_le_bytes());
}
fn words(output: &mut Vec<u8>, values: &[u32]) {
    for value in values {
        word(output, *value);
    }
}
fn site(output: &mut Vec<u8>, at: custody::ResourceSite) -> Result<(), CodegenError> {
    words(output, &[at.function().index(), at.block().index()]);
    match at.position() {
        custody::ResourcePosition::Statement(index) => words(output, &[0, count(index)?]),
        custody::ResourcePosition::Terminator => words(output, &[1, 0]),
    };
    Ok(())
}

#[cfg(test)]
#[path = "resource_layout/tests.rs"]
mod tests;
