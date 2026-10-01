//! Extract source result/optional handlers into ordinary MIR CFG edges.
use super::*;
use jett_hir::{ExpressionKind, HandleKind};
use jett_types::{Type, TypeInterner};

fn has_extractable_handle(expression: &Expression) -> bool {
    match &expression.kind {
        ExpressionKind::Handle {
            kind: HandleKind::Result | HandleKind::Optional | HandleKind::Refinement { .. },
            ..
        } => true,
        ExpressionKind::View(value) | ExpressionKind::FunctionAdapter { value, .. } => has_extractable_handle(value),
        ExpressionKind::Clone(value) => has_extractable_handle(value),
        ExpressionKind::Run(value) | ExpressionKind::Join(value) => has_extractable_handle(value),
        ExpressionKind::Coarsen(value) | ExpressionKind::Declassify(value)
        | ExpressionKind::RefinementValidated(value)
        | ExpressionKind::DisplayResult(value)
        | ExpressionKind::EquatableResult(value)
        | ExpressionKind::InterfaceCoerce { value, .. } | ExpressionKind::InterfaceType(value) => {
            has_extractable_handle(value)
        }
        ExpressionKind::Field { base, .. } => has_extractable_handle(base),
        ExpressionKind::StateIs { value, .. } => has_extractable_handle(value),
        ExpressionKind::Unary { value, .. } => has_extractable_handle(value),
        ExpressionKind::Binary { left, right, .. } => {
            has_extractable_handle(left) || has_extractable_handle(right)
        }
        ExpressionKind::Call { args, .. } => args.iter().any(has_extractable_handle),
        ExpressionKind::ActorSpawn { args, .. } => args.iter().any(has_extractable_handle),
        ExpressionKind::ActorMessage { actor, args, .. } => {
            has_extractable_handle(actor) || args.iter().any(has_extractable_handle)
        }
        ExpressionKind::Intrinsic {
            args,
            refinement_predicates,
            ..
        } => {
            args.iter().any(has_extractable_handle)
                || refinement_predicates.iter().any(|chain| !chain.is_empty())
        }
        ExpressionKind::IndirectCall { callee, args, .. } => {
            has_extractable_handle(callee) || args.iter().any(has_extractable_handle)
        }
        ExpressionKind::ListConstruct { elements } => elements.iter().any(has_extractable_handle),
        ExpressionKind::StringInterpolation(segments) => segments.iter().any(|segment| {
            matches!(segment, hir::StringSegment::Value(value) if has_extractable_handle(value))
        }),
        ExpressionKind::MapConstruct { entries } => entries.iter().any(|entry| {
            has_extractable_handle(&entry.key) || has_extractable_handle(&entry.value)
        }),
        ExpressionKind::StructConstruct {
            fields,
            refinement_predicates,
            ..
        } => {
            fields.iter().any(has_extractable_handle)
                || refinement_predicates.iter().any(|chain| !chain.is_empty())
        }
        ExpressionKind::BitfieldConstruct { fields, .. } => {
            fields.iter().any(has_extractable_handle)
        }
        ExpressionKind::EnumConstruct { payloads, .. }
        | ExpressionKind::MachineConstruct { payloads, .. } => {
            payloads.iter().any(has_extractable_handle)
        }
        ExpressionKind::MachineTransition {
            source, payloads, ..
        } => has_extractable_handle(source) || payloads.iter().any(has_extractable_handle),
        ExpressionKind::ResultOk(value)
        | ExpressionKind::ResultFail(value)
        | ExpressionKind::OptionalSome(value) => has_extractable_handle(value),
        _ => false,
    }
}

