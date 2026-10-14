use super::*;
use crate::checked_types::CheckedExpressionTypes;
use crate::{ExecutionPurpose, ExplicitComptimeValues};
use jett_common::{FileId, SourceOrigin};
use jett_typecheck::CheckOptions;

// The existing public probe's source spellings are preserved. It remains a
// debug Source; these tests do not redefine its release-print admission.
const ORIGINAL_ESCAPE: &str = "namespace app\nactor Counter:\n    mutable int64 count = 1\n    receive read responds int64:\n        respond count\nfunction main() returns nothing:\n    Counter worker = comptime spawn Counter()\n    int64 value = ask worker.read\n    println(value)\n    return nothing\n";

const SHARED: &str = "namespace app\nactor Counter:\n    mutable int64 count = 1\n    receive add(amount: int64):\n        count = count + amount\n    receive read responds int64:\n        respond count\nstruct Pair:\n    left: Counter\n    right: Counter\nfunction create() returns Pair:\n    Counter worker = spawn Counter()\n    send worker.add(4)\n    return Pair(left: clone worker, right: worker)\nfunction baked() returns Pair:\n    return comptime create()\nfunction main() returns nothing:\n    return nothing\n";

fn checked_program(source: &str) -> Arc<CheckedResourceProgram> {
    let file = FileId::new(0);
    Arc::new(
        CheckedResourceProgram::prepare(
            jett_parser::parse(source, file),
            HashMap::from([(file, SourceOrigin::Project)]),
            &[],
            CheckOptions { release: false },
        )
        .expect("checked actor graph test Source"),
    )
}

fn required(program: &Arc<CheckedResourceProgram>) -> ExplicitComptimeValues {
    let evaluation = crate::evaluate_explicit_comptime_expressions_capture(
        program.module(),
        program.checked().reflection_metadata.clone(),
        Arc::new(CheckedExpressionTypes {
            resource_program: Some(program.clone()),
            expressions: program
                .checked()
                .type_map
                .iter()
                .map(|(span, ty)| (*span, program.checked().interner.type_name(*ty)))
                .collect(),
            ..Default::default()
        }),
        Arc::new(HashMap::new()),
    );
    assert!(
        evaluation.diagnostics.is_empty(),
        "{:?}",
        evaluation.diagnostics
    );
    assert!(evaluation.debug_events.is_empty());
    evaluation.values
}

fn runtime(program: &Arc<CheckedResourceProgram>) -> Interpreter {
    let mut interpreter = Interpreter::new();
    interpreter
        .install_checked_resource_program(program.clone(), ExecutionPurpose::ReferenceRuntime)
        .expect("checked actor import runtime");
    interpreter.register_module(program.module());
    interpreter
}

fn pair(value: &Value) -> (u64, u64) {
    let Value::Struct { fields, .. } = value.payload() else {
        panic!("expected imported Pair")
    };
    let actor = |name: &str| {
        let value = &fields
            .iter()
            .find(|(field, _)| field == name)
            .expect("Pair field")
            .1;
        let Value::Actor(identity) = value.payload() else {
            panic!("expected imported actor")
        };
        *identity
    };
    (actor("left"), actor("right"))
}

#[test]
fn original_escape_has_program_bound_frozen_state_without_events() {
    let program = checked_program(ORIGINAL_ESCAPE);
    let values = required(&program);
    let proofs = values.checked_required_values(&program).unwrap();
    assert_eq!(proofs.len(), 1);
    let graph = proofs[0].actor_graph().expect("escaping actor graph");
    assert!(graph.belongs_to(&program));
    assert_eq!(graph.nodes().len(), 1);
    assert_eq!(graph.nodes()[0].state()[0].value(), &Value::Int64(1));
    let mut interpreter = runtime(&program);
    let imported = interpreter.instantiate_actor_graph(graph).unwrap();
    let Value::Actor(identity) = imported.payload() else {
        panic!("imported actor")
    };
    assert_eq!(
        interpreter.actor_instances[identity].state["count"],
        Value::Int64(1)
    );
    assert!(interpreter.take_debug_events().is_empty());
}

