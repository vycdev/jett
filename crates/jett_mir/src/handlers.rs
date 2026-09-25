//! Extract source result/optional handlers into ordinary MIR CFG edges.
use super::*;
use jett_hir::{ExpressionKind, HandleKind};
use jett_types::TypeInterner;

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
        ExpressionKind::Intrinsic { args, .. } => args.iter().any(has_extractable_handle),
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

fn is_plain_copy_scalar(ty: TypeId) -> bool {
    matches!(
        ty,
        TypeInterner::INT8
            | TypeInterner::INT16
            | TypeInterner::INT32
            | TypeInterner::INT64
            | TypeInterner::UINT8
            | TypeInterner::UINT16
            | TypeInterner::UINT32
            | TypeInterner::UINT64
            | TypeInterner::FLOAT32
            | TypeInterner::FLOAT64
            | TypeInterner::BOOL
    )
}

fn valid_ordered_owned_values(values: &[Expression], order: &[usize]) -> bool {
    // MIR has no borrowed temporary. Only copy scalars and retained strings can
    // safely snapshot a direct view before a later handler changes its source.
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
                || is_plain_copy_scalar(value.ty)
                || value.ty == TypeInterner::STRING
        })
}

impl Builder {
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
        if !valid_ordered_owned_values(values, order) {
            return None;
        }
        let mut lowered = values.to_vec();
        for &index in order {
            let value = self.lower_value(&values[index]);
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
            && is_plain_copy_scalar(left.ty)
            && (has_extractable_handle(left) || has_extractable_handle(right))
        {
            // Save the left value before extracting a handler from the right.
            // Its failure block may mutate locals that the left side reads.
            let left_value = self.lower_value(left);
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
        } = &expression.kind
            && payloads.iter().any(has_extractable_handle)
            && let Some(payloads) =
                self.lower_ordered_owned_values(payloads, &(0..payloads.len()).collect::<Vec<_>>())
        {
            let mut lowered = expression.clone();
            lowered.kind = ExpressionKind::EnumConstruct {
                enum_type: *enum_type,
                variant: *variant,
                payloads,
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
            args,
            evaluation_order,
        } = &expression.kind
            && args.iter().any(has_extractable_handle)
            && args.iter().enumerate().all(|(index, arg)| {
                !crate::move_values::intrinsic_borrows(*intrinsic, index)
                    || is_plain_copy_scalar(arg.ty)
                    || arg.ty == TypeInterner::STRING
            })
            && let Some(args) = self.lower_ordered_owned_values(args, evaluation_order)
        {
            let mut lowered = expression.clone();
            lowered.kind = ExpressionKind::Intrinsic {
                intrinsic: *intrinsic,
                type_arguments: type_arguments.clone(),
                reflection_arguments: reflection_arguments.clone(),
                args,
                evaluation_order: evaluation_order.clone(),
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
            && valid_ordered_owned_values(args, evaluation_order)
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
        {
            let unique_order = evaluation_order
                .iter()
                .copied()
                .collect::<std::collections::BTreeSet<_>>()
                .len()
                == args.len();
            if args.len() != evaluation_order.len()
                || !unique_order
                || evaluation_order.iter().any(|&index| index >= args.len())
                || args.iter().any(|arg| {
                    matches!(&arg.kind, ExpressionKind::View(inner) if !matches!(&inner.kind, ExpressionKind::Local(_) | ExpressionKind::Handle { kind: HandleKind::Result | HandleKind::Optional | HandleKind::Refinement { .. }, .. }))
                })
            {
                return expression.clone();
            }
            let mut lowered_args = args.clone();
            for &index in evaluation_order {
                let lowered = self.lower_value(&args[index]);
                if args.len() == 1 || matches!(lowered.kind, ExpressionKind::View(_)) {
                    lowered_args[index] = lowered;
                    continue;
                }
                let local = self.temporary(lowered.ty, lowered.span);
                let span = lowered.span;
                let ty = lowered.ty;
                self.push(
                    StatementKind::Let {
                        local,
                        value: lowered,
                    },
                    span,
                );
                lowered_args[index] = Expression {
                    kind: ExpressionKind::Local(local),
                    ty,
                    span,
                };
            }
            let mut lowered = expression.clone();
            lowered.kind = ExpressionKind::Call {
                function: *function,
                args: lowered_args,
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