fn can_snapshot_view(types: &TypeInterner, ty: TypeId) -> bool {
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

impl Builder<'_> {
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
        if let ExpressionKind::View(value) = &expression.kind {
            let mut lowered = expression.clone();
            lowered.kind = ExpressionKind::View(Box::new(self.lower_value(value)));
            return lowered;
        }
        if let ExpressionKind::Clone(value) = &expression.kind
            && has_extractable_handle(value)
        {
            let mut lowered = expression.clone();
            lowered.kind = ExpressionKind::Clone(Box::new(self.lower_value(value)));
            return lowered;
        }
        if let ExpressionKind::Run(value) = &expression.kind {
            let needs_snapshot = snapshotable_local(self.types, value);
            if has_extractable_handle(value) || needs_snapshot {
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
            && has_extractable_handle(value)
        {
            let mut lowered = expression.clone();
            lowered.kind = ExpressionKind::Join(Box::new(self.lower_value(value)));
            return lowered;
        }
        if let ExpressionKind::Coarsen(value) = &expression.kind
            && (has_extractable_handle(value) || snapshotable_local(self.types, value))
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
            && (has_extractable_handle(value) || snapshotable_local(self.types, value))
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
            && has_extractable_handle(base)
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
            && has_extractable_handle(value)
        {
            let mut lowered = expression.clone();
            lowered.kind = ExpressionKind::StateIs {
                value: Box::new(self.lower_value(value)),
                state: *state,
            };
            return lowered;
        }
        if let ExpressionKind::Unary { op, value } = &expression.kind
            && has_extractable_handle(value)
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
            && (has_extractable_handle(left) || has_extractable_handle(right))
        {
            let left_value = self.lower_value(left);
            if !has_extractable_handle(right) {
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
            && (has_extractable_handle(left) || has_extractable_handle(right))
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
            intrinsic: hir::IntrinsicId::TypeConstructFinish,
            refinement_predicates,
            ..
        } = &expression.kind
            && refinement_predicates.iter().any(|chain| !chain.is_empty())
        {
            return self.lower_refinement_builder_finish(expression, refinement_predicates);
        }
        if let ExpressionKind::ListConstruct { elements } = &expression.kind
            && elements.iter().any(has_extractable_handle)
            && let Some(elements) =
                self.lower_ordered_owned_values(elements, &(0..elements.len()).collect::<Vec<_>>())
        {
            let mut lowered = expression.clone();
            lowered.kind = ExpressionKind::ListConstruct { elements };
            return lowered;
        }
        if let ExpressionKind::StringInterpolation(segments) = &expression.kind
            && segments.iter().any(|segment| {
                matches!(segment, hir::StringSegment::Value(value) if has_extractable_handle(value))
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
            && entries.iter().any(|entry| {
                has_extractable_handle(&entry.key) || has_extractable_handle(&entry.value)
            })
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
            && fields.iter().any(has_extractable_handle)
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
            && fields.iter().any(has_extractable_handle)
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
            && payloads.iter().any(has_extractable_handle)
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
            && payloads.iter().any(has_extractable_handle)
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
            && (has_extractable_handle(source) || payloads.iter().any(has_extractable_handle))
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
            && has_extractable_handle(value)
        {
            let mut lowered = expression.clone();
            lowered.kind = ExpressionKind::ResultOk(Box::new(self.lower_value(value)));
            return lowered;
        }
        if let ExpressionKind::ResultFail(value) = &expression.kind
            && has_extractable_handle(value)
        {
            let mut lowered = expression.clone();
            lowered.kind = ExpressionKind::ResultFail(Box::new(self.lower_value(value)));
            return lowered;
        }
        if let ExpressionKind::OptionalSome(value) = &expression.kind
            && has_extractable_handle(value)
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
            args,
            evaluation_order,
        } = &expression.kind
            && args.iter().any(has_extractable_handle)
            && args.iter().enumerate().all(|(index, arg)| {
                !crate::move_values::intrinsic_borrows(*intrinsic, index, arg)
                    || can_snapshot_view(self.types, arg.ty)
                    || stable_deferred_view(&self.locals, arg)
            })
        {
            let inputs = args
                .iter()
                .enumerate()
                .map(|(index, arg)| {
                    if crate::move_values::intrinsic_borrows(*intrinsic, index, arg)
                        && crate::move_values::is_linear(self.types, arg.ty)
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
            if let Some(args) = self.lower_ordered_owned_values(&inputs, evaluation_order) {
                let mut lowered = expression.clone();
                lowered.kind = ExpressionKind::Intrinsic {
                    intrinsic: *intrinsic,
                    type_arguments: type_arguments.clone(),
                    reflection_arguments: reflection_arguments.clone(),
                    refinement_predicates: refinement_predicates.clone(),
                    args,
                    evaluation_order: evaluation_order.clone(),
                };
                return lowered;
            }
        }
        if let ExpressionKind::ActorSpawn {
            actor_type,
            args,
            evaluation_order,
            constructor,
        } = &expression.kind
            && args.iter().any(has_extractable_handle)
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
            && (has_extractable_handle(actor) || args.iter().any(has_extractable_handle))
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
        } = &expression.kind
            && (has_extractable_handle(callee) || args.iter().any(has_extractable_handle))
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
            && valid_ordered_owned_values(self.types, &self.locals, &inputs, evaluation_order)
        {
            let args = self
                .lower_ordered_owned_values(&inputs, evaluation_order)
                .expect("validated indirect argument order");
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
            };
            return lowered;
        }
        if let ExpressionKind::Call {
            function,
            args,
            evaluation_order,
        } = &expression.kind
            && args.iter().any(has_extractable_handle)
            && let Some(modes) = self.function_param_modes.get(function)
            && let Some(inputs) = call_staging_inputs(args, modes)
            && let Some(args) = self.lower_ordered_owned_values(&inputs, evaluation_order)
        {
            let mut lowered = expression.clone();
            lowered.kind = ExpressionKind::Call {
                function: *function,
                args,
                evaluation_order: evaluation_order.clone(),
            };
            return lowered;
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

    fn lower_refinement_check(
        &mut self,
        input: Expression,
        predicate: &hir::RefinementPredicate,
        span: Span,
    ) -> (LocalId, LocalId) {
        let error_text = self.temporary(TypeInterner::STRING, span);
        self.push(
            StatementKind::CheckRefinement {
                local: error_text,
                call: Expression {
                    kind: ExpressionKind::Call {
                        function: predicate.function,
                        args: vec![input],
                        evaluation_order: vec![0],
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
            assert!(matches!(call.kind, ExpressionKind::Call { .. }));
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
                    .map(move |statement| (block.id, statement))
            })
            .filter_map(|(block, statement)| match &statement.kind {
                StatementKind::Let { value, .. }
                    if matches!(value.kind, ExpressionKind::DisplayResult(_)) =>
                {
                    Some((block, value))
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(guards.len(), 1);
        let (block, guard) = guards[0];
        assert_ne!(block, function.entry);
        let ExpressionKind::DisplayResult(call) = &guard.kind else {
            unreachable!();
        };
        assert!(matches!(call.kind, ExpressionKind::Call { .. }));
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
            hir::validate_local_view_initializer(lowered, source, metadata.ty, &types).unwrap();
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
            let program = lower(&hir, &types).expect("structural HIR lowering");
            validate(&program).expect("malformed alias remains structurally valid MIR");
            let error = crate::move_values::MoveValuePlan::analyze(
                &program,
                inspected_handler_function(&program),
                &types,
            )
            .unwrap_err();
            assert!(error.contains("stable backing local"), "{invalid}: {error}");
        }
    }

    #[test]
    fn handler_call_staging_preserves_bare_local_alias_view_parameters() {
        let (program, types) = lower_handler_source(
            r#"namespace app
function take(view values: list[int64], amount: int64) returns int64:
    return amount
function inspect() returns int64:
    list[int64] source = list(1)
    list[int64] borrowed = view source
    list[int64] forwarded = borrowed
    int64 answer = take(forwarded, (none handle: default 3))
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
        assert!(
            function
                .blocks
                .iter()
                .flat_map(|block| &block.statements)
                .any(|statement| {
                    matches!(&statement.kind, StatementKind::Let { value, .. }
                if matches!(&value.kind, ExpressionKind::Clone(view)
                    if matches!(&view.kind, ExpressionKind::View(local)
                        if matches!(local.kind, ExpressionKind::Local(id) if id == forwarded))))
                })
        );
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
    int64 answer = callable(borrowed, (none handle: default 4))
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
    fn handler_call_staging_snapshots_copy_owned_view_carriers() {
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
        for name in ["borrowed", "callable"] {
            let alias = function
                .locals
                .iter()
                .find(|local| local.name == name)
                .unwrap()
                .id;
            assert!(
                function
                    .blocks
                    .iter()
                    .flat_map(|block| &block.statements)
                    .any(|statement| {
                        matches!(&statement.kind, StatementKind::Let { value, .. }
                    if matches!(&value.kind, ExpressionKind::Clone(view)
                        if matches!(&view.kind, ExpressionKind::View(local)
                            if matches!(local.kind, ExpressionKind::Local(id) if id == alias))))
                    }),
                "{name}"
            );
        }
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
    return take(borrowed, (none handle: default 3))
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
        let program = lower(&hir, &checked.interner).unwrap();
        let error = crate::move_values::MoveValuePlan::analyze(
            &program,
            inspected_handler_function(&program),
            &checked.interner,
        )
        .unwrap_err();
        assert!(
            error.contains("cannot move borrowed native place"),
            "{error}"
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