#[test]
fn each_activation_is_fresh_with_shared_aliases_and_frozen_post_message_state() {
    let program = checked_program(SHARED);
    let values = required(&program);
    let proofs = values.checked_required_values(&program).unwrap();
    let graph = proofs[0].actor_graph().unwrap();
    assert_eq!(
        graph.nodes().len(),
        1,
        "two root positions share one seed actor"
    );
    assert_eq!(graph.nodes()[0].state()[0].value(), &Value::Int64(5));
    let mut interpreter = runtime(&program);
    // A prior runtime actor must not collide with an evaluation seed ID.
    interpreter.actor_instances.insert(
        0,
        ActorInstance {
            type_name: "app.Counter".into(),
            state: HashMap::from([("count".into(), Value::Int64(99))]),
            capabilities: HashMap::new(),
        },
    );
    interpreter.next_actor_id = 1;
    let first = interpreter.instantiate_actor_graph(graph).unwrap();
    let (left, right) = pair(&first);
    assert_eq!(left, right);
    assert_ne!(left, 0);
    interpreter
        .actor_instances
        .get_mut(&left)
        .unwrap()
        .state
        .insert("count".into(), Value::Int64(41));
    assert_eq!(
        interpreter.actor_instances[&right].state["count"],
        Value::Int64(41)
    );
    let second = interpreter.instantiate_actor_graph(graph).unwrap();
    let (next_left, next_right) = pair(&second);
    assert_eq!(next_left, next_right);
    assert_ne!(left, next_left);
    assert_eq!(
        interpreter.actor_instances[&next_left].state["count"],
        Value::Int64(5)
    );
    assert_eq!(graph.nodes()[0].state()[0].value(), &Value::Int64(5));
    assert_eq!(
        interpreter.actor_instances[&0].state["count"],
        Value::Int64(99)
    );
}

#[test]
fn failed_unpublished_import_leaves_existing_actors_and_registry_intact() {
    let program = checked_program(SHARED);
    let values = required(&program);
    let proofs = values.checked_required_values(&program).unwrap();
    let mut malformed = (**proofs[0].actor_graph().unwrap()).clone();
    // Only this private child test can forge template storage; public callers
    // have no constructor and cannot create an associated cache proof.
    malformed.nodes[0].state[0].value = Value::Actor(u64::MAX);
    let mut interpreter = runtime(&program);
    assert!(interpreter.instantiate_actor_graph(&malformed).is_err());
    assert!(interpreter.actor_instances.is_empty());
    assert_eq!(
        interpreter.next_actor_id, 1,
        "failed reservations are retired"
    );
    assert!(interpreter.take_debug_events().is_empty());
}

#[test]
fn public_cache_mirror_cannot_mint_or_replace_a_private_actor_graph() {
    let program = checked_program(ORIGINAL_ESCAPE);
    let mut values = required(&program);
    let proofs = values.checked_required_values(&program).unwrap();
    let span = proofs[0].source_span();
    values.insert(span, crate::ComptimeContext::default(), Value::Actor(777));
    assert!(
        values
            .actor_graph(span, &crate::ComptimeContext::default())
            .is_some(),
        "public mirror insertion cannot remove the private runtime snapshot"
    );
    let checked = values.checked_required_values(&program).unwrap();
    assert!(
        checked[0].actor_graph().is_some(),
        "private original association survives a caller mirror"
    );
    let mut forged = ExplicitComptimeValues::default();
    forged.insert(span, crate::ComptimeContext::default(), Value::Actor(0));
    assert!(forged.checked_required_values(&program).unwrap().is_empty());
    assert!(
        forged
            .actor_graph(span, &crate::ComptimeContext::default())
            .is_none()
    );
    let mut interpreter = runtime(&program);
    interpreter.set_explicit_comptime_values(Arc::new(values.clone()));
    assert_eq!(
        interpreter.call_function("app.main", vec![]).unwrap(),
        Value::Nothing
    );
    assert_eq!(
        interpreter.take_debug_events().len(),
        1,
        "original print executes only at runtime"
    );
    let mut interpreter = runtime(&program);
    interpreter.set_explicit_comptime_values(Arc::new(forged));
    assert!(
        interpreter
            .call_function("app.main", vec![])
            .unwrap_err()
            .contains("no private checked frozen graph")
    );
    let foreign = checked_program(ORIGINAL_ESCAPE);
    assert!(values.checked_required_values(&foreign).is_err());
}

