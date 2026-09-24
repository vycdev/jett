//! Extract capture-free inline functions into ordinary checked HIR functions.
//! Captured closures keep their explicit HIR form until the native function
//! value ABI can carry an environment alongside its code address.

use crate::{
    Block, DeclarationId, DeclarationKind, Expression, ExpressionKind, Function, FunctionId,
    FunctionIdentity, Local, Param, ParamMode, StatementKind, StringSegment,
};
use jett_types::{Type, TypeInterner};

pub(super) fn extract_capture_free(functions: &mut Vec<Function>, types: &TypeInterner) {
    let mut extractor = Extractor {
        types,
        next_id: functions.len() as u32,
        pending: Vec::new(),
    };
    let mut index = 0;
    while index < functions.len() {
        let function = &mut functions[index];
        let parent = ParentInfo {
            identity: &function.identity,
            locals: &function.locals,
        };
        extractor.block(&mut function.body, &parent);
        functions.append(&mut extractor.pending);
        index += 1;
    }
}

struct ParentInfo<'a> {
    identity: &'a FunctionIdentity,
    locals: &'a [Local],
}

struct Extractor<'a> {
    types: &'a TypeInterner,
    next_id: u32,
    pending: Vec<Function>,
}

impl Extractor<'_> {
    fn block(&mut self, block: &mut Block, parent: &ParentInfo<'_>) {
        for statement in &mut block.statements {
            match &mut statement.kind {
                StatementKind::Let { value, .. }
                | StatementKind::HandleDefault(value)
                | StatementKind::Expression(value)
                | StatementKind::Respond(value) => self.expression(value, parent),
                StatementKind::Assign { target, value } => {
                    self.expression(target, parent);
                    self.expression(value, parent);
                }
                StatementKind::Return(value) => {
                    if let Some(value) = value {
                        self.expression(value, parent);
                    }
                }
                StatementKind::If {
                    condition,
                    then_block,
                    else_block,
                } => {
                    self.expression(condition, parent);
                    self.block(then_block, parent);
                    if let Some(else_block) = else_block {
                        self.block(else_block, parent);
                    }
                }
                StatementKind::While { condition, body } => {
                    self.expression(condition, parent);
                    self.block(body, parent);
                }
                StatementKind::For { iterable, body, .. } => {
                    self.expression(iterable, parent);
                    self.block(body, parent);
                }
                StatementKind::Match { scrutinee, arms } => {
                    self.expression(scrutinee, parent);
                    for arm in arms {
                        self.block(&mut arm.body, parent);
                    }
                }
                StatementKind::Assert { condition, message } => {
                    self.expression(condition, parent);
                    if let Some(message) = message {
                        self.expression(message, parent);
                    }
                }
                StatementKind::Breakpoint { condition, .. } => {
                    if let Some(condition) = condition {
                        self.expression(condition, parent);
                    }
                }
                StatementKind::Scope(body) => self.block(body, parent),
                StatementKind::ReflectedTypeDispatch { type_info, arms } => {
                    self.expression(type_info, parent);
                    for arm in arms {
                        self.block(&mut arm.body, parent);
                    }
                }
                StatementKind::Break | StatementKind::Continue | StatementKind::Trace(_) => {}
            }
        }
    }

    fn expression(&mut self, expression: &mut Expression, parent: &ParentInfo<'_>) {
        if let ExpressionKind::InlineFunction {
            params,
            view_params,
            local_floor,
            body,
        } = &expression.kind
        {
            if !view_params.is_empty() || block_uses_capture(body, *local_floor) {
                return;
            }
            let Type::Function {
                params: expected,
                return_type,
            } = self.types.resolve(expression.ty)
            else {
                return;
            };
            if params.len() != expected.len() {
                return;
            }
            let parameters = params
                .iter()
                .zip(expected)
                .map(|(id, expected_ty)| {
                    let local = parent.locals.get(id.index() as usize)?;
                    (local.id == *id && local.ty == *expected_ty).then(|| Param {
                        local: *id,
                        name: local.name.clone(),
                        ty: local.ty,
                        mode: ParamMode::Owned,
                        mutable: local.mutable,
                        span: local.span,
                    })
                })
                .collect::<Option<Vec<_>>>();
            let Some(parameters) = parameters else {
                return;
            };
            let id = FunctionId(self.next_id);
            self.next_id += 1;
            let name = format!(
                "{}$inline{}_{}",
                parent.identity.declaration.name, expression.span.start, expression.span.end
            );
            self.pending.push(Function {
                id,
                identity: FunctionIdentity {
                    declaration: DeclarationId {
                        origin: parent.identity.declaration.origin.clone(),
                        namespace: parent.identity.declaration.namespace.clone(),
                        name,
                        kind: DeclarationKind::Function,
                    },
                    type_arguments: parent.identity.type_arguments.clone(),
                    specialization: parent.identity.specialization.clone(),
                },
                source_definition: None,
                params: parameters,
                return_type: *return_type,
                locals: parent.locals.to_vec(),
                body: body.clone(),
                span: expression.span,
            });
            expression.kind = ExpressionKind::FunctionRef(id);
            return;
        }
        match &mut expression.kind {
            ExpressionKind::Binary { left, right, .. } => {
                self.expression(left, parent);
                self.expression(right, parent);
            }
            ExpressionKind::Unary { value, .. }
            | ExpressionKind::ResultOk(value)
            | ExpressionKind::ResultFail(value)
            | ExpressionKind::OptionalSome(value)
            | ExpressionKind::Comptime(value)
            | ExpressionKind::Declassify(value)
            | ExpressionKind::Coarsen(value)
            | ExpressionKind::RefinementValidated(value)
            | ExpressionKind::StateIs { value, .. }
            | ExpressionKind::Run(value)
            | ExpressionKind::Join(value)
            | ExpressionKind::Cancel(value)
            | ExpressionKind::Field { base: value, .. }
            | ExpressionKind::View(value)
            | ExpressionKind::Clone(value) => self.expression(value, parent),
            ExpressionKind::Call { args, .. }
            | ExpressionKind::Intrinsic { args, .. }
            | ExpressionKind::ActorSpawn { args, .. } => {
                for argument in args {
                    self.expression(argument, parent);
                }
            }
            ExpressionKind::IndirectCall { callee, args, .. } => {
                self.expression(callee, parent);
                for argument in args {
                    self.expression(argument, parent);
                }
            }
            ExpressionKind::StructConstruct { fields, .. }
            | ExpressionKind::BitfieldConstruct { fields, .. } => {
                for field in fields {
                    self.expression(field, parent);
                }
            }
            ExpressionKind::MachineConstruct { payloads, .. }
            | ExpressionKind::EnumConstruct { payloads, .. } => {
                for payload in payloads {
                    self.expression(payload, parent);
                }
            }
            ExpressionKind::MachineTransition {
                source, payloads, ..
            } => {
                self.expression(source, parent);
                for payload in payloads {
                    self.expression(payload, parent);
                }
            }
            ExpressionKind::ListConstruct { elements } => {
                for element in elements {
                    self.expression(element, parent);
                }
            }
            ExpressionKind::MapConstruct { entries } => {
                for entry in entries {
                    self.expression(&mut entry.key, parent);
                    self.expression(&mut entry.value, parent);
                }
            }
            ExpressionKind::Handle {
                target, failure, ..
            } => {
                self.expression(target, parent);
                self.block(failure, parent);
            }
            ExpressionKind::StringInterpolation(parts) => {
                for part in parts {
                    if let StringSegment::Value(value) = part {
                        self.expression(value, parent);
                    }
                }
            }
            ExpressionKind::ActorMessage { actor, args, .. } => {
                self.expression(actor, parent);
                for argument in args {
                    self.expression(argument, parent);
                }
            }
            ExpressionKind::Int(_)
            | ExpressionKind::Float(_)
            | ExpressionKind::String(_)
            | ExpressionKind::Bool(_)
            | ExpressionKind::Nothing
            | ExpressionKind::Local(_)
            | ExpressionKind::FunctionRef(_)
            | ExpressionKind::OptionalNone
            | ExpressionKind::InlineFunction { .. } => {}
        }
    }
}

