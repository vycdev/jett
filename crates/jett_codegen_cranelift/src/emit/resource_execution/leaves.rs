//! Exact selected039a imports for the dedicated Resource family.
//! No leaf declaration grants Source authority or creates a raw runtime key.
use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Leaf {
    EntryScope,
    EntryNetwork,
    EntryOutcome,
    ScopeValidate,
    OperationBegin,
    OperationComplete,
    HookPrepare,
    FactoryPrepare,
    FactoryCommit,
    BorrowBegin,
    BorrowCommit,
    BorrowEnd,
    Close,
    DescriptorClose,
    Descriptor,
    Transfer,
    DestinationFrame,
    SumAdopt,
    SumTag,
    SumBorrow,
    SumViewTag,
    SumViewProject,
    SumFailureRead,
    SumBorrowEnd,
    SumTake,
    SumDrop,
    FailureCompanionTake,
    AbsentSum,
    FailureSum,
    Replace,
    SourcePrepare,
    SourceActual,
    SourceEnter,
    SourceParameter,
    SourceStatus,
    ScopeComplete,
    ReturnPublish,
}
impl Leaf {
    fn spec(self) -> (&'static str, &'static [ir::Type]) {
        use Leaf::*;
        use ir::types::{I32, I64};
        match self {
            EntryScope => ("jett_rt_v1_resource_entry_scope", &[I64, I64, I64]),
            EntryNetwork => ("jett_rt_v1_resource_entry_network", &[I64, I32, I64, I64]),
            EntryOutcome => (
                "jett_rt_v1_resource_entry_outcome",
                &[I64, I64, I32, I32, I32, I64],
            ),
            ScopeValidate => (
                "jett_rt_v1_resource_scope_validate",
                &[I64, I64, I32, I32, I32],
            ),
            OperationBegin => ("jett_rt_v1_resource_operation_begin", &[I64, I64, I32, I64]),
            OperationComplete => ("jett_rt_v1_resource_operation_complete", &[I64, I64, I32]),
            HookPrepare => (
                "jett_rt_v1_resource_hook_prepare",
                &[I64, I64, I32, I64, I64],
            ),
            FactoryPrepare => ("jett_rt_v1_resource_factory_prepare", &[I64, I64, I64, I64]),
            FactoryCommit => (
                "jett_rt_v1_resource_factory_commit",
                &[I64, I64, I64, I64, I64],
            ),
            BorrowBegin => (
                "jett_rt_v1_resource_borrow_begin",
                &[I64, I64, I32, I64, I64],
            ),
            BorrowCommit => (
                "jett_rt_v1_resource_borrow_commit",
                &[I64, I64, I64, I64, I64],
            ),
            BorrowEnd => ("jett_rt_v1_resource_borrow_end", &[I64, I64, I32, I64]),
            Close => ("jett_rt_v1_resource_close", &[I64, I64, I32, I64]),
            DescriptorClose => ("jett_rt_v1_resource_descriptor_close", &[I64, I64, I64]),
            Descriptor => ("jett_rt_v1_resource_descriptor", &[I64, I32, I64]),
            Transfer => (
                "jett_rt_v1_resource_transfer",
                &[I64, I64, I32, I64, I64, I64],
            ),
            DestinationFrame => (
                "jett_rt_v1_resource_destination_frame",
                &[I64, I64, I32, I64],
            ),
            SumAdopt => (
                "jett_rt_v1_resource_sum_adopt",
                &[I64, I64, I32, I64, I64, I64],
            ),
            SumTag => ("jett_rt_v1_resource_sum_tag", &[I64, I64, I64, I64]),
            SumBorrow => ("jett_rt_v1_resource_sum_borrow", &[I64, I64, I32, I64, I64]),
            SumViewTag => (
                "jett_rt_v1_resource_sum_view_tag",
                &[I64, I64, I32, I64, I64],
            ),
            SumViewProject => (
                "jett_rt_v1_resource_sum_view_project",
                &[I64, I64, I32, I64, I64],
            ),
            SumFailureRead => (
                "jett_rt_v1_resource_sum_failure_read",
                &[I64, I64, I32, I64, I64],
            ),
            SumBorrowEnd => ("jett_rt_v1_resource_sum_borrow_end", &[I64, I64, I32, I64]),
            SumTake => (
                "jett_rt_v1_resource_sum_take",
                &[I64, I64, I32, I64, I64, I64],
            ),
            SumDrop => ("jett_rt_v1_resource_sum_drop", &[I64, I64, I32, I64]),
            FailureCompanionTake => (
                "jett_rt_v1_resource_failure_companion_take",
                &[I64, I64, I32, I64, I64],
            ),
            AbsentSum => ("jett_rt_v1_resource_absent_sum", &[I64, I64, I32, I64]),
            FailureSum => (
                "jett_rt_v1_resource_failure_sum",
                &[I64, I64, I32, I64, I64],
            ),
            Replace => (
                "jett_rt_v1_resource_replace",
                &[I64, I64, I32, I64, I64, I64],
            ),
            SourcePrepare => ("jett_rt_v1_resource_source_prepare", &[I64, I64, I32, I64]),
            SourceActual => (
                "jett_rt_v1_resource_source_actual",
                &[I64, I64, I32, I32, I64],
            ),
            SourceEnter => ("jett_rt_v1_resource_source_enter", &[I64, I64, I64]),
            SourceParameter => (
                "jett_rt_v1_resource_source_parameter",
                &[I64, I64, I32, I64],
            ),
            SourceStatus => ("jett_rt_v1_resource_source_status", &[I64, I64]),
            ScopeComplete => ("jett_rt_v1_resource_scope_complete", &[I64, I64, I32, I32]),
            ReturnPublish => (
                "jett_rt_v1_resource_return_publish",
                &[I64, I64, I32, I64, I64],
            ),
        }
    }
    pub(super) fn declare(
        self,
        module: &mut ObjectModule,
    ) -> Result<cranelift_module::FuncId, CodegenError> {
        if module.target_config().pointer_type() != ir::types::I64 {
            return Err(pending(
                "dedicated Resource leaves require selected64-bit context pointers",
            ));
        }
        let (symbol, params) = self.spec();
        let mut signature = module.make_signature();
        signature
            .params
            .extend(params.iter().copied().map(AbiParam::new));
        signature.returns.push(AbiParam::new(ir::types::I32));
        module
            .declare_function(symbol, Linkage::Import, &signature)
            .map_err(|error| CodegenError::Backend(format!("Resource leaf {symbol}: {error}")))
    }
    pub(super) fn call(
        self,
        module: &mut ObjectModule,
        builder: &mut FunctionBuilder<'_>,
        arguments: &[Value],
    ) -> Result<Value, CodegenError> {
        let (_, params) = self.spec();
        if arguments.len() != params.len()
            || arguments
                .iter()
                .zip(params)
                .any(|(value, ty)| builder.func.dfg.value_type(*value) != *ty)
        {
            return Err(pending(
                "Resource leaf actual ABI differs from its selected signature",
            ));
        }
        let id = self.declare(module)?;
        let reference = module.declare_func_in_func(id, builder.func);
        let call = builder.ins().call(reference, arguments);
        builder
            .inst_results(call)
            .first()
            .copied()
            .ok_or_else(|| pending("Resource leaf has no exact Status result"))
    }
    /// The failure block receives this exact Status and runs the full ordinary
    /// companion and Resource cleanup path. A success output is read only on0.
    pub(super) fn checked(
        self,
        module: &mut ObjectModule,
        builder: &mut FunctionBuilder<'_>,
        arguments: &[Value],
        failure: ir::Block,
    ) -> Result<(), CodegenError> {
        let status = self.call(module, builder, arguments)?;
        let accepted = builder.create_block();
        let success = builder.ins().icmp_imm(IntCC::Equal, status, 0);
        builder
            .ins()
            .brif(success, accepted, &[], failure, &[status.into()]);
        builder.switch_to_block(accepted);
        Ok(())
    }
    pub(super) fn output(
        self,
        module: &mut ObjectModule,
        builder: &mut FunctionBuilder<'_>,
        arguments: &[Value],
        failure: ir::Block,
        result_bytes: u32,
    ) -> Result<Value, CodegenError> {
        if !matches!(result_bytes, 4 | 8 | 16) || (result_bytes == 4) != (self == Leaf::SumViewTag)
        {
            return Err(pending(
                "Resource out-storage differs from its exact selected leaf layout",
            ));
        }
        let output_type = if result_bytes == 4 {
            ir::types::I32
        } else {
            ir::types::I64
        };
        let slot = builder.create_sized_stack_slot(ir::StackSlotData::new(
            ir::StackSlotKind::ExplicitSlot,
            result_bytes,
            3,
        ));
        let zero = builder.ins().iconst(output_type, 0);
        builder.ins().stack_store(zero, slot, 0);
        if result_bytes == 16 {
            builder.ins().stack_store(zero, slot, 8);
        }
        let address = builder.ins().stack_addr(ir::types::I64, slot, 0);
        let mut arguments = arguments.to_vec();
        arguments.push(address);
        self.checked(module, builder, &arguments, failure)?;
        Ok(builder
            .ins()
            .stack_load(output_type, slot, if result_bytes == 16 { 8 } else { 0 }))
    }
}
