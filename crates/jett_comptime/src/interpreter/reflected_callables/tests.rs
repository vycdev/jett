use super::*;
use jett_common::{FileId, SourceOrigin};
use jett_typecheck::{CheckOptions, CheckedResourceProgram};

const SOURCE: &str = r#"namespace app
function positive_check(value: int64) returns bool:
    string marker = "predicate"
    trace marker
    return value > 0
type Positive = int64 where positive_check(value)
function requested(view callback: function() returns Positive) returns nothing:
    return nothing
function five() returns int64:
    string marker = "source"
    trace marker
    return 5
function zero() returns int64:
    string marker = "source"
    trace marker
    return 0
function delayed() returns int64:
    string marker = "source"
    trace marker
    return run 5
function captured(offset: int64) returns function() returns int64:
    function() returns int64 callback = function() returns int64:
        string marker = "source"
        trace marker
        return offset
    return callback
function requested_parameter(view callback: function(int64) returns Positive) returns nothing:
    return nothing
function captured_parameter(offset: int64) returns function(int64) returns int64:
    function(int64) returns int64 callback = function(value: int64) returns int64:
        string marker = "source"
        trace marker
        return value + offset
    return callback
function primitive_step(value: int64) returns int64:
    string marker = "primitive"
    trace marker
    return value + 1
function captured_after_primitive(offset: int64) returns function() returns int64:
    int64 adjusted = primitive_step(offset)
    function() returns int64 callback = function() returns int64:
        string marker = "source"
        trace marker
        return adjusted
    return callback
function captured_factory() returns function(int64) returns function() returns int64:
    function(int64) returns function() returns int64 factory = function(offset: int64) returns function() returns int64:
        int64 adjusted = primitive_step(offset)
        function() returns int64 callback = function() returns int64:
            string marker = "source"
            trace marker
            return adjusted
        return callback
    return factory
function forward_factory(view factory: function(int64) returns function() returns int64, offset: int64) returns function() returns int64:
    return factory(offset)
"#;

fn prepared() -> (Interpreter, Arc<CheckedResourceProgram>, TypeExpr) {
    let file = FileId::new(0);
    let program = Arc::new(
        CheckedResourceProgram::prepare(
            jett_parser::parse(SOURCE, file),
            HashMap::from([(file, SourceOrigin::Project)]),
            &[],
            CheckOptions::default(),
        )
        .unwrap(),
    );
    let mut interpreter = Interpreter::new();
    interpreter.register_module(program.module());
    interpreter.set_reflection_metadata(program.checked().reflection_metadata.clone());
    interpreter.set_checked_expression_types(Arc::new(CheckedExpressionTypes {
        resource_program: Some(program.clone()),
        expressions: program
            .checked()
            .type_map
            .iter()
            .map(|(span, ty)| (*span, program.checked().interner.type_name(*ty)))
            .collect(),
        ..Default::default()
    }));
    let requested = Interpreter::simple_type_expr_from_name(
        "function() returns app.Positive",
        Span::new(file, 0, 0),
    )
    .unwrap();
    (interpreter, program, requested)
}

fn adapter(interpreter: &Interpreter, source: Value, requested: &TypeExpr) -> Value {
    interpreter
        .prepare_reflected_callable(source, requested, "function() returns int64")
        .unwrap()
}

fn traces(interpreter: &mut Interpreter) -> Vec<String> {
    interpreter
        .take_debug_events()
        .into_iter()
        .map(|event| event.text)
        .collect()
}

#[test]
fn reflected_callable_field_relation_requires_same_checked_metadata_and_exact_signature_plan() {
    let (mut interpreter, _, requested) = prepared();
    assert!(
        interpreter.has_checked_reflected_callable_plan("function() returns int64", &requested)
    );
    assert!(
        !interpreter
            .has_checked_reflected_callable_plan("function(int64) returns bool", &requested)
    );
    let (_, foreign, _) = prepared();
    interpreter.set_reflection_metadata(foreign.checked().reflection_metadata.clone());
    assert!(
        !interpreter.has_checked_reflected_callable_plan("function() returns int64", &requested)
    );
    assert!(traces(&mut interpreter).is_empty());
}

