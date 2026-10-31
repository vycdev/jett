//! Constructor-owned descriptor storage snapshots in original checked tests.
//! This is an exact reflected-read proof, never a type-only observation policy.

use super::*;
use crate::call_ownership::{
    ArgumentOwnership, CallerBindingFact, CallerBindingMode, CallerOrigin, ObservationProof,
    OwnershipLocalInfo, SourceCallOwnership,
};
use jett_typecheck::{CheckedCallerEffect, CheckedCallerSyntax, CheckedOwnershipContext};

#[derive(Debug, Clone)]
pub(crate) struct OriginalBinding {
    original: CheckedBindingFact,
    current: CallerBindingFact,
    value: Expression,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Binding {
    original: CheckedBindingFact,
    current: CallerBindingFact,
}

#[derive(Clone)]
pub(crate) struct PurposeProof {
    checked: Arc<jett_typecheck::CheckedResourceProgram>,
    call_span: Span,
    source_span: Span,
    context: CheckedOwnershipContext,
    root: CallerBindingFact,
    requested: TypeId,
    bindings: Vec<Binding>,
    constructed_types: Vec<TypeId>,
    admissions: Vec<jett_typecheck::CheckedReflectedCallablePlan>,
}

impl std::fmt::Debug for PurposeProof {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PurposeObservationProof")
            .field("call_span", &self.call_span)
            .field("source_span", &self.source_span)
            .field("context", &self.context)
            .field("root", &self.root)
            .field("bindings", &self.bindings)
            .finish_non_exhaustive()
    }
}

impl PartialEq for PurposeProof {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.checked, &other.checked)
            && self.call_span == other.call_span
            && self.source_span == other.source_span
            && self.context == other.context
            && self.root == other.root
            && self.requested == other.requested
            && self.bindings == other.bindings
            && self.constructed_types == other.constructed_types
            && self.admissions == other.admissions
    }
}
impl Eq for PurposeProof {}

fn test_context(context: CheckedOwnershipContext) -> bool {
    matches!(
        context,
        CheckedOwnershipContext::Verify | CheckedOwnershipContext::Property
    )
}

/// Called only after an original VarDecl's checked definition and local exist.
pub(crate) fn record(
    body: &mut BodyLowerer<'_, '_>,
    declaration: &ast::VarDecl,
    definition: DefId,
    local: LocalId,
    value: &Expression,
) {
    let Some(checked) = &body.parent.reflected_checked_program else {
        return;
    };
    if !test_context(body.ownership_context)
        || !std::ptr::eq(body.parent.module, checked.module())
        || !std::ptr::eq(body.parent.check, checked.checked())
        || declaration.mutable
    {
        return;
    }
    let Some(original) = body.binding_facts.get(&declaration.name.span).copied() else {
        return;
    };
    let Some(stored) = body.locals.get(local.index() as usize) else {
        return;
    };
    if original.definition != definition
        || original.mode != CheckedBindingMode::Owned
        || original.mutable
        || original.ty != stored.ty
        || value.ty != stored.ty
        || stored.id != local
        || stored.view_source.is_some()
        || checked
            .checked()
            .binding_facts
            .get(&original.declaration_span)
            != Some(&original)
        || !matches!(
            body.parent.check.interner.resolve(stored.ty),
            Type::Struct(_) | Type::Function { .. }
        )
        || crate::call_ownership::observation_data_type(&body.parent.check.interner, stored.ty)
    {
        return;
    }
    body.observation_initializers.insert(
        local,
        OriginalBinding {
            original,
            current: CallerBindingFact {
                local,
                declaration_span: original.declaration_span,
                ty: original.ty,
                mode: CallerBindingMode::Owned,
                mutable: false,
            },
            value: value.clone(),
        },
    );
}

