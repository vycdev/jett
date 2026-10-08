//! Genuine reference counterpart to the shared native original-then-READY matrix.
use super::*;

fn first_outcome(
    actual: std::thread::Result<Result<Value, String>>,
    case: &cases::Case,
    release: bool,
) {
    match case.reference {
        cases::ReferenceOutcome::Clean => {
            assert_eq!(
                actual.unwrap().unwrap(),
                Value::Nothing,
                "{} release={release}",
                case.name
            );
        }
        cases::ReferenceOutcome::Error(expected) => {
            assert_eq!(
                actual.unwrap().unwrap_err(),
                expected,
                "{} release={release}",
                case.name
            );
        }
        cases::ReferenceOutcome::ProviderPanic(expected)
        | cases::ReferenceOutcome::CleanupPanic(expected) => {
            let payload = actual.expect_err("the selected Source panic stays observable");
            let message = payload
                .downcast_ref::<String>()
                .map(String::as_str)
                .or_else(|| payload.downcast_ref::<&str>().copied())
                .expect("the scripted provider/finalizer panic has a string payload");
            assert_eq!(message, expected, "{} release={release}", case.name);
        }
    }
}

fn empty_before_teardown(interpreter: &mut Interpreter, expected: &[ProviderEvent]) {
    assert_eq!(
        interpreter.resource_test_observations().unwrap(),
        (expected.to_vec(), 0, 0)
    );
    assert_eq!(interpreter.resource_test_custody_counts().unwrap(), (0, 0));
    assert!(interpreter.take_debug_events().is_empty());
}

fn original_then_ready(first: &cases::Case, ready: &cases::Case, release: bool) {
    assert_eq!(first.source.as_bytes(), ready.source.as_bytes());
    let checked = checked_case(first, release);
    let definition = entry(&checked, "main");
    let mut interpreter = reference_with_required_values(&checked, first, release);
    let combined = first
        .script
        .iter()
        .chain(ready.script.iter())
        .copied()
        .map(script)
        .collect();
    let grant = interpreter.install_resource_test_script(combined).unwrap();
    let context = interpreter.resource_test_entry_context().unwrap();
    let actual = catch_unwind(AssertUnwindSafe(|| {
        interpreter.call_checked_program_entry(definition, vec![grant.clone()])
    }));
    first_outcome(actual, first, release);
    let mut expected = first.events.iter().map(event).collect::<Vec<_>>();
    assert_eq!(interpreter.resource_test_entry_context().unwrap(), context);
    empty_before_teardown(&mut interpreter, &expected);
    assert_eq!(
        interpreter
            .call_checked_program_entry(definition, vec![grant])
            .unwrap(),
        Value::Nothing,
        "{} release={release}: same-grant READY",
        first.name
    );
    expected.extend(ready.events.iter().map(event));
    assert_eq!(interpreter.resource_test_entry_context().unwrap(), context);
    empty_before_teardown(&mut interpreter, &expected);
}

#[test]
fn reflected_field_source06_native_reentry_inputs_match_real_reference() {
    let ready = &reflected_field_cases::CASES[0];
    assert_eq!(ready.name, "reflected_field_original");
    assert_eq!(reflected_field_cases::SOURCE06_EXCEPTIONAL_CASES.len(), 8);
    for release in [false, true] {
        let first_scripts = std::iter::once(ready)
            .chain(reflected_field_cases::SOURCE06_EXCEPTIONAL_CASES.iter())
            .collect::<Vec<_>>();
        assert_eq!(first_scripts.len(), 9);
        for first in first_scripts {
            original_then_ready(first, ready, release);
        }
    }
}
