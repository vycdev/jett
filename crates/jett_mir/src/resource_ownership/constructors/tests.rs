use super::*;
use crate::resource_ownership::tests::{checked, lowered};

const SOURCES: [&str; 3] = [
    include_str!(
        "../../../../jett_comptime/src/resource_execution/fixtures/33_owned_struct_scope_drop_entry.jett"
    ),
    include_str!(
        "../../../../jett_comptime/src/resource_execution/fixtures/34_owned_struct_reverse_entry.jett"
    ),
    include_str!(
        "../../../../jett_comptime/src/resource_execution/fixtures/35_owned_struct_abandoned_prefix_entry.jett"
    ),
];

// Historical originals retain their exact invalid viewed-entry Source bytes.
const INVALID_VIEWED_ENTRY_SOURCES: [&str; 3] = [
    include_str!(
        "../../../../jett_comptime/src/resource_execution/fixtures/30_owned_struct_scope_drop.jett"
    ),
    include_str!(
        "../../../../jett_comptime/src/resource_execution/fixtures/31_owned_struct_reverse.jett"
    ),
    include_str!(
        "../../../../jett_comptime/src/resource_execution/fixtures/32_owned_struct_abandoned_prefix.jett"
    ),
];

#[test]
fn resource_occupied_struct_historical_viewed_entries_keep_exact_e0503_refusal() {
    for release in [false, true] {
        for source in INVALID_VIEWED_ENTRY_SOURCES {
            let stdlib = jett_common::FileId::new(10_000);
            let project = jett_common::FileId::new(0);
            let mut parsed = jett_parser::parse(crate::resource_ownership::tests::SUPPORT, stdlib);
            let primary = jett_parser::parse(source, project);
            assert!(parsed.errors.is_empty());
            assert!(primary.errors.is_empty());
            let declarations = parsed
                .module
                .items
                .iter()
                .filter_map(|item| match item {
                    jett_parser::ast::Item::Resource(resource) => Some(resource.name.span),
                    _ => None,
                })
                .collect::<Vec<_>>();
            let [declaration] = declarations.as_slice() else {
                panic!("historical Source must use the original sole checked support Resource");
            };
            let catalog = [
                (
                    "kernel_create",
                    jett_types::ResourceKernelRecipe::NetworkFactory,
                ),
                (
                    "kernel_borrow",
                    jett_types::ResourceKernelRecipe::NetworkBorrow,
                ),
                ("kernel_close", jett_types::ResourceKernelRecipe::Finalize),
            ]
            .into_iter()
            .map(|(member, recipe)| jett_resolve::ResourceKernelSpec {
                resource_declaration: *declaration,
                member: member.to_owned(),
                recipe,
            })
            .collect::<Vec<_>>();
            parsed.module.items.extend(primary.module.items);
            let checked = jett_typecheck::CheckedResourceProgram::prepare(
                parsed,
                std::collections::HashMap::from([
                    (stdlib, jett_common::SourceOrigin::Stdlib),
                    (project, jett_common::SourceOrigin::Project),
                ]),
                &catalog,
                jett_typecheck::CheckOptions { release },
            );
            let error = match checked {
                Err(error) => error,
                Ok(_) => panic!("historical viewed-main Source was admitted (release={release})"),
            };
            let diagnostics = error
                .diagnostics()
                .expect("historical E0503 checker diagnostics");
            assert!(
                diagnostics
                    .iter()
                    .any(|diagnostic| diagnostic.code.code() == 503
                        && diagnostic.span.file == project
                        && diagnostic.message
                            == "`main` must own runtime capability parameter `net`; remove `view`"),
                "historical viewed-main Source lost exact E0503 (release={release}): {:?}",
                diagnostics
                    .iter()
                    .map(|diagnostic| (
                        diagnostic.code.code(),
                        diagnostic.message.as_str(),
                        diagnostic.span
                    ))
                    .collect::<Vec<_>>()
            );
        }
    }
}

