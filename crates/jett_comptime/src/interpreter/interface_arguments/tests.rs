use super::*;
use crate::debug::render_debug_events;
use jett_common::FileId;

const ORIGINAL_WRONG: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/native/erased_same_owner/05_erased_other_owner.jett"
));
const ORIGINAL_MATCHING: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/native/erased_same_owner/12_erased_matching_owner.jett"
));

fn interpreter(source: &str) -> Interpreter {
    let parsed = jett_parser::parse(source, FileId::new(0));
    assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
    let mut interpreter = Interpreter::new();
    interpreter.register_module(&parsed.module);
    interpreter
}

fn named(name: &str) -> TypeExpr {
    TypeExpr::Named(super::super::Ident {
        name: name.into(),
        span: super::super::Span::new(FileId::new(0), 0, 0),
    })
}

fn generic(name: &str, arguments: Vec<TypeExpr>) -> TypeExpr {
    TypeExpr::Generic(
        super::super::Ident {
            name: name.into(),
            span: super::super::Span::new(FileId::new(0), 0, 0),
        },
        arguments,
        super::super::Span::new(FileId::new(0), 0, 0),
    )
}

fn record(owner: &str) -> Value {
    Value::Struct {
        type_name: owner.into(),
        concrete_type: None,
        fields: vec![("name".into(), Value::String("label".into()))],
    }
}

#[test]
fn erased_owner_original_wrong_source_fails_before_result_print() {
    let mut interpreter = interpreter(ORIGINAL_WRONG);
    assert_eq!(
        interpreter.call_function("app.main", vec![]),
        Err(OWNER_MISMATCH.into())
    );
    assert!(interpreter.take_debug_events().is_empty());
    assert_eq!(interpreter.lexical_scope_floor, 0);
    assert_eq!(interpreter.current_namespace, None);
}

#[test]
fn erased_owner_original_matching_source_keeps_views_and_result_print() {
    let mut interpreter = interpreter(ORIGINAL_MATCHING);
    assert_eq!(
        interpreter.call_function("app.main", vec![]),
        Ok(Value::Nothing)
    );
    assert_eq!(
        render_debug_events(&interpreter.take_debug_events()),
        "left:one:two\n"
    );
}

#[test]
fn erased_owner_direct_and_named_callback_reject_before_body_entry() {
    let source = ORIGINAL_WRONG.replace(
        "return \"left:{self.name}:{other.name}\"",
        "println(\"entered-left-body\")\n        return \"left:{self.name}:{other.name}\"",
    );
    for callback in [false, true] {
        let mut interpreter = interpreter(&source);
        let args = vec![record("app.Left"), record("app.Right")];
        let outcome = if callback {
            interpreter.call_fn_value(Value::NamedFunction("app.Pairable.combine".into()), args)
        } else {
            interpreter.call_function("app.Pairable.combine", args)
        };
        assert_eq!(outcome, Err(OWNER_MISMATCH.into()));
        assert!(interpreter.take_debug_events().is_empty());
        assert_eq!(interpreter.current_namespace, None);
        assert_eq!(interpreter.lexical_scope_floor, 0);
    }
}

#[test]
fn erased_owner_owned_formals_validate_before_installing_any_formal() {
    let source = ORIGINAL_WRONG
        .replace("view self:", "self:")
        .replace("view other:", "other:");
    let mut interpreter = interpreter(&source);
    let scopes = interpreter.scopes.len();
    assert_eq!(
        interpreter.call_function(
            "app.Pairable.combine",
            vec![record("app.Left"), record("app.Right")]
        ),
        Err(OWNER_MISMATCH.into())
    );
    assert_eq!(interpreter.scopes.len(), scopes);
    assert!(interpreter.take_debug_events().is_empty());
}

#[test]
fn erased_owner_primitive_widths_and_nominal_refinements_are_exact() {
    let interpreter = interpreter(ORIGINAL_MATCHING);
    for owner in ["int8", "app.Positive"] {
        let wrong = Value::Int64(7);
        let right = Value::Typed {
            type_name: owner.into(),
            value: Box::new(wrong.clone()),
        };
        assert_eq!(
            interpreter.validate_interface_owner_occurrence(&named(owner), owner, &right),
            Ok(())
        );
        assert_eq!(
            interpreter.validate_interface_owner_occurrence(&named(owner), owner, &wrong),
            Err(OWNER_MISMATCH.into())
        );
        assert_eq!(runtime_type_name(&right).as_deref(), Some(owner));
    }
}

#[test]
fn erased_owner_generic_record_arguments_keep_full_nominal_identity() {
    let interpreter = interpreter(ORIGINAL_MATCHING);
    let owner = "app.Box[int64]";
    let mut actual = record("app.Box");
    if let Value::Struct { concrete_type, .. } = &mut actual {
        *concrete_type = Some(owner.into());
    }
    let expected = generic("app.Box", vec![named("int64")]);
    assert_eq!(
        interpreter.validate_interface_owner_occurrence(&expected, owner, &actual),
        Ok(())
    );
    if let Value::Struct { concrete_type, .. } = &mut actual {
        *concrete_type = Some("app.Box[string]".into());
    }
    assert_eq!(
        interpreter.validate_interface_owner_occurrence(&expected, owner, &actual),
        Err(OWNER_MISMATCH.into())
    );
}