#[test]
fn reflected_callable_read_is_lazy_and_each_call_owns_one_new_result_check() {
    let (mut interpreter, _, requested) = prepared();
    let observed = adapter(
        &interpreter,
        Value::NamedFunction("app.five".into()),
        &requested,
    );
    assert!(traces(&mut interpreter).is_empty());
    for _ in 0..2 {
        let result = interpreter
            .call_fn_value(observed.clone(), Vec::new())
            .unwrap();
        assert_eq!(result.payload(), &Value::Int64(5));
        assert_eq!(
            traces(&mut interpreter),
            [
                "trace marker: string = source\n",
                "trace marker: string = predicate\n",
            ]
        );
    }
}

#[test]
fn reflected_callable_keeps_captured_environment_and_original_debug_identity() {
    let (mut interpreter, _, requested) = prepared();
    let source = interpreter
        .call_function("app.captured", vec![Value::Int64(7)])
        .unwrap();
    let label = source.to_string();
    let observed = adapter(&interpreter, source, &requested);
    assert_eq!(observed.to_string(), label);
    assert!(traces(&mut interpreter).is_empty());
    let result = interpreter.call_fn_value(observed, Vec::new()).unwrap();
    assert_eq!(result.payload(), &Value::Int64(7));
    assert_eq!(traces(&mut interpreter).len(), 2);
}

#[test]
fn reflected_callable_failed_or_pending_result_never_publishes_target_proof() {
    let (mut interpreter, _, requested) = prepared();
    let failed = adapter(
        &interpreter,
        Value::NamedFunction("app.zero".into()),
        &requested,
    );
    let error = interpreter.call_fn_value(failed, Vec::new()).unwrap_err();
    assert!(
        error.contains("refinement type constraint failed for 'app.Positive'"),
        "{error}"
    );
    assert_eq!(traces(&mut interpreter).len(), 2);
    let pending = adapter(
        &interpreter,
        Value::NamedFunction("app.delayed".into()),
        &requested,
    );
    assert_eq!(
        interpreter.call_fn_value(pending, Vec::new()).unwrap_err(),
        "reflected callback return: expected a ready value"
    );
    assert_eq!(
        traces(&mut interpreter),
        ["trace marker: string = source\n"]
    );
}

#[test]
fn reflected_callable_exact_pending_transport_and_foreign_program_refusal_stay_distinct() {
    let (mut interpreter, program, requested) = prepared();
    let value = Value::Pending(Box::new(Value::NamedFunction("app.five".into())));
    let exact =
        Interpreter::simple_type_expr_from_name("function() returns int64", requested.span())
            .unwrap();
    let unchanged = interpreter
        .prepare_reflected_callable(value.clone(), &exact, "function() returns int64")
        .unwrap();
    assert_eq!(unchanged.to_string(), value.to_string());
    assert!(!matches!(unchanged, Value::ReflectedCallable(_)));
    assert!(traces(&mut interpreter).is_empty());
    assert!(
        interpreter
            .prepare_reflected_callable(value, &requested, "function() returns app.Positive")
            .unwrap_err()
            .contains("original source declaration")
    );
    let observed = adapter(
        &interpreter,
        Value::NamedFunction("app.five".into()),
        &requested,
    );
    let (mut foreign, other, _) = prepared();
    assert!(!Arc::ptr_eq(&program, &other));
    assert_eq!(
        foreign.call_fn_value(observed, Vec::new()).unwrap_err(),
        "reflected callback invocation belongs to another checked program"
    );
    assert!(traces(&mut foreign).is_empty());
    assert!(
        interpreter
            .prepare_reflected_callable(Value::Int64(5), &requested, "function() returns int64",)
            .unwrap_err()
            .contains("not a callable descriptor")
    );
}

