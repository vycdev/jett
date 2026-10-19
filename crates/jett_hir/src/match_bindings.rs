//! Canonical enum payload binding origins, scoped to one actual match arm.
//! A match binding is not an ABI parameter or an ownership permission.
use jett_common::Span;
use jett_types::{Type, TypeId, TypeInterner};

use crate::{LocalId, OwnershipLocalInfo, VariantId};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MatchBinding {
    match_span: Span,
    enum_type: TypeId,
    variant: VariantId,
    field_index: usize,
    binder_type: TypeId,
    declaration_span: Span,
}

impl MatchBinding {
    pub fn match_span(self) -> Span {
        self.match_span
    }
    pub fn enum_type(self) -> TypeId {
        self.enum_type
    }
    pub fn variant(self) -> VariantId {
        self.variant
    }
    pub fn field_index(self) -> usize {
        self.field_index
    }
    pub fn binder_type(self) -> TypeId {
        self.binder_type
    }
    pub fn declaration_span(self) -> Span {
        self.declaration_span
    }
    pub(crate) fn matches_binding(self, ty: TypeId, declaration: Span) -> bool {
        self.binder_type == ty && self.declaration_span == declaration
    }
}

fn inside(child: Span, parent: Span) -> bool {
    child.file == parent.file && child.start >= parent.start && child.end <= parent.end
}

/// Prove the canonical enum/tag/field relation. Consumers separately establish
/// the exact LocalId, original/current match association and arm body scope.
pub fn checked_match_binding(
    types: &TypeInterner,
    match_span: Span,
    enum_type: TypeId,
    variant: VariantId,
    field_index: usize,
    binder_type: TypeId,
    declaration_span: Span,
) -> Result<MatchBinding, String> {
    if enum_type == TypeInterner::ERROR
        || binder_type == TypeInterner::ERROR
        || enum_type.index() as usize >= types.len()
        || binder_type.index() as usize >= types.len()
        || !inside(declaration_span, match_span)
    {
        return Err("match binding has an invalid exact type or Source association".into());
    }
    let Type::Enum(id) = types.resolve(enum_type) else {
        return Err("match binding owner is not an exact enum".into());
    };
    let field = types
        .resolve_enum(*id)
        .variants
        .get(variant.index() as usize)
        .and_then(|variant| variant.fields.get(field_index))
        .ok_or("match binding tag or field slot is outside its canonical enum")?;
    if field.1 != binder_type {
        return Err("match binding type differs from its canonical payload field".into());
    }
    Ok(MatchBinding {
        match_span,
        enum_type,
        variant,
        field_index,
        binder_type,
        declaration_span,
    })
}

pub(crate) fn checked_match_arm_bindings(
    types: &TypeInterner,
    match_span: Span,
    enum_type: TypeId,
    variant: Option<VariantId>,
    arm_span: Span,
    bindings: &[LocalId],
    local: impl Fn(LocalId) -> Option<OwnershipLocalInfo>,
) -> Result<Vec<(LocalId, MatchBinding)>, String> {
    if bindings.is_empty() {
        return Ok(Vec::new());
    }
    let variant = variant.ok_or("catch-all match arm cannot declare enum payload bindings")?;
    if enum_type == TypeInterner::ERROR || enum_type.index() as usize >= types.len() {
        return Err("match binding has an invalid exact enum type".into());
    }
    let Type::Enum(id) = types.resolve(enum_type) else {
        return Err("match binding owner is not an exact enum".into());
    };
    let fields = &types
        .resolve_enum(*id)
        .variants
        .get(variant.index() as usize)
        .ok_or("match binding tag is outside its exact enum")?
        .fields;
    if bindings.len() != fields.len() || !inside(arm_span, match_span) {
        return Err("match bindings differ from their exact canonical arm".into());
    }
    let mut result = Vec::new();
    for (field_index, &id) in bindings.iter().enumerate() {
        if bindings[..field_index].contains(&id) {
            return Err("match payload reuses another binding slot".into());
        }
        let metadata = local(id).ok_or("match binding is outside its exact function")?;
        if metadata.mutable
            || metadata.view_source.is_some()
            || metadata.is_view_parameter
            || metadata.view_iteration.is_some()
            || metadata.match_binding.is_some()
            || !inside(metadata.span, arm_span)
        {
            return Err("match payload is not a fresh immutable binding in its exact arm".into());
        }
        let proof = checked_match_binding(
            types,
            match_span,
            enum_type,
            variant,
            field_index,
            metadata.ty,
            metadata.span,
        )?;
        if !jett_typecheck::ownership::is_implicitly_copyable(types, metadata.ty) {
            result.push((id, proof));
        }
    }
    Ok(result)
}
