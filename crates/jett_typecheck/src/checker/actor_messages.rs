use super::*;
use crate::actor_messages::{
    CheckedActorHandlerTarget, CheckedActorMessageCall, CheckedActorMessageKind,
};
use jett_types::ActorId;

#[cfg(test)]
#[path = "actor_messages/tests.rs"]
mod tests;

impl TypeChecker<'_> {
    pub(super) fn record_actor_message_call(
        &mut self,
        outer_span: Span,
        kind: CheckedActorMessageKind,
        inner: &Expr,
        receiver: &Expr,
        receiver_type: TypeId,
        actor: ActorId,
        handler_index: usize,
        response_type: TypeId,
        args: &[ast::CallArg],
    ) {
        let Some(&actor_definition) = self.actor_definitions.get(&actor) else {
            return;
        };
        if self.resolve.scope_table.def(actor_definition).kind != DefKind::Actor
            || self.type_env.get(&actor_definition) != Some(&receiver_type)
            || self.interner.resolve(receiver_type) != &Type::Actor(actor)
        {
            return;
        }
        let target = CheckedActorHandlerTarget {
            actor_definition,
            actor,
            handler_index,
        };
        let Some(message) = self
            .interner
            .resolve_actor(actor)
            .messages
            .get(handler_index)
            .cloned()
        else {
            return;
        };
        let Some(modes) = self.actor_message_modes.get(&target).cloned() else {
            return;
        };
        let (original_receiver, original_handler, original_args) = match inner {
            Expr::Call(callee, arguments, _) => match callee.as_ref() {
                Expr::FieldAccess(base, handler, _) => {
                    (base.as_ref(), handler, arguments.as_slice())
                }
                _ => return,
            },
            Expr::FieldAccess(base, handler, _) => (base.as_ref(), handler, &[][..]),
            _ => return,
        };
        if !std::ptr::eq(original_receiver, receiver)
            || original_handler.name != message.name
            || response_type != message.responds
            || args.len() != original_args.len()
            || args
                .iter()
                .zip(original_args)
                .any(|(argument, original)| !std::ptr::eq(argument, original))
            || message.params.len() != args.len()
            || modes.len() != message.params.len()
        {
            return;
        }
        let order = match self.active_generic_instantiations.last() {
            Some(active) => active.call_argument_orders.get(&inner.span()),
            None => self.call_argument_orders.get(&inner.span()),
        };
        let Some(order) = order.cloned() else {
            return;
        };
        if order.source_indices.len() != args.len() {
            return;
        }
        let mut inverse = vec![None; args.len()];
        for (parameter_index, &source_index) in order.source_indices.iter().enumerate() {
            let Some(slot) = inverse.get_mut(source_index) else {
                return;
            };
            if slot.replace(parameter_index).is_some() {
                return;
            }
        }
        let mut arguments = Vec::with_capacity(args.len());
        for (source_index, argument) in args.iter().enumerate() {
            let (Some(parameter_index), Some(actual_type)) = (
                inverse[source_index],
                self.checked_expression_type(argument.value.span()),
            ) else {
                return;
            };
            let parameter_type = message.params[parameter_index].1;
            if actual_type == TypeInterner::ERROR || parameter_type == TypeInterner::ERROR {
                return;
            }
            let written_view = Self::is_explicit_view(&argument.value);
            let copyable = crate::ownership::is_implicitly_copyable(&self.interner, actual_type);
            // A message crosses an actor boundary even when the receiver gives
            // its private incoming owner read-only access inside the handler.
            // Test observation does not turn a borrowed caller into that owner.
            if written_view || (!copyable && self.initializer_is_view(&argument.value)) {
                self.sink.emit(Diagnostic::error(
                    401,
                    "cannot send a view to an actor; transfer ownership or clone the value",
                    argument.value.span(),
                ));
            }
            let callee_access = if modes[parameter_index] {
                CheckedCalleeAccess::View
            } else {
                CheckedCalleeAccess::Owned
            };
            arguments.push(CheckedArgumentOwnership {
                source_span: argument.value.span(),
                source_index,
                parameter_index,
                actual_type,
                parameter_type,
                syntax: if written_view {
                    CheckedCallerSyntax::WrittenView
                } else {
                    CheckedCallerSyntax::Bare
                },
                origin: self.caller_origin(Some(&argument.value), written_view),
                callee_access,
                effect: if copyable {
                    CheckedCallerEffect::Copy
                } else if callee_access == CheckedCalleeAccess::View {
                    CheckedCallerEffect::RelinquishOwned
                } else {
                    CheckedCallerEffect::TransferOwned
                },
            });
        }
        let packet = CheckedActorMessageCall::new(
            self.actor_check_session.clone(),
            target,
            kind,
            self.ownership_context,
            outer_span,
            receiver.span(),
            receiver_type,
            response_type,
            arguments,
        );
        if let Some(active) = self.active_generic_instantiations.last_mut() {
            active.actor_message_calls.insert(outer_span, packet);
        } else {
            self.actor_message_calls.insert(outer_span, packet);
        }
    }
}