#[test]
fn erased_owner_nested_occupied_payloads_require_matching_owners() {
    let interpreter = interpreter(ORIGINAL_MATCHING);
    let owner = "app.Left";
    let expected = generic("list", vec![generic("optional", vec![named(owner)])]);
    for actual in [
        Value::List(vec![Value::OptionalSome(Box::new(record(owner)))]),
        Value::List(vec![Value::OptionalNone]),
        Value::List(vec![]),
    ] {
        assert_eq!(
            interpreter.validate_interface_owner_occurrence(&expected, owner, &actual),
            Ok(())
        );
    }
    let wrong = Value::List(vec![Value::OptionalSome(Box::new(record("app.Right")))]);
    assert_eq!(
        interpreter.validate_interface_owner_occurrence(&expected, owner, &wrong),
        Err(OWNER_MISMATCH.into())
    );
}

#[test]
fn erased_owner_map_values_and_active_result_arms_are_checked() {
    let interpreter = interpreter(ORIGINAL_MATCHING);
    let owner = "app.Left";
    let expected = generic(
        "map",
        vec![
            named("string"),
            generic("result", vec![named(owner), named("string")]),
        ],
    );
    let unchanged_error = Value::Map(vec![(
        Value::String("key".into()),
        Value::ResultFail(Box::new(Value::String("earlier".into()))),
    )]);
    assert_eq!(
        interpreter.validate_interface_owner_occurrence(&expected, owner, &unchanged_error),
        Ok(())
    );
    let wrong = Value::Map(vec![(
        Value::String("key".into()),
        Value::ResultOk(Box::new(record("app.Right"))),
    )]);
    assert_eq!(
        interpreter.validate_interface_owner_occurrence(&expected, owner, &wrong),
        Err(OWNER_MISMATCH.into())
    );
}

#[test]
fn erased_owner_pending_receiver_keeps_canonical_dispatch_failure() {
    let mut interpreter = interpreter(ORIGINAL_MATCHING);
    let pending = Value::Pending(Box::new(record("app.Left")));
    let typed_pending = Value::Typed {
        type_name: "app.Left".into(),
        value: Box::new(pending.clone()),
    };
    for receiver in [pending, typed_pending] {
        for callback in [false, true] {
            let args = vec![receiver.clone(), record("app.Right")];
            let actual = if callback {
                interpreter.call_fn_value(Value::NamedFunction("app.Pairable.combine".into()), args)
            } else {
                interpreter.call_function("app.Pairable.combine", args)
            };
            assert_eq!(
                actual,
                Err("undefined function 'app.Pairable.combine'".into())
            );
        }
    }
    assert!(interpreter.take_debug_events().is_empty());
}

#[test]
fn erased_owner_pending_argument_uses_identity_without_joining_payload() {
    let interpreter = interpreter(ORIGINAL_MATCHING);
    let owner = "app.Left";
    let matching = Value::Pending(Box::new(record(owner)));
    let wrong = Value::Pending(Box::new(record("app.Right")));
    assert_eq!(
        interpreter.validate_interface_owner_occurrence(&named(owner), owner, &matching),
        Ok(())
    );
    assert_eq!(
        interpreter.validate_interface_owner_occurrence(&named(owner), owner, &wrong),
        Err(OWNER_MISMATCH.into())
    );
    assert!(matches!(matching, Value::Pending(_)));
    assert!(matches!(wrong, Value::Pending(_)));
}

#[test]
fn erased_owner_preflight_does_not_expand_caller_namespace_aliases() {
    let mut interpreter = interpreter(ORIGINAL_MATCHING);
    let parsed = jett_parser::parse(
        "namespace fake\nstruct Left:\n    name: string\n",
        FileId::new(1),
    );
    assert!(parsed.errors.is_empty());
    interpreter.register_module(&parsed.module);
    interpreter
        .namespace_alias_scopes
        .last_mut()
        .unwrap()
        .insert("app".into(), "fake".into());
    let declared = named("app.Left");
    assert_eq!(
        interpreter.resolved_concrete_type_display(&interpreter.interface_parameter_type(
            &declared,
            &HashMap::new(),
            Some("app")
        )),
        "app.Left"
    );
    let declared = named("Left");
    assert_eq!(
        interpreter.resolved_concrete_type_display(&interpreter.interface_parameter_type(
            &declared,
            &HashMap::new(),
            Some("app")
        )),
        "app.Left"
    );
    assert_eq!(
        interpreter
            .namespace_alias_scopes
            .last()
            .unwrap()
            .get("app")
            .map(String::as_str),
        Some("fake")
    );
}
