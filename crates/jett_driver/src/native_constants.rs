//! Import explicitly evaluated primitive constants into the typed handoff.
//!
//! This pass never evaluates ordinary expressions. Composite baked values stay
//! explicitly marked Comptime until their native constant layouts are defined;
//! the backend rejects those markers rather than running their source bodies.

use jett_common::Span;
use jett_comptime::Value;
use jett_hir::{
    Block, Expression, ExpressionKind as E, Program, StatementKind as S, StringSegment,
};
use std::collections::HashMap;

pub(crate) fn bake_primitives(program: &mut Program, values: &HashMap<Span, Value>) {
    for function in &mut program.functions {
        block(&mut function.body, values);
    }
}

fn block(body: &mut Block, values: &HashMap<Span, Value>) {
    for statement in &mut body.statements {
        match &mut statement.kind {
            S::Let { value, .. }
            | S::Expression(value)
            | S::HandleDefault(value)
            | S::Respond(value) => expression(value, values),
            S::Assign { target, value } => {
                expression(target, values);
                expression(value, values);
            }
            S::Return(value)
            | S::Breakpoint {
                condition: value, ..
            } => {
                if let Some(value) = value {
                    expression(value, values);
                }
            }
            S::If {
                condition,
                then_block,
                else_block,
            } => {
                expression(condition, values);
                block(then_block, values);
                if let Some(body) = else_block {
                    block(body, values);
                }
            }
            S::While { condition, body } => {
                expression(condition, values);
                block(body, values);
            }
            S::For { iterable, body, .. } => {
                expression(iterable, values);
                block(body, values);
            }
            S::Match { scrutinee, arms } => {
                expression(scrutinee, values);
                for arm in arms {
                    block(&mut arm.body, values);
                }
            }
            S::Assert { condition, message } => {
                expression(condition, values);
                if let Some(message) = message {
                    expression(message, values);
                }
            }
            S::Scope(body) => block(body, values),
            S::ReflectedTypeDispatch { type_info, arms } => {
                expression(type_info, values);
                for arm in arms {
                    block(&mut arm.body, values);
                }
            }
            S::Break | S::Continue | S::Trace(_) => {}
        }
    }
}

fn expression(expr: &mut Expression, values: &HashMap<Span, Value>) {
    if matches!(expr.kind, E::Comptime(_)) {
        let replacement = match values.get(&expr.span) {
            Some(Value::Int64(value)) => Some(E::Int(i128::from(*value))),
            Some(Value::Uint64(value)) => Some(E::Int(i128::from(*value))),
            Some(Value::Float64(value)) => Some(E::Float(*value)),
            Some(Value::Bool(value)) => Some(E::Bool(*value)),
            Some(Value::String(value)) => Some(E::String(value.clone())),
            Some(Value::Nothing) => Some(E::Nothing),
            _ => None,
        };
        if let Some(replacement) = replacement {
            expr.kind = replacement;
        }
        // Never lower or execute the original computation as a substitute for
        // a missing baked value. Preserve the checked type and span exactly.
        return;
    }
    match &mut expr.kind {
        E::Binary { left, right, .. } => {
            expression(left, values);
            expression(right, values);
        }
        E::Unary { value, .. }
        | E::ResultOk(value)
        | E::ResultFail(value)
        | E::OptionalSome(value)
        | E::Declassify(value)
        | E::Coarsen(value)
        | E::RefinementValidated(value)
        | E::StateIs { value, .. }
        | E::Run(value)
        | E::Join(value)
        | E::Cancel(value)
        | E::View(value)
        | E::Clone(value)
        | E::Field { base: value, .. } => expression(value, values),
        E::Call { args, .. }
        | E::Intrinsic { args, .. }
        | E::ActorSpawn { args, .. }
        | E::StructConstruct { fields: args, .. }
        | E::BitfieldConstruct { fields: args, .. }
        | E::MachineConstruct { payloads: args, .. }
        | E::EnumConstruct { payloads: args, .. }
        | E::ListConstruct { elements: args } => expressions(args, values),
        E::IndirectCall { callee, args, .. } => {
            expression(callee, values);
            expressions(args, values);
        }
        E::MachineTransition {
            source, payloads, ..
        } => {
            expression(source, values);
            expressions(payloads, values);
        }
        E::ActorMessage { actor, args, .. } => {
            expression(actor, values);
            expressions(args, values);
        }
        E::MapConstruct { entries } => {
            for entry in entries {
                expression(&mut entry.key, values);
                expression(&mut entry.value, values);
            }
        }
        E::Handle {
            target, failure, ..
        } => {
            expression(target, values);
            block(failure, values);
        }
        E::StringInterpolation(parts) => {
            for part in parts {
                if let StringSegment::Value(value) = part {
                    expression(value, values);
                }
            }
        }
        E::InlineFunction { body, .. } => block(body, values),
        E::Int(_)
        | E::Float(_)
        | E::String(_)
        | E::Bool(_)
        | E::Nothing
        | E::OptionalNone
        | E::Local(_)
        | E::FunctionRef(_)
        | E::Comptime(_) => {}
    }
}

fn expressions(expressions: &mut [Expression], values: &HashMap<Span, Value>) {
    for expr in expressions {
        expression(expr, values);
    }
}
