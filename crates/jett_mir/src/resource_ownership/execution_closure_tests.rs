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
fn resource_free_nested_call_to_custody_family_keeps_exact_source_abi_region() {
    const SOURCE: &str = "namespace app\nfunction label(view net: Network) returns int64:\n    use resource_probe\n    resource_probe.TestHandle token = resource_probe.create(view net, 1) handle error:\n        return 0\n    resource_probe.close(token)\n    return 1\nfunction main(net: Network) returns nothing:\n    string text = \"{label(view net)}\"\n    return nothing\n";
    for release in [false, true] {
        let checked = checked(SOURCE, release);
        let types = &checked.checked().interner;
        let program = lowered(&checked);
        let plan = validate_resource_ownership(&program, types).unwrap();
        let main = named(&program, "main");
        let label = named(&program, "label");
        let regions = main
            .blocks
            .iter()
            .flat_map(|block| &block.statements)
            .filter_map(|statement| match &statement.kind {
                StatementKind::ResourceCall(ResourceCallNode::Invoke { region, .. }) => {
                    Some(main.resource_call_region(*region).unwrap())
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(regions.len(), 1);
        let region = regions[0];
        assert_eq!(region.function(), label.id);
        assert_eq!(region.parent(), None);
        assert_eq!(region.evaluation_order(), &[0]);
        assert_eq!(region.actuals().len(), 1);
        assert_eq!(region.actuals()[0].source_index(), 0);
        assert_eq!(region.actuals()[0].parameter(), 0);
        assert!(region.actuals()[0].ordinary().is_some());
        assert_eq!(
            main.local(region.output().unwrap()).unwrap().ty,
            TypeInterner::INT64
        );
        assert!(matches!(&region.original().kind,
            hir::ExpressionKind::Call { function, ownership: hir::CallOwnership::Source(_), .. }
            if *function == label.id));
        let calls = plan
            .function(main.id)
            .unwrap()
            .operations()
            .iter()
            .filter_map(|operation| match operation.role() {
                ResourceOperationRole::InvokeSourceFunction {
                    function, formals, ..
                } => Some((*function, formals)),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].0, label.id);
        assert_eq!(calls[0].1.len(), 1);
        assert_eq!(calls[0].1[0].parameter_type(), TypeInterner::NETWORK);
        assert_eq!(calls[0].1[0].access(), ParamMode::View);
        assert_eq!(calls[0].1[0].effect(), ResourceArgumentEffect::RetainBorrow);
    }
}

const ORDINARY_ENDPOINT_ORDER: &str = r#"namespace app
function absent() returns list[optional[resource_probe.TestHandle]]:
    return list(none, none)
function one() returns int64:
    list[optional[resource_probe.TestHandle]] values = absent()
    return 1
function two() returns int64:
    list[optional[resource_probe.TestHandle]] values = absent()
    return 2
function combine(first: int64, second: int64) returns int64:
    list[optional[resource_probe.TestHandle]] values = absent()
    return first + second
function ordered() returns bool:
    return one() < two()
function named_ordered() returns int64:
    return combine(second: one(), first: two())
function logical_and(flag: bool) returns bool:
    return flag and one() == 1
function logical_or(flag: bool) returns bool:
    return flag or two() == 2
function main() returns nothing:
    bool comparison = ordered()
    int64 named = named_ordered()
    bool conjunction = logical_and(false)
    bool disjunction = logical_or(true)
    return nothing
"#;

fn invoked_regions(function: &Function) -> Vec<&ResourceCallRegion> {
    function
        .blocks
        .iter()
        .flat_map(|block| &block.statements)
        .filter_map(|statement| match &statement.kind {
            StatementKind::ResourceCall(ResourceCallNode::Invoke { region, .. }) => {
                Some(function.resource_call_region(*region).unwrap())
            }
            _ => None,
        })
        .collect()
}

#[test]
fn ordinary_resource_family_endpoints_preserve_comparison_and_named_actual_order() {
    for release in [false, true] {
        let checked = checked(ORDINARY_ENDPOINT_ORDER, release);
        let types = &checked.checked().interner;
        let program = lowered(&checked);
        validate_resource_ownership(&program, types).unwrap();
        let one = named(&program, "one").id;
        let two = named(&program, "two").id;
        let combine = named(&program, "combine").id;
        let ordered = named(&program, "ordered");
        let calls = invoked_regions(ordered);
        assert_eq!(
            calls.iter().map(|call| call.function()).collect::<Vec<_>>(),
            [one, two]
        );
        for call in &calls {
            assert_eq!(call.parent(), None);
            assert!(call.actuals().is_empty());
            assert_eq!(
                ordered.local(call.output().unwrap()).unwrap().ty,
                TypeInterner::INT64
            );
            assert!(matches!(
                call.original().kind,
                hir::ExpressionKind::Call {
                    ownership: hir::CallOwnership::Source(_),
                    ..
                }
            ));
        }
        let named_call = named(&program, "named_ordered");
        let calls = invoked_regions(named_call);
        assert_eq!(
            calls.iter().map(|call| call.function()).collect::<Vec<_>>(),
            [one, two, combine]
        );
        let outer = calls[2];
        assert_eq!(outer.parent(), None);
        assert_eq!(outer.evaluation_order(), [1, 0]);
        assert_eq!(
            outer
                .actuals()
                .iter()
                .map(|actual| (actual.source_index(), actual.parameter()))
                .collect::<Vec<_>>(),
            [(0, 1), (1, 0)]
        );
        for (child, actual) in calls[..2].iter().zip(outer.actuals()) {
            assert_eq!(child.parent(), Some(outer.id()));
            assert!(actual.ordinary().is_some());
            assert!(
                matches!(actual.endpoint().kind, hir::ExpressionKind::Local(local) if Some(local) == child.output())
            );
        }
        let outer_id = outer.id();
        let owner = named_call.id;
        let mut corrupted = program.clone();
        let function = &mut corrupted.functions[owner.index() as usize];
        let stage = function
            .blocks
            .iter_mut()
            .flat_map(|block| &mut block.statements)
            .find_map(|statement| match &mut statement.kind {
                StatementKind::ResourceCall(ResourceCallNode::Stage {
                    region,
                    source_index: 0,
                    parameter,
                    ..
                }) if *region == outer_id => Some(parameter),
                _ => None,
            })
            .unwrap();
        *stage = 0;
        function.resource_lowering.as_mut().unwrap().blocks = function.blocks.clone();
        assert!(validate_resource_ownership(&corrupted, types).is_err());
    }
}

#[test]
fn ordinary_resource_family_rhs_stays_on_the_selected_short_circuit_branch() {
    for release in [false, true] {
        let checked = checked(ORDINARY_ENDPOINT_ORDER, release);
        let types = &checked.checked().interner;
        let program = lowered(&checked);
        validate_resource_ownership(&program, types).unwrap();
        for (name, target, and) in [("logical_and", "one", true), ("logical_or", "two", false)] {
            let function = named(&program, name);
            let calls = invoked_regions(function);
            assert_eq!(calls.len(), 1);
            assert_eq!(calls[0].function(), named(&program, target).id);
            let TerminatorKind::Branch {
                condition,
                then_block,
                else_block,
            } = &function.blocks[0].terminator.kind
            else {
                panic!("checked flag must select the RHS or bypass");
            };
            assert!(matches!(condition.kind, hir::ExpressionKind::Local(_)));
            let (selected, bypass) = if and {
                (*then_block, *else_block)
            } else {
                (*else_block, *then_block)
            };
            assert_eq!(calls[0].begin().block, selected);
            assert_eq!(calls[0].invocation().unwrap().0.block, selected);
            let bypass = &function.blocks[bypass.index() as usize];
            assert!(
                bypass
                    .statements
                    .iter()
                    .all(|statement| !matches!(statement.kind, StatementKind::ResourceCall(_)))
            );
            assert!(bypass.statements.iter().any(|statement| matches!(&statement.kind,
                StatementKind::Let { value: Expression { kind: hir::ExpressionKind::Bool(value), .. }, .. } if *value == !and)));
            let TerminatorKind::Goto(right_join) =
                function.blocks[selected.index() as usize].terminator.kind
            else {
                panic!("RHS must join after its invocation");
            };
            let TerminatorKind::Goto(bypass_join) = bypass.terminator.kind else {
                panic!("bypass must join without invoking RHS");
            };
            assert_eq!(right_join, bypass_join);
        }
    }
}
