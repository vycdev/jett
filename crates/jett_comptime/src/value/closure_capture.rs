//! Private capture-time proof. Admission cannot mint or reseal this witness.
use std::sync::Arc;

use jett_common::Span;
use jett_parser::ast::{Block, Expr, FunctionDef};
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

#[derive(Debug, Clone, Copy)]
enum Owner {
    Named(DefId),
    Closure(Span),
}

#[derive(Debug, Clone)]
pub(crate) struct OriginalClosureOwner {
    checked: Arc<CheckedResourceProgram>,
    owner: Owner,
    namespace: Option<String>,
    definition: DefId,
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
            definition: selected.id,
        })
    }

    fn body(&self) -> Option<&Block> {
        match self.owner {
            Owner::Named(definition) => original_source::named_function(&self.checked, definition)
                .map(|function| &function.body),
            Owner::Closure(span) => match original_source::closure_node(&self.checked, span)? {
                Expr::InlineFn(_, _, body, _) => Some(body),
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
            owner: Owner::Closure(proof.body_span),
            namespace: proof.origin.namespace.clone(),
            definition: proof.origin.definition,
        })
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
