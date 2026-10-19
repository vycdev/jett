use super::*;
use crate::resource_ownership::tests::checked;
use jett_common::FileId;
use jett_comptime::checked_types::CheckedExpressionTypes;
use std::sync::Arc;

const SOURCE: &str = r#"namespace app
function absent() returns list[optional[resource_probe.TestHandle]]:
    return list(none, none)
function probe() returns int64:
    list[optional[resource_probe.TestHandle]] values = absent()
    return 17
int64 baked = comptime probe()
verify absent_values:
    assert probe() == baked
property repeated_values:
    given marker: bool
    assert probe() == baked
    assert marker == marker
"#;

fn fixture(
    property: bool,
    release: bool,
) -> (
    Program,
    Arc<jett_typecheck::CheckedResourceProgram>,
    FunctionId,
    usize,
) {
    let checked = checked(SOURCE, release);
    let evaluation = jett_comptime::evaluate_explicit_comptime_expressions_capture(
        checked.module(),
        checked.checked().reflection_metadata.clone(),
        Arc::new(CheckedExpressionTypes::from_checked_program(&checked)),
        Arc::new(checked.checked().breakpoint_exclusions.clone()),
    );
    assert!(
        evaluation.diagnostics.is_empty(),
        "{:?}",
        evaluation.diagnostics
    );
    let values = Arc::new(evaluation.values);
    let mut program = hir::lower_checked_resource_program_with_test_bodies(&checked).unwrap();
    let types = &checked.checked().interner;
    hir::materialize_checked_required_values(&mut program, &values, types).unwrap();
    let (entry, expected) = if property {
        let cases = jett_comptime::verify::collect_checked_property_cases_with_checked_values(
            checked.clone(),
            values,
        )
        .unwrap();
        let plan = hir::append_checked_native_property_suite(
            &mut program,
            &checked,
            FileId::new(0),
            &cases,
        )
        .unwrap()
        .unwrap();
        (plan.entry(), plan.trials().len())
    } else {
        let entry = hir::append_checked_native_verify_suite(&mut program, &checked, FileId::new(0))
            .unwrap()
            .unwrap();
        (entry, 1)
    };
    hir::complete_value_conversions(&mut program, types).unwrap();
    (
        crate::lower(&program, types).unwrap(),
        checked,
        entry,
        expected,
    )
}

fn assert_plan(program: &Program, types: &TypeInterner, entry: FunctionId, expected: usize) {
    let plan = validate_resource_ownership_for_entry(program, types, entry).unwrap();
    assert!(plan.entry_scope().is_none());
    assert!(plan.original_source(entry).is_none());
    assert!(plan.authenticated_entry(entry).is_some());
    let suite = plan.function(entry).unwrap();
    assert!(suite.parameters().is_empty());
    assert!(suite.owner_slots().is_empty());
    assert!(suite.loans().is_empty());
    assert!(suite.provisional_return().is_none());
    let invocations = suite
        .operations()
        .iter()
        .filter_map(|operation| match operation.role() {
            ResourceOperationRole::InvokeNativeSuite { invocation } => Some(invocation),
            ResourceOperationRole::Complete { .. } => None,
            other => panic!("unexpected suite custody authority: {other:?}"),
        })
        .collect::<Vec<_>>();
    assert_eq!(invocations.len(), expected);
    for (index, invocation) in invocations.iter().enumerate() {
        assert_eq!(invocation.proof().owner(), entry);
        assert_eq!(invocation.proof().target(), invocation.function());
        assert!(
            invocation.proof().belongs_to_archive(
                &program.functions[entry.index() as usize]
                    .resource_lowering
                    .as_ref()
                    .unwrap()
                    .source,
            )
        );
        assert!(invocations[..index].iter().all(|earlier| {
            earlier.proof().expression_ordinal() < invocation.proof().expression_ordinal()
                && !earlier.proof().same_receipt(invocation.proof())
        }));
        assert!(
            invocation
                .operands()
                .iter()
                .all(|operand| matches!(operand, ResourceCallOperand::Ordinary { .. }))
        );
        assert!(
            invocation
                .formals()
                .iter()
                .all(|formal| formal.effect() == ResourceArgumentEffect::Copy)
        );
    }
    for function in plan
        .functions()
        .iter()
        .filter(|function| function.identity().declaration.namespace == "app")
    {
        assert!(!function.operations().iter().any(|operation| matches!(
            operation.role(),
            ResourceOperationRole::Acquire { .. } | ResourceOperationRole::InvokeHook { .. }
        )));
    }
}

#[test]
fn canonical_absent_resource_suites_keep_exact_occurrences_without_provider_inputs() {
    for release in [false, true] {
        for property in [false, true] {
            let (mut program, checked, entry, expected) = fixture(property, release);
            let types = &checked.checked().interner;
            assert_plan(&program, types, entry, expected);
            crate::prepare_native_sequences(&mut program, types);
            crate::prepare_native_uninhabited_sums(&mut program, types);
            crate::prepare_native_generated_functions(&mut program, types);
            assert_plan(&program, types, entry, expected);
        }
    }
}

#[test]
fn suite_context_removal_and_raw_generated_wrappers_cannot_recreate_factory_authority() {
    let (program, checked, entry, expected) = fixture(true, false);
    let types = &checked.checked().interner;
    assert_plan(&program, types, entry, expected);
    let mut erased = program.clone();
    let wrapper = &mut erased.functions[entry.index() as usize];
    wrapper.blocks[0].statements.remove(0);
    assert!(validate_resource_ownership_for_entry(&erased, types, entry).is_err());
    let mut raw = program.clone();
    raw.functions[entry.index() as usize].resource_lowering = None;
    assert!(validate_resource_ownership_for_entry(&raw, types, entry).is_err());
}

#[test]
fn changed_suite_target_stays_refused_when_general_current_cache_is_locally_exact() {
    let (mut program, checked, entry, expected) = fixture(true, false);
    let types = &checked.checked().interner;
    assert_plan(&program, types, entry, expected);
    let wrapper = &mut program.functions[entry.index() as usize];
    let StatementKind::Evaluate(expression) = &mut wrapper.blocks[0].statements[1].kind else {
        panic!("canonical context then call");
    };
    let hir::ExpressionKind::Call { function, .. } = &mut expression.kind else {
        panic!("canonical suite call");
    };
    *function = entry;
    let witness = wrapper.resource_lowering.as_mut().unwrap();
    witness.blocks = wrapper.blocks.clone();
    assert!(validate_resource_ownership_for_entry(&program, types, entry).is_err());
}
