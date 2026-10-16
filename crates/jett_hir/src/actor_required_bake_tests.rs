use super::*;
use crate::evaluated_values::{ValueContext, function_value_candidates, value_expression};
use jett_common::SourceOrigin;
use jett_typecheck::CheckedResourceProgram;

const WITNESS: &str = include_str!(
    "../../jett_comptime/src/resource_execution/fixtures/actor_handler_required_callable.jett"
);
const SCALAR: &str = "namespace app
function five() returns int64:
    return 5
actor Worker:
    mutable int64 count = 0
    receive first responds int64:
        int64 baked = comptime five()
        respond baked
    receive second responds int64:
        int64 baked = comptime five()
        respond baked
function main() returns int64:
    Worker worker = spawn Worker()
    int64 first = ask worker.first
    int64 second = ask worker.second
    return first + second
";
const VIEWS: &str = "namespace app
struct Inner:
    value: int64
struct Outer:
    inner: Inner
function five() returns int64:
    return 5
actor Counter:
    mutable Outer owned = Outer(inner: Inner(value: comptime five()))
    Inner projected = view owned.inner
    Inner alias = view projected
    receive read responds int64:
        int64 baked = comptime five()
        respond projected.value + alias.value + baked
function main() returns int64:
    Counter counter = spawn Counter()
    return ask counter.read
";

fn checked(source: &str, release: bool) -> Arc<CheckedResourceProgram> {
    let file = FileId::new(0);
    Arc::new(
        CheckedResourceProgram::prepare(
            jett_parser::parse(source, file),
            HashMap::from([(file, SourceOrigin::Project)]),
            &[],
            jett_typecheck::CheckOptions { release },
        )
        .unwrap_or_else(|error| panic!("{error:?}; {:?}", error.diagnostics())),
    )
}
fn required(checked: &Arc<CheckedResourceProgram>) -> ExplicitComptimeValues {
    let captured = jett_comptime::evaluate_explicit_comptime_expressions_capture(
        checked.module(),
        checked.checked().reflection_metadata.clone(),
        Arc::new(
            jett_comptime::checked_types::CheckedExpressionTypes::from_checked_program(checked),
        ),
        Arc::new(checked.checked().breakpoint_exclusions.clone()),
    );
    assert!(
        captured.diagnostics.is_empty(),
        "{:?}",
        captured.diagnostics
    );
    assert!(captured.debug_events.is_empty());
    captured.values
}
fn setup(
    source: &str,
    release: bool,
) -> (Program, Arc<CheckedResourceProgram>, ExplicitComptimeValues) {
    let checked = checked(source, release);
    let values = required(&checked);
    let program = crate::lower_checked_ordinary_program(&checked).unwrap();
    (program, checked, values)
}
fn same_program(left: &Program, right: &Program) -> bool {
    left.resource_source == right.resource_source
        && left.resource_manifest == right.resource_manifest
        && left.equality_methods == right.equality_methods
        && crate::resource_materialization::functions_equal(&left.functions, &right.functions)
}
fn actual_bake<'types>(
    before: &Program,
    checked: &Arc<CheckedResourceProgram>,
    values: &ExplicitComptimeValues,
    types: &'types TypeInterner,
) -> CheckedRequiredBake<'types> {
    let functions = function_value_candidates(
        &before.functions,
        &checked.checked().method_value_definitions,
    );
    required_bake_receipt::produce(
        before,
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
    .unwrap()
}
fn bake(
    program: &mut Program,
    checked: &Arc<CheckedResourceProgram>,
    values: &ExplicitComptimeValues,
) -> Result<(), Vec<LowerError>> {
    crate::bake_required_values(
        program,
        values,
        &checked.checked().interner,
        &checked.checked().reflection_metadata,
        &checked.checked().method_value_definitions,
    )
}
fn materialized_plans(program: &Program) -> Vec<ReflectedCallablePlan> {
    let mut result = Vec::new();
    for function in &program.functions {
        let mut body = function.body.clone();
        crate::resource_materialization::walk_block(&mut body, &mut |expression| {
            if let ExpressionKind::InterfaceCoerce { adapters, .. } = &expression.kind {
                result.extend(adapters.iter().filter_map(|adapter| {
                    adapter
                        .reflected
                        .as_ref()
                        .filter(|plan| plan.is_materialized() && !plan.is_sealed())
                        .cloned()
                }));
            }
            Ok(())
        })
        .unwrap();
    }
    result
}

