//! Original named-call envelopes. Alias parameters never enter Vec<Value> as bytes.
use super::*;
use jett_typecheck::{
    CheckedCalleeAccess, CheckedCallerEffect, CheckedCallerOrigin, CheckedCallerSyntax,
    CheckedInvocationShape, CheckedInvocationTarget,
};

pub(in crate::interpreter) struct PreparedNamedFrame {
    caller_activation: ActivationId,
    call: receipts::CallReceipt,
    pub(super) callee: FunctionReceipt,
    aliases: Vec<Option<storage::AliasTicket>>,
}

#[cfg(test)]
mod tests {
    //! Private transport controls; native execution evidence is recorded separately.
    use super::*;
    use jett_common::{FileId, SourceOrigin};
    use jett_typecheck::{CheckOptions, CheckedResourceProgram};

    const SOURCE: &str = r#"namespace app
struct Packet:
    data: bytes
    sibling: bytes
function make_packet(data: bytes, sibling: bytes) returns Packet:
    return Packet(data: data, sibling: sibling)
function inspect(view data: bytes, marker: int64) returns nothing:
    return nothing
function other(view data: bytes, marker: int64) returns nothing:
    return nothing
function scenario(data: bytes, sibling: bytes) returns nothing:
    bytes borrowed = view make_packet(data, sibling).data
    bytes forwarded = borrowed
    inspect(marker: 7, data: view forwarded)
    return nothing
"#;

