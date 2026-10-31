//! Distinct original-constructor/worker gate under the existing absence tests.
//! Reuse its twelve exact typed Value goldens, rather than the new main guards.
use super::*;
use std::panic::{AssertUnwindSafe, catch_unwind};
#[path = "../../../../../jett_driver/tests/native_conformance/resource_absent_aggregate_baseline_inputs.rs"]
mod inputs;

fn original_calls(
    checked: &Arc<CheckedResourceProgram>,
    purpose: ExecutionPurpose,
    release: bool,
    entered: &mut Vec<inputs::Phase>,
) -> Result<(), inputs::Failure> {
    use inputs::{Phase, enter, failure};
    let mut failures = Vec::new();
    let mut calls = 0;
    for (name, expected) in absent_values() {
        enter(entered, Phase::WorkerEligibility);
        let setup = inputs::attempt(|stages| {
            enter(stages, Phase::WorkerEligibility);
            let mut interpreter =
                Interpreter::from_checked_resource_program(checked.clone(), purpose)
                    .map_err(|error| failure(Phase::WorkerEligibility, error))?;
            let installation = interpreter.install_resource_test_script(Vec::new());
            if (purpose == ExecutionPurpose::ReferenceRuntime) != installation.is_ok() {
                return Err(failure(Phase::WorkerEligibility, installation));
            }
            let baseline = interpreter
                .resource_test_entry_context()
                .map_err(|error| failure(Phase::WorkerEligibility, error))?;
            enter(stages, Phase::OriginalConstructorCall);
            let target = source_entry(checked, name);
            let actual = catch_unwind(AssertUnwindSafe(|| {
                interpreter.call_checked_program_entry(target, Vec::new())
            }));
            let actual_text = match &actual {
                Ok(result) => format!("{result:?}"),
                Err(payload) => format!(
                    "UnexpectedPanic({:?})",
                    inputs::panic_message(payload.as_ref())
                ),
            };
            let call_outcome = match actual {
                Ok(Ok(actual)) if actual == expected => Ok(()),
                Ok(actual) => Err(failure(Phase::OriginalConstructorCall, (actual, expected))),
                Err(payload) => Err(inputs::Failure {
                    phase: Phase::OriginalConstructorCall,
                    kind: inputs::FailureKind::UnexpectedPanic,
                    detail: inputs::panic_message(payload.as_ref()),
                }),
            };
            enter(stages, Phase::ReferenceObservations);
            let effects = interpreter.resource_test_observations();
            let custody = interpreter.resource_test_custody_counts();
            let context = interpreter.resource_test_entry_context();
            let debug = interpreter.take_debug_events();
            eprintln!(
                "ABSENT_ORIGINAL name={name:?} purpose={purpose:?} release={release} observed={actual_text:?} effects={effects:?} custody={custody:?} context={context:?} debug={debug:?}"
            );
            let effects_match = if purpose == ExecutionPurpose::ReferenceRuntime {
                effects == Ok((Vec::new(), 0, 0))
            } else {
                effects == Err(ResourceExecutionError::ProviderDisabled.to_string())
            };
            if !effects_match
                || custody != Ok((0, 0))
                || context.as_ref() != Ok(&baseline)
                || !debug.is_empty()
            {
                if let Err(mut first) = call_outcome {
                    first.detail.push_str("; constructor effects/custody/context/debug invariant also failed; see actual log");
                    return Err(first);
                }
                return Err(failure(
                    Phase::ReferenceObservations,
                    "constructor effects/custody/context/debug changed; see log",
                ));
            }
            call_outcome
        });
        calls += 1;
        if let Err(error) = setup.outcome {
            eprintln!(
                "ABSENT_ORIGINAL_FAILURE name={name:?} purpose={purpose:?} release={release} entered={:?} failure={error:?}",
                setup.entered
            );
            failures.push((name, error));
        }
    }
    eprintln!(
        "ABSENT_ORIGINAL_TOTAL purpose={purpose:?} release={release} calls={calls} failed={}",
        failures.len()
    );
    if calls != 12 || !failures.is_empty() {
        if let Some((_, first)) = failures.first() {
            return Err(inputs::Failure {
                phase: first.phase,
                kind: first.kind,
                detail: format!("{calls} original constructor attempts; failures={failures:#?}"),
            });
        }
        return Err(failure(Phase::OriginalConstructorCall, calls));
    }
    Ok(())
}

