use super::*;
use crate::resource_ownership::tests::checked;
use crate::resource_ownership::tests::checked_support;
use jett_comptime::ExplicitComptimeValues;
use jett_comptime::checked_types::CheckedExpressionTypes;
use jett_typecheck::CheckedResourceProgram;
use std::{collections::HashMap, sync::Arc};

const FIXTURE_SUPPORT: &str =
    include_str!("../../../jett_driver/tests/native_conformance/resource/resource_probe.jett");
const PURE_CHOICE: &str = include_str!(
    "../../../jett_driver/tests/native_conformance/resource/41_comptime_hook_pure_choice.jett"
);
const ABSENCE: &str = include_str!(
    "../../../jett_driver/tests/native_conformance/resource/40_comptime_hook_absence.jett"
);

fn required(checked: &Arc<CheckedResourceProgram>) -> ExplicitComptimeValues {
    let expression_types = Arc::new(CheckedExpressionTypes {
        resource_program: Some(checked.clone()),
        expressions: checked
            .checked()
            .type_map
            .iter()
            .map(|(span, ty)| (*span, checked.checked().interner.type_name(*ty)))
            .collect(),
        ..Default::default()
    });
    let captured = jett_comptime::evaluate_explicit_comptime_expressions_capture(
        checked.module(),
        Arc::new(jett_types::ReflectionMetadata::new()),
        expression_types,
        Arc::new(HashMap::new()),
    );
    assert!(
        captured.diagnostics.is_empty(),
        "{:?}",
        captured.diagnostics
    );
    assert!(captured.debug_events.is_empty());
    captured.values
}

fn materialized(checked: &Arc<CheckedResourceProgram>) -> hir::Program {
    let mut program = hir::lower_checked_resource_program(checked).unwrap();
    hir::materialize_checked_required_values(
        &mut program,
        &required(checked),
        &checked.checked().interner,
    )
    .unwrap();
    program
}

fn named<'a>(program: &'a Program, name: &str) -> &'a Function {
    program
        .functions
        .iter()
        .find(|function| {
            function.identity.declaration.namespace == "app"
                && function.identity.declaration.name == name
        })
        .unwrap()
}

fn named_mut<'a>(program: &'a mut Program, name: &str) -> &'a mut Function {
    program
        .functions
        .iter_mut()
        .find(|function| {
            function.identity.declaration.namespace == "app"
                && function.identity.declaration.name == name
        })
        .unwrap()
}

