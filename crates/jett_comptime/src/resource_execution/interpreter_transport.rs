//! Envelope transport in the existing checked source evaluator.
#[path = "interpreter_transport/assignment.rs"]
mod assignment;

#[path = "interpreter_transport/absent_generated.rs"]
mod absent_generated;

use super::*;
use crate::resource_execution::FunctionInvocation;
use crate::resource_execution::{
    CheckedInvocation, EvaluatedValue, ExecutionPurpose, FrameId, OperationFrame, PayloadStep,
    PreparedIntrinsicArguments, ResourceHookDescriptor, ResourceTransport,
};
use jett_parser::ast::VarDecl;
use jett_resolve::DefId;
use jett_typecheck::{CheckedCalleeAccess, CheckedInvocationTarget};
use jett_types::Type;

#[path = "interpreter_transport/entry_context.rs"]
mod entry_context;
use entry_context::SavedResourceEntryContext;

#[path = "interpreter_transport/intrinsic.rs"]
mod intrinsic;
#[path = "interpreter_transport/pipeline.rs"]
mod pipeline;
#[path = "interpreter_transport/required.rs"]
mod required;

// Runtime registration identifies a retained declaration; its cloned body is
// deliberately excluded because only the retained checked body is executed.
fn same_registered_function_header(left: &FunctionDef, right: &FunctionDef) -> bool {
    fn ident(left: &Ident, right: &Ident) -> bool {
        left.name == right.name && left.span == right.span
    }
    fn types(left: &[TypeExpr], right: &[TypeExpr]) -> bool {
        left.len() == right.len() && left.iter().zip(right).all(|(left, right)| ty(left, right))
    }
    fn ty(left: &TypeExpr, right: &TypeExpr) -> bool {
        match (left, right) {
            (TypeExpr::Named(left), TypeExpr::Named(right)) => ident(left, right),
            (
                TypeExpr::Generic(left, left_args, left_span),
                TypeExpr::Generic(right, right_args, right_span),
            ) => left_span == right_span && ident(left, right) && types(left_args, right_args),
            (TypeExpr::View(left, left_span), TypeExpr::View(right, right_span)) => {
                left_span == right_span && ty(left, right)
            }
            (
                TypeExpr::Function(left, left_result, left_span),
                TypeExpr::Function(right, right_result, right_span),
            ) => left_span == right_span && types(left, right) && ty(left_result, right_result),
            (
                TypeExpr::StateQualified(left, left_state, left_span),
                TypeExpr::StateQualified(right, right_state, right_span),
            ) => left_span == right_span && ty(left, right) && ident(left_state, right_state),
            _ => false,
        }
    }
    left.span == right.span
        && ident(&left.name, &right.name)
        && left.exported == right.exported
        && left.body.span == right.body.span
        && left.type_params.len() == right.type_params.len()
        && left
            .type_params
            .iter()
            .zip(&right.type_params)
            .all(|(left, right)| ident(left, right))
        && left.params.len() == right.params.len()
        && left.params.iter().zip(&right.params).all(|(left, right)| {
            left.span == right.span
                && left.view == right.view
                && left.mutable == right.mutable
                && ident(&left.name, &right.name)
                && ty(&left.ty, &right.ty)
        })
        && match (&left.return_type, &right.return_type) {
            (None, None) => true,
            (Some(left), Some(right)) => ty(left, right),
            _ => false,
        }
}

impl Interpreter {
    pub(super) fn check_resource_cleanup(&mut self) -> Result<(), String> {
        match &mut self.resource_transport {
            Some(transport) => transport.check_cleanup().map_err(|error| error.to_string()),
            None => Ok(()),
        }
    }

    pub(super) fn prepare_checked_resource_field_loop(
        &self,
        source: &jett_parser::ast::ForStmt,
    ) -> Result<Option<Arc<crate::resource_execution::PreparedReflectedFieldLoop>>, String> {
        let Some(transport) = self
            .resource_transport
            .as_ref()
            .filter(|transport| transport.checked_source_active)
        else {
            return Ok(None);
        };
        transport
            .checked
            .prepare_reflected_field_loop(source)
            .map_err(|error| error.to_string())
    }

    pub(super) fn exec_checked_resource_type_bind(
        &mut self,
        bind: &jett_parser::ast::ComptimeTypeBindStmt,
    ) -> Result<Option<Signal>, String> {
        // Every fallible source/type/body join precedes interpreter mutation.
        let checked = &self
            .resource_transport
            .as_ref()
            .ok_or("missing checked Resource transport")?
            .checked;
        let prepared = match &bind.value {
            Expr::FieldAccess(base, member, _) if member.name == "type_info" => {
                let Expr::Ident(variable) = base.as_ref() else {
                    return Err("resource execution has no exact checked body".to_string());
                };
                let definition = checked
                    .program()
                    .resolved()
                    .resolutions
                    .get(&variable.span)
                    .ok_or("resource execution has no exact checked body")?;
                let iteration = self
                    .checked_reflected_fields
                    .iter()
                    .rev()
                    .find(|proof| proof.variable() == *definition)
                    .ok_or("resource execution has no exact checked body")?;
                let current = self
                    .get_variable(&variable.name)
                    .ok_or("resource execution has no exact checked body")?;
                iteration
                    .validate_value(current)
                    .map_err(|error| error.to_string())?;
                checked.prepare_reflected_field_scope(bind, iteration)
            }
            _ => checked.prepare_direct_scope(bind),
        }
        .map_err(|error| error.to_string())?;
        let body = prepared.body().map_err(|error| error.to_string())?;
        let bound_type_expr =
            Self::simple_type_expr_from_name(&prepared.reflection().type_name, bind.value.span())
                .ok_or("checked scoped type has no canonical interpreter syntax")?;
        let projection = prepared.projection();
        let binding = ClosureScopedTypeBinding {
            name: bind.name.name.clone(),
            canonical_name: prepared.bound_name().to_string(),
            reflection: Some(prepared.reflection().clone()),
        };
        let saved_cursor = self
            .resource_transport
            .as_ref()
            .ok_or("missing checked Resource transport")?
            .cursor();
        self.resource_transport
            .as_mut()
            .ok_or("missing checked Resource transport")?
            .checked
            .install_direct_scope(&prepared)
            .map_err(|error| error.to_string())?;
        let saved_checked_scope = self.active_checked_scope.replace(projection);
        self.scoped_type_bindings.push(binding);
        self.type_arg_scopes
            .push(HashMap::from([(bind.name.name.clone(), bound_type_expr)]));
        let result = self.exec_block_inner(body);
        self.type_arg_scopes.pop();
        self.scoped_type_bindings.pop();
        self.active_checked_scope = saved_checked_scope;
        let cleanup = self.check_resource_cleanup();
        let restore = match &mut self.resource_transport {
            Some(transport) => transport
                .checked
                .restore_cursor(saved_cursor)
                .map_err(|error| error.to_string()),
            None => Err("missing checked Resource transport".to_string()),
        };
        match (result, cleanup, restore) {
            (_, Err(error), _) => Err(error),
            (Err(error), Ok(()), _) | (_, Ok(()), Err(error)) => Err(error),
            (Ok(value), Ok(()), Ok(())) => Ok(value),
        }
    }

    fn resource_flow(value: EvaluatedValue) -> ExprFlow {
        if value.custody.is_empty() && !value.value.contains_live_resource_or_grant() {
            ExprFlow::Value(value.value)
        } else {
            ExprFlow::Resource(value)
        }
    }