#[test]
fn whole_original_actor_witness_bakes_with_exact_owner_cache_and_full_snapshot() {
    for release in [false, true] {
        let (mut program, checked, values) = setup(WITNESS, release);
        let source = program.resource_source.functions().to_vec();
        let before = program.clone();
        let proof = values.checked_required_values(&checked).unwrap().remove(0);
        let mut selected = 0;
        for owner in &before.functions {
            let mut body = owner.body.clone();
            crate::resource_materialization::walk_block(&mut body, &mut |expression| {
                if matches!(expression.kind, ExpressionKind::Comptime { .. }) {
                    assert!(crate::resource_materialization::proof_matches(
                        &proof,
                        owner,
                        expression,
                        &checked,
                        &before.resource_source
                    ));
                    selected += 1;
                }
                Ok(())
            })
            .unwrap();
        }
        assert_eq!(selected, 1);
        bake(&mut program, &checked, &values).unwrap();
        assert!(crate::resource_materialization::functions_equal(
            &source,
            program.resource_source.functions()
        ));
        assert!(same_required_cache(&program, &values));
        validate_conversion_input(&program).unwrap();
        crate::actor_handlers::validate_program(&program).unwrap();
        let before_handler = before
            .functions
            .iter()
            .find(|function| function.actor_handler.is_some())
            .unwrap();
        let handler = &program.functions[before_handler.id.index() as usize];
        let entry = handler.actor_handler.as_ref().unwrap();
        assert_eq!(
            entry.target(),
            before_handler.actor_handler.as_ref().unwrap().target()
        );
        assert_eq!(
            entry.roles(),
            before_handler.actor_handler.as_ref().unwrap().roles()
        );
        assert!(entry.validate(handler).is_ok());
        assert!(
            before_handler
                .actor_handler
                .as_ref()
                .unwrap()
                .validate(handler)
                .is_err()
        );
        assert_eq!(handler.params, before_handler.params);
        assert_eq!(handler.capture_count, before_handler.capture_count);
        assert_eq!(
            &handler.locals[..before_handler.locals.len()],
            before_handler.locals.as_slice()
        );
        assert_eq!(materialized_plans(&program).len(), 1);
        let mut required_left = 0;
        for function in &program.functions {
            let mut body = function.body.clone();
            crate::resource_materialization::walk_block(&mut body, &mut |expression| {
                required_left +=
                    usize::from(matches!(expression.kind, ExpressionKind::Comptime { .. }));
                Ok(())
            })
            .unwrap();
        }
        assert_eq!(required_left, 0);
        crate::complete_value_conversions(&mut program, &checked.checked().interner).unwrap();
        crate::validate(&program).unwrap();
        crate::validate_backend_types(&program, &checked.checked().interner).unwrap();
        assert!(
            crate::resource_materialization::functions_equal(
                program.resource_source.execution_functions(),
                &program.functions,
            ),
            "the actual sealed table must remain the exact authenticated MIR input"
        );
        // Runtime execution remains a separate genuine native gate.
    }
}

#[test]
fn equal_scalar_actor_occurrences_keep_distinct_exact_handlers_through_completion() {
    for release in [false, true] {
        let (mut program, checked, values) = setup(SCALAR, release);
        let proofs = values.checked_required_values(&checked).unwrap();
        assert_eq!(proofs.len(), 2);
        assert_ne!(proofs[0].owner(), proofs[1].owner());
        let source = program.resource_source.functions().to_vec();
        for owner in &program.functions {
            let mut body = owner.body.clone();
            crate::resource_materialization::walk_block(&mut body, &mut |expression| {
                if matches!(expression.kind, ExpressionKind::Comptime { .. }) {
                    assert_eq!(
                        proofs
                            .iter()
                            .filter(|proof| crate::resource_materialization::proof_matches(
                                proof,
                                owner,
                                expression,
                                &checked,
                                &program.resource_source
                            ))
                            .count(),
                        1
                    );
                }
                Ok(())
            })
            .unwrap();
        }
        bake(&mut program, &checked, &values).unwrap();
        crate::actor_handlers::validate_program(&program).unwrap();
        crate::complete_value_conversions(&mut program, &checked.checked().interner).unwrap();
        crate::validate(&program).unwrap();
        crate::validate_backend_types(&program, &checked.checked().interner).unwrap();
        assert!(crate::resource_materialization::functions_equal(
            &source,
            program.resource_source.functions()
        ));
    }
}

