//! Materialize native list iterables once, before the loop backedge.
//! This slice yields copyable elements only. Move-only yielded views need
//! projected-place loans and are deliberately left unsupported.
use super::*;
use jett_hir::{BinaryOp, ExpressionKind};
use jett_types::{Type, TypeInterner};
fn local(id: LocalId, ty: TypeId, span: Span) -> Expression {
    Expression {
        kind: ExpressionKind::Local(id),
        ty,
        span,
    }
}
fn temporary(function: &mut Function, ty: TypeId, span: Span) -> LocalId {
    let id = LocalId::new(function.locals.len() as u32);
    function.locals.push(Local {
        id,
        name: format!("$iterator{}", id.index()),
        ty,
        mutable: true,
        span,
    });
    id
}
pub fn prepare_native_sequences(program: &mut Program, types: &TypeInterner) {
    for function in &mut program.functions {
        let count = function.blocks.len();
        for index in 0..count {
            let header = function.blocks[index].id;
            let TerminatorKind::ForEach {
                key,
                value: None,
                by_view,
                iterable,
                body,
                exit,
            } = function.blocks[index].terminator.kind.clone()
            else {
                continue;
            };
            if iterable.ty.index() as usize >= types.len() {
                continue;
            }
            let Type::List(element) = types.resolve(iterable.ty) else {
                continue;
            };
            let element = *element;
            if !matches!(
                types.resolve(element),
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
                    | Type::Bool
                    | Type::String
                    | Type::Nothing
            ) {
                continue;
            }
            // HIR lowering emits the preheader before the header; backedges
            // are later blocks. Require that structural preheader explicitly.
            let Some(preheader) = function.blocks[..index].iter().position(
                |b| matches!(b.terminator.kind, TerminatorKind::Goto(target) if target == header),
            ) else {
                continue;
            };
            let span = iterable.span;
            let cursor = temporary(function, TypeInterner::INT64, span);
            let length = temporary(function, TypeInterner::INT64, span);
            let value = if let ExpressionKind::View(inner) = &iterable.kind {
                *inner.clone()
            } else {
                iterable.clone()
            };
            let borrowed_local = if by_view {
                if let ExpressionKind::Local(id) = value.kind {
                    Some(id)
                } else {
                    None
                }
            } else {
                None
            };
            let mut init = Vec::new();
            let source = if let Some(id) = borrowed_local {
                id
            } else {
                let id = temporary(function, iterable.ty, span);
                init.push(Statement {
                    kind: StatementKind::Let { local: id, value },
                    span,
                });
                id
            };
            if by_view {
                init.push(Statement {
                    kind: StatementKind::IterationBorrow {
                        source,
                        token: cursor,
                        start: true,
                    },
                    span,
                });
            }
            init.push(Statement {
                kind: StatementKind::Let {
                    local: cursor,
                    value: Expression {
                        kind: ExpressionKind::Int(0),
                        ty: TypeInterner::INT64,
                        span,
                    },
                },
                span,
            });
            init.push(Statement {
                kind: StatementKind::SequenceLength {
                    source,
                    target: length,
                },
                span,
            });
            function.blocks[preheader].statements.extend(init);
            function.blocks[index].terminator.kind = TerminatorKind::Branch {
                condition: Expression {
                    kind: ExpressionKind::Binary {
                        left: Box::new(local(cursor, TypeInterner::INT64, span)),
                        op: BinaryOp::Less,
                        right: Box::new(local(length, TypeInterner::INT64, span)),
                    },
                    ty: TypeInterner::BOOL,
                    span,
                },
                then_block: body,
                else_block: exit,
            };
            let mut prefix = vec![
                Statement {
                    kind: StatementKind::SequenceGet {
                        source,
                        index: cursor,
                        target: key,
                    },
                    span,
                },
                Statement {
                    kind: StatementKind::Assign {
                        target: local(cursor, TypeInterner::INT64, span),
                        value: Expression {
                            kind: ExpressionKind::Binary {
                                left: Box::new(local(cursor, TypeInterner::INT64, span)),
                                op: BinaryOp::Add,
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
                },
            ];
            prefix.append(&mut function.blocks[body.index() as usize].statements);
            function.blocks[body.index() as usize].statements = prefix;
            if by_view {
                function.blocks[exit.index() as usize].statements.insert(
                    0,
                    Statement {
                        kind: StatementKind::IterationBorrow {
                            source,
                            token: cursor,
                            start: false,
                        },
                        span,
                    },
                );
            }
        }
    }
}
