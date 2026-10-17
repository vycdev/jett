//! Ordinary actor entry uses original actor/handler identity, never function or Resource authority.
use super::*;
use jett_parser::ast::{ActorDef, Ident, ReceiveHandler};
use jett_typecheck::actor_messages::{
    CheckedActorBindingRole, CheckedActorHandler, CheckedActorHandlerTarget,
    CheckedActorMessageCall, CheckedActorMessageKind, CheckedActorMessageRoot,
};
use jett_types::ActorId;

#[derive(Clone)]
pub(crate) struct CheckedActorInstanceOrigin {
    program: Arc<CheckedResourceProgram>,
    definition: DefId,
    actor: ActorId,
    item: usize,
}

pub(crate) struct PreparedActorMessage<'source> {
    source: &'source Expr,
    key: CheckedAttemptKey,
    packet: CheckedActorMessageCall,
}

pub(crate) struct PreparedActorHandler {
    reference: CheckedBodyReference,
    origin: CheckedActorInstanceOrigin,
}

pub(crate) struct PreparedActorSpawn {
    origin: CheckedActorInstanceOrigin,
    parameter_indices: Vec<usize>,
    initializers: Vec<CheckedBodyReference>,
}

fn message_parts(
    source: &Expr,
) -> Result<(CheckedActorMessageKind, &Expr, &[CallArg], &Ident), ResourceExecutionError> {
    let (kind, inner) = match source {
        Expr::Send(inner, _) => (CheckedActorMessageKind::Send, inner.as_ref()),
        Expr::Ask(inner, _) => (CheckedActorMessageKind::Ask, inner.as_ref()),
        _ => return Err(ResourceExecutionError::InvalidInvocation),
    };
    let (callee, args) = match inner {
        Expr::Call(callee, args, _) => (callee.as_ref(), args.as_slice()),
        Expr::FieldAccess(..) => (inner, &[][..]),
        _ => return Err(ResourceExecutionError::InvalidInvocation),
    };
    let Expr::FieldAccess(receiver, member, _) = callee else {
        return Err(ResourceExecutionError::InvalidInvocation);
    };
    Ok((kind, receiver, args, member))
}

impl CheckedActorInstanceOrigin {
    /// Caller must already own the genuine checked graph import or Spawn occurrence.
    /// This brand grants no provider/custody or generic function entry.
    pub(crate) fn from_checked_actor(
        program: Arc<CheckedResourceProgram>,
        actor: ActorId,
    ) -> Result<Self, ResourceExecutionError> {
        let canonical = &program.checked().interner.resolve_actor(actor).name;
        let matches = program.module().items.iter().enumerate().filter_map(|(item, node)| {
            let Item::Actor(original) = node else { return None };
            let mut declarations = program.resolved().scope_table.definitions.iter().filter(|definition| {
                definition.kind == jett_resolve::DefKind::Actor
                    && definition.span == original.name.span && definition.name == *canonical
                    && program.checked().definition_types.get(&definition.id)
                        .is_some_and(|ty| matches!(program.checked().interner.resolve(*ty), Type::Actor(actual) if *actual == actor))
            });
            let declaration = declarations.next()?;
            if declarations.next().is_some() { return None; }
            Some((item, declaration.id))
        }).collect::<Vec<_>>();
        let [(item, definition)] = matches.as_slice() else {
            return Err(ResourceExecutionError::MissingCheckedBody);
        };
        Ok(Self {
            program,
            definition: *definition,
            actor,
            item: *item,
        })
    }

    pub(crate) fn original(&self) -> Result<&ActorDef, ResourceExecutionError> {
        match self.program.module().items.get(self.item) {
            Some(Item::Actor(actor)) => Ok(actor),
            _ => Err(ResourceExecutionError::MissingCheckedBody),
        }
    }

