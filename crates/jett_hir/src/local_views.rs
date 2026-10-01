//! Checked local borrow origins. Native admission is limited to immutable
//! local chains whose initializers preserve the existing backing value.

use jett_types::{Type, TypeId, TypeInterner};

use crate::{
    Expression, ExpressionKind, Function, Local, LocalId, Param, ParamMode, Program,
    ValidationError,
};

/// Resolve an alias to its backing local. Invalid IDs and cycles have no root.
pub fn local_view_root(locals: &[Local], mut id: LocalId) -> Option<LocalId> {
    for _ in 0..locals.len() {
        let local = locals
            .get(id.index() as usize)
            .filter(|local| local.id == id)?;
        match local.view_source {
            Some(source) => id = source,
            None => return Some(id),
        }
    }
    None
}

pub fn is_borrowed_local(locals: &[Local], params: &[Param], id: LocalId) -> bool {
    locals
        .get(id.index() as usize)
        .is_some_and(|local| local.id == id && local.view_source.is_some())
        || params
            .iter()
            .any(|param| param.local == id && param.mode == ParamMode::View)
}

pub(super) fn immutable_view_chain(locals: &[Local], mut id: LocalId) -> bool {
    for _ in 0..locals.len() {
        let Some(local) = locals
            .get(id.index() as usize)
            .filter(|local| local.id == id)
        else {
            return false;
        };
        if local.mutable {
            return false;
        }
        match local.view_source {
            Some(source) => id = source,
            None => return true,
        }
    }
    false
}

pub(super) fn validate_structure(function: &Function) -> Vec<ValidationError> {
    let mut errors = Vec::new();
    for local in &function.locals {
        if local.view_source.is_none() {
            continue;
        }
        let message = if function.params.iter().any(|param| param.local == local.id) {
            Some("parameter ownership must use ParamMode, not a local alias origin")
        } else if local_view_root(&function.locals, local.id).is_none() {
            Some("borrowed local origin is outside its function or contains a cycle")
        } else if !immutable_view_chain(&function.locals, local.id) {
            Some("native borrowed alias requires immutable bindings along its stable origin")
        } else {
            None
        };
        if let Some(message) = message {
            errors.push(ValidationError {
                span: local.span,
                message: message.into(),
            });
        }
    }
    errors
}

/// Validate alias metadata after transformations or LocalId remapping. Full
/// backend type validation additionally checks each alias initializer.
pub fn validate_local_views(
    program: &Program,
    types: &TypeInterner,
) -> Result<(), Vec<ValidationError>> {
    let mut errors = Vec::new();
    for function in &program.functions {
        errors.extend(validate_structure(function));
        for local in &function.locals {
            if local.view_source.is_some()
                && (local.ty.index() as usize) < types.len()
                && jett_typecheck::ownership::is_implicitly_copyable(types, local.ty)
            {
                errors.push(ValidationError {
                    span: local.span,
                    message: "implicitly copyable locals cannot carry a borrowed alias origin"
                        .into(),
                });
            }
            let Some(source) = local
                .view_source
                .and_then(|source| function.locals.get(source.index() as usize))
            else {
                continue;
            };
            if !same_backing_type(types, local.ty, source.ty) {
                errors.push(ValidationError {
                    span: local.span,
                    message: "native borrowed alias cannot change its backing representation"
                        .into(),
                });
            }
        }
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

fn backing_type(types: &TypeInterner, mut ty: TypeId) -> Option<TypeId> {
    for _ in 0..types.len() {
        if ty.index() as usize >= types.len() {
            return None;
        }
        match types.resolve(ty) {
            Type::Secret(inner) | Type::Refinement { base: inner, .. } => ty = *inner,
            _ => return Some(ty),
        }
    }
    None
}

fn erased_interface(types: &TypeInterner, mut ty: TypeId) -> Option<bool> {
    for _ in 0..types.len() {
        if ty.index() as usize >= types.len() {
            return None;
        }
        match types.resolve(ty) {
            Type::Secret(inner) => ty = *inner,
            Type::Interface(_) => return Some(true),
            _ => return Some(false),
        }
    }
    None
}

fn same_backing_type(types: &TypeInterner, left: TypeId, right: TypeId) -> bool {
    let Some(left_backing) = backing_type(types, left) else {
        return false;
    };
    backing_type(types, right) == Some(left_backing)
        && erased_interface(types, left) == erased_interface(types, right)
}

pub fn validate_local_view_initializer(
    value: &Expression,
    source: LocalId,
    target: TypeId,
    types: &TypeInterner,
) -> Result<(), &'static str> {
    if !same_backing_type(types, value.ty, target) {
        return Err("native borrowed alias cannot change its backing representation");
    }
    let mut value = value;
    loop {
        match &value.kind {
            ExpressionKind::Local(id) if *id == source => return Ok(()),
            ExpressionKind::View(inner)
            | ExpressionKind::Coarsen(inner)
            | ExpressionKind::Declassify(inner) => {
                if !same_backing_type(types, value.ty, inner.ty) {
                    return Err("native borrowed alias cannot change its backing representation");
                }
                value = inner;
            }
            ExpressionKind::InterfaceCoerce {
                value: inner,
                adapters,
            } if adapters.is_empty() => {
                if !same_backing_type(types, value.ty, inner.ty) {
                    return Err(
                        "native borrowed alias cannot allocate an interface or container conversion",
                    );
                }
                value = inner;
            }
            _ => {
                return Err(
                    "native borrowed alias initializer must preserve its stable backing local",
                );
            }
        }
    }
}
