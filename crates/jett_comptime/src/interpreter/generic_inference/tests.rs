use super::*;
use crate::interpreter::{CallArg, Expr, FileId, Ident, Span, Value, type_expr_display};

fn span() -> Span {
    Span::new(FileId::new(0), 0, 0)
}

fn ident(name: &str) -> Ident {
    Ident {
        name: name.into(),
        span: span(),
    }
}

fn named(name: &str) -> TypeExpr {
    TypeExpr::Named(ident(name))
}

fn generic(name: &str, args: Vec<TypeExpr>) -> TypeExpr {
    TypeExpr::Generic(ident(name), args, span())
}

fn interpreter(source: &str) -> Interpreter {
    let parsed = jett_parser::parse(source, FileId::new(0));
    assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
    let mut interpreter = Interpreter::new();
    interpreter.register_module(&parsed.module);
    interpreter
}

fn transparent_wrappers(expression: Expr) -> Vec<Expr> {
    vec![
        expression.clone(),
        Expr::Paren(Box::new(expression.clone()), span()),
        Expr::View(Box::new(expression.clone()), span()),
        Expr::Paren(
            Box::new(Expr::View(Box::new(expression.clone()), span())),
            span(),
        ),
        Expr::View(
            Box::new(Expr::Paren(Box::new(expression.clone()), span())),
            span(),
        ),
        Expr::Paren(
            Box::new(Expr::View(
                Box::new(Expr::Paren(Box::new(expression), span())),
                span(),
            )),
            span(),
        ),
    ]
}

fn infer_argument(interpreter: &Interpreter, callee: &str, expression: Expr) -> String {
    let arguments = [CallArg {
        name: None,
        value: expression,
        span: span(),
    }];
    let inferred = interpreter
        .inferred_user_function_type_args(&Expr::Ident(ident(callee)), &[], &arguments, None)
        .expect("source type witness");
    assert_eq!(inferred.len(), 1);
    type_expr_display(&inferred[0])
}

#[test]
fn transparent_argument_wrappers_preserve_root_and_nested_alias_inference() {
    let mut interpreter = interpreter(
        r#"type Count = int64
function whole[T](view value: T) returns nothing:
    return nothing
function payload[T](view value: optional[T]) returns nothing:
    return nothing
"#,
    );
    for (name, value, ty, callee, expected) in [
        ("count", Value::Int64(7), named("Count"), "whole", "int64"),
        (
            "maybe",
            Value::OptionalSome(Box::new(Value::Int64(7))),
            generic("optional", vec![named("Count")]),
            "payload",
            "Count",
        ),
        (
            "values",
            Value::List(vec![Value::Int64(7)]),
            generic("list", vec![named("Count")]),
            "whole",
            "list[Count]",
        ),
    ] {
        interpreter.set_variable_with_type(name, value, ty);
        for expression in transparent_wrappers(Expr::Ident(ident(name))) {
            assert_eq!(infer_argument(&interpreter, callee, expression), expected);
        }
    }
}

#[test]
fn transparent_callback_wrappers_preserve_aliases_and_view_parameter_modes() {
    let mut interpreter = interpreter(
        r#"type Count = int64
function whole[T](view value: T) returns nothing:
    return nothing
function input[T](view callback: function(view T) returns T) returns nothing:
    return nothing
function owned(value: Count) returns Count:
    return value
function borrowed(view value: Count) returns Count:
    return value
function double_view(view value: view Count) returns Count:
    return value
"#,
    );
    for (name, view, typed_view, expected) in [
        ("owned", false, false, "function(Count) returns Count"),
        (
            "borrowed",
            true,
            false,
            "function(view Count) returns Count",
        ),
        (
            "double_view",
            true,
            true,
            "function(view Count) returns Count",
        ),
    ] {
        let inline = Expr::InlineFn(
            vec![Param {
                view,
                mutable: false,
                name: ident("value"),
                ty: if typed_view {
                    TypeExpr::View(Box::new(named("Count")), span())
                } else {
                    named("Count")
                },
                span: span(),
            }],
            Some(named("Count")),
            jett_parser::ast::Block {
                stmts: Vec::new(),
                span: span(),
            },
            span(),
        );
        for expression in transparent_wrappers(Expr::Ident(ident(name))) {
            assert_eq!(
                interpreter
                    .named_function_argument_type(&expression)
                    .as_ref()
                    .map(type_expr_display)
                    .as_deref(),
                Some(expected)
            );
            assert_eq!(infer_argument(&interpreter, "whole", expression), expected);
        }
        for expression in transparent_wrappers(inline) {
            assert_eq!(infer_argument(&interpreter, "whole", expression), expected);
        }
        if view {
            for expression in transparent_wrappers(Expr::Ident(ident(name))) {
                assert_eq!(infer_argument(&interpreter, "input", expression), "Count");
            }
        }
        let signature = interpreter
            .named_function_argument_type(&Expr::Ident(ident(name)))
            .unwrap();
        interpreter.set_variable_with_type("stored", Value::NamedFunction(name.into()), signature);
        for expression in transparent_wrappers(Expr::Ident(ident("stored"))) {
            assert_eq!(infer_argument(&interpreter, "whole", expression), expected);
        }
    }
}