    pub(crate) fn matches_registration(
        &self,
        registered: &ActorDef,
    ) -> Result<(), ResourceExecutionError> {
        let mut original = self.original()?.clone();
        let declaration = self
            .program
            .resolved()
            .scope_table
            .definitions
            .get(self.definition.index() as usize)
            .filter(|definition| {
                definition.id == self.definition
                    && definition.kind == jett_resolve::DefKind::Actor
                    && definition.span == original.name.span
            })
            .ok_or(ResourceExecutionError::MissingCheckedBody)?;
        original.name.name = declaration.name.clone();
        if format!("{original:?}") != format!("{registered:?}") {
            return Err(ResourceExecutionError::MissingCheckedBody);
        }
        Ok(())
    }
}

impl PreparedActorMessage<'_> {
    pub(crate) fn packet(&self) -> &CheckedActorMessageCall {
        &self.packet
    }
    pub(crate) fn receiver(&self) -> Result<&Expr, ResourceExecutionError> {
        Ok(message_parts(self.source)?.1)
    }
    pub(crate) fn arguments(&self) -> Result<&[CallArg], ResourceExecutionError> {
        Ok(message_parts(self.source)?.2)
    }
}

impl PreparedActorSpawn {
    pub(crate) fn original(&self) -> Result<&ActorDef, ResourceExecutionError> {
        self.origin.original()
    }
    pub(crate) fn origin(&self) -> CheckedActorInstanceOrigin {
        self.origin.clone()
    }
    pub(crate) fn program(&self) -> &Arc<CheckedResourceProgram> {
        &self.origin.program
    }
    pub(crate) fn capability_type(
        &self,
        parameter: usize,
    ) -> Result<TypeId, ResourceExecutionError> {
        self.origin
            .program
            .checked()
            .interner
            .resolve_actor(self.origin.actor)
            .capability_params
            .get(parameter)
            .map(|(_, ty)| *ty)
            .ok_or(ResourceExecutionError::InvalidInvocation)
    }
    pub(crate) fn state_type(&self, field: usize) -> Result<TypeId, ResourceExecutionError> {
        self.origin
            .program
            .checked()
            .interner
            .resolve_actor(self.origin.actor)
            .state_fields
            .get(field)
            .map(|(_, ty)| *ty)
            .ok_or(ResourceExecutionError::InvalidOwner)
    }
    pub(crate) fn parameter_indices(&self) -> &[usize] {
        &self.parameter_indices
    }
    pub(crate) fn namespace(&self) -> Option<&str> {
        self.origin.program.resolved().scope_table.definitions
            [self.origin.definition.index() as usize]
            .namespace
            .as_deref()
    }
    pub(crate) fn initializers(&self) -> &[CheckedBodyReference] {
        &self.initializers
    }
}

impl PreparedActorHandler {
    pub(crate) fn target(&self) -> CheckedActorHandlerTarget {
        let BodyRoot::ActorHandler(target) = self.reference.cursor.root else {
            unreachable!("private actor handler cursor")
        };
        target
    }
    pub(crate) fn reference(&self) -> &CheckedBodyReference {
        &self.reference
    }
    pub(crate) fn original(&self) -> Result<(&ActorDef, &ReceiveHandler), ResourceExecutionError> {
        let actor = self.origin.original()?;
        let handler = actor
            .handlers
            .get(self.target().handler_index)
            .ok_or(ResourceExecutionError::MissingCheckedBody)?;
        Ok((actor, handler))
    }
    pub(crate) fn metadata(&self) -> Result<&CheckedActorHandler, ResourceExecutionError> {
        self.origin
            .program
            .checked()
            .actor_handler(self.target())
            .ok_or(ResourceExecutionError::MissingCheckedBody)
    }
    pub(crate) fn program(&self) -> &Arc<CheckedResourceProgram> {
        &self.origin.program
    }
    pub(crate) fn namespace(&self) -> Option<&str> {
        self.origin.program.resolved().scope_table.definitions
            [self.origin.definition.index() as usize]
            .namespace
            .as_deref()
    }
    pub(crate) fn projection(&self) -> Result<Arc<CheckedScopedTypes>, ResourceExecutionError> {
        let body = self.metadata()?.body();
        let types = &self.origin.program.checked().interner;
        let (expressions, bindings) = crate::checked_types::original_body_types(
            &body.type_map,
            &body.comptime_type_bindings,
            types,
        );
        Ok(Arc::new(CheckedScopedTypes {
            bound_type: types.resolve_actor(self.origin.actor).name.clone(),
            reflection: None,
            expressions,
            bindings,
        }))
    }
    pub(crate) fn matches_instance(&self, origin: &CheckedActorInstanceOrigin) -> bool {
        Arc::ptr_eq(&self.origin.program, &origin.program)
            && self.origin.definition == origin.definition
            && self.origin.actor == origin.actor
            && self.origin.item == origin.item
    }
    pub(crate) fn role_name(
        &self,
        role: CheckedActorBindingRole,
    ) -> Result<&Ident, ResourceExecutionError> {
        let (actor, handler) = self.original()?;
        match role {
            CheckedActorBindingRole::Capability { parameter } => actor
                .capability_params
                .get(parameter)
                .map(|param| &param.name),
            CheckedActorBindingRole::State { field } => {
                actor.state_fields.get(field).map(|field| &field.name)
            }
            CheckedActorBindingRole::Message { parameter } => {
                handler.params.get(parameter).map(|param| &param.name)
            }
        }
        .ok_or(ResourceExecutionError::MissingCheckedBody)
    }
}

