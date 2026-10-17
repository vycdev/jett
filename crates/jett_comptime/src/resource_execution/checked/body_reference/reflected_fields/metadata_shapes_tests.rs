//! Checked field types for heterogeneous, bitfield and generic owners.

use super::*;
use crate::{Interpreter, Value};
use jett_common::FileId;
use jett_parser::ast::{ComptimeTypeBindStmt, ForStmt, FunctionDef, Item, Stmt};
use jett_types::{ReflectionMetadata, Type, TypeInterner};
use std::sync::Arc;

const SHAPES_SOURCE: &str = include_str!("metadata_shapes.jett");

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

fn original_loop(function: &FunctionDef) -> &ForStmt {
    function
        .body
        .stmts
        .iter()
        .find_map(|statement| match statement {
            Stmt::For(statement) => Some(statement),
            _ => None,
        })
        .expect("one original direct field loop")
}

fn original_binding(statement: &ForStmt) -> &ComptimeTypeBindStmt {
    statement
        .body
        .stmts
        .iter()
        .find_map(|statement| match statement {
            Stmt::ComptimeTypeBind(binding) => Some(binding),
            _ => None,
        })
        .expect("original field.type_info binding")
}

#[test]
fn heterogeneous_bitfield_and_concrete_generic_owners_select_original_ordinal_types() {
    for release in [false, true] {
        let program = crate::resource_execution::tests::program(SHAPES_SOURCE, release);
        let mut checked =
            CheckedExecution::new(program.clone(), ExecutionPurpose::ReferenceRuntime).unwrap();
        for (name, is_bitfield, expected) in [
            (
                "heterogeneous_fields",
                false,
                vec![
                    TypeInterner::STRING,
                    TypeInterner::INT64,
                    TypeInterner::BOOL,
                ],
            ),
            ("bitfield_fields", true, vec![TypeInterner::UINT64]),
            (
                "generic_fields",
                false,
                vec![TypeInterner::INT64, TypeInterner::STRING],
            ),
        ] {
            let function = original_function(&program, name);
            let definition = checked.declaration_definition(function.name.span).unwrap();
            let parent = checked
                .prepare_function_body(&checked.entry(definition).unwrap())
                .unwrap();
            checked.install_function_body(&parent).unwrap();
            let before = checked.attempt_key(&checked.cursor()).unwrap();
            let source_loop = original_loop(function);
            let binding = original_binding(source_loop);
            let prepared = checked
                .prepare_reflected_field_loop(source_loop)
                .unwrap()
                .unwrap();
            assert_eq!(checked.attempt_key(&checked.cursor()).unwrap(), before);
            assert_eq!(
                prepared
                    .fields
                    .iter()
                    .map(|field| field.0)
                    .collect::<Vec<_>>(),
                expected
            );
            assert_eq!(
                matches!(
                    program.checked().interner.resolve(prepared.owner),
                    Type::Bitfield(_)
                ),
                is_bitfield
            );
            if !is_bitfield {
                assert!(matches!(
                    program.checked().interner.resolve(prepared.owner),
                    Type::Struct(_)
                ));
            }
            for index in 0..prepared.fields.len() {
                let scope = checked
                    .prepare_reflected_field_scope(binding, &prepared.iteration(index).unwrap())
                    .unwrap();
                assert_eq!(checked.attempt_key(&checked.cursor()).unwrap(), before);
                assert!(std::ptr::eq(scope.body().unwrap(), &binding.body));
                assert_eq!(scope.reflection(), &prepared.fields[index].1.type_info);
                let selected = scope.reference.cursor.scopes.last().unwrap();
                assert_eq!(
                    selected.selection,
                    CheckedComptimeTypeSelection::ReflectedIteration(index)
                );
                assert_eq!(selected.bound_type, expected[index]);
                checked.install_direct_scope(&scope).unwrap();
                checked.install_function_body(&parent).unwrap();
                assert_eq!(checked.attempt_key(&checked.cursor()).unwrap(), before);
            }
        }
    }
}

#[test]
fn selected_metadata_shapes_execute_in_source_order_without_runtime_authority() {
    for release in [false, true] {
        let program = crate::resource_execution::tests::program(SHAPES_SOURCE, release);
        let checked =
            CheckedExecution::new(program.clone(), ExecutionPurpose::ReferenceRuntime).unwrap();
        let mut interpreter = Interpreter::from_checked_resource_program(
            program.clone(),
            ExecutionPurpose::ReferenceRuntime,
        )
        .unwrap();
        interpreter.set_reflection_metadata(Arc::new(ReflectionMetadata::new()));
        let baseline = interpreter.resource_test_entry_context().unwrap();
        for _ in 0..2 {
            for (name, expected) in [
                (
                    "heterogeneous_fields",
                    "label:string|count:int64|enabled:bool|",
                ),
                ("bitfield_fields", "sequence:uint64|"),
                ("generic_fields", "first:int64|second:string|"),
            ] {
                let definition = checked
                    .declaration_definition(original_function(&program, name).name.span)
                    .unwrap();
                assert_eq!(
                    interpreter
                        .call_checked_program_entry(definition, Vec::new())
                        .unwrap(),
                    Value::String(expected.to_string()),
                    "{name}, release={release}"
                );
                assert_eq!(interpreter.resource_test_entry_context().unwrap(), baseline);
                assert_eq!(
                    interpreter.resource_test_observations(),
                    Err(ResourceExecutionError::ProviderDisabled.to_string())
                );
                assert_eq!(interpreter.resource_test_custody_counts().unwrap(), (0, 0));
                assert!(interpreter.take_debug_events().is_empty());
            }
        }
    }
}