    fn program() -> Arc<CheckedResourceProgram> {
        let file = FileId::new(0);
        Arc::new(
            CheckedResourceProgram::prepare(
                jett_parser::parse(SOURCE, file),
                HashMap::from([(file, SourceOrigin::Project)]),
                &[],
                CheckOptions::default(),
            )
            .unwrap(),
        )
    }
    fn function(program: &Arc<CheckedResourceProgram>, name: &str) -> FunctionReceipt {
        let source = program
            .module()
            .items
            .iter()
            .find_map(|item| match item {
                Item::Function(function) if function.name.name == name => Some(function),
                _ => None,
            })
            .unwrap();
        receipts::function_receipt(program, &format!("app.{name}"), &Some("app".into()), source)
            .unwrap()
            .unwrap()
    }
    fn declaration<'a>(function: &'a FunctionReceipt, name: &str) -> &'a jett_parser::ast::VarDecl {
        function
            .original()
            .unwrap()
            .body
            .stmts
            .iter()
            .find_map(|statement| match statement {
                Stmt::VarDecl(declaration) if declaration.name.name == name => Some(declaration),
                _ => None,
            })
            .unwrap()
    }
    fn started() -> (Interpreter, FunctionReceipt, ActivationId) {
        let program = program();
        let function = function(&program, "scenario");
        let mut interpreter = Interpreter::from_checked_resource_program(
            program.clone(),
            crate::ExecutionPurpose::ReferenceRuntime,
        )
        .unwrap();
        interpreter.checked_expression_types = Some(Arc::new(
            CheckedExpressionTypes::from_checked_program(&program),
        ));
        interpreter.push_scope(); // synthetic parameter storage, not the source lifetime
        let context = interpreter.temporary_local_views.as_mut().unwrap();
        context.entry_allowed = true;
        let activation = context.enter(function.clone()).unwrap();
        interpreter.push_scope();
        let scope = interpreter.scopes.len() - 1;
        assert_eq!(
            interpreter
                .temporary_local_views
                .as_mut()
                .unwrap()
                .begin(&function.original().unwrap().body, scope)
                .unwrap(),
            Some(activation)
        );
        (interpreter, function, activation)
    }
    fn whole(function: &FunctionReceipt) -> Value {
        let receipt = receipts::producer_receipt(function, declaration(function, "borrowed"))
            .unwrap()
            .unwrap();
        let types = &function.program.checked().interner;
        let Type::Struct(id) = types.resolve(receipt.whole) else {
            panic!("checked Packet");
        };
        Value::Struct {
            type_name: types.resolve_struct(*id).name.clone(),
            concrete_type: None,
            fields: vec![
                ("data".into(), Value::Bytes(vec![65, 66])),
                ("sibling".into(), Value::Bytes(vec![89, 90])),
            ],
        }
    }
    fn install(interpreter: &mut Interpreter, function: &FunctionReceipt) -> storage::AliasTicket {
        let declaration = declaration(function, "borrowed");
        let receipt = receipts::producer_receipt(function, declaration)
            .unwrap()
            .unwrap();
        let fact = receipt.fact;
        let context = interpreter.temporary_local_views.as_mut().unwrap();
        let slot = context.reserve(receipt).unwrap();
        let ticket = context.acquire(slot, whole(function)).unwrap();
        interpreter
            .publish_temporary_alias(fact, "borrowed", ticket.clone(), &declaration.ty)
            .unwrap();
        ticket
    }
    fn forwarded(interpreter: &mut Interpreter, function: &FunctionReceipt) {
        assert!(matches!(
            interpreter
                .exec_temporary_binding(declaration(function, "forwarded"))
                .unwrap(),
            Some(None)
        ));
    }
    fn frame(interpreter: &Interpreter, function: &FunctionReceipt) -> PreparedNamedFrame {
        let expression = function
            .original()
            .unwrap()
            .body
            .stmts
            .iter()
            .find_map(|statement| match statement {
                Stmt::Expr(statement) => Some(&statement.expr),
                _ => None,
            })
            .unwrap();
        let Expr::Call(callee, arguments, span) = expression else {
            panic!("original inspect call");
        };
        let call = receipts::checked_call(function, callee, arguments, *span).unwrap();
        assert_eq!(call.order, vec![1, 0]);
        let alias = interpreter
            .temporary_alias(&arguments[1].value)
            .unwrap()
            .unwrap();
        PreparedNamedFrame {
            caller_activation: interpreter
                .temporary_local_views
                .as_ref()
                .unwrap()
                .current()
                .unwrap()
                .id,
            callee: receipts::named_target(&call).unwrap().unwrap(),
            call,
            aliases: vec![Some(alias), None],
        }
    }
    fn finish(interpreter: &mut Interpreter, activation: ActivationId) {
        let context = interpreter.temporary_local_views.as_mut().unwrap();
        context.complete(activation).unwrap();
        assert!(
            context
                .current()
                .unwrap()
                .owners
                .iter()
                .all(|owner| !matches!(owner.state, storage::OwnerState::Live(_)))
        );
        context.leave(activation).unwrap();
        interpreter.pop_scope();
        interpreter.pop_scope();
        assert!(
            interpreter
                .temporary_local_views
                .as_ref()
                .unwrap()
                .activations
                .is_empty()
        );
    }

    #[test]
    fn whole_owner_keeps_unviewed_sibling_until_root_retirement() {
        let (mut interpreter, function, activation) = started();
        let ticket = install(&mut interpreter, &function);
        let context = interpreter.temporary_local_views.as_ref().unwrap();
        assert_eq!(
            context.observe(&ticket).unwrap(),
            &Value::Bytes(vec![65, 66])
        );
        let storage::OwnerState::Live(Value::Struct { fields, .. }) =
            &context.current().unwrap().owners[0].state
        else {
            panic!("whole Packet retained");
        };
        assert_eq!(fields[1], ("sibling".into(), Value::Bytes(vec![89, 90])));
        interpreter
            .temporary_local_views
            .as_mut()
            .unwrap()
            .complete(activation)
            .unwrap();
        let context = interpreter.temporary_local_views.as_ref().unwrap();
        assert!(context.observe(&ticket).is_err());
        let ended = context
            .audit
            .iter()
            .position(|event| matches!(event, storage::AuditEvent::AliasEnded { .. }))
            .unwrap();
        let retired = context
            .audit
            .iter()
            .position(|event| matches!(event, storage::AuditEvent::OwnerRetired { .. }))
            .unwrap();
        assert!(ended < retired);
        assert_eq!(
            context
                .audit
                .iter()
                .filter(|event| matches!(event, storage::AuditEvent::OwnerRetired { .. }))
                .count(),
            1
        );
        assert!(
            interpreter
                .temporary_local_views
                .as_mut()
                .unwrap()
                .complete(activation)
                .is_err()
        );
        interpreter
            .temporary_local_views
            .as_mut()
            .unwrap()
            .leave(activation)
            .unwrap();
        interpreter.pop_scope();
        interpreter.pop_scope();
        assert!(
            interpreter
                .temporary_local_views
                .as_ref()
                .unwrap()
                .current()
                .is_none()
        );
    }

    #[test]
    fn forwarding_keeps_one_ticket_and_explicit_clone_is_independent() {
        let (mut interpreter, function, activation) = started();
        let ticket = install(&mut interpreter, &function);
        forwarded(&mut interpreter, &function);
        let frame = frame(&interpreter, &function);
        assert!(frame.alias(0).unwrap() == &ticket);
        let context = interpreter.temporary_local_views.as_mut().unwrap();
        let Value::Bytes(mut cloned) = context.explicit_clone(&ticket).unwrap() else {
            panic!("bytes clone");
        };
        cloned.push(67);
        assert_eq!(cloned, vec![65, 66, 67]);
        assert_eq!(
            context.observe(&ticket).unwrap(),
            &Value::Bytes(vec![65, 66])
        );
        assert_eq!(context.current().unwrap().owners.len(), 1);
        assert_eq!(
            context
                .audit
                .iter()
                .filter(|event| matches!(event, storage::AuditEvent::ExplicitClone { .. }))
                .count(),
            1
        );
        finish(&mut interpreter, activation);
    }

    #[test]
    fn named_frame_rejects_wrong_foreign_empty_and_stale_contexts() {
        let (mut interpreter, function, activation) = started();
        let ticket = install(&mut interpreter, &function);
        forwarded(&mut interpreter, &function);
        let mut frame = frame(&interpreter, &function);
        frame.validate_target(&interpreter, &frame.callee).unwrap();
        assert!(
            frame
                .validate_target(&interpreter, &self::function(&function.program, "other"))
                .is_err()
        );
        let foreign = program();
        assert!(
            frame
                .validate_target(&interpreter, &self::function(&foreign, "inspect"))
                .is_err()
        );
        let caller = interpreter
            .temporary_local_views
            .as_mut()
            .unwrap()
            .activations
            .pop()
            .unwrap();
        assert!(frame.validate_target(&interpreter, &frame.callee).is_err());
        interpreter
            .temporary_local_views
            .as_mut()
            .unwrap()
            .activations
            .push(caller);
        frame.aliases[0].as_mut().unwrap().owner_activation = activation.checked_add(1).unwrap();
        assert!(frame.validate_target(&interpreter, &frame.callee).is_err());
        assert!(
            interpreter
                .temporary_local_views
                .as_ref()
                .unwrap()
                .observe(&ticket)
                .is_ok()
        );
        finish(&mut interpreter, activation);
    }

    #[test]
    fn malformed_alias_arity_and_permutation_refuse_before_publication() {
        let (mut interpreter, function, activation) = started();
        install(&mut interpreter, &function);
        forwarded(&mut interpreter, &function);
        let mut frame = frame(&interpreter, &function);
        let before = interpreter
            .temporary_local_views
            .as_ref()
            .unwrap()
            .audit
            .len();
        let alias = frame.aliases[0].take();
        assert!(frame.validate_target(&interpreter, &frame.callee).is_err());
        frame.aliases[0] = alias;
        frame.aliases[1] = frame.aliases[0].clone();
        assert!(frame.validate_target(&interpreter, &frame.callee).is_err());
        frame.aliases[1] = None;
        frame.aliases.pop();
        assert!(frame.validate_target(&interpreter, &frame.callee).is_err());
        frame.aliases.push(None);
        frame.call.order[0] = frame.call.order[1];
        assert!(frame.validate_target(&interpreter, &frame.callee).is_err());
        assert_eq!(
            interpreter
                .temporary_local_views
                .as_ref()
                .unwrap()
                .audit
                .len(),
            before
        );
        finish(&mut interpreter, activation);
    }

    #[test]
    fn callee_parameter_accepts_only_placeholder_and_preserves_caller_owner() {
        let (mut interpreter, function, activation) = started();
        let ticket = install(&mut interpreter, &function);
        forwarded(&mut interpreter, &function);
        let frame = frame(&interpreter, &function);
        frame.validate_target(&interpreter, &frame.callee).unwrap();
        let callee = interpreter
            .temporary_local_views
            .as_mut()
            .unwrap()
            .enter(frame.callee.clone())
            .unwrap();
        interpreter.push_scope();
        let source = frame.callee.original().unwrap();
        let sources = [Some("bytes".to_owned()), Some("int64".to_owned())];
        frame
            .validate_values(
                &interpreter,
                source,
                &[Value::Nothing, Value::Int64(7)],
                Some(&sources),
            )
            .unwrap();
        assert!(
            frame
                .validate_values(
                    &interpreter,
                    source,
                    &[Value::Bytes(vec![65, 66]), Value::Int64(7)],
                    Some(&sources)
                )
                .is_err()
        );
        assert!(
            frame
                .validate_values(&interpreter, source, &[Value::Nothing], Some(&sources))
                .is_err()
        );
        interpreter
            .install_temporary_parameter(0, &source.params[0], Some(&frame))
            .unwrap();
        interpreter.pop_scope();
        interpreter
            .temporary_local_views
            .as_mut()
            .unwrap()
            .leave(callee)
            .unwrap();
        assert!(
            interpreter
                .temporary_local_views
                .as_ref()
                .unwrap()
                .observe(&ticket)
                .is_ok()
        );
        finish(&mut interpreter, activation);
    }

    #[test]
    fn failed_acquire_retires_whole_without_publishing_alias() {
        let (mut interpreter, function, activation) = started();
        let receipt = receipts::producer_receipt(&function, declaration(&function, "borrowed"))
            .unwrap()
            .unwrap();
        let mut value = whole(&function);
        let Value::Struct { fields, .. } = &mut value else {
            panic!("Packet");
        };
        fields[0].1 = Value::Pending(Box::new(Value::Bytes(vec![65, 66])));
        let context = interpreter.temporary_local_views.as_mut().unwrap();
        let slot = context.reserve(receipt).unwrap();
        assert!(context.acquire(slot, value).is_err());
        assert!(matches!(
            context.current().unwrap().owners[0].state,
            storage::OwnerState::Retired
        ));
        assert!(context.aliases.iter().all(HashMap::is_empty));
        assert!(
            !context
                .audit
                .iter()
                .any(|event| matches!(event, storage::AuditEvent::AliasPublished { .. }))
        );
        let acquired = context
            .audit
            .iter()
            .position(|event| matches!(event, storage::AuditEvent::Acquired { .. }))
            .unwrap();
        let retired = context
            .audit
            .iter()
            .position(|event| matches!(event, storage::AuditEvent::OwnerRetired { .. }))
            .unwrap();
        assert!(acquired < retired);
        finish(&mut interpreter, activation);
    }

    #[test]
    fn owned_output_packet_is_acquired_before_cleanup_and_bound_to_exact_activation() {
        for mutation in 0..5 {
            let (mut interpreter, function, activation) = started();
            install(&mut interpreter, &function);
            let output = interpreter
                .acquire_temporary_output(
                    Some(activation),
                    Ok(Some(Signal::Return(Value::Nothing, None))),
                )
                .unwrap();
            let BlockOutput::Acquired(output) = output else {
                panic!("owned acquired output");
            };
            interpreter
                .complete_temporary_block(Some(activation))
                .unwrap();
            let events = &interpreter.temporary_local_views.as_ref().unwrap().audit;
            let acquired = events
                .iter()
                .position(|event| {
                    matches!(event,
                storage::AuditEvent::Output { activation: current } if *current == activation)
                })
                .unwrap();
            let retired = events.iter().position(|event| matches!(event,
                storage::AuditEvent::OwnerRetired { activation: current, .. } if *current == activation)).unwrap();
            assert!(acquired < retired);
            let other = self::function(&function.program, "other");
            let foreign = self::function(&program(), "scenario");
            let (expected_activation, expected_function) = match mutation {
                0 => (Some(activation), Some(&function)),
                1 => (None, Some(&function)),
                2 => (Some(activation.checked_add(1).unwrap()), Some(&function)),
                3 => (Some(activation), Some(&other)),
                4 => (Some(activation), Some(&foreign)),
                _ => unreachable!(),
            };
            let result =
                output.into_registered_value(&interpreter, expected_activation, expected_function);
            if mutation == 0 {
                assert_eq!(result.unwrap(), Value::Nothing);
            } else {
                assert!(result.is_err());
            }
            interpreter.pop_scope();
            interpreter
                .temporary_local_views
                .as_mut()
                .unwrap()
                .leave(activation)
                .unwrap();
        }
    }

    #[test]
    fn ordinary_entry_selection_preserves_interface_dispatch_and_refuses_edited_roots() {
        let source = SOURCE.replacen(
            "namespace app\n",
            r#"namespace app
interface Named:
    function name(view self: Named) returns string
implement Named for int64:
    function name(view self: int64) returns string:
        return "value:{self}"
struct NamedPacket:
    member: Named
"#,
            1,
        ) + r#"function report(view item: Named, extra: int64) returns string:
    return "{Named.name(view item)}:{extra}"
function fallback() returns int64:
    return 3
function dispatched() returns string:
    NamedPacket packet = NamedPacket(member: 7)
    optional[int64] absent = none
    return report(view packet.member, absent handle:
        default fallback()
    )
"#;
        let file = FileId::new(0);
        let program = Arc::new(
            CheckedResourceProgram::prepare(
                jett_parser::parse(&source, file),
                HashMap::from([(file, SourceOrigin::Project)]),
                &[],
                CheckOptions::default(),
            )
            .unwrap(),
        );
        let mut interpreter = Interpreter::from_checked_resource_program(
            program.clone(),
            crate::ExecutionPurpose::ReferenceRuntime,
        )
        .unwrap();
        interpreter.checked_expression_types = Some(Arc::new(
            CheckedExpressionTypes::from_checked_program(&program),
        ));
        assert!(interpreter.temporary_local_views.is_some());
        let method_name = "int64 as app.Named.name";
        let method = interpreter.functions.get(method_name).unwrap().clone();
        assert!(
            receipts::function_receipt(
                &program,
                method_name,
                &method.namespace,
                &method.definition,
            )
            .unwrap()
            .is_none()
        );
        assert!(
            interpreter
                .prepare_temporary_function(
                    method_name,
                    &method.namespace,
                    &method.definition,
                    true,
                    None,
                )
                .unwrap()
                .is_none()
        );
        let original = function(&program, "inspect");
        let mut edited = original.original().unwrap().clone();
        edited.body.stmts.clear();
        assert!(
            receipts::function_receipt(&program, "app.inspect", &Some("app".into()), &edited)
                .is_err()
        );
        // An edited span or namespace cannot turn a claimed root registration
        // into an unrelated body and avoid its exact Source authentication.
        edited = original.original().unwrap().clone();
        edited.name.span = method.definition.name.span;
        assert!(receipts::function_receipt(&program, "app.inspect", &None, &edited).is_err());
        // An ineligible body still cannot receive a temporary frame or borrow
        // a source span whose declaration requires the temporary adapter.
        let (mut frame_interpreter, caller, activation) = started();
        install(&mut frame_interpreter, &caller);
        forwarded(&mut frame_interpreter, &caller);
        let frame = frame(&frame_interpreter, &caller);
        assert!(
            interpreter
                .prepare_temporary_function(
                    method_name,
                    &method.namespace,
                    &method.definition,
                    true,
                    Some(&frame),
                )
                .is_err()
        );
        finish(&mut frame_interpreter, activation);
        let mut required = method.definition.as_ref().clone();
        required.body = function(&program, "scenario")
            .original()
            .unwrap()
            .body
            .clone();
        assert!(
            interpreter
                .prepare_temporary_function(method_name, &method.namespace, &required, true, None)
                .is_err()
        );
        let context = interpreter.temporary_local_views.as_ref().unwrap();
        assert!(context.activations.is_empty());
        assert!(context.audit.is_empty());
        assert_eq!(
            interpreter
                .call_checked_function_in_namespace(Some("app"), "dispatched", Vec::new())
                .unwrap(),
            Value::String("value:7:3".into())
        );
        let context = interpreter.temporary_local_views.as_ref().unwrap();
        assert!(context.activations.is_empty());
        assert!(context.aliases.iter().all(HashMap::is_empty));
        assert!(!context.audit.iter().any(|event| matches!(
            event,
            storage::AuditEvent::Reserved { .. } | storage::AuditEvent::Acquired { .. }
        )));
    }
    const MUTUAL_SOURCE: &str = r#"namespace app
