use std::collections::HashMap;
use std::sync::Arc;

use jett_common::{FileId, SourceOrigin};
use jett_diagnostics::Severity;
use jett_parser::{ast::Item, parse};
use jett_resolve::{DefId, DefKind, ResourceKernelSpec};
use jett_typecheck::{CheckOptions, CheckedResourceProgram};
use jett_types::ResourceKernelRecipe;

use super::{ExecutionPurpose, ProviderEvent, ScriptOperation};
use crate::{Interpreter, Value};

const SUPPORT: &str = include_str!("fixtures/resource_probe.jett");

pub(crate) fn program(source: &str, release: bool) -> Arc<CheckedResourceProgram> {
    let stdlib_file = FileId::new(10_000);
    let project_file = FileId::new(0);
    let mut parsed = parse(SUPPORT, stdlib_file);
    let project = parse(source, project_file);
    assert!(
        !parsed
            .errors
            .iter()
            .chain(&project.errors)
            .any(|diagnostic| diagnostic.severity == Severity::Error),
        "Resource source parse prerequisite failed (release={release}): support diagnostics={:?}; project diagnostics={:?}",
        parsed.errors,
        project.errors,
    );
    let resource = parsed
        .module
        .items
        .iter()
        .find_map(|item| match item {
            Item::Resource(resource) => Some(resource.name.span),
            _ => None,
        })
        .expect("actual source Resource declaration");
    parsed.module.items.extend(project.module.items);
    parsed.errors.extend(project.errors);
    let catalog = [
        ("kernel_create", ResourceKernelRecipe::NetworkFactory),
        ("kernel_borrow", ResourceKernelRecipe::NetworkBorrow),
        ("kernel_close", ResourceKernelRecipe::Finalize),
    ]
    .into_iter()
    .map(|(member, recipe)| ResourceKernelSpec {
        resource_declaration: resource,
        member: member.to_string(),
        recipe,
    })
    .collect::<Vec<_>>();
    Arc::new(CheckedResourceProgram::prepare(parsed, HashMap::from([(stdlib_file, SourceOrigin::Stdlib),
        (project_file, SourceOrigin::Project)]), &catalog, CheckOptions { release }).unwrap_or_else(|error| {
            panic!("Resource source check prerequisite failed (release={release}): {error:?}; retained diagnostics={:?}",
                error.diagnostics());
        }))
}

