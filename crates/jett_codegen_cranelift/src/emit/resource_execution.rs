//! Dedicated Source family. The layout constructor owns all entry and custody decisions.
use super::resource_layout::{EmittedResourceLayout, pending};
use super::*;
use jett_mir::{
    ResourceCallOperand as Operand, ResourceCallResult as CallResult,
    ResourceFrameRole as FrameRole, ResourceOperationRole as Role, ResourceSlotStorage as Storage,
};
mod leaves;
#[cfg(test)]
mod tests;
use leaves::Leaf;

#[derive(Clone, Copy)]
pub(super) struct ResourceEmission<'a> {
    pub(super) layout: &'a EmittedResourceLayout<'a>,
    pub(super) function: FunctionId,
    pub(super) scope: Variable,
    pub(super) status: Variable,
    pub(super) frames: &'a [ir::StackSlot],
    pub(super) owners: &'a [ir::StackSlot],
    pub(super) return_companion: ir::StackSlot,
    pub(super) scope_finished: ir::StackSlot,
    pub(super) loans: &'a [ir::StackSlot],
    pub(super) failure: ir::Block,
    pub(super) block: jett_mir::BlockId,
    pub(super) position: jett_mir::ResourcePosition,
}
impl<'a> ResourceEmission<'a> {
    fn function_plan(self) -> Result<&'a jett_mir::ResourceFunctionPlan, CodegenError> {
        self.layout
            .plan()
            .function(self.function)
            .ok_or_else(|| pending("native family lost its exact function plan"))
    }
    fn at_site(self) -> Result<Vec<&'a jett_mir::ResourceOperation>, CodegenError> {
        Ok(self
            .function_plan()?
            .operations()
            .iter()
            .filter(|operation| {
                operation.site().block() == self.block
                    && operation.site().position() == self.position
            })
            .collect())
    }
    fn ordinal(self, operation: &jett_mir::ResourceOperation) -> Result<u32, CodegenError> {
        self.layout
            .operation(self.function, operation.id())
            .ok_or_else(|| pending("native operation has no exact installed row"))
    }
    fn local_slot(
        self,
        local: jett_mir::LocalId,
    ) -> Result<Option<jett_mir::ResourceOwnerSlotId>, CodegenError> {
        Ok(self
            .function_plan()?
            .owner_slots()
            .iter()
            .find_map(|slot| match slot.storage() {
                Storage::Local { header } if header.id == local => Some(slot.id()),
                _ => None,
            }))
    }
}

pub(super) fn clif_type(
    layout: Option<&EmittedResourceLayout<'_>>,
    types: &TypeInterner,
    ty: TypeId,
    role: &str,
) -> Result<Option<ir::Type>, CodegenError> {
    if layout.is_some_and(|layout| layout.plan().type_requires_custody(ty)) {
        Ok(Some(ir::types::I64))
    } else {
        super::clif_type(types, ty, role)
    }
}
pub(super) fn task_scalar(
    layout: Option<&EmittedResourceLayout<'_>>,
    types: &TypeInterner,
    ty: TypeId,
) -> Result<bool, CodegenError> {
    if layout.is_some_and(|layout| layout.plan().type_requires_custody(ty)) {
        Ok(false)
    } else {
        is_task_scalar(types, ty)
    }
}
fn signature(
    module: &ObjectModule,
    function: &Function,
    types: &TypeInterner,
    layout: &EmittedResourceLayout<'_>,
) -> Result<ir::Signature, CodegenError> {
    if layout.plan().function(function.id).is_none() {
        return super::signature(module, function, types);
    }
    let mut signature = module.make_signature();
    signature.params.extend([
        AbiParam::new(ir::types::I64),
        AbiParam::new(ir::types::I64),
        AbiParam::new(ir::types::I64),
    ]);
    for parameter in &function.params {
        if let Some(ty) = clif_type(
            Some(layout),
            types,
            parameter.ty,
            "Resource-family parameter",
        )? {
            signature.params.push(AbiParam::new(ty));
            if task_scalar(Some(layout), types, parameter.ty)? {
                signature.params.push(AbiParam::new(ir::types::I64));
            }
        }
    }
    if let Some(ty) = clif_type(
        Some(layout),
        types,
        function.return_type,
        "Resource-family return",
    )? {
        signature.returns.push(AbiParam::new(ty));
        if task_scalar(Some(layout), types, function.return_type)? {
            signature.returns.push(AbiParam::new(ir::types::I64));
        }
    }
    Ok(signature)
}

pub(super) fn emit(
    program: &Program,
    types: &TypeInterner,
    target: Triple,
    entry: Option<FunctionId>,
    options: CodegenOptions,
) -> Result<ObjectArtifact, CodegenError> {
    let entry = entry.ok_or_else(|| {
        pending("Resource executable requires the exact driver-selected entry FunctionId")
    })?;
    jett_mir::validate(program).map_err(CodegenError::InvalidMir)?;
    for function in &program.functions {
        jett_mir::move_values::validate_local_view_initializers(function, types).map_err(
            |message| contract_error(&function.identity.declaration.name, function.span, message),
        )?;
    }
    let original_plan =
        jett_mir::validate_resource_ownership(program, types).map_err(CodegenError::InvalidMir)?;
    crate::verify::verify_resource_descriptor_bodies(&original_plan)?;
    let mut prepared = program.clone();
    jett_mir::prepare_native_sequences(&mut prepared, types);
    jett_mir::prepare_native_uninhabited_sums(&mut prepared, types);
    jett_mir::prepare_native_generated_functions(&mut prepared, types);
    let program = &prepared;
    let layout = EmittedResourceLayout::from_program(program, types, entry)?;
    let verified = crate::verify::verify_resource_program(layout.plan())?;
    let mut flags = settings::builder();
    if options.optimize {
        use cranelift_codegen::settings::Configurable;
        flags
            .set("opt_level", "speed")
            .map_err(|error| CodegenError::Backend(error.to_string()))?;
    }
    let isa = isa::lookup(target.clone())
        .map_err(|error| CodegenError::Backend(error.to_string()))?
        .finish(settings::Flags::new(flags))
        .map_err(|error| CodegenError::Backend(error.to_string()))?;
    let mut module = ObjectModule::new(
        ObjectBuilder::new(isa, b"jett-resource".to_vec(), default_libcall_names())
            .map_err(|error| CodegenError::Backend(error.to_string()))?,
    );
    let mut declarations =
        DeclaredFunctions::new(program.functions.len(), verified.functions().len());
    for verified in verified.functions() {
        let function = program_function(program, verified.mir_id)?;
        let signature = signature(&module, function, types, &layout)?;
        let native_id = module
            .declare_function(&verified.symbol, Linkage::Local, &signature)
            .map_err(|error| CodegenError::Backend(error.to_string()))?;
        declarations.insert(DeclaredFunction {
            mir_id: function.id,
            native_id,
            symbol: verified.symbol.clone(),
            signature,
            modes: function.params.iter().map(|param| param.mode).collect(),
            parameter_types: function.params.iter().map(|param| param.ty).collect(),
            debug_label: debug::function_label(function).map_err(|message| pending(message))?,
        })?;
    }
    for declaration in declarations.iter() {
        let function = program_function(program, declaration.mir_id)?;
        let mut context = module.make_context();
        context.func.signature = declaration.signature.clone();
        let selected = layout
            .plan()
            .function(function.id)
            .is_some()
            .then_some(&layout);
        translate_function_inner(
            &mut module,
            &declarations,
            program,
            function,
            types,
            &declaration.symbol,
            &mut context,
            selected,
        )?;
        module
            .define_function(declaration.native_id, &mut context)
            .map_err(|error| {
                CodegenError::Backend(format!("Resource-family {}: {error:?}", declaration.symbol))
            })?;
    }
    layout.define_accessor(&mut module)?;
    define_entry(&mut module, &declarations, &layout)?;
    let mut symbols = declarations.symbols();
    symbols.push("jett_aot_resource_v1_manifest".into());
    symbols.push(JETT_AOT_ENTRY_SYMBOL_V1.into());
    let bytes = module
        .finish()
        .emit()
        .map_err(|error| CodegenError::Backend(error.to_string()))?;
    Ok(ObjectArtifact {
        target: target.to_string(),
        symbols,
        bytes,
    })
}

