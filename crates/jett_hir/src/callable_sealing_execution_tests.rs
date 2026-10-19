use super::*;
use crate::evaluated_values::{ValueContext, function_value_candidates, value_expression};
use jett_common::SourceOrigin;
use jett_typecheck::CheckedResourceProgram;

const WITNESS: &str = include_str!(
    "../../jett_comptime/src/resource_execution/fixtures/actor_handler_required_callable.jett"
);
fn checked(release: bool) -> Arc<CheckedResourceProgram> {
    let file = FileId::new(0);
    Arc::new(
        CheckedResourceProgram::prepare(
            jett_parser::parse(WITNESS, file),
            HashMap::from([(file, SourceOrigin::Project)]),
            &[],
            jett_typecheck::CheckOptions { release },
        )
        .unwrap(),
    )
}
fn required(checked: &Arc<CheckedResourceProgram>) -> jett_comptime::ExplicitComptimeValues {
    let result = jett_comptime::evaluate_explicit_comptime_expressions_capture(
        checked.module(),
        checked.checked().reflection_metadata.clone(),
        Arc::new(
            jett_comptime::checked_types::CheckedExpressionTypes::from_checked_program(checked),
        ),
        Arc::new(checked.checked().breakpoint_exclusions.clone()),
    );
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    result.values
}
fn bound(
    original: &Program,
    checked: &Arc<CheckedResourceProgram>,
    values: &jett_comptime::ExplicitComptimeValues,
) -> Program {
    let types = &checked.checked().interner;
    let mut program = original.clone();
    let functions = function_value_candidates(
        &program.functions,
        &checked.checked().method_value_definitions,
    );
    crate::materialize_checked_reflected_values(
        &mut program,
        values,
        types,
        |value, ty, span, locals, bindings| {
            value_expression(
                value,
                ty,
                span,
                &mut ValueContext {
                    actor_seeds: None,
                    types,
                    reflection: &checked.checked().reflection_metadata,
                    functions: &functions,
                    refinement_functions: None,
                    locals,
                    bindings,
                },
            )
        },
    )
    .unwrap();
    let before = program.clone();
    let conversion =
        crate::interface_values::recorded_coercion_for_test(&mut program, &before, types).unwrap();
    crate::actor_handlers::conversion(&mut program, &before, types, &conversion).unwrap();
    crate::mutable_sum_backings::conversion(&mut program, &before, types, &conversion).unwrap();
    assert!(
        crate::temporary_local_views::conversion(&program, &before, types, &conversion)
            .unwrap()
            .is_none()
    );
    super::super::baked::complete_conversion(&mut program, &before).unwrap();
    assert!(crate::resource_materialization::functions_equal(
        program.resource_source.execution_functions(),
        &program.functions,
    ));
    program
}
fn baseline(
    release: bool,
) -> (
    Program,
    Arc<CheckedResourceProgram>,
    jett_comptime::ExplicitComptimeValues,
) {
    let checked = checked(release);
    let values = required(&checked);
    let original = crate::lower_checked_ordinary_program(&checked).unwrap();
    (bound(&original, &checked, &values), checked, values)
}
fn validated_sealed(program: &Program, checked: &Arc<CheckedResourceProgram>) {
    crate::validate(program).unwrap();
    crate::validate_backend_types(program, &checked.checked().interner).unwrap();
    crate::actor_handlers::validate_program(program).unwrap();
    assert!(crate::resource_materialization::functions_equal(
        program.resource_source.execution_functions(),
        &program.functions,
    ));
}

#[test]
fn actual_sealing_receipt_advances_full_actor_required_execution_table_and_is_idempotent() {
    for release in [false, true] {
        let (mut program, checked, values) = baseline(release);
        let types = &checked.checked().interner;
        let original = program.resource_source.functions().to_vec();
        let before = program.clone();
        let receipt = super::super::seal_program_with_receipt(&mut program, types).unwrap();
        assert!(!receipt.0.edits.is_empty());
        assert!(receipt.0.edits.iter().all(|edit| edit.previous.is_some()));
        assert!(Arc::ptr_eq(receipt.checked().unwrap(), &checked));
        assert!(same_program(
            &receipt.replay(&before, types).unwrap(),
            receipt.output()
        ));
        receipt
            .authenticate_output(&before, receipt.output(), types)
            .unwrap();
        let renewed =
            super::super::baked::callable_sealing(receipt.output(), &before, types, &receipt)
                .unwrap()
                .unwrap();
        assert!(crate::resource_materialization::functions_equal(
            renewed.current_functions(),
            &program.functions
        ));
        validated_sealed(&program, &checked);
        assert!(super::super::baked::same_required_cache(&program, &values));
        assert!(crate::resource_materialization::functions_equal(
            &original,
            program.resource_source.functions()
        ));
        let sealed = program.functions.clone();
        crate::complete_value_conversions(&mut program, types).unwrap();
        validated_sealed(&program, &checked);
        assert!(crate::resource_materialization::functions_equal(
            &sealed,
            &program.functions
        ));
        assert!(super::super::baked::same_required_cache(&program, &values));
    }
}

