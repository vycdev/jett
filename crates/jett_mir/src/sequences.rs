//! Materialize native list iterables once, before the loop backedge.
//! Consuming iteration takes initialized element places. Move-only views need
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
            if element.index() as usize >= types.len() {
                continue;
            }
            if by_view
                && !matches!(
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
                )
            {
                continue;
            }
            // A preheader is the unique predecessor reachable from entry
            // without crossing this header. Backedges are dominated by the
            // header; disconnected blocks and numeric block order are irrelevant.
            let Ok(cfg) = ControlFlowGraph::analyze(function) else {
                continue;
            };
            let mut outside = vec![false; function.blocks.len()];
            let mut pending = vec![function.entry];
            while let Some(block) = pending.pop() {
                if block == header || outside[block.index() as usize] {
                    continue;
                }
                outside[block.index() as usize] = true;
                pending.extend_from_slice(cfg.successors(block));
            }
            let candidates = cfg
                .predecessors(header)
                .iter()
                .copied()
                .filter(|id| outside[id.index() as usize])
                .collect::<Vec<_>>();
            let [preheader] = candidates.as_slice() else {
                continue;
            };
            let preheader = preheader.index() as usize;
            if !matches!(function.blocks[preheader].terminator.kind, TerminatorKind::Goto(target) if target == header)
            {
                continue;
            }
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
                        consume: !by_view,
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
                // Handler default may bypass the designated loop exit. Split
                // every edge leaving this CFG region, ending only this token.
                let mut pending = vec![exit];
                while let Some(block) = pending.pop() {
                    if block == header || outside[block.index() as usize] {
                        continue;
                    }
                    outside[block.index() as usize] = true;
                    pending.extend_from_slice(cfg.successors(block));
                }
                let mut region = vec![false; outside.len()];
                let mut pending = vec![header];
                while let Some(block) = pending.pop() {
                    let i = block.index() as usize;
                    if outside[i] || region[i] {
                        continue;
                    }
                    region[i] = true;
                    pending.extend_from_slice(cfg.successors(block));
                }
                for i in 0..region.len() {
                    if !region[i] {
                        continue;
                    }
                    for &target in cfg.successors(function.blocks[i].id) {
                        if region[target.index() as usize] {
                            continue;
                        }
                        let end = BlockId(function.blocks.len() as u32);
                        function.blocks.push(BasicBlock {
                            id: end,
                            statements: vec![Statement {
                                kind: StatementKind::IterationBorrow {
                                    source,
                                    token: cursor,
                                    start: false,
                                },
                                span,
                            }],
                            terminator: Terminator {
                                kind: TerminatorKind::Goto(target),
                                span,
                            },
                        });
                        redirect_edge(&mut function.blocks[i].terminator.kind, target, end);
                    }
                }
            }
        }
    }
}

fn redirect_edge(kind: &mut TerminatorKind, old: BlockId, new: BlockId) {
    let replace = |id: &mut BlockId| {
        if *id == old {
            *id = new;
        }
    };
    match kind {
        TerminatorKind::Goto(id) => replace(id),
        TerminatorKind::Branch {
            then_block,
            else_block,
            ..
        } => {
            replace(then_block);
            replace(else_block);
        }
        TerminatorKind::ForEach { body, exit, .. } => {
            replace(body);
            replace(exit);
        }
        TerminatorKind::Switch {
            variants,
            otherwise,
            ..
        } => {
            for (_, id, _) in variants {
                replace(id);
            }
            if let Some(id) = otherwise {
                replace(id);
            }
        }
        TerminatorKind::ReflectedTypeDispatch {
            arms, otherwise, ..
        } => {
            for arm in arms {
                replace(&mut arm.target);
            }
            replace(otherwise);
        }
        TerminatorKind::Return(_) | TerminatorKind::Respond(_) | TerminatorKind::Unreachable => {}
    }
}