impl Translator<'_, '_> {
    pub(super) fn resource_validate_scope(&mut self) -> Result<(), CodegenError> {
        let resource = self
            .resource
            .ok_or_else(|| pending("Scope prologue has no selected family"))?;
        let scope = self.builder.use_var(resource.scope);
        let context = self.builder.use_var(self.runtime_context);
        let function = self
            .builder
            .ins()
            .iconst(ir::types::I32, i64::from(resource.function.index()));
        let signature = self.builder.ins().iconst(
            ir::types::I32,
            i64::from(
                resource
                    .layout
                    .function_signature(resource.function)
                    .ok_or_else(|| pending("Scope lacks its containing function signature"))?,
            ),
        );
        let template = self.builder.ins().iconst(
            ir::types::I32,
            i64::from(
                resource
                    .layout
                    .frame(
                        resource.function,
                        resource.function_plan()?.root_scope().id(),
                    )
                    .ok_or_else(|| pending("Scope template missing"))?,
            ),
        );
        Leaf::ScopeValidate.checked(
            self.module,
            self.builder,
            &[context, scope, function, signature, template],
            resource.failure,
        )
    }
    pub(super) fn resource_bind_parameters(
        &mut self,
        function: &Function,
    ) -> Result<(), CodegenError> {
        let Some(resource) = self.resource else {
            return Ok(());
        };
        for parameter in &function.params {
            if let Some(slot) = resource.local_slot(parameter.local)? {
                let variable = self.variables[parameter.local.index() as usize]
                    .ok_or_else(|| pending("Resource formal has no carrier variable"))?;
                let value = self.builder.use_var(variable);
                self.builder
                    .ins()
                    .stack_store(value, resource.owners[slot.index()], 0);
            }
        }
        Ok(())
    }
    fn resource_context(&mut self) -> Value {
        self.builder.use_var(self.runtime_context)
    }
    fn resource_frame(&mut self, id: jett_mir::ResourceFrameId) -> Result<Value, CodegenError> {
        let resource = self
            .resource
            .ok_or_else(|| pending("frame lookup has no fresh native family"))?;
        let header = resource
            .function_plan()?
            .frames()
            .get(id.index())
            .ok_or_else(|| pending("frame lookup is outside the fresh plan"))?;
        match header.role() {
            FrameRole::Scope | FrameRole::Return => Ok(self.builder.use_var(resource.scope)),
            FrameRole::Operation => {
                Ok(self
                    .builder
                    .ins()
                    .stack_load(ir::types::I64, resource.frames[id.index()], 0))
            }
        }
    }
    fn resource_owner(&mut self, id: jett_mir::ResourceOwnerSlotId) -> Result<Value, CodegenError> {
        let resource = self
            .resource
            .ok_or_else(|| pending("owner lookup has no fresh native family"))?;
        let slot = resource
            .owners
            .get(id.index())
            .copied()
            .ok_or_else(|| pending("owner storage is outside the exact plan"))?;
        Ok(self.builder.ins().stack_load(ir::types::I64, slot, 0))
    }
    fn resource_store(
        &mut self,
        id: jett_mir::ResourceOwnerSlotId,
        value: Value,
    ) -> Result<(), CodegenError> {
        let resource = self
            .resource
            .ok_or_else(|| pending("owner publication has no fresh native family"))?;
        self.builder
            .ins()
            .stack_store(value, resource.owners[id.index()], 0);
        Ok(())
    }
    fn resource_operation(
        &mut self,
        operation: &jett_mir::ResourceOperation,
    ) -> Result<(Value, Value, Value), CodegenError> {
        let resource = self
            .resource
            .ok_or_else(|| pending("operation has no fresh native family"))?;
        let context = self.resource_context();
        let frame = self.resource_frame(operation.frame())?;
        let ordinal = self
            .builder
            .ins()
            .iconst(ir::types::I32, i64::from(resource.ordinal(operation)?));
        Ok((context, frame, ordinal))
    }
    fn resource_destination(
        &mut self,
        operation: &jett_mir::ResourceOperation,
    ) -> Result<Value, CodegenError> {
        let resource = self
            .resource
            .ok_or_else(|| pending("destination has no fresh native family"))?;
        let (context, frame, ordinal) = self.resource_operation(operation)?;
        Leaf::DestinationFrame.output(
            self.module,
            self.builder,
            &[context, frame, ordinal],
            resource.failure,
            8,
        )
    }
    fn resource_transfer(
        &mut self,
        operation: &jett_mir::ResourceOperation,
    ) -> Result<Value, CodegenError> {
        let resource = self
            .resource
            .ok_or_else(|| pending("transfer has no fresh native family"))?;
        let (source, destination) = match operation.role() {
            Role::Transfer {
                source,
                destination,
            } => (*source, *destination),
            _ => return Err(pending("transfer selected a different exact role")),
        };
        let value = self.resource_owner(source)?;
        let target = self.resource_destination(operation)?;
        let (context, frame, ordinal) = self.resource_operation(operation)?;
        let value = Leaf::Transfer.output(
            self.module,
            self.builder,
            &[context, frame, ordinal, value, target],
            resource.failure,
            8,
        )?;
        self.clear_slot(resource.owners[source.index()]);
        self.resource_store(destination, value)?;
        Ok(value)
    }
    fn resource_loan(&mut self, id: jett_mir::ResourceLoanId) -> Result<Value, CodegenError> {
        let resource = self
            .resource
            .ok_or_else(|| pending("loan lookup has no fresh native family"))?;
        match resource.function_plan()?.loans()[id.index()].source() {
            jett_mir::ResourceLoanSource::Owner(_) => {
                Ok(self
                    .builder
                    .ins()
                    .stack_load(ir::types::I64, resource.loans[id.index()], 0))
            }
            jett_mir::ResourceLoanSource::IncomingViewFormal { parameter, .. } => {
                let header = &resource.function_plan()?.parameters()[parameter];
                let variable = self.variables[header.local.index() as usize]
                    .ok_or_else(|| pending("resident formal has no exact variable"))?;
                Ok(self.builder.use_var(variable))
            }
        }
    }
    fn resource_begin_borrow(
        &mut self,
        operation: &jett_mir::ResourceOperation,
    ) -> Result<Value, CodegenError> {
        let resource = self
            .resource
            .ok_or_else(|| pending("borrow has no fresh native family"))?;
        let Role::Borrow { loan } = operation.role() else {
            return Err(pending("borrow selected another role"));
        };
        let jett_mir::ResourceLoanSource::Owner(source) =
            resource.function_plan()?.loans()[loan.index()].source()
        else {
            return Err(pending(
                "resident forwarding cannot create a child core lease",
            ));
        };
        let owner = self.resource_owner(source)?;
        let (context, frame, ordinal) = self.resource_operation(operation)?;
        let value = Leaf::BorrowBegin.output(
            self.module,
            self.builder,
            &[context, frame, ordinal, owner],
            resource.failure,
            8,
        )?;
        self.builder
            .ins()
            .stack_store(value, resource.loans[loan.index()], 0);
        Ok(value)
    }
}

