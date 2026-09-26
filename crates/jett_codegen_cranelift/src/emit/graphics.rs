use super::*;

impl Translator<'_, '_> {
    fn graphics_callback(
        &mut self,
        descriptor: Value,
        params: &[TypeId],
        result_type: TypeId,
        arguments: &[Option<Value>],
        transfers: &[ir::StackSlot],
        span: Span,
    ) -> Result<LoweredValue, CodegenError> {
        if params.len() != arguments.len() {
            return Err(contract_error(
                self.symbol,
                span,
                "Graphics callback arity changed",
            ));
        }
        let code_index = self
            .builder
            .ins()
            .iconst(ir::types::I64, NATIVE_FUNCTION_CODE_FIELD as i64);
        let address = self.leaf(NativeLeaf::StructField, &[descriptor, code_index], true)?;
        let environment_index = self
            .builder
            .ins()
            .iconst(ir::types::I64, NATIVE_FUNCTION_ENVIRONMENT_FIELD as i64);
        let environment = self.leaf(
            NativeLeaf::StructField,
            &[descriptor, environment_index],
            true,
        )?;
        let pointer_type = self.module.target_config().pointer_type();
        let address = if pointer_type == ir::types::I64 {
            address
        } else {
            self.builder.ins().ireduce(pointer_type, address)
        };
        let mut signature = self.module.make_signature();
        signature
            .params
            .push(runtime_context_abi_param(self.module));
        signature.params.push(AbiParam::new(ir::types::I64));
        let mut native_args = vec![self.builder.use_var(self.runtime_context), environment];
        for (parameter, argument) in params.iter().zip(arguments) {
            if let Some(ty) = clif_type(self.types, *parameter, "Graphics callback parameter")? {
                signature.params.push(AbiParam::new(ty));
                native_args.push(argument.ok_or_else(|| {
                    contract_error(self.symbol, span, "Graphics callback value is absent")
                })?);
            } else if argument.is_some() {
                return Err(contract_error(
                    self.symbol,
                    span,
                    "nothing callback argument has a value",
                ));
            }
        }
        if let Some(ty) = clif_type(self.types, result_type, "Graphics callback result")? {
            signature.returns.push(AbiParam::new(ty));
        }
        let signature = self.builder.import_signature(signature);
        for &slot in transfers {
            self.clear_slot(slot);
        }
        let call = self
            .builder
            .ins()
            .call_indirect(signature, address, &native_args);
        let results = self.builder.func.dfg.inst_results(call).to_vec();
        self.check_failure()?;
        let value = results.first().copied().ok_or_else(|| {
            contract_error(self.symbol, span, "Graphics callback produced no value")
        })?;
        if is_linear(self.types, result_type) {
            self.own_linear(value)
        } else if is_copy_owned(self.types, result_type) {
            self.own(value)
        } else {
            Ok(LoweredValue::Scalar(value))
        }
    }

    fn graphics_finish(
        &mut self,
        value: LoweredValue,
        result_slot: ir::StackSlot,
        result_var: Variable,
        done: ir::Block,
        span: Span,
    ) -> Result<(), CodegenError> {
        let handle = self.scalar(value, span)?;
        self.builder.ins().stack_store(handle, result_slot, 0);
        if let LoweredValue::Owned(_, slot) = value {
            self.clear_slot(slot);
        }
        self.builder.def_var(result_var, handle);
        self.builder.ins().jump(done, &[]);
        Ok(())
    }

    fn graphics_close_session(&mut self, session: Value) -> Result<(), CodegenError> {
        self.leaf(NativeLeaf::GraphicsClose, &[session], true)?;
        Ok(())
    }

