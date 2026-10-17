//! Checked pipeline envelopes; source expressions are evaluated once.
use crate::resource_execution::PreparedPipelineStep;

use super::*;

impl Interpreter {
    pub(super) fn eval_resource_pipeline(&mut self, pipeline: &Expr) -> Result<ExprFlow, String> {
        let Expr::Pipeline(_, steps, _) = pipeline else {
            return Err("checked pipeline lost its original node".to_string());
        };
        let first = self
            .resource_transport
            .as_ref()
            .ok_or("missing checked Resource transport")?
            .checked
            .prepare_pipeline_step(pipeline, 0)
            .map_err(|error| error.to_string())?;
        let first_intrinsic = self.prepare_resource_pipeline_intrinsic(&first)?;
        let operation = self
            .resource_transport
            .as_mut()
            .ok_or("missing checked Resource transport")?
            .begin_operation()
            .map_err(|error| error.to_string())?;
        let result = (|| {
            let mut current =
                match Self::resource_envelope(self.eval_resource_pipeline_initial(&first)?)? {
                    Ok(value) => value,
                    Err(signal) => return Ok(ExprFlow::Signal(signal)),
                };
            let mut first = Some((first, first_intrinsic));
            for index in 0..steps.len() {
                let (prepared, intrinsic) = if index == 0 {
                    first.take().ok_or("checked pipeline lost its first step")?
                } else {
                    let prepared = self
                        .resource_transport
                        .as_ref()
                        .ok_or("missing checked Resource transport")?
                        .checked
                        .prepare_pipeline_step(pipeline, index)
                        .map_err(|error| error.to_string())?;
                    let intrinsic = self.prepare_resource_pipeline_intrinsic(&prepared)?;
                    (prepared, intrinsic)
                };
                let flow =
                    self.eval_resource_pipeline_step(&prepared, current, intrinsic.as_ref())?;
                current = match Self::resource_envelope(flow)? {
                    Ok(value) => value,
                    Err(signal) => return Ok(ExprFlow::Signal(signal)),
                };
            }
            Ok(Self::resource_flow(current))
        })();
        self.finish_resource_operation(operation, result)
    }

    fn prepare_resource_pipeline_intrinsic(
        &self,
        prepared: &PreparedPipelineStep<'_>,
    ) -> Result<Option<PreparedIntrinsicArguments>, String> {
        if matches!(
            prepared.invocation().target(),
            CheckedInvocationTarget::Intrinsic(_)
        ) {
            self.resource_transport
                .as_ref()
                .ok_or("missing checked Resource transport")?
                .checked
                .prepare_pipeline_intrinsic_arguments(prepared)
                .map(Some)
                .map_err(|error| error.to_string())
        } else {
            Ok(None)
        }
    }

    fn eval_resource_pipeline_initial(
        &mut self,
        prepared: &PreparedPipelineStep<'_>,
    ) -> Result<ExprFlow, String> {
        let original = prepared
            .initial()
            .ok_or("checked pipeline has no original first input")?;
        if let Some(definition) = prepared.initial_borrow() {
            let transport = self
                .resource_transport
                .as_mut()
                .ok_or("missing checked Resource transport")?;
            if transport.has_binding(definition) {
                let value = transport
                    .binding(definition, true)
                    .map_err(|error| error.to_string())?;
                return Ok(Self::resource_flow(value));
            }
        }
        self.eval_expr_flow(original)
    }

