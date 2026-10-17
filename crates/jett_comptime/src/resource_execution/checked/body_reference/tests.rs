use super::*;
use jett_common::FileId;

const SOURCE: &str = include_str!("../../fixtures/13_direct_nested_scoped_lifecycle.jett");

fn execution(
    release: bool,
) -> (
    Arc<CheckedResourceProgram>,
    CheckedExecution,
    CheckedBodyReference,
) {
    let program = crate::resource_execution::tests::program(SOURCE, release);
    let mut checked =
        CheckedExecution::new(program.clone(), ExecutionPurpose::ReferenceRuntime).unwrap();
    let function = program
        .module()
        .items
        .iter()
        .find_map(|item| match item {
            Item::Function(function)
                if function.name.name == "scenario"
                    && function.name.span.file == FileId::new(0) =>
            {
                Some(function)
            }
            _ => None,
        })
        .unwrap();
    let definition = checked.declaration_definition(function.name.span).unwrap();
    let reference = checked
        .prepare_function_body(&checked.entry(definition).unwrap())
        .unwrap();
    assert!(std::ptr::eq(reference.function().unwrap(), function));
    checked.install_function_body(&reference).unwrap();
    (program, checked, reference)
}

fn first_binding(block: &Block) -> &ComptimeTypeBindStmt {
    block
        .stmts
        .iter()
        .find_map(|statement| match statement {
            Stmt::ComptimeTypeBind(binding) => Some(binding),
            _ => None,
        })
        .unwrap()
}

fn first_call(block: &Block) -> (&Expr, &[CallArg], Span) {
    let mut calls = Vec::new();
    walk_block(block, &mut |_| {}, &mut |expression| {
        if let Expr::Call(callee, arguments, span) | Expr::GenericCall(callee, _, arguments, span) =
            expression
        {
            calls.push((callee.as_ref(), arguments.as_slice(), *span));
        }
    });
    *calls.first().unwrap()
}

#[test]
fn checked_direct_scope_rejoins_original_nested_type_and_exact_parent_without_mutation() {
    for release in [false, true] {
        let (_, mut checked, original) = execution(release);
        let outer = first_binding(&original.function().unwrap().body);
        let outer_entry = checked.prepare_direct_scope(outer).unwrap();
        assert!(
            checked.body.scopes.is_empty(),
            "preparation must not install its selection"
        );
        assert!(std::ptr::eq(outer_entry.body().unwrap(), &outer.body));
        checked.install_direct_scope(&outer_entry).unwrap();
        assert_eq!(checked.body.scopes.len(), 1);
        let outer_cursor = checked.cursor();
        let nested = first_binding(outer_entry.body().unwrap());
        let nested_entry = checked.prepare_direct_scope(nested).unwrap();
        assert_eq!(checked.body.scopes.len(), 1);
        assert!(std::ptr::eq(nested_entry.body().unwrap(), &nested.body));
        assert_eq!(nested_entry.bound_name(), "int64");
        checked.install_direct_scope(&nested_entry).unwrap();
        assert_eq!(checked.body.scopes.len(), 2);
        assert!(matches!(
            checked.prepare_direct_scope(outer),
            Err(ResourceExecutionError::MissingCheckedBody)
        ));
        assert_eq!(
            checked.body.scopes.len(),
            2,
            "rejected parent must leave the active cursor unchanged"
        );
        checked.restore_cursor(outer_cursor).unwrap();
        assert_eq!(checked.body.scopes.len(), 1);
        checked.install_function_body(&original).unwrap();
        assert!(checked.body.scopes.is_empty());
    }
}

#[test]
fn checked_body_reference_refuses_foreign_program_wrong_root_and_scope_identity() {
    for release in [false, true] {
        let (_, mut checked, original) = execution(release);
        let (_, foreign, _) = execution(release);
        assert_eq!(
            foreign.validate_executable_cursor(&original.cursor),
            Err(ResourceExecutionError::ForeignProgram)
        );
        for corruption in 0..4 {
            let mut bad = original.clone();
            match corruption {
                0 => {
                    bad.cursor.executable.as_mut().unwrap().origin = ExecutableOrigin::Function {
                        definition: checked
                            .declaration_definition(original.function().unwrap().name.span)
                            .unwrap(),
                        item: usize::MAX,
                    }
                }
                1 => {
                    bad.cursor.executable.as_mut().unwrap().origin = ExecutableOrigin::Function {
                        definition: DefId::new(u32::MAX),
                        item: 0,
                    }
                }
                2 => bad.cursor.root = BodyRoot::Generic(usize::MAX),
                3 => {
                    let outer = first_binding(&original.function().unwrap().body);
                    bad = checked.prepare_direct_scope(outer).unwrap().reference;
                    bad.cursor.scopes[0].bound_type = jett_types::TypeInterner::INT64;
                }
                _ => unreachable!(),
            }
            assert!(
                checked.install_function_body(&bad).is_err(),
                "corruption {corruption}"
            );
            assert!(checked.body.scopes.is_empty());
            assert!(std::ptr::eq(
                CheckedBodyReference {
                    cursor: checked.cursor()
                }
                .function()
                .unwrap(),
                original.function().unwrap()
            ));
        }
    }
}

