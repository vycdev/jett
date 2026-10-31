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

pub(super) fn original_then_ready(first: &cases::Case, ready: &cases::Case, release: bool) {
    original_then_ready_with_support(first, ready, release, cases::SUPPORT);
}

pub(super) fn original_then_ready_with_support(
    first: &cases::Case,
    ready: &cases::Case,
    release: bool,
    support: &str,
) {
    assert_eq!(first.source.as_bytes(), ready.source.as_bytes());
    let checked = checked_case_with_support(first, release, support);
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
