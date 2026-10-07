use super::tests::{checked_support, lowered};
use super::*;

const SUPPORT: &str =
    include_str!("../../../jett_driver/tests/native_conformance/resource/resource_probe.jett");
const SOURCES: [&str; 39] = [
    include_str!(
        "../../../jett_driver/tests/native_conformance/resource/57_nested_view_optional_some_written.jett"
    ),
    include_str!(
        "../../../jett_driver/tests/native_conformance/resource/58_if_optional_written_some.jett"
    ),
    include_str!(
        "../../../jett_driver/tests/native_conformance/resource/59_if_optional_written_none.jett"
    ),
    include_str!(
        "../../../jett_driver/tests/native_conformance/resource/60_if_optional_bare_some.jett"
    ),
    include_str!(
        "../../../jett_driver/tests/native_conformance/resource/61_if_optional_bare_none.jett"
    ),
    include_str!(
        "../../../jett_driver/tests/native_conformance/resource/62_if_result_written_ok.jett"
    ),
    include_str!(
        "../../../jett_driver/tests/native_conformance/resource/63_if_result_written_fail.jett"
    ),
    include_str!(
        "../../../jett_driver/tests/native_conformance/resource/64_if_result_bare_ok.jett"
    ),
    include_str!(
        "../../../jett_driver/tests/native_conformance/resource/65_if_result_bare_fail.jett"
    ),
    include_str!(
        "../../../jett_driver/tests/native_conformance/resource/66_scope_optional_written_some.jett"
    ),
    include_str!(
        "../../../jett_driver/tests/native_conformance/resource/67_scope_optional_written_none.jett"
    ),
    include_str!(
        "../../../jett_driver/tests/native_conformance/resource/68_scope_optional_bare_some.jett"
    ),
    include_str!(
        "../../../jett_driver/tests/native_conformance/resource/69_scope_optional_bare_none.jett"
    ),
    include_str!(
        "../../../jett_driver/tests/native_conformance/resource/70_scope_result_written_ok.jett"
    ),
    include_str!(
        "../../../jett_driver/tests/native_conformance/resource/71_scope_result_written_fail.jett"
    ),
    include_str!(
        "../../../jett_driver/tests/native_conformance/resource/72_scope_result_bare_ok.jett"
    ),
    include_str!(
        "../../../jett_driver/tests/native_conformance/resource/73_scope_result_bare_fail.jett"
    ),
    include_str!(
        "../../../jett_driver/tests/native_conformance/resource/74_while_optional_written_some.jett"
    ),
    include_str!(
        "../../../jett_driver/tests/native_conformance/resource/75_while_optional_written_none.jett"
    ),
    include_str!(
        "../../../jett_driver/tests/native_conformance/resource/76_while_optional_bare_some.jett"
    ),
    include_str!(
        "../../../jett_driver/tests/native_conformance/resource/77_while_optional_bare_none.jett"
    ),
    include_str!(
        "../../../jett_driver/tests/native_conformance/resource/78_while_result_written_ok.jett"
    ),
    include_str!(
        "../../../jett_driver/tests/native_conformance/resource/79_while_result_written_fail.jett"
    ),
    include_str!(
        "../../../jett_driver/tests/native_conformance/resource/80_while_result_bare_ok.jett"
    ),
    include_str!(
        "../../../jett_driver/tests/native_conformance/resource/81_while_result_bare_fail.jett"
    ),
    include_str!(
        "../../../jett_driver/tests/native_conformance/resource/82_if_owned_optional_reuse.jett"
    ),
    include_str!(
        "../../../jett_driver/tests/native_conformance/resource/83_if_owned_result_reuse.jett"
    ),
    include_str!(
        "../../../jett_driver/tests/native_conformance/resource/84_scope_owned_optional_reuse.jett"
    ),
    include_str!(
        "../../../jett_driver/tests/native_conformance/resource/85_scope_owned_result_reuse.jett"
    ),
    include_str!(
        "../../../jett_driver/tests/native_conformance/resource/86_while_owned_optional_reuse.jett"
    ),
    include_str!(
        "../../../jett_driver/tests/native_conformance/resource/87_while_owned_result_reuse.jett"
    ),
    include_str!(
        "../../../jett_driver/tests/native_conformance/resource/88_if_sibling_optional_bindings.jett"
    ),
    include_str!(
        "../../../jett_driver/tests/native_conformance/resource/89_scope_sibling_result_bindings.jett"
    ),
    include_str!(
        "../../../jett_driver/tests/native_conformance/resource/90_scope_optional_written_borrow_fail.jett"
    ),
    include_str!(
        "../../../jett_driver/tests/native_conformance/resource/91_scope_result_bare_borrow_fail.jett"
    ),
    include_str!(
        "../../../jett_driver/tests/native_conformance/resource/92_while_optional_bare_borrow_fail.jett"
    ),
    include_str!(
        "../../../jett_driver/tests/native_conformance/resource/93_while_result_written_borrow_fail.jett"
    ),
    include_str!(
        "../../../jett_driver/tests/native_conformance/resource/94_while_owned_optional_break.jett"
    ),
    include_str!(
        "../../../jett_driver/tests/native_conformance/resource/95_while_owned_result_continue.jett"
    ),
];

