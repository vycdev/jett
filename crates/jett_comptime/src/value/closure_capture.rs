//! Private capture-time proof. Admission cannot mint or reseal this witness.
use std::sync::Arc;

use jett_common::Span;
use jett_parser::ast::{Block, Expr, FunctionDef, Item};
use jett_resolve::{DefId, DefKind};
use jett_typecheck::CheckedResourceProgram;
use jett_types::{Type, TypeId};

use super::{Value, reflected_callable::original_source};

#[path = "closure_capture/ast_equal.rs"]
mod ast_equal;
#[path = "closure_capture/checked_context.rs"]
mod checked_context;
#[path = "closure_capture/value_equal.rs"]
mod value_equal;

#[cfg(test)]
#[path = "closure_capture/test_owner_tests.rs"]
mod test_owner_tests;

#[derive(Debug, Clone, Copy)]
enum Owner {
    Named(DefId),
    Closure(Span),
    Verify(usize),
    Property(usize),
    ActorHandler(jett_typecheck::CheckedActorHandlerTarget),
    ActorClosure(jett_typecheck::CheckedActorHandlerTarget, Span),
}

#[derive(Debug, Clone)]
pub(crate) struct OriginalClosureOwner {
    checked: Arc<CheckedResourceProgram>,
    owner: Owner,
    namespace: Option<String>,
    definition: Option<DefId>,
}

impl OriginalClosureOwner {
    pub(crate) fn registered(
        checked: &Arc<CheckedResourceProgram>,
        name: &str,
        current: &FunctionDef,
        namespace: &Option<String>,
    ) -> Option<Self> {
        let mut candidates = checked
            .resolved()
            .scope_table
            .definitions
            .iter()
            .filter(|info| {
                info.kind == DefKind::Function
                    && info.name == name
                    && info.span == current.name.span
                    && &info.namespace == namespace
                    && original_source::named_function(checked, info.id)
                        .is_some_and(|original| ast_equal::function(original, current))
            });
        let selected = candidates.next()?;
        if candidates.next().is_some() {
            return None;
        }
        Some(Self {
            checked: checked.clone(),
            owner: Owner::Named(selected.id),
            namespace: namespace.clone(),
            definition: Some(selected.id),
        })
    }

    pub(crate) fn test_body(
        checked: &Arc<CheckedResourceProgram>,
        original: &Block,
        purpose: crate::ExecutionPurpose,
        namespace: Option<&str>,
    ) -> Option<Self> {
        let mut lexical_namespace = None;
        let mut namespace_file = None;
        for (index, item) in checked.module().items.iter().enumerate() {
            if let Item::Namespace(declaration) = item {
                lexical_namespace = Some(declaration.name.name.clone());
                namespace_file = Some(declaration.name.span.file);
            }
            let (body, owner) = match (item, purpose) {
                (Item::Verify(block), crate::ExecutionPurpose::Verify) => {
                    (&block.body, Owner::Verify(index))
                }
                (Item::Property(block), crate::ExecutionPurpose::Property) => {
                    (&block.body, Owner::Property(index))
                }
                _ => continue,
            };
            if !std::ptr::eq(body, original) {
                continue;
            }
            let selected_namespace = if namespace_file == Some(body.span.file) {
                lexical_namespace
            } else {
                None
            };
            if selected_namespace.as_deref() != namespace {
                return None;
            }
            return Some(Self {
                checked: checked.clone(),
                owner,
                namespace: selected_namespace,
                definition: None,
            });
        }
        None
    }

