use super::tests::{checked, lowered};
use super::*;
use hir::ExpressionKind as E;

const ORIGINAL: &str = include_str!(
    "../../../jett_comptime/src/resource_execution/fixtures/05_later_named_argument_failure_v2.jett"
);
const MINIMAL: &str = include_str!(
    "../../../jett_driver/tests/native_conformance/resource/09_later_handle_minimal.jett"
);
const SUCCESS: &str = include_str!(
    "../../../jett_driver/tests/native_conformance/resource/11_later_handle_success.jett"
);
const RETAINED: &str = include_str!(
    "../../../jett_driver/tests/native_conformance/resource/12_later_handle_written_view.jett"
);
const FIRST: &str = include_str!(
    "../../../jett_driver/tests/native_conformance/resource/13_first_handle_failure.jett"
);

fn outer(program: &Program) -> &Function {
    program
        .functions
        .iter()
        .find(|function| {
            function
                .resource_lowering
                .as_ref()
                .is_some_and(|witness| !witness.regions.is_empty())
        })
        .expect("one authenticated outer call producer")
}

#[test]
fn normalized_source_call_preserves_original_tuple_and_constructor_cfg_in_both_profiles() {
    for release in [false, true] {
        for source in [ORIGINAL, MINIMAL, SUCCESS, RETAINED, FIRST] {
            let checked = checked(source, release);
            let types = &checked.checked().interner;
            let hir = hir::lower_checked_resource_program(&checked).unwrap();
            let mut originals = Vec::new();
            for function in &hir.functions {
                walk::hir_block(&function.body, &mut |value| {
                    if let E::Call {
                        function: target,
                        evaluation_order,
                        ownership: hir::CallOwnership::Source(source),
                        ..
                    } = &value.kind
                        && hir.functions[target.index() as usize]
                            .identity
                            .declaration
                            .name
                            == "accept_staged"
                    {
                        assert_eq!(evaluation_order, &[1, 0, 2]);
                        assert!(
                            source
                                .arguments
                                .iter()
                                .all(|fact| fact.staging == hir::ArgumentStaging::Original)
                        );
                        originals.push(value.clone());
                    }
                });
            }
            assert_eq!(originals.len(), 1);
            let mir = lower(&hir, types).unwrap();
            let function = outer(&mir);
            let witness = function.resource_lowering.as_ref().unwrap();
            assert_eq!(witness.regions.len(), 1);
            let region = &witness.regions[0];
            assert!(crate::breakpoint_regions::expressions_equal(
                region.original(),
                &originals[0]
            ));
            assert_eq!(region.evaluation_order(), &[1, 0, 2]);
            assert_eq!(
                region
                    .actuals()
                    .iter()
                    .map(ResourceCallActual::parameter)
                    .collect::<Vec<_>>(),
                [1, 0, 2]
            );
            assert!(region.actuals()[0].ordinary().is_none());
            assert!(region.actuals()[1].ordinary().is_some());
            assert!(region.actuals()[2].ordinary().is_some());
            assert!(
                !function
                    .blocks
                    .iter()
                    .flat_map(|block| &block.statements)
                    .any(|statement| matches!(statement.kind, StatementKind::BeginCallView { .. }))
            );
            let plan = validate_resource_ownership(&mir, types).unwrap();
            ResourceCompanionPlan::analyze(&plan, function.id).unwrap();
            let flow = plan.function(function.id).unwrap();
            let begins = flow
                .operations()
                .iter()
                .filter(|operation| {
                    matches!(
                        operation.role(),
                        ResourceOperationRole::BeginSourceFunction { .. }
                    )
                })
                .collect::<Vec<_>>();
            assert_eq!(begins.len(), 1);
            let frame = begins[0].frame();
            assert_eq!(
                flow.frames()[frame.index()].parent(),
                Some(ResourceFrameId(0))
            );
            let stages = flow
                .operations()
                .iter()
                .filter(|operation| {
                    matches!(
                        operation.role(),
                        ResourceOperationRole::StageSourceActual { .. }
                    )
                })
                .collect::<Vec<_>>();
            assert_eq!(stages.len(), 3);
            assert!(stages.iter().all(
                |operation| operation.frame() == frame && !operation.is_expression_operation()
            ));
            assert_eq!(
                flow.operations()
                    .iter()
                    .filter(|operation| operation.frame() == frame
                        && matches!(
                            operation.role(),
                            ResourceOperationRole::InvokeSourceFunction { .. }
                        ))
                    .count(),
                1
            );
            let prepared = flow
                .operations()
                .iter()
                .filter_map(|operation| match operation.role() {
                    ResourceOperationRole::PrepareSourceBorrow { loan, parameter } => {
                        Some((*loan, *parameter))
                    }
                    _ => None,
                })
                .collect::<Vec<_>>();
            assert_eq!(prepared.len(), 1);
            assert_eq!(prepared[0].1, 1);
            assert_eq!(flow.loans()[prepared[0].0.index()].parameter(), Some(1));
            for (site, outcome) in region.exits() {
                if *outcome != ResourceCompletion::Abort {
                    continue;
                }
                let end = flow
                    .operations()
                    .iter()
                    .find(|operation| {
                        operation.site() == *site
                            && operation.frame() == frame
                            && matches!(
                                operation.role(),
                                ResourceOperationRole::Complete {
                                    outcome: ResourceCompletion::Abort
                                }
                            )
                    })
                    .unwrap();
                assert!(!end.is_expression_operation());
                let ResourcePosition::Statement(index) = site.position() else {
                    panic!("typed abandoned statement");
                };
                assert!(matches!(
                    function.blocks[site.block().index() as usize]
                        .terminator
                        .kind,
                    TerminatorKind::Return(_)
                ));
                assert!(
                    index
                        < function.blocks[site.block().index() as usize]
                            .statements
                            .len()
                );
            }
        }
    }
}

