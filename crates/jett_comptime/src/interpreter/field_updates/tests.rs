//! Host storage controls do not establish Source admission,
//! checked-place authenticity, evaluator order, ownership proof, or cleanup.
use super::*;

fn inner(value: Value) -> Value {
    Value::Struct {
        type_name: "app.Inner".into(),
        concrete_type: None,
        fields: vec![("value".into(), value)],
    }
}

fn record(value: Value) -> Value {
    Value::Struct {
        type_name: "app.Record".into(),
        concrete_type: None,
        fields: vec![
            ("inner".into(), value),
            ("label".into(), Value::String("before".into())),
        ],
    }
}

fn struct_projection(type_name: &str, fields: &[&str], field_index: usize) -> StorageProjection {
    StorageProjection {
        wrappers: Vec::new(),
        owner: StorageOwner::Struct {
            type_name: type_name.into(),
            concrete_type: None,
            field_names: fields.iter().map(|name| (*name).into()).collect(),
        },
        field_index,
    }
}

fn nested_plan() -> FieldStoragePlan {
    FieldStoragePlan {
        projections: vec![
            struct_projection("app.Record", &["inner", "label"], 0),
            struct_projection("app.Inner", &["value"], 0),
        ],
    }
}

fn staged(value: Value) -> StagedOrdinaryReplacement {
    StagedOrdinaryReplacement { value }
}

#[test]
fn nested_replacement_returns_displaced_owner_and_preserves_siblings() {
    let mut root = record(inner(Value::String("old".into())));
    let displaced = nested_plan()
        .replace(&mut root, staged(Value::String("new".into())))
        .unwrap();
    assert_eq!(displaced, Value::String("old".into()));
    assert_eq!(root, record(inner(Value::String("new".into()))));
}

#[test]
fn independently_owned_clone_is_unchanged_by_replacement() {
    let original = record(inner(Value::List(vec![Value::Int64(1)])));
    let mut copied = original.clone();
    let displaced = nested_plan()
        .replace(&mut copied, staged(Value::List(vec![Value::Int64(9)])))
        .unwrap();
    assert_eq!(original, record(inner(Value::List(vec![Value::Int64(1)]))));
    assert_eq!(copied, record(inner(Value::List(vec![Value::Int64(9)]))));
    assert_eq!(displaced, Value::List(vec![Value::Int64(1)]));
}

#[test]
fn pending_root_and_intermediate_are_rejected_without_payload_observation() {
    let pending_root = Value::Pending(Box::new(record(inner(Value::Int64(1)))));
    assert_eq!(
        nested_plan().preflight(&pending_root),
        Err(FieldStorageError::PendingAncestor { projection: 0 })
    );
    let mut pending_inner = record(Value::Pending(Box::new(inner(Value::Int64(1)))));
    let before = pending_inner.clone();
    let failure = nested_plan()
        .replace(&mut pending_inner, staged(Value::Int64(9)))
        .unwrap_err();
    assert_eq!(
        failure.error,
        FieldStorageError::PendingAncestor { projection: 1 }
    );
    assert_eq!(failure.replacement.value, Value::Int64(9));
    assert_eq!(pending_inner, before);
}

#[test]
fn replacing_child_transports_exact_pending_depth() {
    let mut root = record(inner(Value::Int64(1)));
    let pending = Value::Pending(Box::new(Value::Pending(Box::new(Value::Int64(7)))));
    let displaced = nested_plan().replace(&mut root, staged(pending)).unwrap();
    assert_eq!(displaced, Value::Int64(1));
    let Value::Struct { fields, .. } = root else {
        panic!("record")
    };
    let Value::Struct { fields, .. } = &fields[0].1 else {
        panic!("inner")
    };
    let Value::Pending(once) = &fields[0].1 else {
        panic!("first pending")
    };
    let Value::Pending(ready) = once.as_ref() else {
        panic!("second pending")
    };
    assert_eq!(ready.as_ref(), &Value::Int64(7));
}

