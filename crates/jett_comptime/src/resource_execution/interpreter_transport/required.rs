//! Original required-region transactions. No provider or ownership permission.

use super::*;
use crate::resource_execution::{
    CheckedBodyReference, PreparedRequiredExpression, ResourceExecutionError,
};

impl Interpreter {
    pub(crate) fn checked_explicit_contexts(
        &self,
        expression: &Expr,
        occurrence: Span,
        owner: Option<Span>,
        bindings: &[(Span, String)],
    ) -> Result<Option<Vec<PreparedRequiredExpression>>, String> {
        self.resource_transport
            .as_ref()
            .map(|transport| {
                transport
                    .checked
                    .prepare_explicit_contexts(expression, occurrence, owner, bindings)
                    .map_err(|error| error.to_string())
            })
            .transpose()
    }

    pub(crate) fn checked_namespace_initializer(
        &self,
        declaration: &VarDecl,
    ) -> Result<Option<PreparedRequiredExpression>, String> {
        self.resource_transport
            .as_ref()
            .map(|transport| {
                transport
                    .checked
                    .prepare_namespace_initializer(declaration)
                    .map_err(|error| error.to_string())
            })
            .transpose()
    }

    pub(crate) fn eval_checked_required_expression(
        &mut self,
        prepared: &PreparedRequiredExpression,
        namespace: Option<&str>,
        aliases: &HashMap<String, String>,
        parameters: &[String],
    ) -> Result<Value, String> {
        if self
            .resource_transport
            .as_ref()
            .ok_or("required region has no checked program")?
            .checked
            .purpose()
            != prepared.purpose()
        {
            return Err(ResourceExecutionError::WrongPurpose.to_string());
        }
        self.with_checked_required_region(prepared.reference(), |interpreter| {
            interpreter.eval_closed_comptime_expression(
                namespace,
                aliases,
                prepared.expression().map_err(|error| error.to_string())?,
                parameters,
                prepared.function_projection(),
                prepared.scope_projection(),
                prepared.bindings(),
            )
        })
    }

    pub(crate) fn exec_checked_verify_region(
        &mut self,
        namespace: Option<&str>,
        original: &jett_parser::ast::VerifyBlock,
    ) -> Result<Option<Value>, String> {
        let Some(transport) = &self.resource_transport else {
            return self.exec_block_in_namespace(namespace, &original.body);
        };
        let reference = transport
            .checked
            .prepare_verify_body(original)
            .map_err(|error| error.to_string())?;
        self.with_checked_required_region(&reference, |interpreter| {
            interpreter.exec_block_in_namespace(namespace, &original.body)
        })
    }

    pub(crate) fn with_checked_property_region<T>(
        &mut self,
        original: &jett_parser::ast::PropertyBlock,
        body: impl FnOnce(&mut Self) -> Result<T, String>,
    ) -> Result<T, String> {
        let Some(transport) = &self.resource_transport else {
            return body(self);
        };
        let reference = transport
            .checked
            .prepare_property_body(original)
            .map_err(|error| error.to_string())?;
        self.with_checked_required_region(&reference, body)
    }

    fn with_checked_required_region<T>(
        &mut self,
        reference: &CheckedBodyReference,
        body: impl FnOnce(&mut Self) -> Result<T, String>,
    ) -> Result<T, String> {
        let transport = self
            .resource_transport
            .as_ref()
            .ok_or("required region has no checked program")?;
        transport
            .validate_entry_scopes(self.scopes.len())
            .map_err(|error| error.to_string())?;
        let saved = SavedResourceEntryContext::capture(self)?;
        let depth = saved.scope_depth();
        self.resource_transport
            .as_mut()
            .ok_or("missing checked Resource transport")?
            .checked
            .install_function_body(reference)
            .map_err(|error| error.to_string())?;
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| body(self)));
        let cleanup = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            self.resource_transport
                .as_mut()
                .ok_or("missing checked Resource transport")?
                .unwind_all()
                .map_err(|error| error.to_string())
        }));
        let restore = saved.restore(self);
        let completion = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            self.resource_transport
                .as_mut()
                .ok_or("missing checked Resource transport")?
                .restore_entry_scopes(depth)
                .map_err(|error| error.to_string())
        }));
        let result = match (result, restore) {
            (Ok(Ok(_)), Err(error)) => Ok(Err(error)),
            (result, _) => result,
        };
        Self::complete_resource_entry(result, Self::complete_resource_cleanup(cleanup, completion))
    }
}
