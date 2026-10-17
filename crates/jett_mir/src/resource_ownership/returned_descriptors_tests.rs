use super::*;
use crate::resource_ownership::tests::{checked, lowered};

const SOURCE: &str = r#"namespace app
function relay() returns function(resource_probe.TestHandle) returns nothing:
    use resource_probe
    function(resource_probe.TestHandle) returns nothing selected = resource_probe.descriptor()
    function(resource_probe.TestHandle) returns nothing alias = selected
    return alias
function scenario(view net: Network) returns nothing:
    use resource_probe
    function(resource_probe.TestHandle) returns nothing selected = relay()
    function(resource_probe.TestHandle) returns nothing dispose = selected
    resource_probe.TestHandle token = resource_probe.create(view net, 721) handle error:
        return nothing
    dispose(token)
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
        .unwrap()
}
fn scenario_mut(program: &mut Program) -> &mut Function {
    program
        .functions
        .iter_mut()
        .find(|function| {
            function.identity.declaration.namespace == "app"
                && function.identity.declaration.name == "scenario"
        })
        .unwrap()
}
fn indirect(function: &Function) -> &Expression {
    function
        .blocks
        .iter()
        .flat_map(|block| &block.statements)
        .find_map(|statement| match &statement.kind {
            StatementKind::Evaluate(value) if matches!(value.kind, E::IndirectCall { .. }) => {
                Some(value)
            }
            _ => None,
        })
        .unwrap()
}
#[test]
fn resource_returned_hook_preserves_alias_relay_and_indirect_authority() {
    for release in [false, true] {
        let checked = checked(SOURCE, release);
        let types = &checked.checked().interner;
        let program = lowered(&checked);
        let ownership = validate_resource_ownership(&program, types).unwrap();
        let function = scenario(&program);
        let plan = ownership.function(function.id).unwrap();
        let call = indirect(function);
        let operation = plan
            .operations_for_expression(call)
            .find(|operation| operation.indirect_hook_target().is_some())
            .unwrap();
        let hook = operation.indirect_hook_target().unwrap();
        assert_eq!(hook.recipe(), jett_types::ResourceKernelRecipe::Finalize);
        let ResourceOperationRole::InvokeHook {
            source, operands, ..
        } = operation.role()
        else {
            panic!("exact indirect hook operation");
        };
        assert_eq!(
            source.target,
            hir::CallTarget::Indirect {
                signature_type: hook.function_type()
            }
        );
        assert_eq!(source.bridge, hir::CallBridge::Direct);
        assert!(matches!(
            &operands[..],
            [ResourceCallOperand::Owned { parameter: 0, .. }]
        ));
        let E::IndirectCall { callee, .. } = &call.kind else {
            unreachable!();
        };
        assert_eq!(plan.descriptor_value(callee), Some(hook));
        assert!(plan.descriptor_value(&callee.as_ref().clone()).is_none());
        assert!(
            plan.operations_for_expression(&call.clone())
                .next()
                .is_none()
        );
        let companion = ResourceCompanionPlan::analyze(&ownership, function.id).unwrap();
        for local in &function.locals {
            if plan.descriptor_local(local.id).is_some() {
                assert!(
                    !companion
                        .storage()
                        .owned_locals
                        .contains(&(local.id.index() as usize))
                );
                assert!(!plan.owner_slots().iter().any(|slot| matches!(slot.storage(), ResourceSlotStorage::Local { header } if header.id == local.id)));
            }
        }
        for name in ["relay", "descriptor"] {
            let returned = ownership
                .functions()
                .iter()
                .find(|plan| plan.identity().declaration.name == name)
                .unwrap();
            assert_eq!(returned.descriptor_return(), Some(hook));
            ResourceCompanionPlan::analyze(&ownership, returned.function()).unwrap();
        }
    }
}
#[test]
fn resource_returned_hook_supports_immediate_and_unused_metadata() {
    for release in [false, true] {
        for body in [
            "    resource_probe.TestHandle token = resource_probe.create(view net, 721) handle error:\n        return nothing\n    resource_probe.descriptor()(token)\n    return nothing\n",
            "    function(resource_probe.TestHandle) returns nothing unused = resource_probe.descriptor()\n    return nothing\n",
        ] {
            let source = format!(
                "namespace app\nfunction scenario(view net: Network) returns nothing:\n    use resource_probe\n{body}"
            );
            let checked = checked(&source, release);
            let program = lowered(&checked);
            let ownership =
                validate_resource_ownership(&program, &checked.checked().interner).unwrap();
            for plan in ownership.functions() {
                ResourceCompanionPlan::analyze(&ownership, plan.function()).unwrap();
            }
        }
    }
}
#[test]
fn resource_returned_hook_refuses_changed_retained_returns_and_resealed_escapes() {
    for release in [false, true] {
        let checked = checked(SOURCE, release);
        let types = &checked.checked().interner;
        let baseline = lowered(&checked);
        let mut changed = baseline.clone();
        let function = scenario_mut(&mut changed);
        let binding = function
            .resource_lowering
            .as_ref()
            .unwrap()
            .descriptors
            .bindings[0]
            .clone();
        let escape = Expression {
            kind: E::Local(binding.current.id),
            ty: binding.current.ty,
            span: binding.value.current.span,
        };
        function.blocks[function.entry.index() as usize]
            .statements
            .push(Statement {
                span: escape.span,
                kind: StatementKind::Evaluate(escape),
            });
        function.resource_lowering.as_mut().unwrap().blocks = function.blocks.clone();
        assert!(validate_resource_ownership(&changed, types).is_err());
        let mut changed = baseline.clone();
        let function = changed
            .functions
            .iter_mut()
            .find(|function| function.identity.declaration.name == "relay")
            .unwrap();
        let block = function
            .blocks
            .iter_mut()
            .find(|block| matches!(block.terminator.kind, TerminatorKind::Return(Some(_))))
            .unwrap();
        if let TerminatorKind::Return(Some(value)) = &mut block.terminator.kind {
            value.span = function.span;
        }
        function.resource_lowering.as_mut().unwrap().blocks = function.blocks.clone();
        assert!(validate_resource_ownership(&changed, types).is_err());
        let mut changed = baseline.clone();
        let function = scenario_mut(&mut changed);
        function
            .resource_lowering
            .as_mut()
            .unwrap()
            .descriptors
            .returned = baseline
            .functions
            .iter()
            .find(|function| function.identity.declaration.name == "descriptor")
            .unwrap()
            .resource_lowering
            .as_ref()
            .unwrap()
            .descriptors
            .returned
            .clone();
        assert!(validate_resource_ownership(&changed, types).is_err());
    }
}
use crate::resource_ownership::tests::{SUPPORT, checked_support};
const DESCRIPTOR_SUPPORT: &str = r#"export function factory_descriptor() returns function(view Network, int64) returns result[TestHandle, string]:
    return kernel_create
