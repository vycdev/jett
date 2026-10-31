//! Capture headers derived from a private graph seed and original actor Source.
use super::*;
use crate::{DeclarationKind, Param};
use jett_parser::ast::Item;
use jett_resolve::DefKind;

#[derive(Debug, Clone, PartialEq)]
pub struct ActorGraphCaptureBinding {
    handler: FunctionId,
    node: usize,
    field: usize,
    parameter: Param,
    actor_type: TypeId,
}
impl ActorGraphCaptureBinding {
    pub fn handler(&self) -> FunctionId {
        self.handler
    }
    pub fn node(&self) -> usize {
        self.node
    }
    pub fn field(&self) -> usize {
        self.field
    }
    pub fn parameter(&self) -> &Param {
        &self.parameter
    }
    pub fn actor_type(&self) -> TypeId {
        self.actor_type
    }
    pub fn field_type(&self) -> TypeId {
        self.parameter.ty
    }
    pub fn mutable(&self) -> bool {
        self.parameter.mutable
    }
}
impl ActorGraphPlan {
    /// No caller-supplied actor name, TypeId or formal can mint this relation.
    pub fn actor_capture_bindings(&self) -> Result<Vec<ActorGraphCaptureBinding>, String> {
        let origin = &self.0.origin;
        let graph = origin
            .proof
            .actor_graph()
            .ok_or("capture graph has no retained template")?;
        if !origin.proof.belongs_to(&origin.checked) || !graph.belongs_to(&origin.checked) {
            return Err("capture graph belongs to a foreign checked program".into());
        }
        let types = &origin.checked.checked().interner;
        origin.archive.validate_types(types)?;
        let functions = origin
            .functions
            .get()
            .ok_or("graph capture declarations were not sealed")?;
        let mut rows = Vec::new();
        for (node_index, node) in graph.nodes().iter().enumerate() {
            let Type::Actor(actor_id) = types.resolve(node.ty()) else {
                return Err("graph seed lost nominal Actor type".into());
            };
            let shape = types.resolve_actor(*actor_id);
            let definitions = origin
                .checked
                .resolved()
                .scope_table
                .definitions
                .iter()
                .filter(|definition| {
                    definition.kind == DefKind::Actor
                        && origin
                            .checked
                            .checked()
                            .definition_types
                            .get(&definition.id)
                            == Some(&node.ty())
                })
                .collect::<Vec<_>>();
            let [definition] = definitions.as_slice() else {
                return Err("graph seed has no unique original actor DefId/type".into());
            };
            let actors = origin
                .checked
                .module()
                .items
                .iter()
                .filter_map(|item| match item {
                    Item::Actor(actor) if actor.name.span == definition.span => Some(actor),
                    _ => None,
                })
                .collect::<Vec<_>>();
            let [actor] = actors.as_slice() else {
                return Err("graph actor has no unique original AST declaration".into());
            };
            if shape.name != definition.name
                || actor.capability_params.len() != shape.capability_params.len()
                || actor.state_fields.len() != shape.state_fields.len()
                || actor.handlers.len() != shape.messages.len()
                || node.captures().len() != shape.capability_params.len()
                || node.state().len() != shape.state_fields.len()
            {
                return Err("graph actor capture/state cardinality changed".into());
            }
            for (message_index, handler_source) in actor.handlers.iter().enumerate() {
                let handlers = functions
                    .iter()
                    .filter(|function| {
                        function.identity.declaration.kind == DeclarationKind::ActorHandler
                            && function.span == handler_source.span
                    })
                    .collect::<Vec<_>>();
                let [handler] = handlers.as_slice() else {
                    return Err("graph field has no unique original handler header".into());
                };
                let original = origin
                    .archive
                    .execution_functions()
                    .get(handler.id.index() as usize)
                    .filter(|function| {
                        function.id == handler.id
                            && function.identity == handler.identity
                            && function.span == handler.span
                            && function.capture_count == handler.capture_count
                            && function.params == handler.params
                            && function.locals == handler.locals
                            && function.return_type == handler.return_type
                            && function.debug_kind == handler.debug_kind
                    })
                    .ok_or("graph handler differs from whole original Source archive")?;
                let message = &shape.messages[message_index];
                let captured = shape.capability_params.len() + shape.state_fields.len();
                if original.capture_count != captured
                    || original.params.len() != captured + message.params.len()
                    || original.return_type != message.responds
                    || handler_source.params.len() != message.params.len()
                {
                    return Err("graph handler formal/return layout changed".into());
                }
                for (field_index, ((field_name, field_ty), frozen)) in shape
                    .capability_params
                    .iter()
                    .chain(&shape.state_fields)
                    .zip(node.captures().iter().chain(node.state()))
                    .enumerate()
                {
                    let parameter = &original.params[field_index];
                    let (name, span, mutable) = if field_index < actor.capability_params.len() {
                        let source = &actor.capability_params[field_index];
                        (&source.name.name, source.span, source.mutable)
                    } else {
                        let source =
                            &actor.state_fields[field_index - actor.capability_params.len()];
                        (&source.name.name, source.span, source.mutable)
                    };
                    if frozen.name() != field_name
                        || frozen.ty() != *field_ty
                        || name != field_name
                        || parameter.name != *field_name
                        || parameter.ty != *field_ty
                        || parameter.span != span
                        || parameter.mutable != mutable
                        || original
                            .locals
                            .get(parameter.local.index() as usize)
                            .is_none_or(|local| {
                                local.id != parameter.local
                                    || local.name != parameter.name
                                    || local.ty != parameter.ty
                                    || local.span != parameter.span
                                    || local.mutable != parameter.mutable
                            })
                    {
                        return Err("graph field changed exact typed Source capture header".into());
                    }
                    rows.push(ActorGraphCaptureBinding {
                        handler: original.id,
                        node: node_index,
                        field: field_index,
                        parameter: parameter.clone(),
                        actor_type: node.ty(),
                    });
                }
            }
        }
        Ok(rows)
    }
}