fn helper(program: &Program) -> &Function {
    program
        .functions
        .iter()
        .find(|f| {
            f.identity.declaration.namespace == "app"
                && matches!(
                    f.identity.declaration.name.as_str(),
                    "retire_owned" | "retire_pair"
                )
        })
        .unwrap()
}
fn helper_mut(program: &mut Program) -> &mut Function {
    program
        .functions
        .iter_mut()
        .find(|f| {
            f.identity.declaration.namespace == "app"
                && matches!(
                    f.identity.declaration.name.as_str(),
                    "retire_owned" | "retire_pair"
                )
        })
        .unwrap()
}
fn reseal_general(function: &mut Function) {
    let w = function.resource_lowering.as_mut().unwrap();
    w.blocks = function.blocks.clone();
    w.locals = function.locals.clone();
    w.parameters = function.params.clone();
}
fn refused(program: &Program, types: &TypeInterner) {
    assert!(
        validate_resource_ownership(program, types).is_err(),
        "forged constructor ownership was admitted"
    );
    let mut selected = helper(program).clone();
    let before = selected.clone();
    assert!(
        !crate::sequences::prune::unreachable(&mut selected),
        "forged constructor prune accepted; private witness present={}",
        before.resource_lowering.is_some()
    );
    assert!(
        crate::breakpoint_regions::blocks_equal(&selected.blocks, &before.blocks),
        "refused prune must preserve public graph"
    );
    let mut selected = before.clone();
    assert!(!crate::sequences::prune::unused_locals(&mut selected));
    assert_eq!(selected.locals, before.locals);
    assert!(crate::breakpoint_regions::blocks_equal(
        &selected.blocks,
        &before.blocks
    ));
}

#[test]
fn resource_occupied_struct_constructor_preserves_owned_formals_lexical_order_and_operation_output()
{
    for release in [false, true] {
        for (index, source) in SOURCES.into_iter().enumerate() {
            let checked = checked(source, release);
            let types = &checked.checked().interner;
            let mut program = lowered(&checked);
            for canonical in [false, true] {
                if canonical {
                    crate::prepare_native_sequences(&mut program, types);
                    crate::prepare_native_uninhabited_sums(&mut program, types);
                    crate::prepare_native_generated_functions(&mut program, types);
                }
                crate::validate(&program).unwrap();
                validate_call_ownership(&program, types).unwrap();
                let function = helper(&program);
                let witness = function.resource_lowering.as_ref().unwrap();
                assert_eq!(witness.constructors.len(), 1);
                let region = function
                    .resource_constructor(witness.constructors[0].id())
                    .unwrap();
                assert_eq!(
                    region.evaluation_order(),
                    if index == 1 {
                        &[1, 0, 2][..]
                    } else {
                        &[0, 1][..]
                    }
                );
                let plan = validate_resource_ownership(&program, types).unwrap();
                let selected = plan.function(function.id).unwrap();
                let carriers = selected.carriers().unwrap();
                let begin = carriers
                    .operations()
                    .iter()
                    .find(|operation| {
                        operation.site() == region.begin()
                            && matches!(
                                operation.role(),
                                ResourceCarrierOperationRole::BeginConstructor { .. }
                            )
                    })
                    .unwrap();
                let ResourceCarrierOperationRole::BeginConstructor { destination, .. } =
                    begin.role()
                else {
                    unreachable!()
                };
                let frame = begin.frame();
                assert_eq!(
                    selected.frames()[frame.index()].role(),
                    ResourceFrameRole::Operation
                );
                assert_eq!(
                    selected.frames()[frame.index()].parent(),
                    Some(selected.root_scope().id())
                );
                for (parameter, param) in function.params.iter().enumerate() {
                    assert_eq!(param.mode, ParamMode::Owned);
                    let slot = selected.owner_slots().iter().find(|slot| matches!(slot.storage(),ResourceSlotStorage::Local { header } if header.id == param.local)).unwrap();
                    assert!(matches!(slot.shape(), ResourceShape::Plain { .. }));
                    assert!(carriers.operations().iter().any(|operation| matches!(operation.role(),ResourceCarrierOperationRole::ConstructorChild { child, .. } if child.value == ResourceCarrierValue::LeafOwned { slot:slot.id() })) ,"formal {parameter} must be adopted through its exact plain holder");
                }
                let commit = carriers.operations().iter().find(|operation| matches!(operation.role(),ResourceCarrierOperationRole::CommitConstructor { destination:found } if found == destination)).unwrap();
                let (_, output) = region.commit().unwrap();
                assert!(
                    output.mutable,
                    "constructor output keeps its exact generated temporary header"
                );
                let output_slot = carriers.slots().iter().find(|slot| matches!(slot.storage(),ResourceSlotStorage::Local { header } if header.id == output.id)).unwrap();
                assert_eq!(output_slot.frame(), selected.root_scope().id());
                let transfer = carriers.operations().iter().find(|operation| operation.site() == commit.site() && matches!(operation.role(),ResourceCarrierOperationRole::Transfer { source,destination:to } if source == destination && *to == output_slot.id())).unwrap();
                assert!(transfer.id().index() > commit.id().index());
                assert_eq!(transfer.frame(), frame);
                for (site, outcome) in region.exits() {
                    assert!(selected.operations().iter().any(|operation| operation.site() == *site && operation.frame() == frame && matches!(operation.role(),ResourceOperationRole::Complete { outcome:found } if found == outcome)));
                }
                if index == 2 {
                    assert_eq!(
                        region
                            .exits()
                            .iter()
                            .filter(|(_, o)| *o == ResourceCompletion::Abort)
                            .count(),
                        1
                    );
                    assert_ne!(
                        region.fields()[0].stage().block(),
                        region.fields()[1].stage().block()
                    );
                    for field in region.fields() {
                        assert_eq!(
                            selected
                                .execution_frame(field.stage().block(), field.stage().position()),
                            Some(frame)
                        );
                    }
                }
                assert!(
                    crate::copy_values::CopyValuePlan::analyze_storage(
                        function,
                        types,
                        Some(&program)
                    )
                    .is_err()
                );
                assert!(
                    crate::move_values::MoveValuePlan::analyze(&program, function, types).is_err()
                );
                ResourceCompanionPlan::analyze(&plan, function.id).unwrap();
            }
        }
    }
}

