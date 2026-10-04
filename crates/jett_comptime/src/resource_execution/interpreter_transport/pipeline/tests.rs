use jett_common::FileId;
use jett_parser::ast::Item;
use jett_resolve::{DefId, DefKind};

use super::*;
use crate::resource_execution::tests::program;
use crate::resource_execution::{ProviderEvent, ScriptOperation};

const CONSTRUCT: &str = include_str!("../../fixtures/18_pipeline_construct_move_close.jett");
const RETAIN: &str = include_str!("../../fixtures/19_pipeline_written_view_retains_owner.jett");
const RELINQUISH: &str = include_str!("../../fixtures/20_pipeline_bare_owner_view_operation.jett");
const ABORT: &str = include_str!("../../fixtures/21_pipeline_named_actual_abort.jett");
const DOMAIN: &str = include_str!("../../fixtures/22_pipeline_domain_failure_stops_next_step.jett");
const INDIRECT: &str = include_str!("../../fixtures/23_pipeline_indirect_close_descriptor.jett");

fn entry(checked: &jett_typecheck::CheckedResourceProgram) -> DefId {
    let functions = checked
        .module()
        .items
        .iter()
        .filter_map(|item| match item {
            Item::Function(function)
                if function.name.span.file == FileId::new(0)
                    && function.name.name == "scenario" =>
            {
                Some(function)
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    let [function] = functions.as_slice() else {
        panic!("one original primary scenario");
    };
    let definitions = checked
        .resolved()
        .scope_table
        .definitions
        .iter()
        .filter(|info| {
            info.kind == DefKind::Function
                && info.span == function.name.span
                && info.namespace.as_deref() == Some("app")
        })
        .map(|info| info.id)
        .collect::<Vec<_>>();
    let [definition] = definitions.as_slice() else {
        panic!("one exact original declaration");
    };
    *definition
}

fn execute(
    source: &str,
    script: Vec<ScriptOperation>,
    returned: Value,
    events: Vec<ProviderEvent>,
) {
    for release in [false, true] {
        let checked = program(source, release);
        let target = entry(&checked);
        let mut interpreter =
            Interpreter::from_checked_resource_program(checked, ExecutionPurpose::ReferenceRuntime)
                .unwrap();
        let grant = interpreter
            .install_resource_test_script(script.clone())
            .unwrap();
        assert_eq!(
            interpreter
                .call_checked_program_entry(target, vec![grant])
                .unwrap(),
            returned,
            "release={release}"
        );
        assert_eq!(
            interpreter.resource_test_observations().unwrap(),
            (events.clone(), 0, 0)
        );
        assert!(interpreter.take_debug_events().is_empty());
    }
}

#[test]
fn resource_pipeline_constructs_moves_and_closes_one_source_owner() {
    execute(
        CONSTRUCT,
        vec![ScriptOperation::Construct {
            label: 1801,
            outcome: Ok(()),
        }],
        Value::Nothing,
        vec![
            ProviderEvent::Constructed(1801),
            ProviderEvent::Finalized(1801),
        ],
    );
}

#[test]
fn resource_pipeline_written_view_keeps_root_for_later_explicit_close() {
    execute(
        RETAIN,
        vec![
            ScriptOperation::Construct {
                label: 1901,
                outcome: Ok(()),
            },
            ScriptOperation::Borrow {
                label: 1901,
                outcome: Ok(5),
            },
        ],
        Value::Int64(5),
        vec![
            ProviderEvent::Constructed(1901),
            ProviderEvent::Borrowed(1901),
            ProviderEvent::Finalized(1901),
        ],
    );
}

#[test]
fn resource_pipeline_bare_owner_backs_view_only_until_operation_cleanup() {
    execute(
        RELINQUISH,
        vec![
            ScriptOperation::Construct {
                label: 2001,
                outcome: Ok(()),
            },
            ScriptOperation::Borrow {
                label: 2001,
                outcome: Ok(5),
            },
        ],
        Value::Int64(5),
        vec![
            ProviderEvent::Constructed(2001),
            ProviderEvent::Borrowed(2001),
            ProviderEvent::Finalized(2001),
        ],
    );
}

#[test]
fn resource_pipeline_input_precedes_named_actual_abort_and_callee_never_enters() {
    execute(
        ABORT,
        vec![
            ScriptOperation::Construct {
                label: 2101,
                outcome: Ok(()),
            },
            ScriptOperation::Borrow {
                label: 2101,
                outcome: Ok(1),
            },
        ],
        Value::Nothing,
        vec![
            ProviderEvent::Constructed(2101),
            ProviderEvent::Borrowed(2101),
            ProviderEvent::Finalized(2101),
        ],
    );
}

#[test]
fn resource_pipeline_domain_failure_and_borrow_failure_keep_exact_source_handlers() {
    execute(
        DOMAIN,
        vec![ScriptOperation::Construct {
            label: 2201,
            outcome: Err("denied".into()),
        }],
        Value::Int64(-1),
        vec![ProviderEvent::ConstructionFailed(2201)],
    );
    execute(
        RETAIN,
        vec![
            ScriptOperation::Construct {
                label: 1901,
                outcome: Ok(()),
            },
            ScriptOperation::Borrow {
                label: 1901,
                outcome: Err("unavailable".into()),
            },
        ],
        Value::Int64(-2),
        vec![
            ProviderEvent::Constructed(1901),
            ProviderEvent::BorrowFailed(1901),
            ProviderEvent::Finalized(1901),
        ],
    );
}

#[test]
fn resource_pipeline_indirect_hook_descriptor_executes_only_at_original_step() {
    execute(
        INDIRECT,
        vec![ScriptOperation::Construct {
            label: 2301,
            outcome: Ok(()),
        }],
        Value::Nothing,
        vec![
            ProviderEvent::Constructed(2301),
            ProviderEvent::Finalized(2301),
        ],
    );
}

#[test]
fn resource_pipeline_provider_panic_unwinds_real_call_backing_before_teardown() {
    use std::panic::{AssertUnwindSafe, catch_unwind};
    for release in [false, true] {
        let checked = program(RELINQUISH, release);
        let target = entry(&checked);
        let mut interpreter =
            Interpreter::from_checked_resource_program(checked, ExecutionPurpose::ReferenceRuntime)
                .unwrap();
        let grant = interpreter
            .install_resource_test_script(vec![
                ScriptOperation::Construct {
                    label: 2001,
                    outcome: Ok(()),
                },
                ScriptOperation::BorrowPanic { label: 2001 },
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
                    ProviderEvent::Constructed(2001),
                    ProviderEvent::Borrowed(2001),
                    ProviderEvent::Finalized(2001)
                ],
                0,
                0
            )
        );
        assert!(interpreter.take_debug_events().is_empty());
    }
}
#[test]
fn resource_pipeline_required_regions_execute_absence_without_runtime_authority() {
    use crate::checked_types::CheckedExpressionTypes;
    use crate::resource_execution::CheckedExecution;
    use jett_parser::ast::Item;

    const REQUIRED: &str = include_str!("../../fixtures/28_pipeline_absence_regions.jett");
    for release in [false, true] {
        let program = program(REQUIRED, release);
        let expression_types = std::sync::Arc::new(CheckedExpressionTypes {
            resource_program: Some(program.clone()),
            expressions: program
                .checked()
                .type_map
                .iter()
                .map(|(span, ty)| (*span, program.checked().interner.type_name(*ty)))
                .collect(),
            ..Default::default()
        });
        let required = crate::evaluate_explicit_comptime_expressions_capture(
            program.module(),
            std::sync::Arc::new(jett_types::ReflectionMetadata::new()),
            expression_types.clone(),
            std::sync::Arc::new(std::collections::HashMap::new()),
        );
        assert!(
            required.diagnostics.is_empty(),
            "{:?}",
            required.diagnostics
        );
        assert!(required.debug_events.is_empty());
        assert!(required.values.checked_values_are_mirrored());
        let declaration = program
            .module()
            .items
            .iter()
            .find_map(|item| match item {
                Item::VarDecl(value)
                    if value.name.name == "namespace_marker"
                        && value.name.span.file == jett_common::FileId::new(0) =>
                {
                    Some(value)
                }
                _ => None,
            })
            .unwrap();
        assert_eq!(
            required.values.constant(declaration.name.span),
            Some(&Value::Int64(17))
        );
        let suites = crate::verify::run_verify_blocks_detailed_with_checked_values(
            program.module(),
            std::sync::Arc::new(jett_types::ReflectionMetadata::new()),
            expression_types,
            std::sync::Arc::new(std::collections::HashMap::new()),
            std::sync::Arc::new(required.values.clone()),
        );
        assert_eq!(suites.len(), 2);
        assert!(
            suites
                .iter()
                .all(|suite| suite.passed && suite.debug_events.is_empty()),
            "{suites:?}"
        );
        let original = program
            .module()
            .items
            .iter()
            .find_map(|item| match item {
                Item::Function(function)
                    if function.name.name == "explicit_marker"
                        && function.name.span.file == jett_common::FileId::new(0) =>
                {
                    Some(function)
                }
                _ => None,
            })
            .unwrap();
        let checked =
            CheckedExecution::new(program.clone(), ExecutionPurpose::ReferenceRuntime).unwrap();
        let target = checked.declaration_definition(original.name.span).unwrap();
        let mut interpreter =
            Interpreter::from_checked_resource_program(program, ExecutionPurpose::ReferenceRuntime)
                .unwrap();
        interpreter.set_explicit_comptime_values(std::sync::Arc::new(required.values));
        assert_eq!(
            interpreter
                .call_checked_program_entry(target, Vec::new())
                .unwrap(),
            Value::Int64(17)
        );
        assert_eq!(interpreter.resource_test_custody_counts().unwrap(), (0, 0));
        assert!(interpreter.take_debug_events().is_empty());
    }
}
