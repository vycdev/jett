//! Strict new-wrapper reference baseline; existing twelve typed returns are separate.
use super::*;
#[path = "../../../../../jett_driver/tests/native_conformance/resource_absent_aggregate_baseline_inputs.rs"]
mod inputs;

fn runtime(
    checked: &Arc<CheckedResourceProgram>,
    entered: &mut Vec<inputs::Phase>,
) -> Result<Interpreter, inputs::Failure> {
    use inputs::{Phase, enter, failure};
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
    let metadata = Arc::new(jett_types::ReflectionMetadata::new());
    let exclusions = Arc::new(HashMap::new());
    enter(entered, Phase::RequiredCapture);
    let required = crate::evaluate_explicit_comptime_expressions_capture(
        checked.module(),
        metadata.clone(),
        types.clone(),
        exclusions.clone(),
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
        .checked_required_values(checked)
        .map_err(|error| failure(Phase::RequiredCache, error))?;
    if !required.values.checked_values_are_mirrored()
        || required
            .values
            .values()
            .any(Value::contains_live_resource_or_grant)
    {
        return Err(failure(
            Phase::RequiredCache,
            "unmirrored or runtime-authority required value",
        ));
    }
    let constants = checked
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
    let [constant] = constants.as_slice() else {
        return Err(failure(
            Phase::RequiredCache,
            "original namespace constant is not unique",
        ));
    };
    if required.values.constant(*constant) != Some(&Value::Int64(17)) {
        return Err(failure(
            Phase::RequiredCache,
            required.values.constant(*constant),
        ));
    }
    enter(entered, Phase::ReferenceSetup);
    let mut interpreter = Interpreter::new();
    interpreter.set_reflection_metadata(metadata);
    interpreter.set_checked_expression_types(types);
    interpreter.set_breakpoint_exclusions(exclusions);
    interpreter.set_explicit_comptime_values(Arc::new(required.values));
    // Baked values precede registration; no namespace initializer fallback.
    interpreter.register_module(checked.module());
    interpreter
        .install_checked_resource_program(checked.clone(), ExecutionPurpose::ReferenceRuntime)
        .map_err(|error| failure(Phase::ReferenceSetup, error))?;
    Ok(interpreter)
}

fn execute(
    input: &inputs::Input,
    release: bool,
    entered: &mut Vec<inputs::Phase>,
) -> Result<(), inputs::Failure> {
    use inputs::{Phase, enter, failure};
    let (checked, span) = inputs::prepare(input, release, entered)?;
    let mut interpreter = runtime(&checked, entered)?;
    enter(entered, Phase::ReferenceEntryIdentity);
    let functions = checked
        .module()
        .items
        .iter()
        .filter_map(|item| match item {
            Item::Function(function) if function.span == span => Some(function),
            _ => None,
        })
        .collect::<Vec<_>>();
    let [function] = functions.as_slice() else {
        return Err(failure(
            Phase::ReferenceEntryIdentity,
            "original primary main body missing",
        ));
    };
    let definitions = checked
        .resolved()
        .scope_table
        .definitions
        .iter()
        .filter(|definition| {
            definition.kind == jett_resolve::DefKind::Function
                && definition.span == function.name.span
                && definition.namespace.as_deref() == Some("app")
        })
        .map(|definition| definition.id)
        .collect::<Vec<_>>();
    let [definition] = definitions.as_slice() else {
        return Err(failure(
            Phase::ReferenceEntryIdentity,
            "original main has no unique checked definition",
        ));
    };
    let grant = interpreter
        .install_resource_test_script(Vec::new())
        .map_err(|error| failure(Phase::ReferenceSetup, error))?;
    let baseline = interpreter
        .resource_test_entry_context()
        .map_err(|error| failure(Phase::ReferenceSetup, error))?;
    enter(entered, Phase::ReferenceCall);
    // Retain the interpreter for observations even if the Source panics.
    let actual = catch_unwind(AssertUnwindSafe(|| {
        interpreter.call_checked_program_entry(*definition, vec![grant])
    }));
    let actual_text = match &actual {
        Ok(result) => format!("{result:?}"),
        Err(payload) => format!(
            "UnexpectedPanic({:?})",
            inputs::panic_message(payload.as_ref())
        ),
    };
    let call_outcome = match actual {
        Ok(Ok(Value::Nothing))
            if matches!(input.reference, inputs::ReferenceExpectation::Nothing) =>
        {
            Ok(())
        }
        Ok(Err(actual)) => match input.reference {
            inputs::ReferenceExpectation::Error(expected) if actual == expected => Ok(()),
            _ => Err(failure(Phase::ReferenceCall, actual)),
        },
        Ok(other) => Err(failure(Phase::ReferenceCall, other)),
        Err(payload) => Err(inputs::Failure {
            phase: Phase::ReferenceCall,
            kind: inputs::FailureKind::UnexpectedPanic,
            detail: inputs::panic_message(payload.as_ref()),
        }),
    };
    enter(entered, Phase::ReferenceObservations);
    let observations = interpreter.resource_test_observations();
    let custody = interpreter.resource_test_custody_counts();
    let context = interpreter.resource_test_entry_context();
    let debug = interpreter.take_debug_events();
    eprintln!(
        "ABSENT_REFERENCE name={:?} release={release} observed={actual_text:?} observations={observations:?} custody={custody:?} context={context:?} debug={debug:?}",
        input.name
    );
    if observations != Ok((Vec::new(), 0, 0))
        || custody != Ok((0, 0))
        || context.as_ref() != Ok(&baseline)
        || !debug.is_empty()
    {
        if let Err(mut first) = call_outcome {
            first.detail.push_str(
                "; exact empty effects/custody/context/debug invariant also failed; see actual log",
            );
            return Err(first);
        }
        return Err(failure(
            Phase::ReferenceObservations,
            "exact empty effects/custody/context/debug invariant failed; see actual log",
        ));
    }
    call_outcome
}

#[test]
fn native_resource_absent_aggregate_new_wrappers_record_all_reference_outcomes() {
    assert_eq!(inputs::INPUTS.len(), 14);
    let mut attempts = 0;
    let mut matched = 0;
    let mut failures = Vec::new();
    for release in [false, true] {
        for input in inputs::INPUTS {
            attempts += 1;
            let attempt = inputs::attempt(|entered| execute(input, release, entered));
            match attempt.outcome {
                Ok(()) => {
                    matched += 1;
                    eprintln!(
                        "ABSENT_REFERENCE_MATCH name={:?} release={release} entered={:?}",
                        input.name, attempt.entered
                    );
                }
                Err(failure) => {
                    eprintln!(
                        "ABSENT_REFERENCE_FAILURE name={:?} release={release} entered={:?} failure={failure:?}",
                        input.name, attempt.entered
                    );
                    failures.push((input.name, release, failure));
                }
            }
        }
    }
    eprintln!(
        "ABSENT_REFERENCE_TOTAL attempts={attempts} matched={matched} failed={}",
        failures.len()
    );
    assert_eq!(attempts, 28);
    assert!(
        failures.is_empty(),
        "strict new-wrapper reference failures: {failures:#?}"
    );
    assert_eq!(matched, 28);
}