struct Packet:
    data: bytes
    sibling: bytes
mutual:
    function make_packet(data: bytes, sibling: bytes) returns Packet
    function inspect(view data: bytes, marker: int64) returns int64
    function scenario(data: bytes, sibling: bytes) returns int64
function make_packet(data: bytes, sibling: bytes) returns Packet:
    return Packet(data: data, sibling: sibling)
function inspect(view data: bytes, marker: int64) returns int64:
    return marker
function scenario(data: bytes, sibling: bytes) returns int64:
    bytes borrowed = view make_packet(data, sibling).data
    bytes forwarded = borrowed
    return inspect(marker: 7, data: view forwarded)
"#;

    fn mutual_program(release: bool) -> Arc<CheckedResourceProgram> {
        let file = FileId::new(0);
        Arc::new(
            CheckedResourceProgram::prepare(
                jett_parser::parse(MUTUAL_SOURCE, file),
                HashMap::from([(file, SourceOrigin::Project)]),
                &[],
                CheckOptions { release },
            )
            .unwrap(),
        )
    }

    fn mutual_interpreter(program: &Arc<CheckedResourceProgram>) -> Interpreter {
        let mut interpreter = Interpreter::from_checked_resource_program(
            program.clone(),
            crate::ExecutionPurpose::ReferenceRuntime,
        )
        .unwrap();
        interpreter.checked_expression_types = Some(Arc::new(
            CheckedExpressionTypes::from_checked_program(program),
        ));
        interpreter
    }

    #[test]
    fn mutual_declarations_join_exact_original_producer_caller_and_borrowed_callee() {
        for release in [false, true] {
            let program = mutual_program(release);
            for name in ["make_packet", "inspect", "scenario"] {
                let receipt = function(&program, name);
                let original = receipt.original().unwrap();
                let info = program.resolved().scope_table.def(receipt.definition);
                assert_ne!(info.span, original.name.span);
                assert_eq!(
                    program.resolved().resolutions.get(&original.name.span),
                    Some(&receipt.definition)
                );
                assert!(std::ptr::eq(
                    crate::value::closure_capture::original_named_function(
                        &program,
                        receipt.definition,
                    )
                    .unwrap(),
                    original,
                ));
                assert!(
                    crate::value::closure_capture::OriginalClosureOwner::registered(
                        &program,
                        &info.name,
                        &original.clone(),
                        &info.namespace,
                    )
                    .is_some()
                );
            }
            let mut interpreter = mutual_interpreter(&program);
            assert_eq!(
                interpreter
                    .call_checked_function_in_namespace(
                        Some("app"),
                        "scenario",
                        vec![Value::Bytes(vec![65, 66]), Value::Bytes(vec![89, 90])],
                    )
                    .unwrap(),
                Value::Int64(7)
            );
            let context = interpreter.temporary_local_views.as_ref().unwrap();
            assert!(context.activations.is_empty());
            assert!(context.aliases.iter().all(HashMap::is_empty));
            assert_eq!(
                context
                    .audit
                    .iter()
                    .filter(|event| matches!(event, storage::AuditEvent::Acquired { .. }))
                    .count(),
                1
            );
            assert_eq!(
                context
                    .audit
                    .iter()
                    .filter(|event| matches!(event, storage::AuditEvent::OwnerRetired { .. }))
                    .count(),
                1
            );
        }
    }

    #[test]
    fn mutual_definition_authentication_rejects_edited_body_span_header_and_namespace() {
        let program = mutual_program(false);
        let receipt = function(&program, "make_packet");
        let original = receipt.original().unwrap();
        let info = program.resolved().scope_table.def(receipt.definition);
        for mutation in 0..6 {
            let mut current = original.clone();
            let mut namespace = info.namespace.clone();
            let mut name = info.name.clone();
            match mutation {
                0 => current.body.stmts.clear(),
                1 => current.name.span = info.span,
                2 => current.params[0].view = true,
                3 => current.return_type = None,
                4 => namespace = Some("other".into()),
                5 => name = "app.inspect".into(),
                _ => unreachable!(),
            }
            assert!(
                crate::value::closure_capture::OriginalClosureOwner::registered(
                    &program, &name, &current, &namespace,
                )
                .is_none()
            );
            assert!(receipts::function_receipt(&program, &name, &namespace, &current).is_err());
        }
        let mut interpreter = mutual_interpreter(&program);
        Arc::make_mut(
            &mut interpreter
                .functions
                .get_mut("app.make_packet")
                .unwrap()
                .definition,
        )
        .body
        .stmts
        .clear();
        let error = interpreter
            .call_checked_function_in_namespace(
                Some("app"),
                "scenario",
                vec![Value::Bytes(vec![65, 66]), Value::Bytes(vec![89, 90])],
            )
            .unwrap_err();
        assert_eq!(
            error,
            "temporary local view function differs from its retained checked Source"
        );
        let context = interpreter.temporary_local_views.as_ref().unwrap();
        assert!(context.activations.is_empty());
        assert!(context.aliases.iter().all(HashMap::is_empty));
        assert!(
            !context
                .audit
                .iter()
                .any(|event| matches!(event, storage::AuditEvent::Acquired { .. }))
        );
    }
    const VIEWED_BINDINGS_SOURCE: &str = r#"namespace app
