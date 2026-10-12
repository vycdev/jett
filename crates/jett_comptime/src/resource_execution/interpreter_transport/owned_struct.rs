//! Reached owned actual envelopes stay inside the constructor operation.
use super::*;

impl Interpreter {
    pub(super) fn eval_resource_owned_struct(
        &mut self,
        expression: &Expr,
    ) -> Result<Option<ExprFlow>, String> {
        let proof = self
            .resource_transport
            .as_ref()
            .ok_or("missing checked Resource transport")?
            .checked
            .prepare_owned_struct(expression)
            .map_err(|error| error.to_string())?;
        let Some(proof) = proof else {
            return Ok(None);
        };
        let Expr::Call(_, arguments, _) = expression else {
            return Err("owned struct lost its exact original call".to_string());
        };
        let operation = self
            .resource_transport
            .as_mut()
            .ok_or("missing checked Resource transport")?
            .begin_operation()
            .map_err(|error| error.to_string())?;
        let result = (|| {
            let mut actuals = Vec::with_capacity(arguments.len());
            for (argument, ordinal) in arguments.iter().zip(proof.source_fields()) {
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
                    .validate_value_type(&value, proof.fields()[*ordinal].1)
                    .map_err(|error| error.to_string())?;
                if value.custody.has_borrows() {
                    return Err("owned struct cannot acquire a borrowed payload".to_string());
                }
                transport
                    .hold_actual(&mut value)
                    .map_err(|error| error.to_string())?;
                actuals.push(value);
            }
            let value = self
                .resource_transport
                .as_mut()
                .ok_or("missing checked Resource transport")?
                .assemble_owned_struct(&proof, expression, actuals)
                .map_err(|error| error.to_string())?;
            Ok(Self::resource_flow(value))
        })();
        self.finish_resource_operation(operation, result).map(Some)
    }
}

#[cfg(test)]
#[path = "owned_struct_tests.rs"]
mod tests;