    fn eval_resource_pipeline_step(
        &mut self,
        prepared: &PreparedPipelineStep<'_>,
        mut input: EvaluatedValue,
        intrinsic: Option<&PreparedIntrinsicArguments>,
    ) -> Result<ExprFlow, String> {
        let operation = self
            .resource_transport
            .as_mut()
            .ok_or("missing checked Resource transport")?
            .begin_operation()
            .map_err(|error| error.to_string())?;
        let result = (|| {
            let invocation = prepared.invocation();
            let source0 = invocation
                .arguments()
                .first()
                .ok_or("checked pipeline has no original input occurrence")?;
            let transport = self
                .resource_transport
                .as_mut()
                .ok_or("missing checked Resource transport")?;
            transport
                .validate_value_type(&input, prepared.input_type())
                .map_err(|error| error.to_string())?;
            transport
                .validate_actual(&input, source0.effect)
                .map_err(|error| error.to_string())?;
            transport
                .hold_actual(&mut input)
                .map_err(|error| error.to_string())?;
            let mut actuals = Vec::with_capacity(invocation.arguments().len());
            actuals.push(input);
            for (argument, fact) in prepared
                .extra_arguments()
                .iter()
                .zip(&invocation.arguments()[1..])
            {
                let mut value =
                    match Self::resource_envelope(self.eval_expr_flow(&argument.value)?)? {
                        Ok(value) => value,
                        Err(signal) => return Ok(ExprFlow::Signal(signal)),
                    };
                let transport = self
                    .resource_transport
                    .as_mut()
                    .ok_or("missing checked Resource transport")?;
                transport
                    .validate_actual(&value, fact.effect)
                    .map_err(|error| error.to_string())?;
                transport
                    .hold_actual(&mut value)
                    .map_err(|error| error.to_string())?;
                actuals.push(value);
            }
            let flow = self.dispatch_resource_actuals(
                invocation,
                prepared.callee(),
                prepared.type_arguments(),
                actuals,
                prepared.constructor(),
                operation.frame,
                intrinsic,
            )?;
            let value = match Self::resource_envelope(flow)? {
                Ok(value) => value,
                Err(signal) => return Ok(ExprFlow::Signal(signal)),
            };
            self.resource_transport
                .as_ref()
                .ok_or("missing checked Resource transport")?
                .validate_value_type(&value, prepared.raw_call_type())
                .map_err(|error| error.to_string())?;
            Ok(Self::resource_flow(value))
        })();
        let flow = self.finish_resource_operation(operation, result)?;
        let value = match Self::resource_envelope(flow)? {
            Ok(value) => value,
            Err(signal) => return Ok(ExprFlow::Signal(signal)),
        };
        let flow = if let Some(handle) = &prepared.original().handle {
            self.eval_resource_pipeline_handle(value, handle)?
        } else {
            Self::resource_flow(value)
        };
        if let ExprFlow::Resource(value) = &flow {
            self.resource_transport
                .as_ref()
                .ok_or("missing checked Resource transport")?
                .validate_value_type(value, prepared.output_type())
                .map_err(|error| error.to_string())?;
        }
        Ok(flow)
    }

    fn eval_resource_pipeline_handle(
        &mut self,
        mut target: EvaluatedValue,
        handle: &jett_parser::ast::PipelineStepHandle,
    ) -> Result<ExprFlow, String> {
        while let Value::Typed { value, .. } = target.value {
            target.value = *value;
            target
                .remove_prefix(PayloadStep::Typed)
                .map_err(|error| error.to_string())?;
        }
        match target.value {
            Value::ResultOk(value) => {
                target.value = *value;
                target
                    .remove_prefix(PayloadStep::Ok)
                    .map_err(|error| error.to_string())?;
                Ok(Self::resource_flow(target))
            }
            Value::OptionalSome(value) => {
                target.value = *value;
                target
                    .remove_prefix(PayloadStep::Some)
                    .map_err(|error| error.to_string())?;
                Ok(Self::resource_flow(target))
            }
            Value::ResultFail(error) if target.custody.is_empty() => {
                self.exec_resource_handle(handle.error_name.as_ref(), Some(*error), &handle.body)
            }
            Value::OptionalNone if target.custody.is_empty() => {
                self.exec_resource_handle(None, None, &handle.body)
            }
            _ => Err("checked pipeline handle has no selected sum custody transport".to_string()),
        }
    }
}

#[cfg(test)]
#[path = "pipeline/tests.rs"]
mod tests;