fn refuse_forged_capture(interpreter: &mut Interpreter, source: Value, requested: &TypeExpr) {
    let error = interpreter
        .prepare_reflected_callable(source, requested, "function() returns int64")
        .unwrap_err();
    assert!(
        error.contains("unchanged original closure capture"),
        "{error}"
    );
    assert!(traces(interpreter).is_empty());
}

#[test]
fn reflected_closure_same_span_body_and_header_forgery_refuse_before_effects() {
    let (mut interpreter, program, requested) = prepared();
    let genuine = interpreter
        .call_function("app.captured", vec![Value::Int64(7)])
        .unwrap();
    let mut forged = genuine.clone();
    let Value::Function { body, .. } = &mut forged else {
        panic!("expected closure");
    };
    let Stmt::Return(statement) = body.stmts.last_mut().unwrap() else {
        panic!("expected return");
    };
    let span = statement.value.as_ref().unwrap().span();
    statement.value = Some(Expr::IntLiteral(11, span));
    refuse_forged_capture(&mut interpreter, forged, &requested);

    let mut forged = genuine.clone();
    let Value::Function { params, .. } = &mut forged else {
        panic!("expected closure");
    };
    let original_parameter = program
        .module()
        .items
        .iter()
        .find_map(|item| match item {
            Item::Function(function) if function.name.name == "captured" => {
                function.params.first().cloned()
            }
            _ => None,
        })
        .unwrap();
    params.push(original_parameter);
    refuse_forged_capture(&mut interpreter, forged, &requested);

    let mut forged = genuine.clone();
    let Value::Function { type_context, .. } = &mut forged else {
        panic!("expected closure");
    };
    let TypeExpr::Named(name) = type_context.return_type.as_mut().unwrap() else {
        panic!("expected named return");
    };
    name.name = "bool".into();
    refuse_forged_capture(&mut interpreter, forged, &requested);
    let observed = adapter(&interpreter, genuine, &requested);
    for _ in 0..2 {
        assert_eq!(
            interpreter
                .call_fn_value(observed.clone(), Vec::new())
                .unwrap()
                .payload(),
            &Value::Int64(7)
        );
        assert_eq!(traces(&mut interpreter).len(), 2);
    }
}

#[test]
fn reflected_closure_same_span_parameter_contract_mutation_refuses_before_effects() {
    let (mut interpreter, _, requested) = prepared();
    let requested = Interpreter::simple_type_expr_from_name(
        "function(int64) returns app.Positive",
        requested.span(),
    )
    .unwrap();
    let source = interpreter
        .call_function("app.captured_parameter", vec![Value::Int64(7)])
        .unwrap();
    for mutation in 0..4 {
        let mut forged = source.clone();
        let Value::Function { params, .. } = &mut forged else {
            panic!("expected closure");
        };
        let param = &mut params[0];
        match mutation {
            0 => param.view = true,
            1 => param.mutable = true,
            2 => param.name.name = "other".into(),
            3 => {
                let TypeExpr::Named(name) = &mut param.ty else {
                    panic!("expected named type");
                };
                name.name = "bool".into();
            }
            _ => unreachable!(),
        }
        let error = interpreter
            .prepare_reflected_callable(forged, &requested, "function(int64) returns int64")
            .unwrap_err();
        assert!(
            error.contains("unchanged original closure capture"),
            "{error}"
        );
        assert!(traces(&mut interpreter).is_empty());
    }
    let observed = interpreter
        .prepare_reflected_callable(source, &requested, "function(int64) returns int64")
        .unwrap();
    assert!(traces(&mut interpreter).is_empty());
    assert_eq!(
        interpreter
            .call_fn_value(observed, vec![Value::Int64(2)])
            .unwrap()
            .payload(),
        &Value::Int64(9)
    );
    assert_eq!(traces(&mut interpreter).len(), 2);
}

