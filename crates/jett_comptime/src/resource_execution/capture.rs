//! Raw lexical references are only a refusal guard, never opaque copy authority.
use super::*;

pub(super) fn raw_closure_uses_binding(params: &[Param], body: &Block, name: &str) -> bool {
    block_uses_binding(
        body,
        name,
        params.iter().any(|param| param.name.name == name),
    )
}

fn block_uses_binding(body: &Block, name: &str, mut shadowed: bool) -> bool {
    for statement in &body.stmts {
        let used = match statement {
            Stmt::VarDecl(value) => {
                let used = expression_uses_binding(&value.value, name, shadowed);
                shadowed |= value.name.name == name;
                used
            }
            Stmt::Assign(value) => {
                expression_uses_binding(&value.target, name, shadowed)
                    || expression_uses_binding(&value.value, name, shadowed)
            }
            Stmt::Return(value) => value
                .value
                .as_ref()
                .is_some_and(|value| expression_uses_binding(value, name, shadowed)),
            Stmt::Respond(value) => expression_uses_binding(&value.value, name, shadowed),
            Stmt::ComptimeTypeBind(value) => {
                expression_uses_binding(&value.value, name, shadowed)
                    || block_uses_binding(&value.body, name, shadowed || value.name.name == name)
            }
            Stmt::If(value) => {
                expression_uses_binding(&value.condition, name, shadowed)
                    || block_uses_binding(&value.then_block, name, shadowed)
                    || value.else_ifs.iter().any(|(condition, body)| {
                        expression_uses_binding(condition, name, shadowed)
                            || block_uses_binding(body, name, shadowed)
                    })
                    || value
                        .else_block
                        .as_ref()
                        .is_some_and(|body| block_uses_binding(body, name, shadowed))
            }
            Stmt::For(value) => {
                expression_uses_binding(&value.iterable, name, shadowed)
                    || block_uses_binding(
                        &value.body,
                        name,
                        shadowed
                            || value.variable.name == name
                            || value
                                .value_variable
                                .as_ref()
                                .is_some_and(|binding| binding.name == name),
                    )
            }
            Stmt::While(value) => {
                expression_uses_binding(&value.condition, name, shadowed)
                    || block_uses_binding(&value.body, name, shadowed)
            }
            Stmt::Match(value) => {
                expression_uses_binding(&value.expr, name, shadowed)
                    || value.arms.iter().any(|arm| {
                        let bound = match &arm.pattern {
                            Pattern::Variant(_, bindings) => {
                                bindings.iter().any(|binding| binding.name == name)
                            }
                            Pattern::Ident(_) | Pattern::Other(_) => false,
                        };
                        block_uses_binding(&arm.body, name, shadowed || bound)
                    })
            }
            Stmt::Expr(value) => expression_uses_binding(&value.expr, name, shadowed),
            Stmt::Use(value) => {
                shadowed |=
                    Interpreter::use_bound_name(&value.path.name, value.alias.as_ref()) == name;
                false
            }
            Stmt::Assert(value) => {
                expression_uses_binding(&value.condition, name, shadowed)
                    || value
                        .message
                        .as_ref()
                        .is_some_and(|value| expression_uses_binding(value, name, shadowed))
            }
            Stmt::Trace(value) => !shadowed && value.name.name == name,
            Stmt::Breakpoint(value) => value
                .condition
                .as_ref()
                .is_some_and(|value| expression_uses_binding(value, name, shadowed)),
            Stmt::Break(_) | Stmt::Continue(_) => false,
        };
        if used {
            return true;
        }
    }
    false
}

