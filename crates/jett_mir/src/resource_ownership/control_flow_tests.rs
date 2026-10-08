use super::tests::{checked_support, lowered};
use super::*;
use crate::breakpoint_regions::SequenceEdit;
use hir::ExpressionKind as E;

const SUPPORT: &str =
    include_str!("../../../jett_driver/tests/native_conformance/resource/resource_probe.jett");
const SOURCES: [&str; 5] = [
    include_str!(
        "../../../jett_driver/tests/native_conformance/resource/control_flow/01_for_owned_optional_reuse.jett"
    ),
    include_str!(
        "../../../jett_driver/tests/native_conformance/resource/control_flow/02_for_owned_optional_break.jett"
    ),
    include_str!(
        "../../../jett_driver/tests/native_conformance/resource/control_flow/03_for_owned_result_continue.jett"
    ),
    include_str!(
        "../../../jett_driver/tests/native_conformance/resource/control_flow/04_match_owned_optional_first.jett"
    ),
    include_str!(
        "../../../jett_driver/tests/native_conformance/resource/control_flow/05_match_owned_optional_second.jett"
    ),
];
fn main(program: &Program) -> &Function {
    program
        .functions
        .iter()
        .find(|function| {
            function.identity.declaration.namespace == "app"
                && function.identity.declaration.name == "main"
        })
        .unwrap()
}
fn main_mut(program: &mut Program) -> &mut Function {
    program
        .functions
        .iter_mut()
        .find(|function| {
            function.identity.declaration.namespace == "app"
                && function.identity.declaration.name == "main"
        })
        .unwrap()
}
fn edit(before: &Function, prepared: &Function) -> SequenceEdit {
    let block = before
        .blocks
        .iter()
        .find(|block| matches!(block.terminator.kind, TerminatorKind::ForEach { .. }))
        .unwrap();
    let TerminatorKind::ForEach { body, .. } = block.terminator.kind else {
        unreachable!()
    };
    let preheader = before
        .blocks
        .iter()
        .find(|candidate| {
            matches!(candidate.terminator.kind, TerminatorKind::Goto(target) if target == block.id)
                && prepared.blocks[candidate.id.index() as usize]
                    .statements
                    .len()
                    > candidate.statements.len()
        })
        .unwrap();
    SequenceEdit {
        header: block.id,
        before: block.terminator.clone(),
        after: prepared.blocks[block.id.index() as usize]
            .terminator
            .clone(),
        append: vec![(
            preheader.id,
            prepared.blocks[preheader.id.index() as usize].statements[preheader.statements.len()..]
                .to_vec(),
        )],
        prefix: Some((
            body,
            prepared.blocks[body.index() as usize].statements[..2].to_vec(),
        )),
        redirects: Vec::new(),
        blocks: Vec::new(),
    }
}
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
#[test]
fn resource_control_flow_original_for_and_match_retire_exact_loans_before_backing_reuse() {
    for release in [false, true] {
        for (index, source) in SOURCES.iter().enumerate() {
            let checked = checked_support(source, SUPPORT, release);
            let types = &checked.checked().interner;
            let mut program = lowered(&checked);
            validate_resource_ownership(&program, types).unwrap();
            let original = main(&program).clone();
            let original_rows = original
                .resource_lowering
                .as_ref()
                .unwrap()
                .borrowed_sums
                .clone();
            crate::prepare_native_sequences(&mut program, types);
            let function = main(&program);
            assert!(
                !function
                    .blocks
                    .iter()
                    .any(|block| matches!(block.terminator.kind, TerminatorKind::ForEach { .. })),
                "Source{} release={release}",
                index + 1
            );
            let ownership = validate_resource_ownership(&program, types).unwrap();
            let plan = ownership.function(function.id).unwrap();
            let rows = &function.resource_lowering.as_ref().unwrap().borrowed_sums;
            assert_eq!(rows.len(), if index < 3 { 1 } else { 2 });
            for (old, row) in original_rows.iter().zip(rows) {
                assert_eq!(old.original_initializer(), row.original_initializer());
                assert_eq!(old.alias(), row.alias());
                assert_eq!(old.backing(), row.backing());
                for local in row.retirement_locals() {
                    assert!(function.is_view_local(local));
                    assert!(!plan.owner_slots().iter().any(|slot| matches!(slot.storage(), ResourceSlotStorage::Local { header } if header.id == local)));
                }
                let backing = plan.owner_slots().iter().find(|slot| matches!(slot.storage(), ResourceSlotStorage::Local { header } if header.id == row.backing())).unwrap().id();
                let projection = plan
                    .operations()
                    .iter()
                    .find(|operation| {
                        operation.site() == row.success_site()
                            && matches!(
                                operation.role(),
                                ResourceOperationRole::ProjectSumView { .. }
                            )
                    })
                    .unwrap();
                let ResourceOperationRole::ProjectSumView {
                    source: parent,
                    destination: child,
                    ..
                } = projection.role()
                else {
                    unreachable!()
                };
                let expected_kind = match index {
                    1 => ResourceLexicalExitKind::Break,
                    2 => ResourceLexicalExitKind::Continue,
                    _ => ResourceLexicalExitKind::Fallthrough,
                };
                let exits = function
                    .blocks
                    .iter()
                    .flat_map(|block| {
                        (0..block.statements.len())
                            .map(ResourcePosition::Statement)
                            .chain(std::iter::once(ResourcePosition::Terminator))
                            .filter_map(move |position| {
                                let exit = plan.lexical_exit(block.id, position)?;
                                (exit.kind() == expected_kind
                                    && exit.clear_locals().contains(&row.alias()))
                                .then_some((
                                    ResourceSite {
                                        function: function.id,
                                        block: block.id,
                                        position,
                                    },
                                    exit,
                                ))
                            })
                    })
                    .collect::<Vec<_>>();
                assert!(!exits.is_empty());
                for (_, exit) in &exits {
                    let operations = exit
                        .operation_ids()
                        .iter()
                        .map(|id| &plan.operations()[id.index()])
                        .collect::<Vec<_>>();
                    let child_end = operations.iter().position(|operation| matches!(operation.role(), ResourceOperationRole::EndBorrow { loan } if loan == child)).unwrap();
                    let parent_end = operations.iter().position(|operation| matches!(operation.role(), ResourceOperationRole::EndSumBorrow { loan } if loan == parent)).unwrap();
                    assert!(
                        child_end < parent_end,
                        "Source{} child retires before the parent shell lease",
                        index + 1
                    );
                    assert!(!exit.clear_locals().contains(&row.backing()));
                    assert!(
                        exit.clear_locals()
                            .iter()
                            .all(|local| row.retirement_locals().contains(local)),
                        "an arm cannot retire its sibling alias"
                    );
                }
                let moves = plan.operations().iter().filter(|operation| matches!(operation.role(), ResourceOperationRole::Transfer { source, .. } if *source == backing)).collect::<Vec<_>>();
                assert_eq!(
                    moves.len(),
                    1,
                    "original held is reused once after the borrowed scope"
                );
                for (site, _) in exits {
                    assert!(
                        site_can_reach(function, site, moves[0].site()),
                        "the original backing is reused after this actual lexical boundary"
                    );
                }
                for operation in plan.operations() {
                    if matches!(operation.role(), ResourceOperationRole::Drop { source, .. } if *source == backing)
                    {
                        assert!(
                            !site_can_reach(function, operation.site(), moves[0].site()),
                            "aborting cleanup cannot consume the backing on a path to its later owning unwrap"
                        );
                    }
                }
            }
            let once = program.clone();
            crate::prepare_native_sequences(&mut program, types);
            assert_eq!(
                program, once,
                "the no-op preparation must preserve every authenticated witness"
            );
        }
    }
}
#[test]
fn resource_control_flow_canonical_sequence_refuses_forged_transcripts_and_custody_edits() {
    for release in [false, true] {
        let checked = checked_support(SOURCES[0], SUPPORT, release);
        let types = &checked.checked().interner;
        let baseline = lowered(&checked);
        let before = main(&baseline);
        let mut prepared = baseline.clone();
        crate::prepare_native_sequences(&mut prepared, types);
        let good = main(&prepared);
        let canonical = edit(before, good);
        let mut replay = good.clone();
        replay.resource_lowering = before.resource_lowering.clone();
        control_flow::sequence_transition(&mut replay, before, &canonical, types).unwrap();
        assert_eq!(&replay, good);
        for mutation in 0..10 {
            let mut candidate = good.clone();
            candidate.resource_lowering = before.resource_lowering.clone();
            let mut transcript = canonical.clone();
            let (body, prefix) = transcript.prefix.as_mut().unwrap();
            let body = *body;
            match mutation {
                0 => {
                    prefix.swap(0, 1);
                    candidate.blocks[body.index() as usize]
                        .statements
                        .swap(0, 1);
                }
                1 => {
                    let TerminatorKind::Branch { condition, .. } = &mut transcript.after.kind
                    else {
                        unreachable!()
                    };
                    let E::Binary { op, .. } = &mut condition.kind else {
                        unreachable!()
                    };
                    *op = hir::BinaryOp::LessEqual;
                    candidate.blocks[transcript.header.index() as usize].terminator =
                        transcript.after.clone();
                }
                2 => {
                    let StatementKind::SequenceGet { consume, .. } = &mut prefix[0].kind else {
                        unreachable!()
                    };
                    *consume = false;
                    candidate.blocks[body.index() as usize].statements[0] = prefix[0].clone();
                }
                3 => {
                    let StatementKind::SequenceGet { target, .. } = &mut prefix[0].kind else {
                        unreachable!()
                    };
                    *target = before.resource_lowering.as_ref().unwrap().borrowed_sums[0].backing();
                    candidate.blocks[body.index() as usize].statements[0] = prefix[0].clone();
                }
                4 => {
                    let duplicate = candidate
                        .blocks
                        .iter()
                        .flat_map(|block| &block.statements)
                        .find(|statement| {
                            matches!(statement.kind, StatementKind::ResourceLexicalExit(_))
                        })
                        .unwrap()
                        .clone();
                    prefix.push(duplicate.clone());
                    candidate.blocks[body.index() as usize]
                        .statements
                        .insert(2, duplicate);
                }
                5 => {
                    let TerminatorKind::Branch { else_block, .. } = &mut transcript.after.kind
                    else {
                        unreachable!()
                    };
                    *else_block = body;
                    candidate.blocks[transcript.header.index() as usize].terminator =
                        transcript.after.clone();
                }
                6 => {
                    transcript.append[0].1.swap(0, 2);
                    let (preheader, values) = &transcript.append[0];
                    let start = before.blocks[preheader.index() as usize].statements.len();
                    candidate.blocks[preheader.index() as usize].statements[start..]
                        .clone_from_slice(values);
                }
                7 => {
                    candidate.locals.last_mut().unwrap().ty = before
                        .local(
                            before.resource_lowering.as_ref().unwrap().borrowed_sums[0].backing(),
                        )
                        .unwrap()
                        .ty;
                }
                8 => {
                    let statement = candidate
                        .blocks
                        .iter_mut()
                        .flat_map(|block| &mut block.statements)
                        .find(|statement| {
                            matches!(statement.kind, StatementKind::SumTake { success: true, .. })
                        })
                        .unwrap();
                    let StatementKind::SumTake { source, .. } = &mut statement.kind else {
                        unreachable!()
                    };
                    *source = before.resource_lowering.as_ref().unwrap().borrowed_sums[0].backing();
                }
                9 => {
                    transcript.prefix = None;
                }
                _ => unreachable!(),
            }
            let unchanged = candidate.resource_lowering.clone();
            assert!(
                control_flow::sequence_transition(&mut candidate, before, &transcript, types)
                    .is_err(),
                "mutation={mutation} release={release}"
            );
            assert_eq!(
                candidate.resource_lowering, unchanged,
                "rejected edit grants no replacement witness"
            );
        }
    }
}
#[test]
fn resource_control_flow_prepared_for_and_match_refuse_resealed_duplicate_or_moved_loan_sites() {
    for release in [false, true] {
        for index in [0, 1, 2, 3, 4] {
            let checked = checked_support(SOURCES[index], SUPPORT, release);
            let types = &checked.checked().interner;
            let mut baseline = lowered(&checked);
            crate::prepare_native_sequences(&mut baseline, types);
            validate_resource_ownership(&baseline, types).unwrap();
            for mutation in 0..4 {
                let mut candidate = baseline.clone();
                let function = main_mut(&mut candidate);
                let row = function.resource_lowering.as_ref().unwrap().borrowed_sums[0].clone();
                let project = row.success_site();
                let ResourcePosition::Statement(position) = project.position() else {
                    unreachable!()
                };
                match mutation {
                    0 => {
                        let duplicate = function.blocks[project.block().index() as usize]
                            .statements[position]
                            .clone();
                        function.blocks[project.block().index() as usize]
                            .statements
                            .insert(position + 1, duplicate);
                    }
                    1 => {
                        function.blocks[project.block().index() as usize]
                            .statements
                            .remove(position);
                    }
                    2 => {
                        let exit = function
                            .blocks
                            .iter_mut()
                            .find(|block| {
                                block.statements.iter().any(|statement| {
                                    matches!(statement.kind, StatementKind::ResourceLexicalExit(_))
                                })
                            })
                            .unwrap();
                        let boundary = exit
                            .statements
                            .iter()
                            .position(|statement| {
                                matches!(statement.kind, StatementKind::ResourceLexicalExit(_))
                            })
                            .unwrap();
                        let duplicate = exit.statements[boundary].clone();
                        exit.statements.insert(0, duplicate);
                    }
                    3 => {
                        let block = function
                            .blocks
                            .iter_mut()
                            .find(|block| {
                                matches!(
                                    block.terminator.kind,
                                    TerminatorKind::Switch { .. } | TerminatorKind::Branch { .. }
                                )
                            })
                            .unwrap();
                        match &mut block.terminator.kind {
                            TerminatorKind::Switch { variants, .. } => {
                                let target = variants[0].1;
                                variants[0].1 = variants[1].1;
                                variants[1].1 = target;
                            }
                            TerminatorKind::Branch {
                                then_block,
                                else_block,
                                ..
                            } => {
                                std::mem::swap(then_block, else_block);
                            }
                            _ => unreachable!(),
                        }
                    }
                    _ => unreachable!(),
                }
                let witness = function.resource_lowering.as_mut().unwrap();
                witness.blocks = function.blocks.clone();
                witness.locals = function.locals.clone();
                assert!(
                    validate_resource_ownership(&candidate, types).is_err(),
                    "Source{} mutation={mutation} release={release}",
                    index + 1
                );
                assert!(
                    !crate::sequences::prune::unreachable(main_mut(&mut candidate)),
                    "canonical pruning cannot rehabilitate a forged loan graph"
                );
            }
        }
    }
}

