//! Owned lazy callback conversion. No public constructor can relabel a callable.
use std::sync::Arc;

use jett_common::Span;
use jett_parser::ast::{Block, TypeExpr};
use jett_resolve::{DefId, DefKind};
use jett_typecheck::{CheckedReflectedCallablePlan, CheckedResourceProgram};
use jett_types::TypeId;

use super::Value;

#[path = "reflected_callable_source.rs"]
pub(crate) mod original_source;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SourceIdentity {
    Named(DefId),
    Closure(Span),
}

#[derive(Clone)]
pub struct ReflectedCallableValue {
    source: Box<Value>,
    checked: Arc<CheckedResourceProgram>,
    plan: CheckedReflectedCallablePlan,
    identity: SourceIdentity,
    source_return: TypeExpr,
    requested_return: TypeExpr,
}

impl std::fmt::Debug for ReflectedCallableValue {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ReflectedCallableValue")
            .field("source_label", &self.source.to_string())
            .field("source_type", &self.source_type())
            .field("requested_type", &self.requested_type())
            .finish_non_exhaustive()
    }
}

impl ReflectedCallableValue {
    pub(crate) fn new(
        source: Value,
        checked: Arc<CheckedResourceProgram>,
        actual: TypeId,
        requested: TypeId,
        source_return: TypeExpr,
        requested_return: TypeExpr,
    ) -> Result<Self, String> {
        let plan = checked
            .checked()
            .reflected_callable_plan(actual, requested)
            .ok_or_else(|| {
                "reflected callback has no checked return conversion plan".to_string()
            })?;
        let identity = Self::source_identity(&source, &checked, actual)?;
        Ok(Self {
            source: Box::new(source),
            checked,
            plan,
            identity,
            source_return,
            requested_return,
        })
    }

    pub(crate) fn authenticate_source(
        source: &Value,
        checked: &Arc<CheckedResourceProgram>,
        actual: TypeId,
    ) -> Result<(), String> {
        Self::source_identity(source, checked, actual).map(|_| ())
    }

    fn source_identity(
        source: &Value,
        checked: &Arc<CheckedResourceProgram>,
        actual: TypeId,
    ) -> Result<SourceIdentity, String> {
        if source.contains_live_resource_or_grant() {
            return Err("reflected callback cannot copy live Resource custody".into());
        }
        if !matches!(
            source.payload(),
            Value::NamedFunction(_)
                | Value::Function { .. }
                | Value::ReflectedCallable(_)
                | Value::Pending(_)
        ) {
            return Err("reflected callback source is not a callable descriptor".into());
        }
        fn descriptor(value: &Value) -> &Value {
            // Retain descriptor identity without joining or invoking it. This
            // never observes a pending callback result or establishes a proof.
            match value.payload() {
                Value::Pending(inner) => descriptor(inner),
                value => value,
            }
        }
        let identity = match descriptor(source) {
            Value::NamedFunction(name) => {
                let mut definitions =
                    checked
                        .resolved()
                        .scope_table
                        .definitions
                        .iter()
                        .filter(|info| {
                            info.kind == DefKind::Function
                                && info.name == *name
                                && checked.checked().definition_types.get(&info.id) == Some(&actual)
                                && original_source::named_body(&checked, info.id).is_some()
                        });
                let selected = definitions
                    .next()
                    .ok_or("reflected callback has no exact original source declaration")?;
                if definitions.next().is_some() {
                    return Err(
                        "reflected callback original source declaration is ambiguous".into(),
                    );
                }
                SourceIdentity::Named(selected.id)
            }
            Value::Function {
                type_context, body, ..
            } => {
                if !type_context
                    .reflected_capture
                    .as_ref()
                    .is_some_and(|proof| proof.validate_source(checked, actual, descriptor(source)))
                {
                    return Err(
                        "reflected callback has no unchanged original closure capture".into(),
                    );
                }
                SourceIdentity::Closure(body.span)
            }
            Value::ReflectedCallable(admitted)
                if Arc::ptr_eq(&checked, admitted.checked_program())
                    && admitted.requested_type() == actual =>
            {
                admitted.identity
            }
            _ => {
                return Err(
                    "reflected callback source belongs to another checked callable context".into(),
                );
            }
        };
        Ok(identity)
    }

    pub fn source(&self) -> &Value {
        &self.source
    }
    pub fn source_type(&self) -> TypeId {
        self.plan.source_type()
    }
    pub fn requested_type(&self) -> TypeId {
        self.plan.requested_type()
    }
    pub fn checked_program(&self) -> &Arc<CheckedResourceProgram> {
        &self.checked
    }
    pub fn checked_plan(&self) -> &CheckedReflectedCallablePlan {
        &self.plan
    }
    pub fn source_definition(&self) -> Option<DefId> {
        match self.identity {
            SourceIdentity::Named(definition) => Some(definition),
            _ => None,
        }
    }
    pub fn source_body(&self) -> Option<&Block> {
        match self.identity {
            SourceIdentity::Named(definition) => {
                original_source::named_body(&self.checked, definition)
            }
            SourceIdentity::Closure(span) => original_source::closure_body(&self.checked, span),
        }
    }
    pub(crate) fn source_return(&self) -> &TypeExpr {
        &self.source_return
    }
    pub(crate) fn requested_return(&self) -> &TypeExpr {
        &self.requested_return
    }

    pub(crate) fn same_snapshot(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.checked, &other.checked)
            && self.plan == other.plan
            && self.identity == other.identity
            && super::closure_capture::same_type_expr(&self.source_return, &other.source_return)
            && super::closure_capture::same_type_expr(
                &self.requested_return,
                &other.requested_return,
            )
            && super::closure_capture::same_snapshot(&self.source, &other.source)
    }
}