#[test]
fn occupied_struct_primitive_companions_mir_preserves_each_exact_original_endpoint() {
    let sources = [
        include_str!(
            "../../../../jett_comptime/src/resource_execution/fixtures/36_owned_struct_primitive_companions.jett"
        ),
        include_str!(
            "../../../../jett_comptime/src/resource_execution/fixtures/37_owned_struct_alias_companions.jett"
        ),
        include_str!(
            "../../../../jett_comptime/src/resource_execution/fixtures/38_owned_struct_pending_int64_transport.jett"
        ),
        include_str!(
            "../../../../jett_comptime/src/resource_execution/fixtures/40_owned_struct_pending_companions.jett"
        ),
        include_str!(
            "../../../../jett_comptime/src/resource_execution/fixtures/42_owned_struct_pending_primitive_matrix.jett"
        ),
    ];
    for release in [false, true] {
        for source in sources {
            let support = format!(
                "{}\n{}",
                crate::resource_ownership::tests::SUPPORT,
                include_str!(
                    "../../../../jett_comptime/src/resource_execution/fixtures/resource_companion_probe.jett"
                )
            );
            let checked =
                crate::resource_ownership::tests::checked_support(source, &support, release);
            let types = &checked.checked().interner;
            let mut program = lowered(&checked);
            for canonical in [false, true] {
                if canonical {
                    crate::prepare_native_sequences(&mut program, types);
                    crate::prepare_native_uninhabited_sums(&mut program, types);
                    crate::prepare_native_generated_functions(&mut program, types);
                }
                let plan = validate_resource_ownership(&program, types).unwrap();
                let function = program
                    .functions
                    .iter()
                    .find(|function| {
                        function.identity.declaration.namespace == "app"
                            && function.identity.declaration.name.starts_with("retire_")
                    })
                    .unwrap();
                let witness = function.resource_lowering.as_ref().unwrap();
                assert_eq!(witness.constructors.len(), 1);
                let region = function
                    .resource_constructor(witness.constructors[0].id())
                    .unwrap();
                let selected = plan.function(function.id).unwrap();
                let carriers = selected.carriers().unwrap();
                for field in region.fields() {
                    let child =
                        carriers
                            .operations()
                            .iter()
                            .find_map(|operation| match operation.role() {
                                ResourceCarrierOperationRole::ConstructorChild {
                                    child, ..
                                } if operation.site() == field.stage() => Some(child),
                                _ => None,
                            })
                            .unwrap();
                    assert_eq!(child.index, field.field());
                    if !resource_type_pending(types, field.original().ty) {
                        assert!(
                            matches!(child.value, ResourceCarrierValue::Ordinary { ty } if ty == field.original().ty)
                        );
                    }
                }
                let mut forged = program.clone();
                let function = forged
                    .functions
                    .iter_mut()
                    .find(|current| current.id == function.id)
                    .unwrap();
                let stage = function
                    .blocks
                    .iter_mut()
                    .flat_map(|block| &mut block.statements)
                    .find_map(|statement| match &mut statement.kind {
                        StatementKind::ResourceConstructor(ResourceConstructorNode::Stage {
                            value,
                            ..
                        }) if !resource_type_pending(types, value.ty) => Some(value),
                        _ => None,
                    })
                    .unwrap();
                stage.ty = if stage.ty == TypeInterner::INT8 {
                    TypeInterner::INT16
                } else {
                    TypeInterner::INT8
                };
                assert!(
                    validate_resource_ownership(&forged, types).is_err(),
                    "changing width must not reseal an original constructor"
                );
            }
        }
    }
}