enum Payload:
    empty
    bytes_value(data: bytes)
    items(values: list[bytes])
function kind(view value: Payload) returns string:
    match value:
        empty:
            return "empty"
        bytes_value(data):
            return "bytes"
        items(values):
            return "items"
function count(view values: list[bytes]) returns int64:
    mutable int64 seen = 0
    for item in view values:
        seen = seen + 1
    return seen
"#;

    const WRITTEN_VIEWS_SOURCE: &str = r#"struct Packet:
    data: bytes
    sibling: bytes
function make_packet(data: bytes, sibling: bytes) returns Packet:
    return Packet(data: data, sibling: sibling)
function temporary(data: bytes, sibling: bytes) returns nothing:
    bytes borrowed = view make_packet(data, sibling).data
    return nothing
function nested(data: bytes, sibling: bytes) returns nothing:
    if true:
        bytes borrowed = view make_packet(data, sibling).data
    return nothing
"#;

    fn viewed_binding_program(with_temporaries: bool) -> Arc<CheckedResourceProgram> {
        let source = if with_temporaries {
            format!("{VIEWED_BINDINGS_SOURCE}{WRITTEN_VIEWS_SOURCE}")
        } else {
            VIEWED_BINDINGS_SOURCE.to_string()
        };
        let file = FileId::new(0);
        Arc::new(
            CheckedResourceProgram::prepare(
                jett_parser::parse(&source, file),
                HashMap::from([(file, SourceOrigin::Project)]),
                &[],
                CheckOptions::default(),
            )
            .unwrap(),
        )
    }

    #[test]
    fn viewed_pattern_and_iteration_binders_do_not_require_temporary_owner_admission() {
        for with_temporaries in [false, true] {
            let program = viewed_binding_program(with_temporaries);
            assert_eq!(Context::needed(&program), with_temporaries);
            for name in ["kind", "count"] {
                let receipt = function(&program, name);
                let body = &receipt.original().unwrap().body;
                let viewed_binders: Vec<_> = program
                    .checked()
                    .binding_facts
                    .values()
                    .filter(|fact| {
                        fact.declaration_span.file == body.span.file
                            && fact.declaration_span.start >= body.span.start
                            && fact.declaration_span.end <= body.span.end
                            && matches!(
                                fact.mode,
                                CheckedBindingMode::View {
                                    source: CheckedViewSource::Other
                                }
                            )
                            && program.resolved().scope_table.def(fact.definition).kind
                                == jett_resolve::DefKind::Variable
                    })
                    .collect();
                assert!(
                    !viewed_binders.is_empty(),
                    "genuine checked viewed {name} binders"
                );
                for fact in viewed_binders {
                    assert!(
                        crate::value::closure_capture::original_variable_declaration(
                            &program,
                            fact.declaration_span,
                        )
                        .is_none()
                    );
                }
                assert!(!receipt.has_temporary_views().unwrap());
                assert!(!Context::body_needed(&program, Some(body.span)));
            }
            let mut interpreter = mutual_interpreter(&program);
            assert_eq!(
                interpreter.temporary_local_views.is_some(),
                with_temporaries
            );
            for (variant, fields, expected) in [
                ("empty", vec![], "empty"),
                ("bytes_value", vec![Value::Bytes(vec![65, 66])], "bytes"),
                (
                    "items",
                    vec![Value::List(vec![Value::Bytes(vec![65, 66])])],
                    "items",
                ),
            ] {
                assert_eq!(
                    interpreter
                        .call_function_in_namespace(
                            Some("app"),
                            "kind",
                            vec![Value::Enum {
                                type_name: "app.Payload".into(),
                                variant: variant.into(),
                                fields,
                            }],
                        )
                        .unwrap(),
                    Value::String(expected.into())
                );
            }
            assert_eq!(
                interpreter
                    .call_function_in_namespace(
                        Some("app"),
                        "count",
                        vec![Value::List(vec![
                            Value::Bytes(vec![65]),
                            Value::Bytes(vec![66]),
                        ])],
                    )
                    .unwrap(),
                Value::Int64(2)
            );
            if let Some(context) = &interpreter.temporary_local_views {
                assert!(context.activations.is_empty());
                assert!(context.aliases.iter().all(HashMap::is_empty));
                assert!(context.audit.is_empty());
            }
        }
    }

    #[test]
    fn genuine_written_views_keep_raw_entry_and_unsupported_nested_region_refusals() {
        let program = viewed_binding_program(true);
        for name in ["temporary", "nested"] {
            let receipt = function(&program, name);
            assert!(receipt.has_temporary_views().unwrap());
            assert!(Context::body_needed(
                &program,
                Some(receipt.original().unwrap().body.span),
            ));
            let mut interpreter = mutual_interpreter(&program);
            assert_eq!(
                interpreter
                    .call_function_in_namespace(
                        Some("app"),
                        name,
                        vec![Value::Bytes(vec![65, 66]), Value::Bytes(vec![89, 90])],
                    )
                    .unwrap_err(),
                "temporary local view body needs its original checked entry or caller"
            );
            let context = interpreter.temporary_local_views.as_ref().unwrap();
            assert!(context.activations.is_empty());
            assert!(context.aliases.iter().all(HashMap::is_empty));
            assert!(context.audit.is_empty());
        }
        let mut interpreter = mutual_interpreter(&program);
        assert_eq!(
            interpreter
                .call_checked_function_in_namespace(
                    Some("app"),
                    "nested",
                    vec![Value::Bytes(vec![65, 66]), Value::Bytes(vec![89, 90])],
                )
                .unwrap_err(),
            "temporary local view nested declaration transport remains unsupported"
        );
        let context = interpreter.temporary_local_views.as_ref().unwrap();
        assert!(context.activations.is_empty());
        assert!(context.aliases.iter().all(HashMap::is_empty));
        assert!(
            !context
                .audit
                .iter()
                .any(|event| matches!(event, storage::AuditEvent::Acquired { .. }))
        );
    }
}