    fn resource_envelope(flow: ExprFlow) -> Result<Result<EvaluatedValue, Signal>, String> {
        match flow {
            ExprFlow::Value(value) if !value.contains_live_resource_or_grant() => {
                Ok(Ok(EvaluatedValue::ordinary(value)))
            }
            ExprFlow::Value(_) => {
                Err("physical Resource carrier has no owning or borrowed envelope".to_string())
            }
            ExprFlow::Resource(value) => Ok(Ok(value)),
            ExprFlow::Signal(signal) => Ok(Err(signal)),
        }
    }

    pub(super) fn resource_type_at(&self, span: Span) -> Result<bool, String> {
        let transport = self
            .resource_transport
            .as_ref()
            .ok_or("missing checked Resource transport")?;
        let ty = transport
            .checked
            .expression_type(span)
            .map_err(|error| error.to_string())?;
        transport
            .checked
            .type_contains_resource(ty)
            .map_err(|error| error.to_string())
    }

    pub(super) fn eval_resource_transport(
        &mut self,
        expression: &Expr,
    ) -> Result<Option<ExprFlow>, String> {
        let transport = self
            .resource_transport
            .as_ref()
            .ok_or("missing checked Resource transport")?;
        if !transport.checked_source_active {
            return Ok(None);
        }
        match expression {
            Expr::Ident(identifier) => {
                let definition = transport
                    .checked
                    .resolved_definition(identifier.span)
                    .map_err(|error| error.to_string())?;
                if transport.checked.has_hook(definition) {
                    let descriptor = transport
                        .checked
                        .descriptor(definition)
                        .map_err(|error| error.to_string())?;
                    return Ok(Some(ExprFlow::Value(Value::ResourceHook(descriptor))));
                }
                if transport.has_binding(definition) {
                    let value = self
                        .resource_transport
                        .as_mut()
                        .ok_or("missing checked Resource transport")?
                        .binding(definition, false)
                        .map_err(|error| error.to_string())?;
                    return Ok(Some(Self::resource_flow(value)));
                }
                if self.resource_type_at(identifier.span)?
                    && transport
                        .checked
                        .prepare_named_callable(expression)
                        .map_err(|error| error.to_string())?
                        .is_none()
                {
                    return Err("Resource source binding has no live cleanup custody".to_string());
                }
                Ok(None)
            }
            Expr::View(inner, _) => {
                let bare = Self::unparenthesized(inner);
                if let Expr::Ident(identifier) = bare {
                    let definition = transport
                        .checked
                        .resolved_definition(identifier.span)
                        .map_err(|error| error.to_string())?;
                    if transport.has_binding(definition) {
                        let value = self
                            .resource_transport
                            .as_mut()
                            .ok_or("missing checked Resource transport")?
                            .binding(definition, true)
                            .map_err(|error| error.to_string())?;
                        return Ok(Some(Self::resource_flow(value)));
                    }
                }
                // A written view of an owned producer retains one operation
                // backing ticket; formal binding creates only the borrow.
                let flow = self.eval_expr_flow(inner)?;
                Ok(Some(flow))
            }
            Expr::Paren(inner, _) => self.eval_expr_flow(inner).map(Some),
            Expr::Ok(inner, _) | Expr::Fail(inner, _) | Expr::Some(inner, _)
                if self.resource_type_at(expression.span())? =>
            {
                let mut value = match Self::resource_envelope(self.eval_expr_flow(inner)?)? {
                    Ok(value) => value,
                    Err(signal) => return Ok(Some(ExprFlow::Signal(signal))),
                };
                let step = match expression {
                    Expr::Ok(..) => PayloadStep::Ok,
                    Expr::Fail(..) => PayloadStep::Fail,
                    Expr::Some(..) => PayloadStep::Some,
                    _ => return Err("invalid checked sum transport".to_string()),
                };
                value = value.prefix(step.clone());
                value.value = match step {
                    PayloadStep::Ok => Value::ResultOk(Box::new(value.value)),
                    PayloadStep::Fail => Value::ResultFail(Box::new(value.value)),
                    PayloadStep::Some => Value::OptionalSome(Box::new(value.value)),
                    _ => return Err("invalid checked sum transport".to_string()),
                };
                Ok(Some(Self::resource_flow(value)))
            }
            Expr::Handle(target, binding, body, _) if self.resource_type_at(target.span())? => {
                let mut target = match Self::resource_envelope(self.eval_expr_flow(target)?)? {
                    Ok(value) => value,
                    Err(signal) => return Ok(Some(ExprFlow::Signal(signal))),
                };
                // Typed storage is a separate custody path; this operation does
                // not erase nominal/secret policy. Checker-selected sum typing
                // already establishes the exact occupied arm.
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
                        Ok(Some(Self::resource_flow(target)))
                    }
                    Value::OptionalSome(value) => {
                        target.value = *value;
                        target
                            .remove_prefix(PayloadStep::Some)
                            .map_err(|error| error.to_string())?;
                        Ok(Some(Self::resource_flow(target)))
                    }
                    Value::ResultFail(error) => {
                        if !target.custody.is_empty() {
                            return Err(
                                "Resource-bearing error custody has no selected handler transport"
                                    .to_string(),
                            );
                        }
                        self.exec_resource_handle(binding.as_ref(), Some(*error), body)
                            .map(Some)
                    }
                    Value::OptionalNone => {
                        if !target.custody.is_empty() {
                            return Err("absent Resource sum retained a live owner".to_string());
                        }
                        self.exec_resource_handle(None, None, body).map(Some)
                    }
                    _ => Err("checked Resource handle has no exact sum carrier".to_string()),
                }
            }
            Expr::Default(inner, _) if self.resource_type_at(inner.span())? => {
                let mut value = match Self::resource_envelope(self.eval_expr_flow(inner)?)? {
                    Ok(value) => value,
                    Err(signal) => return Ok(Some(ExprFlow::Signal(signal))),
                };
                self.resource_transport
                    .as_mut()
                    .ok_or("missing checked Resource transport")?
                    .preserve_default(&mut value)
                    .map_err(|error| error.to_string())?;
                Ok(Some(ExprFlow::Signal(Signal::ResourceDefault(value))))
            }
            Expr::Comptime(inner, _) => {
                let previous = self
                    .resource_transport
                    .as_mut()
                    .ok_or("missing checked Resource transport")?
                    .checked
                    .replace_purpose(ExecutionPurpose::ExplicitComptime);
                let result = self.eval_expr_flow(inner);
                self.resource_transport
                    .as_mut()
                    .ok_or("missing checked Resource transport")?
                    .checked
                    .replace_purpose(previous);
                let flow = result?;
                if matches!(&flow, ExprFlow::Resource(_)) {
                    return Err(
                        "required evaluation cannot produce live Resource custody".to_string()
                    );
                }
                Ok(Some(flow))
            }
            Expr::Pipeline(..) => self.eval_resource_pipeline(expression).map(Some),
            // All other ordinary expressions remain the established evaluator.
            // Its operand extraction rejects a transported Resource envelope.
            _ => Ok(None),
        }
    }

    pub(super) fn exec_resource_binding(
        &mut self,
        declaration: &VarDecl,
    ) -> Result<Option<Signal>, String> {
        let value = match Self::resource_envelope(self.eval_expr_flow(&declaration.value)?)? {
            Ok(value) => value,
            Err(signal) => return Ok(Some(signal)),
        };
        // The checked source type and occupied payload paths remain unchanged;
        // opaque carriers do not enter ordinary clone/retag normalization.
        let physical = value.value.clone();
        let ty = self.substitute_type_expr(&declaration.ty);
        self.resource_transport
            .as_mut()
            .ok_or("missing checked Resource transport")?
            .install_binding(declaration.name.span, value)
            .map_err(|error| error.to_string())?;
        self.set_variable_with_type(&declaration.name.name, physical, ty);
        Ok(None)
    }

    fn exec_resource_handle(
        &mut self,
        binding: Option<&Ident>,
        value: Option<Value>,
        body: &Block,
    ) -> Result<ExprFlow, String> {
        self.resource_transport
            .as_mut()
            .ok_or("missing checked Resource transport")?
            .enter_default()
            .map_err(|error| error.to_string())?;
        let result = self.exec_handle_block(binding, value, body, None);
        self.resource_transport
            .as_mut()
            .ok_or("missing checked Resource transport")?
            .leave_default();
        result
    }

    fn finish_resource_operation(
        &mut self,
        operation: OperationFrame,
        result: Result<ExprFlow, String>,
    ) -> Result<ExprFlow, String> {
        // A handler Return already retired its exact acquired suffix before
        // evaluating its operand. Its suspended envelopes acknowledge once;
        // ordinary success and output adoption never consume that authority.
        if matches!(
            &result,
            Err(_)
                | Ok(ExprFlow::Signal(
                    Signal::Return(..) | Signal::ResourceReturn(..)
                ))
        ) && self
            .resource_transport
            .as_mut()
            .ok_or("missing checked Resource transport")?
            .acknowledge_return_operation(&operation)
            .map_err(|error| error.to_string())?
        {
            return result;
        }
        let mut result = result;
        let value = match &mut result {
            Ok(ExprFlow::Resource(value)) => Some(value),
            _ => None,
        };
        let cleanup = self
            .resource_transport
            .as_mut()
            .ok_or("missing checked Resource transport")?
            .end_operation(operation, value)
            .map_err(|error| error.to_string());
        match (result, cleanup) {
            (_, Err(error)) => Err(error),
            (Err(error), Ok(())) => Err(error),
            (Ok(flow), Ok(())) => Ok(flow),
        }
    }

    pub(super) fn eval_resource_call(
        &mut self,
        callee: &Expr,
        type_arguments: &[TypeExpr],
        arguments: &[CallArg],
        span: Span,
    ) -> Result<ExprFlow, String> {
        let invocation = self
            .resource_transport
            .as_ref()
            .ok_or("missing checked Resource transport")?
            .checked
            .invocation(span)
            .map_err(|error| error.to_string())?;
        if invocation.arguments().len() != arguments.len()
            || invocation
                .arguments()
                .iter()
                .zip(arguments)
                .any(|(fact, argument)| fact.source_span != argument.value.span())
        {
            return Err("checked call lost its original source argument occurrence".to_string());
        }
        let intrinsic = if matches!(invocation.target(), CheckedInvocationTarget::Intrinsic(_)) {
            Some(
                self.resource_transport
                    .as_ref()
                    .ok_or("missing checked Resource transport")?
                    .checked
                    .prepare_intrinsic_arguments(callee, type_arguments, arguments, &invocation)
                    .map_err(|error| error.to_string())?,
            )
        } else {
            None
        };
        // Variant construction has a checked function-shaped packet, but its
        // Source endpoint is not a runtime callable value. Prepare its exact
        // nominal schema before evaluating any actuals.
        let constructor = self
            .resource_transport
            .as_ref()
            .ok_or("missing checked Resource transport")?
            .checked
            .enum_constructor(Self::unparenthesized(callee), &invocation)
            .map_err(|error| error.to_string())?;
        let operation = self
            .resource_transport
            .as_mut()
            .ok_or("missing checked Resource transport")?
            .begin_operation()
            .map_err(|error| error.to_string())?;
        let result = (|| {
            let mut actuals = Vec::with_capacity(arguments.len());
            for (argument, fact) in arguments.iter().zip(invocation.arguments()) {
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
            self.dispatch_resource_actuals(
                &invocation,
                callee,
                type_arguments,
                actuals,
                constructor,
                operation.frame,
                intrinsic.as_ref(),
            )
        })();
        self.finish_resource_operation(operation, result)
    }

    // Both routes provide original checked identity and already-evaluated
    // source-order envelopes. This dispatcher never re-evaluates an actual.
    fn dispatch_resource_actuals(
        &mut self,
        invocation: &CheckedInvocation,
        callee: &Expr,
        type_arguments: &[TypeExpr],
        actuals: Vec<EvaluatedValue>,
        constructor: Option<(String, String, jett_types::TypeId)>,
        return_destination: FrameId,
        intrinsic: Option<&PreparedIntrinsicArguments>,
    ) -> Result<ExprFlow, String> {
        if matches!(invocation.target(), CheckedInvocationTarget::Intrinsic(_))
            != intrinsic.is_some()
        {
            return Err("checked intrinsic lost its prepared concrete argument packet".to_string());
        }
        if let Some(prepared) = intrinsic {
            prepared
                .validate(
                    &self
                        .resource_transport
                        .as_ref()
                        .ok_or("missing checked Resource transport")?
                        .checked,
                    invocation,
                )
                .map_err(|error| error.to_string())?;
        }
        if actuals.len() != invocation.arguments().len() {
            return Err("checked call lost its evaluated source argument count".to_string());
        }
        // Source lexical order is retained above. Ownership adoption below
        // follows that same order even when destination parameters permute.
        let mut formal: Vec<Option<EvaluatedValue>> = (0..actuals.len()).map(|_| None).collect();
        for (actual, fact) in actuals.into_iter().zip(invocation.arguments()) {
            let borrowed = self
                .resource_transport
                .as_ref()
                .ok_or("missing checked Resource transport")?
                .formal_value(&actual, fact.callee_access)
                .map_err(|error| error.to_string())?;
            formal[fact.parameter_index] = Some(borrowed.unwrap_or(actual));
        }
        let formal = formal
            .into_iter()
            .map(|value| value.ok_or("checked call has no exact parameter value".to_string()))
            .collect::<Result<Vec<_>, _>>()?;
        if let Some((type_name, variant, ty)) = constructor {
            if formal.iter().any(|value| {
                !value.custody.is_empty() || value.value.contains_live_resource_or_grant()
            }) {
                return Err(
                    "occupied Resource enum construction has no selected custody transport"
                        .to_string(),
                );
            }
            let value = EvaluatedValue::ordinary(Value::Enum {
                type_name,
                variant,
                fields: formal.into_iter().map(|value| value.value).collect(),
            });
            self.resource_transport
                .as_ref()
                .ok_or("missing checked Resource transport")?
                .validate_value_type(&value, ty)
                .map_err(|error| error.to_string())?;
            return Ok(Self::resource_flow(value));
        }
        let bare_callee = Self::unparenthesized(callee);
        match invocation.target() {
            CheckedInvocationTarget::Resolved(definition)
                if self
                    .resource_transport
                    .as_ref()
                    .ok_or("missing checked Resource transport")?
                    .checked
                    .has_hook(*definition) =>
            {
                let resolved = self
                    .resource_transport
                    .as_ref()
                    .ok_or("missing checked Resource transport")?
                    .checked
                    .resolved_definition(bare_callee.span())
                    .map_err(|error| error.to_string())?;
                if resolved != *definition {
                    return Err("checked Resource hook lost its exact declaration".to_string());
                }
                let descriptor = self
                    .resource_transport
                    .as_ref()
                    .ok_or("missing checked Resource transport")?
                    .checked
                    .descriptor(*definition)
                    .map_err(|error| error.to_string())?;
                let value = self
                    .resource_transport
                    .as_mut()
                    .ok_or("missing checked Resource transport")?
                    .invoke_hook(invocation, &descriptor, formal)
                    .map_err(|error| error.to_string())?;
                Ok(Self::resource_flow(value))
            }
            CheckedInvocationTarget::Resolved(definition)
            | CheckedInvocationTarget::Generic(jett_typecheck::CheckedGenericCall {
                definition,
                ..
            }) => {
                let name = self.exact_registered_resource_function(*definition)?;
                let resolved_arguments = match invocation.target() {
                    CheckedInvocationTarget::Generic(call) => call
                        .concrete_args
                        .iter()
                        .map(|ty| {
                            let types = &self
                                .resource_transport
                                .as_ref()
                                .ok_or("missing checked Resource transport")?
                                .checked
                                .program()
                                .checked()
                                .interner;
                            Self::debug_type(&types.type_name(*ty)).ok_or(
                                "checked generic argument has no canonical interpreter type"
                                    .to_string(),
                            )
                        })
                        .collect::<Result<Vec<_>, String>>()?,
                    _ => type_arguments
                        .iter()
                        .map(|ty| self.substitute_type_expr(ty))
                        .collect(),
                };
                let value = self.call_registered_function_envelopes(
                    &name,
                    &resolved_arguments,
                    formal,
                    &FunctionInvocation::Source(invocation),
                    return_destination,
                )?;
                Ok(Self::resource_flow(value))
            }
            CheckedInvocationTarget::Indirect(_) => {
                let function = match Self::resource_envelope(self.eval_expr_flow(bare_callee)?)? {
                    Ok(value) => value,
                    Err(signal) => return Ok(ExprFlow::Signal(signal)),
                };
                match function.value {
                    Value::ResourceHook(descriptor) if function.custody.is_empty() => {
                        let value = self.resource_transport.as_mut().ok_or("missing checked Resource transport")?
                            .invoke_hook(invocation, &descriptor, formal).map_err(|error| error.to_string())?;
                        Ok(Self::resource_flow(value))
                    }
                    Value::NamedFunction(name) if function.custody.is_empty()
                        && (formal.iter().any(|argument| !argument.custody.is_empty() || argument.value.contains_live_resource_or_grant())
                            || self.resource_type_at(bare_callee.span())?) => {
                        let target = self.resource_transport.as_ref().ok_or("missing checked Resource transport")?
                            .checked.prepare_named_indirect(invocation, bare_callee).map_err(|error| error.to_string())?;
                        let expected = self.exact_registered_resource_function(target.definition())?;
                        if name != expected {
                            return Err("named Resource callable differs from its exact original target".to_string());
                        }
                        let value = self.call_registered_function_envelopes(
                            &expected, &[], formal,
                            &FunctionInvocation::NamedSource { source: invocation, target: &target },
                            return_destination,
                        )?;
                        Ok(Self::resource_flow(value))
                    }
                    value if function.custody.is_empty() && !value.contains_live_resource_or_grant()
                        && formal.iter().all(|argument| argument.custody.is_empty() && !argument.value.contains_live_resource_or_grant()) => {
                        Ok(ExprFlow::Value(self.call_fn_value_from_source(value, formal.into_iter().map(|value| value.value).collect(), None)?))
                    }
                    _ => Err("Resource-bearing captured invocation has no checked custody transfer proof".to_string()),
                }
            }
            CheckedInvocationTarget::Intrinsic(_) => {
                if formal.iter().any(|value| {
                    !value.custody.is_empty() || value.value.contains_live_resource_or_grant()
                }) {
                    return Err(
                        "Resource-bearing intrinsic transport has no selected acquisition proof"
                            .to_string(),
                    );
                }
                let prepared =
                    intrinsic.ok_or("checked intrinsic has no prepared concrete arguments")?;
                let values = formal.into_iter().map(|value| value.value).collect();
                Ok(ExprFlow::Value(
                    self.call_resource_intrinsic(invocation, prepared, values)?,
                ))
            }
            CheckedInvocationTarget::Method(_) | CheckedInvocationTarget::Interface(_) => {
                if formal.iter().any(|value| {
                    !value.custody.is_empty() || value.value.contains_live_resource_or_grant()
                }) {
                    return Err("Resource-bearing intrinsic/method transport has no selected acquisition proof".to_string());
                }
                // Evaluate the selected ordinary operation with the already
                // evaluated actual values. No source actual is evaluated twice.
                let values = formal
                    .into_iter()
                    .map(|value| value.value)
                    .collect::<Vec<_>>();
                let name = Self::dotted_expr_name(bare_callee)
                    .ok_or("ordinary checked intrinsic has no source operation")?;
                Ok(ExprFlow::Value(self.call_function_from_resolved_source(
                    &name,
                    type_arguments,
                    values,
                    None,
                )?))
            }
        }
    }

    fn exact_registered_resource_function(&self, definition: DefId) -> Result<String, String> {
        let transport = self
            .resource_transport
            .as_ref()
            .ok_or("missing checked Resource transport")?;
        let original = transport
            .checked
            .retained_function(definition)
            .map_err(|error| error.to_string())?;
        let (namespace, _) = transport
            .checked
            .source_function_context(original.name.span)
            .map_err(|error| error.to_string())?;
        let mut matches = Vec::new();
        for (name, registered) in &self.functions {
            if transport
                .checked
                .declaration_definition(registered.definition.name.span)
                .ok()
                != Some(definition)
            {
                continue;
            }
            if registered.namespace != namespace
                || !same_registered_function_header(&registered.definition, original)
            {
                return Err(
                    "checked registration differs from its retained source declaration".to_string(),
                );
            }
            matches.push(name.clone());
        }
        let [name] = matches.as_slice() else {
            return Err("checked invocation has no exact retained source function".to_string());
        };
        Ok(name.clone())
    }
}

