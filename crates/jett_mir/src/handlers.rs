//! Extract source result/optional handlers into ordinary MIR CFG edges.
use super::*;
use jett_hir::{ExpressionKind, HandleKind};
use jett_types::{Type, TypeInterner};

mod reflected;

#[derive(Clone, Copy, PartialEq, Eq)]
enum LoweringRequirement {
    #[cfg(test)]
    Handlers,
    HandlersAndCallOwnership,
}

#[cfg(test)]
fn has_extractable_handle(expression: &Expression) -> bool {
    expression_needs_lowering(expression, LoweringRequirement::Handlers)
}

fn needs_eager_lowering(expression: &Expression) -> bool {
    expression_needs_lowering(expression, LoweringRequirement::HandlersAndCallOwnership)
}

fn expression_needs_lowering(expression: &Expression, requirement: LoweringRequirement) -> bool {
    let needs = |value: &Expression| expression_needs_lowering(value, requirement);
    let includes_call_ownership = requirement == LoweringRequirement::HandlersAndCallOwnership;
    match &expression.kind {
        ExpressionKind::Handle {
            kind: HandleKind::Result | HandleKind::Optional | HandleKind::Refinement { .. },
            ..
        } => true,
        ExpressionKind::View(value) | ExpressionKind::FunctionAdapter { value, .. } => needs(value),
        ExpressionKind::Clone(value) => needs(value),
        ExpressionKind::Run(value) | ExpressionKind::Join(value) => needs(value),
        ExpressionKind::Coarsen(value)
        | ExpressionKind::Declassify(value)
        | ExpressionKind::RefinementValidated(value)
        | ExpressionKind::DisplayResult(value)
        | ExpressionKind::EquatableResult(value)
        | ExpressionKind::RuntimeFailureMessage(value)
        | ExpressionKind::InterfaceCoerce { value, .. }
        | ExpressionKind::InterfaceType(value) => needs(value),
        ExpressionKind::Field { base, .. } => needs(base),
        ExpressionKind::StateIs { value, .. } => needs(value),
        ExpressionKind::Unary { value, .. } => needs(value),
        ExpressionKind::Binary { left, right, .. } => needs(left) || needs(right),
        ExpressionKind::Call {
            args, ownership, ..
        } => {
            (includes_call_ownership && needs_call_owner_staging(ownership))
                || args.iter().any(needs)
        }
        ExpressionKind::ActorSpawn { args, .. } => args.iter().any(needs),
        ExpressionKind::ActorMessage { actor, args, .. } => needs(actor) || args.iter().any(needs),
        ExpressionKind::Intrinsic {
            args,
            refinement_predicates,
            field_validation,
            ownership,
            ..
        } => {
            (includes_call_ownership && needs_call_owner_staging(ownership))
                || args.iter().any(needs)
                || refinement_predicates.iter().any(|chain| !chain.is_empty())
                || matches!(
                    field_validation,
                    Some(hir::ReflectedFieldValidation::Validate(_))
                )
        }
        ExpressionKind::IndirectCall {
            callee,
            args,
            ownership,
            ..
        } => {
            (includes_call_ownership && needs_call_owner_staging(ownership))
                || needs(callee)
                || args.iter().any(needs)
        }
        ExpressionKind::ListConstruct { elements } => elements.iter().any(needs),
        ExpressionKind::StringInterpolation(segments) => segments
            .iter()
            .any(|segment| matches!(segment, hir::StringSegment::Value(value) if needs(value))),
        ExpressionKind::MapConstruct { entries } => entries
            .iter()
            .any(|entry| needs(&entry.key) || needs(&entry.value)),
        ExpressionKind::StructConstruct {
            fields,
            refinement_predicates,
            ..
        } => {
            fields.iter().any(needs) || refinement_predicates.iter().any(|chain| !chain.is_empty())
        }
        ExpressionKind::BitfieldConstruct { fields, .. } => fields.iter().any(needs),
        ExpressionKind::EnumConstruct { payloads, .. }
        | ExpressionKind::MachineConstruct { payloads, .. } => payloads.iter().any(needs),
        ExpressionKind::MachineTransition {
            source, payloads, ..
        } => needs(source) || payloads.iter().any(needs),
        ExpressionKind::ResultOk(value)
        | ExpressionKind::ResultFail(value)
        | ExpressionKind::OptionalSome(value) => needs(value),
        _ => false,
    }
}

pub(super) fn can_snapshot_view(types: &TypeInterner, ty: TypeId) -> bool {
    fn supported(
        types: &TypeInterner,
        ty: TypeId,
        seen: &mut std::collections::HashSet<TypeId>,
    ) -> bool {
        if ty.index() as usize >= types.len() {
            return false;
        }
        if !seen.insert(ty) {
            return true;
        }
        match types.resolve(ty) {
            Type::Int8
            | Type::Int16
            | Type::Int32
            | Type::Int64
            | Type::Uint8
            | Type::Uint16
            | Type::Uint32
            | Type::Uint64
            | Type::Float32
            | Type::Float64
            | Type::String
            | Type::Bool
            | Type::Bytes
            | Type::Nothing
            | Type::TypeConstruction
            | Type::Function { .. } => true,
            Type::List(inner) => *inner == TypeInterner::NEVER || supported(types, *inner, seen),
            Type::Set(inner) | Type::Secret(inner) | Type::Refinement { base: inner, .. } => {
                supported(types, *inner, seen)
            }
            Type::Optional(inner) => {
                *inner == TypeInterner::NEVER || supported(types, *inner, seen)
            }
            Type::Result(ok, error) => {
                match (*ok == TypeInterner::NEVER, *error == TypeInterner::NEVER) {
                    (true, true) => false,
                    (true, false) => supported(types, *error, seen),
                    (false, true) => supported(types, *ok, seen),
                    (false, false) => supported(types, *ok, seen) && supported(types, *error, seen),
                }
            }
            Type::Map(key, value) => {
                *key == TypeInterner::NEVER
                    || *value == TypeInterner::NEVER
                    || (supported(types, *key, seen) && supported(types, *value, seen))
            }
            Type::Struct(id) => types
                .resolve_struct(*id)
                .fields
                .iter()
                .all(|(_, field_ty)| supported(types, *field_ty, seen)),
            Type::Enum(id) => types.resolve_enum(*id).variants.iter().all(|variant| {
                variant
                    .fields
                    .iter()
                    .all(|(_, field_ty)| supported(types, *field_ty, seen))
            }),
            Type::Bitfield(id) => types
                .resolve_bitfield(*id)
                .fields
                .iter()
                .all(|field| supported(types, field.ty, seen)),
            Type::Machine(id) | Type::MachineState { machine: id, .. } => {
                types.resolve_machine(*id).states.iter().all(|state| {
                    state
                        .fields
                        .iter()
                        .all(|(_, field_ty)| supported(types, *field_ty, seen))
                })
            }
            Type::Resource(_)
            | Type::Capability(_)
            | Type::Actor(_)
            | Type::Interface(_)
            | Type::Never
            | Type::Error => false,
        }
    }

    supported(types, ty, &mut std::collections::HashSet::new())
}

fn valid_ordered_owned_values(
    types: &TypeInterner,
    locals: &[Local],
    values: &[Expression],
    order: &[usize],
) -> bool {
    // MIR has no borrowed temporary. Cloneable views can be snapshotted into
    // owned locals before a later handler changes their source.
    order.len() == values.len()
        && order.iter().all(|&index| index < values.len())
        && order
            .iter()
            .copied()
            .collect::<std::collections::BTreeSet<_>>()
            .len()
            == values.len()
        && values.iter().all(|value| {
            !matches!(value.kind, ExpressionKind::View(_))
                || can_snapshot_view(types, value.ty)
                || stable_deferred_view(locals, value)
        })
}

fn stable_deferred_view(locals: &[Local], expression: &Expression) -> bool {
    let ExpressionKind::View(value) = &expression.kind else {
        return false;
    };
    let ExpressionKind::Local(id) = value.kind else {
        return false;
    };
    let Some(root) = jett_hir::local_view_root(locals, id) else {
        return false;
    };
    locals
        .get(root.index() as usize)
        .is_some_and(|local| local.id == root && !local.mutable)
}

/// Preserve an already-checked retained place until its one endpoint snapshot.
/// Generic value lowering may snapshot a Local below Coarsen/Declassify; that
/// inner acquisition would change the original Source binding/field witness.
fn retained_snapshot_place(expression: &Expression) -> Option<Expression> {
    let mut place = expression;
    loop {
        place = match &place.kind {
            ExpressionKind::Local(_) => return Some(expression.clone()),
            ExpressionKind::Field { base, .. } => base,
            ExpressionKind::View(value)
            | ExpressionKind::Coarsen(value)
            | ExpressionKind::Declassify(value) => value,
            _ => return None,
        };
    }
}

/// Ordered staging distinguishes actual borrows from an endpoint whose checked
/// Source effect requires an owning transfer before physical View access.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OrderedCallViewInput {
    Unstaged,
    Borrowed,
    SourceOwningTransfer,
}

fn ordered_call_view_input(ownership: &hir::CallOwnership, index: usize) -> OrderedCallViewInput {
    use jett_typecheck::{CheckedCalleeAccess as Access, CheckedCallerEffect as Effect};
    match ownership {
        hir::CallOwnership::Source(source) => {
            let Some(argument) = source.arguments.get(index) else {
                return OrderedCallViewInput::Unstaged;
            };
            if argument.staging != hir::ArgumentStaging::Original
                || argument.physical_access != Access::View
            {
                return OrderedCallViewInput::Unstaged;
            }
            match argument.effect {
                Effect::RelinquishOwned => OrderedCallViewInput::SourceOwningTransfer,
                Effect::TransferOwned if source.bridge != hir::CallBridge::Direct => {
                    OrderedCallViewInput::SourceOwningTransfer
                }
                Effect::RetainBorrow | Effect::ObserveData => OrderedCallViewInput::Borrowed,
                _ => OrderedCallViewInput::Unstaged,
            }
        }
        hir::CallOwnership::Generated(generated) => {
            if generated.arguments.get(index).is_some_and(|argument| {
                argument.callee_access == Access::View
                    && matches!(
                        argument.acquisition,
                        hir::GeneratedAcquisition::Borrow { .. } | hir::GeneratedAcquisition::Copy
                    )
            }) {
                OrderedCallViewInput::Borrowed
            } else {
                OrderedCallViewInput::Unstaged
            }
        }
    }
}

/// Retained/generated raw borrows keep their existing origin eligibility.
/// A checked Source owning transfer instead materializes the full endpoint.
fn valid_ordered_borrowed_values(
    types: &TypeInterner,
    locals: &[Local],
    values: &[Expression],
    order: &[usize],
    inputs: &[OrderedCallViewInput],
) -> bool {
    inputs.len() == values.len()
        && order.len() == values.len()
        && order.iter().all(|&index| index < values.len())
        && order
            .iter()
            .copied()
            .collect::<std::collections::BTreeSet<_>>()
            .len()
            == values.len()
        && values.iter().enumerate().all(|(index, value)| {
            !matches!(value.kind, ExpressionKind::View(_))
                || inputs[index] == OrderedCallViewInput::SourceOwningTransfer
                || can_snapshot_view(types, value.ty)
                || stable_deferred_view(locals, value)
                || (inputs[index] == OrderedCallViewInput::Borrowed
                    && (crate::call_views::borrowed_source(types, locals, value).is_some()
                        || crate::call_views::temporary_root(types, locals, value).is_some()))
        })
}

#[derive(Clone, Copy)]
enum ObservationSnapshot {
    Direct,
    Converted {
        actual_type: TypeId,
        source_span: Span,
    },
}

fn observation_snapshot(
    types: &TypeInterner,
    value: &Expression,
    ownership: &hir::CallOwnership,
    index: usize,
) -> Option<ObservationSnapshot> {
    let hir::CallOwnership::Source(source) = ownership else {
        return None;
    };
    let argument = source.arguments.get(index)?;
    argument.observation_proof()?;
    if can_snapshot_view(types, value.ty) {
        return Some(ObservationSnapshot::Direct);
    }
    let ExpressionKind::InterfaceCoerce { .. } = &value.kind else {
        return None;
    };
    let mut raw = value;
    while let ExpressionKind::InterfaceCoerce { value, .. } = &raw.kind {
        raw = value;
    }
    if raw.ty != argument.actual_type
        || raw.span != argument.source_span
        || !hir::observation_data_type(types, raw.ty)
        || !can_snapshot_view(types, raw.ty)
    {
        return None;
    }
    Some(ObservationSnapshot::Converted {
        actual_type: argument.actual_type,
        source_span: argument.source_span,
    })
}

fn snapshotable_local(types: &TypeInterner, expression: &Expression) -> bool {
    if let ExpressionKind::InterfaceCoerce { value, .. } = &expression.kind {
        return snapshotable_local(types, value);
    }
    matches!(expression.kind, ExpressionKind::Local(_))
        && crate::move_values::is_linear(types, expression.ty)
        && can_snapshot_view(types, expression.ty)
}

fn snapshot_local(expression: &Expression, lowered: Expression) -> Expression {
    if let (
        ExpressionKind::InterfaceCoerce { value: source, .. },
        ExpressionKind::InterfaceCoerce { value, adapters },
    ) = (&expression.kind, &lowered.kind)
    {
        return Expression {
            kind: ExpressionKind::InterfaceCoerce {
                value: Box::new(snapshot_local(source, value.as_ref().clone())),
                adapters: adapters.clone(),
            },
            ty: lowered.ty,
            span: lowered.span,
        };
    }
    Expression {
        kind: ExpressionKind::Clone(Box::new(lowered)),
        ty: expression.ty,
        span: expression.span,
    }
}

/// Staging preserves a checked call's borrowing modes. Owned arguments keep
/// their source ownership and must never acquire an implicit alias snapshot.
fn call_staging_inputs(values: &[Expression], modes: &[ParamMode]) -> Option<Vec<Expression>> {
    if values.len() != modes.len() {
        return None;
    }
    values
        .iter()
        .zip(modes)
        .map(|(value, mode)| {
            if *mode == ParamMode::View {
                Some(if matches!(value.kind, ExpressionKind::View(_)) {
                    value.clone()
                } else {
                    Expression {
                        kind: ExpressionKind::View(Box::new(value.clone())),
                        ty: value.ty,
                        span: value.span,
                    }
                })
            } else if matches!(value.kind, ExpressionKind::View(_)) {
                // The frontend rejects an explicit view at an owned boundary.
                // Leave invalid HIR unsupported instead of manufacturing a copy.
                None
            } else {
                Some(value.clone())
            }
        })
        .collect()
}

fn needs_call_owner_staging(ownership: &hir::CallOwnership) -> bool {
    (0..ownership.parameter_count()).any(|index| {
        ownership.source_parameter_effect(index)
            == Some(jett_typecheck::CheckedCallerEffect::ObserveData)
            || (ownership.parameter_physical_access(index)
                == Some(jett_typecheck::CheckedCalleeAccess::View)
                && matches!(
                    ownership.source_parameter_effect(index),
                    Some(
                        jett_typecheck::CheckedCallerEffect::RelinquishOwned
                            | jett_typecheck::CheckedCallerEffect::TransferOwned
                    )
                ))
    })
}

