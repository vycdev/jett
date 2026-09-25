//! Materialize explicitly evaluated values as typed, compiler-owned HIR.
//!
//! Ordinary expressions are never evaluated here. A failed or absent baked
//! value cannot fall back to running the original computation at runtime.

use jett_common::Span;
use jett_comptime::Value;
use jett_hir::{
    Block, Expression, ExpressionKind as E, LowerError, Program, StatementKind as S, StringSegment,
};
use jett_types::TypeInterner;
use std::collections::HashMap;

use crate::native_property_cases::value_expression;

pub(crate) fn bake_values(
    program: &mut Program,
    values: &HashMap<Span, Value>,
    types: &TypeInterner,
) -> Result<(), Vec<LowerError>> {
    let mut baker = Baker {
        values,
        types,
        errors: Vec::new(),
    };
    for function in &mut program.functions {
        baker.block(&mut function.body);
    }
    if baker.errors.is_empty() {
        Ok(())
    } else {
        Err(baker.errors)
    }
}

struct Baker<'a> {
    values: &'a HashMap<Span, Value>,
    types: &'a TypeInterner,
    errors: Vec<LowerError>,
}

impl Baker<'_> {
    fn block(&mut self, body: &mut Block) {
        for statement in &mut body.statements {
            match &mut statement.kind {
                S::Let { value, .. }
                | S::Expression(value)
                | S::HandleDefault(value)
                | S::Respond(value) => self.expression(value),
                S::Assign { target, value } => {
                    self.expression(target);
                    self.expression(value);
                }
                S::Return(value)
                | S::Breakpoint {
                    condition: value, ..
                } => {
                    if let Some(value) = value {
                        self.expression(value);
                    }
                }
                S::If {
                    condition,
                    then_block,
                    else_block,
                } => {
                    self.expression(condition);
                    self.block(then_block);
                    if let Some(body) = else_block {
                        self.block(body);
                    }
                }
                S::While { condition, body } => {
                    self.expression(condition);
                    self.block(body);
                }
                S::For { iterable, body, .. } => {
                    self.expression(iterable);
                    self.block(body);
                }
                S::Match { scrutinee, arms } => {
                    self.expression(scrutinee);
                    for arm in arms {
                        self.block(&mut arm.body);
                    }
                }
                S::Assert { condition, message } => {
                    self.expression(condition);
                    if let Some(message) = message {
                        self.expression(message);
                    }
                }
                S::Scope(body) => self.block(body),
                S::ReflectedTypeDispatch { type_info, arms } => {
                    self.expression(type_info);
                    for arm in arms {
                        self.block(&mut arm.body);
                    }
                }
                S::Break | S::Continue | S::Trace(_) => {}
            }
        }
    }

    fn expression(&mut self, expr: &mut Expression) {
        if matches!(expr.kind, E::Comptime(_)) {
            match self.values.get(&expr.span) {
                Some(value) => match value_expression(value, expr.ty, expr.span, self.types) {
                    Ok(replacement) => *expr = replacement,
                    Err(message) => self.errors.push(LowerError {
                        span: expr.span,
                        message: format!("cannot materialize compile-time value: {message}"),
                    }),
                },
                None => self.errors.push(LowerError {
                    span: expr.span,
                    message: "explicit comptime expression has no evaluated value".into(),
                }),
            }
            return;
        }
        match &mut expr.kind {
            E::Binary { left, right, .. } => {
                self.expression(left);
                self.expression(right);
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
            | E::Field { base: value, .. } => self.expression(value),
            E::Call { args, .. }
            | E::Intrinsic { args, .. }
            | E::ActorSpawn { args, .. }
            | E::StructConstruct { fields: args, .. }
            | E::BitfieldConstruct { fields: args, .. }
            | E::MachineConstruct { payloads: args, .. }
            | E::EnumConstruct { payloads: args, .. }
            | E::ListConstruct { elements: args } => self.expressions(args),
            E::IndirectCall { callee, args, .. } => {
                self.expression(callee);
                self.expressions(args);
            }
            E::MachineTransition {
                source, payloads, ..
            } => {
                self.expression(source);
                self.expressions(payloads);
            }
            E::ActorMessage { actor, args, .. } => {
                self.expression(actor);
                self.expressions(args);
            }
            E::MapConstruct { entries } => {
                for entry in entries {
                    self.expression(&mut entry.key);
                    self.expression(&mut entry.value);
                }
            }
            E::Handle {
                target, failure, ..
            } => {
                self.expression(target);
                self.block(failure);
            }
            E::StringInterpolation(parts) => {
                for part in parts {
                    if let StringSegment::Value(value) = part {
                        self.expression(value);
                    }
                }
            }
            E::InlineFunction { body, .. } => self.block(body),
            E::Int(_)
            | E::Float(_)
            | E::String(_)
            | E::Bool(_)
            | E::Nothing
            | E::OptionalNone
            | E::Local(_)
            | E::FunctionRef(_)
            | E::ClosureRef { .. }
            | E::Comptime(_) => {}
        }
    }

    fn expressions(&mut self, expressions: &mut [Expression]) {
        for expr in expressions {
            self.expression(expr);
        }
    }
}
