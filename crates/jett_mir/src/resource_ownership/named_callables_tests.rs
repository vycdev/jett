use super::*;
use crate::resource_ownership::tests::{checked, lowered};

const SOURCE: &str = r#"namespace app
function wrong_target(token: resource_probe.TestHandle) returns nothing:
    return nothing
function close_selected(token: resource_probe.TestHandle) returns nothing:
    use resource_probe
    resource_probe.close(token)
    return nothing
function scenario(view net: Network) returns nothing:
    use resource_probe
    function(resource_probe.TestHandle) returns nothing dispose = close_selected
    resource_probe.TestHandle token = resource_probe.create(view net, 701) handle error:
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
fn producer(function: &Function) -> &Expression {
    let signature = function.resource_lowering.as_ref().unwrap().named_callables[0]
        .proof
        .signature_type();
    function
        .blocks
        .iter()
        .flat_map(|block| &block.statements)
        .find_map(|statement| match &statement.kind {
            StatementKind::Let { value, .. }
                if value.ty == signature && matches!(value.kind, E::FunctionRef(_)) =>
            {
                Some(value)
            }
            _ => None,
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
fn initializer_mut(function: &mut Function) -> &mut Expression {
    function
        .blocks
        .iter_mut()
        .flat_map(|block| &mut block.statements)
        .find_map(|statement| match &mut statement.kind {
            StatementKind::Let { value, .. } if matches!(value.kind, E::FunctionRef(_)) => {
                Some(value)
            }
            _ => None,
        })
        .unwrap()
}

#[test]
fn resource_named_indirect_keeps_original_source_and_exact_producer_use() {
    for release in [false, true] {
        for selected in ["close_selected", "wrong_target"] {
            let source =
                SOURCE.replace("dispose = close_selected", &format!("dispose = {selected}"));
            let original = checked(&source, release);
            let types = &original.checked().interner;
            let program = lowered(&original);
            let function = scenario(&program);
            let ownership = validate_resource_ownership(&program, types).unwrap();
            let plan = ownership.function(function.id).unwrap();
            let descriptor = producer(function);
            let proof = plan.named_callable_producer(descriptor).unwrap();
            assert_eq!(proof.identity().declaration.name, selected);
            assert_eq!(
                proof.function(),
                match descriptor.kind {
                    E::FunctionRef(id) => id,
                    _ => unreachable!(),
                }
            );
            assert_eq!(proof.signature_type(), descriptor.ty);
            assert!(plan.named_callable_producer(&descriptor.clone()).is_none());
            assert_eq!(plan.named_callable_value(descriptor), Some(proof));
            assert!(plan.named_callable_value(&descriptor.clone()).is_none());
            let call = indirect(function);
            let operation = plan
                .operations_for_expression(call)
                .find(|operation| operation.named_indirect_target().is_some())
                .unwrap();
            assert_eq!(operation.named_indirect_target(), Some(proof));
            let ResourceOperationRole::InvokeSourceFunction {
                function: target,
                source,
                evaluation_order,
                ..
            } = operation.role()
            else {
                panic!("selected Source operation");
            };
            assert_eq!(*target, proof.function());
            assert_eq!(
                source.target,
                hir::CallTarget::Indirect {
                    signature_type: proof.signature_type()
                }
            );
            assert_eq!(source.bridge, hir::CallBridge::Direct);
            assert_eq!(evaluation_order, &[0]);
            assert!(
                plan.operations_for_expression(&call.clone())
                    .next()
                    .is_none()
            );
            let E::IndirectCall { callee, .. } = &call.kind else {
                unreachable!()
            };
            assert_eq!(plan.named_callable_value(callee), Some(proof));
            assert!(
                plan.named_callable_value(&callee.as_ref().clone())
                    .is_none()
            );
            let companion = ResourceCompanionPlan::analyze(&ownership, function.id).unwrap();
            assert!(companion.storage().temporary_slots >= 2);
            for binding in &function.resource_lowering.as_ref().unwrap().named_callables {
                assert!(
                    companion
                        .storage()
                        .owned_locals
                        .contains(&(binding.local().index() as usize))
                );
            }
        }
    }
}

#[test]
fn resource_named_indirect_retains_immutable_alias_chain_and_canonical_local_maps() {
    let source = SOURCE.replace(
        "    function(resource_probe.TestHandle) returns nothing dispose = close_selected",
        "    function(int64) returns int64 unused_callback = function(ignored: int64) returns int64: return ignored\n    function(resource_probe.TestHandle) returns nothing selected = close_selected\n    function(resource_probe.TestHandle) returns nothing dispose = selected",
    );
    for release in [false, true] {
        let original = checked(&source, release);
        let types = &original.checked().interner;
        let mut program = lowered(&original);
        let before = scenario(&program)
            .resource_lowering
            .as_ref()
            .unwrap()
            .named_callables
            .clone();
        assert_eq!(before.len(), 2);
        crate::prepare_native_generated_functions(&mut program, types);
        let function = scenario(&program);
        let after = &function.resource_lowering.as_ref().unwrap().named_callables;
        assert_eq!(after[0].original_header, before[0].original_header);
        assert_eq!(after[1].original_header, before[1].original_header);
        assert!(after[0].current_header.id.index() < before[0].current_header.id.index());
        assert!(after[1].current_header.id.index() < before[1].current_header.id.index());
        let ownership = validate_resource_ownership(&program, types).unwrap();
        let plan = ownership.function(function.id).unwrap();
        let proof = plan.named_callable_producer(producer(function)).unwrap();
        assert_eq!(proof.identity().declaration.name, "close_selected");
        assert_eq!(
            plan.operations_for_expression(indirect(function))
                .filter(|operation| operation.named_indirect_target() == Some(proof))
                .count(),
            1
        );
        let companion = ResourceCompanionPlan::analyze(&ownership, function.id).unwrap();
        for binding in after {
            assert!(
                companion
                    .storage()
                    .owned_locals
                    .contains(&(binding.local().index() as usize))
            );
        }
    }
}

#[test]
fn resource_named_indirect_rejects_target_initializer_binding_and_reseal_swaps() {
    for release in [false, true] {
        let original = checked(SOURCE, release);
        let types = &original.checked().interner;
        let program = lowered(&original);
        let wrong = program
            .functions
            .iter()
            .find(|function| function.identity.declaration.name == "wrong_target")
            .unwrap()
            .id;
        for mutation in 0..7 {
            let mut changed = program.clone();
            let function = scenario_mut(&mut changed);
            match mutation {
                0 => initializer_mut(function).kind = E::FunctionRef(wrong),
                1 => {
                    function.resource_lowering.as_mut().unwrap().named_callables[0]
                        .proof
                        .function = wrong
                }
                2 => {
                    function.resource_lowering.as_mut().unwrap().named_callables[0]
                        .current_header
                        .mutable = true
                }
                3 => {
                    function.resource_lowering.as_mut().unwrap().named_callables[0]
                        .current_header
                        .id = function.params[0].local
                }
                4 | 5 => {
                    initializer_mut(function).kind = E::FunctionRef(wrong);
                    let blocks = function.blocks.clone();
                    let changed_initializer = producer(function).clone();
                    let witness = function.resource_lowering.as_mut().unwrap();
                    witness.blocks = blocks;
                    if mutation == 5 {
                        witness.named_callables[0].current_value = changed_initializer;
                    }
                }
                6 => {
                    let binding = function.resource_lowering.as_ref().unwrap().named_callables[0]
                        .current_header
                        .clone();
                    function.blocks[0].statements.push(Statement {
                        kind: StatementKind::Evaluate(Expression {
                            kind: E::Local(binding.id),
                            ty: binding.ty,
                            span: binding.span,
                        }),
                        span: binding.span,
                    });
                    let blocks = function.blocks.clone();
                    function.resource_lowering.as_mut().unwrap().blocks = blocks;
                }
                _ => unreachable!(),
            }
            assert!(
                validate_resource_ownership(&changed, types).is_err(),
                "mutation {mutation}"
            );
        }
        let mut changed_hir = hir::lower_checked_resource_program(&original).unwrap();
        let caller = changed_hir
            .functions
            .iter_mut()
            .find(|function| function.identity.declaration.name == "scenario")
            .unwrap();
        let initializer = caller
            .body
            .statements
            .iter_mut()
            .find_map(|statement| match &mut statement.kind {
                hir::StatementKind::Let { value, .. }
                    if matches!(value.kind, E::FunctionRef(_)) =>
                {
                    Some(value)
                }
                _ => None,
            })
            .unwrap();
        initializer.kind = E::FunctionRef(wrong);
        assert!(lower(&changed_hir, types).is_err());
    }
}

#[test]
fn resource_named_indirect_does_not_mint_for_mutable_or_escaped_original_binding() {
    for release in [false, true] {
        let original = checked(SOURCE, release);
        let types = &original.checked().interner;
        let mut hir = hir::lower_checked_resource_program(&original).unwrap();
        let execution = authenticate_original(&hir, types).unwrap();
        let caller = hir
            .functions
            .iter_mut()
            .find(|function| function.identity.declaration.name == "scenario")
            .unwrap();
        let binding = caller
            .locals
            .iter_mut()
            .find(|local| local.name == "dispose")
            .unwrap();
        binding.mutable = true;
        assert!(capture(caller, &hir.resource_source, types, &execution).is_empty());
        let mut hir = hir::lower_checked_resource_program(&original).unwrap();
        let caller = hir
            .functions
            .iter_mut()
            .find(|function| function.identity.declaration.name == "scenario")
            .unwrap();
        let binding = caller
            .locals
            .iter()
            .find(|local| local.name == "dispose")
            .unwrap()
            .clone();
        caller.body.statements.push(hir::Statement {
            kind: hir::StatementKind::Return(Some(Expression {
                kind: E::Local(binding.id),
                ty: binding.ty,
                span: binding.span,
            })),
            span: binding.span,
        });
        assert!(capture(caller, &hir.resource_source, types, &execution).is_empty());
    }
}