#[test]
fn occupied_struct_pending_qualifier_keeps_exact_observation_before_and_after_canonicalization() {
    const SOURCE: &str = include_str!(
        "../../../../jett_comptime/src/resource_execution/fixtures/38_owned_struct_pending_int64_transport.jett"
    );
    let support = format!(
        "{}\n{}",
        crate::resource_ownership::tests::SUPPORT,
        include_str!(
            "../../../../jett_comptime/src/resource_execution/fixtures/resource_companion_probe.jett"
        )
    );
    for release in [false, true] {
        let checked = crate::resource_ownership::tests::checked_support(SOURCE, &support, release);
        let types = &checked.checked().interner;
        let mut program = lowered(&checked);
        for canonical in [false, true] {
            if canonical {
                crate::prepare_native_sequences(&mut program, types);
                crate::prepare_native_uninhabited_sums(&mut program, types);
                crate::prepare_native_generated_functions(&mut program, types);
            }
            let plan = validate_resource_ownership(&program, types).unwrap();
            let function = program
                .functions
                .iter()
                .find(|function| {
                    function.identity.declaration.namespace == "app"
                        && function.identity.declaration.name == "retire_pending"
                })
                .unwrap();
            let observations = plan.function(function.id).unwrap().carriers().unwrap().operations().iter().filter(|operation| matches!(operation.role(), ResourceCarrierOperationRole::Observe { observation: ResourceCarrierObservation::Ordinary { ty, .. }, .. } if *ty == TypeInterner::INT64)).count();
            assert_eq!(
                observations, 1,
                "the nested Join keeps one exact typed field observation"
            );
            let mut forged = program.clone();
            let changed = forged
                .functions
                .iter_mut()
                .find(|candidate| candidate.id == function.id)
                .unwrap()
                .blocks
                .iter_mut()
                .flat_map(|block| &mut block.statements)
                .find_map(|statement| {
                    let StatementKind::Let { value, .. } = &mut statement.kind else {
                        return None;
                    };
                    let hir::ExpressionKind::Join(inner) = &mut value.kind else {
                        return None;
                    };
                    matches!(inner.kind, hir::ExpressionKind::Field { .. }).then_some(inner)
                })
                .unwrap();
            changed.ty = TypeInterner::INT8;
            assert!(
                validate_resource_ownership(&forged, types).is_err(),
                "nested qualifier cannot redirect the original field width"
            );
        }
    }
}

