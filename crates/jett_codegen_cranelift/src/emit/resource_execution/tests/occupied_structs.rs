use super::*;
use jett_mir::{ResourceCarrierOperationRole as CarrierRole, ResourceCarrierValue, ResourceShape};

const SOURCES: [&str; 3] = [
    include_str!(
        "../../../../../jett_comptime/src/resource_execution/fixtures/33_owned_struct_scope_drop_entry.jett"
    ),
    include_str!(
        "../../../../../jett_comptime/src/resource_execution/fixtures/34_owned_struct_reverse_entry.jett"
    ),
    include_str!(
        "../../../../../jett_comptime/src/resource_execution/fixtures/35_owned_struct_abandoned_prefix_entry.jett"
    ),
];

#[test]
fn resource_occupied_struct_actual_sources_emit_carrier_and_plain_owner_adoption_in_both_profiles()
{
    for release in [false, true] {
        for source in SOURCES {
            let checked = checked(source, release);
            let types = &checked.checked().interner;
            let mut program = lower_source(&checked);
            jett_mir::prepare_native_sequences(&mut program, types);
            jett_mir::prepare_native_uninhabited_sums(&mut program, types);
            jett_mir::prepare_native_generated_functions(&mut program, types);
            jett_mir::validate_call_ownership(&program, types).unwrap();
            let selected =
                jett_mir::validate_resource_ownership_for_entry(&program, types, entry(&program))
                    .unwrap();
            let helper = program
                .functions
                .iter()
                .find(|f| {
                    f.identity.declaration.namespace == "app"
                        && matches!(
                            f.identity.declaration.name.as_str(),
                            "retire_owned" | "retire_pair"
                        )
                })
                .unwrap();
            let plan = selected.function(helper.id).unwrap();
            let carrier = plan.carriers().unwrap();
            assert!(carrier.operations().iter().any(|op| matches!(op.role(),CarrierRole::ConstructorChild {child,..} if matches!(child.value,ResourceCarrierValue::LeafOwned {slot} if matches!(plan.owner_slots()[slot.index()].shape(),ResourceShape::Plain {..})))));
            let artifact = super::super::super::emit_for_triple(
                &program,
                types,
                HOST,
                Some(entry(&program)),
                CodegenOptions { optimize: release },
            )
            .unwrap();
            let (definitions, imports) = symbols(&artifact);
            assert!(definitions.iter().any(|n| n == JETT_AOT_ENTRY_SYMBOL_V1));
            for leaf in [
                "carrier_construct_begin",
                "carrier_child",
                "carrier_commit",
                "carrier_transfer",
                "operation_begin",
                "operation_complete",
                "scope_complete",
                "source_status",
            ] {
                assert!(
                    imports
                        .iter()
                        .any(|n| n == &format!("jett_rt_v1_resource_{leaf}")),
                    "missing {leaf}"
                );
            }
        }
    }
}

#[test]
fn resource_occupied_struct_public_copied_field_or_header_cannot_reach_ordinary_emission() {
    for release in [false, true] {
        let checked = checked(SOURCES[1], release);
        let types = &checked.checked().interner;
        let original = lower_source(&checked);
        for mutation in 0..3 {
            let mut program = original.clone();
            let function = program
                .functions
                .iter_mut()
                .find(|f| {
                    f.identity.declaration.namespace == "app"
                        && f.identity.declaration.name == "retire_pair"
                })
                .unwrap();
            let (b, i) = function
                .blocks
                .iter()
                .enumerate()
                .find_map(|(b, block)| {
                    block.statements.iter().enumerate().find_map(|(i, s)| {
                        matches!(
                            s.kind,
                            StatementKind::ResourceConstructor(
                                jett_mir::ResourceConstructorNode::Stage {
                                    source_index: 0,
                                    ..
                                }
                            )
                        )
                        .then_some((b, i))
                    })
                })
                .unwrap();
            match mutation {
                0 => {
                    let StatementKind::ResourceConstructor(
                        jett_mir::ResourceConstructorNode::Stage { field, .. },
                    ) = &mut function.blocks[b].statements[i].kind
                    else {
                        unreachable!()
                    };
                    *field = 0;
                }
                1 => {
                    let copy = function.blocks[b].statements[i].clone();
                    function.blocks[b].statements.insert(i + 1, copy);
                }
                2 => {
                    function.locals[function.params[0].local.index() as usize].view_source =
                        Some(function.params[1].local);
                }
                _ => unreachable!(),
            }
            let selected = entry(&program);
            let result = super::super::super::emit_for_triple(
                &program,
                types,
                HOST,
                Some(selected),
                CodegenOptions { optimize: release },
            );
            assert!(
                matches!(result, Err(CodegenError::InvalidMir(_))),
                "forged constructor must refuse at fresh ownership validation"
            );
        }
    }
}