#[test]
fn normalized_first_handled_actual_has_a_proved_empty_abandoned_prefix() {
    for release in [false, true] {
        let checked = checked(FIRST, release);
        let mir = lowered(&checked);
        let function = outer(&mir);
        let region = &function.resource_lowering.as_ref().unwrap().regions[0];
        let plan = validate_resource_ownership(&mir, &checked.checked().interner).unwrap();
        let flow = plan.function(function.id).unwrap();
        let first = region.actuals()[0].start().block();
        let TerminatorKind::Branch {
            else_block: failed, ..
        } = function.blocks[first.index() as usize].terminator.kind
        else {
            panic!("first exact Handle branch");
        };
        let abort = region
            .exits()
            .iter()
            .find(|(site, outcome)| site.block() == failed && *outcome == ResourceCompletion::Abort)
            .unwrap()
            .0;
        assert!(
            !flow
                .operations()
                .iter()
                .any(|operation| operation.site() == abort
                    && matches!(
                        operation.role(),
                        ResourceOperationRole::EndBorrow { .. }
                            | ResourceOperationRole::Drop { .. }
                    ))
        );
    }
}

#[test]
fn normalized_call_public_node_and_cfg_edits_cannot_copy_or_reseal_authority() {
    for release in [false, true] {
        let checked = checked(MINIMAL, release);
        let types = &checked.checked().interner;
        let original = lowered(&checked);
        let index = original
            .functions
            .iter()
            .position(|function| function.id == outer(&original).id)
            .unwrap();
        let region = outer(&original).resource_lowering.as_ref().unwrap().regions[0].clone();
        for mutation in 0..9 {
            let mut edited = original.clone();
            let function = &mut edited.functions[index];
            let actual = region.actuals()[0].stage();
            let ResourcePosition::Statement(stage) = actual.position() else {
                panic!("stage statement");
            };
            match mutation {
                0 => {
                    if let StatementKind::ResourceCall(ResourceCallNode::Stage {
                        source_index,
                        ..
                    }) =
                        &mut function.blocks[actual.block().index() as usize].statements[stage].kind
                    {
                        *source_index += 1;
                    }
                }
                1 => {
                    if let StatementKind::ResourceCall(ResourceCallNode::Stage {
                        parameter, ..
                    }) =
                        &mut function.blocks[actual.block().index() as usize].statements[stage].kind
                    {
                        *parameter = 0;
                    }
                }
                2 => {
                    if let StatementKind::ResourceCall(ResourceCallNode::Stage { value, .. }) =
                        &mut function.blocks[actual.block().index() as usize].statements[stage].kind
                    {
                        value.kind = E::Clone(Box::new(value.clone()));
                    }
                }
                3 => {
                    let statement =
                        function.blocks[actual.block().index() as usize].statements[stage].clone();
                    function.blocks[actual.block().index() as usize]
                        .statements
                        .insert(stage, statement);
                }
                4 => {
                    let ResourcePosition::Statement(begin) = region.begin().position() else {
                        panic!("begin statement");
                    };
                    function.blocks[region.begin().block().index() as usize]
                        .statements
                        .remove(begin);
                }
                5 => {
                    let (site, _) = region.invocation().unwrap();
                    let ResourcePosition::Statement(invoke) = site.position() else {
                        panic!("invoke statement");
                    };
                    function.blocks[site.block().index() as usize]
                        .statements
                        .remove(invoke);
                }
                6 => {
                    let (site, _) = region
                        .exits()
                        .iter()
                        .find(|(_, outcome)| *outcome == ResourceCompletion::Abort)
                        .unwrap();
                    let ResourcePosition::Statement(end) = site.position() else {
                        panic!("end statement");
                    };
                    function.blocks[site.block().index() as usize]
                        .statements
                        .remove(end);
                }
                7 => function.entry = region.actuals()[1].stage().block(),
                8 => {
                    let block =
                        &mut function.blocks[region.actuals()[1].start().block().index() as usize];
                    if let TerminatorKind::Branch {
                        then_block,
                        else_block,
                        ..
                    } = &mut block.terminator.kind
                    {
                        std::mem::swap(then_block, else_block);
                    } else {
                        panic!("exact Handle selector");
                    }
                }
                _ => unreachable!(),
            }
            assert!(
                validate_resource_ownership(&edited, types).is_err(),
                "mutation {mutation} must lose constructor authority"
            );
        }
    }
}