fn borrowed_function(program: &Program) -> &Function {
    program
        .functions
        .iter()
        .find(|function| {
            function.identity.declaration.namespace == "app"
                && function
                    .resource_lowering
                    .as_ref()
                    .is_some_and(|witness| !witness.borrowed_sums.is_empty())
        })
        .expect("fixture contains its original borrowed declaration")
}

fn borrowed_function_mut(program: &mut Program) -> &mut Function {
    program
        .functions
        .iter_mut()
        .find(|function| {
            function.identity.declaration.namespace == "app"
                && function
                    .resource_lowering
                    .as_ref()
                    .is_some_and(|witness| !witness.borrowed_sums.is_empty())
        })
        .expect("fixture contains its original borrowed declaration")
}

fn exits<'a>(
    function: &Function,
    plan: &'a ResourceFunctionPlan,
) -> Vec<(BlockId, ResourcePosition, &'a ResourceLexicalExitPlan)> {
    let mut exits = Vec::new();
    for block in &function.blocks {
        let positions = (0..block.statements.len())
            .map(ResourcePosition::Statement)
            .chain(std::iter::once(ResourcePosition::Terminator));
        for position in positions {
            if let Some(current) = function
                .resource_lexical_exit(block.id, position)
                .expect("constructor-owned lexical exit query")
            {
                let exit = plan
                    .lexical_exit(block.id, position)
                    .expect("fresh custody analysis retains the exact lexical exit");
                assert_eq!(exit.kind(), current.kind());
                for id in exit.operation_ids() {
                    let operation = &plan.operations()[id.index()];
                    assert_eq!(operation.id(), *id);
                    assert_eq!(operation.site().block(), block.id);
                    assert_eq!(operation.site().position(), position);
                    assert!(matches!(
                        operation.role(),
                        ResourceOperationRole::EndBorrow { .. }
                            | ResourceOperationRole::EndSumBorrow { .. }
                    ));
                }
                let distinct = exit
                    .clear_locals()
                    .iter()
                    .map(|local| local.index())
                    .collect::<BTreeSet<_>>();
                assert_eq!(distinct.len(), exit.clear_locals().len());
                assert!(
                    exit.clear_locals()
                        .iter()
                        .all(|local| function.local(*local).is_some())
                );
                exits.push((block.id, position, exit));
            } else {
                assert!(plan.lexical_exit(block.id, position).is_none());
            }
        }
    }
    exits
}

fn ends_loan(
    plan: &ResourceFunctionPlan,
    exit: &ResourceLexicalExitPlan,
    loan: ResourceLoanId,
) -> bool {
    exit.operation_ids().iter().any(|id| {
        matches!(plan.operations()[id.index()].role(), ResourceOperationRole::EndBorrow { loan: current } | ResourceOperationRole::EndSumBorrow { loan: current } if *current == loan)
    })
}