#[test]
fn nested_actor_state_and_root_lists_keep_distinct_seed_nodes() {
    let source = "namespace app\nactor Counter:\n    mutable int64 count = 1\nactor Holder:\n    app.Counter child = spawn app.Counter()\nfunction baked() returns list[Holder]:\n    Holder first = spawn Holder()\n    Holder second = spawn Holder()\n    return list(clone first, first, second)\nfunction main() returns nothing:\n    list[Holder] roots = comptime baked()\n    return nothing\n";
    let program = checked_program(source);
    let values = required(&program);
    let proof = values.checked_required_values(&program).unwrap().remove(0);
    let graph = proof.actor_graph().unwrap();
    assert_eq!(
        graph.nodes().len(),
        4,
        "two holders retain two distinct child instances"
    );
    assert_eq!(
        graph
            .nodes()
            .iter()
            .map(|node| node.seed())
            .collect::<Vec<_>>(),
        vec![0, 1, 2, 3],
        "creation order is child-before-holder despite root-first traversal and a shared first holder"
    );
    assert!(graph.nodes()[0].seed() < graph.nodes()[1].seed());
    let mut interpreter = runtime(&program);
    let imported = interpreter.instantiate_actor_graph(graph).unwrap();
    let Value::List(roots) = imported.payload() else {
        panic!("imported list")
    };
    assert_eq!(roots[0], roots[1]);
    assert_eq!(
        roots[0],
        Value::Actor(1),
        "first root holder follows its child creation ordinal"
    );
    assert_eq!(
        roots[2],
        Value::Actor(3),
        "second root follows its distinct child ordinal"
    );
    assert_ne!(roots[0], roots[2]);
    let imported_again = interpreter.instantiate_actor_graph(graph).unwrap();
    assert_ne!(imported, imported_again);
}
#[test]
fn actor_capture_keeps_the_existing_move_only_closure_contract() {
    let source = "namespace app\nactor Counter:\n    mutable int64 count = 1\nfunction create() returns function() returns Counter:\n    Counter worker = spawn Counter()\n    return function() returns Counter: return clone worker\nfunction main() returns nothing:\n    return nothing\n";
    let file = FileId::new(0);
    let error = CheckedResourceProgram::prepare(
        jett_parser::parse(source, file),
        HashMap::from([(file, SourceOrigin::Project)]),
        &[],
        CheckOptions { release: false },
    )
    .expect_err("move-only actor capture is not admitted by the existing source contract");
    assert!(format!("{error:?}").contains("closure cannot capture move-only"));
}
#[test]
fn actor_callback_state_retains_admitted_primitive_captures_without_initializer_replay() {
    let source = "namespace app\nfunction create_callback() returns function() returns string:\n    string text = \"baked\"\n    return function() returns string: return text\nactor Keeper:\n    mutable function() returns string callback = create_callback()\n    receive read responds string:\n        respond callback()\nfunction main() returns nothing:\n    Keeper worker = comptime spawn Keeper()\n    string text = ask worker.read\n    println(text)\n    return nothing\n";
    let program = checked_program(source);
    let values = required(&program);
    let proof = values.checked_required_values(&program).unwrap().remove(0);
    let graph = proof.actor_graph().unwrap();
    assert_eq!(graph.nodes().len(), 1);
    let field = &graph.nodes()[0].state()[0];
    let Value::Function { type_context, .. } = field.value().payload() else {
        panic!("frozen callback")
    };
    let original = type_context
        .reflected_capture
        .as_ref()
        .expect("original sealed capture");
    let returned = type_context
        .return_type
        .clone()
        .expect("callback return type");
    crate::value::ReflectedCallableValue::authenticate_source(field.value(), &program, field.ty())
        .unwrap();
    let mut interpreter = runtime(&program);
    let imported = interpreter.instantiate_actor_graph(graph).unwrap();
    let Value::Actor(id) = imported.payload() else {
        panic!("actor")
    };
    let Value::Function {
        captures,
        type_context,
        ..
    } = interpreter.actor_instances[id].state["callback"].payload()
    else {
        panic!("callback")
    };
    assert_eq!(captures["text"], Value::String("baked".into()));
    assert!(Arc::ptr_eq(
        original,
        type_context.reflected_capture.as_ref().unwrap()
    ));

    // A private child can compose the same checked source with its exact plan.
    // Import must retain that opaque adapter and witness without Source replay.
    let adapter = Value::ReflectedCallable(Box::new(
        crate::value::ReflectedCallableValue::new(
            field.value().clone(),
            program.clone(),
            field.ty(),
            field.ty(),
            returned.clone(),
            returned,
        )
        .unwrap(),
    ));
    assert_eq!(
        interpreter
            .actor_graph_children(&adapter, field.ty(), &program.checked().interner, &program)
            .unwrap()
            .len(),
        1
    );
    let foreign = checked_program(source);
    assert!(
        interpreter
            .actor_graph_children(&adapter, field.ty(), &foreign.checked().interner, &foreign)
            .is_err()
    );
    let mut adapted = (**graph).clone();
    adapted.nodes[0].state[0].value = adapter;
    let imported = interpreter.instantiate_actor_graph(&adapted).unwrap();
    let Value::Actor(id) = imported.payload() else {
        panic!("adapted actor")
    };
    let Value::ReflectedCallable(adapter) =
        interpreter.actor_instances[id].state["callback"].payload()
    else {
        panic!("retained adapter")
    };
    let Value::Function { type_context, .. } = adapter.source().payload() else {
        panic!("retained source")
    };
    assert!(Arc::ptr_eq(
        original,
        type_context.reflected_capture.as_ref().unwrap()
    ));
    crate::value::ReflectedCallableValue::authenticate_source(
        adapter.source(),
        &program,
        adapter.source_type(),
    )
    .unwrap();
    assert!(interpreter.take_debug_events().is_empty());
}
