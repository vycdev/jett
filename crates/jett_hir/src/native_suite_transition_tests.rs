use super::*;
use crate::{CallOwnership, DeclarationKind};
use jett_common::SourceOrigin;
use jett_comptime::checked_types::CheckedExpressionTypes;
use jett_comptime::verify::{
    CheckedPropertyCases, collect_checked_property_cases_with_checked_values,
};
use jett_resolve::ResourceKernelSpec;
use jett_types::ResourceKernelRecipe;
use std::collections::HashMap;

const SOURCE17: &str = include_str!(
    "../../jett_comptime/src/resource_execution/fixtures/17_absent_aggregate_shapes.jett"
);
const SUPPORT: &str =
    include_str!("../../jett_driver/tests/native_conformance/resource/resource_probe.jett");

fn checked(release: bool) -> Arc<CheckedResourceProgram> {
    let support = FileId::new(10_000);
    let entry = FileId::new(0);
    let mut parsed = jett_parser::parse(SUPPORT, support);
    let source = jett_parser::parse(SOURCE17, entry);
    assert!(parsed.errors.is_empty() && source.errors.is_empty());
    let declarations = parsed
        .module
        .items
        .iter()
        .filter_map(|item| match item {
            jett_parser::ast::Item::Resource(value) => Some(value.name.span),
            _ => None,
        })
        .collect::<Vec<_>>();
    let [declaration] = declarations.as_slice() else {
        panic!("one original Resource declaration");
    };
    let catalog = [
        ("kernel_create", ResourceKernelRecipe::NetworkFactory),
        ("kernel_borrow", ResourceKernelRecipe::NetworkBorrow),
        ("kernel_close", ResourceKernelRecipe::Finalize),
    ]
    .into_iter()
    .map(|(member, recipe)| ResourceKernelSpec {
        resource_declaration: *declaration,
        member: member.into(),
        recipe,
    })
    .collect::<Vec<_>>();
    parsed.module.items.extend(source.module.items);
    Arc::new(
        CheckedResourceProgram::prepare(
            parsed,
            HashMap::from([
                (support, SourceOrigin::Stdlib),
                (entry, SourceOrigin::Project),
            ]),
            &catalog,
            jett_typecheck::CheckOptions { release },
        )
        .unwrap(),
    )
}

fn capture(checked: &Arc<CheckedResourceProgram>) -> Arc<ExplicitComptimeValues> {
    let evaluation = jett_comptime::evaluate_explicit_comptime_expressions_capture(
        checked.module(),
        checked.checked().reflection_metadata.clone(),
        Arc::new(CheckedExpressionTypes::from_checked_program(checked)),
        Arc::new(checked.checked().breakpoint_exclusions.clone()),
    );
    assert!(
        evaluation.diagnostics.is_empty(),
        "{:?}",
        evaluation.diagnostics
    );
    assert!(evaluation.debug_events.is_empty());
    Arc::new(evaluation.values)
}

fn prepared(
    release: bool,
) -> (
    Program,
    Arc<CheckedResourceProgram>,
    Arc<ExplicitComptimeValues>,
) {
    let checked = checked(release);
    let values = capture(&checked);
    let mut program = crate::lower_checked_resource_program_with_test_bodies(&checked).unwrap();
    crate::bake_required_values(
        &mut program,
        &values,
        &checked.checked().interner,
        &checked.checked().reflection_metadata,
        &checked.checked().method_value_definitions,
    )
    .unwrap();
    (program, checked, values)
}

fn cases(
    checked: &Arc<CheckedResourceProgram>,
    values: &Arc<ExplicitComptimeValues>,
) -> CheckedPropertyCases {
    collect_checked_property_cases_with_checked_values(checked.clone(), values.clone()).unwrap()
}

fn receipts(program: &Program, entry: FunctionId) -> Vec<CheckedNativeSuiteCall> {
    let mut body = program.functions[entry.index() as usize].body.clone();
    let mut ordinal = 0;
    let mut output = Vec::new();
    crate::resource_materialization::walk_block(&mut body, &mut |expression| {
        ordinal += 1;
        if let ExpressionKind::Call {
            ownership: CallOwnership::Generated(packet),
            ..
        } = &expression.kind
            && matches!(
                packet.operation,
                crate::GeneratedOperation::NativeSuite { .. }
            )
        {
            output.push(
                program
                    .resource_source
                    .native_suite_call(entry, expression, ordinal)
                    .unwrap()
                    .clone(),
            );
        }
        Ok(())
    })
    .unwrap();
    output
}