fn block_uses_capture(block: &Block, floor: u32) -> bool {
    block
        .statements
        .iter()
        .any(|statement| match &statement.kind {
            StatementKind::Let { value, .. }
            | StatementKind::HandleDefault(value)
            | StatementKind::Expression(value)
            | StatementKind::Respond(value) => expression_uses_capture(value, floor),
            StatementKind::Assign { target, value } => {
                expression_uses_capture(target, floor) || expression_uses_capture(value, floor)
            }
            StatementKind::Return(value) => value
                .as_ref()
                .is_some_and(|value| expression_uses_capture(value, floor)),
            StatementKind::If {
                condition,
                then_block,
                else_block,
            } => {
                expression_uses_capture(condition, floor)
                    || block_uses_capture(then_block, floor)
                    || else_block
                        .as_ref()
                        .is_some_and(|block| block_uses_capture(block, floor))
            }
            StatementKind::While { condition, body } => {
                expression_uses_capture(condition, floor) || block_uses_capture(body, floor)
            }
            StatementKind::For { iterable, body, .. } => {
                expression_uses_capture(iterable, floor) || block_uses_capture(body, floor)
            }
            StatementKind::Match { scrutinee, arms } => {
                expression_uses_capture(scrutinee, floor)
                    || arms.iter().any(|arm| block_uses_capture(&arm.body, floor))
            }
            StatementKind::Assert { condition, message } => {
                expression_uses_capture(condition, floor)
                    || message
                        .as_ref()
                        .is_some_and(|message| expression_uses_capture(message, floor))
            }
            StatementKind::Trace(local) => local.index() < floor,
            StatementKind::Breakpoint {
                condition,
                bindings,
            } => {
                condition
                    .as_ref()
                    .is_some_and(|condition| expression_uses_capture(condition, floor))
                    || bindings.iter().any(|binding| binding.index() < floor)
            }
            StatementKind::Scope(body) => block_uses_capture(body, floor),
            StatementKind::ReflectedTypeDispatch { type_info, arms } => {
                expression_uses_capture(type_info, floor)
                    || arms.iter().any(|arm| block_uses_capture(&arm.body, floor))
            }
            StatementKind::Break | StatementKind::Continue => false,
        })
}

