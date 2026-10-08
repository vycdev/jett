//! The ordinary descriptor selects only the constructor-proved Source body.
//! It never supplies a Resource family's hidden Scope through call_indirect.
use super::*;

fn descriptor_identity(
    builder: &mut FunctionBuilder<'_>,
    code: Value,
    environment: Value,
    expected: Value,
) -> Value {
    let selected = builder
        .ins()
        .icmp(ir::condcodes::IntCC::Equal, code, expected);
    let empty = builder
        .ins()
        .icmp_imm(ir::condcodes::IntCC::Equal, environment, 0);
    builder.ins().band(selected, empty)
}

impl Translator<'_, '_> {
    pub(in crate::emit) fn resource_is_named_value(
        &self,
        expression: &Expression,
    ) -> Result<bool, CodegenError> {
        let Some(resource) = self.resource else {
            return Ok(false);
        };
        let Some(proof) = resource.function_plan()?.named_callable_value(expression) else {
            return Ok(false);
        };
        let selected = program_function(resource.layout.plan().program(), proof.function())?;
        if proof.signature_type() != expression.ty
            || proof.identity() != &selected.identity
            || !matches!(self.types.resolve(expression.ty), Type::Function { .. })
        {
            return Err(pending(
                "ordinary named descriptor occurrence changes its sealed proof",
            ));
        }
        Ok(true)
    }

    pub(in crate::emit) fn resource_validate_named_producer(
        &self,
        expression: &Expression,
        target: FunctionId,
    ) -> Result<(), CodegenError> {
        let Some(resource) = self.resource else {
            return Ok(());
        };
        if resource.layout.plan().function(target).is_none() {
            return Ok(());
        }
        let proof = resource
            .function_plan()?
            .named_callable_producer(expression)
            .ok_or_else(|| {
                pending("Resource family descriptor has no exact current named producer")
            })?;
        let selected = program_function(resource.layout.plan().program(), target)?;
        if !matches!(expression.kind, ExpressionKind::FunctionRef(function) if function == target)
            || proof.function() != target
            || proof.signature_type() != expression.ty
            || proof.identity() != &selected.identity
        {
            return Err(pending(
                "Resource named descriptor changes its sealed producer",
            ));
        }
        Ok(())
    }

    pub(super) fn resource_validate_named_invocation(
        &self,
        invocation: &jett_mir::ResourceOperation,
        callee: Option<&Expression>,
    ) -> Result<(), CodegenError> {
        let Role::InvokeSourceFunction {
            function, source, ..
        } = invocation.role()
        else {
            return Ok(());
        };
        match (&source.target, callee, invocation.named_indirect_target()) {
            (jett_hir::CallTarget::Indirect { signature_type }, Some(callee), Some(proof)) => {
                let resource = self
                    .resource
                    .ok_or_else(|| pending("named Source call lost its family"))?;
                let selected = program_function(resource.layout.plan().program(), *function)?;
                if proof.function() != *function
                    || proof.signature_type() != *signature_type
                    || callee.ty != *signature_type
                    || proof.identity() != &selected.identity
                    || source.bridge != jett_hir::CallBridge::Direct
                    || !source.generated_operands.is_empty()
                {
                    return Err(pending(
                        "named indirect Source call changes its sealed body or tuple",
                    ));
                }
                Ok(())
            }
            (jett_hir::CallTarget::Indirect { .. }, _, _) => Err(pending(
                "indirect Source call has no exact current named target proof",
            )),
            (_, None, None) => Ok(()),
            _ => Err(pending(
                "named indirect target proof is attached to a different Source call",
            )),
        }
    }