#[test]
fn source17_native_suite_transition_retains_whole_source_and_distinct_trial_occurrences() {
    for release in [false, true] {
        let (mut program, checked, values) = prepared(release);
        let original = program.clone();
        assert!(!program.resource_manifest.kinds().is_empty());
        assert!(
            program
                .functions
                .iter()
                .any(|function| function.identity.declaration.name == "refined_shape_contract")
        );
        let verify =
            crate::append_checked_native_verify_suite(&mut program, &checked, FileId::new(0))
                .unwrap()
                .unwrap();
        let cases = cases(&checked, &values);
        let plan = crate::append_checked_native_property_suite(
            &mut program,
            &checked,
            FileId::new(0),
            &cases,
        )
        .unwrap()
        .unwrap();
        assert_eq!(plan.trials().len(), cases.cases().len());
        assert_eq!(
            plan.trials().len(),
            jett_comptime::verify::PROPERTY_DEFAULT_ITERATIONS
        );
        assert_eq!(program.resource_manifest, original.resource_manifest);
        assert_eq!(program.equality_methods, original.equality_methods);
        assert!(crate::resource_materialization::functions_equal(
            program.resource_source.functions(),
            original.resource_source.functions()
        ));
        assert!(crate::resource_materialization::functions_equal(
            &program.functions[..original.functions.len()],
            &original.functions
        ));
        assert_eq!(receipts(&program, verify).len(), 1);
        let calls = receipts(&program, plan.entry());
        assert_eq!(calls.len(), plan.trials().len());
        for (index, call) in calls.iter().enumerate() {
            assert!(call.belongs_to_archive(&program.resource_source));
            assert!(calls[..index].iter().all(|other| !call.same_receipt(other)));
            if let Some(previous) = index.checked_sub(1) {
                assert!(call.expression_ordinal() > calls[previous].expression_ordinal());
            }
        }
        crate::complete_value_conversions(&mut program, &checked.checked().interner).unwrap();
        crate::validate(&program).unwrap();
        program
            .resource_source
            .validate_current(&program, &checked.checked().interner)
            .unwrap();
        assert!(!calls[0].belongs_to_archive(&program.resource_source));
        assert_eq!(receipts(&program, plan.entry()).len(), plan.trials().len());
    }
}

#[test]
fn source17_native_suite_append_refuses_changed_originals_and_manufactured_wrapper() {
    let (original, checked, _) = prepared(false);
    let (foreign, _, _) = prepared(false);
    for case in 0..5 {
        let mut changed = original.clone();
        match case {
            0 => changed
                .functions
                .iter_mut()
                .find(|function| function.identity.declaration.name == "empty_tokens")
                .unwrap()
                .body
                .statements
                .clear(),
            1 => changed.resource_source = foreign.resource_source.clone(),
            2 => changed.resource_source = ResourceSourceArchive::empty(),
            3 => changed.resource_manifest = crate::ResourceManifest::default(),
            4 => {
                let mut fake = changed
                    .functions
                    .iter()
                    .find(|function| function.identity.declaration.kind == DeclarationKind::Verify)
                    .unwrap()
                    .clone();
                fake.id = FunctionId::new(u32::try_from(changed.functions.len()).unwrap());
                fake.identity.declaration.name = "__native_verify_suite:forged".into();
                changed.functions.push(fake);
            }
            _ => unreachable!(),
        }
        let before = changed.clone();
        assert!(
            crate::append_checked_native_verify_suite(&mut changed, &checked, FileId::new(0))
                .is_err(),
            "case {case}"
        );
        assert!(crate::resource_materialization::functions_equal(
            &before.functions,
            &changed.functions
        ));
        assert_eq!(before.resource_source, changed.resource_source);
    }
}