#[test]
fn resource_required_descriptors_preserve_checked_values_and_exact_current_occurrences() {
    let sources = [
        include_str!(
            "../../../jett_driver/tests/native_conformance/resource/33_comptime_hook_close.jett"
        ),
        include_str!(
            "../../../jett_driver/tests/native_conformance/resource/34_comptime_hook_factory.jett"
        ),
        include_str!(
            "../../../jett_driver/tests/native_conformance/resource/35_comptime_hook_borrow.jett"
        ),
        include_str!(
            "../../../jett_driver/tests/native_conformance/resource/36_comptime_hook_alias.jett"
        ),
        include_str!(
            "../../../jett_driver/tests/native_conformance/resource/37_comptime_hook_relay.jett"
        ),
        include_str!(
            "../../../jett_driver/tests/native_conformance/resource/38_comptime_hook_unused.jett"
        ),
        include_str!(
            "../../../jett_driver/tests/native_conformance/resource/39_immediate_comptime_hook_close.jett"
        ),
        include_str!(
            "../../../jett_driver/tests/native_conformance/resource/40_comptime_hook_absence.jett"
        ),
        PURE_CHOICE,
        include_str!(
            "../../../jett_driver/tests/native_conformance/resource/42_comptime_hook_generic.jett"
        ),
        include_str!(
            "../../../jett_driver/tests/native_conformance/resource/43_comptime_hook_scoped.jett"
        ),
    ];
    for release in [false, true] {
        for (fixture, source) in sources.into_iter().enumerate() {
            let checked = checked_support(source, FIXTURE_SUPPORT, release);
            let types = &checked.checked().interner;
            let hir = materialized(&checked);
            let program = crate::lower(&hir, types).unwrap();
            let ownership = validate_resource_ownership(&program, types).unwrap_or_else(|errors| {
                panic!(
                    "required fixture {} release={release}: {errors:?}; main CFG={:#?}",
                    fixture + 33,
                    named(&program, "main").blocks
                )
            });
            assert_eq!(
                ownership.required_only_function_ids(),
                hir.resource_source.required_only_function_ids()
            );
            let function = named(&program, "main");
            let plan = ownership.function(function.id).unwrap();
            let original = ownership.original_source(function.id).unwrap();
            let mut original_required = false;
            walk::hir_block(&original.body, &mut |value| {
                original_required |= matches!(value.kind, E::Comptime { .. });
            });
            assert!(original_required);
            let witness = function.resource_lowering.as_ref().unwrap();
            assert!(witness.descriptors.graph.is_some());
            for row in hir.resource_source.required_materializations() {
                if row.function() != function.id || row.hook().is_none() {
                    continue;
                }
                assert!(witness.descriptors.values.iter().any(|value| {
                    crate::breakpoint_regions::expressions_equal(&value.original, row.original())
                        && crate::breakpoint_regions::expressions_equal(
                            &value.current,
                            row.current(),
                        )
                        && Some(&value.hook) == row.hook()
                }));
                // An archive value is data, never the current MIR occurrence.
                assert!(plan.descriptor_value(row.current()).is_none());
            }
            for block in &function.blocks {
                walk::mir_block(block, &mut |value| {
                    if let E::ResourceHookValue { hook } = &value.kind {
                        assert_eq!(plan.descriptor_value(value), Some(hook));
                        assert!(plan.descriptor_value(&value.clone()).is_none());
                    }
                });
            }
            let companion = ResourceCompanionPlan::analyze(&ownership, function.id).unwrap();
            for local in &function.locals {
                if plan.descriptor_local(local.id).is_some() {
                    assert!(
                        !companion
                            .storage()
                            .owned_locals
                            .contains(&(local.id.index() as usize))
                    );
                    assert!(!plan.owner_slots().iter().any(|slot| matches!(
                        slot.storage(), ResourceSlotStorage::Local { header } if header.id == local.id
                    )));
                }
            }
            for required_only in ownership.required_only_function_ids() {
                let helper = &program.functions[required_only.index() as usize];
                assert_eq!(helper.id, *required_only);
                assert!(ownership.function(*required_only).is_none());
                assert!(ownership.original_source(*required_only).is_some());
                let descriptors = &helper.resource_lowering.as_ref().unwrap().descriptors;
                assert!(descriptors.graph.is_some());
                assert!(descriptors.returned.is_none());
                assert!(descriptors.values.is_empty());
                assert!(descriptors.bindings.is_empty());
            }
            for plan in ownership.functions() {
                ResourceCompanionPlan::analyze(&ownership, plan.function()).unwrap();
            }
        }
    }
}

#[test]
fn resource_required_descriptors_reject_unmaterialized_and_foreign_archive_substitution() {
    for release in [false, true] {
        let checked = checked(SOURCE, release);
        let types = &checked.checked().interner;
        let original = hir::lower_checked_resource_program(&checked).unwrap();
        let mut current = materialized(&checked);
        current.resource_source = original.resource_source;
        assert!(crate::lower(&current, types).is_err());

        let baseline = crate::lower(&materialized(&checked), types).unwrap();
        validate_resource_ownership(&baseline, types).unwrap();
        let foreign = self::checked(SOURCE, release);
        let foreign_hir = materialized(&foreign);
        let foreign_mir = crate::lower(&foreign_hir, &foreign.checked().interner).unwrap();
        let mut changed = baseline.clone();
        let function = named_mut(&mut changed, "scenario");
        function.resource_lowering.as_mut().unwrap().source = named(&foreign_mir, "scenario")
            .resource_lowering
            .as_ref()
            .unwrap()
            .source
            .clone();
        assert!(validate_resource_ownership(&changed, types).is_err());
    }
}

#[test]
fn resource_required_only_helpers_refuse_resealed_control_flow_and_runtime_authority() {
    for release in [false, true] {
        let checked = checked_support(PURE_CHOICE, FIXTURE_SUPPORT, release);
        let types = &checked.checked().interner;
        let baseline = crate::lower(&materialized(&checked), types).unwrap();
        let ownership = validate_resource_ownership(&baseline, types).unwrap();
        let helper_id = named(&baseline, "choose").id;
        assert!(ownership.required_only_function_ids().contains(&helper_id));
        let mut changed = baseline.clone();
        let helper = named_mut(&mut changed, "choose");
        let branch = helper
            .blocks
            .iter_mut()
            .find(|block| matches!(block.terminator.kind, TerminatorKind::Branch { .. }))
            .unwrap();
        if let TerminatorKind::Branch {
            then_block,
            else_block,
            ..
        } = &mut branch.terminator.kind
        {
            *then_block = *else_block;
        }
        helper.resource_lowering.as_mut().unwrap().blocks = helper.blocks.clone();
        assert!(!crate::sequences::prune::unreachable(helper));
        assert!(validate_resource_ownership(&changed, types).is_err());

        let mut changed = baseline.clone();
        let hook = named(&baseline, "main")
            .resource_lowering
            .as_ref()
            .unwrap()
            .descriptors
            .bindings[0]
            .value
            .hook
            .clone();
        named_mut(&mut changed, "choose")
            .resource_lowering
            .as_mut()
            .unwrap()
            .descriptors
            .returned = Some(hook);
        assert!(validate_resource_ownership(&changed, types).is_err());

        let mut changed = baseline.clone();
        named_mut(&mut changed, "choose").resource_lowering = None;
        assert!(validate_resource_ownership(&changed, types).is_err());
    }
}