impl Translator<'_, '_> {
    fn resource_open_operation(
        &mut self,
        frame: jett_mir::ResourceFrameId,
    ) -> Result<(), CodegenError> {
        let resource = self
            .resource
            .ok_or_else(|| pending("operation has no selected family"))?;
        let header = &resource.function_plan()?.frames()[frame.index()];
        if header.role() != FrameRole::Operation {
            return Err(pending("invocation did not select its Operation frame"));
        }
        let parent = self.resource_frame(
            header
                .parent()
                .ok_or_else(|| pending("Operation frame has no parent"))?,
        )?;
        let template = resource
            .layout
            .frame(resource.function, frame)
            .ok_or_else(|| pending("Operation template missing"))?;
        let template = self
            .builder
            .ins()
            .iconst(ir::types::I32, i64::from(template));
        let context = self.resource_context();
        let value = Leaf::OperationBegin.output(
            self.module,
            self.builder,
            &[context, parent, template],
            resource.failure,
            8,
        )?;
        self.builder
            .ins()
            .stack_store(value, resource.frames[frame.index()], 0);
        Ok(())
    }
    fn resource_drop(
        &mut self,
        operation: &jett_mir::ResourceOperation,
    ) -> Result<(), CodegenError> {
        let resource = self
            .resource
            .ok_or_else(|| pending("drop has no selected family"))?;
        let Role::Drop { source, .. } = operation.role() else {
            return Err(pending("drop selected another role"));
        };
        let owner = self.resource_owner(*source)?;
        let (context, frame, ordinal) = self.resource_operation(operation)?;
        // A plain owner and a conditional shell have separate runtime protocols.
        let leaf = if !matches!(
            resource.function_plan()?.owner_slots()[source.index()].shape(),
            jett_mir::ResourceShape::Plain { .. }
        ) {
            Leaf::SumDrop
        } else {
            Leaf::Close
        };
        leaf.checked(
            self.module,
            self.builder,
            &[context, frame, ordinal, owner],
            resource.failure,
        )?;
        self.clear_slot(resource.owners[source.index()]);
        Ok(())
    }
    fn resource_finish_operation(
        &mut self,
        operations: &[&jett_mir::ResourceOperation],
        frame: jett_mir::ResourceFrameId,
    ) -> Result<(), CodegenError> {
        let resource = self
            .resource
            .ok_or_else(|| pending("completion has no selected family"))?;
        for operation in operations
            .iter()
            .copied()
            .filter(|operation| operation.frame() == frame)
        {
            match operation.role() {
                Role::EndBorrow { loan } => {
                    let value = self.resource_loan(*loan)?;
                    let (context, frame, ordinal) = self.resource_operation(operation)?;
                    Leaf::BorrowEnd.checked(
                        self.module,
                        self.builder,
                        &[context, frame, ordinal, value],
                        resource.failure,
                    )?;
                    self.clear_slot(resource.loans[loan.index()]);
                }
                Role::Drop { .. } => self.resource_drop(operation)?,
                Role::Complete { .. } => {
                    let (context, active, ordinal) = self.resource_operation(operation)?;
                    // Clear compiler storage before the fallible retirement. Runtime owns any refusal.
                    self.clear_slot(resource.frames[frame.index()]);
                    Leaf::OperationComplete.checked(
                        self.module,
                        self.builder,
                        &[context, active, ordinal],
                        resource.failure,
                    )?;
                }
                _ => {}
            }
        }
        Ok(())
    }
    fn resource_stage_actual(
        &mut self,
        operations: &[&jett_mir::ResourceOperation],
        frame: jett_mir::ResourceFrameId,
        parameter: usize,
    ) -> Result<(), CodegenError> {
        let resource = self
            .resource
            .ok_or_else(|| pending("actual staging has no selected family"))?;
        for operation in operations
            .iter()
            .copied()
            .filter(|operation| operation.frame() == frame)
        {
            match operation.role() {
                Role::Transfer { destination, .. } if matches!(resource.function_plan()?.owner_slots()[destination.index()].storage(), Storage::Argument { parameter: selected, .. } if *selected == parameter) =>
                {
                    self.resource_transfer(operation)?;
                }
                Role::Borrow { loan } => {
                    let for_parameter = operations.iter().any(|operation| match operation.role() {
                        Role::InvokeHook { operands, .. } | Role::InvokeSourceFunction { operands, .. } => operands.iter().any(|operand| matches!(operand, Operand::Borrowed { parameter: selected, loan: expected } if *selected == parameter && expected == loan)),
                        _ => false,
                    });
                    if for_parameter {
                        self.resource_begin_borrow(operation)?;
                    }
                }
                _ => {}
            }
        }
        Ok(())
    }
    fn resource_operand(
        &mut self,
        operand: &Operand,
        ordinary: &[Option<LoweredValue>],
        _span: Span,
    ) -> Result<Value, CodegenError> {
        match operand {
            Operand::Owned { slot, .. } => self.resource_owner(*slot),
            Operand::Borrowed { loan, .. } => self.resource_loan(*loan),
            Operand::Ordinary { parameter, .. } => {
                let value =
                    ordinary.get(*parameter).copied().flatten().ok_or_else(|| {
                        pending("ordinary actual has not completed in source order")
                    })?;
                Ok(self.payload_bits(value).0)
            }
        }
    }
    fn resource_invoke(
        &mut self,
        expression: &Expression,
        args: &[Expression],
        invocation: &jett_mir::ResourceOperation,
        operations: &[&jett_mir::ResourceOperation],
        descriptor: Option<&Expression>,
    ) -> Result<LoweredValue, CodegenError> {
        let resource = self
            .resource
            .ok_or_else(|| pending("invocation has no selected family"))?;
        let (order, operands, result, source) = match invocation.role() {
            Role::InvokeHook {
                evaluation_order,
                operands,
                result,
                source,
                ..
            }
            | Role::InvokeSourceFunction {
                evaluation_order,
                operands,
                result,
                source,
                ..
            } => (
                evaluation_order.as_slice(),
                operands.as_slice(),
                *result,
                source,
            ),
            _ => return Err(pending("invocation selected another exact role")),
        };
        let indirect_hook = descriptor.is_some();
        let descriptor = if let Some(callee) = descriptor {
            let value = self.expression(callee)?;
            self.scalar(value, callee.span)?
        } else {
            self.builder.ins().iconst(ir::types::I64, 0)
        };
        self.resource_open_operation(invocation.frame())?;
        let physical = match invocation.role() {
            Role::InvokeHook { hook, .. }
                if hook.recipe() == jett_types::ResourceKernelRecipe::NetworkFactory =>
            {
                operations
                    .iter()
                    .copied()
                    .find(|operation| {
                        operation.frame() == invocation.frame()
                            && matches!(operation.role(), Role::Acquire { .. })
                    })
                    .ok_or_else(|| pending("Factory has no exact Acquire wire role"))?
            }
            Role::InvokeHook { hook, .. }
                if hook.recipe() == jett_types::ResourceKernelRecipe::Finalize =>
            {
                operations
                    .iter()
                    .copied()
                    .find(|operation| {
                        operation.frame() == invocation.frame()
                            && matches!(operation.role(), Role::Close { .. })
                    })
                    .ok_or_else(|| pending("Finalize has no exact Close wire role"))?
            }
            _ => invocation,
        };
        if indirect_hook {
            return Err(pending(
                "indirect Resource hook requires its exact descriptor-target wire row",
            ));
        }
        let (context, frame, ordinal) = self.resource_operation(physical)?;
        let call = match invocation.role() {
            Role::InvokeSourceFunction { .. } => Leaf::SourcePrepare.output(
                self.module,
                self.builder,
                &[context, frame, ordinal],
                resource.failure,
                8,
            )?,
            Role::InvokeHook { .. } => Leaf::HookPrepare.output(
                self.module,
                self.builder,
                &[context, frame, ordinal, descriptor],
                resource.failure,
                8,
            )?,
            _ => unreachable!(),
        };
        let mut evaluated = vec![None; args.len()];
        for &parameter in order {
            let argument = args
                .get(parameter)
                .ok_or_else(|| pending("exact invocation actual is absent"))?;
            let fact = source
                .arguments
                .iter()
                .find(|fact| fact.parameter_index == parameter)
                .ok_or_else(|| pending("invocation lost an original Source formal"))?;
            let value = if resource.layout.plan().type_requires_custody(argument.ty) {
                self.expression(argument)?
            } else {
                self.argument(
                    argument,
                    resource_argument_view(invocation, parameter, self.types)?,
                )?
            };
            evaluated[parameter] = Some(value);
            self.resource_stage_actual(operations, invocation.frame(), parameter)?;
            if matches!(invocation.role(), Role::InvokeSourceFunction { .. }) {
                let operand = operands
                    .iter()
                    .find(|operand| match operand {
                        Operand::Ordinary {
                            parameter: selected,
                            ..
                        }
                        | Operand::Owned {
                            parameter: selected,
                            ..
                        }
                        | Operand::Borrowed {
                            parameter: selected,
                            ..
                        } => *selected == parameter,
                    })
                    .ok_or_else(|| pending("Source actual has no exact operand role"))?;
                let bits = self.resource_operand(operand, &evaluated, argument.span)?;
                let source_index = self
                    .builder
                    .ins()
                    .iconst(ir::types::I32, fact.source_index as i64);
                let parameter = self.builder.ins().iconst(ir::types::I32, parameter as i64);
                Leaf::SourceActual.checked(
                    self.module,
                    self.builder,
                    &[context, call, source_index, parameter, bits],
                    resource.failure,
                )?;
            }
        }
        let result_value = match invocation.role() {
            Role::InvokeHook { hook, .. } => {
                let find = |parameter| {
                    operands
                        .iter()
                        .find(|operand| match operand {
                            Operand::Ordinary {
                                parameter: selected,
                                ..
                            }
                            | Operand::Owned {
                                parameter: selected,
                                ..
                            }
                            | Operand::Borrowed {
                                parameter: selected,
                                ..
                            } => *selected == parameter,
                        })
                        .ok_or_else(|| pending("hook recipe lost an exact operand"))
                };
                match hook.recipe() {
                    jett_types::ResourceKernelRecipe::NetworkFactory => {
                        let net = self.resource_operand(find(0)?, &evaluated, expression.span)?;
                        let label = self.resource_operand(find(1)?, &evaluated, expression.span)?;
                        let prepared = Leaf::FactoryPrepare.output(
                            self.module,
                            self.builder,
                            &[context, call, net],
                            resource.failure,
                            8,
                        )?;
                        Leaf::FactoryCommit.output(
                            self.module,
                            self.builder,
                            &[context, prepared, net, label],
                            resource.failure,
                            16,
                        )?
                    }
                    jett_types::ResourceKernelRecipe::NetworkBorrow => {
                        let net = self.resource_operand(find(0)?, &evaluated, expression.span)?;
                        let loan = self.resource_operand(find(1)?, &evaluated, expression.span)?;
                        Leaf::BorrowCommit.output(
                            self.module,
                            self.builder,
                            &[context, call, net, loan],
                            resource.failure,
                            8,
                        )?
                    }
                    jett_types::ResourceKernelRecipe::Finalize => {
                        let close = operations
                            .iter()
                            .copied()
                            .find(|operation| {
                                operation.frame() == invocation.frame()
                                    && matches!(operation.role(), Role::Close { .. })
                            })
                            .ok_or_else(|| pending("Finalize has no exact consumed owner role"))?;
                        let Role::Close { source, .. } = close.role() else {
                            unreachable!();
                        };
                        let owner = self.resource_owner(*source)?;
                        if indirect_hook {
                            Leaf::DescriptorClose.checked(
                                self.module,
                                self.builder,
                                &[context, call, owner],
                                resource.failure,
                            )?;
                        } else {
                            let (context, frame, ordinal) = self.resource_operation(close)?;
                            Leaf::Close.checked(
                                self.module,
                                self.builder,
                                &[context, frame, ordinal, owner],
                                resource.failure,
                            )?;
                        }
                        self.clear_slot(resource.owners[source.index()]);
                        self.builder.ins().iconst(ir::types::I64, 0)
                    }
                }
            }
            Role::InvokeSourceFunction { function, .. } => {
                let scope = Leaf::SourceEnter.output(
                    self.module,
                    self.builder,
                    &[context, call],
                    resource.failure,
                    8,
                )?;
                let mut native = vec![context, self.builder.ins().iconst(ir::types::I64, 0), scope];
                for (parameter, argument) in args.iter().enumerate() {
                    let index = self.builder.ins().iconst(ir::types::I32, parameter as i64);
                    let installed = Leaf::SourceParameter.output(
                        self.module,
                        self.builder,
                        &[context, scope, index],
                        resource.failure,
                        8,
                    )?;
                    if resource.layout.plan().type_requires_custody(argument.ty) {
                        native.push(installed);
                    } else {
                        let unpacked =
                            self.unpack_payload(installed, argument.ty, argument.span)?;
                        native.push(self.scalar(unpacked, argument.span)?);
                        if task_scalar(Some(resource.layout), self.types, argument.ty)? {
                            native.push(
                                self.scalar_task(
                                    evaluated[parameter].ok_or_else(|| {
                                        pending("ordinary Source argument is absent")
                                    })?,
                                    argument.span,
                                )?
                                .1,
                            );
                        }
                        if !resource_argument_view(invocation, parameter, self.types)?
                            && is_linear(self.types, argument.ty)
                        {
                            if let Some(LoweredValue::Owned(_, slot)) = evaluated[parameter] {
                                self.clear_slot(slot);
                            }
                        }
                    }
                }
                let declared = self
                    .declarations
                    .get(*function)
                    .ok_or_else(|| pending("Source family callee is absent"))?;
                let reference = self
                    .module
                    .declare_func_in_func(declared.native_id, self.builder.func);
                let native_call = self.builder.ins().call(reference, &native);
                let results = self.builder.inst_results(native_call).to_vec();
                Leaf::SourceStatus.checked(
                    self.module,
                    self.builder,
                    &[context, call],
                    resource.failure,
                )?;
                self.check_failure()?;
                let bits = results
                    .first()
                    .copied()
                    .ok_or_else(|| pending("Source family call has no domain result"))?;
                // Primitive Pending depth remains part of the ordinary result ABI.
                if task_scalar(Some(resource.layout), self.types, expression.ty)? {
                    let depth = results.get(1).copied().ok_or_else(|| {
                        pending("ordinary Source primitive lost its Pending depth")
                    })?;
                    self.resource_finish_operation(operations, invocation.frame())?;
                    return Ok(LoweredValue::ScalarTask(bits, depth));
                }
                bits
            }
            _ => unreachable!(),
        };
        let lowered = match result {
            CallResult::Owned { slot } => {
                self.resource_store(slot, result_value)?;
                LoweredValue::Scalar(result_value)
            }
            CallResult::Ordinary { ty } if is_linear(self.types, ty) => {
                self.own_linear(result_value)?
            }
            CallResult::Ordinary { ty } if is_copy_owned(self.types, ty) => {
                self.own(result_value)?
            }
            CallResult::Ordinary { .. } => LoweredValue::Scalar(result_value),
        };
        self.resource_finish_operation(operations, invocation.frame())?;
        Ok(lowered)
    }
    pub(super) fn resource_expression(
        &mut self,
        expression: &Expression,
    ) -> Result<Option<LoweredValue>, CodegenError> {
        let Some(resource) = self.resource else {
            return Ok(None);
        };
        let operations = resource
            .function_plan()?
            .operations_for_expression(expression)
            .collect::<Vec<_>>();
        let invocation = operations.iter().copied().find(|operation| {
            matches!(
                operation.role(),
                Role::InvokeHook { .. } | Role::InvokeSourceFunction { .. }
            )
        });
        if let Some(invocation) = invocation {
            let (args, descriptor) = match &expression.kind {
                ExpressionKind::Call { args, .. } | ExpressionKind::ResourceInvoke { args, .. } => {
                    (args.as_slice(), None)
                }
                ExpressionKind::IndirectCall { callee, args, .. } => {
                    (args.as_slice(), Some(callee.as_ref()))
                }
                _ => return Err(pending("exact invocation role changed its expression kind")),
            };
            return self
                .resource_invoke(expression, args, invocation, &operations, descriptor)
                .map(Some);
        }
        if operations.iter().any(|operation| {
            operation.is_expression_operation()
                && matches!(
                    operation.role(),
                    Role::Complete {
                        outcome: jett_mir::ResourceCompletion::Abort
                    }
                )
                && resource.function_plan().is_ok_and(|plan| {
                    plan.frames()[operation.frame().index()].role() == FrameRole::Operation
                })
        }) {
            return Err(pending(
                "statically aborted Resource actual needs its exact custody CFG emitter",
            ));
        }
        if !resource.layout.plan().type_requires_custody(expression.ty) {
            return Ok(None);
        }
        let value = match &expression.kind {
            ExpressionKind::Local(local) => {
                if let Some(slot) = resource.local_slot(*local)? {
                    self.resource_owner(slot)?
                } else {
                    let variable = self.variables[local.index() as usize]
                        .ok_or_else(|| pending("resident Resource has no exact native variable"))?;
                    self.builder.use_var(variable)
                }
            }
            ExpressionKind::View(inner) => return self.expression(inner).map(Some),
            ExpressionKind::ResourceHookValue { .. } => {
                let operation = operations
                    .iter()
                    .copied()
                    .find(|operation| matches!(operation.role(), Role::Descriptor { .. }))
                    .ok_or_else(|| pending("hook value lost its exact descriptor row"))?;
                let context = self.resource_context();
                let ordinal = self
                    .builder
                    .ins()
                    .iconst(ir::types::I32, i64::from(resource.ordinal(operation)?));
                Leaf::Descriptor.output(
                    self.module,
                    self.builder,
                    &[context, ordinal],
                    resource.failure,
                    8,
                )?
            }
            ExpressionKind::OptionalSome(inner) | ExpressionKind::ResultOk(inner) => {
                self.expression(inner)?;
                let operation = operations
                    .iter()
                    .copied()
                    .find(|operation| matches!(operation.role(), Role::SumAdopt { .. }))
                    .ok_or_else(|| pending("occupied Resource sum lost its exact adoption"))?;
                let Role::SumAdopt {
                    source,
                    destination,
                } = operation.role()
                else {
                    unreachable!();
                };
                let owner = self.resource_owner(*source)?;
                let target = self.resource_destination(operation)?;
                let (context, frame, ordinal) = self.resource_operation(operation)?;
                let value = Leaf::SumAdopt.output(
                    self.module,
                    self.builder,
                    &[context, frame, ordinal, owner, target],
                    resource.failure,
                    8,
                )?;
                self.clear_slot(resource.owners[source.index()]);
                self.resource_store(*destination, value)?;
                value
            }
            ExpressionKind::OptionalNone => {
                let operation = operations
                    .iter()
                    .copied()
                    .find(|operation| matches!(operation.role(), Role::CreateAbsentSum { .. }))
                    .ok_or_else(|| pending("absent Resource sum lost its exact constructor"))?;
                let Role::CreateAbsentSum { destination } = operation.role() else {
                    unreachable!();
                };
                let (context, frame, ordinal) = self.resource_operation(operation)?;
                let value = Leaf::AbsentSum.output(
                    self.module,
                    self.builder,
                    &[context, frame, ordinal],
                    resource.failure,
                    8,
                )?;
                self.resource_store(*destination, value)?;
                value
            }
            ExpressionKind::ResultFail(inner) => {
                let evaluated = self.expression(inner)?;
                let operation = operations
                    .iter()
                    .copied()
                    .find(|operation| matches!(operation.role(), Role::CreateFailureSum { .. }))
                    .ok_or_else(|| {
                        pending("Resource Fail lost its exact ordinary companion constructor")
                    })?;
                let Role::CreateFailureSum { destination, .. } = operation.role() else {
                    unreachable!();
                };
                let (context, frame, ordinal) = self.resource_operation(operation)?;
                let owned = self.own_copy_value(evaluated, inner.ty, inner.span)?;
                let companion = self.scalar(owned, inner.span)?;
                let value = Leaf::FailureSum.output(
                    self.module,
                    self.builder,
                    &[context, frame, ordinal, companion],
                    resource.failure,
                    8,
                )?;
                if let LoweredValue::Owned(_, slot) = owned {
                    self.clear_slot(slot);
                }
                self.resource_store(*destination, value)?;
                value
            }
            _ => {
                return Err(pending(
                    "native Resource expression requires its next finite custody emitter",
                ));
            }
        };
        Ok(Some(LoweredValue::Scalar(value)))
    }
}