// A lexical boundary may share a block with later statements or precede a loop
// backedge. Search instruction boundaries, rather than comparing block IDs.
fn site_can_reach(function: &Function, start: ResourceSite, end: ResourceSite) -> bool {
    let cfg = ControlFlowGraph::analyze(function).unwrap();
    let position = |site: ResourceSite| match site.position() {
        ResourcePosition::Statement(index) => index,
        ResourcePosition::Terminator => function.blocks[site.block().index() as usize]
            .statements
            .len(),
    };
    let mut pending = vec![(start.block(), position(start) + 1)];
    let mut seen = BTreeSet::new();
    while let Some((block, floor)) = pending.pop() {
        if !seen.insert((block.index(), floor)) {
            continue;
        }
        if block == end.block() && position(end) >= floor {
            return true;
        }
        pending.extend(cfg.successors(block).iter().map(|target| (*target, 0)));
    }
    false
}
fn operation_precedes(
    function: &Function,
    start: &ResourceOperation,
    end: &ResourceOperation,
) -> bool {
    if start.site() == end.site() {
        start.id().index() < end.id().index()
    } else {
        site_can_reach(function, start.site(), end.site())
    }
}
fn consumes_slot(role: &ResourceOperationRole, slot: ResourceOwnerSlotId) -> bool {
    match role {
        ResourceOperationRole::Transfer { source, .. } | ResourceOperationRole::SumTake { source, .. }
        | ResourceOperationRole::TakeFailureCompanion { source, .. } | ResourceOperationRole::Close { source, .. }
        | ResourceOperationRole::Drop { source, .. } | ResourceOperationRole::SumAdopt { source, .. } => *source == slot,
        ResourceOperationRole::Replace { destination, replacement, .. } => *destination == slot || *replacement == slot,
        ResourceOperationRole::SelfRebind { slot: current } => *current == slot,
        ResourceOperationRole::InvokeSourceFunction { operands, .. } | ResourceOperationRole::InvokeHook { operands, .. } => operands.iter().any(|operand| matches!(operand, ResourceCallOperand::Owned { slot: current, .. } if *current == slot)),
        _ => false,
    }
}

fn assert_forged_refused(mut program: Program, types: &TypeInterner, control: &str) {
    let function = borrowed_function_mut(&mut program);
    let witness = function.resource_lowering.as_mut().unwrap();
    witness.blocks = function.blocks.clone();
    witness.locals = function.locals.clone();
    assert!(
        validate_resource_ownership(&program, types).is_err(),
        "{control}"
    );
    assert!(
        !crate::sequences::prune::unreachable(borrowed_function_mut(&mut program)),
        "canonical {control}"
    );
}

#[test]
fn resource_lexical_borrows_shared_sources_keep_exact_exit_cleanup_after_canonical_pruning() {
    for release in [false, true] {
        let mut projections = 0;
        for (index, source) in SOURCES.into_iter().enumerate() {
            let fixture = index + 57;
            let checked = checked_support(source, SUPPORT, release);
            let types = &checked.checked().interner;
            let mut program = lowered(&checked);
            for function in &mut program.functions {
                assert!(
                    crate::sequences::prune::unreachable(function),
                    "Source{fixture} release={release}"
                );
            }
            let ownership = validate_resource_ownership(&program, types)
                .unwrap_or_else(|errors| panic!("Source{fixture} release={release}: {errors:?}"));
            let function = borrowed_function(&program);
            let plan = ownership.function(function.id).unwrap();
            let exits = exits(function, plan);
            assert!(!exits.is_empty(), "Source{fixture} release={release}");
            let rows = &function.resource_lowering.as_ref().unwrap().borrowed_sums;
            assert_eq!(
                rows.len(),
                if fixture == 88 || fixture == 89 { 2 } else { 1 }
            );
            projections += rows.len();
            for operation in plan.operations() {
                let ResourceOperationRole::ProjectSumView {
                    source: parent,
                    destination: child,
                    ..
                } = operation.role()
                else {
                    continue;
                };
                let row = function
                    .resource_borrowed_sum_projection(
                        operation.site().block(),
                        operation.site().position(),
                    )
                    .unwrap()
                    .unwrap();
                let cleanup = exits
                    .iter()
                    .filter(|(_, _, exit)| ends_loan(plan, exit, *child))
                    .collect::<Vec<_>>();
                assert!(!cleanup.is_empty(), "Source{fixture} child={child:?}");
                for (_, _, exit) in cleanup {
                    for local in [row.alias(), row.source(), row.output()] {
                        assert!(
                            exit.clear_locals().contains(&local),
                            "Source{fixture} stale local={local:?}"
                        );
                        assert!(function.is_view_local(local));
                        assert!(!plan.owner_slots().iter().any(|slot| matches!(slot.storage(), ResourceSlotStorage::Local { header } if header.id == local)));
                    }
                }
                if matches!(
                    plan.loans()[parent.index()].source(),
                    ResourceLoanSource::IncomingViewFormal { .. }
                ) {
                    assert!(!plan.operations().iter().any(|operation| matches!(operation.role(), ResourceOperationRole::EndSumBorrow { loan } if loan == parent)), "Source{fixture} must preserve its resident caller lease");
                }
            }
            let expected = match fixture {
                57 => ResourceLexicalExitKind::Return,
                94 => ResourceLexicalExitKind::Break,
                95 => ResourceLexicalExitKind::Continue,
                _ => ResourceLexicalExitKind::Fallthrough,
            };
            assert!(exits.iter().any(|(_, _, exit)| exit.kind() == expected && !exit.operation_ids().is_empty()), "Source{fixture} lacks {expected:?} cleanup");
        }
        assert_eq!(projections, 41);
    }
}

