//! Extract source result/optional handlers into ordinary MIR CFG edges.
use super::*;
use jett_hir::{ExpressionKind, HandleKind};
use jett_types::TypeInterner;
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