fn resource_argument_view(
    invocation: &jett_mir::ResourceOperation,
    parameter: usize,
    types: &TypeInterner,
) -> Result<bool, CodegenError> {
    match invocation.role() {
        Role::InvokeHook { hook, .. } => match types.resolve(hook.function_type()) {
            Type::Function { view_params, .. } => view_params
                .get(parameter)
                .copied()
                .ok_or_else(|| pending("hook physical access has no exact formal")),
            _ => Err(pending("hook has no exact checked physical signature")),
        },
        Role::InvokeSourceFunction { formals, .. } => formals
            .iter()
            .find(|formal| formal.parameter() == parameter)
            .map(|formal| formal.access() == jett_mir::ParamMode::View)
            .ok_or_else(|| pending("Source call physical access has no exact original formal")),
        _ => Err(pending("formal access requires an exact invocation role")),
    }
}

impl Translator<'_, '_> {
    pub(super) fn resource_statement(
        &mut self,
        statement: &Statement,
    ) -> Result<bool, CodegenError> {
        let Some(resource) = self.resource else {
            return Ok(false);
        };
        let operations = resource.at_site()?;
        let site_only = operations
            .iter()
            .copied()
            .filter(|operation| !operation.is_expression_operation())
            .collect::<Vec<_>>();
        match &statement.kind {
            StatementKind::Let { local, value }
                if resource.layout.plan().type_requires_custody(value.ty) =>
            {
                let evaluated = self.expression(value)?;
                let bits = if let Some(destination) = resource.local_slot(*local)? {
                    let operation = site_only.iter().copied().find(|operation| matches!(operation.role(), Role::Transfer { destination: selected, .. } if *selected == destination));
                    if let Some(operation) = operation {
                        self.resource_transfer(operation)?
                    } else {
                        self.scalar(evaluated, value.span)?
                    }
                } else {
                    if let Some(operation) = site_only
                        .iter()
                        .copied()
                        .find(|operation| matches!(operation.role(), Role::Borrow { .. }))
                    {
                        self.resource_begin_borrow(operation)?
                    } else {
                        self.scalar(evaluated, value.span)?
                    }
                };
                self.define_local(*local, LoweredValue::Scalar(bits), statement.span)?;
                Ok(true)
            }
            StatementKind::Assign { target, value }
                if resource.layout.plan().type_requires_custody(target.ty) =>
            {
                let ExpressionKind::Local(local) = target.kind else {
                    return Err(pending(
                        "Resource projected assignment transport is pending",
                    ));
                };
                self.expression(value)?;
                let destination = resource
                    .local_slot(local)?
                    .ok_or_else(|| pending("Resource assignment lost its owning local"))?;
                let transfer = site_only.iter().copied().find(|operation| matches!(operation.role(), Role::Transfer { destination: selected, .. } if *selected == destination)).ok_or_else(|| pending("Resource replacement has no exact RHS transfer"))?;
                let bits = if let Some(replace) = site_only.iter().copied().find(|operation| matches!(operation.role(), Role::Replace { destination: selected, .. } if *selected == destination)) {
                    let Role::Transfer { source, .. } = transfer.role() else { unreachable!(); };
                    let old = self.resource_owner(destination)?; let rhs = self.resource_owner(*source)?;
                    let (context, frame, ordinal) = self.resource_operation(replace)?;
                    let bits = Leaf::Replace.output(self.module, self.builder, &[context, frame, ordinal, old, rhs], resource.failure, 8)?;
                    self.clear_slot(resource.owners[source.index()]); self.resource_store(destination, bits)?; bits
                } else { self.resource_transfer(transfer)? };
                self.define_local(local, LoweredValue::Scalar(bits), statement.span)?;
                Ok(true)
            }
            StatementKind::Evaluate(expression) | StatementKind::HandleDefault(expression)
                if resource.layout.plan().type_requires_custody(expression.ty) =>
            {
                self.expression(expression)?;
                for operation in site_only {
                    if matches!(operation.role(), Role::Drop { .. }) {
                        self.resource_drop(operation)?;
                    }
                }
                Ok(true)
            }
            StatementKind::SumTag { source, target }
                if resource
                    .layout
                    .plan()
                    .type_requires_custody(self.local_types[source.index() as usize].ty) =>
            {
                let slot = resource
                    .local_slot(*source)?
                    .ok_or_else(|| pending("Resource sum tag has no exact owner"))?;
                let value = self.resource_owner(slot)?;
                let context = self.resource_context();
                let frame = self.builder.use_var(resource.scope);
                let output = self.builder.create_sized_stack_slot(ir::StackSlotData::new(
                    ir::StackSlotKind::ExplicitSlot,
                    4,
                    2,
                ));
                let zero = self.builder.ins().iconst(ir::types::I32, 0);
                self.builder.ins().stack_store(zero, output, 0);
                let address = self.builder.ins().stack_addr(ir::types::I64, output, 0);
                Leaf::SumTag.checked(
                    self.module,
                    self.builder,
                    &[context, frame, value, address],
                    resource.failure,
                )?;
                let tag = self.builder.ins().stack_load(ir::types::I32, output, 0);
                let tag = self.builder.ins().ireduce(ir::types::I8, tag);
                self.define_local(*target, LoweredValue::Scalar(tag), statement.span)?;
                Ok(true)
            }
            StatementKind::SumTake {
                source,
                target,
                success,
            } if resource
                .layout
                .plan()
                .type_requires_custody(self.local_types[source.index() as usize].ty) =>
            {
                let operation = operations
                    .iter()
                    .copied()
                    .find(|operation| {
                        if *success {
                            matches!(operation.role(), Role::SumTake { .. })
                        } else {
                            matches!(operation.role(), Role::TakeFailureCompanion { .. })
                        }
                    })
                    .ok_or_else(|| {
                        pending("Resource sum extraction lost its exact selected-edge role")
                    })?;
                let source_slot = resource
                    .local_slot(*source)?
                    .ok_or_else(|| pending("Resource sum extraction lost its shell owner"))?;
                let value = self.resource_owner(source_slot)?;
                let (context, frame, ordinal) = self.resource_operation(operation)?;
                let lowered = if let Role::SumTake { destination, .. } = operation.role() {
                    let destination_frame = self.resource_destination(operation)?;
                    let bits = Leaf::SumTake.output(
                        self.module,
                        self.builder,
                        &[context, frame, ordinal, value, destination_frame],
                        resource.failure,
                        8,
                    )?;
                    self.resource_store(*destination, bits)?;
                    LoweredValue::Scalar(bits)
                } else {
                    let bits = Leaf::FailureCompanionTake.output(
                        self.module,
                        self.builder,
                        &[context, frame, ordinal, value],
                        resource.failure,
                        8,
                    )?;
                    self.own(bits)?
                };
                self.clear_slot(resource.owners[source_slot.index()]);
                self.define_local(*target, lowered, statement.span)?;
                Ok(true)
            }
            _ => Ok(false),
        }
    }
    fn resource_scope_completion(
        &mut self,
        operation: &jett_mir::ResourceOperation,
        body: Value,
        checked: bool,
    ) -> Result<(), CodegenError> {
        let resource = self
            .resource
            .ok_or_else(|| pending("completion has no selected family"))?;
        let context = self.resource_context();
        let scope = self.builder.use_var(resource.scope);
        let ordinal = self
            .builder
            .ins()
            .iconst(ir::types::I32, i64::from(resource.ordinal(operation)?));
        let one = self.builder.ins().iconst(ir::types::I64, 1);
        self.builder
            .ins()
            .stack_store(one, resource.scope_finished, 0);
        if checked {
            Leaf::ScopeComplete.checked(
                self.module,
                self.builder,
                &[context, scope, ordinal, body],
                resource.failure,
            )
        } else {
            Leaf::ScopeComplete
                .call(self.module, self.builder, &[context, scope, ordinal, body])
                .map(|_| ())
        }
    }
    pub(super) fn resource_return(
        &mut self,
        expression: Option<&Expression>,
        span: Span,
    ) -> Result<(), CodegenError> {
        let resource = self
            .resource
            .ok_or_else(|| pending("return has no selected family"))?;
        let operations = resource.at_site()?;
        let mut result = if let Some(expression) = expression {
            self.expression(expression)?
        } else {
            self.nothing()
        };
        let completion = operations
            .iter()
            .copied()
            .find(|operation| {
                operation.frame()
                    == resource
                        .function_plan()
                        .expect("selected plan")
                        .root_scope()
                        .id()
                    && matches!(operation.role(), Role::Complete { .. })
            })
            .ok_or_else(|| pending("return has no exact Scope completion"))?;
        let publication = operations
            .iter()
            .copied()
            .find(|operation| matches!(operation.role(), Role::CompleteReturnAfterCleanup { .. }));
        if let Some(publication) = publication {
            let Role::CompleteReturnAfterCleanup { source } = publication.role() else {
                unreachable!();
            };
            let transfer = operations.iter().copied().find(|operation| matches!(operation.role(), Role::Transfer { destination, .. } if destination == source)).ok_or_else(|| pending("owned return has no exact provisional transfer"))?;
            self.resource_transfer(transfer)?;
        } else if expression.is_some_and(|expression| is_copy_owned(self.types, expression.ty)) {
            let value = self.scalar(result, span)?;
            let leaf =
                if expression.is_some_and(|expression| is_function(self.types, expression.ty)) {
                    NativeLeaf::StructClone
                } else {
                    NativeLeaf::Retain
                };
            let retained = self.leaf(leaf, &[value], true)?;
            self.builder
                .ins()
                .stack_store(retained, resource.return_companion, 0);
            result = LoweredValue::Scalar(retained);
        } else if let LoweredValue::Owned(value, slot) = result {
            // Preserve the ordinary owner through fallible Scope cleanup.
            self.builder
                .ins()
                .stack_store(value, resource.return_companion, 0);
            self.clear_slot(slot);
            result = LoweredValue::Scalar(value);
        }
        self.flush_actor_state(span)?;
        self.drop_all()?;
        let body = self.builder.ins().iconst(ir::types::I32, 0);
        self.resource_scope_completion(completion, body, true)?;
        if let Some(publication) = publication {
            let Role::CompleteReturnAfterCleanup { source } = publication.role() else {
                unreachable!();
            };
            let value = self.resource_owner(*source)?;
            let context = self.resource_context();
            let scope = self.builder.use_var(resource.scope);
            let ordinal = self
                .builder
                .ins()
                .iconst(ir::types::I32, i64::from(resource.ordinal(publication)?));
            let result = Leaf::ReturnPublish.output(
                self.module,
                self.builder,
                &[context, scope, ordinal, value],
                resource.failure,
                8,
            )?;
            self.clear_slot(resource.owners[source.index()]);
            self.builder.ins().return_(&[result]);
        } else if expression.is_some_and(|expression| {
            task_scalar(Some(resource.layout), self.types, expression.ty).unwrap_or(false)
        }) {
            let (bits, depth) = self.scalar_task(result, span)?;
            self.builder.ins().return_(&[bits, depth]);
        } else {
            let bits = self.scalar(result, span)?;
            self.clear_slot(resource.return_companion);
            self.builder.ins().return_(&[bits]);
        }
        Ok(())
    }
    pub(super) fn resource_failure_cleanup(&mut self) -> Result<(), CodegenError> {
        let Some(resource) = self.resource else {
            return Ok(());
        };
        // Ordinary return custody remains ordinary even when a Resource cleanup fails.
        self.drop_slot(resource.return_companion)?;
        for header in resource
            .function_plan()?
            .frames()
            .iter()
            .rev()
            .filter(|header| header.role() == FrameRole::Operation)
        {
            let active = self.builder.ins().stack_load(
                ir::types::I64,
                resource.frames[header.id().index()],
                0,
            );
            let retire = self.builder.create_block();
            let next = self.builder.create_block();
            let live = self.builder.ins().icmp_imm(IntCC::NotEqual, active, 0);
            self.builder.ins().brif(live, retire, &[], next, &[]);
            self.builder.switch_to_block(retire);
            let operation = resource
                .function_plan()?
                .operations()
                .iter()
                .find(|operation| {
                    operation.frame() == header.id()
                        && matches!(operation.role(), Role::Complete { .. })
                })
                .ok_or_else(|| pending("active Operation has no sealed completion row"))?;
            let context = self.resource_context();
            let ordinal = self
                .builder
                .ins()
                .iconst(ir::types::I32, i64::from(resource.ordinal(operation)?));
            self.clear_slot(resource.frames[header.id().index()]);
            // Attempt all enclosing retirement even after the first infrastructure failure.
            Leaf::OperationComplete.call(self.module, self.builder, &[context, active, ordinal])?;
            self.builder.ins().jump(next, &[]);
            self.builder.switch_to_block(next);
        }
        let finished = self
            .builder
            .ins()
            .stack_load(ir::types::I64, resource.scope_finished, 0);
        let retire = self.builder.create_block();
        let next = self.builder.create_block();
        let live = self.builder.ins().icmp_imm(IntCC::Equal, finished, 0);
        self.builder.ins().brif(live, retire, &[], next, &[]);
        self.builder.switch_to_block(retire);
        let operation = resource
            .function_plan()?
            .operations()
            .iter()
            .find(|operation| {
                operation.frame()
                    == resource
                        .function_plan()
                        .expect("selected plan")
                        .root_scope()
                        .id()
                    && matches!(operation.role(), Role::Complete { .. })
            })
            .ok_or_else(|| pending("failure Scope has no original completion projection"))?;
        let body = self.builder.use_var(resource.status);
        self.resource_scope_completion(operation, body, false)?;
        self.builder.ins().jump(next, &[]);
        self.builder.switch_to_block(next);
        Ok(())
    }
}