#[test]
fn resource_lexical_borrows_owned_backing_survives_child_first_scope_and_loop_exits() {
    for release in [false, true] {
        for fixture in [82, 83, 84, 85, 86, 87, 94, 95] {
            let checked = checked_support(SOURCES[fixture - 57], SUPPORT, release);
            let program = lowered(&checked);
            let ownership =
                validate_resource_ownership(&program, &checked.checked().interner).unwrap();
            let function = borrowed_function(&program);
            let plan = ownership.function(function.id).unwrap();
            let exits = exits(function, plan);
            for operation in plan.operations() {
                let ResourceOperationRole::ProjectSumView {
                    source: parent,
                    destination: child,
                    ..
                } = operation.role()
                else {
                    continue;
                };
                let ResourceLoanSource::Owner(backing_slot) = plan.loans()[parent.index()].source()
                else {
                    panic!("Source{fixture} must borrow its retained local sum")
                };
                let row = function
                    .resource_borrowed_sum_projection(
                        operation.site().block(),
                        operation.site().position(),
                    )
                    .unwrap()
                    .unwrap();
                assert!(
                    matches!(plan.owner_slots()[backing_slot.index()].storage(), ResourceSlotStorage::Local { header } if header.id == row.backing())
                );
                let normal = exits
                    .iter()
                    .find(|(_, _, exit)| {
                        ends_loan(plan, exit, *child)
                            && exit.kind() != ResourceLexicalExitKind::Return
                    })
                    .expect("original inner body retires normally before backing reuse");
                assert!(!normal.2.clear_locals().contains(&row.backing()));
                let child_end = normal.2.operation_ids().iter().position(|id| matches!(plan.operations()[id.index()].role(), ResourceOperationRole::EndBorrow { loan } if loan == child)).unwrap();
                let parent_end = normal.2.operation_ids().iter().position(|id| matches!(plan.operations()[id.index()].role(), ResourceOperationRole::EndSumBorrow { loan } if loan == parent)).expect("locally created parent shell lease retires");
                assert!(
                    child_end < parent_end,
                    "Source{fixture} child before parent"
                );
                let transfers = plan.operations().iter().filter(|operation| matches!(operation.role(), ResourceOperationRole::Transfer { source, .. } if *source == backing_slot)).collect::<Vec<_>>();
                assert_eq!(
                    transfers.len(),
                    1,
                    "Source{fixture} moves its original held shell exactly once"
                );
                let moved = transfers[0];
                let ResourceOperationRole::Transfer {
                    destination: shell, ..
                } = moved.role()
                else {
                    unreachable!()
                };
                let exit_site = ResourceSite {
                    function: function.id,
                    block: normal.0,
                    position: normal.1,
                };
                assert!(
                    site_can_reach(function, exit_site, moved.site()),
                    "Source{fixture} must reuse held after its actual lexical boundary"
                );
                let ResourcePosition::Statement(index) = moved.site().position() else {
                    panic!("Source{fixture} owning Handle begins at its exact source Let")
                };
                let ResourceSlotStorage::Local {
                    header: shell_header,
                } = plan.owner_slots()[shell.index()].storage()
                else {
                    panic!("Source{fixture} owning Handle has a dedicated generated source holder")
                };
                assert!(
                    matches!(&function.blocks[moved.site().block().index() as usize].statements[index].kind, StatementKind::Let { local, value } if *local == shell_header.id && matches!(value.kind, hir::ExpressionKind::Local(backing) if backing == row.backing())),
                    "Source{fixture} transfers that exact original backing, without a snapshot or replacement"
                );
                assert_eq!(
                    plan.owner_slots()[shell.index()].shape(),
                    plan.owner_slots()[backing_slot.index()].shape()
                );
                for candidate in plan.operations() {
                    if candidate.id() != moved.id()
                        && consumes_slot(candidate.role(), backing_slot)
                        && operation_precedes(function, candidate, moved)
                    {
                        panic!(
                            "Source{fixture} consumes backing={} before its later unwrap at operation={}",
                            backing_slot.index(),
                            candidate.id().index()
                        );
                    }
                }
                let takes = plan.operations().iter().filter(|operation| matches!(operation.role(), ResourceOperationRole::SumTake { source, success: true, .. } if source == shell)).collect::<Vec<_>>();
                assert_eq!(
                    takes.len(),
                    1,
                    "Source{fixture} extracts the transferred shell once"
                );
                let take = takes[0];
                assert!(operation_precedes(function, moved, take));
                let ResourceOperationRole::SumTake {
                    destination: payload,
                    ..
                } = take.role()
                else {
                    unreachable!()
                };
                let mut slot = *payload;
                let mut previous = take;
                let mut visited = BTreeSet::new();
                loop {
                    assert!(
                        visited.insert(slot.index()),
                        "Source{fixture} final payload transfer chain is acyclic"
                    );
                    let closes = plan.operations().iter().filter(|operation| match operation.role() {
                        ResourceOperationRole::Close { source, .. } => *source == slot,
                        ResourceOperationRole::InvokeSourceFunction { function: callee, operands, .. } => {
                            let declaration = &program.functions[callee.index() as usize].identity.declaration;
                            declaration.namespace == "resource_probe" && declaration.name == "close" && operands.iter().any(|operand| matches!(operand, ResourceCallOperand::Owned { slot: current, .. } if *current == slot))
                        }
                        _ => false,
                    }).collect::<Vec<_>>();
                    if !closes.is_empty() {
                        assert_eq!(
                            closes.len(),
                            1,
                            "Source{fixture} closes the extracted payload exactly once"
                        );
                        assert!(operation_precedes(function, previous, closes[0]));
                        break;
                    }
                    let next = plan.operations().iter().filter(|operation| matches!(operation.role(), ResourceOperationRole::Transfer { source, .. } if *source == slot)).collect::<Vec<_>>();
                    assert_eq!(
                        next.len(),
                        1,
                        "Source{fixture} carries payload={} into one exact owning continuation",
                        slot.index()
                    );
                    assert!(operation_precedes(function, previous, next[0]));
                    let ResourceOperationRole::Transfer { destination, .. } = next[0].role() else {
                        unreachable!()
                    };
                    slot = *destination;
                    previous = next[0];
                }
                if matches!(fixture, 86 | 87 | 94 | 95) {
                    let block = &function.blocks[normal.0.index() as usize];
                    assert!(
                        matches!(block.terminator.kind, TerminatorKind::Goto(_)),
                        "Source{fixture} cleanup precedes its backedge/break/continue"
                    );
                }
            }
        }
    }
}