#[test]
fn transparent_witness_lookup_keeps_outer_checked_fallback_and_other_forms_opaque() {
    let mut interpreter = Interpreter::new();
    interpreter.set_variable_with_type("known", Value::Int64(7), named("Count"));
    let inner_span = Span::new(FileId::new(0), 1, 2);
    let outer_span = Span::new(FileId::new(0), 0, 3);
    let expression = Expr::View(
        Box::new(Expr::Ident(Ident {
            name: "unknown".into(),
            span: inner_span,
        })),
        outer_span,
    );
    let mut checked = crate::checked_types::CheckedExpressionTypes::default();
    checked.expressions.insert(inner_span, "int8".into());
    checked.expressions.insert(outer_span, "int64".into());
    interpreter.checked_expression_types = Some(std::sync::Arc::new(checked));
    assert_eq!(
        interpreter
            .call_argument_type(&expression)
            .as_ref()
            .map(type_expr_display)
            .as_deref(),
        Some("int64")
    );
    for expression in [
        Expr::Clone(Box::new(Expr::Ident(ident("known"))), span()),
        Expr::FieldAccess(
            Box::new(Expr::Ident(ident("known"))),
            ident("field"),
            span(),
        ),
        Expr::Call(Box::new(Expr::Ident(ident("known"))), Vec::new(), span()),
    ] {
        assert!(interpreter.call_argument_type(&expression).is_none());
    }
}

#[test]
fn nested_generic_local_fills_missing_never_without_observing_pending_values() {
    let mut interpreter = interpreter(
        r#"function describe[T](first: T, last: T) returns string:
    return type.name[T]()
function outer[T](empty: list[T], ready: list[int64]) returns string:
    return describe(empty, ready)
function reversed[T](empty: list[T], ready: list[int64]) returns string:
    return describe(ready, empty)
function retain[T](first: T, last: T) returns T:
    return first
function pending[T](empty: list[T], ready: list[int64]) returns list[int64]:
    return retain(empty, ready)
"#,
    );
    for function in ["outer", "reversed"] {
        assert_eq!(
            interpreter.call_function_with_type_args(
                function,
                &[named("<never>")],
                vec![Value::List(Vec::new()), Value::List(vec![Value::Int64(7)])],
            ),
            Ok(Value::String("list[int64]".into()))
        );
    }
    let pending = Value::Pending(Box::new(Value::Pending(Box::new(Value::List(Vec::new())))));
    assert_eq!(
        interpreter.call_function_with_type_args(
            "pending",
            &[named("<never>")],
            vec![pending.clone(), Value::List(vec![Value::Int64(7)])],
        ),
        Ok(pending)
    );
}

#[test]
fn never_inference_keeps_first_concrete_alias_in_declaration_argument_order() {
    let mut interpreter = interpreter(
        r#"type Count = int64
type Other = int64
function describe[T](first: T, last: T) returns string:
    return type.name[T]()
"#,
    );
    interpreter.set_variable_with_type(
        "count",
        Value::ResultOk(Box::new(Value::Int64(7))),
        generic("result", vec![named("Count"), named("<never>")]),
    );
    interpreter.set_variable_with_type(
        "other",
        Value::ResultFail(Box::new(Value::String("bad".into()))),
        generic("result", vec![named("Other"), named("string")]),
    );
    let call = |first: &str, last: &str| {
        Expr::Call(
            Box::new(Expr::Ident(ident("describe"))),
            vec![
                CallArg {
                    name: Some(ident("last")),
                    value: Expr::Ident(ident(last)),
                    span: span(),
                },
                CallArg {
                    name: Some(ident("first")),
                    value: Expr::Ident(ident(first)),
                    span: span(),
                },
            ],
            span(),
        )
    };
    assert_eq!(
        interpreter.eval_expr(&call("count", "other")),
        Ok(Value::String("result[Count, string]".into()))
    );
    assert_eq!(
        interpreter.eval_expr(&call("other", "count")),
        Ok(Value::String("result[Other, string]".into()))
    );
}

