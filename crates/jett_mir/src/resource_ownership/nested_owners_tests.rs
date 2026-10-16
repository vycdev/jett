use super::tests::{checked, lowered};
use super::*;

const LOOP_SOURCE: &str = r#"namespace app
function scenario(view net: Network, mode: int64) returns nothing:
    use resource_probe
    mutable int64 remaining = 2
    while remaining > 0:
        resource_probe.TestHandle token = resource_probe.create(view net, 4) handle error:
            return nothing
        remaining = remaining - 1
        if mode == 0:
            continue
        if mode == 1:
            break
    return nothing
"#;

fn scenario(program: &Program) -> &Function {
    program
        .functions
        .iter()
        .find(|function| {
            function.identity.declaration.namespace == "app"
                && function.identity.declaration.name == "scenario"
        })
        .expect("original nested owner function")
}
fn scenario_mut(program: &mut Program) -> &mut Function {
    program
        .functions
        .iter_mut()
        .find(|function| {
            function.identity.declaration.namespace == "app"
                && function.identity.declaration.name == "scenario"
        })
        .expect("original nested owner function")
}

fn assert_loop_retirements(program: &Program, types: &TypeInterner) {
    let function = scenario(program);
    let ownership = validate_resource_ownership(program, types).unwrap();
    let plan = ownership.function(function.id).unwrap();
    let witness = function.resource_lowering.as_ref().unwrap();
    assert_eq!(witness.lexical_owners.len(), 1);
    let original = witness.lexical_owners[0].original_local();
    let local = witness.lexical_owners[0].current_local().unwrap();
    let slot = plan.owner_slots().iter().find(|slot| {
        matches!(slot.storage(), ResourceSlotStorage::Local { header } if header.id == local)
    }).expect("nested declaration retains its exact owned slot");
    assert_eq!(slot.frame(), plan.root_scope().id());
    assert_eq!(
        plan.frames()
            .iter()
            .filter(|frame| { frame.role() == ResourceFrameRole::Scope })
            .count(),
        1,
        "lexical retirement must preserve the resident RootScope"
    );
    let mut kinds = Vec::new();
    for block in &function.blocks {
        for (index, statement) in block.statements.iter().enumerate() {
            if !matches!(statement.kind, StatementKind::ResourceLexicalExit(_)) {
                continue;
            }
            let position = ResourcePosition::Statement(index);
            let current = function
                .resource_lexical_exit(block.id, position)
                .unwrap()
                .expect("exact private lexical exit");
            assert_eq!(current.declarations().collect::<Vec<_>>(), vec![original]);
            assert_eq!(current.current_locals(function).unwrap(), vec![local]);
            let exit = plan.lexical_exit(block.id, position).unwrap();
            assert_eq!(exit.kind(), current.kind());
            assert_eq!(exit.clear_locals(), &[local]);
            assert_eq!(exit.operation_ids().len(), 1);
            let operation = &plan.operations()[exit.operation_ids()[0].index()];
            assert_eq!(operation.site().function(), function.id);
            assert_eq!(operation.site().block(), block.id);
            assert_eq!(operation.site().position(), position);
            assert_eq!(operation.frame(), slot.frame());
            assert!(!operation.is_expression_operation());
            assert!(matches!(operation.role(), ResourceOperationRole::Drop {
                source, occupancy: ResourceOccupancy::Occupied,
            } if *source == slot.id()));
            assert!(
                !plan.operations().iter().any(|operation| {
                    operation.site().block() == block.id
                        && operation.site().position() == position
                        && matches!(operation.role(), ResourceOperationRole::Complete { .. })
                }),
                "a lexical boundary must not complete the RootScope"
            );
            kinds.push(exit.kind());
        }
    }
    assert_eq!(kinds.len(), 3);
    for expected in [
        ResourceLexicalExitKind::Continue,
        ResourceLexicalExitKind::Break,
        ResourceLexicalExitKind::Fallthrough,
    ] {
        assert_eq!(kinds.iter().filter(|kind| **kind == expected).count(), 1);
    }
}

#[test]
fn nested_owned_resource_loop_exits_retire_exact_owner_and_preserve_root_scope() {
    for release in [false, true] {
        let checked = checked(LOOP_SOURCE, release);
        let types = &checked.checked().interner;
        let mut program = lowered(&checked);
        assert_loop_retirements(&program, types);
        for _ in 0..2 {
            let function = scenario_mut(&mut program);
            assert!(crate::sequences::prune::unreachable(function));
            assert!(crate::sequences::prune::unused_locals(function));
            assert_loop_retirements(&program, types);
        }
    }
}

fn reseal_general(function: &mut Function) {
    let blocks = function.blocks.clone();
    let locals = function.locals.clone();
    let witness = function.resource_lowering.as_mut().unwrap();
    witness.blocks = blocks;
    witness.locals = locals;
}
fn assert_refused(program: &Program, types: &TypeInterner, mutation: usize) {
    assert!(
        validate_resource_ownership(program, types).is_err(),
        "owned lexical mutation {mutation} was admitted"
    );
    let before = scenario(program).clone();
    let mut function = before.clone();
    assert!(
        !crate::sequences::prune::unreachable(&mut function),
        "owned lexical mutation {mutation} authorized unreachable pruning"
    );
    assert!(crate::breakpoint_regions::blocks_equal(
        &function.blocks,
        &before.blocks
    ));
    assert_eq!(function.locals, before.locals);
    let mut function = before.clone();
    assert!(
        !crate::sequences::prune::unused_locals(&mut function),
        "owned lexical mutation {mutation} authorized local compaction"
    );
    assert!(crate::breakpoint_regions::blocks_equal(
        &function.blocks,
        &before.blocks
    ));
    assert_eq!(function.locals, before.locals);
}

#[test]
fn nested_owned_resource_records_and_exit_membership_refuse_general_resealing() {
    for release in [false, true] {
        let checked = checked(LOOP_SOURCE, release);
        let types = &checked.checked().interner;
        let baseline = lowered(&checked);
        validate_resource_ownership(&baseline, types).unwrap();
        for mutation in 0..4 {
            let mut forged = baseline.clone();
            let function = scenario_mut(&mut forged);
            match mutation {
                0 => {
                    function
                        .resource_lowering
                        .as_mut()
                        .unwrap()
                        .lexical_owners
                        .clear();
                }
                1 => {
                    let witness = function.resource_lowering.as_mut().unwrap();
                    witness
                        .lexical_owners
                        .push(witness.lexical_owners[0].clone());
                }
                2 => {
                    let witness = function.resource_lowering.as_mut().unwrap();
                    let scope = witness
                        .lexical_exits
                        .iter_mut()
                        .flat_map(|exit| &mut exit.scopes)
                        .find(|scope| !scope.active.is_empty())
                        .unwrap();
                    scope.active.clear();
                }
                3 => {
                    let (block, index) = function
                        .blocks
                        .iter()
                        .find_map(|block| {
                            block
                                .statements
                                .iter()
                                .position(|statement| {
                                    matches!(statement.kind, StatementKind::ResourceLexicalExit(_))
                                })
                                .map(|index| (block.id, index))
                        })
                        .unwrap();
                    function.blocks[block.index() as usize]
                        .statements
                        .remove(index);
                }
                _ => unreachable!(),
            }
            reseal_general(function);
            crate::validate(&forged).unwrap();
            assert_refused(&forged, types, mutation);
        }
    }
}