#[test]
fn required_constructor_and_handler_renew_snapshots_without_rebuilding_persistent_views() {
    for release in [false, true] {
        let (mut program, checked, values) = setup(VIEWS, release);
        let before = program.clone();
        let constructor = before
            .functions
            .iter()
            .find(|function| !function.actor_state_views.is_empty())
            .unwrap();
        assert_eq!(constructor.actor_state_views.len(), 2);
        let handler = before
            .functions
            .iter()
            .find(|function| function.actor_handler.is_some())
            .unwrap();
        let roles = handler.actor_handler.as_ref().unwrap().roles().to_vec();
        bake(&mut program, &checked, &values).unwrap();
        let after = &program.functions[constructor.id.index() as usize];
        for (old, new) in constructor
            .actor_state_views
            .iter()
            .zip(&after.actor_state_views)
        {
            assert!(old.matches_constructor(constructor));
            assert!(!old.matches_constructor(after));
            assert!(new.matches_constructor(after));
            assert_eq!(old.local(), new.local());
            assert_eq!(old.root(), new.root());
            assert_eq!(old.field(), new.field());
            assert_eq!(old.initializer(), new.initializer());
            new.validate(&after.locals, &checked.checked().interner)
                .unwrap();
        }
        let after_handler = &program.functions[handler.id.index() as usize];
        assert_eq!(
            after_handler.actor_handler.as_ref().unwrap().roles(),
            roles.as_slice()
        );
        crate::actor_handlers::validate_program(&program).unwrap();
        crate::complete_value_conversions(&mut program, &checked.checked().interner).unwrap();
        crate::validate(&program).unwrap();
        crate::validate_backend_types(&program, &checked.checked().interner).unwrap();
    }
}

#[test]
fn actual_required_bake_receipt_retains_tokens_cache_program_and_arena() {
    for release in [false, true] {
        let (before, checked, values) = setup(WITNESS, release);
        let types = &checked.checked().interner;
        let receipt = actual_bake(&before, &checked, &values, types);
        let output = receipt.candidate();
        let replay = receipt
            .replay(&before.clone(), &values.clone(), types)
            .unwrap();
        assert!(same_program(&output, &replay));
        assert_eq!(materialized_plans(&output), materialized_plans(&replay));
        assert!(!materialized_plans(&output).is_empty());
        let other = actual_bake(&before, &checked, &values, types).candidate();
        assert_ne!(materialized_plans(&output), materialized_plans(&other));
        assert!(receipt.authenticate_output(&before, &other, types).is_err());
        let other_cache = required(&checked);
        assert!(!other_cache.same_checked_cache(&values));
        assert!(receipt.replay(&before, &other_cache, types).is_err());
        let mut wrong_cache_output = output.clone();
        assert!(
            crate::actor_handlers::required_bake(
                &mut wrong_cache_output,
                &before,
                &other_cache,
                types,
                &receipt
            )
            .is_err()
        );
        assert!(same_program(&wrong_cache_output, &output));
        let (foreign, foreign_checked, _) = setup(WITNESS, release);
        let foreign_types = &foreign_checked.checked().interner;
        assert!(!std::ptr::eq(types, foreign_types));
        assert!(receipt.replay(&before, &values, foreign_types).is_err());
        assert!(receipt.replay(&foreign, &values, types).is_err());
        let proof = values.checked_required_values(&checked).unwrap().remove(0);
        let mut public_mirror = values.clone();
        public_mirror.insert(
            proof.source_span(),
            jett_comptime::ComptimeContext::default(),
            Value::Int64(99),
        );
        assert!(same_program(
            &receipt.replay(&before, &public_mirror, types).unwrap(),
            &output
        ));
    }
}

#[test]
fn actor_required_snapshot_seam_refuses_complete_input_and_output_edits_transactionally() {
    for release in [false, true] {
        let (before, checked, values) = setup(WITNESS, release);
        let types = &checked.checked().interner;
        let receipt = actual_bake(&before, &checked, &values, types);
        let output = receipt.candidate();
        for corruption in 0..8 {
            let mut changed = output.clone();
            let handler = changed
                .functions
                .iter()
                .position(|function| function.actor_handler.is_some())
                .unwrap();
            match corruption {
                0 => changed.functions[handler].body.span.end += 1,
                1 => changed.functions[handler].capture_count += 1,
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
                    let (foreign, _, _) = setup(WITNESS, release);
                    changed.functions[handler].actor_handler = foreign
                        .functions
                        .iter()
                        .find(|function| function.actor_handler.is_some())
                        .unwrap()
                        .actor_handler
                        .clone();
                }
                _ => unreachable!(),
            }
            let changed_before = changed.clone();
            assert!(
                crate::actor_handlers::required_bake(
                    &mut changed,
                    &before,
                    &values,
                    types,
                    &receipt
                )
                .is_err()
            );
            assert!(same_program(&changed, &changed_before));
        }
        let mut changed_before = before.clone();
        changed_before.functions[0].span.end += 1;
        let mut candidate = output.clone();
        assert!(
            crate::actor_handlers::required_bake(
                &mut candidate,
                &changed_before,
                &values,
                types,
                &receipt
            )
            .is_err()
        );
        assert!(same_program(&candidate, &output));
        let mut forged = ExplicitComptimeValues::default();
        let proof = values.checked_required_values(&checked).unwrap().remove(0);
        forged.insert(
            proof.source_span(),
            jett_comptime::ComptimeContext::default(),
            proof.value().clone(),
        );
        let mut unchanged = before.clone();
        assert!(bake(&mut unchanged, &checked, &forged).is_err());
        assert!(same_program(&unchanged, &before));
    }
}