impl Builder<'_> {
    /// Stage a checked invocation in lexical source order. Physical borrowing
    /// never grants retention: a relinquished input first acquires its own slot.
    fn lower_ordered_call_values(
        &mut self,
        values: &[Expression],
        order: &[usize],
        ownership: &mut hir::CallOwnership,
    ) -> Option<Vec<Expression>> {
        // A staging attempt changes CFG, locals, handlers and active scopes.
        // Restore all of them if a later operand cannot prove its acquisition.
        let checkpoint = self.clone();
        let original_ownership = ownership.clone();
        let result = self.try_lower_ordered_call_values(values, order, ownership);
        if result.is_none() {
            *self = checkpoint;
            *ownership = original_ownership;
        }
        result
    }

    fn lower_converted_observation_snapshot(
        &mut self,
        value: &Expression,
        actual_type: TypeId,
        source_span: Span,
    ) -> Option<Expression> {
        if let ExpressionKind::InterfaceCoerce {
            value: raw,
            adapters,
        } = &value.kind
        {
            return Some(Expression {
                kind: ExpressionKind::InterfaceCoerce {
                    value: Box::new(self.lower_converted_observation_snapshot(
                        raw,
                        actual_type,
                        source_span,
                    )?),
                    adapters: adapters.clone(),
                },
                ty: value.ty,
                span: value.span,
            });
        }
        if value.ty != actual_type || value.span != source_span {
            return None;
        }
        Some(Expression {
            kind: ExpressionKind::Clone(Box::new(self.lower_value(value))),
            ty: value.ty,
            span: value.span,
        })
    }

    fn try_lower_ordered_call_values(
        &mut self,
        values: &[Expression],
        order: &[usize],
        ownership: &mut hir::CallOwnership,
    ) -> Option<Vec<Expression>> {
        if ownership.parameter_count() != values.len() {
            return None;
        }
        let inputs = (0..values.len())
            .map(|index| ordered_call_view_input(ownership, index))
            .collect::<Vec<_>>();
        if !valid_ordered_borrowed_values(self.types, &self.locals, values, order, &inputs) {
            return None;
        }
        if let hir::CallOwnership::Source(source) = ownership {
            if source
                .arguments
                .iter()
                .any(|argument| argument.staging != hir::ArgumentStaging::Original)
            {
                return None;
            }
        }
        // Refuse an unsupported snapshot before changing the builder or scopes.
        if values.iter().enumerate().any(|(index, value)| {
            ownership.source_parameter_effect(index)
                == Some(jett_typecheck::CheckedCallerEffect::ObserveData)
                && ownership.parameter_physical_access(index)
                    == Some(jett_typecheck::CheckedCalleeAccess::Owned)
                && observation_snapshot(self.types, value, ownership, index).is_none()
        }) {
            return None;
        }
        self.call_view_scopes.push(Vec::new());
        let mut lowered = values.to_vec();
        for &index in order {
            if inputs[index] == OrderedCallViewInput::Borrowed
                && matches!(ownership, hir::CallOwnership::Generated(_))
            {
                lowered[index] =
                    self.lower_generated_call_view(index, &values[index], ownership)?;
                continue;
            }
            if ownership.source_parameter_effect(index)
                == Some(jett_typecheck::CheckedCallerEffect::ObserveData)
                && ownership.parameter_physical_access(index)
                    == Some(jett_typecheck::CheckedCalleeAccess::Owned)
            {
                let snapshot = observation_snapshot(self.types, &values[index], ownership, index)?;
                let value = match snapshot {
                    ObservationSnapshot::Direct => Expression {
                        kind: ExpressionKind::Clone(Box::new(self.lower_value(&values[index]))),
                        ty: values[index].ty,
                        span: values[index].span,
                    },
                    ObservationSnapshot::Converted {
                        actual_type,
                        source_span,
                    } => self.lower_converted_observation_snapshot(
                        &values[index],
                        actual_type,
                        source_span,
                    )?,
                };
                let owner = self.temporary(value.ty, value.span);
                ownership.stage_observation(index, owner).ok()?;
                self.push(
                    StatementKind::Let {
                        local: owner,
                        value,
                    },
                    values[index].span,
                );
                lowered[index] = Expression {
                    kind: ExpressionKind::Local(owner),
                    ty: values[index].ty,
                    span: values[index].span,
                };
                continue;
            }
            if inputs[index] == OrderedCallViewInput::SourceOwningTransfer {
                let ExpressionKind::View(original) = &values[index].kind else {
                    return None;
                };
                // Evaluate the full endpoint/producer, not a projected parent.
                // An ordinary Let moves a linear owner; no snapshot is inserted.
                let value = self.lower_value(original);
                let owner = self.temporary(value.ty, value.span);
                self.push(
                    StatementKind::Let {
                        local: owner,
                        value,
                    },
                    original.span,
                );
                let projection = Expression {
                    kind: ExpressionKind::View(Box::new(Expression {
                        kind: ExpressionKind::Local(owner),
                        ty: original.ty,
                        span: original.span,
                    })),
                    ty: values[index].ty,
                    span: values[index].span,
                };
                lowered[index] =
                    self.stage_checked_call_view(index, Some(owner), owner, projection, ownership)?;
                continue;
            }
            if inputs[index] == OrderedCallViewInput::Borrowed
                && let hir::CallOwnership::Source(source) = ownership
                && source.arguments[index].retained_snapshot_type() == Some(values[index].ty)
                && can_snapshot_view(self.types, values[index].ty)
            {
                let endpoint = retained_snapshot_place(&values[index])
                    .unwrap_or_else(|| self.lower_value(&values[index]));
                let value = Expression {
                    kind: ExpressionKind::Clone(Box::new(endpoint)),
                    ty: values[index].ty,
                    span: values[index].span,
                };
                let owner = self.temporary(value.ty, value.span);
                self.push(
                    StatementKind::Let {
                        local: owner,
                        value,
                    },
                    values[index].span,
                );
                let projection = Expression {
                    kind: ExpressionKind::View(Box::new(Expression {
                        kind: ExpressionKind::Local(owner),
                        ty: values[index].ty,
                        span: values[index].span,
                    })),
                    ty: values[index].ty,
                    span: values[index].span,
                };
                let loan = self.call_view_temporary(projection.ty, owner, projection.span);
                ownership.stage_retained_snapshot(index, owner, loan).ok()?;
                self.push(
                    StatementKind::BeginCallView {
                        local: loan,
                        value: projection.clone(),
                    },
                    projection.span,
                );
                self.call_view_scopes.last_mut()?.push(loan);
                lowered[index] = Expression {
                    kind: ExpressionKind::View(Box::new(Expression {
                        kind: ExpressionKind::Local(loan),
                        ty: projection.ty,
                        span: projection.span,
                    })),
                    ty: projection.ty,
                    span: projection.span,
                };
                continue;
            }
            if inputs[index] == OrderedCallViewInput::Borrowed
                && let Some((source, projection)) = self.call_view_initializer(&values[index])
            {
                lowered[index] =
                    self.stage_checked_call_view(index, None, source, projection, ownership)?;
                continue;
            }
            if ownership.source_parameter_effect(index)
                == Some(jett_typecheck::CheckedCallerEffect::TransferOwned)
                && ownership.parameter_physical_access(index)
                    == Some(jett_typecheck::CheckedCalleeAccess::Owned)
            {
                // Acquire the endpoint at this source position. A later
                // handler cannot turn a required transfer into a snapshot.
                let value = self.lower_value(&values[index]);
                let owner = self.temporary(value.ty, value.span);
                self.push(
                    StatementKind::Let {
                        local: owner,
                        value,
                    },
                    values[index].span,
                );
                ownership.stage_acquisition(index, owner).ok()?;
                lowered[index] = Expression {
                    kind: ExpressionKind::Local(owner),
                    ty: values[index].ty,
                    span: values[index].span,
                };
                continue;
            }
            let single =
                self.lower_ordered_owned_values(std::slice::from_ref(&values[index]), &[0])?;
            lowered[index] = single.into_iter().next()?;
            if matches!(
                ownership.source_parameter_effect(index),
                Some(
                    jett_typecheck::CheckedCallerEffect::TransferOwned
                        | jett_typecheck::CheckedCallerEffect::Copy
                )
            ) && let ExpressionKind::Local(local) = lowered[index].kind
            {
                ownership.stage_acquisition(index, local).ok()?;
            }
        }
        Some(lowered)
    }

    fn lower_generated_call_view(
        &mut self,
        index: usize,
        value: &Expression,
        ownership: &mut hir::CallOwnership,
    ) -> Option<Expression> {
        let hir::CallOwnership::Generated(generated) = ownership else {
            return None;
        };
        let argument = generated.arguments.get(index)?;
        if argument.staging != (hir::GeneratedArgumentStaging::Existing { loan: None }) {
            return None;
        }
        let witness = argument.original_witness();
        let producer = witness.owned_producer();
        let snapshot =
            witness.ordinary_snapshot_proof().is_some() && can_snapshot_view(self.types, value.ty);
        let ExpressionKind::View(endpoint) = &value.kind else {
            return None;
        };
        let (source, projection, backing) = if producer || snapshot {
            // Capture the full endpoint at this lexical argument position.
            // A produced owner moves once; ordinary data uses its sealed copy.
            let initial = if producer {
                self.lower_value(endpoint)
            } else {
                Expression {
                    kind: ExpressionKind::Clone(Box::new(self.lower_value(value))),
                    ty: value.ty,
                    span: value.span,
                }
            };
            let owner = self.temporary(initial.ty, initial.span);
            self.push(
                StatementKind::Let {
                    local: owner,
                    value: initial,
                },
                value.span,
            );
            let projection = Expression {
                kind: ExpressionKind::View(Box::new(Expression {
                    kind: ExpressionKind::Local(owner),
                    ty: value.ty,
                    span: endpoint.span,
                })),
                ty: value.ty,
                span: value.span,
            };
            (owner, projection, Some((owner, producer)))
        } else {
            let source = crate::call_views::borrowed_source(self.types, &self.locals, value)?;
            (source, value.clone(), None)
        };
        let loan = self.call_view_temporary(projection.ty, source, projection.span);
        let staging = match backing {
            Some((owner, true)) => hir::GeneratedArgumentStaging::OwnedProducer { owner, loan },
            Some((owner, false)) => hir::GeneratedArgumentStaging::OrdinarySnapshot { owner, loan },
            None => hir::GeneratedArgumentStaging::Existing { loan: Some(loan) },
        };
        ownership.stage_generated_parameter(index, staging).ok()?;
        self.push(
            StatementKind::BeginCallView {
                local: loan,
                value: projection.clone(),
            },
            projection.span,
        );
        self.call_view_scopes.last_mut()?.push(loan);
        Some(Expression {
            kind: ExpressionKind::View(Box::new(Expression {
                kind: ExpressionKind::Local(loan),
                ty: projection.ty,
                span: projection.span,
            })),
            ty: projection.ty,
            span: projection.span,
        })
    }

    fn stage_checked_call_view(
        &mut self,
        index: usize,
        acquired_owner: Option<LocalId>,
        source: LocalId,
        projection: Expression,
        ownership: &mut hir::CallOwnership,
    ) -> Option<Expression> {
        let local = self.call_view_temporary(projection.ty, source, projection.span);
        ownership
            .stage_parameter(index, acquired_owner, local)
            .ok()?;
        self.push(
            StatementKind::BeginCallView {
                local,
                value: projection.clone(),
            },
            projection.span,
        );
        self.call_view_scopes.last_mut()?.push(local);
        Some(Expression {
            kind: ExpressionKind::View(Box::new(Expression {
                kind: ExpressionKind::Local(local),
                ty: projection.ty,
                span: projection.span,
            })),
            ty: projection.ty,
            span: projection.span,
        })
    }

    fn refinement_predicate_input(
        &self,
        source: LocalId,
        source_type: TypeId,
        predicate: &hir::RefinementPredicate,
        span: Span,
    ) -> Expression {
        let source_value = Expression {
            kind: ExpressionKind::Local(source),
            ty: source_type,
            span,
        };
        let mut input = Expression {
            kind: ExpressionKind::Clone(Box::new(source_value)),
            ty: source_type,
            span,
        };
        if input.ty != predicate.base_type && input.ty != predicate.input_type {
            input = Expression {
                kind: ExpressionKind::Coarsen(Box::new(input)),
                ty: predicate.base_type,
                span,
            };
        }
        if input.ty != predicate.input_type {
            input = Expression {
                kind: ExpressionKind::Declassify(Box::new(input)),
                ty: predicate.input_type,
                span,
            };
        }
        input
    }

    fn temporary(&mut self, ty: TypeId, span: Span) -> LocalId {
        let id = LocalId::new(self.locals.len() as u32);
        self.locals.push(Local {
            id,
            name: format!("$native{}", id.index()),
            ty,
            debug_ty: ty,
            debug_type_name: None,
            mutable: true,
            view_source: None,
            span,
        });
        id
    }

    fn call_view_temporary(&mut self, ty: TypeId, source: LocalId, span: Span) -> LocalId {
        let id = LocalId::new(self.locals.len() as u32);
        self.locals.push(Local {
            id,
            name: format!("$callView{}", id.index()),
            ty,
            debug_ty: ty,
            debug_type_name: None,
            mutable: false,
            view_source: Some(source),
            span,
        });
        self.view_params.push(id);
        id
    }

    fn call_view_initializer(&mut self, expression: &Expression) -> Option<(LocalId, Expression)> {
        if let Some(source) =
            crate::call_views::borrowed_source(self.types, &self.locals, expression)
        {
            return Some((source, expression.clone()));
        }
        let root = crate::call_views::temporary_root(self.types, &self.locals, expression)?;
        // Ordinary owning storage for the original producer, evaluated once.
        // The stage borrows that storage; neither step clones erased authority.
        let value = self.lower_value(&root);
        let source = self.temporary(root.ty, root.span);
        self.push(
            StatementKind::Let {
                local: source,
                value,
            },
            root.span,
        );
        let rewritten = crate::call_views::replace_borrowed_root(
            expression,
            Expression {
                kind: ExpressionKind::Local(source),
                ty: root.ty,
                span: root.span,
            },
        )?;
        if crate::call_views::borrowed_source(self.types, &self.locals, &rewritten) != Some(source)
        {
            return None;
        }
        Some((source, rewritten))
    }

    fn finish_call_view_scope(&mut self, expression: Expression) -> Expression {
        let Some(scope) = self.call_view_scopes.pop() else {
            return expression;
        };
        if scope.is_empty() {
            return expression;
        }
        // Materialize the complete consuming operation before ending its loans.
        let local = self.temporary(expression.ty, expression.span);
        self.push(
            StatementKind::Let {
                local,
                value: expression.clone(),
            },
            expression.span,
        );
        for stage in scope.into_iter().rev() {
            self.push(StatementKind::EndCallView { local: stage }, expression.span);
        }
        Expression {
            kind: ExpressionKind::Local(local),
            ty: expression.ty,
            span: expression.span,
        }
    }

    pub(super) fn end_call_views_since(&mut self, depth: usize, span: Span) {
        let abandoned = self
            .call_view_scopes
            .iter()
            .skip(depth)
            .rev()
            .flat_map(|scope| scope.iter().rev().copied())
            .collect::<Vec<_>>();
        for local in abandoned {
            self.push(StatementKind::EndCallView { local }, span);
        }
    }

    fn lower_ordered_owned_values(
        &mut self,
        values: &[Expression],
        order: &[usize],
    ) -> Option<Vec<Expression>> {
        if !valid_ordered_owned_values(self.types, &self.locals, values, order) {
            return None;
        }
        let mut lowered = values.to_vec();
        for &index in order {
            // Reading an immutable local through a view has no effect and
            // cannot change which value is observed after an earlier handler.
            // Keep noncopyable views at the call instead of cloning authority.
            if stable_deferred_view(&self.locals, &values[index])
                && !can_snapshot_view(self.types, values[index].ty)
            {
                continue;
            }
            let mut value = self.lower_value(&values[index]);
            if matches!(values[index].kind, ExpressionKind::View(_))
                && (crate::move_values::is_linear(self.types, value.ty)
                    || crate::move_values::is_copy_owned(self.types, value.ty))
            {
                value = Expression {
                    kind: ExpressionKind::Clone(Box::new(value)),
                    ty: values[index].ty,
                    span: values[index].span,
                };
            }
            let local = self.temporary(value.ty, value.span);
            self.push(StatementKind::Let { local, value }, values[index].span);
            lowered[index] = Expression {
                kind: ExpressionKind::Local(local),
                ty: values[index].ty,
                span: values[index].span,
            };
        }
        Some(lowered)
    }

    pub(super) fn lower_value(&mut self, expression: &Expression) -> Expression {
        if let ExpressionKind::FunctionAdapter { value, function } = &expression.kind {
            let mut lowered = expression.clone();
            lowered.kind = ExpressionKind::FunctionAdapter {
                value: Box::new(self.lower_value(value)),
                function: *function,
            };
            return lowered;
        }
        if let ExpressionKind::InterfaceCoerce { value, adapters } = &expression.kind {
            let mut lowered = expression.clone();
            lowered.kind = ExpressionKind::InterfaceCoerce {
                value: Box::new(self.lower_value(value)),
                adapters: adapters.clone(),
            };
            return lowered;
        }
        if let ExpressionKind::InterfaceType(value) = &expression.kind {
            let mut lowered = expression.clone();
            lowered.kind = ExpressionKind::InterfaceType(Box::new(self.lower_value(value)));
            return lowered;
        }
        if let ExpressionKind::RefinementValidated(value) = &expression.kind {
            let mut lowered = expression.clone();
            lowered.kind = ExpressionKind::RefinementValidated(Box::new(self.lower_value(value)));
            return lowered;
        }
        if let ExpressionKind::DisplayResult(value) = &expression.kind {
            let mut lowered = expression.clone();
            lowered.kind = ExpressionKind::DisplayResult(Box::new(self.lower_value(value)));
            return lowered;
        }
        if let ExpressionKind::EquatableResult(value) = &expression.kind {
            let mut lowered = expression.clone();
            lowered.kind = ExpressionKind::EquatableResult(Box::new(self.lower_value(value)));
            return lowered;
        }
        if let ExpressionKind::RuntimeFailureMessage(value) = &expression.kind {
            let mut lowered = expression.clone();
            lowered.kind = ExpressionKind::RuntimeFailureMessage(Box::new(self.lower_value(value)));
            return lowered;
        }
        if let ExpressionKind::View(value) = &expression.kind {
            let mut lowered = expression.clone();
            lowered.kind = ExpressionKind::View(Box::new(self.lower_value(value)));
            return lowered;
        }
        if let ExpressionKind::Clone(value) = &expression.kind
            && needs_eager_lowering(value)
        {
            let mut lowered = expression.clone();
            lowered.kind = ExpressionKind::Clone(Box::new(self.lower_value(value)));
            return lowered;
        }
        if let ExpressionKind::Run(value) = &expression.kind {
            let needs_snapshot = snapshotable_local(self.types, value);
            if needs_eager_lowering(value) || needs_snapshot {
                let mut lowered = expression.clone();
                let source = self.lower_value(value);
                // `run` does not consume a cloneable local in the interpreter.
                let source = if needs_snapshot {
                    snapshot_local(value, source)
                } else {
                    source
                };
                lowered.kind = ExpressionKind::Run(Box::new(source));
                return lowered;
            }
        }
        if let ExpressionKind::Join(value) = &expression.kind
            && needs_eager_lowering(value)
        {
            let mut lowered = expression.clone();
            lowered.kind = ExpressionKind::Join(Box::new(self.lower_value(value)));
            return lowered;
        }
        if let ExpressionKind::Coarsen(value) = &expression.kind
            && (needs_eager_lowering(value) || snapshotable_local(self.types, value))
        {
            let mut lowered = expression.clone();
            let source = self.lower_value(value);
            let source = if snapshotable_local(self.types, value) {
                snapshot_local(value, source)
            } else {
                source
            };
            lowered.kind = ExpressionKind::Coarsen(Box::new(source));
            return lowered;
        }
        if let ExpressionKind::Declassify(value) = &expression.kind
            && (needs_eager_lowering(value) || snapshotable_local(self.types, value))
        {
            let mut lowered = expression.clone();
            let source = self.lower_value(value);
            let source = if snapshotable_local(self.types, value) {
                snapshot_local(value, source)
            } else {
                source
            };
            lowered.kind = ExpressionKind::Declassify(Box::new(source));
            return lowered;
        }
        if let ExpressionKind::Field {
            base,
            owner_type,
            field,
        } = &expression.kind
            && needs_eager_lowering(base)
        {
            let mut lowered = expression.clone();
            lowered.kind = ExpressionKind::Field {
                base: Box::new(self.lower_value(base)),
                owner_type: *owner_type,
                field: *field,
            };
            return lowered;
        }
        if let ExpressionKind::StateIs { value, state } = &expression.kind
            && needs_eager_lowering(value)
        {
            let mut lowered = expression.clone();
            lowered.kind = ExpressionKind::StateIs {
                value: Box::new(self.lower_value(value)),
                state: *state,
            };
            return lowered;
        }
        if let ExpressionKind::Unary { op, value } = &expression.kind
            && needs_eager_lowering(value)
        {
            let mut lowered = expression.clone();
            lowered.kind = ExpressionKind::Unary {
                op: *op,
                value: Box::new(self.lower_value(value)),
            };
            return lowered;
        }
        if let ExpressionKind::Binary { left, op, right } = &expression.kind
            && matches!(op, hir::BinaryOp::And | hir::BinaryOp::Or)
            && (needs_eager_lowering(left) || needs_eager_lowering(right))
        {
            let left_value = self.lower_value(left);
            if !needs_eager_lowering(right) {
                let mut lowered = expression.clone();
                lowered.kind = ExpressionKind::Binary {
                    left: Box::new(left_value),
                    op: *op,
                    right: right.clone(),
                };
                return lowered;
            }
            let span = expression.span;
            let left_local = self.temporary(TypeInterner::BOOL, left.span);
            self.push(
                StatementKind::Let {
                    local: left_local,
                    value: left_value,
                },
                left.span,
            );
            let right_block = self.new_block(right.span);
            let bypass = self.new_block(span);
            let continuation = self.new_block(span);
            let output = self.temporary(TypeInterner::BOOL, span);
            let (then_block, else_block) = if *op == hir::BinaryOp::And {
                (right_block, bypass)
            } else {
                (bypass, right_block)
            };
            self.terminate(
                TerminatorKind::Branch {
                    condition: Expression {
                        kind: ExpressionKind::Local(left_local),
                        ty: TypeInterner::BOOL,
                        span: left.span,
                    },
                    then_block,
                    else_block,
                },
                span,
            );
            self.current = bypass;
            self.push(
                StatementKind::Let {
                    local: output,
                    value: Expression {
                        kind: ExpressionKind::Bool(*op == hir::BinaryOp::Or),
                        ty: TypeInterner::BOOL,
                        span,
                    },
                },
                span,
            );
            self.close_to(continuation, span);
            self.current = right_block;
            let right_value = self.lower_value(right);
            self.push(
                StatementKind::Let {
                    local: output,
                    value: right_value,
                },
                span,
            );
            self.close_to(continuation, span);
            self.current = continuation;
            return Expression {
                kind: ExpressionKind::Local(output),
                ty: TypeInterner::BOOL,
                span,
            };
        }
        if let ExpressionKind::Binary { left, op, right } = &expression.kind
            && !matches!(op, hir::BinaryOp::And | hir::BinaryOp::Or)
            && can_snapshot_view(self.types, left.ty)
            && (needs_eager_lowering(left) || needs_eager_lowering(right))
        {
            // Save the left value before extracting a handler from the right.
            // Its failure block may mutate locals that the left side reads.
            let mut left_value = self.lower_value(left);
            if crate::move_values::is_linear(self.types, left.ty) {
                let borrowed = if matches!(left_value.kind, ExpressionKind::View(_)) {
                    left_value
                } else {
                    Expression {
                        kind: ExpressionKind::View(Box::new(left_value)),
                        ty: left.ty,
                        span: left.span,
                    }
                };
                left_value = Expression {
                    kind: ExpressionKind::Clone(Box::new(borrowed)),
                    ty: left.ty,
                    span: left.span,
                };
            }
            let left_local = self.temporary(left.ty, left.span);
            self.push(
                StatementKind::Let {
                    local: left_local,
                    value: left_value,
                },
                left.span,
            );
            let right_value = self.lower_value(right);
            let mut lowered = expression.clone();
            lowered.kind = ExpressionKind::Binary {
                left: Box::new(Expression {
                    kind: ExpressionKind::Local(left_local),
                    ty: left.ty,
                    span: left.span,
                }),
                op: *op,
                right: Box::new(right_value),
            };
            return lowered;
        }
        if let ExpressionKind::Handle {
            target,
            kind: HandleKind::Refinement { predicates, .. },
            error_local,
            failure,
        } = &expression.kind
        {
            return self.lower_refinement_handle(
                expression,
                target,
                predicates,
                *error_local,
                failure,
            );
        }
        if let ExpressionKind::StructConstruct {
            struct_type,
            fields,
            evaluation_order,
            validates_refinements: true,
            refinement_predicates,
        } = &expression.kind
            && refinement_predicates.len() == fields.len()
            && refinement_predicates.iter().any(|chain| !chain.is_empty())
        {
            return self.lower_refinement_struct_construct(
                expression,
                *struct_type,
                fields,
                evaluation_order,
                refinement_predicates,
            );
        }
        if let ExpressionKind::Intrinsic {
            field_validation: Some(hir::ReflectedFieldValidation::Validate(plans)),
            ..
        } = &expression.kind
        {
            // Invalid plans stay visible for native contract rejection; never
            // turn missing or malformed proof metadata into an unchecked read.
            return self
                .lower_reflected_field_read(expression, plans)
                .unwrap_or_else(|| expression.clone());
        }
        if let ExpressionKind::Intrinsic {
            intrinsic: hir::IntrinsicId::TypeConstructFinish,
            refinement_predicates,
            ..
        } = &expression.kind
            && refinement_predicates.iter().any(|chain| !chain.is_empty())
        {
            return self.lower_refinement_builder_finish(expression, refinement_predicates);
        }
        if let ExpressionKind::ListConstruct { elements } = &expression.kind
            && elements.iter().any(needs_eager_lowering)
            && let Some(elements) =
                self.lower_ordered_owned_values(elements, &(0..elements.len()).collect::<Vec<_>>())
        {
            let mut lowered = expression.clone();
            lowered.kind = ExpressionKind::ListConstruct { elements };
            return lowered;
        }
        if let ExpressionKind::StringInterpolation(segments) = &expression.kind
            && segments.iter().any(|segment| {
                matches!(segment, hir::StringSegment::Value(value) if needs_eager_lowering(value))
            })
        {
            let values = segments
                .iter()
                .filter_map(|segment| match segment {
                    hir::StringSegment::Text(_) => None,
                    hir::StringSegment::Value(value) => Some(value.clone()),
                })
                .collect::<Vec<_>>();
            if let Some(values) =
                self.lower_ordered_owned_values(&values, &(0..values.len()).collect::<Vec<_>>())
            {
                let mut values = values.into_iter();
                let segments = segments
                    .iter()
                    .map(|segment| match segment {
                        hir::StringSegment::Text(text) => hir::StringSegment::Text(text.clone()),
                        hir::StringSegment::Value(_) => {
                            hir::StringSegment::Value(values.next().expect("interpolation value"))
                        }
                    })
                    .collect();
                let mut lowered = expression.clone();
                lowered.kind = ExpressionKind::StringInterpolation(segments);
                return lowered;
            }
        }
        if let ExpressionKind::MapConstruct { entries } = &expression.kind
            && entries
                .iter()
                .any(|entry| needs_eager_lowering(&entry.key) || needs_eager_lowering(&entry.value))
        {
            let values: Vec<_> = entries
                .iter()
                .flat_map(|entry| [entry.key.clone(), entry.value.clone()])
                .collect();
            if let Some(values) =
                self.lower_ordered_owned_values(&values, &(0..values.len()).collect::<Vec<_>>())
            {
                let entries = values
                    .chunks_exact(2)
                    .map(|pair| hir::MapEntry {
                        key: pair[0].clone(),
                        value: pair[1].clone(),
                    })
                    .collect();
                let mut lowered = expression.clone();
                lowered.kind = ExpressionKind::MapConstruct { entries };
                return lowered;
            }
        }
        if let ExpressionKind::StructConstruct {
            struct_type,
            fields,
            evaluation_order,
            validates_refinements,
            refinement_predicates,
        } = &expression.kind
            && fields.iter().any(needs_eager_lowering)
            && let Some(fields) = self.lower_ordered_owned_values(fields, evaluation_order)
        {
            let mut lowered = expression.clone();
            lowered.kind = ExpressionKind::StructConstruct {
                struct_type: *struct_type,
                fields,
                evaluation_order: evaluation_order.clone(),
                validates_refinements: *validates_refinements,
                refinement_predicates: refinement_predicates.clone(),
            };
            return lowered;
        }
        if let ExpressionKind::BitfieldConstruct {
            bitfield_type,
            fields,
            evaluation_order,
            validates_widths,
        } = &expression.kind
            && fields.iter().any(needs_eager_lowering)
            && let Some(fields) = self.lower_ordered_owned_values(fields, evaluation_order)
        {
            let mut lowered = expression.clone();
            lowered.kind = ExpressionKind::BitfieldConstruct {
                bitfield_type: *bitfield_type,
                fields,
                evaluation_order: evaluation_order.clone(),
                validates_widths: *validates_widths,
            };
            return lowered;
        }
        if let ExpressionKind::EnumConstruct {
            enum_type,
            variant,
            payloads,
            evaluation_order,
        } = &expression.kind
            && payloads.iter().any(needs_eager_lowering)
            && let Some(payloads) = self.lower_ordered_owned_values(payloads, evaluation_order)
        {
            let mut lowered = expression.clone();
            lowered.kind = ExpressionKind::EnumConstruct {
                enum_type: *enum_type,
                variant: *variant,
                payloads,
                evaluation_order: evaluation_order.clone(),
            };
            return lowered;
        }
        if let ExpressionKind::MachineConstruct {
            state_type,
            state,
            payloads,
        } = &expression.kind
            && payloads.iter().any(needs_eager_lowering)
            && let Some(payloads) =
                self.lower_ordered_owned_values(payloads, &(0..payloads.len()).collect::<Vec<_>>())
        {
            let mut lowered = expression.clone();
            lowered.kind = ExpressionKind::MachineConstruct {
                state_type: *state_type,
                state: *state,
                payloads,
            };
            return lowered;
        }
        if let ExpressionKind::MachineTransition {
            source,
            state_type,
            target,
            payloads,
        } = &expression.kind
            && (needs_eager_lowering(source) || payloads.iter().any(needs_eager_lowering))
        {
            let values = std::iter::once(source.as_ref().clone())
                .chain(payloads.iter().cloned())
                .collect::<Vec<_>>();
            let order = (0..values.len()).collect::<Vec<_>>();
            if let Some(mut values) = self.lower_ordered_owned_values(&values, &order) {
                let mut lowered = expression.clone();
                lowered.kind = ExpressionKind::MachineTransition {
                    source: Box::new(values.remove(0)),
                    state_type: *state_type,
                    target: *target,
                    payloads: values,
                };
                return lowered;
            }
        }
        if let ExpressionKind::ResultOk(value) = &expression.kind
            && needs_eager_lowering(value)
        {
            let mut lowered = expression.clone();
            lowered.kind = ExpressionKind::ResultOk(Box::new(self.lower_value(value)));
            return lowered;
        }
        if let ExpressionKind::ResultFail(value) = &expression.kind
            && needs_eager_lowering(value)
        {
            let mut lowered = expression.clone();
            lowered.kind = ExpressionKind::ResultFail(Box::new(self.lower_value(value)));
            return lowered;
        }
        if let ExpressionKind::OptionalSome(value) = &expression.kind
            && needs_eager_lowering(value)
        {
            let mut lowered = expression.clone();
            lowered.kind = ExpressionKind::OptionalSome(Box::new(self.lower_value(value)));
            return lowered;
        }
        if let ExpressionKind::Intrinsic {
            intrinsic,
            type_arguments,
            reflection_arguments,
            refinement_predicates,
            field_validation,
            args,
            evaluation_order,
            ownership,
        } = &expression.kind
            && (args.iter().any(needs_eager_lowering) || needs_call_owner_staging(ownership))
        {
            // Normalize closed physical View roles before the common preflight
            // distinguishes a source owner transfer from a retained/raw borrow.
            let inputs = args
                .iter()
                .enumerate()
                .map(|(index, arg)| {
                    if ownership.parameter_physical_access(index)
                        == Some(jett_typecheck::CheckedCalleeAccess::View)
                        && !matches!(arg.kind, ExpressionKind::View(_))
                    {
                        Expression {
                            kind: ExpressionKind::View(Box::new(arg.clone())),
                            ty: arg.ty,
                            span: arg.span,
                        }
                    } else {
                        arg.clone()
                    }
                })
                .collect::<Vec<_>>();
            let mut ownership = ownership.clone();
            if let Some(args) =
                self.lower_ordered_call_values(&inputs, evaluation_order, &mut ownership)
            {
                let mut lowered = expression.clone();
                lowered.kind = ExpressionKind::Intrinsic {
                    intrinsic: *intrinsic,
                    type_arguments: type_arguments.clone(),
                    reflection_arguments: reflection_arguments.clone(),
                    refinement_predicates: refinement_predicates.clone(),
                    field_validation: field_validation.clone(),
                    args,
                    evaluation_order: evaluation_order.clone(),
                    ownership,
                };
                return self.finish_call_view_scope(lowered);
            }
        }
        if let ExpressionKind::ActorSpawn {
            actor_type,
            args,
            evaluation_order,
            constructor,
        } = &expression.kind
            && args.iter().any(needs_eager_lowering)
            && let Some(args) = self.lower_ordered_owned_values(args, evaluation_order)
        {
            let mut lowered = expression.clone();
            lowered.kind = ExpressionKind::ActorSpawn {
                actor_type: actor_type.clone(),
                args,
                evaluation_order: evaluation_order.clone(),
                constructor: *constructor,
            };
            return lowered;
        }
        if let ExpressionKind::ActorMessage {
            actor,
            message,
            handler,
            args,
            evaluation_order,
            kind,
        } = &expression.kind
            && (needs_eager_lowering(actor) || args.iter().any(needs_eager_lowering))
            && valid_ordered_owned_values(self.types, &self.locals, args, evaluation_order)
        {
            let actor_value = self.lower_value(actor);
            let actor_local = self.temporary(actor.ty, actor.span);
            self.push(
                StatementKind::Let {
                    local: actor_local,
                    value: actor_value,
                },
                actor.span,
            );
            let args = self
                .lower_ordered_owned_values(args, evaluation_order)
                .expect("validated actor message argument order");
            let mut lowered = expression.clone();
            lowered.kind = ExpressionKind::ActorMessage {
                actor: Box::new(Expression {
                    kind: ExpressionKind::Local(actor_local),
                    ty: actor.ty,
                    span: actor.span,
                }),
                message: message.clone(),
                handler: *handler,
                args,
                evaluation_order: evaluation_order.clone(),
                kind: *kind,
            };
            return lowered;
        }
        if let ExpressionKind::IndirectCall {
            callee,
            args,
            evaluation_order,
            ownership,
        } = &expression.kind
            && (needs_eager_lowering(callee)
                || args.iter().any(needs_eager_lowering)
                || needs_call_owner_staging(ownership))
            && !matches!(callee.kind, ExpressionKind::View(_))
            && let Type::Function { view_params, .. } = self.types.resolve(
                crate::move_values::representation_type(self.types, callee.ty),
            )
            && let Some(inputs) = call_staging_inputs(
                args,
                &view_params
                    .iter()
                    .map(|view| {
                        if *view {
                            ParamMode::View
                        } else {
                            ParamMode::Owned
                        }
                    })
                    .collect::<Vec<_>>(),
            )
        {
            let mut ownership = ownership.clone();
            let Some(args) =
                self.lower_ordered_call_values(&inputs, evaluation_order, &mut ownership)
            else {
                return expression.clone();
            };
            // Source evaluates call arguments before resolving a function value
            // held in a mutable local. A handler may rebind that local.
            let observed_callee = self.lower_value(callee);
            let callee_value = Expression {
                kind: ExpressionKind::Clone(Box::new(observed_callee)),
                ty: callee.ty,
                span: callee.span,
            };
            let callee_local = self.temporary(callee.ty, callee.span);
            self.push(
                StatementKind::Let {
                    local: callee_local,
                    value: callee_value,
                },
                callee.span,
            );
            let mut lowered = expression.clone();
            lowered.kind = ExpressionKind::IndirectCall {
                callee: Box::new(Expression {
                    kind: ExpressionKind::Local(callee_local),
                    ty: callee.ty,
                    span: callee.span,
                }),
                args,
                evaluation_order: evaluation_order.clone(),
                ownership,
            };
            return self.finish_call_view_scope(lowered);
        }
        if let ExpressionKind::Call {
            function,
            args,
            evaluation_order,
            ownership,
        } = &expression.kind
            && (args.iter().any(needs_eager_lowering) || needs_call_owner_staging(ownership))
            && let Some(modes) = self.function_param_modes.get(function)
            && let Some(inputs) = call_staging_inputs(args, modes)
        {
            let mut ownership = ownership.clone();
            let Some(args) =
                self.lower_ordered_call_values(&inputs, evaluation_order, &mut ownership)
            else {
                return expression.clone();
            };
            let mut lowered = expression.clone();
            lowered.kind = ExpressionKind::Call {
                function: *function,
                args,
                evaluation_order: evaluation_order.clone(),
                ownership,
            };
            return self.finish_call_view_scope(lowered);
        }
        let ExpressionKind::Handle {
            target,
            kind: HandleKind::Result | HandleKind::Optional,
            error_local,
            failure,
        } = &expression.kind
        else {
            // Other expression-level control flow remains explicitly unsupported
            // by native ownership validation, rather than being eagerly hoisted.
            return expression.clone();
        };
        let span = expression.span;
        let mut value = self.lower_value(target);
        // A source handle leaves a local sum available for subsequent reads.
        // SumTake consumes only this snapshot, including on a view parameter.
        if snapshotable_local(self.types, target) {
            value = snapshot_local(target, value);
        }
        let source = self.temporary(target.ty, span);
        self.push(
            StatementKind::Let {
                local: source,
                value,
            },
            span,
        );
        let tag = self.temporary(TypeInterner::BOOL, span);
        self.push(
            StatementKind::SumTag {
                source,
                target: tag,
            },
            span,
        );
        let success = self.new_block(span);
        let failed = self.new_block(span);
        let continuation = self.new_block(span);
        let output = self.temporary(expression.ty, span);
        self.terminate(
            TerminatorKind::Branch {
                condition: Expression {
                    kind: ExpressionKind::Local(tag),
                    ty: TypeInterner::BOOL,
                    span,
                },
                then_block: success,
                else_block: failed,
            },
            span,
        );
        self.current = success;
        self.push(
            StatementKind::SumTake {
                source,
                target: output,
                success: true,
            },
            span,
        );
        self.close_to(continuation, span);
        self.current = failed;
        if let Some(error) = error_local {
            self.push(
                StatementKind::SumTake {
                    source,
                    target: *error,
                    success: false,
                },
                span,
            );
        }
        self.handlers.push((output, continuation));
        self.lower_block(failure);
        self.handlers.pop();
        // A failure path must yield or exit; never fabricate an initialized
        // payload for an invalid fallthrough.
        if self.open() {
            self.terminate(TerminatorKind::Unreachable, span);
        }
        self.current = continuation;
        Expression {
            kind: ExpressionKind::Local(output),
            ty: expression.ty,
            span,
        }
    }

    fn lower_reflected_field_read(
        &mut self,
        expression: &Expression,
        plans: &[hir::ReflectedFieldPlan],
    ) -> Option<Expression> {
        let ExpressionKind::Intrinsic {
            intrinsic,
            type_arguments,
            args,
            evaluation_order,
            ownership,
            ..
        } = &expression.kind
        else {
            return None;
        };
        let [owner_type, requested] = type_arguments.as_slice() else {
            return None;
        };
        if *requested != expression.ty {
            return None;
        }
        hir::validate_reflected_field_plans(self.types, *intrinsic, *owner_type, *requested, plans)
            .ok()?;
        let layout = hir::reflected_field_layout(self.types, *intrinsic, *owner_type)?;
        let metadata_type = args.get(1)?.ty;
        if !hir::valid_reflected_field_metadata_type(self.types, metadata_type) {
            return None;
        }
        let Type::Struct(metadata_id) = self.types.resolve(metadata_type) else {
            return None;
        };
        let metadata = self.types.resolve_struct(*metadata_id);
        if args.len() != layout.len() + 2
            || args[0].ty != *owner_type
            || args[1..].iter().any(|arg| arg.ty != metadata_type)
        {
            return None;
        }
        let member_type = metadata.fields[2].1;
        if plans
            .iter()
            .all(|plan| matches!(plan.action, hir::ReflectedFieldAction::Exact))
        {
            let mut raw = expression.clone();
            if let ExpressionKind::Intrinsic {
                field_validation, ..
            } = &mut raw.kind
            {
                *field_validation = Some(hir::ReflectedFieldValidation::Read);
            }
            return Some(self.lower_value(&raw));
        }
        let inputs: Vec<_> = args
            .iter()
            .enumerate()
            .map(|(index, arg)| {
                if ownership.parameter_physical_access(index)
                    == Some(jett_typecheck::CheckedCalleeAccess::View)
                    && !matches!(arg.kind, ExpressionKind::View(_))
                {
                    Expression {
                        kind: ExpressionKind::View(Box::new(arg.clone())),
                        ty: arg.ty,
                        span: arg.span,
                    }
                } else {
                    arg.clone()
                }
            })
            .collect();
        let mut ownership = ownership.clone();
        let args = self.lower_ordered_call_values(&inputs, evaluation_order, &mut ownership)?;
        let mut raw = expression.clone();
        if let ExpressionKind::Intrinsic {
            field_validation,
            args: raw_args,
            ownership: raw_ownership,
            ..
        } = &mut raw.kind
        {
            *field_validation = Some(hir::ReflectedFieldValidation::Read);
            *raw_args = args.clone();
            *raw_ownership = ownership;
        }
        let span = expression.span;
        let candidate = self.temporary(expression.ty, span);
        // The existing raw getter completes every selector, metadata, owner,
        // pending and payload read check before any proof predicate executes.
        self.push(
            StatementKind::Let {
                local: candidate,
                value: raw,
            },
            span,
        );
        let selected_member = if layout.iter().any(|(member, _, _)| member.is_some()) {
            let member = self.temporary(member_type, span);
            self.push(
                StatementKind::Let {
                    local: member,
                    value: Expression {
                        kind: ExpressionKind::Field {
                            base: Box::new(args[1].clone()),
                            owner_type: metadata_type,
                            field: hir::FieldId::new(2),
                        },
                        ty: member_type,
                        span,
                    },
                },
                span,
            );
            let present = self.temporary(TypeInterner::BOOL, span);
            self.push(
                StatementKind::SumTag {
                    source: member,
                    target: present,
                },
                span,
            );
            let accepted = self.new_block(span);
            let absent = self.new_block(span);
            self.terminate(
                TerminatorKind::Branch {
                    condition: Expression {
                        kind: ExpressionKind::Local(present),
                        ty: TypeInterner::BOOL,
                        span,
                    },
                    then_block: accepted,
                    else_block: absent,
                },
                span,
            );
            self.current = absent;
            self.terminate(TerminatorKind::Unreachable, span);
            self.current = accepted;
            let text = self.temporary(TypeInterner::STRING, span);
            self.push(
                StatementKind::SumTake {
                    source: member,
                    target: text,
                    success: true,
                },
                span,
            );
            Some(text)
        } else {
            None
        };
        let continuation = self.new_block(span);
        for ((member, index, _), plan) in layout.iter().zip(plans) {
            let case = self.new_block(span);
            let next = self.new_block(span);
            let field = |index, ty| Expression {
                kind: ExpressionKind::Field {
                    base: Box::new(args[1].clone()),
                    owner_type: metadata_type,
                    field: hir::FieldId::new(index),
                },
                ty,
                span,
            };
            let index_matches = Expression {
                kind: ExpressionKind::Binary {
                    left: Box::new(field(0, TypeInterner::INT64)),
                    op: hir::BinaryOp::Equal,
                    right: Box::new(Expression {
                        kind: ExpressionKind::Int(*index as i128),
                        ty: TypeInterner::INT64,
                        span,
                    }),
                },
                ty: TypeInterner::BOOL,
                span,
            };
            let condition = if let Some(member) = member {
                let member_matches = Expression {
                    kind: ExpressionKind::Binary {
                        left: Box::new(Expression {
                            kind: ExpressionKind::View(Box::new(Expression {
                                kind: ExpressionKind::Local(selected_member?),
                                ty: TypeInterner::STRING,
                                span,
                            })),
                            ty: TypeInterner::STRING,
                            span,
                        }),
                        op: hir::BinaryOp::Equal,
                        right: Box::new(Expression {
                            kind: ExpressionKind::String(member.clone()),
                            ty: TypeInterner::STRING,
                            span,
                        }),
                    },
                    ty: TypeInterner::BOOL,
                    span,
                };
                Expression {
                    kind: ExpressionKind::Binary {
                        left: Box::new(index_matches),
                        op: hir::BinaryOp::And,
                        right: Box::new(member_matches),
                    },
                    ty: TypeInterner::BOOL,
                    span,
                }
            } else {
                index_matches
            };
            self.terminate(
                TerminatorKind::Branch {
                    condition,
                    then_block: case,
                    else_block: next,
                },
                span,
            );
            self.current = case;
            self.lower_reflected_plan(candidate, plan, span);
            self.close_to(continuation, span);
            self.current = next;
        }
        // A successful getter already proved one of these exact member/index
        // pairs. Never fabricate a result if a malformed plan misses that pair.
        self.terminate(TerminatorKind::Unreachable, span);
        self.current = continuation;
        Some(self.finish_call_view_scope(Expression {
            kind: ExpressionKind::Local(candidate),
            ty: expression.ty,
            span,
        }))
    }

    fn lower_refinement_check(
        &mut self,
        input: Expression,
        predicate: &hir::RefinementPredicate,
        span: Span,
    ) -> (LocalId, LocalId) {
        let args = vec![input];
        let ownership = hir::CallOwnership::generated(
            hir::GeneratedOperation::RefinementPredicate {
                function: predicate.function,
            },
            &args,
            &[predicate.input_type],
            &[jett_typecheck::CheckedCalleeAccess::Owned],
            TypeInterner::BOOL,
            &[0],
            self.types,
        )
        .expect("checked refinement predicate has a unary owned input");
        let error_text = self.temporary(TypeInterner::STRING, span);
        self.push(
            StatementKind::CheckRefinement {
                local: error_text,
                call: Expression {
                    kind: ExpressionKind::Call {
                        function: predicate.function,
                        args,
                        evaluation_order: vec![0],
                        ownership,
                    },
                    ty: TypeInterner::BOOL,
                    span,
                },
                type_name: predicate.type_name.clone(),
            },
            span,
        );
        let passed = self.temporary(TypeInterner::BOOL, span);
        self.push(
            StatementKind::Let {
                local: passed,
                value: Expression {
                    kind: ExpressionKind::Binary {
                        left: Box::new(Expression {
                            kind: ExpressionKind::View(Box::new(Expression {
                                kind: ExpressionKind::Local(error_text),
                                ty: TypeInterner::STRING,
                                span,
                            })),
                            ty: TypeInterner::STRING,
                            span,
                        }),
                        op: hir::BinaryOp::Equal,
                        right: Box::new(Expression {
                            kind: ExpressionKind::String(String::new()),
                            ty: TypeInterner::STRING,
                            span,
                        }),
                    },
                    ty: TypeInterner::BOOL,
                    span,
                },
            },
            span,
        );
        (error_text, passed)
    }

    fn lower_refinement_handle(
        &mut self,
        expression: &Expression,
        target: &Expression,
        predicates: &[hir::RefinementPredicate],
        error_local: Option<LocalId>,
        failure: &hir::Block,
    ) -> Expression {
        if predicates.is_empty() {
            return expression.clone();
        }
        let span = expression.span;
        let value = self.lower_value(target);
        // Refinement validation reads a cloneable local without consuming the
        // source value in the interpreter. Keep the candidate in its own owner.
        let value = if snapshotable_local(self.types, target) {
            snapshot_local(target, value)
        } else {
            value
        };
        let source = self.temporary(target.ty, span);
        self.push(
            StatementKind::Let {
                local: source,
                value,
            },
            span,
        );
        let failed = self.new_block(failure.span);
        let continuation = self.new_block(span);
        let output = self.temporary(expression.ty, span);

        for predicate in predicates {
            let input = self.refinement_predicate_input(source, target.ty, predicate, span);
            let (error_text, passed) = self.lower_refinement_check(input, predicate, span);
            let next = self.new_block(span);
            let rejected = self.new_block(span);
            self.terminate(
                TerminatorKind::Branch {
                    condition: Expression {
                        kind: ExpressionKind::Local(passed),
                        ty: TypeInterner::BOOL,
                        span,
                    },
                    then_block: next,
                    else_block: rejected,
                },
                span,
            );
            self.current = rejected;
            if let Some(error_local) = error_local {
                self.push(
                    StatementKind::Let {
                        local: error_local,
                        value: Expression {
                            kind: ExpressionKind::Local(error_text),
                            ty: TypeInterner::STRING,
                            span,
                        },
                    },
                    span,
                );
            }
            self.terminate(TerminatorKind::Goto(failed), span);
            self.current = next;
        }

        self.push(
            StatementKind::Let {
                local: output,
                value: Expression {
                    kind: ExpressionKind::RefinementValidated(Box::new(Expression {
                        kind: ExpressionKind::Local(source),
                        ty: target.ty,
                        span,
                    })),
                    ty: expression.ty,
                    span,
                },
            },
            span,
        );
        self.close_to(continuation, span);
        self.current = failed;
        self.handlers.push((output, continuation));
        self.lower_block(failure);
        self.handlers.pop();
        if self.open() {
            self.terminate(TerminatorKind::Unreachable, span);
        }
        self.current = continuation;
        Expression {
            kind: ExpressionKind::Local(output),
            ty: expression.ty,
            span,
        }
    }

    fn lower_refinement_enum_builder_finish(
        &mut self,
        expression: &Expression,
        enum_type: TypeId,
        refinement_predicates: &[Vec<hir::RefinementPredicate>],
    ) -> Expression {
        let Type::Enum(id) = self.types.resolve(enum_type) else {
            return expression.clone();
        };
        let variants = self
            .types
            .resolve_enum(*id)
            .variants
            .iter()
            .map(|variant| variant.fields.iter().map(|(_, ty)| *ty).collect::<Vec<_>>())
            .collect::<Vec<_>>();
        if variants.iter().map(Vec::len).sum::<usize>() != refinement_predicates.len() {
            return expression.clone();
        }
        let span = expression.span;
        let mut raw = expression.clone();
        if let ExpressionKind::Intrinsic {
            refinement_predicates,
            ..
        } = &mut raw.kind
        {
            refinement_predicates.clear();
        }
        let raw = self.lower_value(&raw);
        let source = self.temporary(expression.ty, span);
        self.push(
            StatementKind::Let {
                local: source,
                value: raw,
            },
            span,
        );
        let tag = self.temporary(TypeInterner::BOOL, span);
        self.push(
            StatementKind::SumTag {
                source,
                target: tag,
            },
            span,
        );
        let accepted = self.new_block(span);
        let failed = self.new_block(span);
        let validated = self.new_block(span);
        let continuation = self.new_block(span);
        let output = self.temporary(expression.ty, span);
        self.terminate(
            TerminatorKind::Branch {
                condition: Expression {
                    kind: ExpressionKind::Local(tag),
                    ty: TypeInterner::BOOL,
                    span,
                },
                then_block: accepted,
                else_block: failed,
            },
            span,
        );
        self.current = failed;
        self.push(
            StatementKind::Let {
                local: output,
                value: Expression {
                    kind: ExpressionKind::Local(source),
                    ty: expression.ty,
                    span,
                },
            },
            span,
        );
        self.close_to(continuation, span);
        self.current = accepted;
        let built = self.temporary(enum_type, span);
        self.push(
            StatementKind::SumTake {
                source,
                target: built,
                success: true,
            },
            span,
        );
        let mut offset = 0;
        let mut arms = Vec::with_capacity(variants.len());
        for (index, fields) in variants.iter().enumerate() {
            let block = self.new_block(span);
            let has_predicates = refinement_predicates[offset..offset + fields.len()]
                .iter()
                .any(|chain| !chain.is_empty());
            let bindings = if has_predicates {
                fields
                    .iter()
                    .map(|ty| self.temporary(*ty, span))
                    .collect::<Vec<_>>()
            } else {
                Vec::new()
            };
            arms.push((
                hir::VariantId::new(index as u32),
                block,
                bindings,
                offset,
                index,
            ));
            offset += fields.len();
        }
        self.terminate(
            TerminatorKind::Switch {
                scrutinee: Expression {
                    kind: ExpressionKind::Clone(Box::new(Expression {
                        kind: ExpressionKind::Local(built),
                        ty: enum_type,
                        span,
                    })),
                    ty: enum_type,
                    span,
                },
                variants: arms
                    .iter()
                    .map(|(variant, block, bindings, _, _)| (*variant, *block, bindings.clone()))
                    .collect(),
                otherwise: None,
            },
            span,
        );
        for (_, block, bindings, offset, variant_index) in arms {
            self.current = block;
            for (index, binding) in bindings.iter().enumerate() {
                for predicate in &refinement_predicates[offset + index] {
                    let input = self.refinement_predicate_input(
                        *binding,
                        variants[variant_index][index],
                        predicate,
                        span,
                    );
                    let (error_text, passed) = self.lower_refinement_check(input, predicate, span);
                    let next = self.new_block(span);
                    let rejected = self.new_block(span);
                    self.terminate(
                        TerminatorKind::Branch {
                            condition: Expression {
                                kind: ExpressionKind::Local(passed),
                                ty: TypeInterner::BOOL,
                                span,
                            },
                            then_block: next,
                            else_block: rejected,
                        },
                        span,
                    );
                    self.current = rejected;
                    self.push(
                        StatementKind::Let {
                            local: output,
                            value: Expression {
                                kind: ExpressionKind::ResultFail(Box::new(Expression {
                                    kind: ExpressionKind::Local(error_text),
                                    ty: TypeInterner::STRING,
                                    span,
                                })),
                                ty: expression.ty,
                                span,
                            },
                        },
                        span,
                    );
                    self.close_to(continuation, span);
                    self.current = next;
                }
            }
            self.close_to(validated, span);
        }
        self.current = validated;
        self.push(
            StatementKind::Let {
                local: output,
                value: Expression {
                    kind: ExpressionKind::ResultOk(Box::new(Expression {
                        kind: ExpressionKind::Local(built),
                        ty: enum_type,
                        span,
                    })),
                    ty: expression.ty,
                    span,
                },
            },
            span,
        );
        self.close_to(continuation, span);
        self.current = continuation;
        Expression {
            kind: ExpressionKind::Local(output),
            ty: expression.ty,
            span,
        }
    }

    fn lower_refinement_machine_builder_finish(
        &mut self,
        expression: &Expression,
        owner_type: TypeId,
        refinement_predicates: &[Vec<hir::RefinementPredicate>],
    ) -> Expression {
        let (machine, selected_state) = match self.types.resolve(owner_type) {
            Type::Machine(machine) => (*machine, None),
            Type::MachineState { machine, state } => (*machine, Some(*state)),
            _ => return expression.clone(),
        };
        let states =
            self.types
                .resolve_machine(machine)
                .states
                .iter()
                .enumerate()
                .filter(|(index, _)| {
                    selected_state.is_none_or(|state| state.index() as usize == *index)
                })
                .map(|(index, state)| {
                    let narrowed =
                        if selected_state.is_some() {
                            owner_type
                        } else {
                            self.types.type_ids().find(|id| matches!(
                        self.types.resolve(*id),
                        Type::MachineState { machine: id_machine, state }
                            if *id_machine == machine && state.index() as usize == index
                    )).expect("checked reflected finish interns all machine states")
                        };
                    (
                        hir::StateId::new(index as u32),
                        narrowed,
                        state.fields.iter().map(|(_, ty)| *ty).collect::<Vec<_>>(),
                    )
                })
                .collect::<Vec<_>>();
        if states
            .iter()
            .map(|(_, _, fields)| fields.len())
            .sum::<usize>()
            != refinement_predicates.len()
        {
            return expression.clone();
        }
        let span = expression.span;
        let mut raw = expression.clone();
        if let ExpressionKind::Intrinsic {
            refinement_predicates,
            ..
        } = &mut raw.kind
        {
            refinement_predicates.clear();
        }
        let raw = self.lower_value(&raw);
        let source = self.temporary(expression.ty, span);
        self.push(
            StatementKind::Let {
                local: source,
                value: raw,
            },
            span,
        );
        let tag = self.temporary(TypeInterner::BOOL, span);
        self.push(
            StatementKind::SumTag {
                source,
                target: tag,
            },
            span,
        );
        let accepted = self.new_block(span);
        let failed = self.new_block(span);
        let validated = self.new_block(span);
        let continuation = self.new_block(span);
        let output = self.temporary(expression.ty, span);
        self.terminate(
            TerminatorKind::Branch {
                condition: Expression {
                    kind: ExpressionKind::Local(tag),
                    ty: TypeInterner::BOOL,
                    span,
                },
                then_block: accepted,
                else_block: failed,
            },
            span,
        );
        self.current = failed;
        self.push(
            StatementKind::Let {
                local: output,
                value: Expression {
                    kind: ExpressionKind::Local(source),
                    ty: expression.ty,
                    span,
                },
            },
            span,
        );
        self.close_to(continuation, span);
        self.current = accepted;
        let built = self.temporary(owner_type, span);
        self.push(
            StatementKind::SumTake {
                source,
                target: built,
                success: true,
            },
            span,
        );
        let mut offset = 0;
        for (state, narrowed_type, fields) in states {
            let selected = self.new_block(span);
            let next_state = self.new_block(span);
            self.terminate(
                TerminatorKind::Branch {
                    condition: Expression {
                        kind: ExpressionKind::StateIs {
                            value: Box::new(Expression {
                                kind: ExpressionKind::Local(built),
                                ty: owner_type,
                                span,
                            }),
                            state,
                        },
                        ty: TypeInterner::BOOL,
                        span,
                    },
                    then_block: selected,
                    else_block: next_state,
                },
                span,
            );
            self.current = selected;
            for (index, field_type) in fields.iter().enumerate() {
                for predicate in &refinement_predicates[offset + index] {
                    let field = Expression {
                        kind: ExpressionKind::Field {
                            base: Box::new(Expression {
                                kind: ExpressionKind::Local(built),
                                ty: narrowed_type,
                                span,
                            }),
                            owner_type: narrowed_type,
                            field: hir::FieldId::new(index as u32),
                        },
                        ty: *field_type,
                        span,
                    };
                    let value = self.temporary(*field_type, span);
                    self.push(
                        StatementKind::Let {
                            local: value,
                            value: Expression {
                                kind: ExpressionKind::Clone(Box::new(field)),
                                ty: *field_type,
                                span,
                            },
                        },
                        span,
                    );
                    let input =
                        self.refinement_predicate_input(value, *field_type, predicate, span);
                    let (error_text, passed) = self.lower_refinement_check(input, predicate, span);
                    let next = self.new_block(span);
                    let rejected = self.new_block(span);
                    self.terminate(
                        TerminatorKind::Branch {
                            condition: Expression {
                                kind: ExpressionKind::Local(passed),
                                ty: TypeInterner::BOOL,
                                span,
                            },
                            then_block: next,
                            else_block: rejected,
                        },
                        span,
                    );
                    self.current = rejected;
                    self.push(
                        StatementKind::Let {
                            local: output,
                            value: Expression {
                                kind: ExpressionKind::ResultFail(Box::new(Expression {
                                    kind: ExpressionKind::Local(error_text),
                                    ty: TypeInterner::STRING,
                                    span,
                                })),
                                ty: expression.ty,
                                span,
                            },
                        },
                        span,
                    );
                    self.close_to(continuation, span);
                    self.current = next;
                }
            }
            self.close_to(validated, span);
            self.current = next_state;
            offset += fields.len();
        }
        self.terminate(TerminatorKind::Unreachable, span);
        self.current = validated;
        self.push(
            StatementKind::Let {
                local: output,
                value: Expression {
                    kind: ExpressionKind::ResultOk(Box::new(Expression {
                        kind: ExpressionKind::Local(built),
                        ty: owner_type,
                        span,
                    })),
                    ty: expression.ty,
                    span,
                },
            },
            span,
        );
        self.close_to(continuation, span);
        self.current = continuation;
        Expression {
            kind: ExpressionKind::Local(output),
            ty: expression.ty,
            span,
        }
    }

    fn lower_refinement_builder_finish(
        &mut self,
        expression: &Expression,
        refinement_predicates: &[Vec<hir::RefinementPredicate>],
    ) -> Expression {
        let Type::Result(struct_type, _) = self.types.resolve(expression.ty) else {
            return expression.clone();
        };
        let struct_type = *struct_type;
        if matches!(self.types.resolve(struct_type), Type::Enum(_)) {
            return self.lower_refinement_enum_builder_finish(
                expression,
                struct_type,
                refinement_predicates,
            );
        }
        if matches!(
            self.types.resolve(struct_type),
            Type::Machine(_) | Type::MachineState { .. }
        ) {
            return self.lower_refinement_machine_builder_finish(
                expression,
                struct_type,
                refinement_predicates,
            );
        }
        let Type::Struct(id) = self.types.resolve(struct_type) else {
            return expression.clone();
        };
        let field_types = self
            .types
            .resolve_struct(*id)
            .fields
            .iter()
            .map(|(_, ty)| *ty)
            .collect::<Vec<_>>();
        if field_types.len() != refinement_predicates.len() {
            return expression.clone();
        }
        let span = expression.span;
        let mut raw = expression.clone();
        if let ExpressionKind::Intrinsic {
            refinement_predicates,
            ..
        } = &mut raw.kind
        {
            refinement_predicates.clear();
        }
        let raw = self.lower_value(&raw);
        let source = self.temporary(expression.ty, span);
        self.push(
            StatementKind::Let {
                local: source,
                value: raw,
            },
            span,
        );
        let tag = self.temporary(TypeInterner::BOOL, span);
        self.push(
            StatementKind::SumTag {
                source,
                target: tag,
            },
            span,
        );
        let accepted = self.new_block(span);
        let failed = self.new_block(span);
        let continuation = self.new_block(span);
        let output = self.temporary(expression.ty, span);
        self.terminate(
            TerminatorKind::Branch {
                condition: Expression {
                    kind: ExpressionKind::Local(tag),
                    ty: TypeInterner::BOOL,
                    span,
                },
                then_block: accepted,
                else_block: failed,
            },
            span,
        );
        self.current = failed;
        self.push(
            StatementKind::Let {
                local: output,
                value: Expression {
                    kind: ExpressionKind::Local(source),
                    ty: expression.ty,
                    span,
                },
            },
            span,
        );
        self.close_to(continuation, span);
        self.current = accepted;
        let built = self.temporary(struct_type, span);
        self.push(
            StatementKind::SumTake {
                source,
                target: built,
                success: true,
            },
            span,
        );
        for (index, predicates) in refinement_predicates.iter().enumerate() {
            for predicate in predicates {
                let field = Expression {
                    kind: ExpressionKind::Field {
                        base: Box::new(Expression {
                            kind: ExpressionKind::Local(built),
                            ty: struct_type,
                            span,
                        }),
                        owner_type: struct_type,
                        field: hir::FieldId::new(index as u32),
                    },
                    ty: field_types[index],
                    span,
                };
                let value = self.temporary(field.ty, span);
                self.push(
                    StatementKind::Let {
                        local: value,
                        value: Expression {
                            kind: ExpressionKind::Clone(Box::new(field)),
                            ty: field_types[index],
                            span,
                        },
                    },
                    span,
                );
                let input =
                    self.refinement_predicate_input(value, field_types[index], predicate, span);
                let (error_text, passed) = self.lower_refinement_check(input, predicate, span);
                let next = self.new_block(span);
                let rejected = self.new_block(span);
                self.terminate(
                    TerminatorKind::Branch {
                        condition: Expression {
                            kind: ExpressionKind::Local(passed),
                            ty: TypeInterner::BOOL,
                            span,
                        },
                        then_block: next,
                        else_block: rejected,
                    },
                    span,
                );
                self.current = rejected;
                self.push(
                    StatementKind::Let {
                        local: output,
                        value: Expression {
                            kind: ExpressionKind::ResultFail(Box::new(Expression {
                                kind: ExpressionKind::Local(error_text),
                                ty: TypeInterner::STRING,
                                span,
                            })),
                            ty: expression.ty,
                            span,
                        },
                    },
                    span,
                );
                self.close_to(continuation, span);
                self.current = next;
            }
        }
        self.push(
            StatementKind::Let {
                local: output,
                value: Expression {
                    kind: ExpressionKind::ResultOk(Box::new(Expression {
                        kind: ExpressionKind::Local(built),
                        ty: struct_type,
                        span,
                    })),
                    ty: expression.ty,
                    span,
                },
            },
            span,
        );
        self.close_to(continuation, span);
        self.current = continuation;
        Expression {
            kind: ExpressionKind::Local(output),
            ty: expression.ty,
            span,
        }
    }

    fn lower_refinement_struct_construct(
        &mut self,
        expression: &Expression,
        struct_type: TypeId,
        fields: &[Expression],
        evaluation_order: &[usize],
        refinement_predicates: &[Vec<hir::RefinementPredicate>],
    ) -> Expression {
        if evaluation_order.len() != fields.len()
            || evaluation_order.iter().any(|&index| index >= fields.len())
        {
            return expression.clone();
        }
        let span = expression.span;
        let continuation = self.new_block(span);
        let output = self.temporary(expression.ty, span);
        let mut evaluated = vec![None; fields.len()];
        for &index in evaluation_order {
            let value = self.lower_value(&fields[index]);
            let local = self.temporary(value.ty, value.span);
            self.push(StatementKind::Let { local, value }, span);
            evaluated[index] = Some(local);
        }
        let mut prepared = vec![None; fields.len()];
        for &index in evaluation_order {
            let source = evaluated[index].expect("validated field evaluation order");
            let field = &fields[index];
            let mut field_type = field.ty;
            for predicate in &refinement_predicates[index] {
                let input = self.refinement_predicate_input(source, field.ty, predicate, span);
                let (error_text, passed) = self.lower_refinement_check(input, predicate, span);
                let next = self.new_block(span);
                let rejected = self.new_block(span);
                self.terminate(
                    TerminatorKind::Branch {
                        condition: Expression {
                            kind: ExpressionKind::Local(passed),
                            ty: TypeInterner::BOOL,
                            span,
                        },
                        then_block: next,
                        else_block: rejected,
                    },
                    span,
                );
                self.current = rejected;
                self.push(
                    StatementKind::Let {
                        local: output,
                        value: Expression {
                            kind: ExpressionKind::ResultFail(Box::new(Expression {
                                kind: ExpressionKind::Local(error_text),
                                ty: TypeInterner::STRING,
                                span,
                            })),
                            ty: expression.ty,
                            span,
                        },
                    },
                    span,
                );
                self.close_to(continuation, span);
                self.current = next;
                field_type = predicate.refined_type;
            }
            let field_value = if field_type != field.ty {
                Expression {
                    kind: ExpressionKind::RefinementValidated(Box::new(Expression {
                        kind: ExpressionKind::Local(source),
                        ty: field.ty,
                        span,
                    })),
                    ty: field_type,
                    span,
                }
            } else {
                Expression {
                    kind: ExpressionKind::Local(source),
                    ty: field.ty,
                    span,
                }
            };
            prepared[index] = Some(field_value);
        }
        let fields = prepared
            .into_iter()
            .map(|value| value.expect("validated field evaluation order"))
            .collect();
        self.push(
            StatementKind::Let {
                local: output,
                value: Expression {
                    kind: ExpressionKind::ResultOk(Box::new(Expression {
                        kind: ExpressionKind::StructConstruct {
                            struct_type,
                            fields,
                            evaluation_order: (0..evaluation_order.len()).collect(),
                            validates_refinements: false,
                            refinement_predicates: Vec::new(),
                        },
                        ty: struct_type,
                        span,
                    })),
                    ty: expression.ty,
                    span,
                },
            },
            span,
        );
        self.close_to(continuation, span);
        self.current = continuation;
        Expression {
            kind: ExpressionKind::Local(output),
            ty: expression.ty,
            span,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    mod reflected_recursive;

    fn handler_source_hir(source: &str) -> (hir::Program, TypeInterner) {
        handler_source_hir_with_equatable(source, false)
    }

    fn handler_source_hir_with_equatable(
        source: &str,
        include_equatable: bool,
    ) -> (hir::Program, TypeInterner) {
        let file = jett_common::FileId::new(0);
        let mut parsed = jett_parser::parse(source, file);
        assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
        let mut origins =
            std::collections::HashMap::from([(file, jett_common::SourceOrigin::Project)]);
        if include_equatable {
            let prelude_file = jett_common::FileId::new(jett_common::STDLIB_FILE_ID_START);
            let mut prelude = jett_parser::parse(
                "namespace stdlib\nexport interface Equatable:\n    function equals(view self: Equatable, view other: Equatable) returns bool\n",
                prelude_file,
            );
            assert!(prelude.errors.is_empty());
            prelude.module.items.append(&mut parsed.module.items);
            parsed.module.items = prelude.module.items;
            origins.insert(prelude_file, jett_common::SourceOrigin::Stdlib);
        }
        let resolved = jett_resolve::resolve(&parsed.module);
        let checked = jett_typecheck::check(&parsed.module, &resolved);
        assert!(
            checked
                .diagnostics
                .iter()
                .all(|diagnostic| { diagnostic.severity != jett_diagnostics::Severity::Error }),
            "{:?}",
            checked.diagnostics
        );
        let hir =
            jett_hir::lower(&parsed.module, &resolved, &checked, &origins).expect("HIR lowering");
        (hir, checked.interner)
    }

    fn lower_handler_source(source: &str) -> (Program, TypeInterner) {
        let (hir, types) = handler_source_hir(source);
        (lower(&hir, &types).expect("MIR lowering"), types)
    }

    fn inspected_handler_function(program: &Program) -> &Function {
        program
            .functions
            .iter()
            .find(|function| function.identity.declaration.name == "inspect")
            .unwrap()
    }

    #[test]
    fn failed_checked_call_staging_restores_all_builder_state() {
        let source = "function take(view first: list[int64], view second: list[int64]) returns int64:\n    return 7\nfunction exercise(first: list[int64], second: list[int64]) returns int64:\n    return take(first, second)\n";
        let (program, types) = handler_source_hir(source);
        let function = program
            .functions
            .iter()
            .find(|function| function.identity.declaration.name == "exercise")
            .unwrap();
        let call = function
            .body
            .statements
            .iter()
            .find_map(|statement| match &statement.kind {
                hir::StatementKind::Return(Some(value)) => Some(value),
                _ => None,
            })
            .unwrap();
        let ExpressionKind::Call {
            function: target,
            args,
            evaluation_order,
            ownership,
        } = &call.kind
        else {
            panic!("checked source invocation");
        };
        let modes = program
            .functions
            .iter()
            .map(|function| {
                (
                    function.id,
                    function
                        .params
                        .iter()
                        .map(|param| param.mode)
                        .collect::<Vec<_>>(),
                )
            })
            .collect::<std::collections::HashMap<_, _>>();
        let mut inputs = call_staging_inputs(args, &modes[target]).unwrap();
        // The first argument will already have acquired an owner and loan
        // when the later malformed physical operand fails its stage proof.
        inputs[1] = args[1].clone();
        let mut builder = Builder::new(function.span, &types, &modes);
        builder.locals = function.locals.clone();
        let previous = builder.clone();
        let mut packet = ownership.clone();
        assert!(
            builder
                .lower_ordered_call_values(&inputs, evaluation_order, &mut packet)
                .is_none()
        );
        assert_eq!(packet, *ownership);
        assert_eq!(builder.blocks, previous.blocks);
        assert_eq!(builder.current, previous.current);
        assert_eq!(builder.locals, previous.locals);
        assert_eq!(builder.loops, previous.loops);
        assert_eq!(builder.handlers, previous.handlers);
        assert_eq!(builder.view_params, previous.view_params);
        assert_eq!(builder.call_view_scopes, previous.call_view_scopes);
    }

    #[test]
    fn native_reflected_root_reads_run_selector_guards_before_slot_predicates() {
        for (owner, getter, declaration, expected) in [
            (
                "Record",
                "field_value",
                "struct Record:\n    raw: int64\n    positive: Positive\n    higher: Higher\n    sibling: Sibling\n",
                5,
            ),
            (
                "Event",
                "variant_field_value",
                "enum Event:\n    first(raw: int64, positive: Positive, higher: Higher, sibling: Sibling)\n    second(higher: Higher, raw: int64)\n",
                7,
            ),
            (
                "Session",
                "machine_field_value",
                "machine Session:\n    states:\n        first(raw: int64, positive: Positive, higher: Higher, sibling: Sibling)\n        second(higher: Higher, raw: int64)\n    transitions:\n        first to second\n",
                7,
            ),
        ] {
            let source = format!(
                "namespace app\ntype Positive = int64 where value > 0\ntype Higher = Positive where value > 10\ntype Sibling = int64 where value >= 0\n{declaration}function inspect(view source: {owner}, view field: TypeField) returns Higher:\n    return type.{getter}[{owner}, Higher](view source, view field)\n"
            );
            let (program, types) = lower_handler_source(&source);
            validate(&program).unwrap();
            let function = inspected_handler_function(&program);
            let raw_reads = function
                .blocks
                .iter()
                .flat_map(|b| &b.statements)
                .filter_map(|s| match &s.kind {
                    StatementKind::Let {
                        local,
                        value:
                            Expression {
                                kind:
                                    ExpressionKind::Intrinsic {
                                        intrinsic,
                                        field_validation: Some(hir::ReflectedFieldValidation::Read),
                                        ..
                                    },
                                ..
                            },
                    } if hir::is_reflected_field_intrinsic(*intrinsic) => Some(*local),
                    _ => None,
                })
                .collect::<Vec<_>>();
            assert_eq!(raw_reads.len(), 1, "{owner}: getter is staged once");
            let entry = &function.blocks[function.entry.index() as usize];
            assert!(entry.statements.iter().any(
                |s| matches!(s.kind, StatementKind::Let { local, .. } if local == raw_reads[0])
            ));
            assert!(
                !entry
                    .statements
                    .iter()
                    .any(|s| matches!(s.kind, StatementKind::CheckRefinement { .. })),
                "raw guard runs before predicates"
            );
            let checks = function
                .blocks
                .iter()
                .flat_map(|b| &b.statements)
                .filter(|s| matches!(s.kind, StatementKind::CheckRefinement { .. }))
                .count();
            assert_eq!(
                checks, expected,
                "{owner}: exact slot skips, ancestor suffix only, sibling full chain"
            );
            let members = function
                .blocks
                .iter()
                .flat_map(|b| &b.statements)
                .filter(|s| matches!(s.kind, StatementKind::SumTake { .. }))
                .count();
            assert_eq!(
                members,
                usize::from(owner != "Record"),
                "only enum/machine extract the validated optional member"
            );
            for block in &function.blocks {
                if let TerminatorKind::Branch {
                    condition:
                        Expression {
                            kind:
                                ExpressionKind::Binary {
                                    left,
                                    op: hir::BinaryOp::And,
                                    right,
                                },
                            ..
                        },
                    ..
                } = &block.terminator.kind
                {
                    assert_eq!(left.ty, TypeInterner::BOOL);
                    let ExpressionKind::Binary {
                        left,
                        op: hir::BinaryOp::Equal,
                        right,
                    } = &right.kind
                    else {
                        panic!("canonical member condition");
                    };
                    assert_eq!(left.ty, TypeInterner::STRING);
                    assert_eq!(right.ty, TypeInterner::STRING);
                }
            }
            crate::move_values::MoveValuePlan::analyze(&program, function, &types).unwrap();
        }
    }

    #[test]
    fn native_reflected_root_owned_reads_keep_candidate_and_error_cleanup() {
        let (program, types) = lower_handler_source(
            r#"namespace app
type Nonempty = list[int64] where true
struct Parcel:
    exact: Nonempty
    raw: list[int64]
function inspect(view source: Parcel, view field: TypeField) returns Nonempty:
    return type.field_value[Parcel, Nonempty](view source, view field)
"#,
        );
        validate(&program).unwrap();
        let function = inspected_handler_function(&program);
        let plan = crate::move_values::MoveValuePlan::analyze(&program, function, &types).unwrap();
        let candidate = function
            .blocks
            .iter()
            .flat_map(|b| &b.statements)
            .find_map(|s| match &s.kind {
                StatementKind::Let {
                    local,
                    value:
                        Expression {
                            kind:
                                ExpressionKind::Intrinsic {
                                    field_validation: Some(hir::ReflectedFieldValidation::Read),
                                    ..
                                },
                            ..
                        },
                } => Some(*local),
                _ => None,
            })
            .unwrap();
        assert!(
            plan.owned_locals.contains(&(candidate.index() as usize)),
            "returned clone belongs to this function"
        );
        let checks = function
            .blocks
            .iter()
            .flat_map(|b| &b.statements)
            .filter_map(|s| match &s.kind {
                StatementKind::CheckRefinement {
                    local, type_name, ..
                } => Some((*local, type_name.as_str())),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(
            checks.len(),
            1,
            "exact declared list refinement skips its predicate"
        );
        assert_eq!(checks[0].1, "app.Nonempty");
        let (block, message) = function
            .blocks
            .iter()
            .enumerate()
            .find_map(|(index, b)| {
                b.statements.iter().find_map(|s| match &s.kind {
                    StatementKind::Evaluate(Expression {
                        kind: ExpressionKind::RuntimeFailureMessage(message),
                        ..
                    }) => Some((index, message)),
                    _ => None,
                })
            })
            .unwrap();
        assert!(matches!(message.kind, ExpressionKind::Local(local) if local == checks[0].0));
        assert!(plan.owned_locals.contains(&(checks[0].0.index() as usize)));
        assert!(plan.live_in[block].contains(&(checks[0].0.index() as usize)));
        assert!(matches!(
            function.blocks[block].terminator.kind,
            TerminatorKind::Unreachable
        ));
    }

    #[test]
    fn native_reflected_root_mir_rejects_unlowered_and_wrong_predicate_contracts() {
        let source = r#"namespace app
type Positive = int64 where value > 0
type Sibling = int64 where value >= 0
struct Record:
    raw: int64
function inspect(view source: Record, view field: TypeField) returns Positive:
    return type.field_value[Record, Positive](view source, view field)
"#;
        let (original, types) = lower_handler_source(source);
        let sibling = original
            .functions
            .iter()
            .find(|f| f.identity.declaration.name == "Sibling")
            .unwrap()
            .id;
        for mutation in 0..3 {
            let mut program = original.clone();
            let function = program
                .functions
                .iter_mut()
                .find(|f| f.identity.declaration.name == "inspect")
                .unwrap();
            if mutation < 2 {
                let value = function
                    .blocks
                    .iter_mut()
                    .flat_map(|b| &mut b.statements)
                    .find_map(|s| match &mut s.kind {
                        StatementKind::Let {
                            value:
                                Expression {
                                    kind:
                                        ExpressionKind::Intrinsic {
                                            field_validation, ..
                                        },
                                    ..
                                },
                            ..
                        } => Some(field_validation),
                        _ => None,
                    })
                    .unwrap();
                *value = if mutation == 0 {
                    None
                } else {
                    Some(hir::ReflectedFieldValidation::Validate(Vec::new()))
                };
            } else {
                let call = function
                    .blocks
                    .iter_mut()
                    .flat_map(|b| &mut b.statements)
                    .find_map(|s| match &mut s.kind {
                        StatementKind::CheckRefinement { call, .. } => Some(call),
                        _ => None,
                    })
                    .unwrap();
                let ExpressionKind::Call { function, .. } = &mut call.kind else {
                    panic!("predicate call");
                };
                *function = sibling;
            }
            let errors = validate(&program).expect_err("malformed reflected MIR");
            assert!(errors.iter().any(|e| e.message.contains(if mutation < 2 {
                "proof plans were not lowered"
            } else {
                "exact predicate declaration"
            })));
        }
        let (mut hir, _) = handler_source_hir(source);
        let function = hir
            .functions
            .iter_mut()
            .find(|f| f.identity.declaration.name == "inspect")
            .unwrap();
        let hir::StatementKind::Return(Some(Expression {
            kind:
                ExpressionKind::Intrinsic {
                    field_validation: Some(hir::ReflectedFieldValidation::Validate(plans)),
                    ..
                },
            ..
        })) = &mut function.body.statements[0].kind
        else {
            panic!("source plans");
        };
        plans[0].source_type = TypeInterner::BOOL;
        let errors = lower(&hir, &types)
            .expect_err("malformed source type plan cannot become unchecked Read");
        assert!(
            errors.iter().any(|error| error
                .message
                .contains("reflected validation types disagree with declared field and request")),
            "{errors:?}"
        );
    }

    #[test]
    fn native_return_refinement_stages_once_and_forwards_the_original_error() {
        let (program, types) = lower_handler_source(
            r#"namespace app
type Positive = int64 where value > 0
type Higher = Positive where value > 10
function source(raw: int64) returns int64:
    return run raw
function inspect(raw: int64) returns Higher:
    return source(raw)
"#,
        );
        validate(&program).unwrap();
        let function = inspected_handler_function(&program);
        let checks = function
            .blocks
            .iter()
            .flat_map(|block| &block.statements)
            .filter_map(|statement| {
                if let StatementKind::CheckRefinement { type_name, .. } = &statement.kind {
                    Some(type_name.as_str())
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();
        assert_eq!(checks, ["app.Positive", "app.Higher"]);
        let candidate_calls = function.blocks.iter().flat_map(|block| &block.statements).filter(|statement|
            matches!(&statement.kind, StatementKind::Let { value: Expression { kind: ExpressionKind::Call { function: target, .. }, .. }, .. }
                if program.functions[target.index() as usize].identity.declaration.name == "source")
        ).count();
        assert_eq!(candidate_calls, 1, "return candidate evaluated once");
        let errors = function
            .blocks
            .iter()
            .filter_map(|block| {
                let message =
                    block
                        .statements
                        .iter()
                        .find_map(|statement| match &statement.kind {
                            StatementKind::Evaluate(Expression {
                                kind: ExpressionKind::RuntimeFailureMessage(message),
                                ty,
                                ..
                            }) => {
                                assert_eq!(*ty, TypeInterner::NOTHING);
                                assert_eq!(message.ty, TypeInterner::STRING);
                                let ExpressionKind::Local(local) = message.kind else {
                                    panic!("borrow staged predicate error");
                                };
                                Some(local)
                            }
                            _ => None,
                        })?;
                assert!(matches!(block.terminator.kind, TerminatorKind::Unreachable));
                Some(message)
            })
            .collect::<Vec<_>>();
        assert_eq!(errors.len(), 1);
        let error = errors[0];
        assert_eq!(function.local(error).unwrap().name, "$return.error");
        let assignments = function.blocks.iter().flat_map(|block| &block.statements).filter(|statement|
            matches!(statement.kind, StatementKind::Let { local, .. } if local == error)
        ).count();
        assert_eq!(
            assignments, 2,
            "either rejected predicate forwards its exact error"
        );
        let plan = crate::move_values::MoveValuePlan::analyze(&program, function, &types).unwrap();
        assert!(plan.owned_locals.contains(&(error.index() as usize)));
        assert!(
            plan.live_in
                .iter()
                .any(|live| live.contains(&(error.index() as usize)))
        );
    }

    #[test]
    fn native_return_dynamic_failure_keeps_nested_message_handlers_in_order() {
        let (mut hir, types) = handler_source_hir(
            r#"namespace app
function inspect(value: optional[string]) returns nothing:
    string message = value handle: default "missing"
    return nothing
"#,
        );
        let function = hir
            .functions
            .iter_mut()
            .find(|function| function.identity.declaration.name == "inspect")
            .unwrap();
        let hir::StatementKind::Let { value, .. } = &function.body.statements[0].kind else {
            panic!("message handler");
        };
        let boundary = Expression {
            kind: ExpressionKind::RuntimeFailureMessage(Box::new(value.clone())),
            ty: TypeInterner::NOTHING,
            span: value.span,
        };
        assert!(has_extractable_handle(&boundary));
        function.body.statements[0].kind = hir::StatementKind::Expression(boundary);
        let program = lower(&hir, &types).unwrap();
        validate(&program).unwrap();
        let function = inspected_handler_function(&program);
        assert!(
            function
                .blocks
                .iter()
                .flat_map(|block| &block.statements)
                .any(|statement| matches!(statement.kind, StatementKind::SumTag { .. }))
        );
        let message = function
            .blocks
            .iter()
            .flat_map(|block| &block.statements)
            .find_map(|statement| match &statement.kind {
                StatementKind::Evaluate(Expression {
                    kind: ExpressionKind::RuntimeFailureMessage(message),
                    ..
                }) => Some(message),
                _ => None,
            })
            .unwrap();
        assert!(
            matches!(message.kind, ExpressionKind::Local(_)),
            "handler must be extracted before failure"
        );
        crate::move_values::MoveValuePlan::analyze(&program, function, &types).unwrap();
    }

    #[test]
    fn native_return_owned_candidates_and_error_messages_keep_cleanup_ownership() {
        let (program, types) = lower_handler_source(
            r#"namespace app
type Nonempty = list[int64] where true
function inspect(items: list[int64]) returns Nonempty:
    return items
"#,
        );
        let function = inspected_handler_function(&program);
        validate(&program).unwrap();
        let entry = &function.blocks[function.entry.index() as usize];
        let StatementKind::Let {
            local: candidate,
            value,
        } = &entry.statements[0].kind
        else {
            panic!("staged candidate");
        };
        assert!(matches!(&value.kind, ExpressionKind::Clone(inner)
            if matches!(inner.kind, ExpressionKind::Local(local) if local == function.params[0].local)));
        let plan = crate::move_values::MoveValuePlan::analyze(&program, function, &types).unwrap();
        for local in [function.params[0].local, *candidate] {
            assert!(plan.owned_locals.contains(&(local.index() as usize)));
        }
        let (index, message) = function
            .blocks
            .iter()
            .enumerate()
            .find_map(|(index, block)| {
                block
                    .statements
                    .iter()
                    .find_map(|statement| match &statement.kind {
                        StatementKind::Evaluate(Expression {
                            kind: ExpressionKind::RuntimeFailureMessage(message),
                            ..
                        }) => Some((index, message)),
                        _ => None,
                    })
            })
            .unwrap();
        let ExpressionKind::Local(error) = message.kind else {
            panic!("borrow original error");
        };
        assert!(plan.owned_locals.contains(&(error.index() as usize)));
        assert!(plan.live_in[index].contains(&(error.index() as usize)));
        let returns = function
            .blocks
            .iter()
            .filter_map(|block| {
                if let TerminatorKind::Return(Some(value)) = &block.terminator.kind {
                    Some(value)
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();
        assert_eq!(returns.len(), 1);
        assert_eq!(returns[0].ty, function.return_type);
        let ExpressionKind::Local(output) = returns[0].kind else {
            panic!("owned validated result");
        };
        assert!(plan.owned_locals.contains(&(output.index() as usize)));
    }

    const NESTED_BYTE_CALL_SOURCE: &str = r#"namespace app
function render(view value: bytes) returns string:
    return "hex"
function quote(value: string) returns string:
    return value
function inspect(view value: bytes) returns string:
    bytes item = clone value
    return quote("0x{render(item)}")
"#;

    fn nested_byte_calls(
        function: &Function,
        target: hir::FunctionId,
    ) -> Vec<(BlockId, usize, &Expression)> {
        function
            .blocks
            .iter()
            .flat_map(|block| {
                block.statements.iter().enumerate().filter_map(move |(index, statement)| {
                let StatementKind::Let { value, .. } = &statement.kind else { return None; };
                matches!(value.kind, ExpressionKind::Call { function, .. } if function == target)
                    .then_some((block.id, index, value))
            })
            })
            .collect()
    }

    #[test]
    fn nested_checked_byte_call_stages_owned_input_inside_interpolation_and_outer_call() {
        let (hir, types) = handler_source_hir(NESTED_BYTE_CALL_SOURCE);
        let original = hir
            .functions
            .iter()
            .find(|function| function.identity.declaration.name == "inspect")
            .unwrap();
        let result = original
            .body
            .statements
            .iter()
            .find_map(|statement| match &statement.kind {
                hir::StatementKind::Return(Some(value)) => Some(value),
                _ => None,
            })
            .unwrap();
        assert!(
            !has_extractable_handle(result),
            "no source handler caused the original refusal"
        );
        assert!(needs_eager_lowering(result));
        let target = hir
            .functions
            .iter()
            .find(|function| function.identity.declaration.name == "render")
            .unwrap()
            .id;
        let program = lower(&hir, &types).expect("nested ownership demand must reach render");
        let function = inspected_handler_function(&program);
        let calls = nested_byte_calls(function, target);
        assert_eq!(calls.len(), 1);
        let (block, call_index, call) = calls[0];
        let ExpressionKind::Call {
            ownership: hir::CallOwnership::Source(packet),
            ..
        } = &call.kind
        else {
            panic!("source-backed render call");
        };
        assert_eq!(
            packet.arguments[0].effect,
            jett_typecheck::CheckedCallerEffect::RelinquishOwned
        );
        let hir::ArgumentStaging::Relinquished { owner, loan } = packet.arguments[0].staging else {
            panic!("bare byte input acquired once before its physical View");
        };
        let item = function
            .locals
            .iter()
            .find(|local| local.name == "item")
            .unwrap()
            .id;
        let acquisitions = crate::validate_caller_acquisitions(&program, function, &types).unwrap();
        assert_eq!(
            acquisitions.owner_initializer(owner).unwrap().binding,
            Some(item)
        );
        assert_eq!(function.local(loan).unwrap().view_source, Some(owner));
        let statements = &function.blocks[block.index() as usize].statements;
        let owner_index = statements
            .iter()
            .position(|statement| {
                matches!(&statement.kind,
            StatementKind::Let { local, value } if *local == owner
                && matches!(value.kind, ExpressionKind::Local(source) if source == item))
            })
            .expect("full endpoint moves without Clone");
        let begin = statements
            .iter()
            .position(|statement| {
                matches!(statement.kind,
            StatementKind::BeginCallView { local, .. } if local == loan)
            })
            .unwrap();
        let end = statements
            .iter()
            .position(|statement| {
                matches!(statement.kind,
            StatementKind::EndCallView { local } if local == loan)
            })
            .unwrap();
        assert!(owner_index < begin && begin < call_index && call_index < end);
        crate::move_values::MoveValuePlan::analyze(&program, function, &types).unwrap();
    }

    #[test]
    fn nested_checked_byte_call_keeps_written_view_before_last_bare_consumption() {
        let source = NESTED_BYTE_CALL_SOURCE
            .replace("0x{render(item)}", "0x{render(view item)}:{render(item)}");
        let (hir, types) = handler_source_hir(&source);
        let target = hir
            .functions
            .iter()
            .find(|function| function.identity.declaration.name == "render")
            .unwrap()
            .id;
        let program = lower(&hir, &types).expect("written view followed by one last owning input");
        let function = inspected_handler_function(&program);
        let calls = nested_byte_calls(function, target);
        assert_eq!(calls.len(), 2);
        assert_eq!(calls[0].0, calls[1].0);
        assert!(
            calls[0].1 < calls[1].1,
            "interpolation retains lexical evaluation order"
        );
        let effects = calls
            .iter()
            .map(|(_, _, call)| {
                let ExpressionKind::Call {
                    ownership: hir::CallOwnership::Source(packet),
                    ..
                } = &call.kind
                else {
                    panic!("source call");
                };
                (packet.arguments[0].effect, packet.arguments[0].staging)
            })
            .collect::<Vec<_>>();
        assert_eq!(
            effects[0],
            (
                jett_typecheck::CheckedCallerEffect::RetainBorrow,
                hir::ArgumentStaging::Original
            )
        );
        assert_eq!(
            effects[1].0,
            jett_typecheck::CheckedCallerEffect::RelinquishOwned
        );
        assert!(matches!(
            effects[1].1,
            hir::ArgumentStaging::Relinquished { .. }
        ));
        crate::move_values::MoveValuePlan::analyze(&program, function, &types).unwrap();
    }

    #[test]
    fn nested_checked_byte_call_cannot_revert_to_original_owning_stage() {
        let (hir, types) = handler_source_hir(NESTED_BYTE_CALL_SOURCE);
        let target = hir
            .functions
            .iter()
            .find(|function| function.identity.declaration.name == "render")
            .unwrap()
            .id;
        let mut program = lower(&hir, &types).expect("valid staged control");
        let function = inspected_handler_function(&program);
        let (block, index, _) = nested_byte_calls(function, target)[0];
        let identity = function.id;
        let function = program
            .functions
            .iter_mut()
            .find(|function| function.id == identity)
            .unwrap();
        let StatementKind::Let { value, .. } =
            &mut function.blocks[block.index() as usize].statements[index].kind
        else {
            panic!("eager result");
        };
        let ExpressionKind::Call {
            ownership: hir::CallOwnership::Source(packet),
            ..
        } = &mut value.kind
        else {
            panic!("render call");
        };
        packet.arguments[0].staging = hir::ArgumentStaging::Original;
        let errors = crate::validate_call_ownership(&program, &types)
            .expect_err("mandatory owning stage still enforced");
        assert!(
            errors
                .iter()
                .any(|error| error.message.contains("requires explicit owning staging")),
            "{errors:?}"
        );
    }

    #[test]
    fn nested_checked_byte_call_keeps_lazy_boolean_rhs_in_its_selected_cfg_arm() {
        for operator in ["&&", "||"] {
            let source = r#"namespace app
function check(view value: bytes) returns bool:
    return true
function inspect(gate: bool, item: bytes) returns bool:
    return gate OPERATOR check(item)
"#
            .replace("OPERATOR", operator);
            let (hir, types) = handler_source_hir(&source);
            let target = hir
                .functions
                .iter()
                .find(|function| function.identity.declaration.name == "check")
                .unwrap()
                .id;
            let program = lower(&hir, &types).expect("lazy checked call demand");
            let function = inspected_handler_function(&program);
            let calls = nested_byte_calls(function, target);
            assert_eq!(calls.len(), 1);
            let entry = &function.blocks[function.entry.index() as usize];
            let TerminatorKind::Branch {
                then_block,
                else_block,
                ..
            } = entry.terminator.kind
            else {
                panic!("lazy branch required");
            };
            let (selected, bypass) = if operator == "&&" {
                (then_block, else_block)
            } else {
                (else_block, then_block)
            };
            assert_eq!(calls[0].0, selected);
            assert_ne!(calls[0].0, function.entry);
            assert!(function.blocks[bypass.index() as usize].statements.iter().any(|statement|
                matches!(statement.kind, StatementKind::Let { value: Expression { kind: ExpressionKind::Bool(value), .. }, .. }
                    if value == (operator == "||"))));
            crate::move_values::MoveValuePlan::analyze(&program, function, &types).unwrap();
        }
    }

    #[test]
    fn equatable_result_precedes_later_handlers_without_extra_owned_storage() {
        for operator in ["==", "!="] {
            let source = r#"namespace app
struct Item:
    id: int64
implement Equatable for Item:
    function equals(view self: Item, view other: Item) returns bool:
        return run true
function combine(first: bool, second: bool) returns bool:
    return first && second
function inspect(view left: Item, view right: Item) returns bool:
    optional[bool] later = none
    return combine(left OPERATOR right, (later handle: default true))
"#
            .replace("OPERATOR", operator);
            let (hir, types) = handler_source_hir_with_equatable(&source, true);
            let program = lower(&hir, &types).expect("MIR lowering");
            validate(&program).expect("equality argument handler CFG");
            let function = inspected_handler_function(&program);
            let entry = &function.blocks[function.entry.index() as usize];
            let guards = entry
                .statements
                .iter()
                .enumerate()
                .filter_map(|(index, statement)| {
                    let StatementKind::Let { local, value } = &statement.kind else {
                        return None;
                    };
                    let boundary = if let ExpressionKind::Unary {
                        op: hir::UnaryOp::Not,
                        value,
                    } = &value.kind
                    {
                        value.as_ref()
                    } else {
                        value
                    };
                    matches!(boundary.kind, ExpressionKind::EquatableResult(_))
                        .then_some((index, *local))
                })
                .collect::<Vec<_>>();
            assert_eq!(guards.len(), 1, "{operator}");
            let (guard_index, guard_local) = guards[0];
            let handler_index = entry
                .statements
                .iter()
                .position(|statement| matches!(statement.kind, StatementKind::SumTag { .. }))
                .unwrap();
            assert!(guard_index < handler_index, "{operator}");
            let checked =
                crate::move_values::MoveValuePlan::analyze(&program, function, &types).unwrap();
            assert!(
                !checked
                    .owned_locals
                    .contains(&(guard_local.index() as usize))
            );

            let mut unchecked = program.clone();
            let lowered = unchecked
                .functions
                .iter_mut()
                .find(|candidate| candidate.id == function.id)
                .unwrap();
            let StatementKind::Let { value, .. } =
                &mut lowered.blocks[function.entry.index() as usize].statements[guard_index].kind
            else {
                unreachable!();
            };
            if let ExpressionKind::Unary { value: inner, .. } = &mut value.kind {
                let ExpressionKind::EquatableResult(call) = &inner.kind else {
                    unreachable!();
                };
                **inner = call.as_ref().clone();
            } else {
                let ExpressionKind::EquatableResult(call) = &value.kind else {
                    unreachable!();
                };
                *value = call.as_ref().clone();
            }
            let unchecked_plan = crate::move_values::MoveValuePlan::analyze(
                &unchecked,
                inspected_handler_function(&unchecked),
                &types,
            )
            .unwrap();
            assert_eq!(
                checked.temporary_slots, unchecked_plan.temporary_slots,
                "{operator}"
            );
            assert_eq!(
                checked.owned_locals, unchecked_plan.owned_locals,
                "{operator}"
            );
            assert_eq!(
                checked.live_after_statement, unchecked_plan.live_after_statement,
                "{operator}"
            );
        }
    }

    fn eager_generated_boundary_call<'a>(
        function: &'a Function,
        block: BlockId,
        boundary_index: usize,
        result: &Expression,
        equality: bool,
    ) -> &'a Expression {
        let ExpressionKind::Local(output) = result.kind else {
            panic!("checked boundary must observe its eagerly completed call result");
        };
        let metadata = function.local(output).expect("exact call result metadata");
        assert_eq!(metadata.ty, result.ty);
        assert_eq!(metadata.debug_ty, result.ty);
        assert_eq!(metadata.span, result.span);
        assert!(metadata.view_source.is_none());
        assert!(function.parameter_for_local(output).is_none());
        let definitions = function
            .blocks
            .iter()
            .flat_map(|block| {
                block
                    .statements
                    .iter()
                    .enumerate()
                    .filter_map(move |(index, statement)| {
                        let StatementKind::Let { local, value } = &statement.kind else {
                            return None;
                        };
                        (*local == output).then_some((block.id, index, value))
                    })
            })
            .collect::<Vec<_>>();
        assert_eq!(definitions.len(), 1, "one eagerly completed operation");
        let (defined_block, call_index, call) = definitions[0];
        assert_eq!(defined_block, block);
        assert!(call_index < boundary_index);
        assert_eq!(call.ty, result.ty);
        assert_eq!(call.span, result.span);
        let ExpressionKind::Call {
            function: method,
            args,
            ownership: hir::CallOwnership::Generated(packet),
            ..
        } = &call.kind
        else {
            panic!("exact eagerly materialized generated method call");
        };
        match (&packet.operation, equality) {
            (hir::GeneratedOperation::Equality { method: expected }, true)
            | (hir::GeneratedOperation::Display { method: expected }, false) => {
                assert_eq!(method, expected)
            }
            _ => panic!("result must belong to its exact checked boundary operation"),
        }
        assert_eq!(args.len(), if equality { 2 } else { 1 });
        assert_eq!(packet.arguments.len(), args.len());
        let statements = &function.blocks[block.index() as usize].statements;
        for (value, argument) in args.iter().zip(&packet.arguments) {
            let loan = match argument.staging {
                hir::GeneratedArgumentStaging::Existing { loan: Some(loan) }
                | hir::GeneratedArgumentStaging::OwnedProducer { loan, .. }
                | hir::GeneratedArgumentStaging::OrdinarySnapshot { loan, .. } => loan,
                _ => panic!("eager call operand must retain its exact scoped loan"),
            };
            assert!(matches!(&value.kind, ExpressionKind::View(inner)
                if matches!(inner.kind, ExpressionKind::Local(local) if local == loan)));
            let begins = statements.iter().enumerate().filter_map(|(index, statement)|
                matches!(statement.kind, StatementKind::BeginCallView { local, .. } if local == loan)
                    .then_some(index)).collect::<Vec<_>>();
            let ends = statements
                .iter()
                .enumerate()
                .filter_map(|(index, statement)| {
                    matches!(statement.kind, StatementKind::EndCallView { local } if local == loan)
                        .then_some(index)
                })
                .collect::<Vec<_>>();
            assert_eq!(begins.len(), 1);
            assert_eq!(ends.len(), 1);
            assert!(begins[0] < call_index);
            assert!(call_index < ends[0] && ends[0] < boundary_index);
        }
        call
    }

    #[test]
    fn equatable_result_retains_operand_handlers_before_call_and_negation() {
        for operator in ["==", "!="] {
            let source = r#"namespace app
struct Item:
    id: int64
implement Equatable for Item:
    function equals(view self: Item, view other: Item) returns bool:
        return true
function inspect(view left: optional[Item], view right: Item) returns bool:
    return (left handle: default Item(id: 7)) OPERATOR right
"#
            .replace("OPERATOR", operator);
            let (hir, types) = handler_source_hir_with_equatable(&source, true);
            let program = lower(&hir, &types).expect("MIR lowering");
            validate(&program).expect("equality operand handler CFG");
            let function = inspected_handler_function(&program);
            crate::move_values::MoveValuePlan::analyze(&program, function, &types).unwrap();
            assert!(
                function.blocks[function.entry.index() as usize]
                    .statements
                    .iter()
                    .any(|statement| matches!(statement.kind, StatementKind::SumTag { .. }))
            );
            let guards = function
                .blocks
                .iter()
                .filter_map(|block| {
                    let TerminatorKind::Return(Some(value)) = &block.terminator.kind else {
                        return None;
                    };
                    assert_eq!(
                        matches!(
                            value.kind,
                            ExpressionKind::Unary {
                                op: hir::UnaryOp::Not,
                                ..
                            }
                        ),
                        operator == "!="
                    );
                    let boundary = if let ExpressionKind::Unary {
                        op: hir::UnaryOp::Not,
                        value,
                    } = &value.kind
                    {
                        value.as_ref()
                    } else {
                        value
                    };
                    let ExpressionKind::EquatableResult(call) = &boundary.kind else {
                        return None;
                    };
                    Some((block.id, boundary, call))
                })
                .collect::<Vec<_>>();
            assert_eq!(guards.len(), 1, "{operator}");
            let (block, boundary, call) = guards[0];
            assert_ne!(block, function.entry);
            let call = eager_generated_boundary_call(
                function,
                block,
                function.blocks[block.index() as usize].statements.len(),
                call,
                true,
            );
            assert!(matches!(call.kind, ExpressionKind::Call { .. }));
            assert!(!has_extractable_handle(call));
            assert!(!has_extractable_handle(boundary));
        }
    }

    #[test]
    fn display_result_is_checked_before_later_interpolation_handlers() {
        let (program, types) = lower_handler_source(
            r#"interface Displayable:
    function display(view self: Displayable) returns string
namespace app
struct Item:
    value: int64
implement Displayable for Item:
    function display(view self: Item) returns string:
        return run "shown"
function inspect(view item: Item) returns string:
    optional[string] later = none
    return "{item}:{later handle: default "later"}"
"#,
        );
        validate(&program).expect("interpolation handler CFG");
        let function = inspected_handler_function(&program);
        let entry = &function.blocks[function.entry.index() as usize];
        let guards = entry
            .statements
            .iter()
            .enumerate()
            .filter_map(|(index, statement)| match &statement.kind {
                StatementKind::Let { local, value }
                    if matches!(value.kind, ExpressionKind::DisplayResult(_)) =>
                {
                    Some((index, *local, value))
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(guards.len(), 1);
        let (guard_index, guard_local, guard) = guards[0];
        let ExpressionKind::DisplayResult(call) = &guard.kind else {
            unreachable!();
        };
        assert!(matches!(call.kind, ExpressionKind::Call { .. }));
        let handler_index = entry
            .statements
            .iter()
            .position(|statement| matches!(statement.kind, StatementKind::SumTag { .. }))
            .expect("later optional handler");
        assert!(guard_index < handler_index);
        assert!(function.blocks.iter().any(|block| {
            matches!(&block.terminator.kind,
                TerminatorKind::Return(Some(Expression {
                    kind: ExpressionKind::StringInterpolation(segments), ..
                })) if segments.iter().any(|segment|
                    matches!(segment, hir::StringSegment::Value(value)
                        if matches!(value.kind, ExpressionKind::Local(local) if local == guard_local))))
        }));

        let checked_plan =
            crate::move_values::MoveValuePlan::analyze(&program, function, &types).unwrap();
        assert!(
            checked_plan
                .owned_locals
                .contains(&(guard_local.index() as usize))
        );
        let mut unchecked = program.clone();
        let unchecked_function = unchecked
            .functions
            .iter_mut()
            .find(|candidate| candidate.id == function.id)
            .unwrap();
        let StatementKind::Let { value, .. } =
            &mut unchecked_function.blocks[function.entry.index() as usize].statements[guard_index]
                .kind
        else {
            unreachable!();
        };
        *value = call.as_ref().clone();
        let unchecked_plan = crate::move_values::MoveValuePlan::analyze(
            &unchecked,
            inspected_handler_function(&unchecked),
            &types,
        )
        .unwrap();
        assert_eq!(checked_plan.temporary_slots, unchecked_plan.temporary_slots);
        assert_eq!(checked_plan.owned_locals, unchecked_plan.owned_locals);
        assert_eq!(
            checked_plan.live_after_statement,
            unchecked_plan.live_after_statement
        );
    }

    #[test]
    fn display_result_keeps_receiver_handlers_inside_the_checked_call() {
        let (program, types) = lower_handler_source(
            r#"interface Displayable:
    function display(view self: Displayable) returns string
namespace app
struct Item:
    value: int64
implement Displayable for Item:
    function display(view self: Item) returns string:
        return "shown"
function inspect(view item: optional[Item]) returns string:
    return "{item handle: default Item(value: 7)}"
"#,
        );
        validate(&program).expect("display receiver handler CFG");
        let function = inspected_handler_function(&program);
        crate::move_values::MoveValuePlan::analyze(&program, function, &types).unwrap();
        assert!(
            function.blocks[function.entry.index() as usize]
                .statements
                .iter()
                .any(|statement| matches!(statement.kind, StatementKind::SumTag { .. }))
        );
        let guards = function
            .blocks
            .iter()
            .flat_map(|block| {
                block
                    .statements
                    .iter()
                    .enumerate()
                    .map(move |(index, statement)| (block.id, index, statement))
            })
            .filter_map(|(block, index, statement)| match &statement.kind {
                StatementKind::Let { value, .. }
                    if matches!(value.kind, ExpressionKind::DisplayResult(_)) =>
                {
                    Some((block, index, value))
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(guards.len(), 1);
        let (block, guard_index, guard) = guards[0];
        assert_ne!(block, function.entry);
        let ExpressionKind::DisplayResult(call) = &guard.kind else {
            unreachable!();
        };
        let call = eager_generated_boundary_call(function, block, guard_index, call, false);
        assert!(matches!(call.kind, ExpressionKind::Call { .. }));
        assert!(!has_extractable_handle(call));
        assert!(!has_extractable_handle(guard));
    }

    #[test]
    fn alias_let_preserves_coarsen_declassify_and_forwarded_backing() {
        let (hir, types) = handler_source_hir(
            r#"namespace app
type Numbers = list[int64] where true
function inspect(view refined: Numbers, view hidden: secret[list[int64]]) returns list[int64]:
    list[int64] plain = coarsen refined
    list[int64] forwarded = plain
    list[int64] exposed = declassify hidden
    list[int64] exposed_alias = exposed
    trace forwarded
    return clone exposed_alias
"#,
        );
        let program = lower(&hir, &types).expect("MIR lowering");
        validate(&program).expect("preserved alias MIR");
        let function = inspected_handler_function(&program);
        let original = hir
            .functions
            .iter()
            .find(|original| original.id == function.id)
            .unwrap();
        let plan = crate::move_values::MoveValuePlan::analyze(&program, function, &types).unwrap();
        let mut aliases = 0;
        for statement in &original.body.statements {
            let hir::StatementKind::Let { local, value } = &statement.kind else {
                continue;
            };
            let metadata = function.local(*local).unwrap();
            let Some(source) = metadata.view_source else {
                continue;
            };
            aliases += 1;
            let lowered = function
                .blocks
                .iter()
                .flat_map(|block| &block.statements)
                .find_map(|statement| match &statement.kind {
                    StatementKind::Let {
                        local: target,
                        value,
                    } if target == local => Some(value),
                    _ => None,
                })
                .unwrap();
            assert_eq!(lowered, value, "{} initializer was changed", metadata.name);
            hir::validate_local_view_initializer(
                lowered,
                source,
                function.local(source).unwrap().ty,
                metadata.ty,
                &types,
            )
            .unwrap();
            assert!(!plan.owned_locals.contains(&(local.index() as usize)));
            if metadata.name == "forwarded" || metadata.name == "exposed_alias" {
                assert!(function.local(source).unwrap().view_source.is_some());
            }
        }
        assert_eq!(aliases, 4);
    }

    #[test]
    fn owned_let_keeps_coarsen_and_declassify_snapshots() {
        let (program, types) = lower_handler_source(
            r#"namespace app
type Numbers = list[int64] where true
function inspect(refined: Numbers, hidden: secret[list[int64]]) returns list[int64]:
    list[int64] plain = coarsen refined
    list[int64] exposed = declassify hidden
    trace refined
    trace hidden
    trace plain
    return exposed
"#,
        );
        let function = inspected_handler_function(&program);
        let plan = crate::move_values::MoveValuePlan::analyze(&program, function, &types).unwrap();
        for name in ["plain", "exposed"] {
            let local = function
                .locals
                .iter()
                .find(|local| local.name == name)
                .unwrap();
            assert!(local.view_source.is_none());
            assert!(plan.owned_locals.contains(&(local.id.index() as usize)));
            let value = function
                .blocks
                .iter()
                .flat_map(|block| &block.statements)
                .find_map(|statement| match &statement.kind {
                    StatementKind::Let {
                        local: target,
                        value,
                    } if *target == local.id => Some(value),
                    _ => None,
                })
                .unwrap();
            let (ExpressionKind::Coarsen(snapshot) | ExpressionKind::Declassify(snapshot)) =
                &value.kind
            else {
                panic!("expected a qualification wrapper: {value:?}");
            };
            assert!(matches!(&snapshot.kind, ExpressionKind::Clone(value)
                if matches!(value.kind, ExpressionKind::Local(_))));
        }
    }

    #[test]
    fn alias_let_does_not_sanitize_invalid_hir_initializers() {
        for invalid in ["clone", "handle", "different source"] {
            let (mut hir, mut types) = handler_source_hir(
                r#"namespace app
function inspect(view source: list[int64], view other: list[int64]) returns list[int64]:
    list[int64] borrowed = view source
    return clone borrowed
"#,
            );
            let function = hir
                .functions
                .iter_mut()
                .find(|function| function.identity.declaration.name == "inspect")
                .unwrap();
            let source = function
                .locals
                .iter()
                .find(|local| local.name == "source")
                .unwrap();
            let other = function
                .locals
                .iter()
                .find(|local| local.name == "other")
                .unwrap();
            let input = Expression {
                kind: ExpressionKind::Local(source.id),
                ty: source.ty,
                span: source.span,
            };
            let other = Expression {
                kind: ExpressionKind::Local(other.id),
                ty: other.ty,
                span: other.span,
            };
            let clone = Expression {
                kind: ExpressionKind::Clone(Box::new(input.clone())),
                ..input.clone()
            };
            let malformed = match invalid {
                "clone" => clone,
                "different source" => other,
                "handle" => Expression {
                    kind: ExpressionKind::Handle {
                        target: Box::new(Expression {
                            kind: ExpressionKind::OptionalSome(Box::new(clone)),
                            ty: types.intern(Type::Optional(input.ty)),
                            span: input.span,
                        }),
                        kind: HandleKind::Optional,
                        error_local: None,
                        failure: hir::Block {
                            statements: vec![hir::Statement {
                                kind: hir::StatementKind::HandleDefault(Expression {
                                    kind: ExpressionKind::Clone(Box::new(other)),
                                    ..input.clone()
                                }),
                                span: input.span,
                            }],
                            span: input.span,
                        },
                    },
                    ..input.clone()
                },
                _ => unreachable!(),
            };
            let hir::StatementKind::Let { value, .. } = &mut function.body.statements[0].kind
            else {
                panic!("expected alias declaration");
            };
            *value = malformed;
            hir::validate(&hir).expect("malformed alias remains structurally valid HIR");
            assert!(
                hir::validate_backend_types(&hir, &types).is_err(),
                "{invalid}"
            );
            let errors =
                lower(&hir, &types).expect_err("malformed alias refused before MIR acquisition");
            assert!(
                errors
                    .iter()
                    .any(|error| error.message.contains("stable backing local")),
                "{invalid}: {errors:?}"
            );
        }
    }

    fn staged_alias_source_argument(
        function: &Function,
        alias: LocalId,
    ) -> (&hir::ArgumentOwnership, &Expression, BlockId, usize) {
        let calls = function.blocks.iter().flat_map(|block| {
            block.statements.iter().enumerate().filter_map(move |(index, statement)| {
                let value = match &statement.kind {
                    StatementKind::Let { value, .. } | StatementKind::Evaluate(value) => value,
                    _ => return None,
                };
                let ExpressionKind::Call { ownership: hir::CallOwnership::Source(packet), args, .. } = &value.kind
                    else { return None; };
                let argument = packet.arguments.iter().position(|argument|
                    matches!(&argument.origin, hir::CallerOrigin::Binding(fact) if fact.local == alias))?;
                Some((&packet.arguments[argument], &args[argument], block.id, index))
            })
        }).collect::<Vec<_>>();
        assert_eq!(calls.len(), 1, "one exact Source argument for the alias");
        calls[0]
    }

    fn assert_staged_alias_has_retained_snapshot(
        program: &Program,
        function: &Function,
        alias: LocalId,
        types: &TypeInterner,
    ) {
        let (argument, actual, call_block, call_index) =
            staged_alias_source_argument(function, alias);
        assert_eq!(
            argument.effect,
            jett_typecheck::CheckedCallerEffect::RetainBorrow
        );
        assert_eq!(
            argument.syntax,
            jett_typecheck::CheckedCallerSyntax::WrittenView
        );
        let hir::ArgumentStaging::RetainedSnapshot { owner, loan } = argument.staging else {
            panic!("ordinary alias endpoint snapshot");
        };
        let physical = argument
            .retained_snapshot_span()
            .expect("sealed physical occurrence");
        assert_eq!(argument.retained_snapshot_type(), Some(actual.ty));
        assert_eq!(actual.span, physical);
        assert!(matches!(&actual.kind, ExpressionKind::View(value)
            if matches!(value.kind, ExpressionKind::Local(id) if id == loan)));

        let statements = || {
            function.blocks.iter().flat_map(|block| {
                block
                    .statements
                    .iter()
                    .enumerate()
                    .map(move |(index, statement)| (block.id, index, statement))
            })
        };
        let owners = statements()
            .filter_map(|(block, index, statement)| match &statement.kind {
                StatementKind::Let { local, value } if *local == owner => {
                    Some((block, index, value))
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(owners.len(), 1, "unique owning endpoint initializer");
        let (owner_block, owner_index, initializer) = owners[0];
        let ExpressionKind::Clone(endpoint) = &initializer.kind else {
            panic!("full endpoint Clone");
        };
        let ExpressionKind::View(original) = &endpoint.kind else {
            panic!("unchanged written View");
        };
        assert!(matches!(original.kind, ExpressionKind::Local(id) if id == alias));
        assert_eq!(initializer.span, physical);
        assert_eq!(endpoint.span, physical);
        assert_eq!(
            endpoint.span, argument.source_span,
            "original written View occurrence"
        );
        assert!(function.local(owner).unwrap().view_source.is_none());
        assert_eq!(function.local(loan).unwrap().view_source, Some(owner));

        let begins = statements()
            .filter_map(|(block, index, statement)| match &statement.kind {
                StatementKind::BeginCallView { local, value } if *local == loan => {
                    Some((block, index, value))
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(begins.len(), 1, "unique owner-backed Begin");
        let (begin_block, begin_index, projection) = begins[0];
        assert_eq!(begin_block, owner_block);
        assert!(owner_index < begin_index);
        assert!(matches!(&projection.kind, ExpressionKind::View(value)
            if matches!(value.kind, ExpressionKind::Local(id) if id == owner)));
        let tag_index = function.blocks[owner_block.index() as usize]
            .statements
            .iter()
            .position(|statement| matches!(statement.kind, StatementKind::SumTag { .. }))
            .expect("later argument handler tag");
        assert!(
            begin_index < tag_index,
            "capture alias endpoint before later handler"
        );

        let ends = statements()
            .filter_map(|(block, index, statement)| {
                matches!(statement.kind, StatementKind::EndCallView { local } if local == loan)
                    .then_some((block, index))
            })
            .collect::<Vec<_>>();
        assert_eq!(ends.len(), 1, "unique completion End");
        let (end_block, end_index) = ends[0];
        assert_eq!(end_block, call_block);
        assert!(call_index < end_index);

        // Persistent alias storage remains precisely its checked initializer.
        let alias_values = statements()
            .filter_map(|(_, _, statement)| match &statement.kind {
                StatementKind::Let { local, value } if *local == alias => Some(value),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(alias_values.len(), 1);
        let metadata = function.local(alias).unwrap();
        let backing = metadata
            .view_source
            .expect("original alias stays nonowning");
        hir::validate_local_view_initializer(
            alias_values[0],
            backing,
            function.local(backing).unwrap().ty,
            metadata.ty,
            types,
        )
        .unwrap();
        let plan = crate::move_values::MoveValuePlan::analyze(program, function, types).unwrap();
        assert!(plan.owned_locals.contains(&(owner.index() as usize)));
        assert!(!plan.owned_locals.contains(&(alias.index() as usize)));
        assert!(!plan.owned_locals.contains(&(loan.index() as usize)));
        assert!(
            plan.live_after_statement[call_block.index() as usize][call_index]
                .contains(&(owner.index() as usize))
        );
        assert!(
            !plan.live_after_statement[end_block.index() as usize][end_index]
                .contains(&(owner.index() as usize)),
            "owner becomes dead after End"
        );
    }

    fn assert_staged_alias_has_raw_borrow(function: &Function, alias: LocalId) {
        let (argument, _, _, _) = staged_alias_source_argument(function, alias);
        assert_eq!(
            argument.effect,
            jett_typecheck::CheckedCallerEffect::RetainBorrow
        );
        assert_eq!(
            argument.syntax,
            jett_typecheck::CheckedCallerSyntax::WrittenView
        );
        assert!(
            argument.retained_snapshot_type().is_none(),
            "no descriptor data-copy proof"
        );
        assert!(matches!(
            argument.staging,
            hir::ArgumentStaging::Borrowed { .. }
        ));
        assert_staged_alias_is_borrowed(function, alias);
    }
    fn assert_staged_alias_is_borrowed(function: &Function, alias: LocalId) {
        let statements = || function.blocks.iter().flat_map(|block| &block.statements);
        let loans = statements()
            .filter_map(|statement| match &statement.kind {
                StatementKind::BeginCallView { local, value }
                    if matches!(&value.kind, ExpressionKind::View(value)
                    if matches!(value.kind, ExpressionKind::Local(id) if id == alias)) =>
                {
                    Some(*local)
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(loans.len(), 1, "exact alias loan");
        assert!(statements().any(|statement| matches!(statement.kind,
            StatementKind::EndCallView { local } if local == loans[0])));
        assert!(!statements().any(|statement| matches!(&statement.kind,
            StatementKind::Let { value, .. } if matches!(&value.kind,
                ExpressionKind::Clone(value) if matches!(&value.kind,
                    ExpressionKind::View(value) if matches!(value.kind,
                        ExpressionKind::Local(id) if id == alias))))));
    }

    #[test]
    fn handler_call_staging_preserves_explicit_local_alias_view_parameters() {
        let (program, types) = lower_handler_source(
            r#"namespace app
function take(view values: list[int64], amount: int64) returns int64:
    return amount
function inspect() returns int64:
    list[int64] source = list(1)
    list[int64] borrowed = view source
    list[int64] forwarded = borrowed
    int64 answer = take(view forwarded, (none handle: default 3))
    trace source
    return answer
"#,
        );
        validate(&program).expect("extracted handler CFG");
        let function = inspected_handler_function(&program);
        crate::move_values::MoveValuePlan::analyze(&program, function, &types).unwrap();
        let forwarded = function
            .locals
            .iter()
            .find(|local| local.name == "forwarded")
            .unwrap()
            .id;
        assert_staged_alias_has_retained_snapshot(&program, function, forwarded, &types);
    }

    #[test]
    fn handler_indirect_staging_snapshots_alias_callee_after_borrowed_arguments() {
        let (program, types) = lower_handler_source(
            r#"namespace app
function take(view values: list[int64], amount: int64) returns int64:
    return amount
function inspect() returns int64:
    list[int64] source = list(1)
    list[int64] borrowed = view source
    function(view list[int64], int64) returns int64 callback = take
    function(view list[int64], int64) returns int64 callable = view callback
    int64 answer = callable(view borrowed, (none handle: default 4))
    trace source
    trace callback
    trace callable
    return answer
"#,
        );
        validate(&program).expect("extracted indirect handler CFG");
        let function = inspected_handler_function(&program);
        crate::move_values::MoveValuePlan::analyze(&program, function, &types).unwrap();
        let callable = function
            .locals
            .iter()
            .find(|local| local.name == "callable")
            .unwrap()
            .id;
        let stages = function
            .blocks
            .iter()
            .flat_map(|block| &block.statements)
            .filter_map(|statement| match &statement.kind {
                StatementKind::Let { local, value }
                    if matches!(&value.kind,
                    ExpressionKind::Clone(value) if matches!(value.kind,
                        ExpressionKind::Local(id) if id == callable)) =>
                {
                    Some(*local)
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(stages.len(), 1);
        let callee_stage = stages[0];
        assert!(
            function
                .blocks
                .iter()
                .flat_map(|block| &block.statements)
                .any(|statement| {
                    matches!(&statement.kind, StatementKind::Let { value, .. }
                if matches!(&value.kind, ExpressionKind::IndirectCall { callee, .. }
                    if matches!(callee.kind, ExpressionKind::Local(id) if id == callee_stage)))
                })
        );
        assert!(function.blocks.iter().any(|block| {
            block.statements.iter().any(|statement| {
                matches!(&statement.kind,
                StatementKind::Let { local, .. } if *local == callee_stage)
            }) && matches!(block.terminator.kind, TerminatorKind::Return(_))
        }));
    }

    #[test]
    fn handler_call_staging_snapshots_data_but_keeps_function_alias_borrowed() {
        let (program, types) = lower_handler_source(
            r#"namespace app
function increment(value: int64) returns int64:
    return value + 1
function take(view text: secret[string], view callback: function(int64) returns int64, amount: int64) returns int64:
    return callback(amount)
function inspect() returns int64:
    secret[string] source = "kept"
    secret[string] borrowed = view source
    function(int64) returns int64 callback = increment
    function(int64) returns int64 callable = view callback
    int64 answer = take(view borrowed, view callable, (none handle: default 4))
    trace source
    trace callback
    return answer
"#,
        );
        validate(&program).expect("copy-owned staged views");
        let function = inspected_handler_function(&program);
        crate::move_values::MoveValuePlan::analyze(&program, function, &types).unwrap();
        let alias = |name| {
            function
                .locals
                .iter()
                .find(|local| local.name == name)
                .unwrap()
                .id
        };
        assert_staged_alias_has_retained_snapshot(&program, function, alias("borrowed"), &types);
        assert_staged_alias_has_raw_borrow(function, alias("callable"));
    }

    #[test]
    fn handler_call_staging_keeps_owned_alias_boundaries_rejected() {
        let file = jett_common::FileId::new(0);
        let parsed = jett_parser::parse(
            r#"namespace app
function take(view values: list[int64], amount: int64) returns int64:
    return amount
function inspect() returns int64:
    list[int64] source = list(1)
    list[int64] borrowed = view source
    return take(view borrowed, (none handle: default 3))
"#,
            file,
        );
        let resolved = jett_resolve::resolve(&parsed.module);
        let checked = jett_typecheck::check(&parsed.module, &resolved);
        assert!(parsed.errors.is_empty());
        assert!(
            checked
                .diagnostics
                .iter()
                .all(|diagnostic| { diagnostic.severity != jett_diagnostics::Severity::Error }),
            "{:?}",
            checked.diagnostics
        );
        let mut hir = jett_hir::lower(
            &parsed.module,
            &resolved,
            &checked,
            &std::collections::HashMap::from([(file, jett_common::SourceOrigin::Project)]),
        )
        .unwrap();
        hir.functions
            .iter_mut()
            .find(|function| function.identity.declaration.name == "take")
            .unwrap()
            .params[0]
            .mode = ParamMode::Owned;
        let errors = lower(&hir, &checked.interner)
            .expect_err("physical formal mutation refused before staging");
        assert!(
            errors.iter().any(|error| error.message.contains(
                "source function ownership signature differs from the exact HIR declaration"
            )),
            "{errors:?}"
        );

        let span = Span::new(file, 0, 1);
        let explicit = Expression {
            kind: ExpressionKind::View(Box::new(Expression {
                kind: ExpressionKind::Local(LocalId::new(0)),
                ty: TypeInterner::STRING,
                span,
            })),
            ty: TypeInterner::STRING,
            span,
        };
        assert!(call_staging_inputs(&[explicit], &[ParamMode::Owned]).is_none());
    }

    #[test]
    fn aggregate_view_snapshot_rejects_nested_resources() {
        let mut types = TypeInterner::new();
        let resource = types.intern(Type::Resource("Socket".to_string()));
        let list_of_resources = types.intern(Type::List(resource));
        let list_of_integers = types.intern(Type::List(TypeInterner::INT64));

        assert!(!can_snapshot_view(&types, resource));
        assert!(!can_snapshot_view(&types, list_of_resources));
        assert!(can_snapshot_view(&types, list_of_integers));
    }

    #[test]
    fn sum_snapshot_ignores_only_absent_never_payloads() {
        let mut types = TypeInterner::new();
        let resource = types.intern(Type::Resource("Socket".to_string()));
        let secret_never = types.intern(Type::Secret(TypeInterner::NEVER));
        let absent = types.intern(Type::Optional(TypeInterner::NEVER));
        let no_success = types.intern(Type::Result(TypeInterner::NEVER, TypeInterner::STRING));
        let no_error = types.intern(Type::Result(TypeInterner::STRING, TypeInterner::NEVER));
        let impossible = types.intern(Type::Result(TypeInterner::NEVER, TypeInterner::NEVER));
        let resource_success = types.intern(Type::Result(resource, TypeInterner::NEVER));
        let resource_failure = types.intern(Type::Result(TypeInterner::NEVER, resource));
        let secret_payload = types.intern(Type::Optional(secret_never));
        let empty_list = types.intern(Type::List(TypeInterner::NEVER));

        assert!(can_snapshot_view(&types, absent));
        assert!(can_snapshot_view(&types, no_success));
        assert!(can_snapshot_view(&types, no_error));
        assert!(can_snapshot_view(&types, empty_list));
        for rejected in [
            TypeInterner::NEVER,
            impossible,
            resource_success,
            resource_failure,
            secret_never,
            secret_payload,
        ] {
            assert!(!can_snapshot_view(&types, rejected), "{rejected:?}");
        }
    }

    #[test]
    fn contextual_sum_snapshot_clones_the_original_local_before_conversion() {
        let mut types = TypeInterner::new();
        let empty = types.intern(Type::List(TypeInterner::NEVER));
        let strings = types.intern(Type::List(TypeInterner::STRING));
        let original = types.intern(Type::Optional(empty));
        let contextual = types.intern(Type::Optional(strings));
        let span = Span::new(jett_common::FileId::new(0), 0, 1);
        let local = Expression {
            kind: ExpressionKind::Local(LocalId::new(0)),
            ty: original,
            span,
        };
        let conversion = Expression {
            kind: ExpressionKind::interface_coerce(Box::new(local.clone())),
            ty: contextual,
            span,
        };
        assert!(snapshotable_local(&types, &conversion));
        let snapshot = snapshot_local(&conversion, conversion.clone());
        let ExpressionKind::InterfaceCoerce { value, .. } = snapshot.kind else {
            panic!("expected retained contextual conversion");
        };
        let ExpressionKind::Clone(value) = value.kind else {
            panic!("expected source clone before conversion");
        };
        assert_eq!(*value, local);
        assert_eq!(snapshot.ty, contextual);

        let empty_map = types.intern(Type::Map(TypeInterner::STRING, TypeInterner::NEVER));
        let optional_map = types.intern(Type::Optional(empty_map));
        assert!(can_snapshot_view(&types, optional_map));
        let resource = types.intern(Type::Resource("Socket".into()));
        let resource_list = types.intern(Type::List(resource));
        let optional_resource = types.intern(Type::Optional(resource_list));
        assert!(!can_snapshot_view(&types, optional_resource));
        assert!(!can_snapshot_view(&types, TypeInterner::NEVER));
    }
}
