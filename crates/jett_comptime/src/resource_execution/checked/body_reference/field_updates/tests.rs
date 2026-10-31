//! Checked Source identity and physical cursor controls.
//! These controls do not establish runtime policy or execution.
use super::*;

const SOURCE: &str = r#"namespace app
struct Record:
    value: int64
    label: string
function change(mutable owned: Record) returns Record:
    owned.value = 7
    return owned
function factory() returns function(Record) returns Record:
    return function(mutable owned: Record) returns Record:
        owned.value = 8
        return owned
"#;

fn source_function<'program>(
    program: &'program CheckedResourceProgram,
    name: &str,
) -> &'program FunctionDef {
    program
        .module()
        .items
        .iter()
        .find_map(|item| match item {
            Item::Function(function)
                if function.name.name == name
                    && function.name.span.file == jett_common::FileId::new(0) =>
            {
                Some(function)
            }
            _ => None,
        })
        .expect("genuine project function")
}

fn named_reference(execution: &CheckedExecution, name: &str) -> CheckedBodyReference {
    let original = source_function(execution.program(), name);
    let definition = execution
        .program()
        .resolved()
        .scope_table
        .definitions
        .iter()
        .find(|definition| {
            definition.kind == jett_resolve::DefKind::Function
                && definition.span == original.name.span
        })
        .expect("resolved original declaration");
    execution
        .prepare_field_function_body(&definition.name, &definition.namespace, original, None, &[])
        .expect("complete ordinary Source body")
}

#[test]
fn original_parameter_and_assignment_share_one_body_identity() {
    let program = crate::resource_execution::tests::program(SOURCE, false);
    let mut execution =
        CheckedExecution::new(program.clone(), ExecutionPurpose::ReferenceRuntime).unwrap();
    let reference = named_reference(&execution, "change");
    execution.install_field_function_body(&reference).unwrap();
    let function = source_function(&program, "change");
    let binding = execution
        .prepare_field_binding(&function.params[0].name)
        .unwrap();
    let Stmt::Assign(assignment) = &function.body.stmts[0] else {
        panic!("assignment")
    };
    let prepared = execution.prepare_field_assignment(assignment).unwrap();
    assert!(prepared.accepts_binding(&binding));
    execution.revalidate_field_assignment(&prepared).unwrap();
    assert!(
        execution
            .prepare_field_assignment(&assignment.clone())
            .is_err()
    );
    assert!(
        execution
            .prepare_field_binding(&function.params[0].name.clone())
            .is_err()
    );
}

#[test]
fn retained_body_reference_cannot_cross_programs() {
    let first = crate::resource_execution::tests::program(SOURCE, false);
    let second = crate::resource_execution::tests::program(SOURCE, false);
    let first = CheckedExecution::new(first, ExecutionPurpose::ReferenceRuntime).unwrap();
    let mut second = CheckedExecution::new(second, ExecutionPurpose::ReferenceRuntime).unwrap();
    assert!(matches!(
        second.install_field_function_body(&named_reference(&first, "change")),
        Err(ResourceExecutionError::ForeignProgram)
    ));
}

#[test]
fn field_only_inline_identity_cannot_authorize_resource_entry() {
    let program = crate::resource_execution::tests::program(SOURCE, false);
    let mut execution = CheckedExecution::new(program, ExecutionPurpose::ReferenceRuntime).unwrap();
    let mut reference = named_reference(&execution, "factory");
    let mut inline = None;
    walk_region(
        reference.region().unwrap(),
        &mut |_| {},
        &mut |node| {
            if matches!(node, Expr::InlineFn(..)) {
                inline = Some(node);
            }
        },
        &mut |_| {},
    );
    let index = expression_index(
        reference.region().unwrap(),
        inline.expect("original inline node"),
    )
    .unwrap();
    reference
        .cursor
        .executable
        .as_mut()
        .unwrap()
        .path
        .push(RegionStep::InlineBody(index));
    execution.install_field_function_body(&reference).unwrap();
    let (parameters, _) = reference.field_inline_parts().unwrap();
    execution
        .prepare_field_binding(&parameters[0].name)
        .unwrap();
    assert!(matches!(
        execution.install_function_body(&reference),
        Err(ResourceExecutionError::WrongPurpose)
    ));
}

#[test]
fn inline_selector_ignores_unrelated_resource_hook_declarations() {
    let program = crate::resource_execution::tests::program(SOURCE, false);
    assert!(!program.checked().resource_hooks.is_empty());
    let mut interpreter = crate::Interpreter::from_checked_resource_program(
        program.clone(),
        ExecutionPurpose::ReferenceRuntime,
    )
    .unwrap();
    interpreter.set_checked_expression_types(Arc::new(
        crate::checked_types::CheckedExpressionTypes::from_checked_program(&program),
    ));
    let closure = interpreter
        .call_checked_function_in_namespace(Some("app"), "factory", vec![])
        .expect("genuine checked factory capture");
    let crate::Value::Function { type_context, .. } = &closure else {
        panic!("factory must return its inline function");
    };
    let proof = crate::value::closure_capture::field_invocation_proof(&closure, &program)
        .expect("runtime capture retains its original Source witness");
    let mut execution = CheckedExecution::new(program, ExecutionPurpose::ReferenceRuntime).unwrap();
    let reference = execution
        .prepare_field_inline_body(&proof, type_context)
        .expect("unrelated hooks must not affect this original inline body");
    execution.install_field_function_body(&reference).unwrap();
    let (parameters, body) = reference.field_inline_parts().unwrap();
    let binding = execution
        .prepare_field_binding(&parameters[0].name)
        .unwrap();
    let Stmt::Assign(assignment) = &body.stmts[0] else {
        panic!("original inline field assignment");
    };
    let prepared = execution.prepare_field_assignment(assignment).unwrap();
    assert!(prepared.accepts_binding(&binding));
    assert!(matches!(
        execution.install_function_body(&reference),
        Err(ResourceExecutionError::WrongPurpose)
    ));
}