impl Interpreter {
    fn call_registered_function_envelopes(
        &mut self,
        name: &str,
        type_args: &[TypeExpr],
        args: Vec<EvaluatedValue>,
        invocation: &FunctionInvocation,
        return_destination: FrameId,
    ) -> Result<EvaluatedValue, String> {
        let resolved_name = name.to_string();
        let physical_arguments = args
            .iter()
            .map(|argument| argument.value.clone())
            .collect::<Vec<_>>();

        // Look up the function definition.
        let registered = self
            .functions
            .get(&resolved_name)
            .ok_or_else(|| {
                let name = self
                    .registry_name(&self.interface_methods, name)
                    .unwrap_or_else(|| name.to_string());
                format!("undefined function '{name}'")
            })?
            .clone();
        let transport = self
            .resource_transport
            .as_ref()
            .ok_or("missing checked Resource transport")?;
        let definition = match invocation {
            FunctionInvocation::Entry { definition, .. } => *definition,
            FunctionInvocation::NamedSource { target, .. } => target.definition(),
            FunctionInvocation::Source(invocation) => match invocation.target() {
                CheckedInvocationTarget::Resolved(definition) => *definition,
                CheckedInvocationTarget::Generic(call) => call.definition,
                _ => {
                    return Err("checked source call has no exact retained declaration".to_string());
                }
            },
        };
        let original = transport
            .checked
            .retained_function(definition)
            .map_err(|error| error.to_string())?;
        let (namespace, trusted_stdlib) = transport
            .checked
            .source_function_context(original.name.span)
            .map_err(|error| error.to_string())?;
        if transport
            .checked
            .declaration_definition(registered.definition.name.span)
            .map_err(|error| error.to_string())?
            != definition
            || registered.namespace != namespace
            || !same_registered_function_header(&registered.definition, original)
        {
            return Err(
                "checked registration differs from its retained source declaration".to_string(),
            );
        }
        // The original checked AST supplies both header and executable body.
        // A host-edited registration clone is never executable authority.
        let body_reference = transport
            .checked
            .prepare_function_body(invocation)
            .map_err(|error| error.to_string())?;
        let func = body_reference
            .function()
            .map_err(|error| error.to_string())?;

        if args.len() != func.params.len() {
            return Err(format!(
                "function '{}' expects {} argument(s), got {}",
                resolved_name,
                func.params.len(),
                args.len()
            ));
        }

        let type_scope = self.type_scope_for_function(&func, type_args)?;
        let arguments: Vec<TypeExpr> = func
            .type_params
            .iter()
            .map(|param| type_scope[&param.name].clone())
            .collect();
        let captured_arguments = self.captured_type_arguments(&arguments);
        let expression_types = match &self.checked_expression_types {
            Some(types) => types
                .select(func.name.span, &captured_arguments, &physical_arguments)
                .map_err(|error| format!("{resolved_name}: {error}"))?,
            None => None,
        };
        let saved_resource_cursor = self
            .resource_transport
            .as_ref()
            .ok_or("missing checked Resource transport")?
            .cursor();
        self.resource_transport
            .as_mut()
            .ok_or("missing checked Resource transport")?
            .checked
            .install_function_body(&body_reference)
            .map_err(|error| error.to_string())?;
        let saved_expression_types =
            std::mem::replace(&mut self.active_checked_function, expression_types);
        let saved_type_arguments = std::mem::replace(&mut self.current_type_arguments, arguments);
        let saved_scoped_types = std::mem::take(&mut self.scoped_type_bindings);
        let saved_checked_scope = self.active_checked_scope.take();
        let saved_reflected_fields = std::mem::take(&mut self.checked_reflected_fields);
        // Resolve arguments above in the caller, then keep its type bindings
        // out of the callee's lexical scope, including non-generic callees.
        let saved_type_scopes = std::mem::replace(&mut self.type_arg_scopes, vec![type_scope]);

        let saved_namespace = self.current_namespace.clone();
        let saved_trusted_stdlib = self.current_function_trusted_stdlib;
        self.current_namespace = namespace;
        self.current_function_trusted_stdlib = trusted_stdlib;

        let scope_depth = self.scopes.len();
        let saved_scope_floor = self.lexical_scope_floor;
        self.lexical_scope_floor = scope_depth;
        self.push_scope();
        let saved_proofs = self.allow_checked_refinement_proofs;
        self.resource_transport
            .as_mut()
            .ok_or("missing checked Resource transport")?
            .enter_return(return_destination);
        let call_result = (|| {
            let mut arguments = args.into_iter().map(Some).collect::<Vec<_>>();
            // Install each formal holder in SOURCE acquisition order. Formal
            // permutation affects lookup only, never reverse cleanup ordering.
            let contracts = invocation
                .parameters(
                    &self
                        .resource_transport
                        .as_ref()
                        .ok_or("missing checked Resource transport")?
                        .checked,
                )
                .map_err(|error| error.to_string())?;
            for fact in &contracts {
                let index = fact.index;
                let param = func
                    .params
                    .get(index)
                    .ok_or("checked call has no exact source parameter")?;
                if param.view != (fact.access == CheckedCalleeAccess::View) {
                    return Err(
                        "checked source parameter access differs from retained declaration"
                            .to_string(),
                    );
                }
                let mut arg = arguments
                    .get_mut(index)
                    .and_then(Option::take)
                    .ok_or("checked call repeated a formal value")?;
                let param_ty = self.substitute_type_expr(&param.ty);
                if arg.value.contains_live_resource_or_grant()
                    || !arg.custody.is_empty()
                    || self
                        .resource_transport
                        .as_ref()
                        .ok_or("missing checked Resource transport")?
                        .checked
                        .type_contains_resource(fact.ty)
                        .map_err(|error| error.to_string())?
                {
                    self.resource_transport
                        .as_ref()
                        .ok_or("missing checked Resource transport")?
                        .validate_value_type(&arg, fact.ty)
                        .map_err(|error| error.to_string())?;
                } else {
                    let source_type = match invocation {
                        FunctionInvocation::Source(call)
                        | FunctionInvocation::NamedSource { source: call, .. } => call
                            .arguments()
                            .iter()
                            .find(|argument| argument.parameter_index == index)
                            .map(|argument| {
                                self.resource_transport
                                    .as_ref()
                                    .ok_or("missing checked Resource transport")
                                    .map(|transport| {
                                        transport
                                            .checked
                                            .program()
                                            .checked()
                                            .interner
                                            .type_name(argument.actual_type)
                                    })
                            })
                            .transpose()?,
                        FunctionInvocation::Entry { .. } => None,
                    };
                    arg.value = self.normalize_and_validate_value_from_source(
                        &param_ty,
                        arg.value,
                        source_type.as_deref(),
                    )?;
                }
                let physical = arg.value.clone();
                if arg.value.contains_live_resource_or_grant()
                    || !arg.custody.is_empty()
                    || self
                        .resource_transport
                        .as_ref()
                        .ok_or("missing checked Resource transport")?
                        .checked
                        .type_contains_resource(fact.ty)
                        .map_err(|error| error.to_string())?
                {
                    self.resource_transport
                        .as_mut()
                        .ok_or("missing checked Resource transport")?
                        .install_binding(param.name.span, arg)
                        .map_err(|error| error.to_string())?;
                }
                self.set_variable_with_type(&param.name.name, physical, param_ty);
            }

            let result = self.exec_block_inner(&func.body)?;
            let (mut value, return_source_type) = match result {
                Some(Signal::ResourceReturn(value, source_type)) => (value, source_type),
                Some(Signal::Return(value, source_type)) => {
                    (EvaluatedValue::ordinary(value), source_type)
                }
                Some(Signal::Default(_) | Signal::ResourceDefault(_)) => {
                    return Err("`default` can only be used inside a `handle` block".to_string());
                }
                _ => (EvaluatedValue::ordinary(Value::Nothing), None),
            };
            if let Some(return_type) = &func.return_type {
                let return_type = self.substitute_type_expr(return_type);
                let signature_type = invocation
                    .signature(
                        &self
                            .resource_transport
                            .as_ref()
                            .ok_or("missing checked Resource transport")?
                            .checked,
                    )
                    .map_err(|error| error.to_string())?;
                let Type::Function {
                    return_type: checked_return,
                    ..
                } = self
                    .resource_transport
                    .as_ref()
                    .ok_or("missing checked Resource transport")?
                    .checked
                    .program()
                    .checked()
                    .interner
                    .resolve(signature_type)
                else {
                    return Err("source function has no exact checked result type".to_string());
                };
                let checked_return = *checked_return;
                if value.value.contains_live_resource_or_grant()
                    || !value.custody.is_empty()
                    || self
                        .resource_transport
                        .as_ref()
                        .ok_or("missing checked Resource transport")?
                        .checked
                        .type_contains_resource(checked_return)
                        .map_err(|error| error.to_string())?
                {
                    self.resource_transport
                        .as_ref()
                        .ok_or("missing checked Resource transport")?
                        .validate_value_type(&value, checked_return)
                        .map_err(|error| error.to_string())?;
                } else {
                    value.value = self.normalize_and_validate_value_from_source(
                        &return_type,
                        value.value,
                        return_source_type.as_deref(),
                    )?;
                }
            }

            Ok(value)
        })();
        self.resource_transport
            .as_mut()
            .ok_or("missing checked Resource transport")?
            .leave_return();
        self.allow_checked_refinement_proofs = saved_proofs;
        while self.scopes.len() > scope_depth {
            self.pop_scope();
        }
        self.lexical_scope_floor = saved_scope_floor;
        self.active_checked_scope = saved_checked_scope;
        self.checked_reflected_fields = saved_reflected_fields;
        self.scoped_type_bindings = saved_scoped_types;
        self.type_arg_scopes = saved_type_scopes;
        self.current_type_arguments = saved_type_arguments;
        self.active_checked_function = saved_expression_types;
        self.current_namespace = saved_namespace;
        self.current_function_trusted_stdlib = saved_trusted_stdlib;
        let cleanup = self.check_resource_cleanup();
        let restore = self
            .resource_transport
            .as_mut()
            .ok_or("missing checked Resource transport")?
            .checked
            .restore_cursor(saved_resource_cursor)
            .map_err(|error| error.to_string());
        match (call_result, cleanup, restore) {
            (_, Err(error), _) => Err(error),
            (Err(error), Ok(()), _) | (_, Ok(()), Err(error)) => Err(error),
            (Ok(value), Ok(()), Ok(())) => Ok(value),
        }
    }
}

