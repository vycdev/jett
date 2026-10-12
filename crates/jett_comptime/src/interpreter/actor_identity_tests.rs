use super::*;

fn actor(type_name: &str, state: i64) -> ActorInstance {
    ActorInstance {
        type_name: type_name.into(),
        state: HashMap::from([("count".into(), Value::Int64(state))]),
        capabilities: HashMap::new(),
    }
}

#[test]
fn actor_identity_uses_live_nominal_instances_without_reading_state() {
    let mut interpreter = Interpreter::new();
    interpreter
        .actor_instances
        .insert(0, actor("app.Counter", 7));
    interpreter
        .actor_instances
        .insert(1, actor("app.Counter", 7));
    interpreter.actor_instances.insert(2, actor("app.Other", 7));
    assert_eq!(interpreter.actor_identity_equal(0, 0), Ok(true));
    assert_eq!(interpreter.actor_identity_equal(0, 1), Ok(false));
    interpreter
        .actor_instances
        .get_mut(&0)
        .unwrap()
        .state
        .insert("count".into(), Value::Int64(88));
    assert_eq!(interpreter.actor_identity_equal(0, 0), Ok(true));
    assert_eq!(interpreter.actor_identity_equal(0, 1), Ok(false));
    assert_eq!(
        interpreter.actor_identity_equal(0, 2),
        Err("actor equality requires matching nominal actor types".into())
    );
    assert_eq!(
        interpreter.actor_identity_equal(99, 99),
        Err("actor equality: unknown actor instance #99".into())
    );
    interpreter.actor_instances.remove(&0);
    assert_eq!(
        interpreter.actor_identity_equal(0, 0),
        Err("actor equality: unknown actor instance #0".into())
    );
}

#[test]
fn actor_identity_enum_payloads_validate_liveness_and_preserve_pending_depth() {
    let mut interpreter = Interpreter::new();
    interpreter
        .actor_instances
        .insert(0, actor("app.Counter", 7));
    interpreter
        .actor_instances
        .insert(1, actor("app.Counter", 7));
    let same = Value::Actor(0);
    let distinct = Value::Actor(1);
    assert_eq!(interpreter.enum_payload_equal(&same, &same), Ok(true));
    assert_eq!(interpreter.enum_payload_equal(&same, &distinct), Ok(false));
    let pending = Value::Pending(Box::new(same.clone()));
    assert_eq!(interpreter.enum_payload_equal(&pending, &pending), Ok(true));
    assert_eq!(interpreter.enum_payload_equal(&same, &pending), Ok(false));
    assert_eq!(
        interpreter.enum_payload_equal(&Value::Actor(99), &Value::Actor(99)),
        Err("actor equality: unknown actor instance #99".into())
    );
}