#[test]
fn resource_lexical_borrows_sibling_names_keep_separate_original_keys_and_backings() {
    for release in [false, true] {
        for fixture in [88, 89] {
            let checked = checked_support(SOURCES[fixture - 57], SUPPORT, release);
            let program = lowered(&checked);
            let ownership =
                validate_resource_ownership(&program, &checked.checked().interner).unwrap();
            let function = borrowed_function(&program);
            let rows = &function.resource_lowering.as_ref().unwrap().borrowed_sums;
            assert_eq!(rows.len(), 2);
            assert_eq!(
                function.local(rows[0].alias()).unwrap().name,
                function.local(rows[1].alias()).unwrap().name
            );
            assert_ne!(rows[0].alias(), rows[1].alias());
            assert_ne!(rows[0].backing(), rows[1].backing());
            let plan = ownership.function(function.id).unwrap();
            let exits = exits(function, plan);
            for (index, row) in rows.iter().enumerate() {
                assert!(exits.iter().any(|(_, _, exit)| exit.kind()
                    == ResourceLexicalExitKind::Fallthrough
                    && exit.clear_locals().contains(&row.alias())
                    && !exit.clear_locals().contains(&rows[1 - index].alias())));
            }
        }
    }
}

#[test]
fn resource_lexical_borrows_return_cleanup_is_after_the_observation_operand() {
    for release in [false, true] {
        let checked = checked_support(SOURCES[0], SUPPORT, release);
        let program = lowered(&checked);
        let ownership = validate_resource_ownership(&program, &checked.checked().interner).unwrap();
        let function = borrowed_function(&program);
        let plan = ownership.function(function.id).unwrap();
        let exits = exits(function, plan);
        let child = plan
            .operations()
            .iter()
            .find_map(|operation| match operation.role() {
                ResourceOperationRole::ProjectSumView { destination, .. } => Some(*destination),
                _ => None,
            })
            .unwrap();
        assert!(plan.operations().iter().any(|operation| matches!(operation.role(), ResourceOperationRole::BoundedBorrowUse { loan, .. } if *loan == child)));
        let returns = exits
            .iter()
            .filter(|(_, _, exit)| {
                exit.kind() == ResourceLexicalExitKind::Return && ends_loan(plan, exit, child)
            })
            .collect::<Vec<_>>();
        assert!(!returns.is_empty());
        for (block, position, _) in returns {
            assert_eq!(*position, ResourcePosition::Terminator);
            assert!(matches!(
                function.blocks[block.index() as usize].terminator.kind,
                TerminatorKind::Return(_)
            ));
        }
    }
}