impl CheckedExecution {
    fn original_actor_occurrence(&self, source: &Expr) -> Result<(), ResourceExecutionError> {
        self.validate_field_executable_cursor(&self.body)?;
        let reference = CheckedBodyReference {
            cursor: self.body.clone(),
        };
        let mut matches = 0;
        walk_region(
            reference.region()?,
            &mut |_| {},
            &mut |node| {
                if std::ptr::eq(node, source) {
                    matches += 1;
                }
            },
            &mut |_| {},
        );
        if matches != 1 {
            return Err(ResourceExecutionError::MissingCheckedBody);
        }
        Ok(())
    }

    pub(crate) fn prepare_actor_message<'source>(
        &self,
        source: &'source Expr,
    ) -> Result<PreparedActorMessage<'source>, ResourceExecutionError> {
        self.original_actor_occurrence(source)?;
        let (kind, receiver, arguments, member) = message_parts(source)?;
        let facts = self.facts(&self.body)?;
        let packet = facts
            .actor_messages
            .get(&source.span())
            .ok_or(ResourceExecutionError::MissingCheckedInvocation)?;
        let root = match self.body.root {
            BodyRoot::Ordinary => CheckedActorMessageRoot::Ordinary,
            BodyRoot::Generic(index) => CheckedActorMessageRoot::Generic(index),
            BodyRoot::ActorHandler(target) => CheckedActorMessageRoot::ActorHandler(target),
        };
        let scopes = self
            .body
            .scopes
            .iter()
            .map(|scope| (scope.owner, scope.index))
            .collect::<Vec<_>>();
        if packet.source_span != source.span()
            || !self.program.checked().validates_actor_message_at(
                packet,
                root,
                &scopes,
                source.span(),
            )
        {
            return Err(ResourceExecutionError::InvalidInvocation);
        }
        let origin = CheckedActorInstanceOrigin::from_checked_actor(
            self.program.clone(),
            packet.target.actor,
        )?;
        let actor = origin.original()?;
        let handler = actor
            .handlers
            .get(packet.target.handler_index)
            .ok_or(ResourceExecutionError::InvalidInvocation)?;
        let definition = self
            .program
            .checked()
            .interner
            .resolve_actor(packet.target.actor);
        let message = definition
            .messages
            .get(packet.target.handler_index)
            .ok_or(ResourceExecutionError::InvalidInvocation)?;
        if packet.kind != kind
            || origin.definition != packet.target.actor_definition
            || handler.name.name != member.name
            || message.name != member.name
            || packet.receiver_span != receiver.span()
            || facts.types.get(&receiver.span()) != Some(&packet.receiver_type)
            || !matches!(self.program.checked().interner.resolve(packet.receiver_type), Type::Actor(actual) if *actual == packet.target.actor)
            || packet.response_type != message.responds
            || packet.arguments.len() != arguments.len()
            || handler.params.len() != arguments.len()
            || message.params.len() != arguments.len()
        {
            return Err(ResourceExecutionError::InvalidInvocation);
        }
        let mut used = vec![false; handler.params.len()];
        for (index, (argument, fact)) in arguments.iter().zip(&packet.arguments).enumerate() {
            let Some(parameter) = handler.params.get(fact.parameter_index) else {
                return Err(ResourceExecutionError::InvalidInvocation);
            };
            if fact.source_index != index
                || fact.source_span != argument.value.span()
                || facts.source_types.get(&fact.source_span) != Some(&fact.actual_type)
                || fact.parameter_type != message.params[fact.parameter_index].1
                || (parameter.view || matches!(parameter.ty, jett_parser::ast::TypeExpr::View(..)))
                    != (fact.callee_access == jett_typecheck::CheckedCalleeAccess::View)
                || std::mem::replace(&mut used[fact.parameter_index], true)
            {
                return Err(ResourceExecutionError::InvalidInvocation);
            }
        }
        Ok(PreparedActorMessage {
            source,
            key: self.field_attempt_key(&self.body)?,
            packet: packet.clone(),
        })
    }

    pub(crate) fn revalidate_actor_message(
        &self,
        prepared: &PreparedActorMessage<'_>,
    ) -> Result<(), ResourceExecutionError> {
        let current = self.prepare_actor_message(prepared.source)?;
        if current.key != prepared.key || current.packet != prepared.packet {
            return Err(ResourceExecutionError::InvalidInvocation);
        }
        Ok(())
    }

    pub(crate) fn prepare_actor_handler(
        &self,
        message: &PreparedActorMessage<'_>,
        registered: &ActorDef,
    ) -> Result<PreparedActorHandler, ResourceExecutionError> {
        self.revalidate_actor_message(message)?;
        let target = message.packet.target;
        let origin =
            CheckedActorInstanceOrigin::from_checked_actor(self.program.clone(), target.actor)?;
        origin.matches_registration(registered)?;
        self.validate_actor_handler_identity(target, origin.item)?;
        let reference = CheckedBodyReference {
            cursor: CheckedBodyCursor {
                root: BodyRoot::ActorHandler(target),
                scopes: Vec::new(),
                executable: Some(ExecutableIdentity {
                    program: self.program.clone(),
                    origin: ExecutableOrigin::ActorHandler {
                        definition: target.actor_definition,
                        item: origin.item,
                        handler: target.handler_index,
                    },
                    path: Vec::new(),
                }),
            },
        };
        self.validate_field_executable_cursor(&reference.cursor)?;
        Ok(PreparedActorHandler { reference, origin })
    }

    pub(super) fn validate_actor_handler_identity(
        &self,
        target: CheckedActorHandlerTarget,
        item: usize,
    ) -> Result<(), ResourceExecutionError> {
        let origin =
            CheckedActorInstanceOrigin::from_checked_actor(self.program.clone(), target.actor)?;
        if origin.definition != target.actor_definition || origin.item != item {
            return Err(ResourceExecutionError::MissingCheckedBody);
        }
        let actor = origin.original()?;
        let handler = actor
            .handlers
            .get(target.handler_index)
            .ok_or(ResourceExecutionError::MissingCheckedBody)?;
        let metadata = self
            .program
            .checked()
            .actor_handler(target)
            .filter(|checked| {
                checked.body_span() == handler.body.span && checked.matches_source_actor(actor)
            })
            .ok_or(ResourceExecutionError::InvalidOwner)?;
        let definition = self.program.checked().interner.resolve_actor(target.actor);
        let message = definition
            .messages
            .get(target.handler_index)
            .ok_or(ResourceExecutionError::InvalidOwner)?;
        if definition.capability_params.len() != actor.capability_params.len()
            || definition.state_fields.len() != actor.state_fields.len()
            || message.params.len() != handler.params.len()
            || metadata.response_type() != message.responds
            || metadata.bindings().len()
                != actor.capability_params.len() + actor.state_fields.len() + handler.params.len()
        {
            return Err(ResourceExecutionError::InvalidOwner);
        }
        let mut earlier = Vec::new();
        for (index, binding) in metadata.bindings().iter().enumerate() {
            let (name, ty, expected) = if index < actor.capability_params.len() {
                (
                    &actor.capability_params[index].name,
                    definition.capability_params[index].1,
                    CheckedActorBindingRole::Capability { parameter: index },
                )
            } else if index < actor.capability_params.len() + actor.state_fields.len() {
                let field = index - actor.capability_params.len();
                (
                    &actor.state_fields[field].name,
                    definition.state_fields[field].1,
                    CheckedActorBindingRole::State { field },
                )
            } else {
                let parameter = index - actor.capability_params.len() - actor.state_fields.len();
                (
                    &handler.params[parameter].name,
                    message.params[parameter].1,
                    CheckedActorBindingRole::Message { parameter },
                )
            };
            let info = self
                .program
                .resolved()
                .scope_table
                .definitions
                .get(binding.fact.definition.index() as usize)
                .ok_or(ResourceExecutionError::InvalidOwner)?;
            if binding.role != expected
                || binding.fact.declaration_span != name.span
                || binding.fact.ty != ty
                || info.id != binding.fact.definition
                || info.span != name.span
                || !matches!(
                    info.kind,
                    jett_resolve::DefKind::Variable | jett_resolve::DefKind::Param
                )
            {
                return Err(ResourceExecutionError::InvalidOwner);
            }
            if matches!(binding.role, CheckedActorBindingRole::State { .. }) {
                if let jett_typecheck::CheckedBindingMode::View { source } = binding.fact.mode {
                    let jett_typecheck::CheckedViewSource::Binding(owner) = source else {
                        return Err(ResourceExecutionError::InvalidOwner);
                    };
                    if !earlier.contains(&owner) {
                        return Err(ResourceExecutionError::InvalidOwner);
                    }
                }
            }
            earlier.push(binding.fact.definition);
        }
        Ok(())
    }

    pub(crate) fn prepare_actor_spawn(
        &self,
        source: &Expr,
        registered: &ActorDef,
    ) -> Result<PreparedActorSpawn, ResourceExecutionError> {
        self.original_actor_occurrence(source)?;
        let Expr::Spawn(inner, _) = source else {
            return Err(ResourceExecutionError::InvalidInvocation);
        };
        let Expr::Call(_, arguments, _) = inner.as_ref() else {
            return Err(ResourceExecutionError::InvalidInvocation);
        };
        let ty = self.expression_type(source.span())?;
        let Type::Actor(actor) = self.program.checked().interner.resolve(ty) else {
            return Err(ResourceExecutionError::InvalidInvocation);
        };
        let origin = CheckedActorInstanceOrigin::from_checked_actor(self.program.clone(), *actor)?;
        origin.matches_registration(registered)?;
        let original = origin.original()?;
        let order = self
            .facts(&self.body)?
            .argument_orders
            .get(&inner.span())
            .ok_or(ResourceExecutionError::MissingCheckedInvocation)?;
        if order.source_indices.len() != arguments.len()
            || arguments.len() != original.capability_params.len()
        {
            return Err(ResourceExecutionError::InvalidInvocation);
        }
        let mut indices = vec![usize::MAX; arguments.len()];
        for (parameter, &index) in order.source_indices.iter().enumerate() {
            let slot = indices
                .get_mut(index)
                .ok_or(ResourceExecutionError::InvalidInvocation)?;
            if *slot != usize::MAX {
                return Err(ResourceExecutionError::InvalidInvocation);
            }
            *slot = parameter;
        }
        let mut initializers = Vec::new();
        for field in 0..original.state_fields.len() {
            initializers.push(CheckedBodyReference {
                cursor: CheckedBodyCursor {
                    root: BodyRoot::Ordinary,
                    scopes: Vec::new(),
                    executable: Some(ExecutableIdentity {
                        program: self.program.clone(),
                        origin: ExecutableOrigin::ActorInitializer {
                            item: origin.item,
                            field,
                        },
                        path: Vec::new(),
                    }),
                },
            });
        }
        Ok(PreparedActorSpawn {
            origin,
            parameter_indices: indices,
            initializers,
        })
    }
}

#[cfg(test)]
#[path = "actor_handlers/tests.rs"]
mod tests;
