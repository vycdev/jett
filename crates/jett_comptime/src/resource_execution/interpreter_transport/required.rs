//! Original required-region transactions. No provider or ownership permission.

use super::*;
use crate::resource_execution::{
    CheckedBodyReference, PreparedRequiredExpression, ResourceExecutionError,
};

impl Interpreter {
    pub(in crate::interpreter) fn eval_checked_actor_initializer(
        &mut self,
        reference: &CheckedBodyReference,
    ) -> Result<ExprFlow, String> {
        let saved = self
            .resource_transport
            .as_ref()
            .ok_or("actor initializer has no checked transport")?
            .cursor();
        self.resource_transport
            .as_mut()
            .ok_or("actor initializer has no checked transport")?
            .checked
            .install_function_body(reference)
            .map_err(|error| error.to_string())?;
        let result = reference
            .original_expression()
            .map_err(|error| error.to_string())
            .and_then(|expression| self.eval_expr_flow(expression));
        let restore = self
            .resource_transport
            .as_mut()
            .ok_or("actor initializer lost checked transport")?
            .checked
            .restore_cursor(saved)
            .map_err(|error| error.to_string());
        match (result, restore) {
            (Err(error), _) | (_, Err(error)) => Err(error),
            (Ok(value), Ok(())) => Ok(value),
        }
    }
    pub(crate) fn checked_explicit_contexts(
        &self,
        expression: &Expr,
        occurrence: Span,
        owner: Option<Span>,
        bindings: &[(Span, String)],
    ) -> Result<Option<Vec<PreparedRequiredExpression>>, String> {
        if let Some(transport) = &self.resource_transport {
            return transport
                .checked
                .prepare_explicit_contexts(expression, occurrence, owner, bindings)
                .map(Some)
                .map_err(|error| error.to_string());
        }
        let Some((program, purpose)) = &self.retained_resource_program else {
            return Ok(None);
        };
        // Retained ordinary programs need the same exact original required key,
        // but do not install Resource transport, a provider, or custody state.
        crate::resource_execution::CheckedExecution::new(program.clone(), *purpose)
            .and_then(|checked| {
                checked.prepare_explicit_contexts(expression, occurrence, owner, bindings)
            })
            .map(Some)
            .map_err(|error| error.to_string())
    }

    pub(crate) fn checked_namespace_initializer(
        &self,
        declaration: &VarDecl,
    ) -> Result<Option<PreparedRequiredExpression>, String> {
        if let Some(transport) = &self.resource_transport {
            return transport
                .checked
                .prepare_namespace_initializer(declaration)
                .map(Some)
                .map_err(|error| error.to_string());
        }
        let Some((program, purpose)) = &self.retained_resource_program else {
            return Ok(None);
        };
        // Match the explicit-expression route: ordinary retained programs use
        // the exact original required key without Resource transport or custody.
        crate::resource_execution::CheckedExecution::new(program.clone(), *purpose)
            .and_then(|checked| checked.prepare_namespace_initializer(declaration))
            .map(Some)
            .map_err(|error| error.to_string())
    }

    pub(crate) fn eval_checked_required_expression(
        &mut self,
        prepared: &PreparedRequiredExpression,
        namespace: Option<&str>,
        aliases: &HashMap<String, String>,
        parameters: &[String],
    ) -> Result<Value, String> {
        if self.resource_transport.is_none() {
            let (program, purpose) = self
                .retained_resource_program
                .as_ref()
                .ok_or("required region has no retained checked program")?;
            if !Arc::ptr_eq(program, prepared.checked_program()) {
                return Err(ResourceExecutionError::ForeignProgram.to_string());
            }
            if *purpose != prepared.purpose() || !program.checked().resource_hooks.is_empty() {
                return Err(ResourceExecutionError::WrongPurpose.to_string());
            }
            return self.with_checked_field_region(prepared.reference(), |interpreter| {
                interpreter.eval_closed_comptime_expression(
                    namespace,
                    aliases,
                    prepared.expression().map_err(|error| error.to_string())?,
                    parameters,
                    prepared.function_projection(),
                    prepared.scope_projection(),
                    prepared.bindings(),
                )
            });
        }
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
        self.with_original_test_capture(
            &original.body,
            ExecutionPurpose::Verify,
            namespace,
            |interpreter| {
                let Some(transport) = &interpreter.resource_transport else {
                    return interpreter.exec_block_in_namespace(namespace, &original.body);
                };
                let reference = transport
                    .checked
                    .prepare_verify_body(original)
                    .map_err(|error| error.to_string())?;
                interpreter.with_checked_required_region(&reference, |interpreter| {
                    interpreter.exec_block_in_namespace(namespace, &original.body)
                })
            },
        )
    }

    pub(crate) fn with_checked_property_region<T>(
        &mut self,
        original: &jett_parser::ast::PropertyBlock,
        namespace: Option<&str>,
        body: impl FnOnce(&mut Self) -> Result<T, String>,
    ) -> Result<T, String> {
        self.with_original_test_capture(
            &original.body,
            ExecutionPurpose::Property,
            namespace,
            |interpreter| {
                let Some(transport) = &interpreter.resource_transport else {
                    return body(interpreter);
                };
                let reference = transport
                    .checked
                    .prepare_property_body(original)
                    .map_err(|error| error.to_string())?;
                interpreter.with_checked_required_region(&reference, body)
            },
        )
    }

    fn with_original_test_capture<T>(
        &mut self,
        original: &jett_parser::ast::Block,
        purpose: ExecutionPurpose,
        namespace: Option<&str>,
        body: impl FnOnce(&mut Self) -> Result<T, String>,
    ) -> Result<T, String> {
        let Some((program, selected)) = &self.retained_resource_program else {
            return body(self);
        };
        if *selected != purpose {
            return Err(ResourceExecutionError::WrongPurpose.to_string());
        }
        let origin = crate::value::closure_capture::OriginalClosureOwner::test_body(
            program, original, purpose, namespace,
        )
        .ok_or("checked test capture has no exact original body/purpose/namespace")?;
        let field_body = self.checked_field_test_body(original, purpose)?;
        let saved_field = self.enter_checked_field_body(field_body.as_ref())?;
        let previous = self.active_reflected_capture_origin.replace(origin);
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| body(self)));
        let field_restore = self.restore_checked_field_body(saved_field);
        self.active_reflected_capture_origin = previous;
        match result {
            Ok(Err(error)) => Err(error),
            Ok(Ok(value)) => field_restore.map(|()| value),
            Err(panic) => std::panic::resume_unwind(panic),
        }
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