fn expression_uses_binding(value: &Expr, name: &str, shadowed: bool) -> bool {
    match value {
        Expr::Ident(value) => !shadowed && value.name == name,
        Expr::Binary(left, _, right, _) => {
            expression_uses_binding(left, name, shadowed)
                || expression_uses_binding(right, name, shadowed)
        }
        Expr::Unary(_, inner, _)
        | Expr::FieldAccess(inner, _, _)
        | Expr::Paren(inner, _)
        | Expr::View(inner, _)
        | Expr::Comptime(inner, _)
        | Expr::Ok(inner, _)
        | Expr::Fail(inner, _)
        | Expr::Some(inner, _)
        | Expr::Default(inner, _)
        | Expr::Declassify(inner, _)
        | Expr::Coarsen(inner, _)
        | Expr::At(inner, _, _)
        | Expr::Spawn(inner, _)
        | Expr::Send(inner, _)
        | Expr::Ask(inner, _)
        | Expr::Clone(inner, _)
        | Expr::Run(inner, _)
        | Expr::Join(inner, _)
        | Expr::Cancel(inner, _) => expression_uses_binding(inner, name, shadowed),
        Expr::Call(callee, arguments, _) | Expr::GenericCall(callee, _, arguments, _) => {
            expression_uses_binding(callee, name, shadowed)
                || arguments
                    .iter()
                    .any(|argument| expression_uses_binding(&argument.value, name, shadowed))
        }
        Expr::ListConstruct(values, _) => values
            .iter()
            .any(|value| expression_uses_binding(value, name, shadowed)),
        Expr::MapConstruct(values, _) => values.iter().any(|(key, value)| {
            expression_uses_binding(key, name, shadowed)
                || expression_uses_binding(value, name, shadowed)
        }),
        Expr::Handle(target, error, body, _) => {
            expression_uses_binding(target, name, shadowed)
                || block_uses_binding(
                    body,
                    name,
                    shadowed || error.as_ref().is_some_and(|error| error.name == name),
                )
        }
        Expr::StringInterpolation(parts, _) => parts.iter().any(|part| match part {
            StringPart::Expr(value) => expression_uses_binding(value, name, shadowed),
            StringPart::Literal(_) => false,
        }),
        Expr::Pipeline(initial, steps, _) => {
            expression_uses_binding(initial, name, shadowed)
                || steps.iter().any(|step| {
                    expression_uses_binding(&step.function, name, shadowed)
                        || step.extra_args.iter().any(|argument| {
                            expression_uses_binding(&argument.value, name, shadowed)
                        })
                        || step.handle.as_ref().is_some_and(|handle| {
                            block_uses_binding(
                                &handle.body,
                                name,
                                shadowed
                                    || handle
                                        .error_name
                                        .as_ref()
                                        .is_some_and(|error| error.name == name),
                            )
                        })
                })
        }
        Expr::InlineFn(params, _, body, _) => block_uses_binding(
            body,
            name,
            shadowed || params.iter().any(|param| param.name.name == name),
        ),
        Expr::IntLiteral(..)
        | Expr::FloatLiteral(..)
        | Expr::StringLiteral(..)
        | Expr::BoolLiteral(..)
        | Expr::Nothing(_)
        | Expr::None(_)
        | Expr::EnumVariant(..)
        | Expr::Error(_) => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resource_execution::{ExecutionPurpose, tests::program};

    fn function(source: &str) -> FunctionDef {
        let parsed = jett_parser::parse(source, jett_common::FileId::new(0));
        assert!(parsed.errors.is_empty());
        parsed
            .module
            .items
            .into_iter()
            .find_map(|item| match item {
                Item::Function(function) => Some(function),
                _ => None,
            })
            .unwrap()
    }

    #[test]
    fn raw_opaque_capture_refuses_referenced_binding_but_preserves_unrelated_and_shadowed_data() {
        let checked = program(
            include_str!("fixtures/04_bare_to_view_operation_cleanup.jett"),
            false,
        );
        let mut authority = Interpreter::from_checked_resource_program(
            checked.clone(),
            ExecutionPurpose::ReferenceRuntime,
        )
        .unwrap();
        let grant = authority.install_resource_test_script(Vec::new()).unwrap();
        let mut raw = Interpreter::new();
        raw.set_variable("net", grant.clone());
        let source = function("namespace app\nfunction f() returns nothing:\n    return net\n");
        assert!(
            raw.capture_closure(&source.params, source.return_type.as_ref(), &source.body)
                .unwrap_err()
                .contains("opaque capture")
        );
        let mut raw_checked =
            Interpreter::from_checked_resource_program(checked, ExecutionPurpose::ReferenceRuntime)
                .unwrap();
        raw_checked.set_variable("net", grant);
        assert!(
            raw_checked
                .capture_closure(&source.params, source.return_type.as_ref(), &source.body)
                .unwrap_err()
                .contains("opaque capture")
        );
        for source in [
            "namespace app\nfunction f() returns int64:\n    return 7\n",
            "namespace app\nfunction f(net: int64) returns int64:\n    return net\n",
            "namespace app\nfunction f() returns int64:\n    int64 net = 7\n    return net\n",
        ] {
            let source = function(source);
            let closure = raw
                .capture_closure(&source.params, source.return_type.as_ref(), &source.body)
                .unwrap();
            let Value::Function { captures, .. } = closure else {
                panic!("ordinary closure");
            };
            assert!(!captures.contains_key("net"));
            assert!(
                captures
                    .values()
                    .all(|value| !value.contains_live_resource_or_grant())
            );
        }
    }
}
