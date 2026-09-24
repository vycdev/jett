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
        ExpressionKind::Call { args, .. } => args.iter().any(has_extractable_handle),
        _ => false,
    }
}

impl Builder {
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
    pub(super) fn lower_value(&mut self, expression: &Expression) -> Expression {
        if let ExpressionKind::View(value) = &expression.kind {
            let mut lowered = expression.clone();
            lowered.kind = ExpressionKind::View(Box::new(self.lower_value(value)));
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
        if predicates.is_empty()
            || predicates
                .iter()
                .any(|predicate| predicate.input_type != target.ty)
        {
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
            let input = Expression {
                kind: ExpressionKind::Clone(Box::new(Expression {
                    kind: ExpressionKind::Local(source),
                    ty: target.ty,
                    span,
                })),
                ty: target.ty,
                span,
            };
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
}