/// No supplied mutable expression can create this record: its backing rows
/// came exclusively from the original checked BodyLowerer above.
pub(crate) fn construct(
    body: &BodyLowerer<'_, '_>,
    call_span: Span,
    parameter_index: usize,
    source_span: Span,
    origin: &CallerOrigin,
    expression: &Expression,
    actual_type: TypeId,
) -> Result<Option<ObservationProof>, String> {
    let types = &body.parent.check.interner;
    if parameter_index != 0
        || !test_context(body.ownership_context)
        || !matches!(types.resolve(actual_type), Type::Struct(_))
        || body.intrinsic_ids.get(&call_span) != Some(&IntrinsicId::TypeFieldValue)
    {
        return Ok(None);
    }
    let Some(checked) = &body.parent.reflected_checked_program else {
        return Ok(None);
    };
    if !std::ptr::eq(body.parent.module, checked.module())
        || !std::ptr::eq(body.parent.check, checked.checked())
        || checked.checked().call_ownership.get(&call_span) != body.call_ownership.get(&call_span)
    {
        return Err("observing reflected holder lost its original checked purpose".into());
    }
    let Some(arguments) = body.intrinsic_type_arguments.get(&call_span) else {
        return Ok(None);
    };
    let [owner, requested] = arguments.as_slice() else {
        return Ok(None);
    };
    if *owner != actual_type || !matches!(types.resolve(*requested), Type::Function { .. }) {
        return Ok(None);
    }
    let Some(layout) =
        crate::reflected_fields::reflected_field_layout(types, IntrinsicId::TypeFieldValue, *owner)
    else {
        return Ok(None);
    };
    let admissions = layout
        .into_iter()
        .filter_map(|(_, _, source)| {
            (source != *requested)
                .then(|| {
                    checked
                        .checked()
                        .reflected_callable_plan(source, *requested)
                })
                .flatten()
        })
        .collect::<Vec<_>>();
    if admissions.is_empty() {
        return Ok(None);
    }
    let CallerOrigin::Binding(root) = origin else {
        return Err("observing reflected holder has no original owning local".into());
    };
    let mut value = expression;
    while let ExpressionKind::View(inner) = &value.kind {
        value = inner;
    }
    if !matches!(value.kind, ExpressionKind::Local(local) if local == root.local)
        || value.ty != actual_type
        || root.ty != actual_type
        || root.mode != CallerBindingMode::Owned
        || root.mutable
    {
        return Err("observing reflected holder differs from its immutable checked root".into());
    }
    let mut proof = PurposeProof {
        checked: checked.clone(),
        call_span,
        source_span,
        context: body.ownership_context,
        root: *root,
        requested: *requested,
        bindings: Vec::new(),
        constructed_types: Vec::new(),
        admissions,
    };
    let mut captures = Vec::new();
    reconstruct(body, value, &mut proof, &mut captures, &mut HashSet::new())?;
    captures.sort_unstable_by_key(|(id, _)| id.index());
    captures.dedup();
    Ok(Some(ObservationProof::purpose(
        actual_type,
        captures,
        proof,
    )))
}

fn reconstruct(
    body: &BodyLowerer<'_, '_>,
    value: &Expression,
    proof: &mut PurposeProof,
    captures: &mut Vec<(LocalId, TypeId)>,
    active: &mut HashSet<LocalId>,
) -> Result<(), String> {
    let types = &body.parent.check.interner;
    if !proof.constructed_types.contains(&value.ty) {
        proof.constructed_types.push(value.ty);
    }
    if crate::call_ownership::observation_data_type(types, value.ty) {
        return Ok(());
    }
    if let ExpressionKind::Local(local) = &value.kind {
        if !active.insert(*local) {
            return Err("observing reflected holder has cyclic local backing".into());
        }
        let row = body
            .observation_initializers
            .get(local)
            .ok_or("observing reflected holder has no original immutable constructor row")?;
        if row.current.ty != value.ty || row.value.ty != value.ty {
            return Err("observing reflected holder backing has another checked type".into());
        }
        if !proof
            .bindings
            .iter()
            .any(|binding| binding.current.local == *local)
        {
            proof.bindings.push(Binding {
                original: row.original,
                current: row.current,
            });
        }
        reconstruct(body, &row.value, proof, captures, active)?;
        active.remove(local);
        return Ok(());
    }
    if matches!(types.resolve(value.ty), Type::Function { .. }) {
        let descriptor = body.observation_proof(value, value.ty)?;
        captures.extend_from_slice(descriptor.captures());
        return Ok(());
    }
    let (
        Type::Struct(id),
        ExpressionKind::StructConstruct {
            struct_type,
            fields,
            evaluation_order,
            ..
        },
    ) = (types.resolve(value.ty), &value.kind)
    else {
        return Err(
            "observing reflected holder has no supported original constructor proof".into(),
        );
    };
    let declared = &types.resolve_struct(*id).fields;
    let mut order = evaluation_order.clone();
    order.sort_unstable();
    if *struct_type != value.ty
        || fields.len() != declared.len()
        || order != (0..fields.len()).collect::<Vec<_>>()
        || types
            .nominal_type_arguments(value.ty)
            .iter()
            .any(|ty| !crate::call_ownership::observation_data_type(types, *ty))
    {
        return Err("observing reflected holder constructor changed its checked layout".into());
    }
    for (field, (_, expected)) in fields.iter().zip(declared) {
        if field.ty != *expected {
            return Err("observing reflected holder has another declared field type".into());
        }
        reconstruct(body, field, proof, captures, active)?;
    }
    Ok(())
}

