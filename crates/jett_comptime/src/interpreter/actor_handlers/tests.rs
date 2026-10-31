use super::*;

#[test]
fn actor_result_error_restores_sender_scopes_and_checked_context() {
    let mut interpreter = Interpreter::new();
    interpreter.current_namespace = Some("sender".into());
    interpreter.current_function_trusted_stdlib = true;
    interpreter.allow_checked_refinement_proofs = false;
    let span = Span::new(FileId::new(0), 0, 1);
    let ty = TypeExpr::Named(Ident {
        name: "int64".into(),
        span,
    });
    interpreter
        .type_arg_scopes
        .push(HashMap::from([("SenderType".into(), ty.clone())]));
    interpreter.current_type_arguments.push(ty);
    interpreter.set_variable("retained", Value::Int64(11));
    let depth = interpreter.scopes.len();
    let result: Result<(), String> =
        interpreter.with_actor_frame(None, Some("actor".into()), |this, captured| {
            assert_eq!(captured, depth);
            assert_eq!(this.current_namespace.as_deref(), Some("actor"));
            assert!(this.type_arg_scopes.is_empty());
            assert!(this.current_type_arguments.is_empty());
            this.set_variable("private", Value::Int64(7));
            this.push_scope();
            Err("prepublication failure".into())
        });
    assert_eq!(result.unwrap_err(), "prepublication failure");
    assert_eq!(interpreter.scopes.len(), depth);
    assert_eq!(interpreter.current_namespace.as_deref(), Some("sender"));
    assert!(interpreter.current_function_trusted_stdlib);
    assert!(!interpreter.allow_checked_refinement_proofs);
    assert_eq!(interpreter.type_arg_scopes.len(), 1);
    assert_eq!(interpreter.current_type_arguments.len(), 1);
    assert_eq!(
        interpreter.scopes[0].get("retained"),
        Some(&Value::Int64(11))
    );
    assert!(
        !interpreter
            .scopes
            .iter()
            .any(|scope| scope.contains_key("private"))
    );
}

#[test]
fn checked_source_actor_initializer_and_capture_preserve_original_hook_descriptors() {
    for source in [
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../jett_driver/tests/actor_graph/fixtures/07_nested_hook_actor.jett"
        )),
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../jett_driver/tests/actor_graph/fixtures/08_nested_hook_capture_actor.jett"
        )),
    ] {
        let program = crate::resource_execution::tests::program(source, false);
        let definition = program
            .resolved()
            .scope_table
            .definitions
            .iter()
            .find(|definition| {
                definition.kind == jett_resolve::DefKind::Function && definition.name == "app.main"
            })
            .unwrap()
            .id;
        // Match the real Driver entry: no optional expression-name projection.
        let mut interpreter = Interpreter::from_checked_resource_program(
            program.clone(),
            crate::ExecutionPurpose::ReferenceRuntime,
        )
        .unwrap();
        assert_eq!(
            interpreter
                .call_checked_program_entry(definition, vec![])
                .unwrap(),
            Value::Nothing
        );
        assert_eq!(interpreter.actor_instances.len(), 1);
        let actor = interpreter.actor_instances.values().next().unwrap();
        let descriptor = actor
            .state
            .get("dispose")
            .or_else(|| actor.capabilities.get("dispose"))
            .unwrap();
        let Value::ResourceHook(descriptor) = descriptor.payload() else {
            panic!("exact original initializer/capture must retain a hook descriptor")
        };
        let close = program
            .checked()
            .resource_hooks
            .values()
            .find(|hook| hook.kind == jett_types::ResourceHookKind::Close)
            .unwrap();
        let expected = interpreter
            .resource_transport
            .as_ref()
            .unwrap()
            .checked
            .descriptor(close.definition)
            .unwrap();
        assert!(descriptor.same_descriptor(&expected));
        let foreign = crate::resource_execution::tests::program(source, false);
        let foreign_close = foreign
            .checked()
            .resource_hooks
            .values()
            .find(|hook| hook.kind == jett_types::ResourceHookKind::Close)
            .unwrap();
        let foreign_execution = crate::resource_execution::CheckedExecution::new(
            foreign.clone(),
            crate::ExecutionPurpose::ReferenceRuntime,
        )
        .unwrap();
        let foreign_descriptor = foreign_execution
            .descriptor(foreign_close.definition)
            .unwrap();
        assert!(!descriptor.same_descriptor(&foreign_descriptor));
        assert!(interpreter.take_debug_events().is_empty());
        let transport = interpreter.resource_transport.as_ref().unwrap();
        assert!(
            !transport.checked_source_active,
            "checked top-level entry must restore the caller's inactive Source state"
        );
        let mut registered = program
            .module()
            .items
            .iter()
            .find_map(|item| match item {
                Item::Actor(actor) => Some(actor.clone()),
                _ => None,
            })
            .unwrap();
        registered.name.name = "app.Keeper".into();
        let bodies = transport
            .checked
            .prepare_actor_initializers(&registered)
            .unwrap();
        let guarded = CheckedResourceActorInitializers {
            program: program.clone(),
            bodies,
        };
        let mut foreign_interpreter = Interpreter::from_checked_resource_program(
            foreign,
            crate::ExecutionPurpose::ReferenceRuntime,
        )
        .unwrap();
        assert!(guarded.evaluate(&mut foreign_interpreter, 0).is_err());
        assert!(
            !foreign_interpreter
                .resource_transport
                .as_ref()
                .unwrap()
                .checked_source_active
        );
    }
}