    pub(crate) fn actor_handler(
        checked: &Arc<CheckedResourceProgram>,
        target: jett_typecheck::CheckedActorHandlerTarget,
        namespace: Option<&str>,
    ) -> Option<Self> {
        let info = checked
            .resolved()
            .scope_table
            .definitions
            .get(target.actor_definition.index() as usize)?;
        if info.id != target.actor_definition
            || info.kind != DefKind::Actor
            || info.namespace.as_deref() != namespace
        {
            return None;
        }
        let metadata = checked.checked().actor_handler(target)?;
        let mut matching = checked.module().items.iter().filter_map(|item| {
            let Item::Actor(actor) = item else {
                return None;
            };
            let handler = actor.handlers.get(target.handler_index)?;
            (actor.name.span == info.span
                && handler.body.span == metadata.body_span()
                && metadata.matches_source_actor(actor))
            .then_some(handler)
        });
        matching.next()?;
        if matching.next().is_some() {
            return None;
        }
        Some(Self {
            checked: checked.clone(),
            owner: Owner::ActorHandler(target),
            namespace: namespace.map(str::to_owned),
            definition: None,
        })
    }

    fn actor_target(&self) -> Option<jett_typecheck::CheckedActorHandlerTarget> {
        match self.owner {
            Owner::ActorHandler(target) | Owner::ActorClosure(target, _) => Some(target),
            _ => None,
        }
    }

    fn body(&self) -> Option<&Block> {
        match self.owner {
            Owner::Named(definition) => original_source::named_function(&self.checked, definition)
                .map(|function| &function.body),
            Owner::Closure(span) => match original_source::closure_node(&self.checked, span)? {
                Expr::InlineFn(_, _, body, _) => Some(body),
                _ => None,
            },
            Owner::Verify(index) => match self.checked.module().items.get(index)? {
                Item::Verify(block) => Some(&block.body),
                _ => None,
            },
            Owner::ActorHandler(target) => {
                let info = self
                    .checked
                    .resolved()
                    .scope_table
                    .definitions
                    .get(target.actor_definition.index() as usize)?;
                let metadata = self.checked.checked().actor_handler(target)?;
                self.checked.module().items.iter().find_map(|item| {
                    let Item::Actor(actor) = item else {
                        return None;
                    };
                    let handler = actor.handlers.get(target.handler_index)?;
                    (actor.name.span == info.span
                        && handler.body.span == metadata.body_span()
                        && metadata.matches_source_actor(actor))
                    .then_some(&handler.body)
                })
            }
            Owner::ActorClosure(_, span) => {
                match original_source::closure_node(&self.checked, span)? {
                    Expr::InlineFn(_, _, body, _) => Some(body),
                    _ => None,
                }
            }
            Owner::Property(index) => match self.checked.module().items.get(index)? {
                Item::Property(block) => Some(&block.body),
                _ => None,
            },
        }
    }
}

#[derive(Debug)]
pub(crate) struct CheckedClosureCapture {
    origin: OriginalClosureOwner,
    node_span: Span,
    body_span: Span,
    source_type: TypeId,
    snapshot: Box<Value>,
}

impl CheckedClosureCapture {
    pub(crate) fn field_original_node(&self) -> Option<&Expr> {
        let node = original_source::closure_node_in_body(self.origin.body()?, self.body_span)?;
        let global = original_source::closure_node(&self.origin.checked, self.body_span)?;
        std::ptr::eq(node, global).then_some(node)
    }
    pub(crate) fn seal(origin: &OriginalClosureOwner, value: &Value) -> Option<Arc<Self>> {
        let Value::Function {
            type_context,
            params,
            body,
            namespace,
            ..
        } = value
        else {
            return None;
        };
        if namespace != &origin.namespace || type_context.reflected_capture.is_some() {
            return None;
        }
        let original = original_source::closure_node_in_body(origin.body()?, body.span)?;
        let global = original_source::closure_node(&origin.checked, body.span)?;
        if !std::ptr::eq(original, global) {
            return None;
        }
        let Expr::InlineFn(original_params, original_return, original_body, node_span) = original
        else {
            return None;
        };
        if !ast_equal::params(params, original_params)
            || !ast_equal::optional(
                &type_context.return_type,
                original_return,
                ast_equal::type_expr,
            )
            || !ast_equal::block(body, original_body)
        {
            return None;
        }
        let source_type = checked_context::callable_type(origin, type_context, *node_span)?;
        if !matches!(
            origin.checked.checked().interner.resolve(source_type),
            Type::Function { .. }
        ) {
            return None;
        }
        if value.contains_live_resource_or_grant() {
            return None;
        }
        Some(Arc::new(Self {
            origin: origin.clone(),
            node_span: *node_span,
            body_span: body.span,
            source_type,
            snapshot: Box::new(value.clone()),
        }))
    }

