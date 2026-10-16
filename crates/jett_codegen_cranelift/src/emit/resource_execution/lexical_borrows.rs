//! Emit constructor-certified lexical loan ends and owner drops; keep RootScope active.
use super::*;

impl Translator<'_, '_> {
    pub(super) fn resource_lexical_retirement(&mut self, span: Span) -> Result<bool, CodegenError> {
        let resource = self
            .resource
            .ok_or_else(|| pending("lexical retirement has no selected Resource family"))?;
        let plan = resource.function_plan()?;
        let Some(exit) = plan.lexical_exit(resource.block, resource.position) else {
            return Ok(false);
        };
        let function = resource
            .layout
            .plan()
            .program()
            .functions
            .get(resource.function.index() as usize)
            .filter(|function| function.id == resource.function)
            .ok_or_else(|| pending("lexical retirement lost its exact current function"))?;
        let original = function
            .resource_lexical_exit(resource.block, resource.position)
            .map_err(|message| pending(&message))?
            .ok_or_else(|| pending("lexical retirement lost its private constructor exit"))?;
        if original.kind() != exit.kind()
            || (exit.kind() == jett_mir::ResourceLexicalExitKind::Return)
                != (resource.position == jett_mir::ResourcePosition::Terminator)
        {
            return Err(pending("lexical retirement changed its exact exit kind"));
        }
        for id in exit.operation_ids() {
            let operation = plan
                .operations()
                .get(id.index())
                .filter(|operation| {
                    operation.id() == *id
                        && operation.site().function() == resource.function
                        && operation.site().block() == resource.block
                        && operation.site().position() == resource.position
                        && !operation.is_expression_operation()
                })
                .ok_or_else(|| pending("lexical retirement lost an exact ordered end row"))?;
            match operation.role() {
                Role::EndBorrow { .. } | Role::EndSumBorrow { .. } => {
                    self.resource_end_borrow(operation)?;
                }
                Role::Drop { .. } => self.resource_drop(operation)?,
                _ => return Err(pending("lexical retirement selected a non-retirement row")),
            }
        }
        for local in exit.clear_locals() {
            let index = local.index() as usize;
            let header = self
                .local_types
                .get(index)
                .filter(|header| header.id == *local)
                .ok_or_else(|| pending("lexical retirement lost a current alias header"))?;
            if !resource.layout.plan().type_requires_custody(header.ty)
                || self.local_slots[index].is_some()
            {
                return Err(pending("lexical retirement selected ordinary storage"));
            }
            // A consumed owner can be the pending Return operand. Its exact
            // Transfer clears the Resource slot after lexical retirement; Drop
            // rows above clear slots for owners actually retired here. Only the
            // dead Source variable is cleared unconditionally.
            let variable = variable_for(self.variables, local.index(), span, self.symbol)?
                .ok_or_else(|| pending("lexical retirement lost its nonowning scalar local"))?;
            let zero = self.builder.ins().iconst(ir::types::I64, 0);
            self.builder.try_def_var(variable, zero).map_err(|error| {
                contract_error(
                    self.symbol,
                    span,
                    format!("cannot clear retired Resource alias: {error}"),
                )
            })?;
        }
        Ok(true)
    }
}
