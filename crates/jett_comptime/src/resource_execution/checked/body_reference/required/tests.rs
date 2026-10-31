use super::*;
use crate::checked_types::CheckedExpressionTypes;
use crate::resource_execution::tests::program as checked_program;
use crate::{Interpreter, Value};

const SOURCE: &str = include_str!("../../../fixtures/26_original_required_regions.jett");

fn types(program: &Arc<CheckedResourceProgram>) -> Arc<CheckedExpressionTypes> {
    Arc::new(CheckedExpressionTypes {
        resource_program: Some(program.clone()),
        expressions: program
            .checked()
            .type_map
            .iter()
            .map(|(span, ty)| (*span, program.checked().interner.type_name(*ty)))
            .collect(),
        ..Default::default()
    })
}

#[test]
fn resource_original_required_regions_preserve_existing_descriptor_absence_and_primitive_workers() {
    for release in [false, true] {
        let program = checked_program(SOURCE, release);
        let types = types(&program);
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
        assert!(required.values.checked_values_are_mirrored());
        assert_eq!(required.values.len(), required.values.values().count());
        let blocks = crate::verify::run_verify_blocks_detailed_with_checked_values(
            program.module(),
            Arc::new(jett_types::ReflectionMetadata::new()),
            types.clone(),
            Arc::new(HashMap::new()),
            Arc::new(required.values.clone()),
        );
        assert_eq!(blocks.len(), 2);
        assert!(
            blocks
                .iter()
                .all(|block| block.passed && block.debug_events.is_empty()),
            "{blocks:?}"
        );
        let original = program
            .module()
            .items
            .iter()
            .find_map(|item| match item {
                Item::Function(function) if function.name.name == "scenario" => Some(function),
                _ => None,
            })
            .unwrap();
        let checked =
            CheckedExecution::new(program.clone(), ExecutionPurpose::ReferenceRuntime).unwrap();
        let target = checked.declaration_definition(original.name.span).unwrap();
        // Match the driver: baked constants are installed before module registration.
        let mut interpreter = Interpreter::new();
        interpreter.set_checked_expression_types(types);
        interpreter.set_explicit_comptime_values(Arc::new(required.values));
        interpreter.register_module(program.module());
        interpreter
            .install_checked_resource_program(program, ExecutionPurpose::ReferenceRuntime)
            .unwrap();
        assert_eq!(
            interpreter
                .call_checked_program_entry(target, Vec::new())
                .unwrap(),
            Value::Int64(7)
        );
        assert_eq!(interpreter.resource_test_custody_counts().unwrap(), (0, 0));
        assert!(interpreter.take_debug_events().is_empty());
    }
}

#[test]
fn resource_required_reference_refuses_cloned_suite_initializer_and_wrong_purpose_without_installing()
 {
    for release in [false, true] {
        let program = checked_program(SOURCE, release);
        let mut checked =
            CheckedExecution::new(program.clone(), ExecutionPurpose::NamespaceConstant).unwrap();
        let declaration = program
            .module()
            .items
            .iter()
            .find_map(|item| match item {
                Item::VarDecl(value) => Some(value),
                _ => None,
            })
            .unwrap();
        let prepared = checked.prepare_namespace_initializer(declaration).unwrap();
        assert_eq!(prepared.purpose(), ExecutionPurpose::NamespaceConstant);
        assert!(matches!(
            checked.prepare_namespace_initializer(&declaration.clone()),
            Err(ResourceExecutionError::MissingCheckedBody)
        ));
        checked.replace_purpose(ExecutionPurpose::Verify);
        assert!(matches!(
            checked.prepare_namespace_initializer(declaration),
            Err(ResourceExecutionError::WrongPurpose)
        ));
        let verify = program
            .module()
            .items
            .iter()
            .find_map(|item| match item {
                Item::Verify(value) => Some(value),
                _ => None,
            })
            .unwrap();
        let reference = checked.prepare_verify_body(verify).unwrap();
        assert!(std::ptr::eq(reference.block().unwrap(), &verify.body));
        assert!(matches!(
            checked.prepare_verify_body(&verify.clone()),
            Err(ResourceExecutionError::MissingCheckedBody)
        ));
        let foreign =
            CheckedExecution::new(checked_program(SOURCE, release), ExecutionPurpose::Verify)
                .unwrap();
        assert_eq!(
            foreign.validate_executable_cursor(&reference.cursor),
            Err(ResourceExecutionError::ForeignProgram)
        );
        assert!(matches!(
            foreign.attempt_key(&reference.cursor),
            Err(ResourceExecutionError::ForeignProgram)
        ));
    }
}

