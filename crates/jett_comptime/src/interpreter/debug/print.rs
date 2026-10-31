//! Typed rendering only for accepted compiler-owned Print/Println observations.
use super::*;
use crate::debug::DebugEventKind;
use jett_intrinsics::IntrinsicId;

enum PrintFrame<'a> {
    Value(&'a Value, Option<TypeExpr>),
    Text(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ty(name: &str) -> TypeExpr {
        Interpreter::debug_type(name).unwrap()
    }

    fn registered() -> Interpreter {
        let parsed = jett_parser::parse(
            r#"namespace models
type Token = secret[string]
type TokenAlias = Token
type QualifiedToken = Token where true
interface Named:
    function name(view self: Named) returns string
struct Boxed[T]:
    token: TokenAlias
    value: T
enum Choice:
    item(token: Token, value: int64)
    empty
machine Session:
    states:
        idle
        active(token: Token, value: int64)
    transitions:
        idle to active
"#,
            FileId::new(0),
        );
        assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
        let mut interpreter = Interpreter::new();
        interpreter.register_module(&parsed.module);
        interpreter
    }

    fn secret() -> Value {
        Value::String("unpublished-secret".into())
    }

    #[test]
    fn print_redacts_qualified_aliases_before_payload_and_inner_pending_depth() {
        let interpreter = registered();
        let value = Value::Pending(Box::new(Value::Pending(Box::new(secret()))));
        for name in [
            "secret[string]",
            "models.TokenAlias",
            "models.QualifiedToken",
        ] {
            assert_eq!(
                interpreter.format_debug_print_value(&value, Some(&ty(name))),
                "[redacted]",
                "{name}",
            );
        }
        assert_eq!(
            interpreter
                .format_debug_print_value(&Value::String("[redacted]".into()), Some(&ty("string"))),
            "[redacted]",
        );
        assert_eq!(
            interpreter.format_debug_print_value(
                &Value::Pending(Box::new(Value::String("public".into()))),
                Some(&ty("string"))
            ),
            "pending(public)",
        );
    }

    #[test]
    fn print_erased_generic_owner_uses_exact_fields_and_keeps_public_values() {
        let interpreter = registered();
        let concrete = Value::Struct {
            concrete_type: Some("models.Boxed[list[int64]]".into()),
            type_name: "models.Boxed".into(),
            fields: vec![
                ("token".into(), secret()),
                (
                    "value".into(),
                    Value::List(vec![Value::Int64(7), Value::Int64(9)]),
                ),
            ],
        };
        let erased = Value::Typed {
            type_name: "models.Boxed[list[int64]]".into(),
            value: Box::new(concrete),
        };
        assert_eq!(
            interpreter.format_debug_print_value(&erased, Some(&ty("models.Named"))),
            "models.Boxed(token: [redacted], value: list(7, 9))",
        );
        // Rendering is nonowning, including the concrete identity used later.
        let Value::Struct {
            concrete_type,
            fields,
            ..
        } = erased.payload()
        else {
            panic!("renderer changed the source carrier");
        };
        assert_eq!(concrete_type.as_deref(), Some("models.Boxed[list[int64]]"));
        assert_eq!(fields[0].1, secret());
    }

    #[test]
    fn print_recurses_through_each_typed_container_and_sum_payload() {
        let interpreter = registered();
        let cases = [
            (
                Value::List(vec![secret()]),
                "list[secret[string]]",
                "list([redacted])",
            ),
            (
                Value::Set(vec![secret()]),
                "set[secret[string]]",
                "set([redacted])",
            ),
            (
                Value::Map(vec![(Value::String("public".into()), secret())]),
                "map[string, secret[string]]",
                "map(public: [redacted])",
            ),
            (
                Value::OptionalSome(Box::new(secret())),
                "optional[secret[string]]",
                "some([redacted])",
            ),
            (
                Value::ResultOk(Box::new(secret())),
                "result[secret[string], int64]",
                "ok([redacted])",
            ),
            (
                Value::ResultFail(Box::new(secret())),
                "result[int64, secret[string]]",
                "fail([redacted])",
            ),
            (
                Value::OptionalSome(Box::new(Value::List(vec![Value::Pending(Box::new(
                    secret(),
                ))]))),
                "optional[list[secret[string]]]",
                "some(list([redacted]))",
            ),
        ];
        for (value, name, expected) in cases {
            assert_eq!(
                interpreter.format_debug_print_value(&value, Some(&ty(name))),
                expected,
                "{name}"
            );
        }
    }

    #[test]
    fn print_keeps_enum_machine_and_incomplete_builder_order_with_typed_redaction() {
        let interpreter = registered();
        let choice = Value::Enum {
            type_name: "models.Choice".into(),
            variant: "item".into(),
            fields: vec![Value::Pending(Box::new(secret())), Value::Int64(3)],
        };
        let session = Value::Machine {
            type_name: "models.Session".into(),
            state: "active".into(),
            fields: vec![secret(), Value::Int64(4)],
        };
        assert_eq!(
            interpreter.format_debug_print_value(&choice, Some(&ty("models.Choice"))),
            "models.Choice.item([redacted], 3)"
        );
        assert_eq!(
            interpreter.format_debug_print_value(&session, Some(&ty("models.Session"))),
            "models.Session@active([redacted], 4)"
        );
        for (variant, state, label) in [
            (None, None, "models.Boxed[int64]"),
            (Some("item".into()), None, "models.Boxed[int64].item"),
            (None, Some("active".into()), "models.Boxed[int64]@active"),
        ] {
            let builder = Value::TypeConstruction {
                type_name: "models.Boxed[int64]".into(),
                variant,
                state,
                fields: vec![
                    (1, "value".into(), "int64".into(), Value::Int64(7)),
                    (0, "token".into(), "models.Token".into(), secret()),
                ],
            };
            assert_eq!(
                interpreter.format_debug_print_value(&builder, Some(&ty("TypeConstruction"))),
                format!("TypeConstruction[{label}](value: 7, token: [redacted])")
            );
        }
    }

    #[test]
    fn print_missing_field_or_container_metadata_never_falls_back_to_raw_payload() {
        let interpreter = Interpreter::new();
        let unknown = Value::Struct {
            type_name: "Unknown".into(),
            concrete_type: None,
            fields: vec![("token".into(), secret())],
        };
        assert_eq!(
            interpreter.format_debug_print_value(&unknown, None),
            "Unknown(token: [redacted])"
        );
        assert_eq!(
            interpreter.format_debug_print_value(&Value::List(vec![secret()]), None),
            "list([redacted])"
        );
        let builder = Value::TypeConstruction {
            type_name: "Unknown".into(),
            variant: None,
            state: None,
            fields: vec![(0, "token".into(), "".into(), secret())],
        };
        assert_eq!(
            interpreter.format_debug_print_value(&builder, None),
            "TypeConstruction[Unknown](token: [redacted])"
        );
    }

    #[test]
    fn print_keeps_opaque_labels_without_observing_callback_captures() {
        let interpreter = Interpreter::new();
        let function = Value::Function {
            type_context: Box::default(),
            params: vec![],
            body: jett_parser::ast::Block {
                stmts: vec![],
                span: Span::new(FileId::new(0), 0, 0),
            },
            captures: [("token".into(), secret())].into(),
            capture_types: [("token".into(), ty("secret[string]"))].into(),
            namespace_aliases: Default::default(),
            namespace: None,
        };
        assert_eq!(
            interpreter.format_debug_print_value(&function, None),
            "function()"
        );
        assert_eq!(
            interpreter
                .format_debug_print_value(&Value::NamedFunction("models.inspect".into()), None),
            "function(models.inspect)"
        );
        assert_eq!(
            interpreter.format_debug_print_value(&Value::Actor(7), None),
            "actor#7"
        );
    }

    #[test]
    fn print_authentic_reflected_callables_never_invoke_sources_or_observe_captures() {
        use crate::checked_types::CheckedExpressionTypes;
        use jett_common::SourceOrigin;
        use jett_typecheck::{CheckOptions, CheckedResourceProgram};
        use std::collections::HashMap;
        use std::sync::Arc;

        let source = r#"namespace app
function positive_check(value: int64) returns bool:
    string marker = "predicate"
    trace marker
    return value > 0
type Positive = int64 where positive_check(value)
function requested(view callback: function() returns Positive) returns nothing:
    return nothing
function named() returns int64:
    string marker = "callback"
    trace marker
    return 7
function captured(seed: int64) returns function() returns int64:
    function() returns int64 callback = function() returns int64:
        string marker = "callback"
        trace marker
        return seed
    return callback
"#;
        let file = FileId::new(0);
        let program = Arc::new(
            CheckedResourceProgram::prepare(
                jett_parser::parse(source, file),
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
        let requested = ty("function() returns app.Positive");
        let captured = interpreter
            .call_function("app.captured", vec![Value::Int64(7)])
            .unwrap();
        let Value::Function { capture_types, .. } = &captured else {
            panic!("original source must produce the captured closure");
        };
        assert_eq!(capture_types.len(), 1);
        assert_eq!(
            super::super::super::type_expr_display(capture_types.get("seed").unwrap()),
            "int64"
        );
        assert!(interpreter.take_debug_events().is_empty());
        for original in [Value::NamedFunction("app.named".into()), captured] {
            let label = original.to_string();
            let observed = interpreter
                .prepare_reflected_callable(original, &requested, "function() returns int64")
                .unwrap();
            assert!(matches!(observed, Value::ReflectedCallable(_)));
            assert!(interpreter.take_debug_events().is_empty());
            interpreter.emit_typed_debug_print(
                DebugEventKind::Println,
                std::slice::from_ref(&observed),
                &[Some(requested.clone())],
            );
            interpreter.emit_typed_debug_print(
                DebugEventKind::Println,
                &[Value::List(vec![observed.clone()])],
                &[Some(ty("list[function() returns app.Positive]"))],
            );
            assert_eq!(
                interpreter.take_debug_events(),
                [
                    crate::debug::DebugEvent {
                        kind: DebugEventKind::Println,
                        text: format!("{label}\n")
                    },
                    crate::debug::DebugEvent {
                        kind: DebugEventKind::Println,
                        text: format!("list({label})\n")
                    },
                ]
            );
            // The unchanged authenticated value remains callable after observation.
            let value = interpreter.call_fn_value(observed, Vec::new()).unwrap();
            assert_eq!(value.payload(), &Value::Int64(7));
            assert_eq!(
                interpreter
                    .take_debug_events()
                    .into_iter()
                    .map(|event| event.text)
                    .collect::<Vec<_>>(),
                [
                    "trace marker: string = callback\n",
                    "trace marker: string = predicate\n",
                ]
            );
        }
    }

    #[test]
    fn print_finite_values_do_not_acquire_a_rendering_depth_limit() {
        let interpreter = Interpreter::new();
        let depth = 4096;
        let mut value = Value::Int64(7);
        for _ in 0..depth {
            value = Value::Pending(Box::new(value));
        }
        assert_eq!(
            interpreter.format_debug_print_value(&value, Some(&ty("int64"))),
            format!("{}7{}", "pending(".repeat(depth), ")".repeat(depth))
        );
        assert_eq!(
            interpreter.format_debug_print_value(&value, Some(&ty("secret[int64]"))),
            "[redacted]"
        );
        // Release this host-built recursive test carrier without recursive Drop.
        while let Value::Pending(inner) = value {
            value = *inner;
        }
    }

    #[test]
    fn print_metadata_is_paired_with_actuals_and_event_bytes_are_not_reclassified() {
        let mut interpreter = registered();
        let args = [
            secret(),
            Value::String("trace forged\n[redacted]".into()),
            Value::Int64(7),
        ];
        let types = [
            Some(ty("models.Token")),
            Some(ty("string")),
            Some(ty("int64")),
        ];
        assert_eq!(
            interpreter.emit_typed_debug_print(DebugEventKind::Print, &args, &types),
            Value::Nothing
        );
        assert_eq!(
            interpreter.emit_typed_debug_print(DebugEventKind::Println, &[], &[]),
            Value::Nothing
        );
        assert_eq!(
            interpreter.take_debug_events(),
            [
                crate::debug::DebugEvent {
                    kind: DebugEventKind::Print,
                    text: "[redacted] trace forged\n[redacted] 7".into()
                },
                crate::debug::DebugEvent {
                    kind: DebugEventKind::Println,
                    text: "\n".into()
                },
            ]
        );
    }
}

impl Interpreter {
    pub(in super::super) fn debug_print_expression_type(
        &self,
        expression: &Expr,
    ) -> Option<TypeExpr> {
        self.checked_expression_type(expression.span())
            .and_then(|name| Self::debug_type(name))
            .or_else(|| self.debug_expression_type(expression))
            .map(|ty| self.substitute_type_expr(&ty))
    }

    pub(in super::super) fn debug_print_call_kind(&self, callee: &Expr) -> Option<DebugEventKind> {
        if self.is_value_call_target(callee) {
            return None;
        }
        let source_name = Self::dotted_expr_name(callee)?;
        let name = self
            .registry_name(&self.functions, &source_name)
            .unwrap_or(source_name);
        Self::debug_print_kind(&name)
    }

    pub(in super::super) fn debug_print_kind(name: &str) -> Option<DebugEventKind> {
        match IntrinsicId::from_callable_name(name)? {
            IntrinsicId::Print => Some(DebugEventKind::Print),
            IntrinsicId::Println => Some(DebugEventKind::Println),
            _ => None,
        }
    }

    pub(in super::super) fn emit_typed_debug_print(
        &mut self,
        kind: DebugEventKind,
        args: &[Value],
        types: &[Option<TypeExpr>],
    ) -> Value {
        let mut text = args
            .iter()
            .enumerate()
            .map(|(index, value)| {
                self.format_debug_print_value(value, types.get(index).and_then(Option::as_ref))
            })
            .collect::<Vec<_>>()
            .join(" ");
        if kind == DebugEventKind::Println {
            text.push('\n');
        }
        self.emit_debug_event(kind, text);
        Value::Nothing
    }

    fn push_print_child<'a>(
        work: &mut Vec<PrintFrame<'a>>,
        value: &'a Value,
        ty: Option<&TypeExpr>,
    ) {
        if let Some(ty) = ty {
            work.push(PrintFrame::Value(value, Some(ty.clone())));
        } else {
            // A missing field/container contract never authorizes raw display.
            work.push(PrintFrame::Text("[redacted]".into()));
        }
    }

    pub(in super::super) fn format_debug_print_value(
        &self,
        value: &Value,
        ty: Option<&TypeExpr>,
    ) -> String {
        let mut text = String::new();
        let mut work = vec![PrintFrame::Value(value, ty.cloned())];
        while let Some(frame) = work.pop() {
            match frame {
                PrintFrame::Text(part) => text.push_str(&part),
                PrintFrame::Value(value, ty) => {
                    self.push_print_value(value, ty, &mut work, &mut text);
                }
            }
        }
        text
    }

    fn push_print_value<'a>(
        &self,
        value: &'a Value,
        ty: Option<TypeExpr>,
        work: &mut Vec<PrintFrame<'a>>,
        text: &mut String,
    ) {
        let ty = ty.map(|ty| self.resolved_inference_base_type(&ty));
        let ty = match ty {
            Some(TypeExpr::View(inner, _)) => {
                work.push(PrintFrame::Value(value, Some(*inner)));
                return;
            }
            ty => ty,
        };
        let args = match &ty {
            Some(TypeExpr::Generic(owner, _, _)) if owner.name == "secret" => {
                // Do not inspect the value, its handles, or inner pending depth.
                text.push_str("[redacted]");
                return;
            }
            Some(TypeExpr::Generic(_, args, _)) => args.as_slice(),
            _ => &[],
        };
        match value {
            Value::Typed { type_name, value } => {
                work.push(PrintFrame::Value(value, Self::debug_type(type_name).or(ty)));
            }
            Value::Pending(inner) => {
                text.push_str("pending(");
                work.push(PrintFrame::Text(")".into()));
                work.push(PrintFrame::Value(inner, ty));
            }
            Value::List(items) | Value::Set(items) => {
                text.push_str(if matches!(value, Value::List(_)) {
                    "list("
                } else {
                    "set("
                });
                work.push(PrintFrame::Text(")".into()));
                for (index, item) in items.iter().enumerate().rev() {
                    Self::push_print_child(work, item, args.first());
                    if index != 0 {
                        work.push(PrintFrame::Text(", ".into()));
                    }
                }
            }
            Value::Map(entries) => {
                text.push_str("map(");
                work.push(PrintFrame::Text(")".into()));
                for (index, (key, value)) in entries.iter().enumerate().rev() {
                    Self::push_print_child(work, value, args.get(1));
                    work.push(PrintFrame::Text(": ".into()));
                    Self::push_print_child(work, key, args.first());
                    if index != 0 {
                        work.push(PrintFrame::Text(", ".into()));
                    }
                }
            }
            Value::OptionalSome(inner) | Value::ResultOk(inner) | Value::ResultFail(inner) => {
                let (prefix, index) = match value {
                    Value::OptionalSome(_) => ("some(", 0),
                    Value::ResultOk(_) => ("ok(", 0),
                    Value::ResultFail(_) => ("fail(", 1),
                    _ => unreachable!("matched sum payload"),
                };
                text.push_str(prefix);
                work.push(PrintFrame::Text(")".into()));
                Self::push_print_child(work, inner, args.get(index));
            }
            Value::Struct {
                concrete_type,
                type_name,
                fields,
            } => {
                let owner = concrete_type
                    .as_deref()
                    .and_then(Self::debug_type)
                    .or_else(|| self.debug_nominal_owner(ty, type_name));
                let types = owner
                    .as_ref()
                    .map(|ty| self.debug_fields(ty, None))
                    .unwrap_or_default();
                text.push_str(type_name);
                text.push('(');
                work.push(PrintFrame::Text(")".into()));
                for (index, (name, value)) in fields.iter().enumerate().rev() {
                    let ty = types
                        .iter()
                        .find(|(field, _)| field == name)
                        .map(|(_, ty)| ty);
                    Self::push_print_child(work, value, ty);
                    work.push(PrintFrame::Text(format!("{name}: ")));
                    if index != 0 {
                        work.push(PrintFrame::Text(", ".into()));
                    }
                }
            }
            Value::Enum {
                type_name,
                variant: member,
                fields,
            }
            | Value::Machine {
                type_name,
                state: member,
                fields,
            } => {
                let machine = matches!(value, Value::Machine { .. });
                let owner = self.debug_nominal_owner(ty, type_name);
                let types = owner
                    .as_ref()
                    .map(|ty| self.debug_fields(ty, Some((member, machine))))
                    .unwrap_or_default();
                text.push_str(type_name);
                text.push(if machine { '@' } else { '.' });
                text.push_str(member);
                if !fields.is_empty() {
                    text.push('(');
                    work.push(PrintFrame::Text(")".into()));
                    for (index, value) in fields.iter().enumerate().rev() {
                        Self::push_print_child(work, value, types.get(index).map(|(_, ty)| ty));
                        if index != 0 {
                            work.push(PrintFrame::Text(", ".into()));
                        }
                    }
                }
            }
            Value::TypeConstruction {
                type_name,
                variant,
                state,
                fields,
            } => {
                text.push_str("TypeConstruction[");
                text.push_str(type_name);
                if let Some(variant) = variant {
                    text.push('.');
                    text.push_str(variant);
                }
                if let Some(state) = state {
                    text.push('@');
                    text.push_str(state);
                }
                text.push_str("](");
                work.push(PrintFrame::Text(")".into()));
                for (index, (_, name, type_name, value)) in fields.iter().enumerate().rev() {
                    let ty = (!type_name.is_empty())
                        .then(|| Self::debug_type(type_name))
                        .flatten();
                    Self::push_print_child(work, value, ty.as_ref());
                    work.push(PrintFrame::Text(format!("{name}: ")));
                    if index != 0 {
                        work.push(PrintFrame::Text(", ".into()));
                    }
                }
            }
            // Primitive and opaque labels never inspect captures, actor state,
            // or aggregate fields. Value display remains unchanged elsewhere.
            Value::Int64(_)
            | Value::Uint64(_)
            | Value::Float64(_)
            | Value::String(_)
            | Value::Bool(_)
            | Value::Bytes(_)
            | Value::OptionalNone
            | Value::Nothing
            | Value::Capability(_)
            | Value::Resource(_)
            | Value::ResourceHook(_)
            | Value::GrantedNetwork(_)
            | Value::Actor(_)
            | Value::Error(_)
            | Value::NamedFunction(_)
            | Value::ReflectedCallable(_)
            | Value::Function { .. } => text.push_str(&value.to_string()),
        }
    }
}