#[test]
fn resource_required_scalar_body_refuses_resealed_branch_before_canonical_pruning() {
    const SCALAR: &str = r#"namespace app
export function main(net: Network) returns nothing:
    use resource_probe
    bool stop = comptime true
    if stop:
        int64 ignored = resource_probe.terminal_label()
    resource_probe.TestHandle token = resource_probe.create(view net, 761) handle error:
        return nothing
    resource_probe.close(token)
    return nothing
"#;
    for release in [false, true] {
        let checked = checked(SCALAR, release);
        let types = &checked.checked().interner;
        let hir = materialized(&checked);
        let mut changed = crate::lower(&hir, types).unwrap();
        validate_resource_ownership(&changed, types).unwrap();
        let main = named_mut(&mut changed, "main");
        let descriptors = &main.resource_lowering.as_ref().unwrap().descriptors;
        assert!(descriptors.values.is_empty());
        assert!(descriptors.returned.is_none());
        assert!(
            hir.resource_source
                .required_materializations()
                .iter()
                .any(|row| {
                    row.function() == main.id && matches!(row.current().kind, E::Bool(true))
                })
        );
        let branch = main
            .blocks
            .iter_mut()
            .find(|block| matches!(block.terminator.kind, TerminatorKind::Branch { .. }))
            .unwrap();
        if let TerminatorKind::Branch {
            then_block,
            else_block,
            ..
        } = &mut branch.terminator.kind
        {
            assert_ne!(*then_block, *else_block);
            *then_block = *else_block;
        }
        main.resource_lowering.as_mut().unwrap().blocks = main.blocks.clone();
        assert!(validate_resource_ownership(&changed, types).is_err());
        let accepted = crate::sequences::prune::unreachable(named_mut(&mut changed, "main"));
        assert!(!accepted);
        assert!(validate_resource_ownership(&changed, types).is_err());
    }
}

#[test]
fn resource_required_absence_plans_exact_tag_arms_and_refuses_disconnected_custody() {
    for release in [false, true] {
        let checked = checked_support(ABSENCE, FIXTURE_SUPPORT, release);
        let types = &checked.checked().interner;
        let baseline = crate::lower(&materialized(&checked), types).unwrap();
        let ownership = validate_resource_ownership(&baseline, types).unwrap();
        let main = named(&baseline, "main");
        let plan = ownership.function(main.id).unwrap();
        assert!(plan.operations().iter().any(|operation| matches!(
            operation.role(),
            ResourceOperationRole::CreateAbsentSum { .. }
        )));
        assert!(plan.operations().iter().any(|operation| matches!(
            operation.role(),
            ResourceOperationRole::SumTake { success: true, .. }
        )));
        let close = baseline
            .functions
            .iter()
            .find(|function| {
                function.identity.declaration.namespace == "resource_probe"
                    && function.identity.declaration.name == "close"
            })
            .unwrap();
        assert!(plan.operations().iter().any(|operation| matches!(
            operation.role(), ResourceOperationRole::InvokeSourceFunction { function, .. }
                if *function == close.id
        )));
        assert!(
            ownership
                .function(close.id)
                .unwrap()
                .operations()
                .iter()
                .any(|operation| matches!(operation.role(), ResourceOperationRole::Close { .. }))
        );
        let mut changed = baseline.clone();
        let main = named_mut(&mut changed, "main");
        let branch = main
            .blocks
            .iter_mut()
            .find(|block| matches!(block.terminator.kind, TerminatorKind::Branch { .. }))
            .unwrap();
        if let TerminatorKind::Branch { condition, .. } = &mut branch.terminator.kind {
            condition.kind = E::Bool(false);
        }
        let error = match crate::resource_ownership::flow::analyze(
            &changed,
            named(&changed, "main"),
            types,
        ) {
            Err(error) => error,
            Ok(_) => panic!("wrong selecting tag admitted; release={release}"),
        };
        assert!(
            error.contains("Resource SumTake has no exact selecting typed tag edge"),
            "{error}"
        );

        let mut changed = baseline.clone();
        let main = named_mut(&mut changed, "main");
        let mut disconnected = main
            .blocks
            .iter()
            .find(|block| {
                let mut close_call = false;
                walk::mir_block(block, &mut |value| {
                    close_call |=
                        matches!(value.kind, E::Call { function, .. } if function == close.id);
                });
                close_call
            })
            .unwrap()
            .clone();
        assert!(walk::mir_block_has_custody(&disconnected, main, types));
        disconnected.id = BlockId(main.blocks.len() as u32);
        let disconnected_id = disconnected.id;
        main.blocks.push(disconnected);
        assert!(
            !ControlFlowGraph::analyze(main)
                .unwrap()
                .reverse_postorder()
                .contains(&disconnected_id)
        );
        let error = match crate::resource_ownership::flow::analyze(
            &changed,
            named(&changed, "main"),
            types,
        ) {
            Err(error) => error,
            Ok(_) => panic!("disconnected Resource close admitted; release={release}"),
        };
        // The Return's lexical-exit query authenticates the entire current graph
        // before flow reaches its final disconnected-custody walk.
        assert_eq!(
            error,
            "Resource ownership differs from its initially authenticated Source or constructor-emitted graph"
        );
        assert!(validate_resource_ownership(&changed, types).is_err());
    }
}

