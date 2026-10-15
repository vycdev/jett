//! Exact actor declaration/handler entry, independent of Resource execution.
use super::*;
use crate::resource_execution::{CheckedBodyReference, PreparedActorHandler};
use jett_typecheck::{CheckedActorBindingRole, CheckedCallerEffect};

/// Created only by the active, same-program checked Source Spawn route below.
/// Handler entry remains unable to install any Resource executable cursor.
struct CheckedResourceActorInitializers {
    program: Arc<jett_typecheck::CheckedResourceProgram>,
    bodies: Vec<CheckedBodyReference>,
}

impl CheckedResourceActorInitializers {
    fn evaluate(&self, interpreter: &mut Interpreter, field: usize) -> Result<ExprFlow, String> {
        let transport = interpreter
            .resource_transport
            .as_mut()
            .filter(|transport| Arc::ptr_eq(transport.checked.program(), &self.program))
            .ok_or("actor initializer lost its original checked Resource program")?;
        let reference = self
            .bodies
            .get(field)
            .ok_or("actor initializer lost its original field")?;
        let previous_active = std::mem::replace(&mut transport.checked_source_active, true);
        // This existing entry verifies and installs the retained initializer,
        // evaluates its original expression and restores its Resource cursor.
        let result = interpreter.eval_checked_actor_initializer(reference);
        let restored = interpreter
            .resource_transport
            .as_mut()
            .filter(|transport| Arc::ptr_eq(transport.checked.program(), &self.program))
            .ok_or_else(|| {
                "actor initializer lost its original checked Resource program".to_string()
            })
            .map(|transport| transport.checked_source_active = previous_active);
        match result {
            Err(error) => Err(error),
            Ok(value) => restored.map(|()| value),
        }
    }
}

impl Interpreter {
    fn actor_checked_type(
        handler: &PreparedActorHandler,
        ty: jett_types::TypeId,
        span: Span,
    ) -> Result<TypeExpr, String> {
        Self::actor_program_type(handler.program(), ty, span)
    }

    fn actor_program_type(
        program: &jett_typecheck::CheckedResourceProgram,
        ty: jett_types::TypeId,
        span: Span,
    ) -> Result<TypeExpr, String> {
        let name = program.checked().interner.type_name(ty);
        Self::simple_type_expr_from_name(&name, span)
            .ok_or_else(|| "actor binding has no canonical checked type".into())
    }

    /// Result paths restore every sender context. This does not promise unwind rollback.
    fn with_actor_frame<T>(
        &mut self,
        handler: Option<&PreparedActorHandler>,
        namespace: Option<String>,
        body: impl FnOnce(&mut Self, usize) -> Result<T, String>,
    ) -> Result<T, String> {
        let projection = handler
            .map(|handler| handler.projection().map_err(|error| error.to_string()))
            .transpose()?;
        let capture = handler
            .map(|handler| {
                crate::value::closure_capture::OriginalClosureOwner::actor_handler(
                    handler.program(),
                    handler.target(),
                    handler.namespace(),
                )
                .ok_or_else(|| "actor handler lost its original capture owner".to_string())
            })
            .transpose()?;
        let saved_field =
            self.enter_checked_field_body(handler.map(|handler| handler.reference()))?;
        let base = self.scopes.len();
        let saved_floor = std::mem::replace(&mut self.lexical_scope_floor, base);
        let saved_namespace = std::mem::replace(&mut self.current_namespace, namespace);
        let saved_trusted = std::mem::replace(&mut self.current_function_trusted_stdlib, false);
        let saved_function = self.active_checked_function.take();
        let saved_scope = std::mem::replace(&mut self.active_checked_scope, projection);
        let saved_capture = std::mem::replace(&mut self.active_reflected_capture_origin, capture);
        let saved_bindings = std::mem::take(&mut self.scoped_type_bindings);
        let saved_type_scopes = std::mem::take(&mut self.type_arg_scopes);
        let saved_arguments = std::mem::take(&mut self.current_type_arguments);
        let saved_reflected = std::mem::take(&mut self.checked_reflected_fields);
        let saved_proofs = std::mem::replace(&mut self.allow_checked_refinement_proofs, true);
        let saved_resource = self
            .resource_transport
            .as_mut()
            .map(|transport| std::mem::replace(&mut transport.checked_source_active, false));
        self.push_scope();
        let result = body(self, base);
        while self.scopes.len() > base {
            self.pop_scope();
        }
        self.lexical_scope_floor = saved_floor;
        self.current_namespace = saved_namespace;
        self.current_function_trusted_stdlib = saved_trusted;
        self.active_checked_function = saved_function;
        self.active_checked_scope = saved_scope;
        self.active_reflected_capture_origin = saved_capture;
        self.scoped_type_bindings = saved_bindings;
        self.type_arg_scopes = saved_type_scopes;
        self.current_type_arguments = saved_arguments;
        self.checked_reflected_fields = saved_reflected;
        self.allow_checked_refinement_proofs = saved_proofs;
        if let (Some(transport), Some(saved)) = (&mut self.resource_transport, saved_resource) {
            transport.checked_source_active = saved;
        }
        let restored = self.restore_checked_field_body(saved_field);
        match result {
            Err(error) => Err(error),
            Ok(value) => restored.map(|()| value),
        }
    }