#[test]
fn untouched_pending_child_keeps_depth_and_storage_metadata() {
    let pending = Value::Pending(Box::new(Value::Pending(Box::new(inner(Value::Int64(4))))));
    let mut root = record(pending.clone());
    let plan = FieldStoragePlan {
        projections: vec![struct_projection("app.Record", &["inner", "label"], 1)],
    };
    plan.replace(&mut root, staged(Value::String("after".into())))
        .unwrap();
    assert_eq!(
        root,
        Value::Struct {
            type_name: "app.Record".into(),
            concrete_type: None,
            fields: vec![
                ("inner".into(), pending),
                ("label".into(), Value::String("after".into()))
            ],
        }
    );
}

#[test]
fn exact_typed_wrappers_and_generic_identity_are_preserved() {
    let root_value = Value::Struct {
        type_name: "app.Boxed".into(),
        concrete_type: Some("app.Boxed[int64]".into()),
        fields: vec![("value".into(), Value::Int64(1))],
    };
    let mut root = Value::Typed {
        type_name: "secret[app.Boxed[int64]]".into(),
        value: Box::new(root_value),
    };
    let plan = FieldStoragePlan {
        projections: vec![StorageProjection {
            wrappers: vec!["secret[app.Boxed[int64]]".into()],
            owner: StorageOwner::Struct {
                type_name: "app.Boxed".into(),
                concrete_type: Some("app.Boxed[int64]".into()),
                field_names: vec!["value".into()],
            },
            field_index: 0,
        }],
    };
    plan.replace(&mut root, staged(Value::Int64(8))).unwrap();
    let Value::Typed { type_name, value } = root else {
        panic!("wrapper")
    };
    assert_eq!(type_name, "secret[app.Boxed[int64]]");
    let Value::Struct {
        concrete_type,
        fields,
        ..
    } = *value
    else {
        panic!("record")
    };
    assert_eq!(concrete_type.as_deref(), Some("app.Boxed[int64]"));
    assert_eq!(fields[0].1, Value::Int64(8));
}

#[test]
fn exact_machine_state_is_checked_and_unchanged_by_nested_update() {
    let mut root = Value::Machine {
        type_name: "app.Session".into(),
        state: "open".into(),
        fields: vec![inner(Value::Int64(1))],
    };
    let plan = FieldStoragePlan {
        projections: vec![
            StorageProjection {
                wrappers: Vec::new(),
                owner: StorageOwner::MachineState {
                    type_name: "app.Session".into(),
                    state: "open".into(),
                    field_count: 1,
                },
                field_index: 0,
            },
            struct_projection("app.Inner", &["value"], 0),
        ],
    };
    plan.replace(&mut root, staged(Value::Int64(2))).unwrap();
    assert_eq!(
        root,
        Value::Machine {
            type_name: "app.Session".into(),
            state: "open".into(),
            fields: vec![inner(Value::Int64(2))],
        }
    );
    let Value::Machine { state, .. } = &mut root else {
        panic!("machine")
    };
    *state = "closed".into();
    let before = root.clone();
    let failure = plan
        .replace(&mut root, staged(Value::Int64(3)))
        .unwrap_err();
    assert_eq!(
        failure.error,
        FieldStorageError::OwnerMismatch { projection: 0 }
    );
    assert_eq!(root, before);
}

#[test]
fn missing_path_mismatched_qualification_owner_or_field_leave_root_intact() {
    let root = record(inner(Value::Int64(1)));
    let empty = FieldStoragePlan {
        projections: Vec::new(),
    };
    assert_eq!(empty.preflight(&root), Err(FieldStorageError::EmptyPath));
    let mut wrong_owner = nested_plan();
    wrong_owner.projections[0].owner = StorageOwner::Struct {
        type_name: "foreign.Record".into(),
        concrete_type: None,
        field_names: vec!["inner".into(), "label".into()],
    };
    let mut wrong_wrapper = nested_plan();
    wrong_wrapper.projections[0].wrappers = vec!["app.Record".into()];
    let mut wrong_field = nested_plan();
    wrong_field.projections[0].field_index = 5;
    for plan in [wrong_owner, wrong_wrapper, wrong_field] {
        let mut target = root.clone();
        let failure = plan
            .replace(&mut target, staged(Value::Int64(9)))
            .unwrap_err();
        assert_eq!(failure.replacement.value, Value::Int64(9));
        assert_eq!(target, root);
    }
}