impl PreparedNamedFrame {
    pub(super) fn alias(&self, parameter: usize) -> Option<&storage::AliasTicket> {
        self.aliases.get(parameter).and_then(Option::as_ref)
    }

    pub(super) fn validate_target(
        &self,
        interpreter: &Interpreter,
        target: &FunctionReceipt,
    ) -> Result<(), String> {
        let context = interpreter
            .temporary_local_views
            .as_ref()
            .ok_or("temporary local view call lost its context")?;
        let caller = context
            .current()
            .ok_or("temporary local view call lost its live caller")?;
        if !self.callee.same(target)
            || !Arc::ptr_eq(&context.program, &target.program)
            || caller.id != self.caller_activation
            || !caller.root_active
            || !caller.function.same(&self.call.caller)
        {
            return Err("temporary local view rejects a foreign or stale named frame".into());
        }
        let original = self.call.caller.original()?;
        let calls: Vec<_> = receipts::direct_expressions(&original.body).into_iter().filter(|expression| {
            matches!(expression, Expr::Call(_, _, span) if *span == self.call.span)
        }).collect();
        let [Expr::Call(callee, arguments, span)] = calls.as_slice() else {
            return Err("temporary local view frame lost its unique original call".into());
        };
        let current = receipts::checked_call(&self.call.caller, callee, arguments, *span)?;
        if current.packet != self.call.packet
            || current.order != self.call.order
            || receipts::named_target(&current)?.is_none_or(|selected| !selected.same(target))
            || self.aliases.len() != arguments.len()
        {
            return Err("temporary local view named frame changed its checked join".into());
        }
        for (parameter, source) in current.order.iter().copied().enumerate() {
            let required = interpreter.temporary_alias(&arguments[source].value)?;
            if required.as_ref() != self.alias(parameter) {
                return Err(
                    "temporary local view named frame lost or invented an alias operand".into(),
                );
            }
            if let Some(ticket) = self.alias(parameter) {
                validate_alias(interpreter, &current, source, &arguments[source], ticket)?;
                validate_parameter(target, parameter, ticket)?;
            }
        }
        Ok(())
    }

