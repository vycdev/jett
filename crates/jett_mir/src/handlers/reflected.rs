//! Two ordered passes over a checked reflected producer tree. The candidate is
//! immutable: projections own private snapshots, never replacement payloads.
use super::*;

#[derive(Clone, Copy)]
enum Pass {
    Preflight,
    Predicates,
}

fn needed(plan: &hir::ReflectedFieldPlan, pass: Pass, produced: bool) -> bool {
    use hir::ReflectedFieldAction as A;
    match (&plan.action, pass) {
        (A::Exact, _) => false,
        (A::CallableReturn(_), _) => false,
        (A::Predicates(predicates), Pass::Preflight) => produced && !predicates.is_empty(),
        (A::Unsupported(_), Pass::Predicates) => false,
        (A::Refine { base, predicates }, Pass::Preflight) => {
            needed(base, pass, produced) || (produced && !predicates.is_empty())
        }
        (A::List(child) | A::Set(child) | A::Optional(child), Pass::Predicates) => {
            needed(child, pass, produced)
        }
        (A::Map { key, value }, Pass::Predicates) => {
            needed(key, pass, produced) || needed(value, pass, produced)
        }
        (A::Result { ok, error }, Pass::Predicates) => {
            needed(ok, pass, produced) || needed(error, pass, produced)
        }
        _ => true,
    }
}

fn local(id: LocalId, ty: TypeId, span: Span) -> Expression {
    Expression {
        kind: ExpressionKind::Local(id),
        ty,
        span,
    }
}

