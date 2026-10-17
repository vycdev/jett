//! A closed factory receipt, distinct from Source ownership, enters each suite
//! target through its hidden Scope using the existing ordinary activation ABI.
use super::*;

impl Translator<'_, '_> {
    pub(super) fn resource_native_suite(
        &mut self,
        expression: &Expression,
        invocation: &jett_mir::ResourceOperation,
        operations: &[&jett_mir::ResourceOperation],
    ) -> Result<LoweredValue, CodegenError> {
        let resource = self
            .resource
            .ok_or_else(|| pending("native suite invocation has no selected family"))?;
        let Role::InvokeNativeSuite { invocation: suite } = invocation.role() else {
            return Err(pending("native suite invocation selected another role"));
        };
        let function = suite.function();
        let proof = suite.proof();
        let formals = suite.formals();
        let evaluation_order = suite.evaluation_order();
        let operands = suite.operands();
        let result = suite.result();
        let ExpressionKind::Call {
            function: target,
            args,
            evaluation_order: actual_order,
            ..
        } = &expression.kind
        else {
            return Err(pending("native suite receipt lost its exact direct Call"));
        };
        if *target != function
            || proof.target() != *target
            || !proof.matches(self.function.id, expression, proof.expression_ordinal())
            || evaluation_order != actual_order.as_slice()
            || args.len() != formals.len()
            || operands.len() != args.len()
            || expression.ty != TypeInterner::NOTHING
            || !matches!(result, CallResult::Ordinary { ty } if ty == TypeInterner::NOTHING)
        {
            return Err(pending(
                "native suite Call changed its exact factory receipt",
            ));
        }
        self.resource_open_operation(invocation.frame())?;
        let (context, frame, ordinal) = self.resource_operation(invocation)?;
        let call = Leaf::SourcePrepare.output(
            self.module,
            self.builder,
            &[context, frame, ordinal],
            resource.failure,
            8,
        )?;
        let mut evaluated = vec![None; args.len()];
        for (source_index, &parameter) in evaluation_order.iter().enumerate() {
            let argument = args
                .get(parameter)
                .ok_or_else(|| pending("native suite actual is outside its exact order"))?;
            let formal = formals
                .get(parameter)
                .filter(|formal| formal.parameter() == parameter)
                .ok_or_else(|| pending("native suite actual has no exact generated formal"))?;
            if formal.source_index() != source_index
                || formal.actual_type() != argument.ty
                || formal.parameter_type() != argument.ty
                || formal.access() != jett_mir::ParamMode::Owned
                || formal.effect() != jett_mir::ResourceArgumentEffect::Copy
                || formal.syntax() != jett_mir::ResourceArgumentSyntax::Bare
                || !matches!(operands.get(parameter),
                    Some(Operand::Ordinary { parameter: actual, ty })
                    if *actual == parameter && *ty == argument.ty)
                || resource.layout.plan().type_requires_custody(argument.ty)
                || evaluated[parameter].is_some()
            {
                return Err(pending(
                    "native suite actual changes its ordinary copy contract",
                ));
            }
            let value = self.argument(argument, false)?;
            evaluated[parameter] = Some(value);
            let bits = self.payload_bits(value).0;
            let source_index = self.builder.ins().iconst(
                ir::types::I32,
                i64::try_from(source_index)
                    .map_err(|_| pending("native suite source order exceeds its wire domain"))?,
            );
            let parameter = self.builder.ins().iconst(
                ir::types::I32,
                i64::try_from(parameter)
                    .map_err(|_| pending("native suite parameter exceeds its wire domain"))?,
            );
            Leaf::SourceActual.checked(
                self.module,
                self.builder,
                &[context, call, source_index, parameter, bits],
                resource.failure,
            )?;
        }
        let scope = Leaf::SourceEnter.output(
            self.module,
            self.builder,
            &[context, call],
            resource.failure,
            8,
        )?;
        let environment = self.builder.ins().iconst(ir::types::I64, 0);
        let mut native = vec![context, environment, scope];
        for (parameter, argument) in args.iter().enumerate() {
            let index = self.builder.ins().iconst(
                ir::types::I32,
                i64::try_from(parameter)
                    .map_err(|_| pending("native suite parameter exceeds its wire domain"))?,
            );
            let installed = Leaf::SourceParameter.output(
                self.module,
                self.builder,
                &[context, scope, index],
                resource.failure,
                8,
            )?;
            // The callee retains its ordinary copy parameter itself. A String
            // installation observes the evaluated handle; it mints no second
            // caller owner for that same handle.
            let scalar = if is_copy_owned(self.types, argument.ty) {
                installed
            } else {
                let unpacked = self.unpack_payload(installed, argument.ty, argument.span)?;
                self.scalar(unpacked, argument.span)?
            };
            native.push(scalar);
            if task_scalar(Some(resource.layout), self.types, argument.ty)? {
                let evaluated = evaluated[parameter]
                    .ok_or_else(|| pending("native suite ordinary argument was not evaluated"))?;
                native.push(self.scalar_task(evaluated, argument.span)?.1);
            }
        }
        let declared = self
            .declarations
            .get(function)
            .ok_or_else(|| pending("native suite target has no exact family declaration"))?;
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
        let [nothing] = results.as_slice() else {
            return Err(pending(
                "native suite Nothing call changed its single-word ABI",
            ));
        };
        self.resource_finish_operation(operations, invocation.frame())?;
        Ok(LoweredValue::Scalar(*nothing))
    }
}
