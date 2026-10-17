//! Snapshot comparison retains tags, pending layers, and callable contexts.
use super::{Value, ast_equal};
use crate::value::ClosureTypeContext;
use jett_parser::ast::TypeExpr;
use std::{collections::HashMap, sync::Arc};

fn map<T>(a: &HashMap<String, T>, b: &HashMap<String, T>, same: impl Fn(&T, &T) -> bool) -> bool {
    a.len() == b.len()
        && a.iter()
            .all(|(key, a)| b.get(key).is_some_and(|b| same(a, b)))
}
fn shared<T>(a: &Option<Arc<T>>, b: &Option<Arc<T>>) -> bool {
    ast_equal::optional(a, b, Arc::ptr_eq)
}
fn context(a: &ClosureTypeContext, b: &ClosureTypeContext, proof: bool) -> bool {
    (!proof || shared(&a.reflected_capture, &b.reflected_capture))
        && ast_equal::optional(&a.return_type, &b.return_type, ast_equal::type_expr)
        && shared(&a.checked_scope, &b.checked_scope)
        && shared(&a.checked_function, &b.checked_function)
        && a.scoped_type_bindings == b.scoped_type_bindings
        && map(&a.bindings, &b.bindings, ast_equal::type_expr)
        && ast_equal::list(&a.arguments, &b.arguments, |a, b| {
            a.canonical_name == b.canonical_name
                && a.reflection == b.reflection
                && ast_equal::type_expr(&a.ty, &b.ty)
        })
}
pub(super) fn closure(a: &Value, b: &Value, proof: bool) -> bool {
    let (
        Value::Function {
            type_context: ac,
            params: ap,
            body: ab,
            captures: av,
            capture_types: at,
            namespace_aliases: aa,
            namespace: an,
        },
        Value::Function {
            type_context: bc,
            params: bp,
            body: bb,
            captures: bv,
            capture_types: bt,
            namespace_aliases: ba,
            namespace: bn,
        },
    ) = (a, b)
    else {
        return false;
    };
    context(ac, bc, proof)
        && ast_equal::params(ap, bp)
        && ast_equal::block(ab, bb)
        && map(av, bv, value)
        && map::<TypeExpr>(at, bt, ast_equal::type_expr)
        && aa == ba
        && an == bn
}
pub(super) fn value(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (
            Value::Typed {
                type_name: an,
                value: a,
            },
            Value::Typed {
                type_name: bn,
                value: b,
            },
        ) => an == bn && value(a, b),
        (Value::Int64(a), Value::Int64(b)) => a == b,
        (Value::Uint64(a), Value::Uint64(b)) => a == b,
        (Value::Float64(a), Value::Float64(b)) => a.to_bits() == b.to_bits(),
        (Value::String(a), Value::String(b))
        | (Value::Capability(a), Value::Capability(b))
        | (Value::Error(a), Value::Error(b))
        | (Value::NamedFunction(a), Value::NamedFunction(b)) => a == b,
        (Value::Bool(a), Value::Bool(b)) => a == b,
        (Value::Bytes(a), Value::Bytes(b)) => a == b,
        (Value::Actor(a), Value::Actor(b)) => a == b,
        (Value::OptionalNone, Value::OptionalNone) | (Value::Nothing, Value::Nothing) => true,
        (Value::ResultOk(a), Value::ResultOk(b))
        | (Value::ResultFail(a), Value::ResultFail(b))
        | (Value::OptionalSome(a), Value::OptionalSome(b))
        | (Value::Pending(a), Value::Pending(b)) => value(a, b),
        (Value::List(a), Value::List(b)) | (Value::Set(a), Value::Set(b)) => {
            ast_equal::list(a, b, value)
        }
        (Value::Map(a), Value::Map(b)) => {
            ast_equal::list(a, b, |(ak, av), (bk, bv)| value(ak, bk) && value(av, bv))
        }
        (
            Value::TypeConstruction {
                type_name: an,
                variant: ar,
                state: as_,
                fields: a,
            },
            Value::TypeConstruction {
                type_name: bn,
                variant: br,
                state: bs,
                fields: b,
            },
        ) => {
            an == bn
                && ar == br
                && as_ == bs
                && ast_equal::list(a, b, |(ai, an, at, av), (bi, bn, bt, bv)| {
                    ai == bi && an == bn && at == bt && value(av, bv)
                })
        }
        (
            Value::Struct {
                type_name: an,
                concrete_type: ac,
                fields: a,
            },
            Value::Struct {
                type_name: bn,
                concrete_type: bc,
                fields: b,
            },
        ) => {
            an == bn
                && ac == bc
                && ast_equal::list(a, b, |(an, av), (bn, bv)| an == bn && value(av, bv))
        }
        (
            Value::Enum {
                type_name: an,
                variant: ar,
                fields: a,
            },
            Value::Enum {
                type_name: bn,
                variant: br,
                fields: b,
            },
        )
        | (
            Value::Machine {
                type_name: an,
                state: ar,
                fields: a,
            },
            Value::Machine {
                type_name: bn,
                state: br,
                fields: b,
            },
        ) => an == bn && ar == br && ast_equal::list(a, b, value),
        (Value::ResourceHook(a), Value::ResourceHook(b)) => a.same_descriptor(b),
        (Value::ReflectedCallable(a), Value::ReflectedCallable(b)) => a.same_snapshot(b),
        (Value::Function { .. }, Value::Function { .. }) => closure(a, b, true),
        // Capture construction already refuses live custody; never copy or
        // inspect it to manufacture equality for this unrelated capability.
        (Value::Resource(_), Value::Resource(_))
        | (Value::GrantedNetwork(_), Value::GrantedNetwork(_)) => false,
        _ => false,
    }
}