#[test]
fn reflected_closure_capture_types_context_and_alias_forgery_refuse_before_effects() {
    let (mut interpreter, _, requested) = prepared();
    let genuine = interpreter
        .call_function("app.captured", vec![Value::Int64(7)])
        .unwrap();
    for mutation in 0..8 {
        let mut forged = genuine.clone();
        let Value::Function {
            type_context,
            captures,
            capture_types,
            namespace_aliases,
            namespace,
            ..
        } = &mut forged
        else {
            panic!("expected closure");
        };
        match mutation {
            0 => {
                captures.insert("offset".into(), Value::Int64(0));
            }
            1 => {
                captures.insert("extra".into(), Value::Nothing);
            }
            2 => {
                let TypeExpr::Named(name) = capture_types.get_mut("offset").unwrap() else {
                    panic!("expected capture type");
                };
                name.name = "bool".into();
            }
            3 => {
                type_context.bindings.insert(
                    "Foreign".into(),
                    TypeExpr::Named(Ident {
                        name: "bool".into(),
                        span: requested.span(),
                    }),
                );
            }
            4 => {
                type_context.checked_scope = Some(Arc::new(CheckedScopedTypes::default()));
            }
            5 => {
                type_context.checked_function = Some(Arc::new(CheckedFunctionTypes::default()));
            }
            6 => {
                namespace_aliases.insert("changed".into(), "other".into());
            }
            7 => {
                *namespace = Some("other".into());
            }
            _ => unreachable!(),
        }
        refuse_forged_capture(&mut interpreter, forged, &requested);
    }
    let (_, foreign, _) = prepared();
    let Value::Function { type_context, .. } = &genuine else {
        panic!("expected closure");
    };
    let actual = foreign
        .checked()
        .reflection_metadata
        .source_type_id_for_name("function() returns int64")
        .unwrap();
    assert!(
        !type_context
            .reflected_capture
            .as_ref()
            .unwrap()
            .validate_source(&foreign, actual, &genuine)
    );
    assert!(traces(&mut interpreter).is_empty());
}

#[test]
fn reflected_closure_changed_source_owner_cannot_mint_a_witness() {
    let (mut interpreter, program, requested) = prepared();
    let original = program
        .module()
        .items
        .iter()
        .find_map(|item| match item {
            Item::Function(function) if function.name.name == "captured" => Some(function.clone()),
            _ => None,
        })
        .unwrap();
    let mut changed = original.clone();
    // Keep the original inline node and all source spans while changing its
    // enclosing owner. An exact subtree must not authenticate a changed owner.
    changed.body.stmts.insert(
        0,
        Stmt::Expr(jett_parser::ast::ExprStmt {
            expr: Expr::Nothing(changed.body.span),
            span: changed.body.span,
        }),
    );
    interpreter.register_function_in_namespace(Some("app"), &changed);
    let raw = interpreter
        .call_function("app.captured", vec![Value::Int64(7)])
        .unwrap();
    refuse_forged_capture(&mut interpreter, raw.clone(), &requested);
    // Ordinary closure execution remains available without reflected authority.
    assert_eq!(
        interpreter.call_fn_value(raw, Vec::new()).unwrap(),
        Value::Int64(7)
    );
    assert_eq!(traces(&mut interpreter).len(), 1);

    let mut transplanted = original;
    transplanted.name.name = "five".into();
    interpreter.register_function_in_namespace(Some("app"), &transplanted);
    let raw = interpreter
        .call_function("app.five", vec![Value::Int64(7)])
        .unwrap();
    refuse_forged_capture(&mut interpreter, raw, &requested);
}

