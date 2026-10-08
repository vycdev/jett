//! The ordinary generated variable/debug copy carries no cleanup authority.
use super::*;
use crate::resource_execution::PreparedAbsentBindings;
use jett_parser::ast::{ForStmt, MatchStmt};

impl Interpreter {
    pub(in crate::interpreter) fn eval_checked_absent_for_iterable(
        &mut self,
        source: &ForStmt,
    ) -> Result<ExprFlow, String> {
        if source.view {
            if let Some(transport) = self
                .resource_transport
                .as_mut()
                .filter(|transport| transport.checked_source_active)
            {
                // Authenticate the original For before using its stripped view spelling.
                if transport
                    .checked
                    .prepare_absent_for(source)
                    .map_err(|error| error.to_string())?
                    .is_some()
                {
                    if let Expr::Ident(name) = Self::unparenthesized(&source.iterable) {
                        let definition = transport
                            .checked
                            .resolved_definition(name.span)
                            .map_err(|error| error.to_string())?;
                        if transport.has_binding(definition) {
                            let borrowed = transport
                                .binding(definition, true)
                                .map_err(|error| error.to_string())?;
                            return Ok(Self::resource_flow(borrowed));
                        }
                    }
                }
            }
        }
        self.eval_expr_flow(&source.iterable)
    }

    pub(in crate::interpreter) fn prepare_absent_for_bindings<'source>(
        &self,
        source: &'source ForStmt,
        value: &Value,
    ) -> Result<Option<PreparedAbsentBindings<'source>>, String> {
        match &self.resource_transport {
            Some(transport) => transport
                .prepare_absent_for(source, value)
                .map_err(|error| error.to_string()),
            None => Ok(None),
        }
    }

    pub(in crate::interpreter) fn prepare_absent_match_bindings<'source>(
        &self,
        source: &'source MatchStmt,
        arm: usize,
        value: &Value,
    ) -> Result<Option<PreparedAbsentBindings<'source>>, String> {
        match &self.resource_transport {
            Some(transport) => transport
                .prepare_absent_match(source, arm, value)
                .map_err(|error| error.to_string()),
            None => Ok(None),
        }
    }

    pub(in crate::interpreter) fn set_absent_generated_debug_binding(
        &mut self,
        proof: Option<&PreparedAbsentBindings<'_>>,
        name: &Ident,
        ordinal: usize,
        value: Value,
        fallback: Option<&TypeExpr>,
    ) -> Result<(), String> {
        if let Some(proof) = proof {
            self.resource_transport
                .as_mut()
                .ok_or("missing checked Resource transport")?
                .publish_absent_binding(proof, name, ordinal, value.clone())
                .map_err(|error| error.to_string())?;
        }
        self.set_inferred_debug_binding(name, value, fallback);
        Ok(())
    }

    pub(in crate::interpreter) fn validate_absent_generated_debug_binding(
        &self,
        proof: Option<&PreparedAbsentBindings<'_>>,
        name: &Ident,
        ordinal: usize,
        value: &Value,
    ) -> Result<(), String> {
        if let Some(proof) = proof {
            self.resource_transport
                .as_ref()
                .ok_or("missing checked Resource transport")?
                .validate_absent_binding(proof, name, ordinal, value)
                .map_err(|error| error.to_string())?;
        }
        Ok(())
    }
}