#[test]
fn source17_native_property_requires_exact_cache_and_replays_original_case_identities() {
    let (mut program, checked, values) = prepared(false);
    let fresh = capture(&checked);
    let fresh_cases = cases(&checked, &fresh);
    assert!(
        crate::append_checked_native_property_suite(
            &mut program,
            &checked,
            FileId::new(0),
            &fresh_cases
        )
        .is_err()
    );
    let original_cases = cases(&checked, &values);
    let plan = crate::append_checked_native_property_suite(
        &mut program,
        &checked,
        FileId::new(0),
        &original_cases,
    )
    .unwrap()
    .unwrap();
    crate::complete_value_conversions(&mut program, &checked.checked().interner).unwrap();
    let selected = plan.trials()[7..9]
        .iter()
        .map(|trial| trial.case().clone())
        .collect::<Vec<_>>();
    let replay = crate::rebuild_checked_native_property_replay(
        &program,
        &plan,
        &selected,
        &checked.checked().interner,
    )
    .unwrap();
    assert_eq!(receipts(&replay, plan.entry()).len(), selected.len());
    let reordered = [selected[1].clone(), selected[0].clone()];
    assert!(
        crate::rebuild_checked_native_property_replay(
            &program,
            &plan,
            &reordered,
            &checked.checked().interner
        )
        .is_err()
    );
    let (mut foreign, foreign_checked, foreign_values) = prepared(false);
    let foreign_cases = cases(&foreign_checked, &foreign_values);
    let foreign_plan = crate::append_checked_native_property_suite(
        &mut foreign,
        &foreign_checked,
        FileId::new(0),
        &foreign_cases,
    )
    .unwrap()
    .unwrap();
    assert!(
        crate::rebuild_checked_native_property_replay(
            &program,
            &foreign_plan,
            &selected,
            &checked.checked().interner
        )
        .is_err()
    );
}

#[test]
fn source17_native_suite_current_table_cannot_renew_mutated_trial_or_missing_receipts() {
    let (mut original, checked, values) = prepared(false);
    let sampled = cases(&checked, &values);
    let plan = crate::append_checked_native_property_suite(
        &mut original,
        &checked,
        FileId::new(0),
        &sampled,
    )
    .unwrap()
    .unwrap();
    for case in 0..4 {
        let mut changed = original.clone();
        let wrapper = &mut changed.functions[plan.entry().index() as usize];
        match case {
            0 => {
                wrapper.body.statements.remove(0);
            }
            1 => {
                wrapper.body.statements.swap(0, 3);
            }
            2 => {
                wrapper
                    .body
                    .statements
                    .push(wrapper.body.statements[0].clone());
            }
            3 => {
                wrapper.identity.declaration.namespace = "foreign".into();
            }
            _ => unreachable!(),
        };
        assert!(
            crate::complete_value_conversions(&mut changed, &checked.checked().interner).is_err(),
            "case {case}"
        );
    }
    let mut missing = original.clone();
    missing.resource_source.native_suites = None;
    assert!(
        missing
            .resource_source
            .native_suite_function(plan.entry())
            .is_none()
    );
    assert!(
        missing
            .resource_source
            .validate_current(&missing, &checked.checked().interner)
            .is_err()
    );
    assert!(
        crate::append_checked_native_verify_suite(&mut missing, &checked, FileId::new(0)).is_err()
    );
}

const TEMPORARY: &str = "namespace app\nstruct Packet:\n    data: list[int64]\n    sibling: list[int64]\nfunction make_packet() returns Packet:\n    return Packet(data: list(1, 2), sibling: list(3, 4))\nfunction inspect(view data: list[int64]) returns nothing:\n    return nothing\nfunction main() returns nothing:\n    list[int64] borrowed = view make_packet().data\n    inspect(view borrowed)\n    list[int64] forwarded = borrowed\n    inspect(view forwarded)\n    list[int64] independent = clone forwarded\nverify independent:\n    assert true\nproperty independent_trials:\n    given marker: int64\n    assert marker == marker\n";