#[test]
fn resource_lexical_borrows_refuse_copied_missing_and_swapped_exit_nodes_after_general_reseal() {
    for release in [false, true] {
        let checked = checked_support(SOURCES[31], SUPPORT, release);
        let types = &checked.checked().interner;
        let baseline = lowered(&checked);
        let function = borrowed_function(&baseline);
        let nodes = function
            .blocks
            .iter()
            .flat_map(|block| {
                block
                    .statements
                    .iter()
                    .enumerate()
                    .filter_map(move |(index, statement)| {
                        matches!(statement.kind, StatementKind::ResourceLexicalExit(_))
                            .then_some((block.id, index))
                    })
            })
            .collect::<Vec<_>>();
        assert!(nodes.len() >= 2);
        for mutation in 0..4 {
            let mut forged = baseline.clone();
            let function = borrowed_function_mut(&mut forged);
            let (block, index) = nodes[0];
            match mutation {
                0 => {
                    let statement =
                        function.blocks[block.index() as usize].statements[index].clone();
                    function.blocks.push(BasicBlock {
                        id: BlockId(function.blocks.len() as u32),
                        statements: vec![statement],
                        terminator: Terminator {
                            kind: TerminatorKind::Unreachable,
                            span: function.span,
                        },
                    });
                }
                1 => {
                    function.blocks[block.index() as usize]
                        .statements
                        .remove(index);
                }
                2 => {
                    let other =
                        &function.blocks[nodes[1].0.index() as usize].statements[nodes[1].1].kind;
                    let StatementKind::ResourceLexicalExit(other) = other else {
                        unreachable!()
                    };
                    let other = *other;
                    function.blocks[block.index() as usize].statements[index].kind =
                        StatementKind::ResourceLexicalExit(other);
                }
                3 => {
                    let returning = function
                        .blocks
                        .iter_mut()
                        .find(|block| matches!(block.terminator.kind, TerminatorKind::Return(_)))
                        .unwrap();
                    returning.terminator.kind = TerminatorKind::Goto(function.entry);
                }
                _ => unreachable!(),
            }
            assert_forged_refused(
                forged,
                types,
                &format!("exit mutation={mutation} release={release}"),
            );
        }
    }
}