#[test]
fn never_inference_preserves_root_alias_peeling_and_nested_callback_aliases() {
    let interpreter = interpreter(
        r#"type Count = int64
type Other = int64
function describe[T](first: T, last: T) returns nothing:
    return nothing
"#,
    );
    let inferred = |actual: &[TypeExpr]| {
        interpreter
            .inferred_user_function_type_args_from_types(
                &Expr::Ident(ident("describe")),
                &[],
                actual,
            )
            .unwrap()
            .into_iter()
            .map(|ty| type_expr_display(&ty))
            .collect::<Vec<_>>()
    };
    assert_eq!(inferred(&[named("Count"), named("Other")]), ["int64"]);
    let callback = |result| TypeExpr::Function(Vec::new(), Box::new(result), span());
    assert_eq!(
        inferred(&[
            callback(generic("optional", vec![named("<never>")])),
            callback(generic("optional", vec![named("Count")])),
        ]),
        ["function() returns optional[Count]"]
    );
}

#[test]
fn never_inference_merges_matching_container_slots_in_both_orders() {
    let interpreter = Interpreter::new();
    for owner in ["list", "set", "optional", "secret"] {
        let absent = generic(owner, vec![named("<never>")]);
        let present = generic(owner, vec![named("int64")]);
        for (first, next) in [(&absent, &present), (&present, &absent)] {
            assert_eq!(
                interpreter
                    .merge_inferred_never(first, next, false)
                    .as_ref()
                    .map(type_expr_display),
                Some(type_expr_display(&present))
            );
        }
    }
    for owner in ["map", "result"] {
        let first = generic(owner, vec![named("<never>"), named("string")]);
        let next = generic(owner, vec![named("int64"), named("<never>")]);
        let merged = generic(owner, vec![named("int64"), named("string")]);
        for (first, next) in [(&first, &next), (&next, &first)] {
            assert_eq!(
                interpreter
                    .merge_inferred_never(first, next, false)
                    .as_ref()
                    .map(type_expr_display),
                Some(type_expr_display(&merged))
            );
        }
    }
}

#[test]
fn never_inference_respects_callable_variance_and_view_parameter_modes() {
    let interpreter = Interpreter::new();
    let callback = |input, output, view| {
        TypeExpr::Function(
            vec![if view {
                TypeExpr::View(Box::new(input), span())
            } else {
                input
            }],
            Box::new(output),
            span(),
        )
    };
    for view in [false, true] {
        let first = callback(
            generic("optional", vec![named("int64")]),
            generic("list", vec![named("<never>")]),
            view,
        );
        let next = callback(
            generic("optional", vec![named("<never>")]),
            generic("list", vec![named("string")]),
            view,
        );
        let expected = callback(
            generic("optional", vec![named("<never>")]),
            generic("list", vec![named("string")]),
            view,
        );
        for (first, next) in [(&first, &next), (&next, &first)] {
            assert_eq!(
                interpreter
                    .merge_inferred_never(first, next, false)
                    .as_ref()
                    .map(type_expr_display),
                Some(type_expr_display(&expected))
            );
        }
    }
    assert!(
        interpreter
            .merge_inferred_never(
                &callback(named("int64"), named("nothing"), false),
                &callback(named("int64"), named("nothing"), true),
                false,
            )
            .is_none()
    );
}

#[test]
fn never_inference_does_not_promote_known_leaves_or_partially_merge_conflicts() {
    let interpreter = Interpreter::new();
    for (first, next) in [
        (named("int8"), named("int64")),
        (
            generic("result", vec![named("<never>"), named("int8")]),
            generic("result", vec![named("string"), named("int64")]),
        ),
        (
            generic("Record", vec![named("<never>")]),
            generic("Record", vec![named("int64")]),
        ),
        (
            generic("list", vec![named("<never>")]),
            generic("optional", vec![named("int64")]),
        ),
    ] {
        assert!(
            interpreter
                .merge_inferred_never(&first, &next, false)
                .is_none()
        );
    }
}