/// One C entry wrapper, selecting exactly the installed root and original body status.
fn define_entry(
    module: &mut ObjectModule,
    declarations: &DeclaredFunctions,
    layout: &EmittedResourceLayout<'_>,
) -> Result<(), CodegenError> {
    let selected = layout.entry();
    let entry = declarations
        .get(FunctionId::new(selected.function))
        .ok_or_else(|| pending("driver entry has no declared native family"))?;
    let signature = program_entry_signature(module);
    let id = module
        .declare_function(JETT_AOT_ENTRY_SYMBOL_V1, Linkage::Export, &signature)
        .map_err(|error| CodegenError::Backend(error.to_string()))?;
    let mut context = module.make_context();
    context.func.signature = signature;
    let mut frontend = FunctionBuilderContext::new();
    let mut builder = FunctionBuilder::new(&mut context.func, &mut frontend);
    let initial = builder.create_block();
    builder.append_block_params_for_function_params(initial);
    builder.switch_to_block(initial);
    let ctx = builder.block_params(initial)[0];
    let refused = builder.create_block();
    builder.append_block_param(refused, ir::types::I32);
    let scope_output = builder.create_sized_stack_slot(ir::StackSlotData::new(
        ir::StackSlotKind::ExplicitSlot,
        8,
        3,
    ));
    let result_output = builder.create_sized_stack_slot(ir::StackSlotData::new(
        ir::StackSlotKind::ExplicitSlot,
        24,
        3,
    ));
    let zero = builder.ins().iconst(ir::types::I64, 0);
    builder.ins().stack_store(zero, scope_output, 0);
    builder.ins().stack_store(zero, result_output, 0);
    builder.ins().stack_store(zero, result_output, 8);
    builder.ins().stack_store(zero, result_output, 16);
    let scope_address = builder.ins().stack_addr(ir::types::I64, scope_output, 0);
    let result_address = builder.ins().stack_addr(ir::types::I64, result_output, 0);
    Leaf::EntryScope.checked(
        module,
        &mut builder,
        &[ctx, scope_address, result_address],
        refused,
    )?;
    let root = builder.ins().stack_load(ir::types::I64, scope_output, 0);
    let mut native = vec![ctx, zero, root];
    for (parameter, ty) in entry.parameter_types.iter().enumerate() {
        if *ty != TypeInterner::NETWORK {
            return Err(pending(
                "native root capability differs from the exact selected Network entry",
            ));
        }
        let output = builder.create_sized_stack_slot(ir::StackSlotData::new(
            ir::StackSlotKind::ExplicitSlot,
            8,
            3,
        ));
        builder.ins().stack_store(zero, output, 0);
        builder.ins().stack_store(zero, result_output, 0);
        builder.ins().stack_store(zero, result_output, 8);
        builder.ins().stack_store(zero, result_output, 16);
        let address = builder.ins().stack_addr(ir::types::I64, output, 0);
        let parameter = builder.ins().iconst(ir::types::I32, parameter as i64);
        Leaf::EntryNetwork.checked(
            module,
            &mut builder,
            &[ctx, parameter, address, result_address],
            refused,
        )?;
        native.push(builder.ins().stack_load(ir::types::I64, output, 0));
    }
    let reference = module.declare_func_in_func(entry.native_id, builder.func);
    builder.ins().call(reference, &native);
    let output = builder.create_sized_stack_slot(ir::StackSlotData::new(
        ir::StackSlotKind::ExplicitSlot,
        4,
        2,
    ));
    let zero32 = builder.ins().iconst(ir::types::I32, 0);
    builder.ins().stack_store(zero32, output, 0);
    let function = builder
        .ins()
        .iconst(ir::types::I32, i64::from(selected.function));
    let signature = builder
        .ins()
        .iconst(ir::types::I32, i64::from(selected.signature));
    let template = builder
        .ins()
        .iconst(ir::types::I32, i64::from(selected.scope));
    let address = builder.ins().stack_addr(ir::types::I64, output, 0);
    Leaf::EntryOutcome.checked(
        module,
        &mut builder,
        &[ctx, root, function, signature, template, address],
        refused,
    )?;
    let body_status = builder.ins().stack_load(ir::types::I32, output, 0);
    builder.ins().return_(&[body_status]);
    builder.switch_to_block(refused);
    let status = builder.block_params(refused)[0];
    builder.ins().return_(&[status]);
    builder.seal_all_blocks();
    builder.finalize();
    module
        .define_function(id, &mut context)
        .map_err(|error| CodegenError::Backend(format!("Resource C entry: {error:?}")))
}