    pub(super) fn validate_values(
        &self,
        interpreter: &Interpreter,
        function: &FunctionDef,
        values: &[Value],
        sources: Option<&[Option<String>]>,
    ) -> Result<(), String> {
        // This second check runs before normalization or publication. The callee
        // activation may be entered, so validate the recorded caller by ID rather
        // than relying on the current stack top.
        let context = interpreter
            .temporary_local_views
            .as_ref()
            .ok_or("temporary local view call lost its context")?;
        if !std::ptr::eq(function, self.callee.original()?)
            || values.len() != self.aliases.len()
            || values.len() != function.params.len()
            || sources.is_none_or(|sources| sources.len() != values.len())
            || !context.activations.iter().any(|caller| {
                caller.id == self.caller_activation
                    && caller.root_active
                    && caller.function.same(&self.call.caller)
            })
            || context
                .current()
                .is_none_or(|callee| callee.root_active || !callee.function.same(&self.callee))
            || context
                .activations
                .len()
                .checked_sub(2)
                .and_then(|index| context.activations.get(index))
                .is_none_or(|caller| caller.id != self.caller_activation)
        {
            return Err("temporary local view rejects a malformed parameter frame".into());
        }
        for (parameter, value) in values.iter().enumerate() {
            if let Some(ticket) = self.alias(parameter) {
                if !matches!(value, Value::Nothing) {
                    return Err(
                        "temporary local view rejects a payload-bearing view parameter".into(),
                    );
                }
                validate_parameter(&self.callee, parameter, ticket)?;
                context.observe(ticket)?;
            }
        }
        Ok(())
    }
}

