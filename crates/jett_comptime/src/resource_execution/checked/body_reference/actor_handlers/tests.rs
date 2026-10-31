use super::*;

const SOURCE: &str = r#"namespace app
struct Record:
    value: int64
function change(mutable item: Record, amount: int64) returns Record:
    item.value = amount
    return item
actor Store:
    mutable Record record = Record(value: 1)
    receive set(amount: int64):
        record = change(record, amount)
    receive read responds int64:
        respond record.value
function exercise() returns int64:
    Store first = spawn Store()
    Store second = spawn Store()
    send first.set(7)
    send first.set(9)
    int64 unchanged = ask second.read
    int64 changed = ask first.read
    return changed + unchanged
"#;

fn entry(program: &Arc<CheckedResourceProgram>) -> (CheckedExecution, CheckedBodyReference) {
    let execution =
        CheckedExecution::new(program.clone(), ExecutionPurpose::ReferenceRuntime).unwrap();
    let function = program
        .module()
        .items
        .iter()
        .find_map(|item| match item {
            Item::Function(function) if function.name.name == "exercise" => Some(function),
            _ => None,
        })
        .unwrap();
    let info = program
        .resolved()
        .scope_table
        .definitions
        .iter()
        .find(|info| {
            info.kind == jett_resolve::DefKind::Function && info.span == function.name.span
        })
        .unwrap();
    let reference = execution
        .prepare_field_function_body(&info.name, &info.namespace, function, None, &[])
        .unwrap();
    (execution, reference)
}

fn first_message(reference: &CheckedBodyReference) -> &Expr {
    let mut messages = Vec::new();
    walk_region(
        reference.region().unwrap(),
        &mut |_| {},
        &mut |expression| {
            if matches!(expression, Expr::Send(..)) {
                messages.push(expression);
            }
        },
        &mut |_| {},
    );
    messages[0]
}

#[test]
fn original_actor_message_roles_and_program_origin_cannot_be_substituted() {
    let program = crate::resource_execution::tests::program(SOURCE, false);
    let (mut execution, reference) = entry(&program);
    execution.install_field_function_body(&reference).unwrap();
    let source = first_message(&reference);
    assert!(execution.prepare_actor_message(&source.clone()).is_err());
    let message = execution.prepare_actor_message(source).unwrap();
    let origin = CheckedActorInstanceOrigin::from_checked_actor(
        program.clone(),
        message.packet().target.actor,
    )
    .unwrap();
    let mut registered = origin.original().unwrap().clone();
    registered.name.name = program
        .checked()
        .interner
        .resolve_actor(message.packet().target.actor)
        .name
        .clone();
    let handler = execution
        .prepare_actor_handler(&message, &registered)
        .unwrap();
    assert!(handler.matches_instance(&origin));
    let foreign = crate::resource_execution::tests::program(SOURCE, false);
    let foreign_origin =
        CheckedActorInstanceOrigin::from_checked_actor(foreign, message.packet().target.actor)
            .unwrap();
    assert!(!handler.matches_instance(&foreign_origin));
    registered.handlers[0].params[0].mutable = !registered.handlers[0].params[0].mutable;
    assert!(
        execution
            .prepare_actor_handler(&message, &registered)
            .is_err()
    );
    execution
        .install_field_function_body(handler.reference())
        .unwrap();
    let state = execution
        .prepare_actor_binding(&handler, CheckedActorBindingRole::State { field: 0 })
        .unwrap();
    assert_eq!(
        state.fact().declaration_span,
        handler.original().unwrap().0.state_fields[0].name.span
    );
    assert!(
        execution
            .prepare_actor_binding(&handler, CheckedActorBindingRole::State { field: 1 })
            .is_err()
    );
    assert!(matches!(
        execution.install_function_body(handler.reference()),
        Err(ResourceExecutionError::WrongPurpose)
    ));
    let mut wrong = handler.reference().clone();
    wrong.cursor.root = BodyRoot::Ordinary;
    assert!(execution.install_field_function_body(&wrong).is_err());
}

#[test]
fn checked_actor_runtime_preserves_distinct_instances_and_calls_original_helper() {
    let program = crate::resource_execution::tests::program(SOURCE, false);
    let mut interpreter = crate::Interpreter::from_checked_resource_program(
        program.clone(),
        ExecutionPurpose::ReferenceRuntime,
    )
    .unwrap();
    interpreter.set_checked_expression_types(Arc::new(
        crate::checked_types::CheckedExpressionTypes::from_checked_program(&program),
    ));
    assert_eq!(
        interpreter
            .call_checked_function_in_namespace(Some("app"), "exercise", vec![])
            .unwrap(),
        crate::Value::Int64(10)
    );
}
