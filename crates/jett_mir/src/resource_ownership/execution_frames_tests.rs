use super::tests::{checked, lowered};
use super::*;

const SOURCES: &[&str] = &[
    include_str!(
        "../../../jett_driver/tests/native_conformance/resource/13_first_handle_failure.jett"
    ),
    include_str!(
        "../../../jett_driver/tests/native_conformance/resource/14_first_handle_success.jett"
    ),
];

#[test]
fn staged_scope_sum_sites_and_extracts_use_exact_active_execution_frame() {
    for release in [false, true] {
        for &source in SOURCES {
            let checked = checked(source, release);
            let program = lowered(&checked);
            let main = program
                .functions
                .iter()
                .find(|function| function.identity.declaration.name == "main")
                .unwrap();
            let plan = validate_resource_ownership(&program, &checked.checked().interner).unwrap();
            let flow = plan.function(main.id).unwrap();
            let begin = flow
                .operations()
                .iter()
                .find(|operation| {
                    matches!(
                        operation.role(),
                        ResourceOperationRole::BeginSourceFunction { .. }
                    )
                })
                .unwrap();
            let active = begin.frame();
            let scope = flow.root_scope().id();
            assert_ne!(active, scope);
            assert_eq!(
                flow.execution_frame(begin.site().block(), begin.site().position()),
                Some(scope)
            );
            let mut tags = 0;
            let mut takes = 0;
            for block in &main.blocks {
                for (index, statement) in block.statements.iter().enumerate() {
                    if let StatementKind::SumTag { source, .. } = statement.kind {
                        let Some(slot) = flow.owner_slots().iter().find(|slot|
                            matches!(slot.storage(), ResourceSlotStorage::Local { header } if header.id == source)) else { continue };
                        tags += 1;
                        assert_eq!(slot.frame(), scope);
                        assert_eq!(
                            flow.execution_frame(block.id, ResourcePosition::Statement(index)),
                            Some(active)
                        );
                        let transfer = flow.operations().iter().find(|operation|
                            matches!(operation.role(), ResourceOperationRole::Transfer { destination, .. } if *destination == slot.id())).unwrap();
                        assert_eq!(transfer.frame(), active);
                    }
                    if let StatementKind::SumTake {
                        source, success, ..
                    } = statement.kind
                    {
                        let Some(shell) = flow.owner_slots().iter().find(|slot|
                            matches!(slot.storage(), ResourceSlotStorage::Local { header } if header.id == source)) else { continue };
                        takes += 1;
                        let operation = flow
                            .operations()
                            .iter()
                            .find(|operation| {
                                operation.site().block() == block.id
                                    && operation.site().position()
                                        == ResourcePosition::Statement(index)
                                    && matches!(
                                        operation.role(),
                                        ResourceOperationRole::SumTake { .. }
                                            | ResourceOperationRole::TakeFailureCompanion { .. }
                                    )
                            })
                            .unwrap();
                        assert_eq!(operation.frame(), active);
                        assert_eq!(
                            flow.execution_frame(block.id, ResourcePosition::Statement(index)),
                            Some(active)
                        );
                        assert_eq!(shell.frame(), scope);
                        if success {
                            let ResourceOperationRole::SumTake { destination, .. } =
                                operation.role()
                            else {
                                panic!("success extraction")
                            };
                            assert_eq!(flow.owner_slots()[destination.index()].frame(), scope);
                        }
                    }
                }
                if matches!(block.terminator.kind, TerminatorKind::Return(_)) {
                    assert_eq!(
                        flow.execution_frame(block.id, ResourcePosition::Terminator),
                        Some(scope)
                    );
                }
            }
            assert_eq!((tags, takes), (1, 2));
            assert_eq!(
                flow.execution_frame(BlockId(u32::MAX), ResourcePosition::Terminator),
                None
            );
            assert_eq!(
                flow.execution_frame(main.entry, ResourcePosition::Statement(usize::MAX)),
                None
            );
        }
    }
}

#[test]
fn staged_sum_frame_projection_cannot_authorize_copied_or_altered_current_graph() {
    for release in [false, true] {
        let checked = checked(SOURCES[0], release);
        let original = lowered(&checked);
        let main = original
            .functions
            .iter()
            .find(|function| function.identity.declaration.name == "main")
            .unwrap();
        let (block, index) = main
            .blocks
            .iter()
            .find_map(|block| {
                block
                    .statements
                    .iter()
                    .enumerate()
                    .find_map(|(index, statement)| {
                        matches!(statement.kind, StatementKind::SumTag { .. })
                            .then_some((block.id, index))
                    })
            })
            .unwrap();
        for copied in [false, true] {
            let mut changed = original.clone();
            let current = &mut changed.functions[main.id.index() as usize];
            if copied {
                let statement = current.blocks[block.index() as usize].statements[index].clone();
                current.blocks[block.index() as usize]
                    .statements
                    .insert(index, statement);
            } else {
                let StatementKind::SumTag { source, .. } =
                    &mut current.blocks[block.index() as usize].statements[index].kind
                else {
                    unreachable!()
                };
                *source = current.params[0].local;
            }
            assert!(validate_resource_ownership(&changed, &checked.checked().interner).is_err());
        }
    }
}
