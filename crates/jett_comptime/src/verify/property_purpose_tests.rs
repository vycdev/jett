use super::*;
use crate::ExecutionPurpose;
use jett_common::SourceOrigin;
use jett_typecheck::{CheckOptions, CheckedResourceProgram};
use std::cell::Cell;

const SOURCE: &str =
    "namespace app\nproperty original_purpose:\n    given sample: bool\n    assert true\n";

fn program(release: bool, with_hooks: bool) -> Arc<CheckedResourceProgram> {
    if with_hooks {
        return crate::resource_execution::tests::program(SOURCE, release);
    }
    let file = FileId::new(0);
    let checked = CheckedResourceProgram::prepare(
        jett_parser::parse(SOURCE, file),
        HashMap::from([(file, SourceOrigin::Project)]),
        &[],
        CheckOptions { release },
    )
    .expect("genuine ordinary checked property");
    assert!(checked.checked().resource_hooks.is_empty());
    Arc::new(checked)
}

fn property(checked: &CheckedResourceProgram) -> &PropertyBlock {
    checked
        .module()
        .items
        .iter()
        .find_map(|item| match item {
            Item::Property(property) => Some(property),
            _ => None,
        })
        .expect("physical original property")
}

fn assert_restored_and_reenter(
    interpreter: &mut Interpreter,
    checked: &Arc<CheckedResourceProgram>,
    prior: ExecutionPurpose,
) {
    assert_eq!(interpreter.checked_execution_purpose(prior), Some(prior));
    interpreter
        .authorize_checked_resource_worker(checked.module(), prior)
        .expect("same retained program and purpose restored");
    assert!(interpreter.take_debug_events().is_empty());
    let result = run_property_block(
        interpreter,
        Some("app"),
        property(checked),
        PropertyDefinitions {
            enums: &[],
            structs: &[],
            bitfields: &[],
            aliases: &[],
        },
        None,
    );
    assert!(result.passed, "{:?}", result.error);
    assert_eq!(result.iterations, Some(PROPERTY_DEFAULT_ITERATIONS));
    assert!(result.debug_events.is_empty());
    assert_eq!(interpreter.checked_execution_purpose(prior), Some(prior));
    interpreter
        .authorize_checked_resource_worker(checked.module(), prior)
        .unwrap();
}

#[test]
fn property_execution_purpose_restores_error_and_original_checked_identity() {
    for release in [false, true] {
        for with_hooks in [false, true] {
            for prior in [
                ExecutionPurpose::Verify,
                ExecutionPurpose::NamespaceConstant,
            ] {
                let checked = program(release, with_hooks);
                let mut interpreter =
                    Interpreter::from_checked_resource_program(checked.clone(), prior).unwrap();
                interpreter
                    .authorize_checked_resource_worker(checked.module(), prior)
                    .unwrap();
                let attempts = Cell::new(0);
                let result = with_property_execution_purpose(&mut interpreter, |interpreter| {
                    assert_eq!(
                        interpreter.checked_execution_purpose(ExecutionPurpose::Property),
                        Some(ExecutionPurpose::Property)
                    );
                    interpreter.with_checked_property_region(
                        property(&checked),
                        Some("app"),
                        |_| -> Result<(), String> {
                            attempts.set(attempts.get() + 1);
                            Err("original property body error".to_owned())
                        },
                    )
                });
                assert_eq!(result, Err("original property body error".to_owned()));
                assert_eq!(attempts.get(), 1);
                assert_restored_and_reenter(&mut interpreter, &checked, prior);
            }
        }
    }
}

#[derive(Debug)]
struct PanicMarker(u64);