#[test]
fn resource_required_absence_refuses_disconnected_sum_take_header() {
    for release in [false, true] {
        let checked = checked_support(ABSENCE, FIXTURE_SUPPORT, release);
        let types = &checked.checked().interner;
        let baseline = crate::lower(&materialized(&checked), types).unwrap();
        validate_resource_ownership(&baseline, types).unwrap();
        let mut changed = baseline.clone();
        let main = named_mut(&mut changed, "main");
        let mut disconnected = main
            .blocks
            .iter()
            .find(|block| {
                block.statements.iter().any(|statement| {
                    matches!(statement.kind, StatementKind::SumTake { success: true, .. })
                })
            })
            .unwrap()
            .clone();
        assert!(walk::mir_block_has_custody(&disconnected, main, types));
        disconnected.id = BlockId(main.blocks.len() as u32);
        let disconnected_id = disconnected.id;
        main.blocks.push(disconnected);
        assert!(
            !ControlFlowGraph::analyze(main)
                .unwrap()
                .reverse_postorder()
                .contains(&disconnected_id)
        );
        let error = match crate::resource_ownership::flow::analyze(
            &changed,
            named(&changed, "main"),
            types,
        ) {
            Err(error) => error,
            Ok(_) => panic!("disconnected Resource SumTake header admitted; release={release}"),
        };
        // The whole-body authentication refuses this disconnected typed header
        // before flow reaches its final disconnected-custody walk.
        assert_eq!(
            error,
            "Resource ownership differs from its initially authenticated Source or constructor-emitted graph"
        );
        assert!(validate_resource_ownership(&changed, types).is_err());
    }
}

#[test]
fn resource_required_header_detector_preserves_ordinary_sum_classification() {
    const ORDINARY: &str = r#"namespace app
function ordinary() returns int64:
    optional[int64] maybe = none
    int64 number = maybe handle:
        default 0
    return number
"#;
    for release in [false, true] {
        let checked = checked(ORDINARY, release);
        let types = &checked.checked().interner;
        let hir = hir::lower_checked_resource_program(&checked).unwrap();
        let program = crate::lower(&hir, types).unwrap();
        let function = named(&program, "ordinary");
        let mut header_count = 0;
        for block in &function.blocks {
            if block.statements.iter().any(|statement| {
                matches!(
                    statement.kind,
                    StatementKind::SumTag { .. } | StatementKind::SumTake { .. }
                )
            }) {
                header_count += 1;
                assert!(!walk::mir_block_has_custody(block, function, types));
            }
        }
        assert!(header_count > 0);
        assert!(
            validate_resource_ownership(&program, types)
                .unwrap()
                .function(function.id)
                .is_none()
        );
    }
}