impl Interpreter {
    /// Install one compiler-derived snapshot. Production providers remain disabled.
    /// Debug emission is configured independently and grants no runtime purpose.
    pub fn from_checked_resource_program(
        program: Arc<jett_typecheck::CheckedResourceProgram>,
        purpose: ExecutionPurpose,
    ) -> Result<Self, String> {
        let mut interpreter = Self::new();
        interpreter.register_module(program.module());
        interpreter.install_checked_resource_program(program, purpose)?;
        Ok(interpreter)
    }

    pub fn install_checked_resource_program(
        &mut self,
        program: Arc<jett_typecheck::CheckedResourceProgram>,
        purpose: ExecutionPurpose,
    ) -> Result<(), String> {
        if self.retained_resource_program.is_some() {
            return Err("checked Resource program is already installed".to_string());
        }
        if !program.checked().resource_hooks.is_empty() {
            self.resource_transport = Some(
                ResourceTransport::checked_only(program.clone(), purpose, self.scopes.len())
                    .map_err(|error| error.to_string())?,
            );
        }
        self.retained_resource_program = Some((program, purpose));
        Ok(())
    }

    pub(crate) fn authorize_checked_resource_worker(
        &mut self,
        module: &Module,
        purpose: ExecutionPurpose,
    ) -> Result<(), String> {
        let (program, selected) = self
            .retained_resource_program
            .as_ref()
            .ok_or("worker has no retained checked program")?;
        if purpose == ExecutionPurpose::ReferenceRuntime
            || *selected != purpose
            || !std::ptr::eq(module, program.module())
        {
            return Err(
                "required worker has no exact retained source identity and purpose".to_string(),
            );
        }
        if let Some(transport) = &mut self.resource_transport {
            transport.checked_source_active = true;
        }
        Ok(())
    }