#[test]
fn normalized_call_canonical_compaction_preserves_archival_original_and_current_sites() {
    for release in [false, true] {
        let checked = checked(MINIMAL, release);
        let mut mir = lowered(&checked);
        let id = outer(&mir).id;
        let original = outer(&mir).resource_lowering.as_ref().unwrap().regions[0]
            .original()
            .clone();
        let function = mir
            .functions
            .iter_mut()
            .find(|function| function.id == id)
            .unwrap();
        assert!(crate::sequences::prune::unreachable(function));
        assert!(crate::sequences::prune::unused_locals(function));
        assert!(crate::breakpoint_regions::expressions_equal(
            function.resource_lowering.as_ref().unwrap().regions[0].original(),
            &original
        ));
        validate_resource_ownership(&mir, &checked.checked().interner).unwrap();
    }
}

#[test]
fn normalized_call_original_source_edits_refuse_before_constructor_extraction() {
    for release in [false, true] {
        let checked = checked(MINIMAL, release);
        let types = &checked.checked().interner;
        let original = hir::lower_checked_resource_program(&checked).unwrap();
        for mutation in 0..4 {
            let mut edited = original.clone();
            let function = edited
                .functions
                .iter_mut()
                .find(|function| function.identity.declaration.name == "main")
                .unwrap();
            let call = function
                .body
                .statements
                .iter_mut()
                .find_map(|statement| match &mut statement.kind {
                    hir::StatementKind::Expression(value)
                        if matches!(value.kind, E::Call { .. }) =>
                    {
                        Some(value)
                    }
                    _ => None,
                })
                .expect("original complete Source call");
            let call_span = call.span;
            let E::Call {
                args,
                evaluation_order,
                ownership: hir::CallOwnership::Source(source),
                ..
            } = &mut call.kind
            else {
                panic!("Source call");
            };
            match mutation {
                0 => evaluation_order.swap(0, 1),
                1 => {
                    if let hir::CallerOrigin::Binding(binding) = &mut source.arguments[1].origin {
                        binding.local = function.params[0].local;
                    } else {
                        panic!("original Resource Binding fact");
                    }
                }
                2 => source.arguments[1].syntax = jett_typecheck::CheckedCallerSyntax::WrittenView,
                3 => args[1].kind = E::Clone(Box::new(args[1].clone())),
                _ => unreachable!(),
            }
            let errors = lower(&edited, types).unwrap_err();
            let expected_message = match mutation {
                0 => {
                    "call ownership lexical evaluation order disagrees with its source permutation"
                }
                1 | 2 => "call ownership disagrees with its original source witness",
                3 => "call ownership binding does not match its actual typed operand",
                _ => unreachable!(),
            };
            assert_eq!(errors.len(), 1, "release={release}, mutation={mutation}");
            assert_eq!(errors[0].span, call_span);
            assert_eq!(errors[0].message, expected_message);
        }
    }
}