#[test]
fn checked_source_calls_require_original_occurrence_and_exact_scoped_packet() {
    for release in [false, true] {
        let (_, mut checked, original) = execution(release);
        let outer = first_binding(&original.function().unwrap().body);
        let cloned_binding = outer.clone();
        assert!(matches!(
            checked.prepare_direct_scope(&cloned_binding),
            Err(ResourceExecutionError::MissingCheckedBody)
        ));
        let scoped_call = first_call(&outer.body);
        assert_eq!(
            checked.source_call_dispatch(scoped_call.0, scoped_call.1, scoped_call.2),
            Err(ResourceExecutionError::MissingCheckedInvocation)
        );
        let prepared = checked.prepare_direct_scope(outer).unwrap();
        checked.install_direct_scope(&prepared).unwrap();
        assert_eq!(
            checked.source_call_dispatch(scoped_call.0, scoped_call.1, scoped_call.2),
            Ok(true)
        );
        let cloned_callee = scoped_call.0.clone();
        assert_eq!(
            checked.source_call_dispatch(&cloned_callee, scoped_call.1, scoped_call.2),
            Err(ResourceExecutionError::MissingCheckedInvocation)
        );
        let cloned_arguments = scoped_call.1.to_vec();
        assert_eq!(
            checked.source_call_dispatch(scoped_call.0, &cloned_arguments, scoped_call.2),
            Err(ResourceExecutionError::MissingCheckedInvocation)
        );
        let unselected = CheckedExecution::new(
            checked.program().clone(),
            ExecutionPurpose::ReferenceRuntime,
        )
        .unwrap();
        assert_eq!(
            unselected.source_call_dispatch(scoped_call.0, scoped_call.1, scoped_call.2),
            Err(ResourceExecutionError::MissingCheckedBody)
        );
    }
}

#[test]
fn resource_original_typed_machine_constructor_refuses_copied_source_and_wrong_body() {
    let source = include_str!("../../fixtures/17_absent_aggregate_shapes.jett");
    for release in [false, true] {
        let program = crate::resource_execution::tests::program(source, release);
        let function = program
            .module()
            .items
            .iter()
            .find_map(|item| match item {
                Item::Function(function) if function.name.name == "absent_state" => Some(function),
                _ => None,
            })
            .unwrap();
        let mut checked =
            CheckedExecution::new(program.clone(), ExecutionPurpose::ReferenceRuntime).unwrap();
        let definition = checked.declaration_definition(function.name.span).unwrap();
        let reference = checked
            .prepare_function_body(&checked.entry(definition).unwrap())
            .unwrap();
        checked.install_function_body(&reference).unwrap();
        let (callee, arguments, span) = first_call(&function.body);
        assert!(!checked.has_invocation(span).unwrap());
        assert_eq!(
            checked.source_call_dispatch(callee, arguments, span),
            Ok(false)
        );
        assert_eq!(
            checked.source_call_dispatch(&callee.clone(), arguments, span),
            Err(ResourceExecutionError::MissingCheckedInvocation)
        );
        assert_eq!(
            checked.source_call_dispatch(callee, &arguments.to_vec(), span),
            Err(ResourceExecutionError::MissingCheckedInvocation)
        );
        let wrong = program
            .module()
            .items
            .iter()
            .find_map(|item| match item {
                Item::Function(function) if function.name.name == "empty_tokens" => Some(function),
                _ => None,
            })
            .unwrap();
        let wrong_definition = checked.declaration_definition(wrong.name.span).unwrap();
        let wrong_reference = checked
            .prepare_function_body(&checked.entry(wrong_definition).unwrap())
            .unwrap();
        checked.install_function_body(&wrong_reference).unwrap();
        assert_eq!(
            checked.source_call_dispatch(callee, arguments, span),
            Err(ResourceExecutionError::MissingCheckedInvocation)
        );
    }
}