fn validate_parameter(
    function: &FunctionReceipt,
    parameter: usize,
    ticket: &storage::AliasTicket,
) -> Result<(), String> {
    let original = function.original()?;
    let parameter = original
        .params
        .get(parameter)
        .ok_or("temporary local view parameter is absent")?;
    let fact = function.fact(&parameter.name)?;
    let info = function.program.resolved().scope_table.def(fact.definition);
    if !parameter.view
        || parameter.mutable
        || fact.mutable
        || info.kind != jett_resolve::DefKind::Param
        || fact.ty != ticket.endpoint
        || !matches!(
            function.program.checked().interner.resolve(fact.ty),
            Type::Bytes
        )
        || !matches!(
            fact.mode,
            CheckedBindingMode::View {
                source: CheckedViewSource::Other
            }
        )
    {
        return Err("temporary local view parameter changed its exact borrowed contract".into());
    }
    Ok(())
}

fn validate_alias(
    interpreter: &Interpreter,
    call: &receipts::CallReceipt,
    source: usize,
    argument: &CallArg,
    ticket: &storage::AliasTicket,
) -> Result<(), String> {
    let fact = call
        .packet
        .arguments
        .get(source)
        .ok_or("temporary local view argument fact is absent")?;
    let Expr::View(inner, _) = receipts::unparen(&argument.value) else {
        return Err("temporary local view forwarding call needs its written view".into());
    };
    let Expr::Ident(root) = receipts::unparen(inner) else {
        return Err("temporary local view call projection transport remains unsupported".into());
    };
    let definition = call
        .caller
        .program
        .resolved()
        .resolutions
        .get(&root.span)
        .ok_or("temporary local view argument lost its resolved binding use")?;
    let info = call.caller.program.resolved().scope_table.def(*definition);
    let source_fact = call
        .caller
        .program
        .checked()
        .binding_facts
        .get(&info.span)
        .copied()
        .ok_or("temporary local view argument lost its checked binding declaration")?;
    if source_fact.definition != *definition
        || source_fact.declaration_span != info.span
        || info.name != root.name
    {
        return Err("temporary local view argument changed its use/declaration join".into());
    }
    if fact.actual_type != ticket.endpoint
        || fact.parameter_type != ticket.endpoint
        || fact.syntax != CheckedCallerSyntax::WrittenView
        || fact.callee_access != CheckedCalleeAccess::View
        || fact.effect != CheckedCallerEffect::RetainBorrow
        || fact.origin != CheckedCallerOrigin::Binding(source_fact)
        || !matches!(source_fact.mode, CheckedBindingMode::View { .. })
        || call.caller.program.resolved().resolutions.get(&root.span)
            != Some(&source_fact.definition)
    {
        return Err("temporary local view argument lost its original alias disposition".into());
    }
    let current = interpreter
        .temporary_alias(&argument.value)?
        .ok_or("temporary local view argument lost its live alias")?;
    if &current != ticket {
        return Err("temporary local view argument changed its owner identity or path".into());
    }
    interpreter
        .temporary_local_views
        .as_ref()
        .ok_or("temporary local view call lost its context")?
        .observe(ticket)?;
    Ok(())
}