#[test]
fn actual_sealing_receipt_rejects_foreign_program_cache_arena_and_archive_identity() {
    for release in [false, true] {
        let (mut program, checked, values) = baseline(release);
        let types = &checked.checked().interner;
        let before = program.clone();
        let receipt = super::super::seal_program_with_receipt(&mut program, types).unwrap();
        validated_sealed(&program, &checked);
        let (foreign, foreign_checked, _) = baseline(release);
        let foreign_types = &foreign_checked.checked().interner;
        assert!(!std::ptr::eq(types, foreign_types));
        assert!(receipt.replay(&before, foreign_types).is_err());
        assert!(receipt.replay(&foreign, types).is_err());
        let second_values = required(&checked);
        assert!(!values.same_checked_cache(&second_values));
        let original = crate::lower_checked_ordinary_program(&checked).unwrap();
        let second = bound(&original, &checked, &second_values);
        let mut different_cache = before.clone();
        different_cache.resource_source.reflected_values = second.resource_source.reflected_values;
        assert!(receipt.replay(&different_cache, types).is_err());
        let mut different_required_cache = before.clone();
        different_required_cache.resource_source.required_cache = Some(Arc::new(second_values));
        assert!(receipt.replay(&different_required_cache, types).is_err());
        let mut different_execution = before.clone();
        different_execution.resource_source.install_materialized(
            crate::resource_source::Materialized {
                functions: before.functions.clone(),
                values: Vec::new(),
                required_only: Vec::new(),
            },
        );
        assert!(receipt.replay(&different_execution, types).is_err());
    }
}

#[test]
fn actual_sealing_receipt_refuses_previous_current_and_occurrence_token_splices() {
    for release in [false, true] {
        let (mut program, checked, _) = baseline(release);
        let types = &checked.checked().interner;
        let before = program.clone();
        let receipt = super::super::seal_program_with_receipt(&mut program, types).unwrap();
        validated_sealed(&program, &checked);
        let mut independently_sealed = before.clone();
        let other =
            super::super::seal_program_with_receipt(&mut independently_sealed, types).unwrap();
        assert_ne!(receipt.0.edits[0].current, other.0.edits[0].current);
        for corruption in 0..7 {
            let mut edits = receipt.0.edits.clone();
            match corruption {
                0 => edits[0].previous = None,
                1 => edits[0].current = other.0.edits[0].current.clone(),
                2 => edits[0].expression += 1,
                3 => edits[0].owner = FunctionId::new(u32::MAX),
                4 => edits[0].slot = Slot::Getter { field: 0 },
                5 => edits.push(edits[0].clone()),
                6 => {
                    edits.remove(0);
                }
                _ => unreachable!(),
            }
            assert!(
                produce(
                    &before,
                    receipt.output(),
                    types,
                    receipt.source_epoch().clone(),
                    edits
                )
                .is_err(),
                "corruption {corruption}"
            );
        }
        let mut changed_reference = receipt.0.edits.clone();
        assert!(changed_reference[0].current.data.reference.is_some());
        Arc::make_mut(&mut changed_reference[0].current.data).reference = None;
        let replayed_reference = replay_edits(&before, &changed_reference).unwrap();
        assert!(
            produce(
                &before,
                &replayed_reference,
                types,
                receipt.source_epoch().clone(),
                changed_reference,
            )
            .is_err()
        );
        let wrong_epoch = Arc::new(program.functions.clone());
        assert!(
            produce(
                &before,
                receipt.output(),
                types,
                wrong_epoch,
                receipt.0.edits.clone()
            )
            .is_err()
        );
    }
}

#[test]
fn actual_sealing_receipt_refuses_complete_unrelated_output_edits_without_publication() {
    for release in [false, true] {
        let (mut program, checked, _) = baseline(release);
        let types = &checked.checked().interner;
        let before = program.clone();
        let receipt = super::super::seal_program_with_receipt(&mut program, types).unwrap();
        validated_sealed(&program, &checked);
        for corruption in 0..8 {
            let mut changed = receipt.output().clone();
            let handler = changed
                .functions
                .iter()
                .position(|function| function.actor_handler.is_some())
                .unwrap();
            match corruption {
                0 => changed.functions[0].body.span.end += 1,
                1 => changed.functions[0].capture_count += 1,
                2 => {
                    changed.functions[handler].locals[0].mutable =
                        !changed.functions[handler].locals[0].mutable
                }
                3 => changed.functions.swap(0, 1),
                4 => changed.functions[handler].actor_handler = None,
                5 => {
                    changed
                        .equality_methods
                        .insert(TypeInterner::INT64, FunctionId::new(0));
                }
                6 => {
                    let mut extra = changed.functions[0].clone();
                    extra.id = FunctionId::new(changed.functions.len() as u32);
                    changed.functions.push(extra);
                }
                7 => {
                    changed.functions[handler].return_type = TypeInterner::NOTHING;
                }
                _ => unreachable!(),
            }
            let saved = changed.clone();
            assert!(
                receipt
                    .authenticate_output(&before, &changed, types)
                    .is_err()
            );
            assert!(
                super::super::baked::callable_sealing(&changed, &before, types, &receipt).is_err()
            );
            assert!(same_program(&changed, &saved));
        }
    }
}