export function borrow_descriptor() returns function(view Network, view TestHandle) returns result[int64, string]:
    return kernel_borrow
export function select_create(view net: Network, view anchor: TestHandle, fail_now: bool) returns function(view Network, int64) returns result[TestHandle, string]:
    result[int64, string] observed = borrow(view net, view anchor)
    if fail_now:
        int64 ignored = terminal_label()
    return kernel_create
"#;
const ALL_RECIPES: &str = r#"namespace app
function scenario(view net: Network) returns nothing:
    use resource_probe
    function(view Network, int64) returns result[resource_probe.TestHandle, string] acquire = resource_probe.factory_descriptor()
    function(view Network, view resource_probe.TestHandle) returns result[int64, string] observe = resource_probe.borrow_descriptor()
    function(resource_probe.TestHandle) returns nothing dispose = resource_probe.descriptor()
    resource_probe.TestHandle token = acquire(view net, 731) handle error:
        return nothing
    result[int64, string] observed = observe(view net, view token)
    dispose(token)
    return nothing
"#;
#[test]
fn resource_returned_hook_proves_every_recipe_and_modes() {
    for release in [false, true] {
        let support = format!("{SUPPORT}{DESCRIPTOR_SUPPORT}");
        let checked = checked_support(ALL_RECIPES, &support, release);
        let program = lowered(&checked);
        let ownership = validate_resource_ownership(&program, &checked.checked().interner).unwrap();
        let plan = ownership.function(scenario(&program).id).unwrap();
        let hooks: Vec<_> = plan
            .operations()
            .iter()
            .filter_map(|operation| {
                operation
                    .indirect_hook_target()
                    .map(|hook| (hook.recipe(), operation))
            })
            .collect();
        assert_eq!(hooks.len(), 3);
        for (recipe, operation) in hooks {
            let ResourceOperationRole::InvokeHook {
                operands, result, ..
            } = operation.role()
            else {
                panic!("exact hook operation");
            };
            match recipe {
                jett_types::ResourceKernelRecipe::NetworkFactory => {
                    assert!(matches!(result, ResourceCallResult::Owned { .. }));
                    assert!(
                        operands
                            .iter()
                            .all(|operand| matches!(operand, ResourceCallOperand::Ordinary { .. }))
                    );
                }
                jett_types::ResourceKernelRecipe::NetworkBorrow => {
                    assert!(matches!(result, ResourceCallResult::Ordinary { .. }));
                    assert!(operands.iter().any(|operand| matches!(
                        operand,
                        ResourceCallOperand::Borrowed { parameter: 1, .. }
                    )));
                }
                jett_types::ResourceKernelRecipe::Finalize => {
                    assert!(matches!(
                        &operands[..],
                        [ResourceCallOperand::Owned { parameter: 0, .. }]
                    ));
                }
            }
        }
        for plan in ownership.functions() {
            ResourceCompanionPlan::analyze(&ownership, plan.function()).unwrap();
        }
    }
}
#[test]
fn resource_returned_hook_authenticates_callee_effects_after_actuals() {
    for release in [false, true] {
        let support = format!("{SUPPORT}{DESCRIPTOR_SUPPORT}");
        let source = r#"namespace app
function scenario(view net: Network) returns nothing:
    use resource_probe
    resource_probe.TestHandle anchor = resource_probe.create(view net, 732) handle error:
        return nothing
    resource_probe.TestHandle token = resource_probe.select_create(view net, view anchor, false)(view net, 733) handle error:
        return nothing
    resource_probe.close(token)
    resource_probe.close(anchor)
    return nothing
"#;
        let checked = checked_support(source, &support, release);
        let types = &checked.checked().interner;
        let baseline = lowered(&checked);
        let ownership = validate_resource_ownership(&baseline, types).unwrap();
        for plan in ownership.functions() {
            ResourceCompanionPlan::analyze(&ownership, plan.function()).unwrap();
        }
        let selected = baseline
            .functions
            .iter()
            .find(|function| function.identity.declaration.name == "select_create")
            .unwrap();
        assert_eq!(
            ownership
                .function(selected.id)
                .unwrap()
                .descriptor_return()
                .unwrap()
                .recipe(),
            jett_types::ResourceKernelRecipe::NetworkFactory
        );
        let mut changed = baseline.clone();
        let selected = changed
            .functions
            .iter_mut()
            .find(|function| function.identity.declaration.name == "select_create")
            .unwrap();
        let block = selected
            .blocks
            .iter_mut()
            .find(|block| matches!(block.terminator.kind, TerminatorKind::Branch { .. }))
            .unwrap();
        if let TerminatorKind::Branch { condition, .. } = &mut block.terminator.kind {
            condition.kind = E::Bool(true);
        }
        selected.resource_lowering.as_mut().unwrap().blocks = selected.blocks.clone();
        assert!(validate_resource_ownership(&changed, types).is_err());
    }
}
#[test]
fn resource_returned_hook_shared_all_recipe_and_evaluation_order_sources_validate() {
    const SUPPORT: &str =
        include_str!("../../../jett_driver/tests/native_conformance/resource/resource_probe.jett");
    const SOURCES: [&str; 6] = [
        include_str!(
            "../../../jett_driver/tests/native_conformance/resource/27_returned_hook_factory.jett"
        ),
        include_str!(
            "../../../jett_driver/tests/native_conformance/resource/28_returned_hook_borrow.jett"
        ),
        include_str!(
            "../../../jett_driver/tests/native_conformance/resource/29_immediate_returned_hook_close.jett"
        ),
        include_str!(
            "../../../jett_driver/tests/native_conformance/resource/30_immediate_returned_hook_factory.jett"
        ),
        include_str!(
            "../../../jett_driver/tests/native_conformance/resource/31_immediate_returned_hook_callee_failure.jett"
        ),
        include_str!(
            "../../../jett_driver/tests/native_conformance/resource/32_immediate_returned_hook_actual_failure.jett"
        ),
    ];
    for release in [false, true] {
        for (index, source) in SOURCES.iter().enumerate() {
            let checked = checked_support(source, SUPPORT, release);
            let program = lowered(&checked);
            let ownership = validate_resource_ownership(&program, &checked.checked().interner)
                .unwrap_or_else(|errors| {
                    panic!("shared Source {} release {release}: {errors:?}", index + 27)
                });
            let entry = program
                .functions
                .iter()
                .find(|function| {
                    function.identity.declaration.namespace == "app"
                        && function.identity.declaration.name == "main"
                })
                .unwrap();
            assert!(
                ownership
                    .function(entry.id)
                    .unwrap()
                    .operations()
                    .iter()
                    .any(|operation| operation.indirect_hook_target().is_some())
            );
            for plan in ownership.functions() {
                ResourceCompanionPlan::analyze(&ownership, plan.function()).unwrap();
            }
        }
    }
}
#[test]
fn resource_returned_hook_refuses_resealed_body_before_canonical_pruning() {
    const SUPPORT: &str =
        include_str!("../../../jett_driver/tests/native_conformance/resource/resource_probe.jett");
    const SOURCE: &str = include_str!(
        "../../../jett_driver/tests/native_conformance/resource/30_immediate_returned_hook_factory.jett"
    );
    for release in [false, true] {
        let checked = checked_support(SOURCE, SUPPORT, release);
        let types = &checked.checked().interner;
        let mut changed = lowered(&checked);
        validate_resource_ownership(&changed, types).unwrap();
        let selected = changed
            .functions
            .iter_mut()
            .find(|function| function.identity.declaration.name == "select_create")
            .unwrap();
        let branch = selected
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
        selected.resource_lowering.as_mut().unwrap().blocks = selected.blocks.clone();
        assert!(
            validate_resource_ownership(&changed, types).is_err(),
            "untransformed copied body must be refused"
        );
        let selected = changed
            .functions
            .iter_mut()
            .find(|function| function.identity.declaration.name == "select_create")
            .unwrap();
        let transform_accepted = crate::sequences::prune::unreachable(selected);
        let ownership_accepted = validate_resource_ownership(&changed, types).is_ok();
        assert!(
            !transform_accepted && !ownership_accepted,
            "canonical prune consumed a copied/resealed descriptor body: transform accepted={transform_accepted}, final ownership accepted={ownership_accepted}, release={release}"
        );
    }
}
#[test]
fn resource_returned_hook_refuses_resealed_headers_before_canonical_local_pruning() {
    const SUPPORT: &str =
        include_str!("../../../jett_driver/tests/native_conformance/resource/resource_probe.jett");
    const SOURCE: &str = include_str!(
        "../../../jett_driver/tests/native_conformance/resource/30_immediate_returned_hook_factory.jett"
    );
    for release in [false, true] {
        let checked = checked_support(SOURCE, SUPPORT, release);
        let types = &checked.checked().interner;
        let mut changed = lowered(&checked);
        let selected = changed
            .functions
            .iter_mut()
            .find(|function| function.identity.declaration.name == "select_create")
            .unwrap();
        let mut unproved = selected.locals[0].clone();
        unproved.id = LocalId::new(u32::try_from(selected.locals.len()).unwrap());
        unproved.name = "unproved_unused_header".into();
        unproved.ty = TypeInterner::BOOL;
        unproved.debug_ty = TypeInterner::BOOL;
        unproved.debug_type_name = None;
        unproved.view_source = None;
        unproved.mutable = false;
        selected.locals.push(unproved);
        selected.resource_lowering.as_mut().unwrap().locals = selected.locals.clone();
        assert!(validate_resource_ownership(&changed, types).is_err());
        let selected = changed
            .functions
            .iter_mut()
            .find(|function| function.identity.declaration.name == "select_create")
            .unwrap();
        let transform_accepted = crate::sequences::prune::unused_locals(selected);
        let ownership_accepted = validate_resource_ownership(&changed, types).is_ok();
        assert!(
            !transform_accepted && !ownership_accepted,
            "canonical local prune consumed copied/resealed descriptor headers: transform accepted={transform_accepted}, final ownership accepted={ownership_accepted}, release={release}"
        );
    }
}
#[test]
fn resource_returned_hook_refuses_unmapped_retained_descriptor_targets() {
    const SUPPORT: &str =
        include_str!("../../../jett_driver/tests/native_conformance/resource/resource_probe.jett");
    const SOURCE: &str = include_str!(
        "../../../jett_driver/tests/native_conformance/resource/30_immediate_returned_hook_factory.jett"
    );
    for release in [false, true] {
        let checked = checked_support(SOURCE, SUPPORT, release);
        let program = lowered(&checked);
        let selected = program
            .functions
            .iter()
            .find(|function| function.identity.declaration.name == "select_create")
            .unwrap();
        let mut proof = selected
            .resource_lowering
            .as_ref()
            .unwrap()
            .descriptors
            .clone();
        let branch = selected
            .blocks
            .iter()
            .find(|block| matches!(block.terminator.kind, TerminatorKind::Branch { .. }))
            .unwrap();
        let TerminatorKind::Branch { then_block, .. } = branch.terminator.kind else {
            unreachable!();
        };
        let mut map: Vec<_> = selected.blocks.iter().map(|block| Some(block.id)).collect();
        map[then_block.index() as usize] = None;
        let before = proof.clone();
        assert!(remap_blocks(&mut proof, &map).is_err());
        assert_eq!(
            proof, before,
            "rejected mapping must preserve the retained evidence"
        );
    }
}