    pub(super) fn eval_spawn_flow(&mut self, source: &Expr) -> Result<ExprFlow, String> {
        let Expr::Spawn(inner, _) = source else {
            return Err("spawn: expected original Spawn".into());
        };
        let Some(context) = self
            .field_update_context
            .as_ref()
            .filter(|context| context.active)
        else {
            return self.eval_spawn_flow_unchecked(inner);
        };
        let Expr::Call(callee, arguments, _) = inner.as_ref() else {
            return Err("spawn: expected call expression".into());
        };
        let name = Self::dotted_expr_name(callee).ok_or("spawn: expected actor type name")?;
        let name = self
            .registry_name(&self.actor_defs, &name)
            .ok_or("spawn: unknown checked actor type")?;
        let registered = self
            .actor_defs
            .get(&name)
            .ok_or("spawn: missing checked actor registration")?;
        let prepared = context
            .checked
            .prepare_actor_spawn(source, registered)
            .map_err(|error| error.to_string())?;
        let actor = prepared.original().map_err(|error| error.to_string())?;
        let resource_initializers = self
            .resource_transport
            .as_ref()
            .filter(|transport| transport.checked_source_active)
            .map(|transport| -> Result<_, String> {
                if !Arc::ptr_eq(transport.checked.program(), prepared.program()) {
                    return Err(
                        "actor initializer belongs to a foreign checked Resource program".into(),
                    );
                }
                let bodies = transport
                    .checked
                    .prepare_actor_initializers(registered)
                    .map_err(|error| error.to_string())?;
                if bodies.len() != actor.state_fields.len() {
                    return Err("actor initializer changed its original fields".into());
                }
                for (reference, field) in bodies.iter().zip(&actor.state_fields) {
                    let original = reference
                        .original_expression()
                        .map_err(|error| error.to_string())?;
                    if !std::ptr::eq(original, &field.value) {
                        return Err("actor initializer changed its original Source".into());
                    }
                }
                Ok(CheckedResourceActorInitializers {
                    program: prepared.program().clone(),
                    bodies,
                })
            })
            .transpose()?;
        let mut capabilities = HashMap::new();
        for (argument, &index) in arguments.iter().zip(prepared.parameter_indices()) {
            let value = match self.eval_expr_flow(&argument.value)? {
                ExprFlow::Value(value) => value,
                ExprFlow::Signal(signal) => return Ok(ExprFlow::Signal(signal)),
                ExprFlow::Resource(_) => {
                    return Err("actor Resource custody has no checked transport proof".into());
                }
            };
            if value.contains_live_resource_or_grant() {
                return Err("actor Resource custody has no checked transport proof".into());
            }
            let parameter = &actor.capability_params[index];
            let ty = Self::actor_program_type(
                prepared.program(),
                prepared
                    .capability_type(index)
                    .map_err(|error| error.to_string())?,
                parameter.name.span,
            )?;
            let value = self.normalize_value_for_type(&ty, value)?;
            capabilities.insert(parameter.name.name.clone(), value);
        }
        let state =
            self.with_actor_frame(None, prepared.namespace().map(str::to_owned), |this, _| {
                for (index, parameter) in actor.capability_params.iter().enumerate() {
                    let value = capabilities
                        .get(&parameter.name.name)
                        .ok_or("spawn lost a capability argument")?
                        .clone();
                    let ty = Self::actor_program_type(
                        prepared.program(),
                        prepared
                            .capability_type(index)
                            .map_err(|error| error.to_string())?,
                        parameter.name.span,
                    )?;
                    this.set_variable_with_type(&parameter.name.name, value, ty);
                }
                let mut state = HashMap::new();
                for (index, (field, reference)) in actor
                    .state_fields
                    .iter()
                    .zip(prepared.initializers())
                    .enumerate()
                {
                    let flow = this.with_checked_field_region(reference, |this| {
                        match &resource_initializers {
                            Some(initializers) => initializers.evaluate(this, index),
                            None => this.eval_expr_flow(&field.value),
                        }
                    })?;
                    let value = match flow {
                        ExprFlow::Value(value) => value,
                        ExprFlow::Resource(_) => {
                            return Err(
                                "actor Resource custody has no checked transport proof".into()
                            );
                        }
                        ExprFlow::Signal(_) => {
                            return Err("actor initializer cannot transfer a control signal".into());
                        }
                    };
                    if value.contains_live_resource_or_grant() {
                        return Err("actor Resource custody has no checked transport proof".into());
                    }
                    let ty = Self::actor_program_type(
                        prepared.program(),
                        prepared
                            .state_type(index)
                            .map_err(|error| error.to_string())?,
                        field.name.span,
                    )?;
                    let value = this.normalize_value_for_type(&ty, value)?;
                    // Later initializers resolve actual earlier state in declaration order.
                    this.set_variable_with_type(&field.name.name, value.clone(), ty);
                    state.insert(field.name.name.clone(), value);
                }
                Ok(state)
            })?;
        let identity = self.next_actor_id;
        self.next_actor_id = identity
            .checked_add(1)
            .ok_or("actor identity space exhausted")?;
        self.actor_instances.insert(
            identity,
            ActorInstance {
                type_name: name,
                state,
                capabilities,
                checked_origin: Some(prepared.origin()),
            },
        );
        Ok(ExprFlow::Value(Value::Actor(identity)))
    }

