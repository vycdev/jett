//! Recursive reflected calls restore each caller's field selector and alias.

use super::*;
use crate::{Interpreter, Value};
use jett_common::FileId;
use jett_parser::ast::{FunctionDef, Item, Stmt};
use jett_types::ReflectionMetadata;
use std::sync::Arc;

const RECURSIVE_SOURCE: &str = include_str!("recursive_labels.jett");
const BASE: &str = concat!(
    "first:int64<>first:int64|",
    "second:int64<>second:int64|",
    "named:app.Count<>named:app.Count|",
);
const DEPTH_ONE: &str = concat!(
    "first:int64<",
    "first:int64<>first:int64|second:int64<>second:int64|named:app.Count<>named:app.Count|",
    ">first:int64|",
    "second:int64<",
    "first:int64<>first:int64|second:int64<>second:int64|named:app.Count<>named:app.Count|",
    ">second:int64|",
    "named:app.Count<",
    "first:int64<>first:int64|second:int64<>second:int64|named:app.Count<>named:app.Count|",
    ">named:app.Count|",
);

fn depth_two_expected() -> String {
    // A fixed golden composition, not a second recursive evaluator. Every
    // outer field must observe its own ordinal and alias after the same Source
    // function executes all three inner field scopes twice more in depth.
    format!(
        "first:int64<{DEPTH_ONE}>first:int64|second:int64<{DEPTH_ONE}>second:int64|named:app.Count<{DEPTH_ONE}>named:app.Count|"
    )
}

fn original_function<'a>(program: &'a CheckedResourceProgram, name: &str) -> &'a FunctionDef {
    program
        .module()
        .items
        .iter()
        .find_map(|item| match item {
            Item::Function(function)
                if function.name.span.file == FileId::new(0) && function.name.name == name =>
            {
                Some(function)
            }
            _ => None,
        })
        .expect("one original project function")
}

fn assert_no_resource_effects(interpreter: &mut Interpreter) {
    assert_eq!(
        interpreter.resource_test_observations(),
        Err(ResourceExecutionError::ProviderDisabled.to_string())
    );
    assert_eq!(interpreter.resource_test_custody_counts().unwrap(), (0, 0));
    assert!(interpreter.take_debug_events().is_empty());
}

#[test]
fn same_source_body_recursion_restores_each_callers_field_ordinal_and_alias() {
    for release in [false, true] {
        let program = crate::resource_execution::tests::program(RECURSIVE_SOURCE, release);
        let checked =
            CheckedExecution::new(program.clone(), ExecutionPurpose::ReferenceRuntime).unwrap();
        let original = original_function(&program, "recursive_labels");
        let definition = checked.declaration_definition(original.name.span).unwrap();
        let mut interpreter =
            Interpreter::from_checked_resource_program(program, ExecutionPurpose::ReferenceRuntime)
                .unwrap();
        interpreter.set_reflection_metadata(Arc::new(ReflectionMetadata::new()));
        let baseline = interpreter.resource_test_entry_context().unwrap();
        assert_no_resource_effects(&mut interpreter);
        for (depth, expected) in [
            (0, BASE.to_string()),
            (1, DEPTH_ONE.to_string()),
            (2, depth_two_expected()),
            (1, DEPTH_ONE.to_string()),
            (0, BASE.to_string()),
        ] {
            assert_eq!(
                interpreter
                    .call_checked_program_entry(definition, vec![Value::Int64(depth)])
                    .unwrap(),
                Value::String(expected),
                "depth={depth}, release={release}"
            );
            // Frozen observation includes checked reflected selector depth,
            // checked body/source state, metadata stacks and lexical scopes.
            assert_eq!(interpreter.resource_test_entry_context().unwrap(), baseline);
            assert_no_resource_effects(&mut interpreter);
        }
    }
}

#[test]
fn closed_pure_same_body_recursion_is_selected_by_required_evaluation() {
    let source = format!(
        "{RECURSIVE_SOURCE}{}",
        r#"
export function required_recursive_labels() returns string:
    return comptime recursive_labels(2)
"#
    );
    for release in [false, true] {
        let program = crate::resource_execution::tests::program(&source, release);
        let types = Arc::new(crate::checked_types::CheckedExpressionTypes {
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
            Arc::new(ReflectionMetadata::new()),
            types,
            Arc::new(std::collections::HashMap::new()),
        );
        assert!(
            required.diagnostics.is_empty(),
            "release={release}: {:?}",
            required.diagnostics
        );
        assert!(required.debug_events.is_empty());
        let expected = Value::String(depth_two_expected());
        assert_eq!(required.values.len(), 1);
        assert_eq!(
            required.values.values().cloned().collect::<Vec<_>>(),
            vec![expected.clone()]
        );
        assert!(required.values.checked_values_are_mirrored());
        assert!(
            required
                .values
                .checked_resource_hook_values(&program)
                .unwrap()
                .is_empty()
        );
        let proofs = required.values.checked_required_values(&program).unwrap();
        let [proof] = proofs.as_slice() else {
            panic!("one authenticated required recursion value");
        };
        let original = original_function(&program, "required_recursive_labels");
        let [Stmt::Return(statement)] = original.body.stmts.as_slice() else {
            panic!("one original explicit comptime return");
        };
        assert!(proof.matches_original_comptime(statement.value.as_ref().unwrap()));
        assert!(proof.belongs_to(&program));
        assert!(proof.is_explicit_comptime());
        assert_eq!(proof.value(), &expected);
        assert!(proof.checked_hook(&program).unwrap().is_none());
        // Required capture creates its own worker without a provider. Its API
        // does not expose provider observations or the worker's final snapshot.
    }
}