fn entry(program: &CheckedResourceProgram, name: &str) -> DefId {
    let functions = program
        .module()
        .items
        .iter()
        .filter_map(|item| match item {
            Item::Function(function)
                if function.name.span.file == FileId::new(0) && function.name.name == name =>
            {
                Some(function)
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    let [function] = functions.as_slice() else {
        panic!(
            "primary source entry `{name}` must be unique; found {}",
            functions.len()
        );
    };
    let definitions = program
        .resolved()
        .scope_table
        .definitions
        .iter()
        .filter(|definition| {
            definition.kind == DefKind::Function
                && definition.span == function.name.span
                && definition.namespace.as_deref() == Some("app")
        })
        .map(|definition| definition.id)
        .collect::<Vec<_>>();
    let [definition] = definitions.as_slice() else {
        panic!(
            "primary source entry `{name}` has no unique exact ordinary declaration: {:?}",
            definitions
        );
    };
    *definition
}

fn lifecycle(source: &str, script: Vec<ScriptOperation>, expected: Vec<ProviderEvent>) {
    for release in [false, true] {
        let program = program(source, release);
        let target = entry(&program, "scenario");
        let mut interpreter =
            Interpreter::from_checked_resource_program(program, ExecutionPurpose::ReferenceRuntime)
                .unwrap();
        let grant = interpreter
            .install_resource_test_script(script.clone())
            .unwrap();
        assert_eq!(
            interpreter
                .call_checked_program_entry(target, vec![grant])
                .unwrap(),
            Value::Nothing
        );
        // Inspect before teardown. Registry::Drop is not the proof of source cleanup.
        assert_eq!(
            interpreter.resource_test_observations().unwrap(),
            (expected.clone(), 0, 0)
        );
        assert!(interpreter.take_debug_events().is_empty());
    }
}

#[test]
fn checked_source_constructs_borrows_moves_returns_and_closes_one_owner() {
    lifecycle(
        include_str!("fixtures/01_construct_borrow_move_close.jett"),
        vec![
            ScriptOperation::Construct {
                label: 101,
                outcome: Ok(()),
            },
            ScriptOperation::Borrow {
                label: 101,
                outcome: Ok(7),
            },
            ScriptOperation::Borrow {
                label: 101,
                outcome: Ok(8),
            },
        ],
        vec![
            ProviderEvent::Constructed(101),
            ProviderEvent::Borrowed(101),
            ProviderEvent::Borrowed(101),
            ProviderEvent::Finalized(101),
        ],
    );
}

#[test]
fn checked_source_scope_cleanup_is_reverse_holder_acquisition_before_teardown() {
    lifecycle(
        include_str!("fixtures/02_reverse_scope_drop.jett"),
        vec![
            ScriptOperation::Construct {
                label: 201,
                outcome: Ok(()),
            },
            ScriptOperation::Construct {
                label: 202,
                outcome: Ok(()),
            },
            ScriptOperation::Borrow {
                label: 201,
                outcome: Ok(1),
            },
            ScriptOperation::Borrow {
                label: 202,
                outcome: Ok(2),
            },
        ],
        vec![
            ProviderEvent::Constructed(201),
            ProviderEvent::Constructed(202),
            ProviderEvent::Borrowed(201),
            ProviderEvent::Borrowed(202),
            ProviderEvent::Finalized(202),
            ProviderEvent::Finalized(201),
        ],
    );
}

#[test]
fn checked_source_domain_failures_publish_no_extra_owner() {
    let source = include_str!("fixtures/03_domain_failures.jett");
    lifecycle(
        source,
        vec![ScriptOperation::Construct {
            label: 301,
            outcome: Err("denied".to_string()),
        }],
        vec![ProviderEvent::ConstructionFailed(301)],
    );
    lifecycle(
        source,
        vec![
            ScriptOperation::Construct {
                label: 301,
                outcome: Ok(()),
            },
            ScriptOperation::Borrow {
                label: 301,
                outcome: Err("unavailable".to_string()),
            },
        ],
        vec![
            ProviderEvent::Constructed(301),
            ProviderEvent::BorrowFailed(301),
            ProviderEvent::Finalized(301),
        ],
    );
}

#[test]
fn checked_source_bare_owned_actual_has_one_view_operation_backing() {
    lifecycle(
        include_str!("fixtures/04_bare_to_view_operation_cleanup.jett"),
        vec![
            ScriptOperation::Construct {
                label: 401,
                outcome: Ok(()),
            },
            ScriptOperation::Borrow {
                label: 401,
                outcome: Ok(4),
            },
        ],
        vec![
            ProviderEvent::Constructed(401),
            ProviderEvent::Borrowed(401),
            ProviderEvent::Finalized(401),
        ],
    );
}

#[test]
fn checked_source_occupied_result_moves_across_owned_calls_and_returns() {
    lifecycle(
        include_str!("fixtures/06_return_occupied_result.jett"),
        vec![
            ScriptOperation::Construct {
                label: 601,
                outcome: Ok(()),
            },
            ScriptOperation::Borrow {
                label: 601,
                outcome: Ok(6),
            },
        ],
        vec![
            ProviderEvent::Constructed(601),
            ProviderEvent::Borrowed(601),
            ProviderEvent::Finalized(601),
        ],
    );
}

#[test]
fn checked_source_indirect_hook_descriptor_does_not_finalize_until_invoked() {
    lifecycle(
        include_str!("fixtures/07_indirect_close.jett"),
        vec![ScriptOperation::Construct {
            label: 701,
            outcome: Ok(()),
        }],
        vec![
            ProviderEvent::Constructed(701),
            ProviderEvent::Finalized(701),
        ],
    );
}

#[test]
fn checked_source_capability_free_close_wrapper_still_requires_runtime_purpose() {
    lifecycle(
        include_str!("fixtures/09_capability_free_close_target.jett"),
        vec![ScriptOperation::Construct {
            label: 901,
            outcome: Ok(()),
        }],
        vec![
            ProviderEvent::Constructed(901),
            ProviderEvent::Finalized(901),
        ],
    );
}

#[test]
fn checked_required_purposes_allow_closed_descriptors_and_absence_without_provider() {
    let source = include_str!("fixtures/08_required_descriptor_and_absence.jett");
    for release in [false, true] {
        for purpose in [
            ExecutionPurpose::NamespaceConstant,
            ExecutionPurpose::ExplicitComptime,
            ExecutionPurpose::Verify,
            ExecutionPurpose::Property,
        ] {
            for name in ["descriptor_storage", "absent_close_path"] {
                let program = program(source, release);
                let target = entry(&program, name);
                let mut interpreter =
                    Interpreter::from_checked_resource_program(program, purpose).unwrap();
                assert!(
                    interpreter
                        .install_resource_test_script(Vec::new())
                        .is_err()
                );
                assert_eq!(
                    interpreter
                        .call_checked_program_entry(target, Vec::new())
                        .unwrap(),
                    Value::Nothing
                );
            }
        }
    }
}

#[test]
fn checked_source_later_actual_failure_preserves_lexical_borrow_and_skips_callee() {
    lifecycle(
        include_str!("fixtures/05_later_named_argument_failure_v2.jett"),
        vec![
            ScriptOperation::Construct {
                label: 501,
                outcome: Ok(()),
            },
            ScriptOperation::Borrow {
                label: 501,
                outcome: Ok(5),
            },
        ],
        vec![
            ProviderEvent::Constructed(501),
            ProviderEvent::Borrowed(501),
            ProviderEvent::Finalized(501),
        ],
    );
}

#[test]
fn checked_worker_apis_evaluate_closed_descriptors_and_property_trials_without_a_provider() {
    let source = include_str!("fixtures/08_required_descriptor_and_absence.jett");
    for release in [false, true] {
        let program = program(source, release);
        let types = Arc::new(crate::checked_types::CheckedExpressionTypes {
            resource_program: Some(program.clone()),
            ..Default::default()
        });
        let required = crate::evaluate_explicit_comptime_expressions_capture(
            program.module(),
            Arc::new(jett_types::ReflectionMetadata::new()),
            types.clone(),
            Arc::new(HashMap::new()),
        );
        assert!(
            required.diagnostics.is_empty(),
            "{:?}",
            required.diagnostics
        );
        assert!(required.debug_events.is_empty());
        let blocks = crate::run_verify_blocks_detailed_with_metadata_and_expression_types(
            program.module(),
            None,
            Some(types.clone()),
            None,
        );
        assert_eq!(blocks.len(), 2);
        assert!(
            blocks
                .iter()
                .all(|block| block.passed && block.debug_events.is_empty()),
            "{blocks:?}"
        );
        assert_eq!(blocks.iter().filter(|block| block.is_property).count(), 1);
        assert_eq!(
            blocks
                .iter()
                .find(|block| block.is_property)
                .unwrap()
                .iterations,
            Some(100)
        );
        let foreign = program.module().clone();
        let rejected = crate::run_verify_blocks_detailed_with_metadata_and_expression_types(
            &foreign,
            None,
            Some(types),
            None,
        );
        assert_eq!(rejected.len(), 1);
        assert!(!rejected[0].passed);
        assert!(
            rejected[0]
                .error
                .as_deref()
                .unwrap()
                .contains("foreign checked source module")
        );
    }
}

#[test]
fn raw_runtime_names_and_debug_configuration_do_not_grant_resource_dispatch() {
    let program = program(
        include_str!("fixtures/09_capability_free_close_target.jett"),
        false,
    );
    let mut interpreter =
        Interpreter::from_checked_resource_program(program, ExecutionPurpose::ReferenceRuntime)
            .unwrap();
    let grant = interpreter
        .install_resource_test_script(Vec::new())
        .unwrap();
    assert!(
        interpreter
            .call_function("app.scenario", vec![grant])
            .is_err()
    );
    assert_eq!(
        interpreter.resource_test_observations().unwrap(),
        (Vec::new(), 0, 0)
    );
    let mut raw = Interpreter::new_runtime();
    assert!(
        raw.call_function(
            "resource_probe.kernel_create",
            vec![Value::Capability("Network".to_string()), Value::Int64(1)]
        )
        .is_err()
    );
    assert!(raw.take_debug_events().is_empty());
}

#[test]
fn checked_source_bare_owned_view_provider_panic_unwinds_callee_before_parent_owner() {
    use std::panic::{AssertUnwindSafe, catch_unwind};
    for release in [false, true] {
        let checked = program(
            include_str!("fixtures/04_bare_to_view_operation_cleanup.jett"),
            release,
        );
        let target = entry(&checked, "scenario");
        let mut interpreter =
            Interpreter::from_checked_resource_program(checked, ExecutionPurpose::ReferenceRuntime)
                .unwrap();
        let grant = interpreter
            .install_resource_test_script(vec![
                ScriptOperation::Construct {
                    label: 401,
                    outcome: Ok(()),
                },
                ScriptOperation::BorrowPanic { label: 401 },
            ])
            .unwrap();
        assert!(
            catch_unwind(AssertUnwindSafe(
                || interpreter.call_checked_program_entry(target, vec![grant])
            ))
            .is_err()
        );
        assert_eq!(
            interpreter.resource_test_observations().unwrap(),
            (
                vec![
                    ProviderEvent::Constructed(401),
                    ProviderEvent::Borrowed(401),
                    ProviderEvent::Finalized(401)
                ],
                0,
                0
            )
        );
        assert!(interpreter.take_debug_events().is_empty());
    }
}

#[test]
fn checked_source_ordinary_closure_ignores_unreferenced_live_owner_and_grant() {
    for release in [false, true] {
        let checked = program(
            include_str!("fixtures/10_ordinary_closure_unreferenced_resource.jett"),
            release,
        );
        let target = entry(&checked, "scenario");
        let mut interpreter =
            Interpreter::from_checked_resource_program(checked, ExecutionPurpose::ReferenceRuntime)
                .unwrap();
        let grant = interpreter
            .install_resource_test_script(vec![ScriptOperation::Construct {
                label: 1001,
                outcome: Ok(()),
            }])
            .unwrap();
        assert_eq!(
            interpreter
                .call_checked_program_entry(target, vec![grant])
                .unwrap(),
            Value::Int64(7)
        );
        assert_eq!(
            interpreter.resource_test_observations().unwrap(),
            (
                vec![
                    ProviderEvent::Constructed(1001),
                    ProviderEvent::Finalized(1001)
                ],
                0,
                0
            )
        );
        assert!(interpreter.take_debug_events().is_empty());
    }
}

#[test]
fn checked_source_terminal_runtime_error_cleans_owner_before_teardown() {
    for release in [false, true] {
        let checked = program(
            include_str!("fixtures/11_terminal_assertion_cleanup.jett"),
            release,
        );
        let target = entry(&checked, "scenario");
        let mut interpreter =
            Interpreter::from_checked_resource_program(checked, ExecutionPurpose::ReferenceRuntime)
                .unwrap();
        let grant = interpreter
            .install_resource_test_script(vec![
                ScriptOperation::Construct {
                    label: 1101,
                    outcome: Ok(()),
                },
                ScriptOperation::Borrow {
                    label: 1101,
                    outcome: Ok(7),
                },
            ])
            .unwrap();
        let error = interpreter
            .call_checked_program_entry(target, vec![grant])
            .expect_err("the checked source kernel must return an ordinary terminal error");
        assert_eq!(error, "list.__remove_at: index -1 out of bounds");
        // This same interpreter is still alive: teardown cannot satisfy these counts.
        assert_eq!(
            interpreter.resource_test_observations().unwrap(),
            (
                vec![
                    ProviderEvent::Constructed(1101),
                    ProviderEvent::Borrowed(1101),
                    ProviderEvent::Finalized(1101)
                ],
                0,
                0,
            )
        );
        assert!(interpreter.take_debug_events().is_empty());
    }
}

fn mixed_failure_panic_message(payload: &(dyn std::any::Any + Send)) -> &str {
    if let Some(message) = payload.downcast_ref::<&str>() {
        return message;
    }
    payload
        .downcast_ref::<String>()
        .map(String::as_str)
        .expect("selected private fault has a textual panic payload")
}

#[test]
fn checked_source_cleanup_panic_overrides_provider_panic_after_real_view_cleanup() {
    use std::panic::{AssertUnwindSafe, catch_unwind};
    for release in [false, true] {
        for cleanup_panics in [false, true] {
            let checked = program(
                include_str!("fixtures/04_bare_to_view_operation_cleanup.jett"),
                release,
            );
            let target = entry(&checked, "scenario");
            let mut interpreter = Interpreter::from_checked_resource_program(
                checked,
                ExecutionPurpose::ReferenceRuntime,
            )
            .unwrap();
            let construction = if cleanup_panics {
                ScriptOperation::ConstructFinalizerPanic { label: 401 }
            } else {
                ScriptOperation::Construct {
                    label: 401,
                    outcome: Ok(()),
                }
            };
            let grant = interpreter
                .install_resource_test_script(vec![
                    construction,
                    ScriptOperation::BorrowPanic { label: 401 },
                ])
                .unwrap();
            let panic = catch_unwind(AssertUnwindSafe(|| {
                interpreter.call_checked_program_entry(target, vec![grant])
            }))
            .expect_err("private provider fault must unwind");
            let expected = if cleanup_panics {
                "selected test resource cleanup panic"
            } else {
                "selected test borrow provider panic"
            };
            assert_eq!(mixed_failure_panic_message(panic.as_ref()), expected);
            assert_eq!(
                interpreter.resource_test_observations().unwrap(),
                (
                    vec![
                        ProviderEvent::Constructed(401),
                        ProviderEvent::Borrowed(401),
                        ProviderEvent::Finalized(401)
                    ],
                    0,
                    0
                )
            );
            assert!(interpreter.take_debug_events().is_empty());
        }
    }
}

#[test]
fn checked_source_mixed_failure_continues_older_owner_cleanup_before_teardown() {
    use std::panic::{AssertUnwindSafe, catch_unwind};
    for release in [false, true] {
        let checked = program(
            include_str!("fixtures/12_mixed_failure_cleanup_continues.jett"),
            release,
        );
        let target = entry(&checked, "scenario");
        let mut interpreter =
            Interpreter::from_checked_resource_program(checked, ExecutionPurpose::ReferenceRuntime)
                .unwrap();
        let grant = interpreter
            .install_resource_test_script(vec![
                ScriptOperation::Construct {
                    label: 1201,
                    outcome: Ok(()),
                },
                ScriptOperation::ConstructFinalizerPanic { label: 1202 },
                ScriptOperation::Borrow {
                    label: 1201,
                    outcome: Ok(1),
                },
                ScriptOperation::BorrowPanic { label: 1202 },
            ])
            .unwrap();
        let panic = catch_unwind(AssertUnwindSafe(|| {
            interpreter.call_checked_program_entry(target, vec![grant])
        }))
        .expect_err("both selected private faults unwind");
        assert_eq!(
            mixed_failure_panic_message(panic.as_ref()),
            "selected test resource cleanup panic"
        );
        assert_eq!(
            interpreter.resource_test_observations().unwrap(),
            (
                vec![
                    ProviderEvent::Constructed(1201),
                    ProviderEvent::Constructed(1202),
                    ProviderEvent::Borrowed(1201),
                    ProviderEvent::Borrowed(1202),
                    ProviderEvent::Finalized(1202),
                    ProviderEvent::Finalized(1201)
                ],
                0,
                0
            )
        );
        assert!(interpreter.take_debug_events().is_empty());
    }
}

#[test]
fn checked_source_clean_cleanup_preserves_ordinary_protocol_error() {
    use std::panic::{AssertUnwindSafe, catch_unwind};
    for release in [false, true] {
        for cleanup_panics in [false, true] {
            let checked = program(
                include_str!("fixtures/04_bare_to_view_operation_cleanup.jett"),
                release,
            );
            let target = entry(&checked, "scenario");
            let mut interpreter = Interpreter::from_checked_resource_program(
                checked,
                ExecutionPurpose::ReferenceRuntime,
            )
            .unwrap();
            let construction = if cleanup_panics {
                ScriptOperation::ConstructFinalizerPanic { label: 401 }
            } else {
                ScriptOperation::Construct {
                    label: 401,
                    outcome: Ok(()),
                }
            };
            // Deliberately missing Borrow is a private harness protocol fault,
            // not a source result.fail or a newly permitted provider behavior.
            let grant = interpreter
                .install_resource_test_script(vec![construction])
                .unwrap();
            let outcome = catch_unwind(AssertUnwindSafe(|| {
                interpreter.call_checked_program_entry(target, vec![grant])
            }));
            if cleanup_panics {
                let panic =
                    outcome.expect_err("cleanup panic overrides an ordinary operation error");
                assert_eq!(
                    mixed_failure_panic_message(panic.as_ref()),
                    "selected test resource cleanup panic"
                );
            } else {
                assert_eq!(
                    outcome.expect("protocol error remains an ordinary Err"),
                    Err("resource execution has invalid checked call metadata".to_string())
                );
            }
            assert_eq!(
                interpreter.resource_test_observations().unwrap(),
                (
                    vec![
                        ProviderEvent::Constructed(401),
                        ProviderEvent::Finalized(401)
                    ],
                    0,
                    0
                )
            );
            assert!(interpreter.take_debug_events().is_empty());
        }
    }
}

#[path = "prerequisite_tests.rs"]
mod frontend_prerequisites;

#[test]
fn checked_direct_and_nested_type_scopes_execute_original_lifecycle_and_restore_caller() {
    lifecycle(
        include_str!("fixtures/13_direct_nested_scoped_lifecycle.jett"),
        vec![
            ScriptOperation::Construct {
                label: 1301,
                outcome: Ok(()),
            },
            ScriptOperation::Borrow {
                label: 1301,
                outcome: Ok(1),
            },
            ScriptOperation::Construct {
                label: 1302,
                outcome: Ok(()),
            },
            ScriptOperation::Borrow {
                label: 1302,
                outcome: Ok(2),
            },
            ScriptOperation::Construct {
                label: 1304,
                outcome: Ok(()),
            },
            ScriptOperation::Construct {
                label: 1303,
                outcome: Ok(()),
            },
        ],
        vec![
            ProviderEvent::Constructed(1301),
            ProviderEvent::Borrowed(1301),
            ProviderEvent::Finalized(1301),
            ProviderEvent::Constructed(1302),
            ProviderEvent::Borrowed(1302),
            ProviderEvent::Finalized(1302),
            ProviderEvent::Constructed(1304),
            ProviderEvent::Finalized(1304),
            ProviderEvent::Constructed(1303),
            ProviderEvent::Finalized(1303),
        ],
    );
}

#[test]
fn checked_direct_scopes_keep_named_worker_descriptor_and_absence_entries_provider_free() {
    let source = include_str!("fixtures/14_direct_scoped_descriptor_absence.jett");
    for release in [false, true] {
        for purpose in [
            ExecutionPurpose::NamespaceConstant,
            ExecutionPurpose::ExplicitComptime,
            ExecutionPurpose::Verify,
            ExecutionPurpose::Property,
        ] {
            for name in ["descriptor_storage", "absent_close_path"] {
                let program = program(source, release);
                let target = entry(&program, name);
                let mut interpreter =
                    Interpreter::from_checked_resource_program(program.clone(), purpose).unwrap();
                interpreter
                    .authorize_checked_resource_worker(program.module(), purpose)
                    .unwrap();
                let context = interpreter.resource_test_entry_context().unwrap();
                assert!(
                    interpreter
                        .install_resource_test_script(Vec::new())
                        .is_err()
                );
                assert_eq!(
                    interpreter
                        .call_checked_program_entry(target, Vec::new())
                        .unwrap(),
                    Value::Nothing
                );
                assert_eq!(interpreter.resource_test_custody_counts().unwrap(), (0, 0));
                assert_eq!(interpreter.resource_test_entry_context().unwrap(), context);
                assert!(interpreter.take_debug_events().is_empty());
            }
        }
    }
    // This is a checked named-entry test. Collected required expressions and
    // callable/cache body references remain a separate mandatory later layer.
}

#[test]
fn checked_direct_scope_error_restores_typed_and_legacy_context_before_reentry() {
    let source = include_str!("fixtures/15_direct_scoped_ordinary_error.jett");
    for release in [false, true] {
        let program = program(source, release);
        let target = entry(&program, "scenario");
        let mut interpreter =
            Interpreter::from_checked_resource_program(program, ExecutionPurpose::ReferenceRuntime)
                .unwrap();
        let grant = interpreter
            .install_resource_test_script(vec![
                ScriptOperation::Construct {
                    label: 1501,
                    outcome: Ok(()),
                },
                ScriptOperation::Borrow {
                    label: 1501,
                    outcome: Ok(1),
                },
                ScriptOperation::Construct {
                    label: 1501,
                    outcome: Ok(()),
                },
                ScriptOperation::Borrow {
                    label: 1501,
                    outcome: Ok(2),
                },
            ])
            .unwrap();
        for attempt in 0..2 {
            let error = interpreter
                .call_checked_program_entry(target, vec![grant.clone()])
                .unwrap_err();
            assert_eq!(error, "list.__remove_at: index -1 out of bounds");
            let mut expected = vec![
                ProviderEvent::Constructed(1501),
                ProviderEvent::Borrowed(1501),
                ProviderEvent::Finalized(1501),
            ];
            if attempt == 1 {
                expected.extend(expected.clone());
            }
            assert_eq!(
                interpreter.resource_test_observations().unwrap(),
                (expected, 0, 0)
            );
            assert_eq!(
                interpreter.resource_test_scoped_context().unwrap(),
                (false, 0, 0, 0, None)
            );
            assert!(interpreter.take_debug_events().is_empty());
        }
    }
}

#[test]
fn checked_direct_scopes_restore_after_return_and_default_with_live_owner() {
    let source = include_str!("fixtures/16_direct_scoped_return_default.jett");
    for release in [false, true] {
        for outcome in [Ok(19), Err("unavailable".to_string())] {
            let program = program(source, release);
            let target = entry(&program, "scenario");
            let mut interpreter = Interpreter::from_checked_resource_program(
                program,
                ExecutionPurpose::ReferenceRuntime,
            )
            .unwrap();
            let grant = interpreter
                .install_resource_test_script(vec![
                    ScriptOperation::Construct {
                        label: 1601,
                        outcome: Ok(()),
                    },
                    ScriptOperation::Borrow {
                        label: 1601,
                        outcome: outcome.clone(),
                    },
                ])
                .unwrap();
            assert_eq!(
                interpreter
                    .call_checked_program_entry(target, vec![grant])
                    .unwrap(),
                Value::Int64(if outcome.is_ok() { 19 } else { 17 })
            );
            assert_eq!(
                interpreter.resource_test_observations().unwrap(),
                (
                    vec![
                        ProviderEvent::Constructed(1601),
                        if outcome.is_ok() {
                            ProviderEvent::Borrowed(1601)
                        } else {
                            ProviderEvent::BorrowFailed(1601)
                        },
                        ProviderEvent::Finalized(1601)
                    ],
                    0,
                    0
                )
            );
            assert_eq!(
                interpreter.resource_test_scoped_context().unwrap(),
                (false, 0, 0, 0, None)
            );
            assert!(interpreter.take_debug_events().is_empty());
        }
    }
}

#[test]
fn checked_entries_reuse_the_same_provider_after_clean_and_ordinary_error_completion() {
    let source = include_str!("fixtures/25_repeated_scoped_entry.jett");
    for release in [false, true] {
        for first_errors in [false, true] {
            let program = program(source, release);
            let clean = entry(&program, "clean");
            let first = if first_errors {
                entry(&program, "terminal_error")
            } else {
                clean
            };
            let mut interpreter = Interpreter::from_checked_resource_program(
                program,
                ExecutionPurpose::ReferenceRuntime,
            )
            .unwrap();
            let first_label = if first_errors { 1802 } else { 1801 };
            let grant = interpreter
                .install_resource_test_script(vec![
                    ScriptOperation::Construct {
                        label: first_label,
                        outcome: Ok(()),
                    },
                    ScriptOperation::Borrow {
                        label: first_label,
                        outcome: Ok(11),
                    },
                    ScriptOperation::Construct {
                        label: 1801,
                        outcome: Ok(()),
                    },
                    ScriptOperation::Borrow {
                        label: 1801,
                        outcome: Ok(12),
                    },
                ])
                .unwrap();
            let context = interpreter.resource_test_entry_context().unwrap();
            let first_result = interpreter.call_checked_program_entry(first, vec![grant.clone()]);
            if first_errors {
                assert_eq!(
                    first_result,
                    Err("list.__remove_at: index -1 out of bounds".to_string())
                );
            } else {
                assert_eq!(first_result.unwrap(), Value::Int64(11));
            }
            let mut events = vec![
                ProviderEvent::Constructed(first_label),
                ProviderEvent::Borrowed(first_label),
                ProviderEvent::Finalized(first_label),
            ];
            assert_eq!(
                interpreter.resource_test_observations().unwrap(),
                (events.clone(), 0, 0)
            );
            assert_eq!(interpreter.resource_test_entry_context().unwrap(), context);
            assert_eq!(
                interpreter
                    .call_checked_program_entry(clean, vec![grant])
                    .unwrap(),
                Value::Int64(12)
            );
            events.extend([
                ProviderEvent::Constructed(1801),
                ProviderEvent::Borrowed(1801),
                ProviderEvent::Finalized(1801),
            ]);
            assert_eq!(
                interpreter.resource_test_observations().unwrap(),
                (events, 0, 0)
            );
            assert_eq!(interpreter.resource_test_entry_context().unwrap(), context);
            assert!(interpreter.take_debug_events().is_empty());
        }
    }
}

#[test]
fn checked_entries_restore_after_provider_or_finalizer_panic_before_same_grant_reentry() {
    use std::panic::{AssertUnwindSafe, catch_unwind};
    let source = include_str!("fixtures/25_repeated_scoped_entry.jett");
    for release in [false, true] {
        for cleanup_panics in [false, true] {
            let program = program(source, release);
            let target = entry(&program, "clean");
            let mut interpreter = Interpreter::from_checked_resource_program(
                program,
                ExecutionPurpose::ReferenceRuntime,
            )
            .unwrap();
            let grant = interpreter
                .install_resource_test_script(vec![
                    if cleanup_panics {
                        ScriptOperation::ConstructFinalizerPanic { label: 1801 }
                    } else {
                        ScriptOperation::Construct {
                            label: 1801,
                            outcome: Ok(()),
                        }
                    },
                    if cleanup_panics {
                        ScriptOperation::Borrow {
                            label: 1801,
                            outcome: Ok(7),
                        }
                    } else {
                        ScriptOperation::BorrowPanic { label: 1801 }
                    },
                    ScriptOperation::Construct {
                        label: 1801,
                        outcome: Ok(()),
                    },
                    ScriptOperation::Borrow {
                        label: 1801,
                        outcome: Ok(19),
                    },
                ])
                .unwrap();
            let context = interpreter.resource_test_entry_context().unwrap();
            let panic = catch_unwind(AssertUnwindSafe(|| {
                interpreter.call_checked_program_entry(target, vec![grant.clone()])
            }))
            .expect_err("private scripted host panic remains observable");
            assert_eq!(
                mixed_failure_panic_message(panic.as_ref()),
                if cleanup_panics {
                    "selected test resource cleanup panic"
                } else {
                    "selected test borrow provider panic"
                }
            );
            let mut events = vec![
                ProviderEvent::Constructed(1801),
                ProviderEvent::Borrowed(1801),
                ProviderEvent::Finalized(1801),
            ];
            assert_eq!(
                interpreter.resource_test_observations().unwrap(),
                (events.clone(), 0, 0)
            );
            assert_eq!(interpreter.resource_test_entry_context().unwrap(), context);
            assert_eq!(
                interpreter
                    .call_checked_program_entry(target, vec![grant])
                    .unwrap(),
                Value::Int64(19)
            );
            events.extend([
                ProviderEvent::Constructed(1801),
                ProviderEvent::Borrowed(1801),
                ProviderEvent::Finalized(1801),
            ]);
            assert_eq!(
                interpreter.resource_test_observations().unwrap(),
                (events, 0, 0)
            );
            assert_eq!(interpreter.resource_test_entry_context().unwrap(), context);
            assert!(interpreter.take_debug_events().is_empty());
        }
    }
}

pub(crate) fn same_resource_hook_descriptor_identity(
    left: &super::ResourceHookDescriptor,
    right: &super::ResourceHookDescriptor,
) -> bool {
    Arc::ptr_eq(&left.program, &right.program) && left.definition == right.definition
}

#[test]
fn closed_descriptor_mirror_identity_preserves_function_inequality_and_program_boundary() {
    use super::CheckedExecution;
    use jett_types::ResourceHookKind;
    const SOURCE: &str = include_str!("fixtures/27_original_generic_scoped_required.jett");
    for release in [false, true] {
        let checked_program = program(SOURCE, release);
        let checked =
            CheckedExecution::new(checked_program.clone(), ExecutionPurpose::ReferenceRuntime)
                .unwrap();
        let hook = |kind| {
            checked_program
                .checked()
                .resource_hooks
                .values()
                .find(|hook| hook.kind == kind)
                .unwrap()
                .definition
        };
        let closed = checked.descriptor(hook(ResourceHookKind::Close)).unwrap();
        assert!(same_resource_hook_descriptor_identity(
            &closed,
            &closed.clone()
        ));
        assert!(!same_resource_hook_descriptor_identity(
            &closed,
            &checked
                .descriptor(hook(ResourceHookKind::BorrowOperation))
                .unwrap()
        ));
        let foreign =
            CheckedExecution::new(program(SOURCE, release), ExecutionPurpose::ReferenceRuntime)
                .unwrap();
        assert!(!same_resource_hook_descriptor_identity(
            &closed,
            &foreign.descriptor(hook(ResourceHookKind::Close)).unwrap()
        ));
        assert_ne!(
            Value::ResourceHook(closed.clone()),
            Value::ResourceHook(closed)
        );
    }
}

#[path = "assignment_tests.rs"]
mod mutable_assignment;
