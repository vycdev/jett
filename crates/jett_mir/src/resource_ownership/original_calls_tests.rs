use super::tests::{checked, lowered};
use super::*;
use jett_typecheck::CheckedCallerEffect;

const SOURCE: &str = r#"namespace app
function read_once(view token: resource_probe.TestHandle, view net: Network) returns result[int64, string]:
    use resource_probe
    return resource_probe.borrow(view net, view token)
function scenario(view net: Network) returns nothing:
    use resource_probe
    resource_probe.TestHandle token = resource_probe.create(view net, 2) handle error:
        return nothing
    result[int64, string] observed = read_once(token, view net)
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
fn original_call(function: &Function) -> (&Expression, BlockId, usize) {
    for block in &function.blocks {
        for (index, statement) in block.statements.iter().enumerate() {
            let (StatementKind::Let { value, .. } | StatementKind::Evaluate(value)) =
                &statement.kind
            else {
                continue;
            };
            let hir::ExpressionKind::Call {
                ownership: hir::CallOwnership::Source(source),
                ..
            } = &value.kind
            else {
                continue;
            };
            if source
                .arguments
                .iter()
                .any(|argument| argument.effect == CheckedCallerEffect::RelinquishOwned)
            {
                return (value, block.id, index);
            }
        }
    }
    panic!("exact original bare Resource to View Source call");
}
#[test]
fn resource_original_call_association_keeps_custody_pending_without_ordinary_owner_claim() {
    for release in [false, true] {
        let checked = checked(SOURCE, release);
        let types = &checked.checked().interner;
        let program = lowered(&checked);
        let function = scenario(&program);
        let (call, block, index) = original_call(function);
        assert!(
            original_call_at(
                function,
                &program.resource_manifest,
                types,
                call,
                block,
                index
            )
            .unwrap()
        );
        let acquisitions =
            crate::call_ownership::validate_function(&program, function, types).unwrap();
        assert!(acquisitions.resource_pending);
        let hir::ExpressionKind::Call { args, .. } = &call.kind else {
            unreachable!()
        };
        assert_eq!(acquisitions.argument_binding(&args[0]), None);
        assert!(acquisitions.arguments(call).unwrap().is_empty());
        validate_resource_ownership(&program, types).unwrap();
        assert!(
            crate::validate_caller_acquisitions(&program, function, types)
                .unwrap_err()
                .contains("pending ResourceOwnershipPlan")
        );
        let copied_query = call.clone();
        assert!(
            original_call_at(
                function,
                &program.resource_manifest,
                types,
                &copied_query,
                block,
                index
            )
            .unwrap_err()
            .contains("exact current expression site")
        );
        assert!(
            original_call_at(
                function,
                &program.resource_manifest,
                types,
                call,
                block,
                index + 1
            )
            .is_err()
        );
    }
}
#[test]
fn resource_original_call_association_rejects_lost_duplicated_and_changed_source_current_pairs() {
    for release in [false, true] {
        let checked = checked(SOURCE, release);
        let types = &checked.checked().interner;
        let program = lowered(&checked);
        validate_resource_ownership(&program, types).unwrap();
        for mutation in 0..7 {
            let mut changed = program.clone();
            let function = changed
                .functions
                .iter_mut()
                .find(|function| {
                    function.identity.declaration.namespace == "app"
                        && function.identity.declaration.name == "scenario"
                })
                .unwrap();
            let witness = function.resource_lowering.as_mut().unwrap();
            match mutation {
                0 => witness.calls.clear(),
                1 => witness.calls.push(witness.calls[0].clone()),
                2 => witness.calls[0].original.span.start += 1,
                3 => witness.calls[0].current.span.start += 1,
                4 => {
                    let block = function.blocks.iter_mut().find(|block| block.statements.iter().any(|statement|
                        matches!(&statement.kind, StatementKind::Let { value, .. } if matches!(&value.kind, hir::ExpressionKind::Call { ownership: hir::CallOwnership::Source(source), .. } if source.arguments.iter().any(|argument| argument.effect == CheckedCallerEffect::RelinquishOwned))))).unwrap();
                    let statement = block.statements.iter().find(|statement|
                        matches!(&statement.kind, StatementKind::Let { value, .. } if matches!(&value.kind, hir::ExpressionKind::Call { ownership: hir::CallOwnership::Source(source), .. } if source.arguments.iter().any(|argument| argument.effect == CheckedCallerEffect::RelinquishOwned)))).unwrap().clone();
                    block.statements.push(statement);
                }
                5 | 6 => {
                    let statement = function.blocks.iter_mut().flat_map(|block| &mut block.statements).find(|statement|
                        matches!(&statement.kind, StatementKind::Let { value, .. } if matches!(&value.kind, hir::ExpressionKind::Call { ownership: hir::CallOwnership::Source(source), .. } if source.arguments.iter().any(|argument| argument.effect == CheckedCallerEffect::RelinquishOwned)))).unwrap();
                    let StatementKind::Let { value, .. } = &mut statement.kind else {
                        unreachable!()
                    };
                    let hir::ExpressionKind::Call {
                        evaluation_order,
                        ownership: hir::CallOwnership::Source(source),
                        ..
                    } = &mut value.kind
                    else {
                        unreachable!()
                    };
                    if mutation == 5 {
                        evaluation_order.reverse();
                    } else {
                        source.arguments[0].source_span.start += 1;
                    }
                }
                _ => unreachable!(),
            }
            assert!(
                validate_resource_ownership(&changed, types).is_err(),
                "mutation {mutation}"
            );
        }
        let mut prepared = program.clone();
        crate::prepare_native_generated_functions(&mut prepared, types);
        validate_resource_ownership(&prepared, types).unwrap();
        let (call, block, index) = original_call(scenario(&prepared));
        assert!(
            original_call_at(
                scenario(&prepared),
                &prepared.resource_manifest,
                types,
                call,
                block,
                index
            )
            .unwrap()
        );
    }
}
#[test]
fn resource_original_call_association_captures_unnamed_nested_optional_custody_result() {
    const SOURCE: &str = r#"namespace app
function accept(value: optional[resource_probe.TestHandle]) returns nothing:
    return nothing
function scenario() returns nothing:
    use resource_probe
    accept(resource_probe.empty())
    return nothing
"#;
    for release in [false, true] {
        let checked = checked(SOURCE, release);
        let types = &checked.checked().interner;
        let program = lowered(&checked);
        let function = scenario(&program);
        assert!(
            function
                .locals
                .iter()
                .all(|local| !custody_type(types, local.ty))
        );
        assert!(!custody_type(types, function.return_type));
        let witness = function
            .resource_lowering
            .as_ref()
            .expect("unnamed Resource expression retains original constructor authority");
        assert_eq!(
            witness.calls.len(),
            2,
            "outer Source consumer and original nested optional producer"
        );
        validate_resource_ownership(&program, types).unwrap();
        let mut changed = program.clone();
        changed
            .functions
            .iter_mut()
            .find(|function| function.id == scenario(&program).id)
            .unwrap()
            .resource_lowering = None;
        assert!(validate_call_ownership(&changed, types).is_err());
    }
}