    pub(crate) fn checked_execution_purpose(
        &mut self,
        purpose: ExecutionPurpose,
    ) -> Option<ExecutionPurpose> {
        let (_, current) = self.retained_resource_program.as_mut()?;
        let previous = std::mem::replace(current, purpose);
        if let Some(transport) = &mut self.resource_transport {
            transport.checked.replace_purpose(purpose);
        }
        Some(previous)
    }

    /// Checked entry is an exact declaration, not a source-name hook dispatcher.
    /// Incoming raw live Resource carriers are never accepted as owning inputs.
    pub fn call_checked_program_entry(
        &mut self,
        definition: DefId,
        arguments: Vec<Value>,
    ) -> Result<Value, String> {
        if arguments.iter().any(|argument| {
            matches!(argument, Value::Resource(_))
                || (argument.contains_live_resource_or_grant()
                    && !matches!(argument, Value::GrantedNetwork(_)))
        }) {
            return Err(
                "checked entry cannot adopt custody from a copied Resource carrier".to_string(),
            );
        }
        let transport = self
            .resource_transport
            .as_ref()
            .ok_or("checked entry has no retained program")?;
        if transport.checked.purpose() != ExecutionPurpose::ReferenceRuntime
            && arguments.iter().any(Value::contains_live_resource_or_grant)
        {
            return Err(
                "required worker cannot import live runtime authority or Resource custody"
                    .to_string(),
            );
        }
        transport
            .validate_entry_scopes(self.scopes.len())
            .map_err(|error| error.to_string())?;
        let invocation = transport
            .checked
            .entry(definition)
            .map_err(|error| error.to_string())?;
        let name = self.exact_registered_resource_function(definition)?;
        let saved_context = SavedResourceEntryContext::capture(self)?;
        let scope_depth = saved_context.scope_depth();
        let operation = self
            .resource_transport
            .as_mut()
            .ok_or("missing checked Resource transport")?
            .begin_operation()
            .map_err(|error| error.to_string())?;
        self.resource_transport
            .as_mut()
            .ok_or("missing checked Resource transport")?
            .checked_source_active = true;
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let arguments = arguments
                .into_iter()
                .map(EvaluatedValue::ordinary)
                .collect();
            let value = self.call_registered_function_envelopes(
                &name,
                &[],
                arguments,
                &invocation,
                operation.frame,
            )?;
            let flow = self.finish_resource_operation(operation, Ok(Self::resource_flow(value)))?;
            match flow {
                ExprFlow::Value(value) if !value.contains_live_resource_or_grant() => Ok(value),
                _ => Err(
                    "checked entry cannot export live custody through a raw Value result"
                        .to_string(),
                ),
            }
        }));
        let cleanup = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            self.resource_transport
                .as_mut()
                .ok_or("missing checked Resource transport")?
                .unwind_all()
                .map_err(|error| error.to_string())
        }));
        // A panic may skip ordinary callee/scope restoration. Restore the exact
        // entry metadata before choosing/resuming any result or cleanup outcome.
        let restore = saved_context.restore(self);
        let completion = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            self.resource_transport
                .as_mut()
                .ok_or("missing checked Resource transport")?
                .restore_entry_scopes(scope_depth)
                .map_err(|error| error.to_string())
        }));
        let cleanup = Self::complete_resource_cleanup(cleanup, completion);
        let result = match (result, restore) {
            (Ok(Ok(_)), Err(error)) => Ok(Err(error)),
            (result, _) => result,
        };
        Self::complete_resource_entry(result, cleanup)
    }

    fn complete_resource_cleanup(
        cleanup: std::thread::Result<Result<(), String>>,
        completion: std::thread::Result<Result<(), String>>,
    ) -> std::thread::Result<Result<(), String>> {
        match (cleanup, completion) {
            (Err(panic), completion) => {
                if let Err(other) = completion {
                    std::mem::forget(other);
                }
                Err(panic)
            }
            (Ok(Err(error)), completion) => {
                if let Err(panic) = completion {
                    std::mem::forget(panic);
                }
                Ok(Err(error))
            }
            (Ok(Ok(())), completion) => completion,
        }
    }

    fn complete_resource_entry<T>(
        result: std::thread::Result<Result<T, String>>,
        cleanup: std::thread::Result<Result<(), String>>,
    ) -> Result<T, String> {
        match (result, cleanup) {
            (Err(entry_panic), Err(cleanup_panic)) => {
                std::mem::forget(entry_panic);
                std::panic::resume_unwind(cleanup_panic)
            }
            (Ok(_), Err(cleanup_panic)) => std::panic::resume_unwind(cleanup_panic),
            (Err(entry_panic), Ok(Err(cleanup_error))) => {
                std::mem::forget(entry_panic);
                Err(cleanup_error)
            }
            (Ok(_), Ok(Err(cleanup_error))) => Err(cleanup_error),
            (Err(entry_panic), Ok(Ok(()))) => std::panic::resume_unwind(entry_panic),
            (Ok(result), Ok(Ok(()))) => result,
        }
    }
}