#[test]
fn resource_lexical_borrows_refuse_duplicate_original_keys_sibling_backing_swaps_and_disconnected_takes()
 {
    for release in [false, true] {
        for fixture in [88, 89] {
            let checked = checked_support(SOURCES[fixture - 57], SUPPORT, release);
            let types = &checked.checked().interner;
            let baseline = lowered(&checked);
            let function = borrowed_function(&baseline);
            let rows = &function.resource_lowering.as_ref().unwrap().borrowed_sums;
            let source = rows[0].source();
            let other_backing = rows[1].backing();
            let project = rows[0].success_site();
            for mutation in 0..4 {
                let mut forged = baseline.clone();
                let function = borrowed_function_mut(&mut forged);
                match mutation {
                    0 => {
                        let witness = function.resource_lowering.as_mut().unwrap();
                        witness.borrowed_sums[1] = witness.borrowed_sums[0].clone();
                    }
                    1 => {
                        function
                            .resource_lowering
                            .as_mut()
                            .unwrap()
                            .borrowed_sums
                            .pop();
                    }
                    2 => {
                        let statement = function.blocks.iter_mut().flat_map(|block| &mut block.statements).find(|statement| matches!(statement.kind, StatementKind::Let { local, .. } if local == source)).unwrap();
                        let StatementKind::Let { value, .. } = &mut statement.kind else {
                            unreachable!()
                        };
                        let hir::ExpressionKind::View(inner) = &mut value.kind else {
                            unreachable!()
                        };
                        inner.kind = hir::ExpressionKind::Local(other_backing);
                    }
                    3 => {
                        let ResourcePosition::Statement(index) = project.position() else {
                            unreachable!()
                        };
                        let statement = function.blocks[project.block().index() as usize]
                            .statements[index]
                            .clone();
                        function.blocks.push(BasicBlock {
                            id: BlockId(function.blocks.len() as u32),
                            statements: vec![statement],
                            terminator: Terminator {
                                kind: TerminatorKind::Unreachable,
                                span: function.span,
                            },
                        });
                    }
                    _ => unreachable!(),
                }
                assert_forged_refused(
                    forged,
                    types,
                    &format!("Source{fixture} key mutation={mutation} release={release}"),
                );
            }
        }
    }
}

#[test]
fn resource_lexical_borrows_refuse_changed_exit_membership_with_unchanged_nodes_and_sites() {
    for release in [false, true] {
        let checked = checked_support(SOURCES[31], SUPPORT, release);
        let types = &checked.checked().interner;
        let baseline = lowered(&checked);
        let witness = borrowed_function(&baseline)
            .resource_lowering
            .as_ref()
            .unwrap();
        let candidates = witness
            .lexical_exits
            .iter()
            .enumerate()
            .filter_map(|(index, exit)| {
                (exit.kind() == ResourceLexicalExitKind::Fallthrough
                    && exit.scopes.iter().any(|scope| !scope.active.is_empty()))
                .then_some(index)
            })
            .collect::<Vec<_>>();
        assert_eq!(candidates.len(), 2);
        let [left, right] = [candidates[0], candidates[1]];
        for mutation in 0..3 {
            let mut forged = baseline.clone();
            let function = borrowed_function_mut(&mut forged);
            let original_blocks = function.blocks.clone();
            let witness = function.resource_lowering.as_mut().unwrap();
            let independent = witness.lexical_seal.clone();
            match mutation {
                0 => {
                    let left_scopes = witness.lexical_exits[left].scopes.clone();
                    witness.lexical_exits[left].scopes =
                        witness.lexical_exits[right].scopes.clone();
                    witness.lexical_exits[right].scopes = left_scopes;
                }
                1 => {
                    for scope in &mut witness.lexical_exits[left].scopes {
                        scope.active.clear();
                    }
                }
                2 => {
                    let scope = witness.lexical_exits[left]
                        .scopes
                        .iter_mut()
                        .find(|scope| !scope.active.is_empty())
                        .unwrap();
                    scope.active.push(scope.active[0]);
                }
                _ => unreachable!(),
            }
            assert_eq!(witness.lexical_seal, independent);
            assert!(crate::breakpoint_regions::blocks_equal(
                &function.blocks,
                &original_blocks
            ));
            assert_forged_refused(
                forged,
                types,
                &format!("membership mutation={mutation} release={release}"),
            );
        }
    }
}