#[test]
fn actor_required_materializer_rejects_forged_values_locals_and_late_failure_without_publication() {
    for release in [false, true] {
        let (before, checked, values) = setup(SCALAR, release);
        let types = &checked.checked().interner;
        let functions = function_value_candidates(
            &before.functions,
            &checked.checked().method_value_definitions,
        );
        for corruption in 0..5 {
            let mut program = before.clone();
            let mut calls = 0;
            let result = materialize_checked_reflected_values(
                &mut program,
                &values,
                types,
                |value, ty, span, locals, bindings| {
                    calls += 1;
                    if corruption == 4 && calls == 2 {
                        return Err("second checked replacement refused".into());
                    }
                    let mut output = value_expression(
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
                    )?;
                    match corruption {
                        0 => output.kind = ExpressionKind::Int(6),
                        1 => locals[0].mutable = !locals[0].mutable,
                        2 => {
                            let mut hidden = locals[0].clone();
                            hidden.id = LocalId::new(locals.len() as u32);
                            hidden.mutable = false;
                            hidden.view_source = None;
                            hidden.ty = ty;
                            hidden.debug_ty = ty;
                            let id = hidden.id;
                            locals.push(hidden);
                            bindings.push(Statement {
                                kind: StatementKind::Let {
                                    local: id,
                                    value: output.clone(),
                                },
                                span,
                            });
                        }
                        3 => output.span.end += 1,
                        4 => {}
                        _ => unreachable!(),
                    }
                    Ok(output)
                },
            );
            assert!(result.is_err(), "corruption {corruption}");
            assert!(same_program(&program, &before));
        }
    }
}

#[test]
fn required_float_actor_snapshots_preserve_nan_payloads_and_signed_zero_bits() {
    const SOURCE: &str = "namespace app
function make_nan() returns float64:
    return 0.0 / 0.0
actor Floating:
    receive nan_value responds float64:
        float64 baked = comptime make_nan()
        respond baked
    receive positive responds float64:
        float64 baked = comptime 0.0
        respond baked
    receive negative responds float64:
        float64 baked = comptime -0.0
        respond baked
function main() returns nothing:
    Floating worker = spawn Floating()
    float64 first = ask worker.nan_value
    float64 second = ask worker.positive
    float64 third = ask worker.negative
    return nothing
";
    for release in [false, true] {
        let (mut program, checked, values) = setup(SOURCE, release);
        bake(&mut program, &checked, &values).unwrap();
        crate::actor_handlers::validate_program(&program).unwrap();
        for (name, expected) in [
            ("Floating.nan_value", None),
            ("Floating.positive", Some(0.0f64.to_bits())),
            ("Floating.negative", Some((-0.0f64).to_bits())),
        ] {
            let handler = program
                .functions
                .iter()
                .find(|function| function.identity.declaration.name == name)
                .unwrap();
            let entry = handler.actor_handler.as_ref().unwrap();
            let mut body = handler.body.clone();
            let mut bits = Vec::new();
            crate::resource_materialization::walk_block(&mut body, &mut |expression| {
                if let ExpressionKind::Float(value) = expression.kind {
                    bits.push(value.to_bits());
                }
                Ok(())
            })
            .unwrap();
            assert_eq!(bits.len(), 1);
            if let Some(expected) = expected {
                assert_eq!(bits[0], expected);
            } else {
                assert!(f64::from_bits(bits[0]).is_nan());
            }
            assert!(entry.validate(handler).is_ok());
            let mut changed = handler.clone();
            crate::resource_materialization::walk_block(&mut changed.body, &mut |expression| {
                if let ExpressionKind::Float(value) = &mut expression.kind {
                    let mask = if value.is_nan() { 1 } else { 1u64 << 63 };
                    *value = f64::from_bits(value.to_bits() ^ mask);
                }
                Ok(())
            })
            .unwrap();
            assert!(
                entry.validate(&changed).is_err(),
                "exact {name} float bits must remain authenticated"
            );
        }
        crate::complete_value_conversions(&mut program, &checked.checked().interner).unwrap();
        crate::validate(&program).unwrap();
        crate::validate_backend_types(&program, &checked.checked().interner).unwrap();
    }
}