    pub(super) fn eval_actor_message_flow(&mut self, source: &Expr) -> Result<ExprFlow, String> {
        let is_ask = matches!(source, Expr::Ask(..));
        let inner = match source {
            Expr::Send(inner, _) | Expr::Ask(inner, _) => inner.as_ref(),
            _ => return Err("send/ask: missing original message occurrence".into()),
        };
        let Some(context) = self
            .field_update_context
            .as_ref()
            .filter(|context| context.active)
        else {
            return self
                .eval_actor_message_unchecked(inner, is_ask)
                .map(ExprFlow::Value);
        };
        let prepared = context
            .checked
            .prepare_actor_message(source)
            .map_err(|error| error.to_string())?;
        let receiver = prepared.receiver().map_err(|error| error.to_string())?;
        let value = match self.eval_expr_flow(receiver)? {
            ExprFlow::Value(value) => value,
            ExprFlow::Signal(signal) => return Ok(ExprFlow::Signal(signal)),
            ExprFlow::Resource(_) => {
                return Err("actor Resource custody has no checked transport proof".into());
            }
        };
        let Value::Actor(identity) = value.payload() else {
            return Err("send/ask: expected actor value".into());
        };
        let identity = *identity;
        let mut values = Vec::new();
        for argument in prepared.arguments().map_err(|error| error.to_string())? {
            let value = match self.eval_expr_flow(&argument.value)? {
                ExprFlow::Value(value) => value,
                ExprFlow::Signal(signal) => return Ok(ExprFlow::Signal(signal)),
                ExprFlow::Resource(_) => {
                    return Err("actor Resource custody has no checked transport proof".into());
                }
            };
            if value.contains_live_resource_or_grant() {
                return Err("actor Resource custody has no checked transport proof".into());
            }
            values.push(value);
        }
        let context = self
            .field_update_context
            .as_ref()
            .filter(|context| context.active)
            .ok_or("message lost its checked caller")?;
        context
            .checked
            .revalidate_actor_message(&prepared)
            .map_err(|error| error.to_string())?;
        let instance = self
            .actor_instances
            .get(&identity)
            .ok_or("unknown actor instance")?;
        if instance
            .state
            .values()
            .chain(instance.capabilities.values())
            .any(Value::contains_live_resource_or_grant)
        {
            return Err("actor Resource custody has no checked transport proof".into());
        }
        let registered = self
            .actor_defs
            .get(&instance.type_name)
            .ok_or("unknown actor registration")?;
        let handler = context
            .checked
            .prepare_actor_handler(&prepared, registered)
            .map_err(|error| error.to_string())?;
        if !instance
            .checked_origin
            .as_ref()
            .is_some_and(|origin| handler.matches_instance(origin))
        {
            return Err("actor instance has no matching checked program origin".into());
        }
        let state = instance.state.clone();
        let capabilities = instance.capabilities.clone();
        let (actor, original) = handler.original().map_err(|error| error.to_string())?;
        let mut arguments = vec![Value::Nothing; original.params.len()];
        for (fact, value) in prepared.packet().arguments.iter().zip(values) {
            if !matches!(
                fact.effect,
                CheckedCallerEffect::Copy
                    | CheckedCallerEffect::TransferOwned
                    | CheckedCallerEffect::RelinquishOwned
            ) {
                return Err("actor message has no owned caller transfer".into());
            }
            let ty = Self::actor_checked_type(&handler, fact.parameter_type, fact.source_span)?;
            let source_ty = handler
                .program()
                .checked()
                .interner
                .type_name(fact.actual_type);
            arguments[fact.parameter_index] =
                self.normalize_and_validate_value_from_source(&ty, value, Some(&source_ty))?;
        }
        let metadata = handler.metadata().map_err(|error| error.to_string())?;
        let (response, updated_state) = self.with_actor_frame(
            Some(&handler),
            handler.namespace().map(str::to_owned),
            |this, captured| {
                for binding in metadata.bindings() {
                    let value = match binding.role {
                        CheckedActorBindingRole::Capability { parameter } => {
                            capabilities.get(&actor.capability_params[parameter].name.name)
                        }
                        CheckedActorBindingRole::State { field } => {
                            state.get(&actor.state_fields[field].name.name)
                        }
                        CheckedActorBindingRole::Message { .. } => continue,
                    }
                    .ok_or("actor capture lost its original slot")?
                    .clone();
                    let name = handler
                        .role_name(binding.role)
                        .map_err(|error| error.to_string())?;
                    let ty = Self::actor_checked_type(&handler, binding.fact.ty, name.span)?;
                    this.set_variable_with_type(&name.name, value, ty);
                    this.install_checked_actor_binding(&handler, binding.role)?;
                }
                this.push_scope();
                for (parameter, value) in arguments.into_iter().enumerate() {
                    let role = CheckedActorBindingRole::Message { parameter };
                    let binding = metadata
                        .bindings()
                        .iter()
                        .find(|binding| binding.role == role)
                        .ok_or("actor message lost its checked parameter role")?;
                    let name = handler.role_name(role).map_err(|error| error.to_string())?;
                    let ty = Self::actor_checked_type(&handler, binding.fact.ty, name.span)?;
                    this.set_variable_with_type(&name.name, value, ty);
                    this.install_checked_actor_binding(&handler, role)?;
                }
                let mut response = Value::Nothing;
                for statement in &original.body.stmts {
                    match this.exec_stmt_inner(statement)? {
                        Some(Signal::Respond(value)) => {
                            response = value;
                            break;
                        }
                        Some(Signal::ResourceReturn(..) | Signal::ResourceDefault(..)) => {
                            return Err(
                                "actor Resource custody has no checked transport proof".into()
                            );
                        }
                        Some(
                            Signal::Return(..)
                            | Signal::Break
                            | Signal::Continue
                            | Signal::Default(_),
                        ) => break,
                        None => {}
                    }
                }
                let mut updated = HashMap::new();
                for binding in metadata.bindings() {
                    let CheckedActorBindingRole::State { field } = binding.role else {
                        continue;
                    };
                    let name = &actor.state_fields[field].name;
                    let value = this
                        .scopes
                        .get(captured)
                        .and_then(|scope| scope.get(&name.name))
                        .ok_or("actor state lost its exact captured slot")?
                        .clone();
                    let ty = Self::actor_checked_type(&handler, binding.fact.ty, name.span)?;
                    updated.insert(
                        name.name.clone(),
                        this.normalize_value_for_type(&ty, value)?,
                    );
                }
                Ok((response, updated))
            },
        )?;
        // Preserve established publication order: state first, reply normalization afterwards.
        self.actor_instances
            .get_mut(&identity)
            .ok_or("actor instance disappeared before publication")?
            .state = updated_state;
        if is_ask {
            let ty = Self::actor_checked_type(&handler, metadata.response_type(), original.span)?;
            self.normalize_value_for_type(&ty, response)
                .map(ExprFlow::Value)
        } else {
            Ok(ExprFlow::Value(Value::Nothing))
        }
    }
}

#[cfg(test)]
#[path = "actor_handlers/tests.rs"]
mod tests;
