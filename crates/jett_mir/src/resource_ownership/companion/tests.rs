use super::super::tests::{checked, lowered};
use super::*;
const SOURCE: &str = include_str!("../fixtures/connected_entry.jett");
#[test]
fn resource_companions_keep_ordinary_failure_storage_and_public_custody_refusals() {
    for release in [false, true] {
        let checked = checked(SOURCE, release);
        let program = lowered(&checked);
        let types = &checked.checked().interner;
        let ownership = validate_resource_ownership(&program, types).unwrap();
        for function_plan in ownership.functions() {
            let current = &program.functions[function_plan.function().index() as usize];
            let companion = ResourceCompanionPlan::analyze(&ownership, current.id).unwrap();
            assert!(std::ptr::eq(companion.ownership(), &ownership));
            assert_eq!(companion.function(), current.id);
            for local in &current.locals {
                if resource_type_pending(types, local.ty) {
                    assert!(
                        !companion
                            .storage()
                            .owned_locals
                            .contains(&(local.id.index() as usize))
                    );
                }
            }
            if current
                .locals
                .iter()
                .any(|local| custody_type(types, local.ty))
            {
                assert!(
                    crate::move_values::MoveValuePlan::analyze(&program, current, types)
                        .unwrap_err()
                        .contains("pending ResourceOwnershipPlan")
                );
                assert!(
                    CopyValuePlan::analyze(current, types)
                        .unwrap_err()
                        .contains("pending ResourceOwnershipPlan")
                );
            }
            for statement in current.blocks.iter().flat_map(|block| &block.statements) {
                if let StatementKind::SumTake {
                    target,
                    success: false,
                    ..
                } = statement.kind
                {
                    assert_eq!(current.local(target).unwrap().ty, TypeInterner::STRING);
                    assert!(
                        companion
                            .storage()
                            .owned_locals
                            .contains(&(target.index() as usize))
                    );
                }
            }
        }
        let lifecycle = program
            .functions
            .iter()
            .find(|function| {
                function.identity.declaration.namespace == "app"
                    && function.identity.declaration.name == "lifecycle"
            })
            .unwrap();
        let ordinary_result = lifecycle
            .locals
            .iter()
            .find(|local| local.name == "observed")
            .unwrap();
        assert_eq!(
            types.resolve(ordinary_result.ty),
            &Type::Result(TypeInterner::INT64, TypeInterner::STRING)
        );
        let companion = ResourceCompanionPlan::analyze(&ownership, lifecycle.id).unwrap();
        assert!(
            companion
                .storage()
                .owned_locals
                .contains(&(ordinary_result.id.index() as usize))
        );
        assert!(companion.storage().temporary_slots > 0);
    }
}
#[test]
fn resource_role_projection_requires_the_exact_current_expression_occurrence() {
    for release in [false, true] {
        let checked = checked(SOURCE, release);
        let program = lowered(&checked);
        let ownership = validate_resource_ownership(&program, &checked.checked().interner).unwrap();
        let mut found = 0;
        for function_plan in ownership.functions() {
            let current = &program.functions[function_plan.function().index() as usize];
            let context = CompanionContext::new(&ownership, current.id).unwrap();
            for statement in current.blocks.iter().flat_map(|block| &block.statements) {
                let value = match &statement.kind {
                    StatementKind::Let { value, .. } | StatementKind::Evaluate(value) => value,
                    _ => continue,
                };
                walk::expression(value, &mut |value| {
                    assert!(context.contains(value));
                    let copied = value.clone();
                    assert!(!context.contains(&copied));
                    assert_eq!(function_plan.operations_for_expression(&copied).count(), 0);
                    for operation in function_plan.operations_for_expression(value) {
                        assert_eq!(operation.site().function(), current.id);
                        found += usize::from(matches!(
                            operation.role(),
                            ResourceOperationRole::InvokeHook { .. }
                                | ResourceOperationRole::InvokeSourceFunction { .. }
                        ));
                    }
                });
            }
        }
        assert!(found >= 3);
    }
}
#[test]
fn resource_companion_function_selection_cannot_adopt_an_unrelated_ordinary_body() {
    for release in [false, true] {
        let checked = checked(SOURCE, release);
        let program = lowered(&checked);
        let ownership = validate_resource_ownership(&program, &checked.checked().interner).unwrap();
        let outside = program
            .functions
            .iter()
            .find(|function| ownership.function(function.id).is_none())
            .expect("unused ordinary support function");
        assert!(
            ResourceCompanionPlan::analyze(&ownership, outside.id)
                .unwrap_err()
                .contains("exact fresh function plan")
        );
    }
}
