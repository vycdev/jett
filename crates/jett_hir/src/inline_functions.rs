//! Extract inline functions into ordinary checked HIR functions. Captured
//! locals become leading synthetic parameters supplied by a native environment.

use crate::{
    Block, DeclarationId, DeclarationKind, Expression, ExpressionKind, Function, FunctionDebugKind,
    FunctionId, FunctionIdentity, Local, LocalId, Param, ParamMode, StatementKind, StringSegment,
};
use jett_types::{Type, TypeInterner};
use std::collections::HashSet;

pub(super) fn extract_inline_functions(functions: &mut Vec<Function>, types: &TypeInterner) {
    let mut extractor = Extractor {
        types,
        next_id: functions.len() as u32,
        pending: Vec::new(),
        detached_aliases: HashSet::new(),
        generated_shape_rewrites: Vec::new(),
    };
    let mut index = 0;
    while index < functions.len() {
        let function = &mut functions[index];
        let parent = ParentInfo {
            identity: &function.identity,
            locals: &function.locals,
        };
        extractor.detached_aliases.clear();
        extractor.generated_shape_rewrites.clear();
        extractor.block(&mut function.body, &parent);
        // Only bodies actually transferred into extracted functions can leave
        // stale origins here. Arbitrary orphan metadata must remain an error.
        for &alias in &extractor.detached_aliases {
            if !block_mentions_local(&function.body, alias.index(), true) {
                function.locals[alias.index() as usize].view_source = None;
            }
        }
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
    detached_aliases: HashSet<LocalId>,
    generated_shape_rewrites: Vec<crate::call_ownership::GeneratedInlineExtraction>,
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
            scoped_type_bindings,
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
                view_params: expected_views,
                return_type,
            } = self.types.resolve(expression.ty)
            else {
                return;
            };
            if params.len() != expected.len()
                || params.len() != expected_views.len()
                || params
                    .iter()
                    .zip(expected_views)
                    .any(|(id, view)| view_params.contains(id) != *view)
            {
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
            let original_shape = expression.clone();
            let capture_count = capture_parameters.len();
            capture_parameters.extend(parameters);
            let id = FunctionId(self.next_id);
            self.next_id += 1;
            let name = format!(
                "{}$inline{}_{}",
                parent.identity.declaration.name, expression.span.start, expression.span.end
            );
            let mut locals = parent.locals.to_vec();
            self.detached_aliases
                .extend(parent.locals.iter().filter_map(|local| {
                    (local.view_source.is_some()
                        && local.id.index() >= *local_floor
                        && block_mentions_local(body, local.id.index(), true))
                    .then_some(local.id)
                }));
            // Captures are independent owned environment entries. Their local
            // metadata must not retain an origin in the enclosing function.
            for capture in &captures {
                locals[capture.index() as usize].view_source = None;
            }
            // The copied dense table includes parent and sibling bodies. Their
            // origins do not belong to this newly created function context.
            for local in &mut locals {
                if local.view_source.is_some()
                    && !block_mentions_local(body, local.id.index(), true)
                {
                    local.view_source = None;
                }
            }
            self.pending.push(Function {
                id,
                identity: FunctionIdentity {
                    declaration: DeclarationId {
                        origin: parent.identity.declaration.origin.clone(),
                        namespace: parent.identity.declaration.namespace.clone(),
                        name,
                        kind: match parent.identity.declaration.kind {
                            DeclarationKind::Verify | DeclarationKind::Property => {
                                parent.identity.declaration.kind
                            }
                            _ => DeclarationKind::Function,
                        },
                    },
                    scoped_type_bindings: scoped_type_bindings.clone(),
                    type_arguments: parent.identity.type_arguments.clone(),
                    specialization: parent.identity.specialization.clone(),
                },
                debug_kind: FunctionDebugKind::Inline,
                source_definition: None,
                params: capture_parameters,
                capture_count,
                return_type: *return_type,
                locals,
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
            if let Some(record) =
                crate::call_ownership::generated_inline_extraction(&original_shape, expression)
            {
                self.generated_shape_rewrites.push(record);
            }
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
            | ExpressionKind::Comptime { value, .. }
            | ExpressionKind::Declassify(value)
            | ExpressionKind::Coarsen(value)
            | ExpressionKind::RefinementValidated(value)
            | ExpressionKind::DisplayResult(value)
            | ExpressionKind::EquatableResult(value)
            | ExpressionKind::RuntimeFailureMessage(value)
            | ExpressionKind::FunctionAdapter { value, .. }
            | ExpressionKind::InterfaceCoerce { value, .. }
            | ExpressionKind::InterfaceType(value)
            | ExpressionKind::StateIs { value, .. }
            | ExpressionKind::Run(value)
            | ExpressionKind::Join(value)
            | ExpressionKind::Cancel(value)
            | ExpressionKind::Field { base: value, .. }
            | ExpressionKind::View(value)
            | ExpressionKind::Clone(value) => self.expression(value, parent),
            ExpressionKind::Call {
                args, ownership, ..
            }
            | ExpressionKind::Intrinsic {
                args, ownership, ..
            }
            | ExpressionKind::ResourceInvoke {
                args, ownership, ..
            } => {
                for argument in args {
                    self.expression(argument, parent);
                }
                ownership.apply_generated_inline_extractions(&self.generated_shape_rewrites);
            }
            ExpressionKind::ActorSpawn { args, .. } => {
                for argument in args {
                    self.expression(argument, parent);
                }
            }
            ExpressionKind::IndirectCall {
                callee,
                args,
                ownership,
                ..
            } => {
                self.expression(callee, parent);
                for argument in args {
                    self.expression(argument, parent);
                }
                ownership.apply_generated_inline_extractions(&self.generated_shape_rewrites);
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
            | ExpressionKind::Constant { .. }
            | ExpressionKind::Float(_)
            | ExpressionKind::String(_)
            | ExpressionKind::Bool(_)
            | ExpressionKind::Nothing
            | ExpressionKind::PropertyCaseContext(_)
            | ExpressionKind::RuntimeFailure(_)
            | ExpressionKind::Local(_)
            | ExpressionKind::ResourceHookValue { .. }
            | ExpressionKind::FunctionRef(_)
            | ExpressionKind::ClosureRef { .. }
            | ExpressionKind::OptionalNone
            | ExpressionKind::InlineFunction { .. } => {}
        }
    }
}

pub(crate) fn body_uses_local(block: &Block, target: LocalId) -> bool {
    block_uses_local(block, target.index())
}

fn block_uses_local(block: &Block, target: u32) -> bool {
    block_mentions_local(block, target, false)
}

fn block_mentions_local(block: &Block, target: u32, include_bindings: bool) -> bool {
    block
        .statements
        .iter()
        .any(|statement| match &statement.kind {
            StatementKind::Let { local, value } => {
                (include_bindings && local.index() == target)
                    || expression_mentions_local(value, target, include_bindings)
            }
            StatementKind::HandleDefault(value)
            | StatementKind::Expression(value)
            | StatementKind::Respond(value) => {
                expression_mentions_local(value, target, include_bindings)
            }
            StatementKind::Assign {
                target: place,
                value,
            } => {
                expression_mentions_local(place, target, include_bindings)
                    || expression_mentions_local(value, target, include_bindings)
            }
            StatementKind::Return(value) => value
                .as_ref()
                .is_some_and(|value| expression_mentions_local(value, target, include_bindings)),
            StatementKind::If {
                condition,
                then_block,
                else_block,
            } => {
                expression_mentions_local(condition, target, include_bindings)
                    || block_mentions_local(then_block, target, include_bindings)
                    || else_block
                        .as_ref()
                        .is_some_and(|block| block_mentions_local(block, target, include_bindings))
            }
            StatementKind::While { condition, body } => {
                expression_mentions_local(condition, target, include_bindings)
                    || block_mentions_local(body, target, include_bindings)
            }
            StatementKind::For { iterable, body, .. } => {
                expression_mentions_local(iterable, target, include_bindings)
                    || block_mentions_local(body, target, include_bindings)
            }
            StatementKind::Match { scrutinee, arms } => {
                expression_mentions_local(scrutinee, target, include_bindings)
                    || arms
                        .iter()
                        .any(|arm| block_mentions_local(&arm.body, target, include_bindings))
            }
            StatementKind::Assert { condition, message } => {
                expression_mentions_local(condition, target, include_bindings)
                    || message.as_ref().is_some_and(|message| {
                        expression_mentions_local(message, target, include_bindings)
                    })
            }
            StatementKind::Trace(local) => local.index() == target,
            StatementKind::Breakpoint {
                condition,
                bindings,
            } => {
                condition.as_ref().is_some_and(|condition| {
                    expression_mentions_local(condition, target, include_bindings)
                }) || bindings.iter().any(|binding| binding.index() == target)
            }
            StatementKind::Scope(body) => block_mentions_local(body, target, include_bindings),
            StatementKind::ReflectedTypeDispatch { type_info, arms } => {
                expression_mentions_local(type_info, target, include_bindings)
                    || arms
                        .iter()
                        .any(|arm| block_mentions_local(&arm.body, target, include_bindings))
            }
            StatementKind::Break | StatementKind::Continue => false,
        })
}

fn expression_mentions_local(expression: &Expression, target: u32, include_bindings: bool) -> bool {
    if include_bindings {
        if let ExpressionKind::Call { ownership, .. }
        | ExpressionKind::IndirectCall { ownership, .. }
        | ExpressionKind::Intrinsic { ownership, .. }
        | ExpressionKind::ResourceInvoke { ownership, .. } = &expression.kind
        {
            let mut mentioned = false;
            ownership.metadata_local_ids(|local| mentioned |= local.index() == target);
            if mentioned {
                return true;
            }
        }
    }
    match &expression.kind {
        ExpressionKind::Local(local) => local.index() == target,
        ExpressionKind::Binary { left, right, .. } => {
            expression_mentions_local(left, target, include_bindings)
                || expression_mentions_local(right, target, include_bindings)
        }
        ExpressionKind::Unary { value, .. }
        | ExpressionKind::ResultOk(value)
        | ExpressionKind::ResultFail(value)
        | ExpressionKind::OptionalSome(value)
        | ExpressionKind::Comptime { value, .. }
        | ExpressionKind::Declassify(value)
        | ExpressionKind::Coarsen(value)
        | ExpressionKind::RefinementValidated(value)
        | ExpressionKind::DisplayResult(value)
        | ExpressionKind::EquatableResult(value)
        | ExpressionKind::RuntimeFailureMessage(value)
        | ExpressionKind::FunctionAdapter { value, .. }
        | ExpressionKind::InterfaceCoerce { value, .. }
        | ExpressionKind::InterfaceType(value)
        | ExpressionKind::StateIs { value, .. }
        | ExpressionKind::Run(value)
        | ExpressionKind::Join(value)
        | ExpressionKind::Cancel(value)
        | ExpressionKind::Field { base: value, .. }
        | ExpressionKind::View(value)
        | ExpressionKind::Clone(value) => {
            expression_mentions_local(value, target, include_bindings)
        }
        ExpressionKind::Call { args, .. }
        | ExpressionKind::ResourceInvoke { args, .. }
        | ExpressionKind::Intrinsic { args, .. }
        | ExpressionKind::ActorSpawn { args, .. } => args
            .iter()
            .any(|arg| expression_mentions_local(arg, target, include_bindings)),
        ExpressionKind::IndirectCall { callee, args, .. } => {
            expression_mentions_local(callee, target, include_bindings)
                || args
                    .iter()
                    .any(|arg| expression_mentions_local(arg, target, include_bindings))
        }
        ExpressionKind::StructConstruct { fields, .. }
        | ExpressionKind::BitfieldConstruct { fields, .. } => fields
            .iter()
            .any(|field| expression_mentions_local(field, target, include_bindings)),
        ExpressionKind::MachineConstruct { payloads, .. }
        | ExpressionKind::EnumConstruct { payloads, .. } => payloads
            .iter()
            .any(|payload| expression_mentions_local(payload, target, include_bindings)),
        ExpressionKind::MachineTransition {
            source, payloads, ..
        } => {
            expression_mentions_local(source, target, include_bindings)
                || payloads
                    .iter()
                    .any(|payload| expression_mentions_local(payload, target, include_bindings))
        }
        ExpressionKind::ListConstruct { elements } => elements
            .iter()
            .any(|element| expression_mentions_local(element, target, include_bindings)),
        ExpressionKind::MapConstruct { entries } => entries.iter().any(|entry| {
            expression_mentions_local(&entry.key, target, include_bindings)
                || expression_mentions_local(&entry.value, target, include_bindings)
        }),
        ExpressionKind::Handle {
            target: handled,
            failure,
            ..
        } => {
            expression_mentions_local(handled, target, include_bindings)
                || block_mentions_local(failure, target, include_bindings)
        }
        ExpressionKind::StringInterpolation(parts) => parts.iter().any(|part| match part {
            StringSegment::Text(_) => false,
            StringSegment::Value(value) => {
                expression_mentions_local(value, target, include_bindings)
            }
        }),
        ExpressionKind::InlineFunction { body, .. } => {
            block_mentions_local(body, target, include_bindings)
        }
        ExpressionKind::ClosureRef { captures, .. } => {
            captures.iter().any(|local| local.index() == target)
        }
        ExpressionKind::ActorMessage { actor, args, .. } => {
            expression_mentions_local(actor, target, include_bindings)
                || args
                    .iter()
                    .any(|arg| expression_mentions_local(arg, target, include_bindings))
        }
        ExpressionKind::Int(_)
        | ExpressionKind::Constant { .. }
        | ExpressionKind::Float(_)
        | ExpressionKind::String(_)
        | ExpressionKind::Bool(_)
        | ExpressionKind::Nothing
        | ExpressionKind::PropertyCaseContext(_)
        | ExpressionKind::RuntimeFailure(_)
        | ExpressionKind::ResourceHookValue { .. }
        | ExpressionKind::FunctionRef(_)
        | ExpressionKind::OptionalNone => false,
    }
}
