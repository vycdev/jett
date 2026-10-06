//! Opaque hook metadata uses exact constructor proof, never ordinary callable storage.
use super::*;

impl Translator<'_, '_> {
    pub(in crate::emit) fn resource_is_hook_descriptor(
        &self,
        expression: &Expression,
    ) -> Result<bool, CodegenError> {
        let Some(resource) = self.resource else {
            return Ok(false);
        };
        let Some(hook) = resource.function_plan()?.descriptor_value(expression) else {
            return Ok(false);
        };
        if hook.function_type() != expression.ty
            || !resource
                .layout
                .plan()
                .program()
                .resource_manifest
                .contains_hook(hook)
            || !matches!(self.types.resolve(expression.ty), Type::Function { .. })
        {
            return Err(pending(
                "opaque descriptor occurrence changes its exact hook proof",
            ));
        }
        Ok(true)
    }

    pub(super) fn resource_validate_hook_invocation(
        &self,
        invocation: &jett_mir::ResourceOperation,
        callee: Option<&Expression>,
    ) -> Result<(), CodegenError> {
        let Role::InvokeHook { hook, source, .. } = invocation.role() else {
            return Ok(());
        };
        match (&source.target, callee, invocation.indirect_hook_target()) {
            (jett_hir::CallTarget::Indirect { signature_type }, Some(callee), Some(proof)) => {
                if proof != hook
                    || proof.function_type() != *signature_type
                    || callee.ty != *signature_type
                    || source.bridge != jett_hir::CallBridge::Direct
                    || !source.generated_operands.is_empty()
                    || !self.resource_is_hook_descriptor(callee)?
                    || self
                        .resource
                        .expect("selected family")
                        .function_plan()?
                        .descriptor_value(callee)
                        != Some(proof)
                {
                    return Err(pending(
                        "indirect hook changes its exact descriptor, signature or Source tuple",
                    ));
                }
                Ok(())
            }
            (jett_hir::CallTarget::Indirect { .. }, _, _) => Err(pending(
                "indirect hook has no exact current descriptor target proof",
            )),
            (_, None, None) => Ok(()),
            _ => Err(pending(
                "hook descriptor proof is attached to a different original call",
            )),
        }
    }

    pub(super) fn resource_hook_value(
        &mut self,
        expression: &Expression,
        operations: &[&jett_mir::ResourceOperation],
    ) -> Result<LoweredValue, CodegenError> {
        let resource = self
            .resource
            .ok_or_else(|| pending("descriptor has no family"))?;
        let hook = resource
            .function_plan()?
            .descriptor_value(expression)
            .ok_or_else(|| pending("descriptor has no exact current occurrence"))?;
        let value = match &expression.kind {
            ExpressionKind::Local(local) => {
                if resource.function_plan()?.descriptor_local(*local) != Some(hook)
                    || self.local_slots[local.index() as usize].is_some()
                {
                    return Err(pending(
                        "opaque descriptor Local gained ordinary ownership or changed its header",
                    ));
                }
                let variable = self.variables[local.index() as usize]
                    .ok_or_else(|| pending("opaque descriptor Local has no native variable"))?;
                self.builder.try_use_var(variable).map_err(|error| {
                    pending(&format!(
                        "opaque descriptor Local is not initialized: {error}"
                    ))
                })?
            }
            ExpressionKind::Clone(inner) => {
                if resource.function_plan()?.descriptor_value(inner) != Some(hook) {
                    return Err(pending(
                        "opaque descriptor alias changes its exact producer",
                    ));
                }
                let lowered = self.expression(inner)?;
                self.scalar(lowered, expression.span)?
            }
            ExpressionKind::ResourceHookValue { hook: producer } if producer == hook => {
                let mut selected = operations.iter().copied().filter(|operation|
                    matches!(operation.role(), Role::Descriptor { hook: current } if current == hook));
                let operation = selected
                    .next()
                    .ok_or_else(|| pending("hook value lost its exact descriptor row"))?;
                if selected.next().is_some() {
                    return Err(pending("hook value has duplicate descriptor rows"));
                }
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
            _ => {
                return Err(pending(
                    "opaque descriptor transport has no exact native carrier route",
                ));
            }
        };
        Ok(LoweredValue::Scalar(value))
    }
}