    pub(crate) fn validate_source(
        &self,
        checked: &Arc<CheckedResourceProgram>,
        actual: TypeId,
        value: &Value,
    ) -> bool {
        Arc::ptr_eq(&self.origin.checked, checked)
            && actual == self.source_type
            && matches!(value, Value::Function { type_context, .. } if checked_context::callable_type(&self.origin, type_context, self.node_span) == Some(actual))
            && original_source::closure_node(checked, self.body_span).is_some_and(|node| {
                let Expr::InlineFn(params, result, body, span) = node else {
                    return false;
                };
                let Value::Function {
                    type_context,
                    params: current,
                    body: current_body,
                    ..
                } = value
                else {
                    return false;
                };
                *span == self.node_span
                    && ast_equal::params(params, current)
                    && ast_equal::optional(result, &type_context.return_type, ast_equal::type_expr)
                    && ast_equal::block(body, current_body)
            })
            && value_equal::closure(&self.snapshot, value, false)
    }
}

pub(crate) fn invocation_origin(
    value: &Value,
    checked: &Arc<CheckedResourceProgram>,
) -> Option<OriginalClosureOwner> {
    let Value::Function { type_context, .. } = value.payload() else {
        return None;
    };
    let proof = type_context.reflected_capture.as_ref()?;
    proof
        .validate_source(checked, proof.source_type, value.payload())
        .then(|| OriginalClosureOwner {
            checked: checked.clone(),
            owner: proof
                .origin
                .actor_target()
                .map_or(Owner::Closure(proof.body_span), |target| {
                    Owner::ActorClosure(target, proof.body_span)
                }),
            namespace: proof.origin.namespace.clone(),
            definition: proof.origin.definition,
        })
}

/// Reuse the existing capture snapshot; no span/name-only closure authority.
pub(crate) fn field_invocation_proof(
    value: &Value,
    checked: &Arc<CheckedResourceProgram>,
) -> Option<Arc<CheckedClosureCapture>> {
    let Value::Function { type_context, .. } = value else {
        return None;
    };
    let proof = type_context.reflected_capture.as_ref()?;
    proof
        .validate_source(checked, proof.source_type, value)
        .then(|| proof.clone())
}

pub(crate) fn same_snapshot(a: &Value, b: &Value) -> bool {
    value_equal::value(a, b)
}
pub(crate) fn same_type_expr(
    a: &jett_parser::ast::TypeExpr,
    b: &jett_parser::ast::TypeExpr,
) -> bool {
    ast_equal::type_expr(a, b)
}
pub(crate) fn original_root_types(
    checked: &CheckedResourceProgram,
    installed: &crate::checked_types::CheckedExpressionTypes,
) -> bool {
    checked_context::root_matches(checked, installed)
}

/// Compare data projections against one retained concrete body. This does not
/// grant executable identity or field-write authority by itself.
pub(crate) fn matches_field_function_projection(
    current: &crate::checked_types::CheckedFunctionTypes,
    arguments: &[super::ClosureTypeArgument],
    original: &jett_typecheck::CheckedGenericFunctionInstantiation,
    types: &jett_types::TypeInterner,
) -> bool {
    let context = super::ClosureTypeContext {
        arguments: arguments.to_vec(),
        ..Default::default()
    };
    checked_context::function_matches(current, &context, original, types)
}