#[test]
fn resource_occupied_struct_constructor_refuses_changed_fields_copied_nodes_and_lost_proof_before_prune()
 {
    for release in [false, true] {
        let checked = checked(SOURCES[1], release);
        let types = &checked.checked().interner;
        let program = lowered(&checked);
        validate_resource_ownership(&program, types).unwrap();
        for mutation in 0..8 {
            let mut forged = program.clone();
            let function = helper_mut(&mut forged);
            let region = function.resource_lowering.as_ref().unwrap().constructors[0].clone();
            match mutation {
                0 => {
                    let site = region.fields()[0].stage();
                    let ResourcePosition::Statement(i) = site.position() else {
                        unreachable!()
                    };
                    let StatementKind::ResourceConstructor(ResourceConstructorNode::Stage {
                        field,
                        ..
                    }) = &mut function.blocks[site.block().index() as usize].statements[i].kind
                    else {
                        unreachable!()
                    };
                    *field = 0;
                }
                1 => {
                    let site = region.begin();
                    let ResourcePosition::Statement(i) = site.position() else {
                        unreachable!()
                    };
                    let copy = function.blocks[site.block().index() as usize].statements[i].clone();
                    function.blocks[site.block().index() as usize]
                        .statements
                        .insert(i + 1, copy);
                }
                2 => {
                    function.resource_lowering.as_mut().unwrap().constructors[0].fields[0]
                        .original = region.fields[1].original.clone();
                }
                3 => {
                    function.resource_lowering.as_mut().unwrap().constructors[0].fields[0].start =
                        region.fields[1].start;
                }
                4 => {
                    function.resource_lowering.as_mut().unwrap().constructors[0]
                        .order
                        .swap(0, 1);
                }
                5 => {
                    let (_, output) = region.commit().unwrap();
                    function.locals[output.id.index() as usize].view_source =
                        Some(function.params[0].local);
                }
                6 => {
                    function.resource_lowering = None;
                    refused(&forged, types);
                    continue;
                }
                7 => {
                    let (_, output) = region.commit().unwrap();
                    assert!(function.locals[output.id.index() as usize].mutable);
                    function.locals[output.id.index() as usize].mutable = false;
                }
                _ => unreachable!(),
            }
            reseal_general(function);
            refused(&forged, types);
        }
    }
}

#[test]
fn resource_occupied_struct_constructor_disconnected_stage_and_wrong_prefix_remain_refused() {
    let checked = checked(SOURCES[2], false);
    let types = &checked.checked().interner;
    let program = lowered(&checked);
    for disconnect in [false, true] {
        let mut forged = program.clone();
        let function = helper_mut(&mut forged);
        let region = function.resource_lowering.as_ref().unwrap().constructors[0].clone();
        let site = region.fields()[0].stage();
        let ResourcePosition::Statement(i) = site.position() else {
            unreachable!()
        };
        if disconnect {
            let id = BlockId(u32::try_from(function.blocks.len()).unwrap());
            function.blocks.push(BasicBlock {
                id,
                statements: vec![
                    function.blocks[site.block().index() as usize].statements[i].clone(),
                ],
                terminator: Terminator {
                    kind: TerminatorKind::Unreachable,
                    span: function.span,
                },
            });
        } else {
            let end = region
                .exits()
                .iter()
                .find(|(_, o)| *o == ResourceCompletion::Abort)
                .unwrap()
                .0;
            let ResourcePosition::Statement(i) = end.position() else {
                unreachable!()
            };
            function.blocks[end.block().index() as usize]
                .statements
                .remove(i);
        }
        reseal_general(function);
        refused(&forged, types);
    }
}