#[test]
fn source17_augmented_original_constructors_keep_exact_values_in_all_five_purposes() {
    use inputs::{Phase, failure};
    let input = inputs::INPUTS.last().unwrap(); // Same entire prefix, simple required main wrapper.
    let mut attempted = 0;
    let mut failures = Vec::new();
    for release in [false, true] {
        for purpose in [
            ExecutionPurpose::ReferenceRuntime,
            ExecutionPurpose::NamespaceConstant,
            ExecutionPurpose::ExplicitComptime,
            ExecutionPurpose::Verify,
            ExecutionPurpose::Property,
        ] {
            attempted += 1;
            let attempt = inputs::attempt(|entered| {
                let (checked, _) = inputs::prepare(input, release, entered)?;
                original_calls(&checked, purpose, release, entered)
            });
            if let Err(error) = attempt.outcome {
                eprintln!(
                    "ABSENT_PURPOSE_FAILURE purpose={purpose:?} release={release} entered={:?} failure={error:?}",
                    attempt.entered
                );
                failures.push((purpose, release, error));
            }
        }
    }
    assert_eq!(attempted, 10);
    assert!(
        failures.is_empty(),
        "{}",
        failure(Phase::OriginalConstructorCall, failures).detail
    );
    // Ten accepted preparations imply 120 separately logged exact typed calls.
}

#[test]
fn source17_augmented_required_capture_and_original_verify_property100_are_separate() {
    use inputs::{Phase, enter, failure};
    let input = inputs::INPUTS.last().unwrap();
    let mut failures = Vec::new();
    let mut attempts = 0;
    for release in [false, true] {
        attempts += 1;
        let attempt = inputs::attempt(|entered| {
            let (checked, _) = inputs::prepare(input, release, entered)?;
            let types = Arc::new(crate::checked_types::CheckedExpressionTypes {
                resource_program: Some(checked.clone()),
                expressions: checked
                    .checked()
                    .type_map
                    .iter()
                    .map(|(span, ty)| (*span, checked.checked().interner.type_name(*ty)))
                    .collect(),
                ..Default::default()
            });
            enter(entered, Phase::RequiredCapture);
            let required = crate::evaluate_explicit_comptime_expressions_capture(
                checked.module(),
                Arc::new(jett_types::ReflectionMetadata::new()),
                types.clone(),
                Arc::new(HashMap::new()),
            );
            eprintln!(
                "ABSENT_REQUIRED release={release} diagnostics={:?} debug={:?} values={:?}",
                required.diagnostics,
                required.debug_events,
                required.values.values().collect::<Vec<_>>()
            );
            if !required.diagnostics.is_empty() || !required.debug_events.is_empty() {
                return Err(failure(
                    Phase::RequiredCapture,
                    (&required.diagnostics, &required.debug_events),
                ));
            }
            enter(entered, Phase::RequiredCache);
            required
                .values
                .checked_required_values(&checked)
                .map_err(|error| failure(Phase::RequiredCache, error))?;
            let declarations = checked
                .module()
                .items
                .iter()
                .filter_map(|item| match item {
                    Item::VarDecl(declaration)
                        if declaration.name.span.file == FileId::new(0)
                            && declaration.name.name == "absent_namespace_value" =>
                    {
                        Some(declaration.name.span)
                    }
                    _ => None,
                })
                .collect::<Vec<_>>();
            let [declaration] = declarations.as_slice() else {
                return Err(failure(
                    Phase::RequiredCache,
                    "missing original namespace constant",
                ));
            };
            if required.values.constant(*declaration) != Some(&Value::Int64(17))
                || !required.values.checked_values_are_mirrored()
                || required
                    .values
                    .values()
                    .any(Value::contains_live_resource_or_grant)
            {
                return Err(failure(
                    Phase::RequiredCache,
                    "constant/mirror/authority contract failed",
                ));
            }
            enter(entered, Phase::VerifyPropertyWorkers);
            let blocks = crate::run_verify_blocks_detailed_with_metadata_and_expression_types(
                checked.module(),
                None,
                Some(types),
                None,
            );
            eprintln!("ABSENT_VERIFY_PROPERTY release={release} blocks={blocks:#?}");
            if blocks.len() != 2
                || !blocks
                    .iter()
                    .all(|block| block.passed && block.debug_events.is_empty())
                || blocks.iter().filter(|block| block.is_property).count() != 1
                || blocks
                    .iter()
                    .find(|block| block.is_property)
                    .unwrap()
                    .iterations
                    != Some(100)
            {
                return Err(failure(Phase::VerifyPropertyWorkers, blocks));
            }
            Ok(())
        });
        if let Err(error) = attempt.outcome {
            eprintln!(
                "ABSENT_WORKERS_FAILURE release={release} entered={:?} failure={error:?}",
                attempt.entered
            );
            failures.push((release, error));
        }
    }
    assert_eq!(attempts, 2);
    assert!(
        failures.is_empty(),
        "separate required/verify/property failure: {failures:#?}"
    );
    // This reference-worker result is not native suite execution evidence.
}
