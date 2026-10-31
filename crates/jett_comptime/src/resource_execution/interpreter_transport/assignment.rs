//! Evaluate an owning replacement once, after preparing its exact lexical slot.
use super::*;
use jett_parser::ast::AssignStmt;

impl Interpreter {
    pub(in crate::interpreter) fn exec_resource_assignment(
        &mut self,
        source: &AssignStmt,
    ) -> Result<Option<Signal>, String> {
        let destination = self
            .resource_transport
            .as_ref()
            .ok_or("missing checked Resource transport")?
            .prepare_resource_assignment(source)
            .map_err(|error| error.to_string())?;
        let scope_index = destination.scope_index();
        let Expr::Ident(target) = &source.target else {
            return Err("Resource assignment has no original binding target".to_string());
        };
        if scope_index < self.lexical_scope_floor
            || !self
                .scopes
                .get(scope_index)
                .is_some_and(|scope| scope.contains_key(&target.name))
        {
            return Err("Resource assignment has no exact lexical storage slot".to_string());
        }
        let operation = self
            .resource_transport
            .as_mut()
            .ok_or("missing checked Resource transport")?
            .begin_operation()
            .map_err(|error| error.to_string())?;
        let result = (|| {
            let value = match Self::resource_envelope(self.eval_expr_flow(&source.value)?)? {
                Ok(value) => value,
                Err(signal) => return Ok(ExprFlow::Signal(signal)),
            };
            // Physical mirroring carries no owning custody and deliberately
            // bypasses ordinary clone/retag/refinement normalization.
            let physical = value.value.clone();
            self.resource_transport
                .as_mut()
                .ok_or("missing checked Resource transport")?
                .replace_resource_assignment(destination, value)
                .map_err(|error| error.to_string())?;
            let scope = self
                .scopes
                .get_mut(scope_index)
                .ok_or("Resource assignment lost its exact lexical storage slot")?;
            scope.insert(target.name.clone(), physical);
            Ok(ExprFlow::Value(Value::Nothing))
        })();
        match self.finish_resource_operation(operation, result)? {
            ExprFlow::Value(Value::Nothing) => Ok(None),
            ExprFlow::Signal(signal) => Ok(Some(signal)),
            _ => Err("Resource assignment returned an unexpected expression flow".to_string()),
        }
    }
}
