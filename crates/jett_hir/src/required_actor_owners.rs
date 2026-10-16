//! Exact original ActorDef/ordinal joins for checked required-value metadata.
use super::*;
use jett_parser::ast::ActorDef;
use jett_resolve::{DefId, DefInfo, DefKind};
use jett_types::ActorId;

fn original_actor<'a>(
    checked: &'a CheckedResourceProgram,
    definition: DefId,
) -> Option<(&'a ActorDef, &'a DefInfo, TypeId, ActorId)> {
    let declared = checked
        .resolved()
        .scope_table
        .definitions
        .get(definition.index() as usize)?;
    if declared.id != definition || declared.kind != DefKind::Actor {
        return None;
    }
    let ty = *checked.checked().definition_types.get(&definition)?;
    let Type::Actor(actor_id) = checked.checked().interner.resolve(ty) else {
        return None;
    };
    let originals = checked
        .module()
        .items
        .iter()
        .filter_map(|item| match item {
            Item::Actor(actor) if actor.name.span == declared.span => Some(actor),
            _ => None,
        })
        .collect::<Vec<_>>();
    let [actor] = originals.as_slice() else {
        return None;
    };
    if checked.checked().interner.resolve_actor(*actor_id).name != declared.name {
        return None;
    }
    Some((actor, declared, ty, *actor_id))
}

fn common(
    function: &Function,
    actor: &ActorDef,
    declared: &DefInfo,
    checked: &CheckedResourceProgram,
) -> bool {
    function.source_definition.is_none()
        && function.identity.declaration.namespace
            == declared.namespace.as_deref().unwrap_or_default()
        && checked.source_origins().get(&actor.span.file)
            == Some(&function.identity.declaration.origin)
}

pub(super) fn matches(
    owner: CheckedRequiredOwner,
    function: &Function,
    checked: &Arc<CheckedResourceProgram>,
) -> bool {
    match owner {
        CheckedRequiredOwner::ActorInitializer {
            definition,
            declaration,
            field,
        } => {
            let Some((actor, declared, ty, actor_id)) = original_actor(checked, definition) else {
                return false;
            };
            let Some(initializer) = actor.state_fields.get(field) else {
                return false;
            };
            let Some((name, _)) = checked
                .checked()
                .interner
                .resolve_actor(actor_id)
                .state_fields
                .get(field)
            else {
                return false;
            };
            common(function, actor, declared, checked)
                && function.identity.declaration.kind == DeclarationKind::ActorConstructor
                && function.identity.declaration.name == actor.name.name
                && function.actor_handler.is_none()
                && function.span == actor.span
                && function.body.span == actor.span
                && function.return_type == ty
                && initializer.name.span == declaration
                && initializer.name.name == *name
        }
        CheckedRequiredOwner::ActorHandler {
            target,
            declaration,
        } => {
            let Some((actor, declared, _, actor_id)) =
                original_actor(checked, target.actor_definition)
            else {
                return false;
            };
            let Some(handler) = actor.handlers.get(target.handler_index) else {
                return false;
            };
            let Some(receipt) = checked.checked().actor_handler(target) else {
                return false;
            };
            common(function, actor, declared, checked)
                && target.actor == actor_id
                && function.identity.declaration.kind == DeclarationKind::ActorHandler
                && function.identity.declaration.name
                    == format!("{}.{}", actor.name.name, handler.name.name)
                && function.span == handler.span
                && function.body.span == handler.body.span
                && handler.name.span == declaration
                && receipt.matches_source_actor(actor)
                && receipt.body_span() == handler.body.span
                && receipt.response_type() == function.return_type
                && function.actor_handler.as_ref().is_some_and(|entry| {
                    entry.target() == target && entry.validate(function).is_ok()
                })
        }
        _ => false,
    }
}