impl PurposeProof {
    pub(crate) fn metadata_types(&self, mut visit: impl FnMut(TypeId)) {
        for &ty in &self.constructed_types {
            visit(ty);
        }
        visit(self.requested);
    }
    pub(crate) fn metadata_locals(&self, mut visit: impl FnMut(LocalId)) {
        visit(self.root.local);
        for binding in &self.bindings {
            visit(binding.current.local);
        }
    }
    pub(crate) fn remap_locals<E>(
        &mut self,
        map: &mut impl FnMut(LocalId) -> Result<LocalId, E>,
    ) -> Result<(), E> {
        self.root.local = map(self.root.local)?;
        for binding in &mut self.bindings {
            binding.current.local = map(binding.current.local)?;
        }
        Ok(())
    }
    pub(crate) fn validate(
        &self,
        argument: &ArgumentOwnership,
        source: &SourceCallOwnership,
        types: &TypeInterner,
        local: &impl Fn(LocalId) -> Option<OwnershipLocalInfo>,
    ) -> Result<(), String> {
        if !test_context(self.context)
            || source.context != self.context
            || source.target
                != crate::call_ownership::CallTarget::Intrinsic(IntrinsicId::TypeFieldValue)
            || argument.effect != CheckedCallerEffect::ObserveData
            || argument.syntax != CheckedCallerSyntax::WrittenView
            || argument.parameter_index != 0
            || argument.source_index != 0
            || argument.source_span != self.source_span
            || argument.actual_type != self.root.ty
            || argument.origin != CallerOrigin::Binding(self.root)
            || self
                .bindings
                .first()
                .is_none_or(|binding| binding.current != self.root)
            || self.admissions.is_empty()
            || self.admissions.iter().any(|admission| {
                admission.requested_type() != self.requested
                    || !admission.validate(self.checked.checked())
            })
        {
            return Err(
                "call ownership reflected observation lost its exact checked read/root/purpose"
                    .into(),
            );
        }
        let original_types = &self.checked.checked().interner;
        for &ty in &self.constructed_types {
            if ty.index() as usize >= types.len()
                || ty.index() as usize >= original_types.len()
                || types.resolve(ty) != original_types.resolve(ty)
                || types.nominal_type_arguments(ty) != original_types.nominal_type_arguments(ty)
                || matches!(original_types.resolve(ty), Type::Struct(id)
                    if types.resolve_struct(*id) != original_types.resolve_struct(*id))
            {
                return Err(
                    "call ownership reflected observation changed an original constructor type"
                        .into(),
                );
            }
        }
        for binding in &self.bindings {
            let fact = binding.current;
            let Some(stored) = local(fact.local) else {
                return Err(
                    "call ownership reflected observation binding is outside its owner".into(),
                );
            };
            if self
                .checked
                .checked()
                .binding_facts
                .get(&binding.original.declaration_span)
                != Some(&binding.original)
                || binding.original.mode != CheckedBindingMode::Owned
                || binding.original.mutable
                || binding.original.ty != fact.ty
                || binding.original.declaration_span != fact.declaration_span
                || fact.mode != CallerBindingMode::Owned
                || fact.mutable
                || stored.mutable
                || stored.ty != fact.ty
                || stored.view_source.is_some()
                || stored.is_view_parameter
                || stored.view_iteration.is_some()
                || stored.span.file != fact.declaration_span.file
                || fact.declaration_span.start < stored.span.start
                || fact.declaration_span.end > stored.span.end
            {
                return Err("call ownership reflected observation lost original immutable binding ownership".into());
            }
        }
        Ok(())
    }

    /// This join relies on the separately mandatory whole-source/current graph
    /// seal; keeping a signature or cloning a public packet cannot replace it.
    pub(crate) fn validate_read(&self, expression: &Expression) -> Result<(), String> {
        let ExpressionKind::Intrinsic {
            intrinsic: IntrinsicId::TypeFieldValue,
            type_arguments,
            field_validation: Some(ReflectedFieldValidation::Validate(plans)),
            ..
        } = &expression.kind
        else {
            return Err("call ownership reflected observation has no sealed callable read".into());
        };
        if expression.span != self.call_span || type_arguments.as_slice() != [self.root.ty, self.requested]
            || !plans.iter().any(|plan| matches!(&plan.action, ReflectedFieldAction::CallableReturn(callable)
                if callable.authenticates_observation_read(&self.checked, self.call_span, self.context)
                    && self.admissions.contains(callable.checked_plan())))
        { return Err("call ownership reflected observation lost its original sealed callable read".into()); }
        Ok(())
    }
}

#[cfg(test)]
#[path = "call_observation_tests.rs"]
mod tests;
