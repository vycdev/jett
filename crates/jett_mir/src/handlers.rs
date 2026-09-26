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
        ExpressionKind::View(value) => has_extractable_handle(value),
        ExpressionKind::Unary { value, .. } => has_extractable_handle(value),
        ExpressionKind::Binary { left, right, .. } => {
            has_extractable_handle(left) || has_extractable_handle(right)
        }
        ExpressionKind::Call { args, .. } => args.iter().any(has_extractable_handle),
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
            | Type::Function { .. } => true,
            Type::List(inner)
            | Type::Set(inner)
            | Type::Optional(inner)
            | Type::Secret(inner)
            | Type::Refinement { base: inner, .. } => supported(types, *inner, seen),
            Type::Map(key, value) | Type::Result(key, value) => {
                supported(types, *key, seen) && supported(types, *value, seen)
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
            | Type::TypeConstruction
            | Type::Never
            | Type::Error => false,
        }
    }

    supported(types, ty, &mut std::collections::HashSet::new())
}

fn valid_ordered_owned_values(
    types: &TypeInterner,
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
            !matches!(value.kind, ExpressionKind::View(_)) || can_snapshot_view(types, value.ty)
        })
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
            mutable: true,
            span,
        });
        id
    }

    fn lower_ordered_owned_values(
        &mut self,
        values: &[Expression],
        order: &[usize],
    ) -> Option<Vec<Expression>> {
        if !valid_ordered_owned_values(self.types, values, order) {
            return None;
        }
        let mut lowered = values.to_vec();
        for &index in order {
            let mut value = self.lower_value(&values[index]);
            if matches!(values[index].kind, ExpressionKind::View(_))
                && crate::move_values::is_linear(self.types, value.ty)
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
        if let ExpressionKind::View(value) = &expression.kind {
            let mut lowered = expression.clone();
            lowered.kind = ExpressionKind::View(Box::new(self.lower_value(value)));
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
                !crate::move_values::intrinsic_borrows(*intrinsic, index)
                    || can_snapshot_view(self.types, arg.ty)
            })
        {
            let inputs = args
                .iter()
                .enumerate()
                .map(|(index, arg)| {
                    if crate::move_values::intrinsic_borrows(*intrinsic, index)
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
        if let ExpressionKind::IndirectCall {
            callee,
            args,
            evaluation_order,
        } = &expression.kind
            && (has_extractable_handle(callee) || args.iter().any(has_extractable_handle))
            && !matches!(callee.kind, ExpressionKind::View(_))
            && valid_ordered_owned_values(self.types, args, evaluation_order)
        {
            let args = self
                .lower_ordered_owned_values(args, evaluation_order)
                .expect("validated indirect argument order");
            // Source evaluates call arguments before resolving a function value
            // held in a mutable local. A handler may rebind that local.
            let callee_value = self.lower_value(callee);
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
            && let Some(args) = self.lower_ordered_owned_values(args, evaluation_order)
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
        let value = self.lower_value(target);
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
            let passed = self.temporary(TypeInterner::BOOL, span);
            self.push(
                StatementKind::Let {
                    local: passed,
                    value: Expression {
                        kind: ExpressionKind::Call {
                            function: predicate.function,
                            args: vec![input],
                            evaluation_order: vec![0],
                        },
                        ty: TypeInterner::BOOL,
                        span,
                    },
                },
                span,
            );
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
                            kind: ExpressionKind::String(format!(
                                "refinement type constraint failed for '{}'",
                                predicate.type_name
                            )),
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
                    let passed = self.temporary(TypeInterner::BOOL, span);
                    self.push(
                        StatementKind::Let {
                            local: passed,
                            value: Expression {
                                kind: ExpressionKind::Call {
                                    function: predicate.function,
                                    args: vec![self.refinement_predicate_input(
                                        *binding,
                                        variants[variant_index][index],
                                        predicate,
                                        span,
                                    )],
                                    evaluation_order: vec![0],
                                },
                                ty: TypeInterner::BOOL,
                                span,
                            },
                        },
                        span,
                    );
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
                                    kind: ExpressionKind::String(format!(
                                        "refinement type constraint failed for '{}'",
                                        predicate.type_name
                                    )),
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
                    let passed = self.temporary(TypeInterner::BOOL, span);
                    self.push(
                        StatementKind::Let {
                            local: passed,
                            value: Expression {
                                kind: ExpressionKind::Call {
                                    function: predicate.function,
                                    args: vec![self.refinement_predicate_input(
                                        value,
                                        *field_type,
                                        predicate,
                                        span,
                                    )],
                                    evaluation_order: vec![0],
                                },
                                ty: TypeInterner::BOOL,
                                span,
                            },
                        },
                        span,
                    );
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
                                    kind: ExpressionKind::String(format!(
                                        "refinement type constraint failed for '{}'",
                                        predicate.type_name
                                    )),
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
                let passed = self.temporary(TypeInterner::BOOL, span);
                self.push(
                    StatementKind::Let {
                        local: passed,
                        value: Expression {
                            kind: ExpressionKind::Call {
                                function: predicate.function,
                                args: vec![self.refinement_predicate_input(
                                    value,
                                    field_types[index],
                                    predicate,
                                    span,
                                )],
                                evaluation_order: vec![0],
                            },
                            ty: TypeInterner::BOOL,
                            span,
                        },
                    },
                    span,
                );
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
                                kind: ExpressionKind::String(format!(
                                    "refinement type constraint failed for '{}'",
                                    predicate.type_name
                                )),
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
                let passed = self.temporary(TypeInterner::BOOL, span);
                self.push(
                    StatementKind::Let {
                        local: passed,
                        value: Expression {
                            kind: ExpressionKind::Call {
                                function: predicate.function,
                                args: vec![input],
                                evaluation_order: vec![0],
                            },
                            ty: TypeInterner::BOOL,
                            span,
                        },
                    },
                    span,
                );
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
                                kind: ExpressionKind::String(format!(
                                    "refinement type constraint failed for '{}'",
                                    predicate.type_name
                                )),
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
}
