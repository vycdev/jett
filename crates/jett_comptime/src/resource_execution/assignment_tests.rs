use super::*;
use std::panic::{AssertUnwindSafe, catch_unwind};

const SOURCE: &str = include_str!("fixtures/24_mutable_assignment.jett");

fn run(
    name: &str,
    script: Vec<ScriptOperation>,
    expected: Vec<ProviderEvent>,
    panic_expected: bool,
) {
    for release in [false, true] {
        let program = program(SOURCE, release);
        let definition = entry(&program, name);
        let mut interpreter =
            Interpreter::from_checked_resource_program(program, ExecutionPurpose::ReferenceRuntime)
                .unwrap();
        let grant = interpreter
            .install_resource_test_script(script.clone())
            .unwrap();
        let result = catch_unwind(AssertUnwindSafe(|| {
            interpreter.call_checked_program_entry(definition, vec![grant])
        }));
        if panic_expected {
            assert!(
                result.is_err(),
                "the old-owner finalizer panic must propagate"
            );
        } else {
            assert_eq!(result.unwrap().unwrap(), Value::Nothing);
        }
        assert_eq!(
            interpreter.resource_test_observations().unwrap(),
            (expected.clone(), 0, 0)
        );
        assert!(interpreter.take_debug_events().is_empty());
    }
}

fn constructed(label: i64) -> ScriptOperation {
    ScriptOperation::Construct {
        label,
        outcome: Ok(()),
    }
}

#[test]
fn mutable_resource_replacement_finalizes_old_before_using_new() {
    run(
        "replace_live",
        vec![
            constructed(2401),
            constructed(2402),
            ScriptOperation::Borrow {
                label: 2402,
                outcome: Ok(7),
            },
        ],
        vec![
            ProviderEvent::Constructed(2401),
            ProviderEvent::Constructed(2402),
            ProviderEvent::Finalized(2401),
            ProviderEvent::Borrowed(2402),
            ProviderEvent::Finalized(2402),
        ],
        false,
    );
}

#[test]
fn mutable_resource_can_rebind_its_original_consumed_slot() {
    run(
        "rebind_after_close",
        vec![constructed(2411), constructed(2412)],
        vec![
            ProviderEvent::Constructed(2411),
            ProviderEvent::Finalized(2411),
            ProviderEvent::Constructed(2412),
            ProviderEvent::Finalized(2412),
        ],
        false,
    );
}

#[test]
fn mutable_resource_self_move_retains_exactly_one_cleanup_owner() {
    run(
        "rebind_self",
        vec![constructed(2421)],
        vec![
            ProviderEvent::Constructed(2421),
            ProviderEvent::Finalized(2421),
        ],
        false,
    );
}

#[test]
fn mutable_resource_failed_rhs_handler_can_borrow_previous_owner() {
    run(
        "failed_rhs_keeps_owner",
        vec![
            constructed(2431),
            ScriptOperation::Construct {
                label: 2432,
                outcome: Err("rhs sentinel".to_string()),
            },
            ScriptOperation::Borrow {
                label: 2431,
                outcome: Ok(7),
            },
        ],
        vec![
            ProviderEvent::Constructed(2431),
            ProviderEvent::ConstructionFailed(2432),
            ProviderEvent::Borrowed(2431),
            ProviderEvent::Finalized(2431),
        ],
        false,
    );
}

#[test]
fn mutable_resource_old_finalizer_panic_also_cleans_staged_replacement() {
    run(
        "replace_live",
        vec![
            ScriptOperation::ConstructFinalizerPanic { label: 2401 },
            constructed(2402),
        ],
        vec![
            ProviderEvent::Constructed(2401),
            ProviderEvent::Constructed(2402),
            ProviderEvent::Finalized(2401),
            ProviderEvent::Finalized(2402),
        ],
        true,
    );
}
