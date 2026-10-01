//! Materialize explicitly evaluated values as typed, compiler-owned HIR.
//!
//! Ordinary expressions are never evaluated here. A failed or absent baked
//! value cannot fall back to running the original computation at runtime.

use jett_common::Span;
use jett_comptime::{ComptimeContext, ExplicitComptimeValues};
use jett_hir::{
    Block, Expression, ExpressionKind as E, Local, LowerError, Program, Statement,
    StatementKind as S, StringSegment,
};
use jett_types::{ReflectionMetadata, TypeInterner};
use std::collections::HashSet;

use crate::native_property_cases::{
    FunctionValueCandidate, ValueContext, function_value_candidates, value_expression,
};

pub(crate) fn bake_values(
    program: &mut Program,
    values: &ExplicitComptimeValues,
    types: &TypeInterner,
    reflection: &ReflectionMetadata,
    method_value_definitions: &HashSet<Span>,
) -> Result<(), Vec<LowerError>> {
    let function_values = function_value_candidates(&program.functions, method_value_definitions);
    let mut errors = Vec::new();
    for function in &mut program.functions {
        let specialization = &function.identity.specialization;
        let mut baker = Baker {
            context: ComptimeContext {
                type_arguments: function
                    .identity
                    .type_arguments
                    .iter()
                    .map(|ty| types.type_name(*ty))
                    .collect(),
                type_argument_reflections: specialization.type_argument_reflections.clone(),
                type_info_kinds: specialization.type_info_kinds.clone(),
                type_info_primitives: specialization.type_info_primitives.clone(),
                type_kind_values: specialization.type_kind_values.clone(),
                type_primitive_values: specialization.type_primitive_values.clone(),
                scoped_type_bindings: scoped_bindings(
                    &function.identity.scoped_type_bindings,
                    types,
                ),
            },
            values,
            types,
            reflection,
            function_values: &function_values,
            locals: &mut function.locals,
            bindings: Vec::new(),
            errors: Vec::new(),
        };
        baker.block(&mut function.body);
        function.body.statements.splice(0..0, baker.bindings);
        errors.extend(baker.errors);
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

fn scoped_bindings(
    bindings: &[jett_hir::ScopedTypeBinding],
    types: &TypeInterner,
) -> Vec<jett_comptime::value::ClosureScopedTypeBinding> {
    bindings
        .iter()
        .map(|binding| jett_comptime::value::ClosureScopedTypeBinding {
            name: binding.name.clone(),
            canonical_name: types.type_name(binding.ty),
            reflection: Some(binding.reflection.clone()),
        })
        .collect()
}

struct Baker<'a> {
    context: ComptimeContext,
    values: &'a ExplicitComptimeValues,
    types: &'a TypeInterner,
    reflection: &'a ReflectionMetadata,
    function_values: &'a [FunctionValueCandidate],
    locals: &'a mut Vec<Local>,
    bindings: Vec<Statement>,
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
        if let E::Constant { declaration } = expr.kind {
            match self.values.constant(declaration) {
                Some(value) => match value_expression(
                    value,
                    expr.ty,
                    expr.span,
                    &mut ValueContext {
                        types: self.types,
                        reflection: self.reflection,
                        functions: self.function_values,
                        refinement_functions: None,
                        locals: &mut *self.locals,
                        bindings: &mut self.bindings,
                    },
                ) {
                    Ok(replacement) => *expr = replacement,
                    Err(message) => self.errors.push(LowerError {
                        span: expr.span,
                        message: format!("cannot materialize constant: {message}"),
                    }),
                },
                None => self.errors.push(LowerError {
                    span: expr.span,
                    message: "namespace constant has no checked compile-time value".into(),
                }),
            }
            return;
        }
        if let E::Comptime {
            bindings,
            source_span,
            ..
        } = &expr.kind
        {
            let mut context = self.context.clone();
            if !bindings.is_empty() {
                context.scoped_type_bindings = scoped_bindings(bindings, self.types);
            }
            match self.values.get(*source_span, &context) {
                Some(value) => match value_expression(
                    value,
                    expr.ty,
                    expr.span,
                    &mut ValueContext {
                        types: self.types,
                        reflection: self.reflection,
                        functions: self.function_values,
                        refinement_functions: None,
                        locals: &mut *self.locals,
                        bindings: &mut self.bindings,
                    },
                ) {
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
            | E::DisplayResult(value)
            | E::EquatableResult(value)
            | E::FunctionAdapter { value, .. }
            | E::InterfaceCoerce { value, .. }
            | E::InterfaceType(value)
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
            | E::Constant { .. }
            | E::Float(_)
            | E::String(_)
            | E::Bool(_)
            | E::Nothing
            | E::PropertyCaseContext(_)
            | E::RuntimeFailure(_)
            | E::OptionalNone
            | E::Local(_)
            | E::FunctionRef(_)
            | E::ClosureRef { .. }
            | E::Comptime { .. } => {}
        }
    }

    fn expressions(&mut self, expressions: &mut [Expression]) {
        for expr in expressions {
            self.expression(expr);
        }
    }
}