#[test]
fn resource_required_scalar_helper_refuses_changed_value_before_canonical_pruning() {
    const HELPER: &str = include_str!(
        "../../../jett_driver/tests/native_conformance/resource/44_comptime_scalar_helper.jett"
    );
    for release in [false, true] {
        let checked = checked(HELPER, release);
        let types = &checked.checked().interner;
        let hir = materialized(&checked);
        let baseline = crate::lower(&hir, types).unwrap();
        let ownership = validate_resource_ownership(&baseline, types).unwrap();
        let helper = named(&baseline, "selector");
        assert!(helper.resource_lowering.is_some());
        assert!(!has_execution_records(helper, types));
        assert!(
            helper
                .resource_lowering
                .as_ref()
                .unwrap()
                .descriptors
                .graph
                .is_some()
        );
        assert!(ownership.function(helper.id).is_none());
        assert!(!ownership.required_only_function_ids().contains(&helper.id));
        assert!(
            hir.resource_source
                .required_materializations()
                .iter()
                .any(|row| {
                    row.function() == helper.id && matches!(row.current().kind, E::Bool(false))
                })
        );
        let mut compacted = baseline.clone();
        assert!(crate::sequences::prune::unreachable(named_mut(
            &mut compacted,
            "selector"
        )));
        let compacted_plan = validate_resource_ownership(&compacted, types).unwrap();
        assert!(compacted_plan.function(helper.id).is_none());
        let original_helper = compacted_plan.original_source(helper.id).unwrap();
        assert!(
            original_helper
                .body
                .statements
                .iter()
                .any(|statement| matches!(
                    &statement.kind, hir::StatementKind::Return(Some(value))
                        if matches!(value.kind, E::Comptime { .. })
                ))
        );
        let mut missing = baseline.clone();
        named_mut(&mut missing, "selector").resource_lowering = None;
        assert!(validate_resource_ownership(&missing, types).is_err());
        let mut changed = baseline.clone();
        let helper = named_mut(&mut changed, "selector");
        let witness_present = helper.resource_lowering.is_some();
        let returned = helper
            .blocks
            .iter_mut()
            .find_map(|block| match &mut block.terminator.kind {
                TerminatorKind::Return(Some(value)) => Some(value),
                _ => None,
            })
            .unwrap();
        assert!(matches!(returned.kind, E::Bool(false)));
        returned.kind = E::Bool(true);
        helper.resource_lowering.as_mut().unwrap().blocks = helper.blocks.clone();
        let validation_accepted = validate_resource_ownership(&changed, types).is_ok();
        let transform_accepted =
            crate::sequences::prune::unreachable(named_mut(&mut changed, "selector"));
        let ownership_accepted = validate_resource_ownership(&changed, types).is_ok();
        assert!(
            !validation_accepted && !transform_accepted && !ownership_accepted,
            "required scalar helper escaped authentication: witness present={witness_present}, validation accepted={validation_accepted}, transform accepted={transform_accepted}, ownership accepted={ownership_accepted}, release={release}"
        );
    }
}

const SOURCE: &str = r#"namespace app
function scenario(view net: Network) returns nothing:
    use resource_probe
    function(resource_probe.TestHandle) returns nothing dispose = comptime resource_probe.descriptor()
    resource_probe.TestHandle token = resource_probe.create(view net, 761) handle error:
        return nothing
    dispose(token)
    return nothing
"#;

#[test]
fn resource_required_descriptor_public_hook_substitution_does_not_mint_materialization() {
    for release in [false, true] {
        let checked = checked(SOURCE, release);
        let types = &checked.checked().interner;
        let mut program = hir::lower_checked_resource_program(&checked).unwrap();
        let archive = program.resource_source.clone();
        let hook = program
            .resource_manifest
            .hooks()
            .find(|hook| hook.recipe() == jett_types::ResourceKernelRecipe::Finalize)
            .unwrap()
            .clone();
        let function = program
            .functions
            .iter_mut()
            .find(|function| function.identity.declaration.name == "scenario")
            .unwrap();
        let value = function
            .body
            .statements
            .iter_mut()
            .find_map(|statement| match &mut statement.kind {
                hir::StatementKind::Let { value, .. }
                    if matches!(value.kind, E::Comptime { .. }) =>
                {
                    Some(value)
                }
                _ => None,
            })
            .unwrap();
        assert_eq!(value.ty, hook.function_type());
        value.kind = E::ResourceHookValue { hook };
        assert_eq!(program.resource_source, archive);
        assert!(archive.functions().iter().any(|function| {
            let mut original_required = false;
            walk::hir_block(&function.body, &mut |value| {
                original_required |= matches!(value.kind, E::Comptime { .. });
            });
            original_required
        }));
        let errors = crate::lower(&program, types).unwrap_err();
        assert!(errors.iter().any(|error| {
            error
                .message
                .contains("Resource lowering differs from its original checked HIR archive")
        }));
    }
}
