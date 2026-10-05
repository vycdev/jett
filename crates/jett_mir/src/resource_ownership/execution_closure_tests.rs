use super::tests::{checked, lowered};
use super::*;
const SOURCE: &str = include_str!("fixtures/connected_entry.jett");
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
#[test]
fn resource_original_execution_closure_captures_resource_free_source_predecessors() {
    for release in [false, true] {
        let checked = checked(SOURCE, release);
        let types = &checked.checked().interner;
        let program = lowered(&checked);
        let plan = validate_resource_ownership(&program, types).unwrap();
        let entry = named(&program, "main");
        assert_eq!(entry.return_type, TypeInterner::NOTHING);
        assert!(
            entry
                .locals
                .iter()
                .all(|local| !custody_type(types, local.ty))
        );
        let entry_plan = plan
            .function(entry.id)
            .expect("exact original predecessor Scope");
        assert_eq!(entry_plan.root_scope().role(), ResourceFrameRole::Scope);
        assert!(entry_plan.provisional_return().is_none());
        let operations = entry_plan
            .operations()
            .iter()
            .filter_map(|operation| {
                if let ResourceOperationRole::InvokeSourceFunction {
                    function, formals, ..
                } = operation.role()
                {
                    Some((*function, formals))
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();
        assert_eq!(operations.len(), 1);
        assert_eq!(operations[0].0, named(&program, "lifecycle").id);
        assert_eq!(operations[0].1.len(), 1);
        assert_eq!(operations[0].1[0].parameter_type(), TypeInterner::NETWORK);
        assert_eq!(operations[0].1[0].access(), ParamMode::View);
    }
}
#[test]
fn resource_execution_closure_cannot_be_removed_or_recaptured_from_edited_public_graphs() {
    for release in [false, true] {
        let checked = checked(SOURCE, release);
        let types = &checked.checked().interner;
        let program = lowered(&checked);
        let entry_id = named(&program, "main").id;
        let mut changed = program.clone();
        changed.functions[entry_id.index() as usize].resource_lowering = None;
        assert!(
            validate_resource_ownership(&changed, types)
                .unwrap_err()
                .iter()
                .any(|error| error
                    .message
                    .contains("initially authenticated Source constructor witness"))
        );
        let mut hir = hir::lower_checked_resource_program(&checked).unwrap();
        let index = hir
            .functions
            .iter()
            .position(|function| function.id == entry_id)
            .unwrap();
        let original = hir.functions[index].body.clone();
        hir.functions[index].body.statements.clear();
        assert!(
            lower(&hir, types)
                .unwrap_err()
                .iter()
                .any(|error| error.message.contains("original checked HIR archive"))
        );
        hir.functions[index].body = original;
        let closure = authenticate_original(&hir, types).unwrap();
        assert!(closure.contains(entry_id));
    }
}
#[test]
fn resource_sum_shell_constructors_and_failure_extraction_have_exact_distinct_plan_roles() {
    const ABSENT: &str = "namespace app\nfunction discard(value: optional[resource_probe.TestHandle]) returns nothing:\n    return nothing\nfunction main(net: Network) returns nothing:\n    use resource_probe\n    discard(resource_probe.empty())\n    resource_probe.TestHandle token = resource_probe.create(view net, 3) handle error:\n        return nothing\n    resource_probe.close(token)\n    return nothing\n";
    for release in [false, true] {
        let checked = checked(ABSENT, release);
        let types = &checked.checked().interner;
        let program = lowered(&checked);
        let plan = validate_resource_ownership(&program, types).unwrap();
        assert!(
            plan.functions()
                .iter()
                .flat_map(|function| function.operations())
                .any(|operation| matches!(
                    operation.role(),
                    ResourceOperationRole::CreateAbsentSum { .. }
                ))
        );
        assert!(plan.functions().iter().flat_map(|function| function.operations()).any(|operation| matches!(operation.role(), ResourceOperationRole::TakeFailureCompanion { failure, target, .. } if *failure == TypeInterner::STRING && target.ty == *failure)));
        for function in plan.functions() {
            for operation in function.operations() {
                if let ResourceOperationRole::CreateAbsentSum { destination } = operation.role() {
                    assert!(matches!(
                        function.owner_slots()[destination.index()].shape(),
                        ResourceShape::Optional { .. }
                    ));
                }
            }
        }
    }
}

#[test]
fn resource_free_nested_call_to_custody_family_cannot_silently_use_the_ordinary_abi() {
    const SOURCE: &str = "namespace app\nfunction label(view net: Network) returns int64:\n    use resource_probe\n    resource_probe.TestHandle token = resource_probe.create(view net, 1) handle error:\n        return 0\n    resource_probe.close(token)\n    return 1\nfunction main(net: Network) returns nothing:\n    string text = \"{label(view net)}\"\n    return nothing\n";
    for release in [false, true] {
        let checked = checked(SOURCE, release);
        let program = lowered(&checked);
        let errors =
            validate_resource_ownership(&program, &checked.checked().interner).unwrap_err();
        assert!(errors.iter().any(|error| {
            error.message.contains(
                "nested Resource evaluation requires exact source-order canonical staging",
            )
        }));
    }
}