#[test]
fn normalized_handle_default_joins_the_same_operation_before_one_invocation() {
    for release in [false, true] {
        let source = MINIMAL.replace("\r\n", "\n").replace(
            "marker: fail_marker() handle error:\n        return nothing\n    , net: view net",
            "marker: (fail_marker() handle error: default 7), net: view net",
        );
        assert_ne!(source, MINIMAL, "exact canonical original marker Handle");
        let checked = checked(&source, release);
        let types = &checked.checked().interner;
        let mir = lowered(&checked);
        let function = outer(&mir);
        let region = &function.resource_lowering.as_ref().unwrap().regions[0];
        assert_eq!(region.exits().len(), 1);
        assert_eq!(region.exits()[0].1, ResourceCompletion::Normal);
        let plan = validate_resource_ownership(&mir, types).unwrap();
        ResourceCompanionPlan::analyze(&plan, function.id).unwrap();
        let flow = plan.function(function.id).unwrap();
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
        assert_eq!(
            flow.operations()
                .iter()
                .filter(|operation| operation.frame() == begin.frame()
                    && matches!(
                        operation.role(),
                        ResourceOperationRole::InvokeSourceFunction { .. }
                    ))
                .count(),
            1
        );
        assert!(
            !flow
                .operations()
                .iter()
                .any(|operation| operation.frame() == begin.frame()
                    && matches!(
                        operation.role(),
                        ResourceOperationRole::Complete {
                            outcome: ResourceCompletion::Abort
                        }
                    ))
        );
    }
}

#[test]
fn normalized_abandonment_retires_only_its_prefix_before_another_owner_return() {
    const SOURCE: &str = r#"namespace app
function accept_staged(marker: int64, view token: resource_probe.TestHandle, view net: Network) returns nothing:
    return nothing
function fail_marker() returns result[int64, string]:
    return fail("later marker")
function scenario(view net: Network, token: resource_probe.TestHandle, other: resource_probe.TestHandle) returns resource_probe.TestHandle:
    accept_staged(token: token, marker: fail_marker() handle error:
        return other
    , net: view net)
    return other
"#;
    for release in [false, true] {
        let checked = checked(SOURCE, release);
        let types = &checked.checked().interner;
        let mir = lowered(&checked);
        let function = outer(&mir);
        let region = &function.resource_lowering.as_ref().unwrap().regions[0];
        let plan = validate_resource_ownership(&mir, types).unwrap();
        ResourceCompanionPlan::analyze(&plan, function.id).unwrap();
        let flow = plan.function(function.id).unwrap();
        let other = function.params[2].local;
        let other_slot = flow.owner_slots().iter().find(|slot| matches!(slot.storage(), ResourceSlotStorage::Local { header } if header.id == other)).unwrap().id();
        for (site, outcome) in region.exits() {
            if *outcome != ResourceCompletion::Abort {
                continue;
            }
            assert!(!flow.operations().iter().any(|operation| operation.site() == *site && matches!(operation.role(), ResourceOperationRole::Drop { source, .. } if *source == other_slot)));
            assert!(flow.operations().iter().any(|operation| operation.site().block() == site.block() && operation.site().position() == ResourcePosition::Terminator && matches!(operation.role(), ResourceOperationRole::Transfer { source, .. } if *source == other_slot)));
        }
    }
}