#[test]
fn reflected_closure_forged_installed_checked_context_cannot_mint_a_witness() {
    let (mut interpreter, program, requested) = prepared();
    let declaration = program
        .module()
        .items
        .iter()
        .find_map(|item| match item {
            Item::Function(function) if function.name.name == "captured" => {
                Some(function.name.span)
            }
            _ => None,
        })
        .unwrap();
    let mut installed = interpreter
        .checked_expression_types
        .as_ref()
        .unwrap()
        .as_ref()
        .clone();
    installed
        .functions
        .insert(declaration, vec![Arc::new(CheckedFunctionTypes::default())]);
    interpreter.set_checked_expression_types(Arc::new(installed));
    let raw = interpreter
        .call_function("app.captured", vec![Value::Int64(7)])
        .unwrap();
    refuse_forged_capture(&mut interpreter, raw.clone(), &requested);
    assert_eq!(
        interpreter.call_fn_value(raw, Vec::new()).unwrap(),
        Value::Int64(7)
    );
    assert_eq!(traces(&mut interpreter).len(), 1);
}

#[test]
fn reflected_closure_forged_root_type_context_refuses_mint_and_admission() {
    for binding_map in [false, true] {
        let (mut interpreter, _, requested) = prepared();
        let genuine = interpreter
            .call_function("app.captured", vec![Value::Int64(7)])
            .unwrap();
        let mut installed = interpreter
            .checked_expression_types
            .as_ref()
            .unwrap()
            .as_ref()
            .clone();
        let foreign_span = Span::new(requested.span().file, u32::MAX - 1, u32::MAX);
        if binding_map {
            installed
                .bindings
                .insert(foreign_span, vec![Arc::new(CheckedScopedTypes::default())]);
        } else {
            installed.expressions.insert(foreign_span, "bool".into());
        }
        interpreter.set_checked_expression_types(Arc::new(installed));
        let error = interpreter
            .prepare_reflected_callable(genuine, &requested, "function() returns int64")
            .unwrap_err();
        assert!(
            error.contains("checked expression context changed its original source"),
            "{error}"
        );
        assert!(traces(&mut interpreter).is_empty());
        let raw = interpreter
            .call_function("app.captured", vec![Value::Int64(7)])
            .unwrap();
        let Value::Function { type_context, .. } = &raw else {
            panic!("expected closure");
        };
        assert!(type_context.reflected_capture.is_none());
        assert_eq!(
            interpreter.call_fn_value(raw, Vec::new()).unwrap(),
            Value::Int64(7)
        );
        assert_eq!(traces(&mut interpreter).len(), 1);
    }
}

#[test]
fn reflected_closure_root_arithmetic_fact_forgery_refuses_before_source_invocation() {
    let (mut interpreter, program, requested) = prepared();
    let requested = Interpreter::simple_type_expr_from_name(
        "function(int64) returns app.Positive",
        requested.span(),
    )
    .unwrap();
    let source = interpreter
        .call_function("app.captured_parameter", vec![Value::Int64(7)])
        .unwrap();
    let observed = interpreter
        .prepare_reflected_callable(source, &requested, "function(int64) returns int64")
        .unwrap();
    let arithmetic = program
        .module()
        .items
        .iter()
        .find_map(|item| {
            let Item::Function(function) = item else {
                return None;
            };
            if function.name.name != "captured_parameter" {
                return None;
            }
            let Stmt::VarDecl(declaration) = function.body.stmts.first()? else {
                return None;
            };
            let Expr::InlineFn(_, _, body, _) = &declaration.value else {
                return None;
            };
            let Stmt::Return(statement) = body.stmts.last()? else {
                return None;
            };
            statement.value.as_ref().map(Expr::span)
        })
        .unwrap();
    let mut installed = interpreter
        .checked_expression_types
        .as_ref()
        .unwrap()
        .as_ref()
        .clone();
    installed.expressions.insert(arithmetic, "int8".into());
    interpreter.set_checked_expression_types(Arc::new(installed));
    let error = interpreter
        .call_fn_value(observed, vec![Value::Int64(130)])
        .unwrap_err();
    assert!(
        error.contains("checked expression context changed its original source"),
        "{error}"
    );
    assert!(traces(&mut interpreter).is_empty());
    let raw = interpreter
        .call_function("app.captured_parameter", vec![Value::Int64(7)])
        .unwrap();
    let Value::Function { type_context, .. } = &raw else {
        panic!("expected closure");
    };
    assert!(type_context.reflected_capture.is_none());
    assert_eq!(
        interpreter
            .call_fn_value(raw, vec![Value::Int64(130)])
            .unwrap(),
        Value::Int64(-119)
    );
    assert_eq!(traces(&mut interpreter).len(), 1);
}