#[test]
fn temporary_only_archive_appends_verify_and_property_without_graph_or_resource_authority() {
    for release in [false, true] {
        let file = FileId::new(0);
        let checked = Arc::new(
            CheckedResourceProgram::prepare(
                jett_parser::parse(TEMPORARY, file),
                HashMap::from([(file, SourceOrigin::Project)]),
                &[],
                jett_typecheck::CheckOptions { release },
            )
            .unwrap(),
        );
        let values = capture(&checked);
        let mut program = crate::lower_checked_ordinary_program_with_test_bodies(&checked).unwrap();
        let main = program
            .functions
            .iter()
            .find(|function| function.identity.declaration.name == "main")
            .unwrap()
            .id;
        let original_markers = crate::temporary_local_backings(
            &program,
            &program.functions[main.index() as usize],
            &checked.checked().interner,
        )
        .unwrap();
        assert_eq!(original_markers.len(), 1);
        assert!(program.resource_manifest.kinds().is_empty());
        assert!(program.resource_source.graph_values.is_none());
        crate::bake_required_values(
            &mut program,
            &values,
            &checked.checked().interner,
            &checked.checked().reflection_metadata,
            &checked.checked().method_value_definitions,
        )
        .unwrap();
        crate::append_checked_native_verify_suite(&mut program, &checked, file)
            .unwrap()
            .unwrap();
        let sampled = cases(&checked, &values);
        let plan =
            crate::append_checked_native_property_suite(&mut program, &checked, file, &sampled)
                .unwrap()
                .unwrap();
        crate::complete_value_conversions(&mut program, &checked.checked().interner).unwrap();
        crate::validate(&program).unwrap();
        crate::validate_backend_types(&program, &checked.checked().interner).unwrap();
        assert_eq!(
            crate::temporary_local_backings(
                &program,
                &program.functions[main.index() as usize],
                &checked.checked().interner,
            )
            .unwrap(),
            original_markers
        );
        assert!(program.resource_manifest.kinds().is_empty());
        assert!(program.resource_source.graph_values.is_none());
        assert_eq!(
            receipts(&program, plan.entry()).len(),
            sampled.cases().len()
        );
    }
}

#[test]
fn suite_and_temporary_tables_replay_fresh_required_callable_sealing() {
    let reflected = "namespace app\ntype Positive = int64 where value > 0\nstruct Holder:\n    callback: function() returns int64\nfunction five() returns int64:\n    return 5\nfunction positive() returns Positive:\n    return 5\nfunction stored() returns function() returns Positive:\n    Holder holder = Holder(callback: five)\n    mutable function() returns Positive selected = positive\n    for field in type.fields[Holder]():\n        selected = type.field_value[Holder, function() returns Positive](view holder, view field)\n    return selected\n";
    let source = TEMPORARY.replace("namespace app\n", reflected).replace(
        "function main() returns nothing:\n",
        "function main() returns nothing:\n    function() returns Positive baked = comptime stored()\n    int64 observed = coarsen baked()\n",
    );
    let file = FileId::new(0);
    let checked = Arc::new(
        CheckedResourceProgram::prepare(
            jett_parser::parse(&source, file),
            HashMap::from([(file, SourceOrigin::Project)]),
            &[],
            jett_typecheck::CheckOptions::default(),
        )
        .unwrap(),
    );
    let values = capture(&checked);
    let mut program = crate::lower_checked_ordinary_program_with_test_bodies(&checked).unwrap();
    crate::bake_required_values(
        &mut program,
        &values,
        &checked.checked().interner,
        &checked.checked().reflection_metadata,
        &checked.checked().method_value_definitions,
    )
    .unwrap();
    let mut staged = false;
    for function in &program.functions {
        let mut body = function.body.clone();
        crate::resource_materialization::walk_block(&mut body, &mut |expression| {
            if let ExpressionKind::InterfaceCoerce { adapters, .. } = &expression.kind {
                staged |= adapters.iter().any(|adapter| {
                    adapter
                        .reflected
                        .as_ref()
                        .is_some_and(|plan| plan.is_materialized() && !plan.is_sealed())
                });
            }
            Ok(())
        })
        .unwrap();
    }
    assert!(
        staged,
        "required callable remains at its exact pre-conversion adapter site"
    );
    crate::append_checked_native_verify_suite(&mut program, &checked, file)
        .unwrap()
        .unwrap();
    let sampled = cases(&checked, &values);
    let plan = crate::append_checked_native_property_suite(&mut program, &checked, file, &sampled)
        .unwrap()
        .unwrap();
    crate::complete_value_conversions(&mut program, &checked.checked().interner).unwrap();
    program
        .resource_source
        .validate_current(&program, &checked.checked().interner)
        .unwrap();
    assert!(
        program
            .functions
            .iter()
            .flat_map(crate::reflected_callable_plans)
            .all(|plan| plan.is_sealed())
    );
    assert_eq!(
        receipts(&program, plan.entry()).len(),
        sampled.cases().len()
    );
    // A subsequent pass must validate the fully sealed table, not the earlier
    // pre-sealing snapshot or a callable-metadata-erased approximation.
    crate::complete_value_conversions(&mut program, &checked.checked().interner).unwrap();
    program
        .resource_source
        .validate_current(&program, &checked.checked().interner)
        .unwrap();
}