    pub(super) fn resource_validate_named_descriptor(
        &mut self,
        descriptor: Value,
        invocation: &jett_mir::ResourceOperation,
        span: Span,
    ) -> Result<(), CodegenError> {
        let resource = self
            .resource
            .ok_or_else(|| pending("named descriptor has no current family"))?;
        let proof = invocation
            .named_indirect_target()
            .ok_or_else(|| pending("named descriptor has no exact invocation proof"))?;
        self.leaf(NativeLeaf::FunctionCallableCheck, &[descriptor], true)?;
        let code_index = self
            .builder
            .ins()
            .iconst(ir::types::I64, NATIVE_FUNCTION_CODE_FIELD as i64);
        let code = self.leaf(NativeLeaf::StructField, &[descriptor, code_index], true)?;
        let environment_index = self
            .builder
            .ins()
            .iconst(ir::types::I64, NATIVE_FUNCTION_ENVIRONMENT_FIELD as i64);
        let environment = self.leaf(
            NativeLeaf::StructField,
            &[descriptor, environment_index],
            true,
        )?;
        let declaration = self.declarations.get(proof.function()).ok_or_else(|| {
            contract_error(self.symbol, span, "named Resource target is not reachable")
        })?;
        let reference = self
            .module
            .declare_func_in_func(declaration.native_id, self.builder.func);
        let pointer = self.module.target_config().pointer_type();
        let expected = self.builder.ins().func_addr(pointer, reference);
        let expected = if pointer == ir::types::I64 {
            expected
        } else {
            self.builder.ins().uextend(ir::types::I64, expected)
        };
        let valid = descriptor_identity(self.builder, code, environment, expected);
        let accepted = self.builder.create_block();
        let refused = self.builder.ins().iconst(ir::types::I32, 1);
        self.builder
            .ins()
            .brif(valid, accepted, &[], resource.failure, &[refused.into()]);
        self.builder.switch_to_block(accepted);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cranelift_codegen::{control::ControlPlane, settings::Configurable};

    // Optimize the actual emitted predicate, rather than a Rust copy of it.
    // Constant inputs expose a code substitution and a nonempty environment
    // independently; these controls do not claim native Source execution.
    fn optimized_identity(code: i64, environment: i64, expected: i64) -> i64 {
        let mut flags = settings::builder();
        flags.set("opt_level", "speed").unwrap();
        let isa = isa::lookup(HOST)
            .unwrap()
            .finish(settings::Flags::new(flags))
            .unwrap();
        let mut context = Context::new();
        context.func.signature = ir::Signature::new(isa.default_call_conv());
        context
            .func
            .signature
            .returns
            .push(AbiParam::new(ir::types::I8));
        let mut frontend = FunctionBuilderContext::new();
        {
            let mut builder = FunctionBuilder::new(&mut context.func, &mut frontend);
            let entry = builder.create_block();
            builder.switch_to_block(entry);
            let code = builder.ins().iconst(ir::types::I64, code);
            let environment = builder.ins().iconst(ir::types::I64, environment);
            let expected = builder.ins().iconst(ir::types::I64, expected);
            let accepted = descriptor_identity(&mut builder, code, environment, expected);
            builder.ins().return_(&[accepted]);
            builder.seal_all_blocks();
            builder.finalize();
        }
        context.verify(isa.as_ref()).unwrap();
        context
            .optimize(isa.as_ref(), &mut ControlPlane::default())
            .unwrap();
        context.verify(isa.as_ref()).unwrap();
        let returned = context
            .func
            .layout
            .blocks()
            .flat_map(|block| context.func.layout.block_insts(block))
            .find(|instruction| context.func.dfg.insts[*instruction].opcode() == ir::Opcode::Return)
            .unwrap();
        let result = context
            .func
            .dfg
            .resolve_aliases(context.func.dfg.inst_args(returned)[0]);
        let ir::ValueDef::Result(instruction, _) = context.func.dfg.value_def(result) else {
            panic!("identity predicate did not reduce to a constant");
        };
        let ir::InstructionData::UnaryImm { imm, .. } = context.func.dfg.insts[instruction] else {
            panic!("identity predicate did not reduce to an immediate");
        };
        imm.bits()
    }

    #[test]
    fn resource_named_indirect_code_identity_and_environment_are_independent_guards() {
        assert_eq!(optimized_identity(701, 0, 701), 1);
        assert_eq!(optimized_identity(702, 0, 701), 0);
        assert_eq!(optimized_identity(701, 522, 701), 0);
        assert_eq!(optimized_identity(702, 522, 701), 0);
    }
}