#[test]
fn property_execution_purpose_restores_original_panic_box_before_reentry() {
    for release in [false, true] {
        for with_hooks in [false, true] {
            for prior in [
                ExecutionPurpose::Verify,
                ExecutionPurpose::NamespaceConstant,
            ] {
                let checked = program(release, with_hooks);
                let mut interpreter =
                    Interpreter::from_checked_resource_program(checked.clone(), prior).unwrap();
                interpreter
                    .authorize_checked_resource_worker(checked.module(), prior)
                    .unwrap();
                let attempts = Cell::new(0);
                let payload = Box::new(PanicMarker(73));
                let identity = std::ptr::from_ref(payload.as_ref());
                let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    with_property_execution_purpose(&mut interpreter, |interpreter| {
                        interpreter.with_checked_property_region(
                            property(&checked),
                            Some("app"),
                            |_| -> Result<(), String> {
                                attempts.set(attempts.get() + 1);
                                std::panic::resume_unwind(payload)
                            },
                        )
                    })
                }));
                let payload = caught.err().expect("original panic must propagate");
                let payload = payload
                    .downcast::<PanicMarker>()
                    .unwrap_or_else(|_| panic!("panic payload type changed"));
                assert_eq!(payload.0, 73);
                assert!(std::ptr::eq(identity, std::ptr::from_ref(payload.as_ref())));
                assert_eq!(attempts.get(), 1);
                assert_restored_and_reenter(&mut interpreter, &checked, prior);
            }
        }
    }
}

#[derive(Clone, Copy)]
enum Refusal {
    ClonedBlock,
    WrongNamespace,
}

#[test]
fn property_execution_purpose_refuses_foreign_association_before_effects() {
    for release in [false, true] {
        for with_hooks in [false, true] {
            for refusal in [Refusal::ClonedBlock, Refusal::WrongNamespace] {
                let checked = program(release, with_hooks);
                let prior = ExecutionPurpose::Verify;
                let mut interpreter =
                    Interpreter::from_checked_resource_program(checked.clone(), prior).unwrap();
                interpreter
                    .authorize_checked_resource_worker(checked.module(), prior)
                    .unwrap();
                let original = property(&checked);
                let cloned = original.clone();
                let (selected, namespace) = match refusal {
                    Refusal::ClonedBlock => (&cloned, Some("app")),
                    Refusal::WrongNamespace => (original, Some("other")),
                };
                let attempts = Cell::new(0);
                let result = with_property_execution_purpose(&mut interpreter, |interpreter| {
                    interpreter.with_checked_property_region(selected, namespace, |_| {
                        attempts.set(attempts.get() + 1);
                        Ok(())
                    })
                });
                assert_eq!(
                    result.as_ref().map_err(String::as_str),
                    Err("checked test capture has no exact original body/purpose/namespace")
                );
                assert_eq!(attempts.get(), 0);
                assert_restored_and_reenter(&mut interpreter, &checked, prior);
            }
        }
    }
}

#[test]
fn property_execution_purpose_keeps_metadata_free_error_and_panic_unchecked() {
    let mut interpreter = Interpreter::new();
    assert_eq!(
        interpreter.checked_execution_purpose(ExecutionPurpose::Verify),
        None
    );
    let error = with_property_execution_purpose(&mut interpreter, |interpreter| {
        assert_eq!(
            interpreter.checked_execution_purpose(ExecutionPurpose::Property),
            None
        );
        Err::<(), _>("ordinary error")
    });
    assert_eq!(error, Err("ordinary error"));
    assert_eq!(
        interpreter.checked_execution_purpose(ExecutionPurpose::Verify),
        None
    );
    let payload = Box::new(PanicMarker(91));
    let identity = std::ptr::from_ref(payload.as_ref());
    let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        with_property_execution_purpose(&mut interpreter, |_| -> () {
            std::panic::resume_unwind(payload)
        })
    }));
    let payload = caught
        .err()
        .expect("raw panic remains a panic")
        .downcast::<PanicMarker>()
        .unwrap_or_else(|_| panic!("raw panic payload changed"));
    assert_eq!(payload.0, 91);
    assert!(std::ptr::eq(identity, std::ptr::from_ref(payload.as_ref())));
    assert_eq!(
        interpreter.checked_execution_purpose(ExecutionPurpose::Verify),
        None
    );
    assert!(interpreter.take_debug_events().is_empty());
}