#[test]
fn resource_required_cache_key_keeps_exact_program_original_node_and_scope_identity() {
    for release in [false, true] {
        let program = checked_program(SOURCE, release);
        let checked =
            CheckedExecution::new(program.clone(), ExecutionPurpose::NamespaceConstant).unwrap();
        let declaration = program
            .module()
            .items
            .iter()
            .find_map(|item| match item {
                Item::VarDecl(value) => Some(value),
                _ => None,
            })
            .unwrap();
        let original = checked.prepare_namespace_initializer(declaration).unwrap();
        let foreign_program = checked_program(SOURCE, release);
        let foreign =
            CheckedExecution::new(foreign_program.clone(), ExecutionPurpose::NamespaceConstant)
                .unwrap();
        let foreign_declaration = foreign_program
            .module()
            .items
            .iter()
            .find_map(|item| match item {
                Item::VarDecl(value) => Some(value),
                _ => None,
            })
            .unwrap();
        let other = foreign
            .prepare_namespace_initializer(foreign_declaration)
            .unwrap();
        assert_ne!(original.key(), other.key());
        assert_eq!(
            original.key(),
            checked
                .prepare_namespace_initializer(declaration)
                .unwrap()
                .key()
        );
        let clone = original.expression().unwrap().clone();
        assert!(matches!(
            checked.required_expression(original.reference.cursor.clone(), &clone, Vec::new()),
            Err(ResourceExecutionError::MissingCheckedBody)
        ));
    }
}

#[test]
fn resource_required_generic_and_nested_scopes_use_exact_selected_cache_contexts() {
    for release in [false, true] {
        let program = checked_program(
            include_str!("../../../fixtures/27_original_generic_scoped_required.jett"),
            release,
        );
        let required = crate::evaluate_explicit_comptime_expressions_capture(
            program.module(),
            Arc::new(jett_types::ReflectionMetadata::new()),
            types(&program),
            Arc::new(HashMap::new()),
        );
        assert!(
            required.diagnostics.is_empty(),
            "{:?}",
            required.diagnostics
        );
        assert!(required.debug_events.is_empty());
        assert!(required.values.checked_values_are_mirrored());
        let checked =
            CheckedExecution::new(program.clone(), ExecutionPurpose::ReferenceRuntime).unwrap();
        let mut interpreter = Interpreter::from_checked_resource_program(
            program.clone(),
            ExecutionPurpose::ReferenceRuntime,
        )
        .unwrap();
        interpreter.set_explicit_comptime_values(Arc::new(required.values));
        for name in ["generic_label", "scoped_label"] {
            let function = program
                .module()
                .items
                .iter()
                .find_map(|item| match item {
                    Item::Function(function) if function.name.name == name => Some(function),
                    _ => None,
                })
                .unwrap();
            let target = checked.declaration_definition(function.name.span).unwrap();
            assert_eq!(
                interpreter
                    .call_checked_program_entry(target, Vec::new())
                    .unwrap(),
                Value::String("int64".to_string())
            );
            assert_eq!(interpreter.resource_test_custody_counts().unwrap(), (0, 0));
            assert!(interpreter.take_debug_events().is_empty());
        }
    }
}
