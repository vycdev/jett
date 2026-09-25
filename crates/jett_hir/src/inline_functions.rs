//! Extract inline functions into ordinary checked HIR functions. Captured
//! locals become leading synthetic parameters supplied by a native environment.

use crate::{
    Block, DeclarationId, DeclarationKind, Expression, ExpressionKind, Function, FunctionId,
    FunctionIdentity, Local, LocalId, Param, ParamMode, StatementKind, StringSegment,
};
use jett_types::{Type, TypeInterner};

pub(super) fn extract_inline_functions(functions: &mut Vec<Function>, types: &TypeInterner) {
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
            let captures = (0..*local_floor)
                .filter(|id| block_uses_local(body, *id))
                .map(LocalId::new)
                .collect::<Vec<_>>();
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
                        mode: if view_params.contains(id) {
                            ParamMode::View
                        } else {
                            ParamMode::Owned
                        },
                        mutable: local.mutable,
                        span: local.span,
                    })
                })
                .collect::<Option<Vec<_>>>();
            let Some(parameters) = parameters else {
                return;
            };
            let capture_parameters = captures
                .iter()
                .map(|id| {
                    let local = parent.locals.get(id.index() as usize)?;
                    (local.id == *id).then(|| Param {
                        local: *id,
                        name: local.name.clone(),
                        ty: local.ty,
                        mode: ParamMode::Owned,
                        mutable: local.mutable,
                        span: local.span,
                    })
                })
                .collect::<Option<Vec<_>>>();
            let Some(mut capture_parameters) = capture_parameters else {
                return;
            };
            let capture_count = capture_parameters.len();
            capture_parameters.extend(parameters);
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
                params: capture_parameters,
                capture_count,
                return_type: *return_type,
                locals: parent.locals.to_vec(),
                body: body.clone(),
                span: expression.span,
            });
            expression.kind = if captures.is_empty() {
                ExpressionKind::FunctionRef(id)
            } else {
                ExpressionKind::ClosureRef {
                    function: id,
                    captures,
                }
            };
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
            | ExpressionKind::ClosureRef { .. }
            | ExpressionKind::OptionalNone
            | ExpressionKind::InlineFunction { .. } => {}
        }
    }
}

fn block_uses_local(block: &Block, target: u32) -> bool {
    block
        .statements
        .iter()
        .any(|statement| match &statement.kind {
            StatementKind::Let { value, .. }
            | StatementKind::HandleDefault(value)
            | StatementKind::Expression(value)
            | StatementKind::Respond(value) => expression_uses_local(value, target),
            StatementKind::Assign {
                target: place,
                value,
            } => expression_uses_local(place, target) || expression_uses_local(value, target),
            StatementKind::Return(value) => value
                .as_ref()
                .is_some_and(|value| expression_uses_local(value, target)),
            StatementKind::If {
                condition,
                then_block,
                else_block,
            } => {
                expression_uses_local(condition, target)
                    || block_uses_local(then_block, target)
                    || else_block
                        .as_ref()
                        .is_some_and(|block| block_uses_local(block, target))
            }
            StatementKind::While { condition, body } => {
                expression_uses_local(condition, target) || block_uses_local(body, target)
            }
            StatementKind::For { iterable, body, .. } => {
                expression_uses_local(iterable, target) || block_uses_local(body, target)
            }
            StatementKind::Match { scrutinee, arms } => {
                expression_uses_local(scrutinee, target)
                    || arms.iter().any(|arm| block_uses_local(&arm.body, target))
            }
            StatementKind::Assert { condition, message } => {
                expression_uses_local(condition, target)
                    || message
                        .as_ref()
                        .is_some_and(|message| expression_uses_local(message, target))
            }
            StatementKind::Trace(local) => local.index() == target,
            StatementKind::Breakpoint {
                condition,
                bindings,
            } => {
                condition
                    .as_ref()
                    .is_some_and(|condition| expression_uses_local(condition, target))
                    || bindings.iter().any(|binding| binding.index() == target)
            }
            StatementKind::Scope(body) => block_uses_local(body, target),
            StatementKind::ReflectedTypeDispatch { type_info, arms } => {
                expression_uses_local(type_info, target)
                    || arms.iter().any(|arm| block_uses_local(&arm.body, target))
            }
            StatementKind::Break | StatementKind::Continue => false,
        })
}

fn expression_uses_local(expression: &Expression, target: u32) -> bool {
    match &expression.kind {
        ExpressionKind::Local(local) => local.index() == target,
        ExpressionKind::Binary { left, right, .. } => {
            expression_uses_local(left, target) || expression_uses_local(right, target)
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
        | ExpressionKind::Clone(value) => expression_uses_local(value, target),
        ExpressionKind::Call { args, .. }
        | ExpressionKind::Intrinsic { args, .. }
        | ExpressionKind::ActorSpawn { args, .. } => {
            args.iter().any(|arg| expression_uses_local(arg, target))
        }
        ExpressionKind::IndirectCall { callee, args, .. } => {
            expression_uses_local(callee, target)
                || args.iter().any(|arg| expression_uses_local(arg, target))
        }
        ExpressionKind::StructConstruct { fields, .. }
        | ExpressionKind::BitfieldConstruct { fields, .. } => fields
            .iter()
            .any(|field| expression_uses_local(field, target)),
        ExpressionKind::MachineConstruct { payloads, .. }
        | ExpressionKind::EnumConstruct { payloads, .. } => payloads
            .iter()
            .any(|payload| expression_uses_local(payload, target)),
        ExpressionKind::MachineTransition {
            source, payloads, ..
        } => {
            expression_uses_local(source, target)
                || payloads
                    .iter()
                    .any(|payload| expression_uses_local(payload, target))
        }
        ExpressionKind::ListConstruct { elements } => elements
            .iter()
            .any(|element| expression_uses_local(element, target)),
        ExpressionKind::MapConstruct { entries } => entries.iter().any(|entry| {
            expression_uses_local(&entry.key, target) || expression_uses_local(&entry.value, target)
        }),
        ExpressionKind::Handle {
            target: handled,
            failure,
            ..
        } => expression_uses_local(handled, target) || block_uses_local(failure, target),
        ExpressionKind::StringInterpolation(parts) => parts.iter().any(|part| match part {
            StringSegment::Text(_) => false,
            StringSegment::Value(value) => expression_uses_local(value, target),
        }),
        ExpressionKind::InlineFunction { body, .. } => block_uses_local(body, target),
        ExpressionKind::ClosureRef { captures, .. } => {
            captures.iter().any(|local| local.index() == target)
        }
        ExpressionKind::ActorMessage { actor, args, .. } => {
            expression_uses_local(actor, target)
                || args.iter().any(|arg| expression_uses_local(arg, target))
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