    fn graphics_failure_result(
        &mut self,
        value: LoweredValue,
        session: Option<Value>,
        result_slot: ir::StackSlot,
        result_var: Variable,
        done: ir::Block,
        span: Span,
    ) -> Result<(), CodegenError> {
        if let Some(session) = session {
            self.graphics_close_session(session)?;
        }
        let handle = self.scalar(value, span)?;
        let failure = self.builder.ins().iconst(
            ir::types::I32,
            i64::from(jett_runtime::native_abi::values::SUM_FAILURE),
        );
        let message = self.leaf(NativeLeaf::SumTake, &[handle, failure], true)?;
        if let LoweredValue::Owned(_, slot) = value {
            self.clear_slot(slot);
        }
        let message = self.own(message)?;
        let message_handle = self.scalar(message, span)?;
        let owns = self.builder.ins().iconst(ir::types::I32, 1);
        let result = self.leaf(NativeLeaf::SumNew, &[failure, message_handle, owns], true)?;
        if let LoweredValue::Owned(_, slot) = message {
            self.clear_slot(slot);
        }
        self.graphics_finish(
            LoweredValue::Scalar(result),
            result_slot,
            result_var,
            done,
            span,
        )
    }

    fn graphics_tag(&mut self, value: LoweredValue, span: Span) -> Result<Value, CodegenError> {
        let handle = self.scalar(value, span)?;
        self.leaf(NativeLeaf::SumTag, &[handle], true)
    }