// Draft append to resource_ownership/control_flow_tests.rs after Source registration.
// UNEXECUTED: Root must first compile/check both new Source programs and derive
// provider transcripts from the reference executor. No proposed outcome is acceptance.
const MULTI_LOOP_SOURCES: [&str; 2] = [
    include_str!(
        "../../../jett_driver/tests/native_conformance/resource/control_flow/07_consecutive_for_owned_optional_reuse.jett"
    ),
    include_str!(
        "../../../jett_driver/tests/native_conformance/resource/control_flow/08_nested_for_owned_optional_reuse.jett"
    ),
];
#[test]
fn resource_control_flow_repeated_sequence_transitions_preserve_all_sibling_and_nested_loans() {
    for release in [false, true] {
        for (index, source) in MULTI_LOOP_SOURCES.iter().enumerate() {
            let checked = checked_support(source, SUPPORT, release);
            let types = &checked.checked().interner;
            let mut program = lowered(&checked);
            validate_resource_ownership(&program, types).unwrap();
            let original = main(&program).clone();
            let original_witness = original.resource_lowering.as_ref().unwrap();
            assert_eq!(
                original
                    .blocks
                    .iter()
                    .filter(|block| matches!(block.terminator.kind, TerminatorKind::ForEach { .. }))
                    .count(),
                2
            );
            assert_eq!(original_witness.borrowed_sums.len(), 2);
            crate::prepare_native_sequences(&mut program, types);
            let function = main(&program);
            assert!(
                !function
                    .blocks
                    .iter()
                    .any(|block| matches!(block.terminator.kind, TerminatorKind::ForEach { .. }))
            );
            let plan = validate_resource_ownership(&program, types).unwrap();
            let custody = plan.function(function.id).unwrap();
            let witness = function.resource_lowering.as_ref().unwrap();
            assert_eq!(witness.borrowed_sums.len(), 2);
            for (before, after) in original_witness
                .borrowed_sums
                .iter()
                .zip(&witness.borrowed_sums)
            {
                assert_eq!(before.original_initializer(), after.original_initializer());
                assert_eq!(before.alias(), after.alias());
                assert_eq!(before.backing(), after.backing());
            }
            let rows = &witness.borrowed_sums;
            assert_eq!(rows[0].backing(), rows[1].backing());
            assert_ne!(rows[0].alias(), rows[1].alias());
            if index == 0 {
                let first_alias = function.local(rows[0].alias()).unwrap();
                let second_alias = function.local(rows[1].alias()).unwrap();
                assert_eq!(first_alias.name, "token");
                assert_eq!(second_alias.name, "token");
                assert_ne!(first_alias.span, second_alias.span);
            }
            assert_eq!(
                function
                    .blocks
                    .iter()
                    .flat_map(|block| &block.statements)
                    .filter(|statement| matches!(
                        statement.kind,
                        StatementKind::SequenceGet { consume: true, .. }
                    ))
                    .count(),
                2
            );
            let projects = custody
                .operations()
                .iter()
                .filter_map(|operation| {
                    let ResourceOperationRole::ProjectSumView {
                        source,
                        destination,
                        ..
                    } = operation.role()
                    else {
                        return None;
                    };
                    Some((operation, *source, *destination))
                })
                .collect::<Vec<_>>();
            assert_eq!(projects.len(), 2);
            for (position, row) in rows.iter().enumerate() {
                let (_, parent, child) = projects
                    .iter()
                    .find(|(operation, _, _)| operation.site() == row.success_site())
                    .unwrap();
                let exits = function
                    .blocks
                    .iter()
                    .flat_map(|block| {
                        (0..block.statements.len())
                            .map(ResourcePosition::Statement)
                            .chain(std::iter::once(ResourcePosition::Terminator))
                            .filter_map(move |site| custody.lexical_exit(block.id, site))
                    })
                    .filter(|exit| {
                        exit.kind() == ResourceLexicalExitKind::Fallthrough
                            && exit.clear_locals().contains(&row.alias())
                    })
                    .collect::<Vec<_>>();
                assert!(!exits.is_empty());
                for exit in exits {
                    let roles = exit
                        .operation_ids()
                        .iter()
                        .map(|id| custody.operations()[id.index()].role())
                        .collect::<Vec<_>>();
                    let child_end = roles.iter().position(|role| matches!(role, ResourceOperationRole::EndBorrow { loan } if loan == child)).unwrap();
                    let parent_end = roles.iter().position(|role| matches!(role, ResourceOperationRole::EndSumBorrow { loan } if loan == parent)).unwrap();
                    assert!(child_end < parent_end);
                    assert!(!exit.clear_locals().contains(&row.backing()));
                    if index == 0 || position == 1 {
                        assert!(
                            !exit.clear_locals().contains(&rows[1 - position].alias()),
                            "sibling fallthrough or nested inner exit cannot retire another live scope"
                        );
                    }
                }
            }
            let once = program.clone();
            crate::prepare_native_sequences(&mut program, types);
            assert_eq!(program, once);
        }
    }
}