fn expression_uses_capture(expression: &Expression, floor: u32) -> bool {
    match &expression.kind {
        ExpressionKind::Local(local) => local.index() < floor,
        ExpressionKind::Binary { left, right, .. } => {
            expression_uses_capture(left, floor) || expression_uses_capture(right, floor)
        }
        ExpressionKind::Unary { value, .. }
        | ExpressionKind::ResultOk(value)
        | ExpressionKind::ResultFail(value)
        | ExpressionKind::OptionalSome(value)
        | ExpressionKind::Comptime(value)
        | ExpressionKind::Declassify(value)
        | ExpressionKind::Coarsen(value)
        | ExpressionKind::RefinementValidated(value)
        | ExpressionKind::StateIs { value, .. }
        | ExpressionKind::Run(value)
        | ExpressionKind::Join(value)
        | ExpressionKind::Cancel(value)
        | ExpressionKind::Field { base: value, .. }
        | ExpressionKind::View(value)
        | ExpressionKind::Clone(value) => expression_uses_capture(value, floor),
        ExpressionKind::Call { args, .. }
        | ExpressionKind::Intrinsic { args, .. }
        | ExpressionKind::ActorSpawn { args, .. } => {
            args.iter().any(|arg| expression_uses_capture(arg, floor))
        }
        ExpressionKind::IndirectCall { callee, args, .. } => {
            expression_uses_capture(callee, floor)
                || args.iter().any(|arg| expression_uses_capture(arg, floor))
        }
        ExpressionKind::StructConstruct { fields, .. }
        | ExpressionKind::BitfieldConstruct { fields, .. } => fields
            .iter()
            .any(|field| expression_uses_capture(field, floor)),
        ExpressionKind::MachineConstruct { payloads, .. }
        | ExpressionKind::EnumConstruct { payloads, .. } => payloads
            .iter()
            .any(|payload| expression_uses_capture(payload, floor)),
        ExpressionKind::MachineTransition {
            source, payloads, ..
        } => {
            expression_uses_capture(source, floor)
                || payloads
                    .iter()
                    .any(|payload| expression_uses_capture(payload, floor))
        }
        ExpressionKind::ListConstruct { elements } => elements
            .iter()
            .any(|element| expression_uses_capture(element, floor)),
        ExpressionKind::MapConstruct { entries } => entries.iter().any(|entry| {
            expression_uses_capture(&entry.key, floor)
                || expression_uses_capture(&entry.value, floor)
        }),
        ExpressionKind::Handle {
            target, failure, ..
        } => expression_uses_capture(target, floor) || block_uses_capture(failure, floor),
        ExpressionKind::StringInterpolation(parts) => parts.iter().any(|part| match part {
            StringSegment::Text(_) => false,
            StringSegment::Value(value) => expression_uses_capture(value, floor),
        }),
        ExpressionKind::InlineFunction { body, .. } => block_uses_capture(body, floor),
        ExpressionKind::ActorMessage { actor, args, .. } => {
            expression_uses_capture(actor, floor)
                || args.iter().any(|arg| expression_uses_capture(arg, floor))
        }
        ExpressionKind::Int(_)
        | ExpressionKind::Float(_)
        | ExpressionKind::String(_)
        | ExpressionKind::Bool(_)
        | ExpressionKind::Nothing
        | ExpressionKind::FunctionRef(_)
        | ExpressionKind::OptionalNone => false,
    }
}
