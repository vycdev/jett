//! A checked reflected read owns an adapter; its future result owns the check.
use super::*;
use crate::value::ReflectedCallableValue;

#[cfg(test)]
mod tests;

impl Interpreter {
    fn authenticate_current_reflected_source(
        &self,
        value: &Value,
        program: &Arc<jett_typecheck::CheckedResourceProgram>,
        actual: jett_types::TypeId,
    ) -> Result<(), String> {
        if !self
            .checked_expression_types
            .as_ref()
            .is_some_and(|installed| {
                installed
                    .resource_program
                    .as_ref()
                    .is_some_and(|current| Arc::ptr_eq(current, program))
                    && crate::value::closure_capture::original_root_types(program, installed)
            })
        {
            return Err(
                "reflected callback checked expression context changed its original source".into(),
            );
        }
        ReflectedCallableValue::authenticate_source(value, program, actual)?;
        match value.payload() {
            Value::Pending(inner) => {
                self.authenticate_current_reflected_source(inner, program, actual)
            }
            Value::NamedFunction(name) => {
                let current = self
                    .functions
                    .get(name)
                    .ok_or("reflected callback original function is not registered")?;
                if crate::value::closure_capture::OriginalClosureOwner::registered(
                    program,
                    name,
                    &current.definition,
                    &current.namespace,
                )
                .is_none()
                {
                    return Err(
                        "reflected callback current function changed its original source".into(),
                    );
                }
                Ok(())
            }
            Value::ReflectedCallable(admitted) => self.authenticate_current_reflected_source(
                admitted.source(),
                program,
                admitted.source_type(),
            ),
            Value::Function { .. } => Ok(()),
            _ => Err("reflected callback source is not a callable descriptor".into()),
        }
    }

    pub(super) fn has_checked_reflected_callable_plan(
        &self,
        actual_type_name: &str,
        requested: &TypeExpr,
    ) -> bool {
        if !matches!(requested, TypeExpr::Function(..)) {
            return false;
        }
        let Some(program) = self
            .checked_expression_types
            .as_ref()
            .and_then(|types| types.resource_program.as_ref())
        else {
            return false;
        };
        let checked = program.checked();
        if !self
            .reflection_metadata
            .as_ref()
            .is_some_and(|metadata| Arc::ptr_eq(metadata, &checked.reflection_metadata))
        {
            return false;
        }
        let actual = checked
            .reflection_metadata
            .source_type_id_for_name(actual_type_name);
        let target = checked
            .reflection_metadata
            .source_type_id_for_name(&type_expr_display(requested));
        matches!((actual, target), (Some(actual), Some(target))
            if checked.reflected_callable_plan(actual, target).is_some())
    }

    pub(super) fn prepare_reflected_callable(
        &self,
        value: Value,
        requested: &TypeExpr,
        actual_type_name: &str,
    ) -> Result<Value, String> {
        let program = self
            .checked_expression_types
            .as_ref()
            .and_then(|types| types.resource_program.as_ref())
            .ok_or_else(|| {
                "reflected field refinement: unsupported callable or nominal refinement conversion"
                    .to_string()
            })?;
        let checked = program.checked();
        if !self
            .reflection_metadata
            .as_ref()
            .is_some_and(|metadata| Arc::ptr_eq(metadata, &checked.reflection_metadata))
        {
            return Err("reflected callback metadata belongs to another checked program".into());
        }
        let requested_name = type_expr_display(requested);
        let actual = checked
            .reflection_metadata
            .source_type_id_for_name(actual_type_name)
            .ok_or_else(|| "reflected callback has no declared source signature".to_string())?;
        let target = checked
            .reflection_metadata
            .source_type_id_for_name(&requested_name)
            .ok_or_else(|| "reflected callback has no checked requested signature".to_string())?;
        self.authenticate_current_reflected_source(&value, program, actual)?;
        if actual == target {
            // Authenticate the retained descriptor, then transport its exact
            // wrappers without invoking, joining, or establishing a new proof.
            return Ok(value);
        }
        let plan = checked
            .reflected_callable_plan(actual, target)
            .ok_or_else(|| {
                "reflected field refinement: unsupported callable or nominal refinement conversion"
                    .to_string()
            })?;
        let source_return = Self::simple_type_expr_from_name(
            &checked.interner.type_name(plan.source_return()),
            requested.span(),
        )
        .ok_or_else(|| {
            "reflected callback source return has no exact type expression".to_string()
        })?;
        let requested_return = Self::simple_type_expr_from_name(
            &checked.interner.type_name(plan.requested_return()),
            requested.span(),
        )
        .ok_or_else(|| {
            "reflected callback requested return has no exact type expression".to_string()
        })?;
        Ok(Value::ReflectedCallable(Box::new(
            ReflectedCallableValue::new(
                value,
                program.clone(),
                actual,
                target,
                source_return,
                requested_return,
            )?,
        )))
    }

    pub(super) fn call_reflected_callable(
        &mut self,
        adapter: ReflectedCallableValue,
        args: Vec<Value>,
        source_types: Option<&[Option<String>]>,
    ) -> Result<Value, String> {
        let current = self
            .checked_expression_types
            .as_ref()
            .and_then(|types| types.resource_program.as_ref())
            .ok_or_else(|| {
                "reflected callback invocation has no original checked program".to_string()
            })?;
        if !Arc::ptr_eq(current, adapter.checked_program()) {
            return Err("reflected callback invocation belongs to another checked program".into());
        }
        if !adapter.checked_plan().validate(current.checked()) {
            return Err("reflected callback invocation changed its checked return plan".into());
        }
        self.authenticate_current_reflected_source(
            adapter.source(),
            current,
            adapter.source_type(),
        )?;
        // Retain the original Value, source signature and closure context. A read
        // never calls it. Its own call boundary performs the source validations.
        let mut result =
            self.call_fn_value_from_source(adapter.source().clone(), args, source_types)?;
        let actual_name = type_expr_display(adapter.source_return());
        let requested = adapter.requested_return();
        let mut checks = Vec::new();
        self.plan_produced_refinements(
            requested,
            Some(adapter.source_return()),
            &result,
            &mut Vec::new(),
            &mut checks,
        )?;
        // Finish every occupied shape/pending preflight before the first new
        // predicate. Do not inspect or implicitly join a pending result.
        for check in &checks {
            let slot = Self::produced_value_slot(&mut result, &check.path).ok_or_else(|| {
                "reflected callback return shape changed during preflight".to_string()
            })?;
            if matches!(slot.payload(), Value::Pending(_)) {
                return Err("reflected callback return: expected a ready value".into());
            }
        }
        let saved_proofs = self.allow_checked_refinement_proofs;
        // The opaque adapter retains an authenticated program/type plan even
        // when an ordinary higher-order helper has no source argument map.
        self.allow_checked_refinement_proofs = true;
        let output = self
            .establish_reflected_field_refinement(result, requested, &actual_name, true)
            .and_then(|value| {
                self.normalize_and_validate_value_from_source(
                    requested,
                    value,
                    Some(&type_expr_display(requested)),
                )
            });
        self.allow_checked_refinement_proofs = saved_proofs;
        output
    }
}