impl Builder<'_> {
    pub(super) fn lower_reflected_plan(
        &mut self,
        candidate: LocalId,
        plan: &hir::ReflectedFieldPlan,
        span: Span,
    ) {
        self.reflected_pass(candidate, plan, Pass::Preflight, span);
        // The second traversal starts only after the entire structural pass.
        if self.open() {
            self.reflected_pass(candidate, plan, Pass::Predicates, span);
        }
    }

    fn reflected_pass(
        &mut self,
        source: LocalId,
        plan: &hir::ReflectedFieldPlan,
        pass: Pass,
        span: Span,
    ) {
        if !self.open() || !needed(plan, pass, self.reflected_produced_return) {
            return;
        }
        use hir::ReflectedFieldAction as A;
        match &plan.action {
            A::Exact => {}
            A::CallableReturn(_) => {}
            A::Unsupported(reason) => {
                self.push(
                    StatementKind::Evaluate(Expression {
                        kind: ExpressionKind::RuntimeFailure(reason.message().into()),
                        ty: TypeInterner::NOTHING,
                        span,
                    }),
                    span,
                );
                self.terminate(TerminatorKind::Unreachable, span);
            }
            A::Predicates(predicates) => {
                if matches!(pass, Pass::Preflight) {
                    self.push(
                        StatementKind::ReflectedValueReady {
                            source,
                            inspect_pending: true,
                        },
                        span,
                    );
                } else {
                    let source_type = if self.reflected_produced_return {
                        self.locals[source.index() as usize].ty
                    } else {
                        plan.requested_type
                    };
                    self.reflected_predicates(source, source_type, predicates, span);
                }
            }
            A::Refine { base, predicates } => {
                if self.reflected_produced_return
                    && matches!(pass, Pass::Preflight)
                    && !predicates.is_empty()
                {
                    self.push(
                        StatementKind::ReflectedValueReady {
                            source,
                            inspect_pending: true,
                        },
                        span,
                    );
                }
                if needed(base, pass, self.reflected_produced_return) {
                    let source_type = if self.reflected_produced_return {
                        self.locals[source.index() as usize].ty
                    } else {
                        plan.requested_type
                    };
                    let base_type = if self.reflected_produced_return {
                        base.source_type
                    } else {
                        base.requested_type
                    };
                    let snapshot = self.temporary(base_type, span);
                    self.push(
                        StatementKind::Let {
                            local: snapshot,
                            value: Expression {
                                kind: ExpressionKind::Coarsen(Box::new(Expression {
                                    kind: ExpressionKind::Clone(Box::new(local(
                                        source,
                                        source_type,
                                        span,
                                    ))),
                                    ty: source_type,
                                    span,
                                })),
                                ty: base_type,
                                span,
                            },
                        },
                        span,
                    );
                    self.reflected_pass(snapshot, base, pass, span);
                }
                if self.open() && matches!(pass, Pass::Predicates) {
                    let source_type = if self.reflected_produced_return {
                        self.locals[source.index() as usize].ty
                    } else {
                        plan.requested_type
                    };
                    self.reflected_predicates(source, source_type, predicates, span);
                }
            }
            A::List(child) => self.reflected_sequence(
                source,
                ReflectedContainerKind::List,
                &[(SequencePart::Element, child.as_ref())],
                pass,
                span,
            ),
            A::Set(child) => self.reflected_sequence(
                source,
                ReflectedContainerKind::Set,
                &[(SequencePart::Element, child.as_ref())],
                pass,
                span,
            ),
            A::Map { key, value } => self.reflected_sequence(
                source,
                ReflectedContainerKind::Map,
                &[
                    (SequencePart::Key, key.as_ref()),
                    (SequencePart::Value, value.as_ref()),
                ],
                pass,
                span,
            ),
            A::Optional(child) => self.reflected_sum(
                source,
                ReflectedContainerKind::Optional,
                child,
                None,
                pass,
                span,
            ),
            A::Result { ok, error } => self.reflected_sum(
                source,
                ReflectedContainerKind::Result,
                ok,
                Some(error),
                pass,
                span,
            ),
        }
    }

    fn reflected_sequence(
        &mut self,
        source: LocalId,
        kind: ReflectedContainerKind,
        children: &[(SequencePart, &hir::ReflectedFieldPlan)],
        pass: Pass,
        span: Span,
    ) {
        if matches!(pass, Pass::Preflight) {
            self.push(
                StatementKind::ReflectedContainerReady { source, kind },
                span,
            );
        }
        if !children
            .iter()
            .any(|(_, plan)| needed(plan, pass, self.reflected_produced_return))
        {
            return;
        }
        let cursor = self.temporary(TypeInterner::INT64, span);
        let length = self.temporary(TypeInterner::INT64, span);
        self.push(
            StatementKind::Let {
                local: cursor,
                value: Expression {
                    kind: ExpressionKind::Int(0),
                    ty: TypeInterner::INT64,
                    span,
                },
            },
            span,
        );
        self.push(
            StatementKind::SequenceLength {
                source: SequenceSource::Local(source),
                target: length,
            },
            span,
        );
        let header = self.new_block(span);
        let body = self.new_block(span);
        let exit = self.new_block(span);
        self.close_to(header, span);
        self.current = header;
        self.terminate(
            TerminatorKind::Branch {
                condition: Expression {
                    kind: ExpressionKind::Binary {
                        left: Box::new(local(cursor, TypeInterner::INT64, span)),
                        op: hir::BinaryOp::Less,
                        right: Box::new(local(length, TypeInterner::INT64, span)),
                    },
                    ty: TypeInterner::BOOL,
                    span,
                },
                then_block: body,
                else_block: exit,
            },
            span,
        );
        self.current = body;
        for (part, plan) in children {
            if !self.open() || !needed(plan, pass, self.reflected_produced_return) {
                continue;
            }
            let child_type = if self.reflected_produced_return {
                plan.source_type
            } else {
                plan.requested_type
            };
            let child = self.temporary(child_type, span);
            self.push(
                StatementKind::SequenceGet {
                    consume: false,
                    source: SequenceSource::Local(source),
                    index: cursor,
                    target: child,
                    part: *part,
                },
                span,
            );
            self.reflected_pass(child, plan, pass, span);
        }
        if self.open() {
            self.push(
                StatementKind::Assign {
                    target: local(cursor, TypeInterner::INT64, span),
                    value: Expression {
                        kind: ExpressionKind::Binary {
                            left: Box::new(local(cursor, TypeInterner::INT64, span)),
                            op: hir::BinaryOp::Add,
                            right: Box::new(Expression {
                                kind: ExpressionKind::Int(1),
                                ty: TypeInterner::INT64,
                                span,
                            }),
                        },
                        ty: TypeInterner::INT64,
                        span,
                    },
                },
                span,
            );
            self.close_to(header, span);
        }
        self.current = exit;
    }

    fn reflected_sum(
        &mut self,
        source: LocalId,
        kind: ReflectedContainerKind,
        ok: &hir::ReflectedFieldPlan,
        error: Option<&hir::ReflectedFieldPlan>,
        pass: Pass,
        span: Span,
    ) {
        if matches!(pass, Pass::Preflight) {
            self.push(
                StatementKind::ReflectedContainerReady { source, kind },
                span,
            );
        }
        if !needed(ok, pass, self.reflected_produced_return)
            && !error.is_some_and(|plan| needed(plan, pass, self.reflected_produced_return))
        {
            return;
        }
        let present = self.temporary(TypeInterner::BOOL, span);
        self.push(
            StatementKind::SumTag {
                source,
                target: present,
            },
            span,
        );
        let accepted = self.new_block(span);
        let failed = self.new_block(span);
        let continuation = self.new_block(span);
        self.terminate(
            TerminatorKind::Branch {
                condition: local(present, TypeInterner::BOOL, span),
                then_block: accepted,
                else_block: failed,
            },
            span,
        );
        for (block, child, success) in [(accepted, Some(ok), true), (failed, error, false)] {
            self.current = block;
            if let Some(child) =
                child.filter(|plan| needed(plan, pass, self.reflected_produced_return))
            {
                let source_type = self.locals[source.index() as usize].ty;
                let snapshot = self.temporary(source_type, span);
                self.push(
                    StatementKind::Let {
                        local: snapshot,
                        value: Expression {
                            kind: ExpressionKind::Clone(Box::new(local(source, source_type, span))),
                            ty: source_type,
                            span,
                        },
                    },
                    span,
                );
                let child_type = if self.reflected_produced_return {
                    child.source_type
                } else {
                    child.requested_type
                };
                let payload = self.temporary(child_type, span);
                self.push(
                    StatementKind::SumTake {
                        source: snapshot,
                        target: payload,
                        success,
                    },
                    span,
                );
                self.reflected_pass(payload, child, pass, span);
            }
            self.close_to(continuation, span);
        }
        self.current = continuation;
    }

    fn reflected_predicates(
        &mut self,
        source: LocalId,
        source_type: TypeId,
        predicates: &[hir::RefinementPredicate],
        span: Span,
    ) {
        for predicate in predicates {
            let input = self.refinement_predicate_input(source, source_type, predicate, span);
            let (error_text, passed) = self.lower_refinement_check(input, predicate, span);
            let accepted = self.new_block(span);
            let rejected = self.new_block(span);
            self.terminate(
                TerminatorKind::Branch {
                    condition: local(passed, TypeInterner::BOOL, span),
                    then_block: accepted,
                    else_block: rejected,
                },
                span,
            );
            self.current = rejected;
            self.push(
                StatementKind::Evaluate(Expression {
                    kind: ExpressionKind::RuntimeFailureMessage(Box::new(local(
                        error_text,
                        TypeInterner::STRING,
                        span,
                    ))),
                    ty: TypeInterner::NOTHING,
                    span,
                }),
                span,
            );
            self.terminate(TerminatorKind::Unreachable, span);
            self.current = accepted;
        }
    }
}