#[cfg(test)]
impl Interpreter {
    pub(crate) fn install_resource_test_script(
        &mut self,
        operations: Vec<crate::resource_execution::ScriptOperation>,
    ) -> Result<Value, String> {
        let grant = self
            .resource_transport
            .as_mut()
            .ok_or("missing checked Resource transport")?
            .install_script(operations)
            .map_err(|error| error.to_string())?;
        Ok(Value::GrantedNetwork(grant))
    }
    pub(crate) fn resource_test_scoped_context(
        &self,
    ) -> Result<(bool, usize, usize, usize, Option<usize>), String> {
        let transport = self
            .resource_transport
            .as_ref()
            .ok_or("missing checked Resource transport")?;
        Ok((
            self.active_checked_scope.is_some(),
            self.scoped_type_bindings.len(),
            self.current_type_arguments.len(),
            self.type_arg_scopes.len(),
            transport.checked.original_body_scope_depth(),
        ))
    }
    pub(crate) fn resource_test_custody_counts(&self) -> Result<(usize, usize), String> {
        let transport = self
            .resource_transport
            .as_ref()
            .ok_or("missing checked Resource transport")?;
        Ok((transport.live_owners(), transport.registry_live_count()))
    }
    pub(crate) fn resource_test_observations(
        &self,
    ) -> Result<(Vec<crate::resource_execution::ProviderEvent>, usize, usize), String> {
        let transport = self
            .resource_transport
            .as_ref()
            .ok_or("missing checked Resource transport")?;
        Ok((
            transport
                .provider_events()
                .map_err(|error| error.to_string())?,
            transport.live_owners(),
            transport.registry_live_count(),
        ))
    }
}

