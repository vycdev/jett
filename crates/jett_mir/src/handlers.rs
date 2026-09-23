//! Extract source result/optional handlers into ordinary MIR CFG edges.
use super::*;
use jett_hir::{ExpressionKind, HandleKind};
use jett_types::TypeInterner;

fn has_extractable_handle(expression: &Expression) -> bool {
    match &expression.kind {
        ExpressionKind::Handle {
            kind: HandleKind::Result | HandleKind::Optional,
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
                    matches!(&arg.kind, ExpressionKind::View(inner) if !matches!(&inner.kind, ExpressionKind::Local(_) | ExpressionKind::Handle { kind: HandleKind::Result | HandleKind::Optional, .. }))
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
}