    pub(super) fn graphics_run(
        &mut self,
        type_arguments: &[TypeId],
        args: &[Expression],
        evaluated: &[LoweredValue],
        span: Span,
    ) -> Result<LoweredValue, CodegenError> {
        let [state_type] = type_arguments else {
            return Err(contract_error(
                self.symbol,
                span,
                "Graphics state type is absent",
            ));
        };
        let [_, config_arg, _, update_arg, render_arg] = args else {
            return Err(contract_error(
                self.symbol,
                span,
                "Graphics call arity changed",
            ));
        };
        let [display, config, initial, update, render] = evaluated else {
            return Err(contract_error(
                self.symbol,
                span,
                "Graphics operands are absent",
            ));
        };
        let Type::Function {
            params: update_params,
            return_type: update_result,
            ..
        } = self.types.resolve(update_arg.ty)
        else {
            return Err(contract_error(
                self.symbol,
                span,
                "Graphics update is not a function",
            ));
        };
        let update_params = update_params.clone();
        let update_result = *update_result;
        let Type::Function {
            params: render_params,
            return_type: scene_type,
            ..
        } = self.types.resolve(render_arg.ty)
        else {
            return Err(contract_error(
                self.symbol,
                span,
                "Graphics render is not a function",
            ));
        };
        let render_params = render_params.clone();
        let scene_type = *scene_type;
        let authority = self.scalar(*display, span)?;
        let config_handle = self.scalar(*config, config_arg.span)?;
        let update_descriptor = self.scalar(*update, update_arg.span)?;
        let render_descriptor = self.scalar(*render, render_arg.span)?;
        let zero = self.builder.ins().iconst(ir::types::I64, 0);
        let width_index = self.builder.ins().iconst(ir::types::I64, 1);
        let height_index = self.builder.ins().iconst(ir::types::I64, 2);
        let width = self.leaf(NativeLeaf::StructField, &[config_handle, width_index], true)?;
        let height = self.leaf(
            NativeLeaf::StructField,
            &[config_handle, height_index],
            true,
        )?;

        let result_slot = *self
            .temporary_slots
            .get(self.next_temporary)
            .ok_or_else(|| {
                contract_error(
                    self.symbol,
                    span,
                    "Graphics result ownership slot is absent",
                )
            })?;
        self.next_temporary += 1;
        let result_var = self.builder.declare_var(ir::types::I64);
        let done = self.builder.create_block();

        let state_var = clif_type(self.types, *state_type, "Graphics state")?
            .map(|ty| self.builder.declare_var(ty));
        let state_slot = match initial {
            LoweredValue::Owned(_, slot) => Some(*slot),
            _ => None,
        };
        if let Some(variable) = state_var {
            self.builder.def_var(variable, self.scalar(*initial, span)?);
        }

        let validated_config =
            self.leaf(NativeLeaf::GraphicsValidateConfig, &[config_handle], true)?;
        let validated_config = self.own_linear(validated_config)?;
        let config_ok = self.builder.create_block();
        let config_bad = self.builder.create_block();
        let tag = self.graphics_tag(validated_config, span)?;
        self.builder
            .ins()
            .brif(tag, config_ok, &[], config_bad, &[]);
        self.builder.switch_to_block(config_bad);
        self.graphics_finish(validated_config, result_slot, result_var, done, span)?;
        self.builder.switch_to_block(config_ok);
        if let LoweredValue::Owned(_, slot) = validated_config {
            self.drop_slot(slot)?;
        }

        let current_state = state_var.map(|variable| self.builder.use_var(variable));
        let first_scene = self.graphics_callback(
            render_descriptor,
            &render_params,
            scene_type,
            &[current_state],
            &[],
            span,
        )?;
        let first_scene_handle = self.scalar(first_scene, span)?;
        let validated_scene = self.leaf(
            NativeLeaf::GraphicsValidateScene,
            &[width, height, first_scene_handle],
            true,
        )?;
        let validated_scene = self.own_linear(validated_scene)?;
        let scene_ok = self.builder.create_block();
        let scene_bad = self.builder.create_block();
        let tag = self.graphics_tag(validated_scene, span)?;
        self.builder.ins().brif(tag, scene_ok, &[], scene_bad, &[]);
        self.builder.switch_to_block(scene_bad);
        self.graphics_finish(validated_scene, result_slot, result_var, done, span)?;
        self.builder.switch_to_block(scene_ok);
        if let LoweredValue::Owned(_, slot) = validated_scene {
            self.drop_slot(slot)?;
        }

        let opened = self.leaf(NativeLeaf::GraphicsOpen, &[authority, config_handle], true)?;
        let opened = self.own_linear(opened)?;
        let open_ok = self.builder.create_block();
        let open_bad = self.builder.create_block();
        let tag = self.graphics_tag(opened, span)?;
        self.builder.ins().brif(tag, open_ok, &[], open_bad, &[]);
        self.builder.switch_to_block(open_bad);
        self.graphics_failure_result(opened, None, result_slot, result_var, done, span)?;
        self.builder.switch_to_block(open_ok);
        let success = self.builder.ins().iconst(
            ir::types::I32,
            i64::from(jett_runtime::native_abi::values::SUM_SUCCESS),
        );
        let session = self.leaf(
            NativeLeaf::SumTake,
            &[self.scalar(opened, span)?, success],
            true,
        )?;
        if let LoweredValue::Owned(_, slot) = opened {
            self.clear_slot(slot);
        }

        let presented = self.leaf(
            NativeLeaf::GraphicsPresent,
            &[session, first_scene_handle],
            true,
        )?;
        let presented = self.own_linear(presented)?;
        let first_present_ok = self.builder.create_block();
        let first_present_bad = self.builder.create_block();
        let tag = self.graphics_tag(presented, span)?;
        self.builder
            .ins()
            .brif(tag, first_present_ok, &[], first_present_bad, &[]);
        self.builder.switch_to_block(first_present_bad);
        self.graphics_close_session(session)?;
        self.graphics_finish(presented, result_slot, result_var, done, span)?;
        self.builder.switch_to_block(first_present_ok);
        if let LoweredValue::Owned(_, slot) = presented {
            self.drop_slot(slot)?;
        }
        if let LoweredValue::Owned(_, slot) = first_scene {
            self.drop_slot(slot)?;
        }

        let loop_head = self.builder.create_block();
        self.builder.ins().jump(loop_head, &[]);
        self.builder.switch_to_block(loop_head);
        let next = self.leaf(NativeLeaf::GraphicsNextKey, &[session], true)?;
        let next = self.own_linear(next)?;
        let next_ok = self.builder.create_block();
        let next_bad = self.builder.create_block();
        let tag = self.graphics_tag(next, span)?;
        self.builder.ins().brif(tag, next_ok, &[], next_bad, &[]);
        self.builder.switch_to_block(next_bad);
        self.graphics_failure_result(next, Some(session), result_slot, result_var, done, span)?;
        self.builder.switch_to_block(next_ok);
        let optional = self.leaf(
            NativeLeaf::SumTake,
            &[self.scalar(next, span)?, success],
            true,
        )?;
        if let LoweredValue::Owned(_, slot) = next {
            self.clear_slot(slot);
        }
        let optional = self.own_linear(optional)?;
        let key_ready = self.builder.create_block();
        let closed = self.builder.create_block();
        let tag = self.graphics_tag(optional, span)?;
        self.builder.ins().brif(tag, key_ready, &[], closed, &[]);
        self.builder.switch_to_block(closed);
        if let LoweredValue::Owned(_, slot) = optional {
            self.drop_slot(slot)?;
        }
        self.graphics_close_session(session)?;
        let nothing = self.builder.ins().iconst(ir::types::I64, 0);
        let borrowed = self.builder.ins().iconst(ir::types::I32, 0);
        let completed = self.leaf(NativeLeaf::SumNew, &[success, nothing, borrowed], true)?;
        self.graphics_finish(
            LoweredValue::Scalar(completed),
            result_slot,
            result_var,
            done,
            span,
        )?;

        self.builder.switch_to_block(key_ready);
        let borrowed = self.builder.ins().iconst(ir::types::I32, 0);
        let key_index = self.leaf(
            NativeLeaf::SumTake,
            &[self.scalar(optional, span)?, success],
            true,
        )?;
        if let LoweredValue::Owned(_, slot) = optional {
            self.clear_slot(slot);
        }
        let one = self.builder.ins().iconst(ir::types::I64, 1);
        let key = self.leaf(NativeLeaf::StructNew, &[one], true)?;
        let key = self.own_linear(key)?;
        let key_handle = self.scalar(key, span)?;
        self.leaf(
            NativeLeaf::StructInit,
            &[key_handle, zero, key_index, borrowed],
            true,
        )?;
        let mut transfers = Vec::new();
        if is_linear(self.types, *state_type) {
            transfers.push(state_slot.ok_or_else(|| {
                contract_error(
                    self.symbol,
                    span,
                    "linear Graphics state has no owning slot",
                )
            })?);
        }
        if let LoweredValue::Owned(_, slot) = key {
            transfers.push(slot);
        }
        let current_state = state_var.map(|variable| self.builder.use_var(variable));
        let updated = self.graphics_callback(
            update_descriptor,
            &update_params,
            update_result,
            &[current_state, Some(key_handle)],
            &transfers,
            span,
        )?;
        if let Some(variable) = state_var {
            let value = self.scalar(updated, span)?;
            if let Some(state_slot) = state_slot {
                if is_copy_owned(self.types, *state_type) {
                    self.drop_slot(state_slot)?;
                }
                self.builder.ins().stack_store(value, state_slot, 0);
                if let LoweredValue::Owned(_, slot) = updated {
                    self.clear_slot(slot);
                }
            }
            self.builder.def_var(variable, value);
        }

        let current_state = state_var.map(|variable| self.builder.use_var(variable));
        let scene = self.graphics_callback(
            render_descriptor,
            &render_params,
            scene_type,
            &[current_state],
            &[],
            span,
        )?;
        let scene_handle = self.scalar(scene, span)?;
        let presented = self.leaf(NativeLeaf::GraphicsPresent, &[session, scene_handle], true)?;
        let presented = self.own_linear(presented)?;
        let present_ok = self.builder.create_block();
        let present_bad = self.builder.create_block();
        let tag = self.graphics_tag(presented, span)?;
        self.builder
            .ins()
            .brif(tag, present_ok, &[], present_bad, &[]);
        self.builder.switch_to_block(present_bad);
        self.graphics_close_session(session)?;
        self.graphics_finish(presented, result_slot, result_var, done, span)?;
        self.builder.switch_to_block(present_ok);
        if let LoweredValue::Owned(_, slot) = presented {
            self.drop_slot(slot)?;
        }
        if let LoweredValue::Owned(_, slot) = scene {
            self.drop_slot(slot)?;
        }
        self.builder.ins().jump(loop_head, &[]);

        self.builder.switch_to_block(done);
        let result = self.builder.use_var(result_var);
        Ok(LoweredValue::Owned(result, result_slot))
    }
}