#[cfg(test)]
mod completion_tests {
    use super::*;
    use std::panic::{AssertUnwindSafe, catch_unwind};

    #[derive(Clone, Copy)]
    enum Outcome {
        Success,
        Error,
        Panic,
    }

    #[test]
    fn checked_entry_completion_selects_cleanup_before_every_entry_outcome() {
        for entry in [Outcome::Success, Outcome::Error, Outcome::Panic] {
            for cleanup in [Outcome::Success, Outcome::Error, Outcome::Panic] {
                let entry_result: std::thread::Result<Result<Value, String>> = match entry {
                    Outcome::Success => Ok(Ok(Value::Nothing)),
                    Outcome::Error => Ok(Err("entry sentinel".to_string())),
                    Outcome::Panic => Err(Box::new("entry panic")),
                };
                let cleanup_result: std::thread::Result<Result<(), String>> = match cleanup {
                    Outcome::Success => Ok(Ok(())),
                    Outcome::Error => Ok(Err("cleanup sentinel".to_string())),
                    Outcome::Panic => Err(Box::new("cleanup panic")),
                };
                let completed = catch_unwind(AssertUnwindSafe(|| {
                    Interpreter::complete_resource_entry(entry_result, cleanup_result)
                }));
                match (entry, cleanup) {
                    (_, Outcome::Panic) => {
                        let panic = completed.expect_err("cleanup panic must win");
                        assert_eq!(panic.downcast_ref::<&str>(), Some(&"cleanup panic"));
                    }
                    (_, Outcome::Error) => assert_eq!(
                        completed.expect("cleanup error is ordinary"),
                        Err("cleanup sentinel".to_string())
                    ),
                    (Outcome::Panic, Outcome::Success) => {
                        let panic = completed.expect_err("clean cleanup preserves the entry panic");
                        assert_eq!(panic.downcast_ref::<&str>(), Some(&"entry panic"));
                    }
                    (Outcome::Error, Outcome::Success) => assert_eq!(
                        completed.expect("entry error is ordinary"),
                        Err("entry sentinel".to_string())
                    ),
                    (Outcome::Success, Outcome::Success) => {
                        assert_eq!(completed.unwrap().unwrap(), Value::Nothing)
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod declaration_registration_tests {
    use super::*;
    use crate::resource_execution::CheckedExecution;
    use jett_common::FileId;

    const QUALIFIED: &str = r#"namespace helper
export function answer() returns int64:
    return 7
namespace other
export function scenario() returns int64:
    return 99
namespace app
export function scenario() returns int64:
    use helper
    return helper.answer()
"#;

    const MUTUAL: &str = r#"namespace app
mutual:
    function first() returns int64
    function second() returns int64
function first() returns int64:
    return second()
function second() returns int64:
    return 11
export function scenario() returns int64:
    return first()
"#;

    fn target(
        program: &Arc<jett_typecheck::CheckedResourceProgram>,
        namespace: &str,
        name: &str,
    ) -> DefId {
        let checked =
            CheckedExecution::new(program.clone(), ExecutionPurpose::ReferenceRuntime).unwrap();
        let definitions = program
            .module()
            .items
            .iter()
            .filter_map(|item| match item {
                Item::Function(function)
                    if function.name.span.file == FileId::new(0) && function.name.name == name =>
                {
                    let definition = checked.declaration_definition(function.name.span).unwrap();
                    (program.resolved().scope_table.definitions[definition.index() as usize]
                        .namespace
                        .as_deref()
                        == Some(namespace))
                    .then_some(definition)
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        let [definition] = definitions.as_slice() else {
            panic!("unique exact retained test entry");
        };
        *definition
    }

    fn runtime(source: &str, release: bool) -> (Interpreter, DefId) {
        let program = crate::resource_execution::tests::program(source, release);
        let definition = target(&program, "app", "scenario");
        let mut interpreter =
            Interpreter::from_checked_resource_program(program, ExecutionPurpose::ReferenceRuntime)
                .unwrap();
        interpreter
            .install_resource_test_script(Vec::new())
            .unwrap();
        (interpreter, definition)
    }

    fn empty_before_teardown(interpreter: &mut Interpreter) {
        assert_eq!(
            interpreter.resource_test_observations().unwrap(),
            (Vec::new(), 0, 0)
        );
        assert!(interpreter.take_debug_events().is_empty());
    }

    #[test]
    fn checked_registration_calls_original_qualified_and_mutual_bodies() {
        for release in [false, true] {
            for (source, result) in [(QUALIFIED, 7), (MUTUAL, 11)] {
                let (mut interpreter, definition) = runtime(source, release);
                assert_eq!(
                    interpreter
                        .call_checked_program_entry(definition, Vec::new())
                        .unwrap(),
                    Value::Int64(result)
                );
                empty_before_teardown(&mut interpreter);
            }
        }
    }

    #[test]
    fn checked_registration_refuses_namespace_header_missing_wrong_and_duplicate_identity() {
        for release in [false, true] {
            for corruption in 0..5 {
                let (mut interpreter, definition) = runtime(QUALIFIED, release);
                match corruption {
                    0 => {
                        interpreter
                            .functions
                            .get_mut("app.scenario")
                            .unwrap()
                            .namespace = Some("other".to_string())
                    }
                    1 => {
                        let registered = interpreter.functions.get_mut("app.scenario").unwrap();
                        let function = Arc::make_mut(&mut registered.definition);
                        let Some(TypeExpr::Named(result)) = &mut function.return_type else {
                            panic!("named source result");
                        };
                        result.name = "string".to_string();
                    }
                    2 => {
                        interpreter.functions.remove("app.scenario");
                    }
                    3 => {
                        let wrong = interpreter.functions.get("other.scenario").unwrap().clone();
                        interpreter
                            .functions
                            .insert("app.scenario".to_string(), wrong);
                    }
                    4 => {
                        let duplicate = interpreter.functions.get("app.scenario").unwrap().clone();
                        interpreter
                            .functions
                            .insert("host.duplicate".to_string(), duplicate);
                    }
                    _ => unreachable!(),
                }
                let error = interpreter
                    .call_checked_program_entry(definition, Vec::new())
                    .expect_err("malformed registration must refuse before provider activity");
                assert_eq!(
                    error,
                    if corruption < 2 {
                        "checked registration differs from its retained source declaration"
                    } else {
                        "checked invocation has no exact retained source function"
                    }
                );
                empty_before_teardown(&mut interpreter);
            }
        }
    }

    #[test]
    fn checked_registration_generic_body_uses_accepted_concrete_selection() {
        const SOURCE: &str = r#"namespace app
function echo[T](value: T) returns T:
    return value
export function scenario() returns int64:
    return echo[int64](7)
"#;
        for release in [false, true] {
            let (mut interpreter, definition) = runtime(SOURCE, release);
            assert_eq!(
                interpreter
                    .call_checked_program_entry(definition, Vec::new())
                    .unwrap(),
                Value::Int64(7)
            );
            empty_before_teardown(&mut interpreter);
        }
    }

    #[test]
    fn checked_registration_cloned_body_cannot_replace_retained_executable_source() {
        for release in [false, true] {
            let (mut interpreter, definition) = runtime(QUALIFIED, release);
            let registered = interpreter.functions.get_mut("helper.answer").unwrap();
            let function = Arc::make_mut(&mut registered.definition);
            let Stmt::Return(result) = &mut function.body.stmts[0] else {
                panic!("original return");
            };
            let original_span = result.value.as_ref().unwrap().span();
            result.value = Some(Expr::IntLiteral(99, original_span));
            // Header/body spans are unchanged; executable authority remains the
            // exact retained program rather than this host-edited clone.
            assert_eq!(
                interpreter
                    .call_checked_program_entry(definition, Vec::new())
                    .unwrap(),
                Value::Int64(7)
            );
            empty_before_teardown(&mut interpreter);
        }
    }
}

#[cfg(test)]
mod entry_readiness_tests {
    use super::*;
    use crate::resource_execution::{
        ProviderEvent, ResourceExecutionError, ScriptOperation, tests::program,
    };
    use jett_types::ResourceHookKind;

    #[test]
    fn checked_host_entry_refusal_preserves_an_outer_operation_and_real_owner() {
        for release in [false, true] {
            let program = program(
                include_str!("fixtures/25_repeated_scoped_entry.jett"),
                release,
            );
            let mut interpreter = Interpreter::from_checked_resource_program(
                program.clone(),
                ExecutionPurpose::ReferenceRuntime,
            )
            .unwrap();
            let original = program
                .module()
                .items
                .iter()
                .find_map(|item| match item {
                    Item::Function(function)
                        if function.name.name == "clean"
                            && function.name.span.file == jett_common::FileId::new(0) =>
                    {
                        Some(function)
                    }
                    _ => None,
                })
                .unwrap();
            let target = interpreter
                .resource_transport
                .as_ref()
                .unwrap()
                .checked
                .declaration_definition(original.name.span)
                .unwrap();
            let grant = interpreter
                .install_resource_test_script(vec![
                    ScriptOperation::Construct {
                        label: 1801,
                        outcome: Ok(()),
                    },
                    ScriptOperation::Construct {
                        label: 1801,
                        outcome: Ok(()),
                    },
                    ScriptOperation::Borrow {
                        label: 1801,
                        outcome: Ok(21),
                    },
                ])
                .unwrap();
            let Value::GrantedNetwork(network) = &grant else {
                panic!("actual private grant");
            };
            let runtime = interpreter.resource_transport.as_mut().unwrap();
            runtime.checked_source_active = true;
            let operation = runtime.begin_operation().unwrap();
            let hook = runtime
                .checked
                .program()
                .checked()
                .resource_hooks
                .values()
                .find(|hook| hook.kind == ResourceHookKind::Construct)
                .unwrap()
                .definition;
            let occurrence = *runtime
                .checked
                .program()
                .checked()
                .call_ownership
                .iter()
                .find(|(_, packet)| packet.target == CheckedInvocationTarget::Resolved(hook))
                .unwrap()
                .0;
            let invocation = runtime.checked.invocation(occurrence).unwrap();
            let descriptor = runtime.checked.descriptor(hook).unwrap();
            let owner = runtime
                .invoke_hook(
                    &invocation,
                    &descriptor,
                    vec![
                        EvaluatedValue::ordinary(Value::GrantedNetwork(network.clone())),
                        EvaluatedValue::ordinary(Value::Int64(1801)),
                    ],
                )
                .unwrap();
            assert!(!owner.custody.is_empty());
            let context = interpreter.resource_test_entry_context().unwrap();
            assert_eq!(
                interpreter.call_checked_program_entry(target, vec![grant.clone()]),
                Err(ResourceExecutionError::InvalidFrame.to_string())
            );
            assert_eq!(interpreter.resource_test_entry_context().unwrap(), context);
            assert_eq!(
                interpreter.resource_test_observations().unwrap(),
                (vec![ProviderEvent::Constructed(1801)], 1, 1)
            );
            interpreter
                .resource_transport
                .as_mut()
                .unwrap()
                .end_operation(operation, None)
                .unwrap();
            drop(owner);
            interpreter
                .resource_transport
                .as_mut()
                .unwrap()
                .checked_source_active = false;
            assert_eq!(
                interpreter.resource_test_observations().unwrap(),
                (
                    vec![
                        ProviderEvent::Constructed(1801),
                        ProviderEvent::Finalized(1801)
                    ],
                    0,
                    0
                )
            );
            assert_eq!(
                interpreter
                    .call_checked_program_entry(target, vec![grant])
                    .unwrap(),
                Value::Int64(21)
            );
            assert_eq!(
                interpreter.resource_test_observations().unwrap(),
                (
                    vec![
                        ProviderEvent::Constructed(1801),
                        ProviderEvent::Finalized(1801),
                        ProviderEvent::Constructed(1801),
                        ProviderEvent::Borrowed(1801),
                        ProviderEvent::Finalized(1801)
                    ],
                    0,
                    0
                )
            );
            assert!(interpreter.take_debug_events().is_empty());
        }
    }
}