#[test]
fn reflected_closure_pending_nothing_snapshot_preserves_layers_without_joining() {
    let (mut interpreter, _, requested) = prepared();
    interpreter.set_variable(
        "unobserved",
        Value::Pending(Box::new(Value::Pending(Box::new(Value::Nothing)))),
    );
    let source = interpreter
        .call_function("app.captured", vec![Value::Int64(7)])
        .unwrap();
    let Value::Function { captures, .. } = &source else {
        panic!("expected closure");
    };
    assert!(
        matches!(captures.get("unobserved"), Some(Value::Pending(outer)) if matches!(outer.as_ref(), Value::Pending(inner) if matches!(inner.as_ref(), Value::Nothing)))
    );
    let exact =
        Interpreter::simple_type_expr_from_name("function() returns int64", requested.span())
            .unwrap();
    let pending = Value::Pending(Box::new(Value::Pending(Box::new(source.clone()))));
    let unchanged = interpreter
        .prepare_reflected_callable(pending, &exact, "function() returns int64")
        .unwrap();
    assert!(
        matches!(unchanged, Value::Pending(outer) if matches!(outer.as_ref(), Value::Pending(inner) if matches!(inner.as_ref(), Value::Function { .. })))
    );
    assert!(traces(&mut interpreter).is_empty());
    let mut forged = source.clone();
    let Value::Function { captures, .. } = &mut forged else {
        panic!("expected closure");
    };
    captures.insert(
        "unobserved".into(),
        Value::Pending(Box::new(Value::Nothing)),
    );
    refuse_forged_capture(&mut interpreter, forged, &requested);
    let observed = adapter(&interpreter, source, &requested);
    assert_eq!(
        interpreter
            .call_fn_value(observed, Vec::new())
            .unwrap()
            .payload(),
        &Value::Int64(7)
    );
    assert_eq!(traces(&mut interpreter).len(), 2);
}

#[test]
fn reflected_closure_dynamic_factories_restore_mint_origin_after_primitive_calls() {
    let (mut interpreter, _, requested) = prepared();
    let captured_factory = interpreter
        .call_function("app.captured_factory", Vec::new())
        .unwrap();
    assert!(traces(&mut interpreter).is_empty());
    for factory in [
        Value::NamedFunction("app.captured_after_primitive".into()),
        captured_factory,
    ] {
        let source = interpreter
            .call_function("app.forward_factory", vec![factory, Value::Int64(7)])
            .unwrap();
        assert_eq!(
            traces(&mut interpreter),
            ["trace marker: string = primitive\n"]
        );
        assert!(interpreter.active_reflected_capture_origin.is_none());
        assert!(interpreter.active_checked_function.is_none());
        assert!(interpreter.active_checked_scope.is_none());
        assert!(interpreter.allow_checked_refinement_proofs);
        let observed = adapter(&interpreter, source, &requested);
        assert!(traces(&mut interpreter).is_empty());
        let result = interpreter.call_fn_value(observed, Vec::new()).unwrap();
        assert_eq!(result.payload(), &Value::Int64(8));
        assert_eq!(
            traces(&mut interpreter),
            [
                "trace marker: string = source\n",
                "trace marker: string = predicate\n",
            ]
        );
        assert!(interpreter.active_reflected_capture_origin.is_none());
    }
}
