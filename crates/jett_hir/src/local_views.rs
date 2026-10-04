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

/// Validate dependency metadata after transformations or LocalId remapping.
/// Full backend validation proves each alias's typed initializer and rejects
/// borrowed metadata without an initializer. A projected endpoint can differ
/// from the type of its whole backing local.
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

/// Secrecy can be added without acquiring a new nominal predicate proof.
fn secret_promotion(types: &TypeInterner, source: TypeId, mut target: TypeId) -> bool {
    if source.index() as usize >= types.len() {
        return false;
    }
    for _ in 0..types.len() {
        if target.index() as usize >= types.len() {
            return false;
        }
        if source == target {
            return true;
        }
        let Type::Secret(inner) = types.resolve(target) else {
            return false;
        };
        target = *inner;
    }
    false
}

fn refinement_ancestor(types: &TypeInterner, mut source: TypeId, target: TypeId) -> bool {
    for _ in 0..types.len() {
        if source.index() as usize >= types.len() {
            return false;
        }
        let Type::Refinement { base, .. } = types.resolve(source) else {
            return false;
        };
        source = *base;
        if source == target {
            return true;
        }
    }
    false
}

/// Match the checker's already-secret exception without discarding a field's
/// nominal refinement identity or peeling the aggregate receiver's wrappers.
fn declared_field_is_secret(types: &TypeInterner, mut ty: TypeId) -> Option<bool> {
    for _ in 0..types.len() {
        if ty.index() as usize >= types.len() {
            return None;
        }
        match types.resolve(ty) {
            Type::Secret(_) => return Some(true),
            Type::Refinement { base, .. } => ty = *base,
            _ => return Some(false),
        }
    }
    None
}

pub(crate) fn validate_field_projection(
    value: &Expression,
    types: &TypeInterner,
) -> Result<(), &'static str> {
    let ExpressionKind::Field {
        base,
        owner_type,
        field,
    } = &value.kind
    else {
        return Err("borrowed projection metadata does not describe a field");
    };
    if owner_type.index() as usize >= types.len() || base.ty.index() as usize >= types.len() {
        return Err("native borrowed projection has an invalid field owner");
    }
    let secret_owner = matches!(
        types.resolve(base.ty),
        Type::Secret(inner)
            if *inner == *owner_type
                && matches!(
                    types.resolve(*inner),
                    Type::Struct(_) | Type::Bitfield(_) | Type::MachineState { .. }
                )
    );
    if base.ty != *owner_type && !secret_owner {
        return Err("native borrowed projection has an invalid field owner");
    }
    let field_type = match types.resolve(*owner_type) {
        Type::Struct(struct_id) => types
            .resolve_struct(*struct_id)
            .fields
            .get(field.index() as usize)
            .map(|(_, ty)| *ty),
        Type::Bitfield(bitfield_id) => types
            .resolve_bitfield(*bitfield_id)
            .fields
            .get(field.index() as usize)
            .map(|field| field.ty),
        Type::MachineState { machine, state } => {
            let Some(state) = types.resolve_machine(*machine).state(*state) else {
                return Err("native borrowed projection has an invalid machine state");
            };
            state.fields.get(field.index() as usize).map(|(_, ty)| *ty)
        }
        _ => {
            return Err(
                "native borrowed projection requires ordinary struct, bitfield or state-qualified machine fields",
            );
        }
    };
    let Some(field_type) = field_type else {
        return Err("native borrowed projection has an invalid field index");
    };
    let Some(already_secret) = declared_field_is_secret(types, field_type) else {
        return Err("native borrowed projection has an invalid field endpoint type");
    };
    let valid_result = if secret_owner && field_type != TypeInterner::NOTHING && !already_secret {
        matches!(types.resolve(value.ty), Type::Secret(inner) if *inner == field_type)
    } else {
        value.ty == field_type
    };
    if !valid_result {
        return Err("native borrowed projection has an invalid field endpoint type");
    }
    Ok(())
}

pub fn validate_local_view_initializer(
    value: &Expression,
    source: LocalId,
    source_type: TypeId,
    target: TypeId,
    types: &TypeInterner,
) -> Result<(), &'static str> {
    if !same_backing_type(types, value.ty, target) {
        return Err("native borrowed alias cannot change its backing representation");
    }
    if !secret_promotion(types, value.ty, target) {
        return Err("native borrowed alias cannot introduce or discard a nominal refinement");
    }
    let mut value = value;
    loop {
        match &value.kind {
            ExpressionKind::Local(id)
                if *id == source && secret_promotion(types, source_type, value.ty) =>
            {
                return Ok(());
            }
            ExpressionKind::Field { base, .. } => {
                validate_field_projection(value, types)?;
                value = base;
            }
            ExpressionKind::View(inner) => {
                if !same_backing_type(types, value.ty, inner.ty) {
                    return Err("native borrowed alias cannot change its backing representation");
                }
                if !secret_promotion(types, inner.ty, value.ty) {
                    return Err(
                        "native borrowed alias cannot introduce or discard a nominal refinement",
                    );
                }
                value = inner;
            }
            ExpressionKind::Coarsen(inner) => {
                if !same_backing_type(types, value.ty, inner.ty)
                    || !refinement_ancestor(types, inner.ty, value.ty)
                {
                    return Err(
                        "native borrowed alias coarsen requires an existing refinement ancestor",
                    );
                }
                value = inner;
            }
            ExpressionKind::Declassify(inner) => {
                if !same_backing_type(types, value.ty, inner.ty)
                    || inner.ty.index() as usize >= types.len()
                    || !matches!(types.resolve(inner.ty), Type::Secret(payload) if *payload == value.ty)
                {
                    return Err(
                        "native borrowed alias declassification requires the exact secret inner type",
                    );
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
                if !secret_promotion(types, inner.ty, value.ty) {
                    return Err(
                        "native borrowed alias conversion cannot introduce or discard a nominal refinement",
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