impl Interpreter {
    pub(in crate::interpreter) fn eval_temporary_call(
        &mut self,
        callee: &Expr,
        type_args: &[TypeExpr],
        arguments: &[CallArg],
        span: Span,
    ) -> Result<Option<ExprFlow>, String> {
        let Some(context) = &self.temporary_local_views else {
            return Ok(None);
        };
        let Some(caller) = context.current() else {
            return Ok(None);
        };
        if !caller.root_active {
            return Ok(None);
        }
        let caller_activation = caller.id;
        let function = caller.function.clone();
        self.temporary_program()?;
        // No authority is inferred for nested/control-flow regions. An alias
        // encountered through those paths is refused by its read adapter.
        if function.source_call(callee, arguments, span).is_err() {
            return Ok(None);
        }
        let call = receipts::checked_call(&function, callee, arguments, span)?;
        let mut lexical_aliases = Vec::with_capacity(arguments.len());
        for (source, argument) in arguments.iter().enumerate() {
            let alias = self.temporary_alias(&argument.value)?;
            if let Some(ticket) = &alias {
                validate_alias(self, &call, source, argument, ticket)?;
            }
            lexical_aliases.push(alias);
        }
        let any_alias = lexical_aliases.iter().any(Option::is_some);
        if let CheckedInvocationTarget::Intrinsic(intrinsic) = &call.packet.target {
            return if any_alias {
                self.eval_temporary_bytes_observer(&call, *intrinsic, arguments, lexical_aliases)
                    .map(Some)
            } else {
                Ok(None)
            };
        }
        let Some(target) = receipts::named_target(&call)? else {
            return if any_alias {
                Err("temporary local view callable transport remains unsupported".into())
            } else {
                Ok(None)
            };
        };
        if !type_args.is_empty() {
            return if any_alias {
                Err("temporary local view generic call transport remains unsupported".into())
            } else {
                Ok(None)
            };
        }
        let info = target.program.resolved().scope_table.def(target.definition);
        let name = info.name.clone();
        let registered = self
            .functions
            .get(&name)
            .ok_or("temporary local view named callee is not registered")?;
        let registered_receipt = receipts::function_receipt(
            &target.program,
            &name,
            &registered.namespace,
            &registered.definition,
        )?
        .ok_or("temporary local view named callee is not ordinary")?;
        if !target.same(&registered_receipt) || self.is_value_call_target(receipts::unparen(callee))
        {
            return Err("temporary local view call changed its registered named target".into());
        }
        let mut lexical = Vec::with_capacity(arguments.len());
        for (argument, alias) in arguments.iter().zip(lexical_aliases) {
            let source_type = self
                .checked_refinement_source_type(argument.value.span())
                .cloned();
            let value = match &alias {
                Some(_) => Value::Nothing,
                None => match self.eval_expr_flow(&argument.value)? {
                    ExprFlow::Value(value) => value,
                    ExprFlow::Signal(signal) => return Ok(Some(ExprFlow::Signal(signal))),
                    ExprFlow::Resource(_) => {
                        return Err(
                            "temporary local view named frame cannot carry Resource custody".into(),
                        );
                    }
                },
            };
            lexical.push(Some((value, source_type, alias)));
        }
        let mut values = Vec::with_capacity(lexical.len());
        let mut source_types = Vec::with_capacity(lexical.len());
        let mut aliases = Vec::with_capacity(lexical.len());
        for source in &call.order {
            let (value, source_type, alias) = lexical
                .get_mut(*source)
                .and_then(Option::take)
                .ok_or("temporary local view call repeats or loses a lexical argument")?;
            values.push(value);
            source_types.push(source_type);
            aliases.push(alias);
        }
        let frame = PreparedNamedFrame {
            caller_activation,
            call,
            callee: target,
            aliases,
        };
        frame.validate_target(self, &frame.callee)?;
        Ok(Some(ExprFlow::Value(
            self.call_registered_function_with_temporary_frame(
                &name,
                &[],
                values,
                Some(&source_types),
                Some(frame),
            )?,
        )))
    }

    fn eval_temporary_bytes_observer(
        &mut self,
        call: &receipts::CallReceipt,
        intrinsic: IntrinsicId,
        arguments: &[CallArg],
        aliases: Vec<Option<storage::AliasTicket>>,
    ) -> Result<ExprFlow, String> {
        if !self.current_function_trusted_stdlib
            || !matches!(
                intrinsic,
                IntrinsicId::BytesLength | IntrinsicId::BytesToHex | IntrinsicId::BytesGet
            )
        {
            return Err(
                "temporary local view observer transport remains unsupported for this intrinsic"
                    .into(),
            );
        }
        let CheckedInvocationShape::Intrinsic { .. } = &call.packet.shape else {
            return Err("temporary local view observer has no intrinsic packet".into());
        };
        let expected_arity = if intrinsic == IntrinsicId::BytesGet {
            2
        } else {
            1
        };
        if arguments.len() != expected_arity
            || call.order != (0..expected_arity).collect::<Vec<_>>()
            || aliases.first().and_then(Option::as_ref).is_none()
            || aliases.iter().skip(1).any(Option::is_some)
        {
            return Err(
                "temporary local view byte observer changed its exact operand frame".into(),
            );
        }
        let ticket = aliases[0]
            .as_ref()
            .ok_or("temporary local view byte observer lost its alias")?;
        // Evaluate independent operands in original lexical order. Borrow the
        // bytes only afterward; the evaluator never receives an owned snapshot.
        let index = if intrinsic == IntrinsicId::BytesGet {
            match self.eval_expr_flow(&arguments[1].value)? {
                ExprFlow::Value(Value::Int64(index)) => Some(index),
                ExprFlow::Signal(signal) => return Ok(ExprFlow::Signal(signal)),
                _ => return Err("bytes.get expects an int64 index".into()),
            }
        } else {
            None
        };
        let context = self
            .temporary_local_views
            .as_ref()
            .ok_or("temporary local view observer lost its context")?;
        let Value::Bytes(bytes) = context.observe(ticket)? else {
            return Err("temporary local view observer lost its exact bytes payload".into());
        };
        let result = match intrinsic {
            IntrinsicId::BytesLength => {
                Value::Int64(i64::try_from(bytes.len()).map_err(|_| "bytes.length exceeds int64")?)
            }
            IntrinsicId::BytesToHex => {
                Value::String(bytes.iter().map(|byte| format!("{byte:02x}")).collect())
            }
            IntrinsicId::BytesGet => match index
                .and_then(|index| usize::try_from(index).ok())
                .and_then(|index| bytes.get(index))
            {
                Some(byte) => Value::OptionalSome(Box::new(Value::Int64(i64::from(*byte)))),
                None => Value::OptionalNone,
            },
            _ => return Err("temporary local view observer escaped its closed dispatch".into()),
        };
        Ok(ExprFlow::Value(result))
    }
}
