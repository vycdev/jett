use std::collections::HashSet;

use jett_common::Span;
use jett_types::{
    ActorId, BitfieldId, EnumId, FunctionSig, InterfaceId, MachineId, StructId, Type, TypeId,
    TypeInterner,
};

use crate::{
    Block, Expression, ExpressionKind, Function, HandleKind, Program, Statement, StatementKind,
    StringSegment, ValidationError,
};

/// Reject recovery-only types before a completed HIR program can reach MIR.
///
/// Only types reachable from HIR are inspected. `TypeInterner::ERROR` is
/// always registered so checking every interned type would reject every valid
/// compilation. Named definitions are followed transitively because their
/// field and signature types are part of the backend-visible representation.
pub fn validate_backend_types(
    program: &Program,
    interner: &TypeInterner,
) -> Result<(), Vec<ValidationError>> {
    let mut validator = BackendTypeValidator {
        interner,
        visited_types: HashSet::new(),
        visited_definitions: HashSet::new(),
        local_views: Vec::new(),
        local_types: Vec::new(),
        local_view_initializers: HashSet::new(),
        borrowed_parameters: HashSet::new(),
        errors: Vec::new(),
    };
    for function in &program.functions {
        validator.function(function);
    }
    if let Err(errors) = crate::validate_local_views(program, interner) {
        validator.errors.extend(errors);
    }
    for (&owner, &method) in &program.equality_methods {
        let target = program
            .functions
            .get(method.index() as usize)
            .filter(|function| function.id == method);
        if let Some(target) = target {
            if owner.index() as usize >= interner.len()
                || !matches!(interner.resolve(owner), Type::Struct(_))
                || target.capture_count != 0
                || target.params.len() != 2
                || target.return_type != TypeInterner::BOOL
                || target.params.iter().any(|parameter| {
                    parameter.ty != owner || parameter.mode != crate::ParamMode::View
                })
            {
                validator.error(
                    target.span,
                    "equality target must have two exact struct views and return bool",
                );
            }
        } else {
            validator.error(
                Span::new(jett_common::FileId::new(0), 0, 0),
                "equality target is absent from HIR",
            );
        }
    }
    if validator.errors.is_empty() {
        Ok(())
    } else {
        Err(validator.errors)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum DefinitionKey {
    Struct(StructId),
    Bitfield(BitfieldId),
    Enum(EnumId),
    Interface(InterfaceId),
    Actor(ActorId),
    Machine(MachineId),
}

struct BackendTypeValidator<'a> {
    interner: &'a TypeInterner,
    visited_types: HashSet<TypeId>,
    visited_definitions: HashSet<DefinitionKey>,
    local_views: Vec<Option<crate::LocalId>>,
    local_types: Vec<TypeId>,
    local_view_initializers: HashSet<crate::LocalId>,
    borrowed_parameters: HashSet<crate::LocalId>,
    errors: Vec<ValidationError>,
}

impl BackendTypeValidator<'_> {
    fn error(&mut self, span: Span, message: impl Into<String>) {
        self.errors.push(ValidationError {
            span,
            message: message.into(),
        });
    }

    fn function(&mut self, function: &Function) {
        self.local_views = function
            .locals
            .iter()
            .map(|local| local.view_source)
            .collect();
        self.local_types = function.locals.iter().map(|local| local.ty).collect();
        self.local_view_initializers.clear();
        self.borrowed_parameters = function
            .params
            .iter()
            .filter(|param| param.mode == crate::ParamMode::View)
            .map(|param| param.local)
            .collect();
        let function_name = &function.identity.declaration.name;
        for (index, type_argument) in function.identity.type_arguments.iter().enumerate() {
            self.type_id(
                *type_argument,
                function.span,
                format!("function `{function_name}` type argument {index}"),
            );
        }
        for binding in &function.identity.scoped_type_bindings {
            self.type_id(
                binding.ty,
                function.span,
                format!("function `{function_name}` scoped type `{}`", binding.name),
            );
        }
        for parameter in &function.params {
            self.type_id(
                parameter.ty,
                parameter.span,
                format!("function `{function_name}` parameter `{}`", parameter.name),
            );
        }
        self.type_id(
            function.return_type,
            function.span,
            format!("function `{function_name}` return type"),
        );
        for local in &function.locals {
            self.type_id(
                local.ty,
                local.span,
                format!("function `{function_name}` local `{}`", local.name),
            );
        }
        self.block(&function.body, function_name);
        for local in &function.locals {
            if local.view_source.is_some() && !self.local_view_initializers.contains(&local.id) {
                self.error(local.span, "borrowed local has no validated initializer");
            }
        }
    }

    fn block(&mut self, block: &Block, function_name: &str) {
        for statement in &block.statements {
            self.statement(statement, function_name);
        }
    }

    fn statement(&mut self, statement: &Statement, function_name: &str) {
        match &statement.kind {
            StatementKind::Let { local, value } => {
                if let Some(Some(source)) = self.local_views.get(local.index() as usize)
                    && let Some(&ty) = self.local_types.get(local.index() as usize)
                {
                    let source = *source;
                    let validation = self
                        .local_types
                        .get(source.index() as usize)
                        .ok_or("borrowed local origin is outside its function")
                        .and_then(|&source_type| {
                            crate::local_views::validate_local_view_initializer(
                                value,
                                source,
                                source_type,
                                ty,
                                self.interner,
                            )
                        });
                    match validation {
                        Ok(()) => {
                            self.local_view_initializers.insert(*local);
                        }
                        Err(message) => self.error(value.span, message),
                    }
                }
                self.expression(value, function_name);
            }
            StatementKind::HandleDefault(value)
            | StatementKind::Expression(value)
            | StatementKind::Respond(value) => self.expression(value, function_name),
            StatementKind::Assign { target, value } => {
                self.expression(target, function_name);
                self.expression(value, function_name);
            }
            StatementKind::Return(value) => {
                if let Some(value) = value {
                    self.expression(value, function_name);
                }
            }
            StatementKind::If {
                condition,
                then_block,
                else_block,
            } => {
                self.expression(condition, function_name);
                self.block(then_block, function_name);
                if let Some(else_block) = else_block {
                    self.block(else_block, function_name);
                }
            }
            StatementKind::While { condition, body } => {
                self.expression(condition, function_name);
                self.block(body, function_name);
            }
            StatementKind::For { iterable, body, .. } => {
                self.expression(iterable, function_name);
                self.block(body, function_name);
            }
            StatementKind::Match { scrutinee, arms } => {
                self.expression(scrutinee, function_name);
                for arm in arms {
                    self.block(&arm.body, function_name);
                }
            }
            StatementKind::Assert { condition, message } => {
                self.expression(condition, function_name);
                if let Some(message) = message {
                    self.expression(message, function_name);
                }
            }
            StatementKind::Breakpoint { condition, .. } => {
                if let Some(condition) = condition {
                    self.expression(condition, function_name);
                }
            }
            StatementKind::Scope(block) => self.block(block, function_name),
            StatementKind::ReflectedTypeDispatch { type_info, arms } => {
                self.expression(type_info, function_name);
                for arm in arms {
                    self.type_id(
                        arm.bound_type,
                        statement.span,
                        format!(
                            "function `{function_name}` reflected type arm {}",
                            arm.iteration_index
                        ),
                    );
                    self.block(&arm.body, function_name);
                }
            }
            StatementKind::Break | StatementKind::Continue | StatementKind::Trace(_) => {}
        }
    }

    fn expression(&mut self, expression: &Expression, function_name: &str) {
        self.type_id(
            expression.ty,
            expression.span,
            format!("function `{function_name}` expression result"),
        );
        match &expression.kind {
            ExpressionKind::Comptime {
                value, bindings, ..
            } => {
                for binding in bindings {
                    self.type_id(
                        binding.ty,
                        expression.span,
                        format!("comptime binding `{}`", binding.name),
                    );
                }
                self.expression(value, function_name);
            }
            ExpressionKind::Binary { left, right, .. } => {
                self.expression(left, function_name);
                self.expression(right, function_name);
            }
            ExpressionKind::RuntimeFailureMessage(value) => {
                self.expression(value, function_name);
                if value.ty != TypeInterner::STRING || expression.ty != TypeInterner::NOTHING {
                    self.error(
                        expression.span,
                        "dynamic runtime failure requires exact string input and nothing output",
                    );
                }
            }
            ExpressionKind::DisplayResult(value) => {
                self.expression(value, function_name);
                if expression.ty != TypeInterner::STRING || value.ty != TypeInterner::STRING {
                    self.error(
                        expression.span,
                        "display result boundary requires exact string input and output",
                    );
                }
            }
            ExpressionKind::EquatableResult(value) => {
                self.expression(value, function_name);
                if expression.ty != value.ty || !qualified_bool(self.interner, value.ty) {
                    self.error(
                        expression.span,
                        "equality result boundary requires the same bool type with only outer secret qualification",
                    );
                }
            }
            ExpressionKind::Unary { value, .. }
            | ExpressionKind::ResultOk(value)
            | ExpressionKind::ResultFail(value)
            | ExpressionKind::OptionalSome(value)
            | ExpressionKind::Declassify(value)
            | ExpressionKind::Coarsen(value)
            | ExpressionKind::RefinementValidated(value)
            | ExpressionKind::FunctionAdapter { value, .. }
            | ExpressionKind::InterfaceType(value)
            | ExpressionKind::Run(value)
            | ExpressionKind::Join(value)
            | ExpressionKind::Cancel(value)
            | ExpressionKind::View(value)
            | ExpressionKind::Clone(value) => self.expression(value, function_name),
            ExpressionKind::InterfaceCoerce { value, adapters } => {
                self.expression(value, function_name);
                for entry in adapters {
                    self.type_id(
                        entry.source,
                        expression.span,
                        "container callback source".into(),
                    );
                    self.type_id(
                        entry.target,
                        expression.span,
                        "container callback target".into(),
                    );
                }
            }
            ExpressionKind::Call { args, .. } => {
                for argument in args {
                    self.expression(argument, function_name);
                }
            }
            ExpressionKind::Intrinsic {
                intrinsic,
                type_arguments,
                field_validation,
                args,
                ..
            } => {
                if crate::is_reflected_field_intrinsic(*intrinsic) {
                    match (type_arguments.as_slice(), field_validation) {
                        (
                            [owner, requested],
                            Some(crate::ReflectedFieldValidation::Validate(plans)),
                        ) if *requested == expression.ty => {
                            if let Err(message) = crate::validate_reflected_field_plans(
                                self.interner,
                                *intrinsic,
                                *owner,
                                *requested,
                                plans,
                            ) {
                                self.error(expression.span, message);
                            }
                        }
                        _ => self.error(
                            expression.span,
                            "reflected read has no complete checked validation plan",
                        ),
                    }
                } else if field_validation.is_some() {
                    self.error(
                        expression.span,
                        "non-selector intrinsic carries reflected validation plans",
                    );
                }
                for (index, type_argument) in type_arguments.iter().enumerate() {
                    self.type_id(
                        *type_argument,
                        expression.span,
                        format!("function `{function_name}` intrinsic type argument {index}"),
                    );
                }
                for argument in args {
                    self.expression(argument, function_name);
                }
            }
            ExpressionKind::IndirectCall { callee, args, .. } => {
                self.expression(callee, function_name);
                for argument in args {
                    self.expression(argument, function_name);
                }
            }
            ExpressionKind::StructConstruct {
                struct_type,
                fields,
                ..
            } => {
                self.type_id(
                    *struct_type,
                    expression.span,
                    format!("function `{function_name}` struct construction target"),
                );
                for field in fields {
                    self.expression(field, function_name);
                }
            }
            ExpressionKind::BitfieldConstruct {
                bitfield_type,
                fields,
                ..
            } => {
                self.type_id(
                    *bitfield_type,
                    expression.span,
                    format!("function `{function_name}` bitfield construction target"),
                );
                for field in fields {
                    self.expression(field, function_name);
                }
            }
            ExpressionKind::MachineConstruct {
                state_type,
                payloads,
                ..
            } => {
                self.type_id(
                    *state_type,
                    expression.span,
                    format!("function `{function_name}` machine construction state"),
                );
                for payload in payloads {
                    self.expression(payload, function_name);
                }
            }
            ExpressionKind::MachineTransition {
                source,
                state_type,
                payloads,
                ..
            } => {
                self.expression(source, function_name);
                self.type_id(
                    *state_type,
                    expression.span,
                    format!("function `{function_name}` machine transition state"),
                );
                for payload in payloads {
                    self.expression(payload, function_name);
                }
            }
            ExpressionKind::ListConstruct { elements } => {
                for element in elements {
                    self.expression(element, function_name);
                }
            }
            ExpressionKind::MapConstruct { entries } => {
                for entry in entries {
                    self.expression(&entry.key, function_name);
                    self.expression(&entry.value, function_name);
                }
            }
            ExpressionKind::Handle {
                target,
                kind,
                failure,
                ..
            } => {
                self.expression(target, function_name);
                match kind {
                    HandleKind::Refinement {
                        refined_type,
                        predicates,
                    } => {
                        self.type_id(
                            *refined_type,
                            expression.span,
                            format!("function `{function_name}` refinement handle target"),
                        );
                        for predicate in predicates {
                            self.type_id(
                                predicate.refined_type,
                                expression.span,
                                format!("function `{function_name}` refinement predicate type"),
                            );
                        }
                    }
                    HandleKind::Result | HandleKind::Optional => {}
                }
                self.block(failure, function_name);
            }
            ExpressionKind::EnumConstruct {
                enum_type,
                payloads,
                ..
            } => {
                self.type_id(
                    *enum_type,
                    expression.span,
                    format!("function `{function_name}` enum construction target"),
                );
                for payload in payloads {
                    self.expression(payload, function_name);
                }
            }
            ExpressionKind::StringInterpolation(segments) => {
                for segment in segments {
                    if let StringSegment::Value(value) = segment {
                        self.expression(value, function_name);
                    }
                }
            }
            ExpressionKind::StateIs { value, .. } => self.expression(value, function_name),
            ExpressionKind::InlineFunction {
                scoped_type_bindings,
                body,
                ..
            } => {
                for binding in scoped_type_bindings {
                    self.type_id(
                        binding.ty,
                        expression.span,
                        format!(
                            "function `{function_name}` inline scoped type `{}`",
                            binding.name
                        ),
                    );
                }
                self.block(body, function_name);
            }
            ExpressionKind::ActorSpawn { args, .. } => {
                for argument in args {
                    self.expression(argument, function_name);
                }
            }
            ExpressionKind::ActorMessage { actor, args, .. } => {
                self.expression(actor, function_name);
                for argument in args {
                    self.expression(argument, function_name);
                }
            }
            ExpressionKind::Field {
                base, owner_type, ..
            } => {
                self.expression(base, function_name);
                self.type_id(
                    *owner_type,
                    expression.span,
                    format!("function `{function_name}` field owner"),
                );
            }
            ExpressionKind::ClosureRef { captures, .. } => {
                if captures.iter().any(|id| {
                    (self
                        .local_views
                        .get(id.index() as usize)
                        .is_some_and(Option::is_some)
                        || self.borrowed_parameters.contains(id))
                        && self.local_types.get(id.index() as usize).is_some_and(|ty| {
                            (ty.index() as usize) < self.interner.len()
                                && !jett_typecheck::ownership::is_implicitly_copyable(
                                    self.interner,
                                    *ty,
                                )
                        })
                }) {
                    self.error(
                        expression.span,
                        "native callbacks cannot capture a borrowed local alias",
                    );
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
            | ExpressionKind::FunctionRef(_)
            | ExpressionKind::OptionalNone => {}
        }
    }

    fn type_id(&mut self, type_id: TypeId, span: Span, context: String) {
        if type_id.index() as usize >= self.interner.len() {
            self.error(
                span,
                format!(
                    "backend HIR references unknown type ID {} in {context}",
                    type_id.index()
                ),
            );
            return;
        }
        if !self.visited_types.insert(type_id) {
            return;
        }

        // An unused nominal type parameter remains part of checked identity.
        // It must be closed even when no payload field refers to its type.
        let nominal_arguments = self.interner.nominal_type_arguments(type_id).to_vec();
        for (index, argument) in nominal_arguments.into_iter().enumerate() {
            self.type_id(
                argument,
                span,
                format!("{context} nominal type argument {index}"),
            );
        }

        match self.interner.resolve(type_id).clone() {
            Type::List(element) | Type::Set(element) | Type::Optional(element) => {
                self.type_id(element, span, format!("{context} element type"));
            }
            Type::Secret(inner) => {
                self.type_id(inner, span, format!("{context} secret value type"));
            }
            Type::Map(key, value) => {
                self.type_id(key, span, format!("{context} map key type"));
                self.type_id(value, span, format!("{context} map value type"));
            }
            Type::Result(value, error) => {
                self.type_id(value, span, format!("{context} result value type"));
                self.type_id(error, span, format!("{context} result error type"));
            }
            Type::Function {
                params,
                view_params,
                return_type,
            } => {
                if params.len() != view_params.len() {
                    self.error(
                        span,
                        format!("{context} function view parameter count mismatch"),
                    );
                }
                for (index, parameter) in params.into_iter().enumerate() {
                    self.type_id(
                        parameter,
                        span,
                        format!("{context} function parameter {index}"),
                    );
                }
                self.type_id(return_type, span, format!("{context} function return type"));
            }
            Type::Refinement { name, base } => {
                self.type_id(base, span, format!("{context} refinement `{name}` base"));
            }
            Type::Struct(id) => self.struct_definition(id, span, &context),
            Type::Bitfield(id) => self.bitfield_definition(id, span, &context),
            Type::Enum(id) => self.enum_definition(id, span, &context),
            Type::Interface(id) => self.interface_definition(id, span, &context),
            Type::Actor(id) => self.actor_definition(id, span, &context),
            Type::Machine(id) | Type::MachineState { machine: id, .. } => {
                self.machine_definition(id, span, &context);
            }
            Type::Error => self.error(
                span,
                format!("backend HIR contains unresolved `<error>` type in {context}"),
            ),
            Type::Int8
            | Type::Int16
            | Type::Int32
            | Type::Int64
            | Type::Uint8
            | Type::Uint16
            | Type::Uint32
            | Type::Uint64
            | Type::Float32
            | Type::Float64
            | Type::String
            | Type::Bool
            | Type::Bytes
            | Type::Nothing
            | Type::TypeConstruction
            | Type::Never
            | Type::Capability(_)
            | Type::Resource(_) => {}
        }
    }

    fn struct_definition(&mut self, id: StructId, span: Span, context: &str) {
        if !self.visited_definitions.insert(DefinitionKey::Struct(id)) {
            return;
        }
        let definition = self.interner.resolve_struct(id).clone();
        for (field_name, field_type) in definition.fields {
            self.type_id(
                field_type,
                span,
                format!(
                    "{context} struct `{}` field `{field_name}`",
                    definition.name
                ),
            );
        }
        for signature in definition.methods {
            self.function_signature(
                signature,
                span,
                format!("{context} struct `{}`", definition.name),
            );
        }
    }

    fn bitfield_definition(&mut self, id: BitfieldId, span: Span, context: &str) {
        if !self.visited_definitions.insert(DefinitionKey::Bitfield(id)) {
            return;
        }
        let definition = self.interner.resolve_bitfield(id).clone();
        for field in definition.fields {
            self.type_id(
                field.ty,
                span,
                format!(
                    "{context} bitfield `{}` field `{}`",
                    definition.name, field.name
                ),
            );
        }
    }

    fn enum_definition(&mut self, id: EnumId, span: Span, context: &str) {
        if !self.visited_definitions.insert(DefinitionKey::Enum(id)) {
            return;
        }
        let definition = self.interner.resolve_enum(id).clone();
        for variant in definition.variants {
            for (field_name, field_type) in variant.fields {
                self.type_id(
                    field_type,
                    span,
                    format!(
                        "{context} enum `{}` variant `{}` field `{field_name}`",
                        definition.name, variant.name
                    ),
                );
            }
        }
    }

    fn interface_definition(&mut self, id: InterfaceId, span: Span, context: &str) {
        if !self
            .visited_definitions
            .insert(DefinitionKey::Interface(id))
        {
            return;
        }
        let definition = self.interner.resolve_interface(id).clone();
        for signature in definition.methods {
            self.function_signature(
                signature,
                span,
                format!("{context} interface `{}`", definition.name),
            );
        }
    }

    fn actor_definition(&mut self, id: ActorId, span: Span, context: &str) {
        if !self.visited_definitions.insert(DefinitionKey::Actor(id)) {
            return;
        }
        let definition = self.interner.resolve_actor(id).clone();
        for (name, type_id) in definition.capability_params {
            self.type_id(
                type_id,
                span,
                format!("{context} actor `{}` capability `{name}`", definition.name),
            );
        }
        for (name, type_id) in definition.state_fields {
            self.type_id(
                type_id,
                span,
                format!("{context} actor `{}` state `{name}`", definition.name),
            );
        }
        for message in definition.messages {
            for (name, type_id) in message.params {
                self.type_id(
                    type_id,
                    span,
                    format!(
                        "{context} actor `{}` message `{}` parameter `{name}`",
                        definition.name, message.name
                    ),
                );
            }
            self.type_id(
                message.responds,
                span,
                format!(
                    "{context} actor `{}` message `{}` response",
                    definition.name, message.name
                ),
            );
        }
    }

    fn machine_definition(&mut self, id: MachineId, span: Span, context: &str) {
        if !self.visited_definitions.insert(DefinitionKey::Machine(id)) {
            return;
        }
        let definition = self.interner.resolve_machine(id).clone();
        for state in definition.states {
            for (field_name, field_type) in state.fields {
                self.type_id(
                    field_type,
                    span,
                    format!(
                        "{context} machine `{}` state `{}` field `{field_name}`",
                        definition.name, state.name
                    ),
                );
            }
        }
    }

    fn function_signature(&mut self, signature: FunctionSig, span: Span, context: String) {
        for (parameter_name, parameter_type, _) in signature.params {
            self.type_id(
                parameter_type,
                span,
                format!(
                    "{context} method `{}` parameter `{parameter_name}`",
                    signature.name
                ),
            );
        }
        self.type_id(
            signature.return_type,
            span,
            format!("{context} method `{}` return type", signature.name),
        );
    }
}

fn qualified_bool(types: &TypeInterner, mut ty: TypeId) -> bool {
    for _ in 0..types.len() {
        if ty == TypeInterner::BOOL {
            return true;
        }
        if ty.index() as usize >= types.len() {
            return false;
        }
        match types.resolve(ty) {
            Type::Secret(inner) => ty = *inner,
            _ => return false,
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use jett_common::{FileId, SourceOrigin};
    use jett_types::{CapabilityKind, StructDef};

    use super::*;
    use crate::{
        Block, DeclarationId, DeclarationKind, Expression, ExpressionKind, Function, FunctionId,
        FunctionIdentity, IntrinsicId, Program, Statement,
    };

    fn test_span() -> Span {
        Span::new(FileId::new(0), 0, 1)
    }

    fn program_with(return_type: TypeId, statements: Vec<Statement>) -> Program {
        let span = test_span();
        Program {
            equality_methods: Default::default(),
            functions: vec![Function {
                id: FunctionId(0),
                identity: FunctionIdentity {
                    declaration: DeclarationId {
                        origin: SourceOrigin::Project,
                        namespace: "test".to_string(),
                        name: "main".to_string(),
                        kind: DeclarationKind::Function,
                    },
                    scoped_type_bindings: Vec::new(),
                    type_arguments: Vec::new(),
                    specialization: Default::default(),
                },
                source_definition: None,
                debug_kind: crate::FunctionDebugKind::named("test", "main"),
                params: Vec::new(),
                capture_count: 0,
                return_type,
                locals: Vec::new(),
                body: Block { statements, span },
                span,
            }],
        }
    }

    #[test]
    fn equatable_result_preserves_exact_bool_qualification_without_refinement_peeling() {
        let mut interner = TypeInterner::new();
        let secret = interner.intern(Type::Secret(TypeInterner::BOOL));
        let nested_secret = interner.intern(Type::Secret(secret));
        let refined = interner.intern(Type::Refinement {
            name: "test.Truth".into(),
            base: TypeInterner::BOOL,
        });
        let secret_refined = interner.intern(Type::Secret(refined));
        for (input, output, valid) in [
            (TypeInterner::BOOL, TypeInterner::BOOL, true),
            (secret, secret, true),
            (nested_secret, nested_secret, true),
            (TypeInterner::BOOL, secret, false),
            (secret, TypeInterner::BOOL, false),
            (secret, nested_secret, false),
            (refined, refined, false),
            (secret_refined, secret_refined, false),
            (TypeInterner::INT64, TypeInterner::INT64, false),
            (TypeInterner::STRING, TypeInterner::BOOL, false),
        ] {
            let expression = Expression {
                kind: ExpressionKind::EquatableResult(Box::new(Expression {
                    kind: ExpressionKind::Bool(true),
                    ty: input,
                    span: test_span(),
                })),
                ty: output,
                span: test_span(),
            };
            let program = program_with(
                output,
                vec![Statement {
                    kind: StatementKind::Return(Some(expression)),
                    span: test_span(),
                }],
            );
            let result = validate_backend_types(&program, &interner);
            if valid {
                result.expect("unchanged qualified bool boundary");
            } else {
                let errors = result.expect_err("invalid equality boundary type");
                assert!(errors.iter().any(|error| error.message ==
                    "equality result boundary requires the same bool type with only outer secret qualification"));
            }
        }
    }

    #[test]
    fn native_return_dynamic_failure_requires_exact_string_and_nothing() {
        let mut interner = TypeInterner::new();
        let secret_string = interner.intern(Type::Secret(TypeInterner::STRING));
        for (input, output, valid) in [
            (TypeInterner::STRING, TypeInterner::NOTHING, true),
            (TypeInterner::INT64, TypeInterner::NOTHING, false),
            (secret_string, TypeInterner::NOTHING, false),
            (TypeInterner::STRING, TypeInterner::STRING, false),
        ] {
            let boundary = Expression {
                kind: ExpressionKind::RuntimeFailureMessage(Box::new(Expression {
                    kind: if input == TypeInterner::INT64 {
                        ExpressionKind::Int(7)
                    } else {
                        ExpressionKind::String("error".into())
                    },
                    ty: input,
                    span: test_span(),
                })),
                ty: output,
                span: test_span(),
            };
            let program = program_with(
                TypeInterner::NOTHING,
                vec![Statement {
                    kind: StatementKind::Expression(boundary),
                    span: test_span(),
                }],
            );
            let result = validate_backend_types(&program, &interner);
            if valid {
                result.expect("exact borrowed error boundary");
            } else {
                assert!(result.expect_err("malformed dynamic error").iter().any(|error|
                    error.message == "dynamic runtime failure requires exact string input and nothing output"));
            }
        }
    }

    #[test]
    fn display_result_requires_exact_string_input_and_output() {
        let mut interner = TypeInterner::new();
        let secret = interner.intern(Type::Secret(TypeInterner::STRING));
        for (input, output, valid) in [
            (TypeInterner::STRING, TypeInterner::STRING, true),
            (TypeInterner::INT64, TypeInterner::STRING, false),
            (TypeInterner::STRING, TypeInterner::INT64, false),
            (secret, secret, false),
        ] {
            let expression = Expression {
                kind: ExpressionKind::DisplayResult(Box::new(Expression {
                    kind: if input == TypeInterner::INT64 {
                        ExpressionKind::Int(1)
                    } else {
                        ExpressionKind::String("shown".into())
                    },
                    ty: input,
                    span: test_span(),
                })),
                ty: output,
                span: test_span(),
            };
            let program = program_with(
                output,
                vec![Statement {
                    kind: StatementKind::Return(Some(expression)),
                    span: test_span(),
                }],
            );
            let result = validate_backend_types(&program, &interner);
            if valid {
                result.expect("exact string boundary");
            } else {
                let errors = result.expect_err("invalid display boundary types");
                assert!(errors.iter().any(|error| error.message
                    == "display result boundary requires exact string input and output"));
            }
        }
    }

    #[test]
    fn accepts_never_capabilities_and_recursive_nominal_types() {
        let mut interner = TypeInterner::new();
        let recursive_id = interner.add_struct(StructDef {
            name: "Recursive".to_string(),
            fields: Vec::new(),
            methods: Vec::new(),
        });
        let recursive_type = interner.intern(Type::Struct(recursive_id));
        let optional_recursive = interner.intern(Type::Optional(recursive_type));
        let mut fields = vec![
            ("next".to_string(), optional_recursive),
            ("bottom".to_string(), TypeInterner::NEVER),
        ];
        fields.extend(
            CapabilityKind::ALL
                .into_iter()
                .map(|kind| (kind.name().to_string(), TypeInterner::capability(kind))),
        );
        interner.update_struct(
            recursive_id,
            StructDef {
                name: "Recursive".to_string(),
                fields,
                methods: Vec::new(),
            },
        );

        validate_backend_types(&program_with(recursive_type, Vec::new()), &interner)
            .expect("accepted backend types should validate");
    }

    #[test]
    fn rejects_error_types_in_comptime_binding_contexts() {
        let span = test_span();
        let statement = Statement {
            kind: StatementKind::Expression(Expression {
                kind: ExpressionKind::Comptime {
                    source_span: span,
                    value: Box::new(Expression {
                        kind: ExpressionKind::Int(7),
                        ty: TypeInterner::INT64,
                        span,
                    }),
                    bindings: vec![crate::ScopedTypeBinding {
                        name: "Bound".into(),
                        ty: TypeInterner::ERROR,
                        reflection: jett_types::ReflectionTypeInfo {
                            type_name: "int64".into(),
                            kind: "primitive".into(),
                            primitive_tag: Some("int64".into()),
                            has_secret: false,
                            args: Vec::new(),
                        },
                    }],
                },
                ty: TypeInterner::INT64,
                span,
            }),
            span,
        };
        let errors = validate_backend_types(
            &program_with(TypeInterner::NOTHING, vec![statement]),
            &TypeInterner::new(),
        )
        .expect_err("comptime type bindings are backend-visible");
        assert!(
            errors
                .iter()
                .any(|error| error.message.contains("comptime binding `Bound`")
                    && error.message.contains("unresolved `<error>` type"))
        );
    }

    #[test]
    fn backend_type_gate_visits_unused_nominal_argument_edges() {
        for invalid in [false, true] {
            let mut interner = TypeInterner::new();
            let marker = interner.add_struct(StructDef {
                name: "test.Marker".into(),
                fields: Vec::new(),
                methods: Vec::new(),
            });
            let marker = interner.intern(Type::Struct(marker));
            let argument = if invalid {
                interner.intern(Type::Optional(TypeInterner::ERROR))
            } else {
                TypeInterner::INT64
            };
            interner
                .register_nominal_type_arguments(marker, vec![argument])
                .unwrap();
            let result = validate_backend_types(&program_with(marker, Vec::new()), &interner);
            if invalid {
                let errors = result.expect_err("unused nominal arguments must be closed");
                assert!(
                    errors
                        .iter()
                        .any(|error| error.message.contains("nominal type argument 0")
                            && error.message.contains("unresolved `<error>` type"))
                );
            } else {
                result.expect("closed unused nominal argument");
            }
        }
    }

    #[test]
    fn rejects_a_direct_error_type() {
        let interner = TypeInterner::new();
        let errors =
            validate_backend_types(&program_with(TypeInterner::ERROR, Vec::new()), &interner)
                .expect_err("the error sentinel must not reach a backend");

        assert!(errors.iter().any(|error| {
            error.message.contains("unresolved `<error>` type")
                && error.message.contains("return type")
        }));
    }

    #[test]
    fn rejects_an_error_nested_in_a_recursive_definition() {
        let mut interner = TypeInterner::new();
        let recursive_id = interner.add_struct(StructDef {
            name: "Recursive".to_string(),
            fields: Vec::new(),
            methods: Vec::new(),
        });
        let recursive_type = interner.intern(Type::Struct(recursive_id));
        let optional_recursive = interner.intern(Type::Optional(recursive_type));
        let error_list = interner.intern(Type::List(TypeInterner::ERROR));
        let nested_error = interner.intern(Type::Map(TypeInterner::STRING, error_list));
        interner.update_struct(
            recursive_id,
            StructDef {
                name: "Recursive".to_string(),
                fields: vec![
                    ("next".to_string(), optional_recursive),
                    ("broken".to_string(), nested_error),
                ],
                methods: Vec::new(),
            },
        );

        let errors = validate_backend_types(&program_with(recursive_type, Vec::new()), &interner)
            .expect_err("a nested error sentinel must not reach a backend");

        assert!(errors.iter().any(|error| {
            error.message.contains("unresolved `<error>` type")
                && error.message.contains("field `broken`")
                && error.message.contains("map value type")
        }));
    }

    #[test]
    fn rejects_error_types_in_hir_intrinsic_operands() {
        let mut interner = TypeInterner::new();
        let nested_error = interner.intern(Type::Optional(TypeInterner::ERROR));
        let span = test_span();
        let statement = Statement {
            kind: StatementKind::Expression(Expression {
                kind: ExpressionKind::Intrinsic {
                    intrinsic: IntrinsicId::TypeName,
                    type_arguments: vec![nested_error],
                    reflection_arguments: Vec::new(),
                    refinement_predicates: Vec::new(),
                    field_validation: None,
                    args: Vec::new(),
                    evaluation_order: Vec::new(),
                },
                ty: TypeInterner::STRING,
                span,
            }),
            span,
        };

        let errors = validate_backend_types(
            &program_with(TypeInterner::NOTHING, vec![statement]),
            &interner,
        )
        .expect_err("intrinsic type operands are backend-visible");

        assert!(errors.iter().any(|error| {
            error.message.contains("unresolved `<error>` type")
                && error.message.contains("intrinsic type argument 0")
        }));
    }

    #[test]
    fn hir_lowering_runs_the_backend_type_gate() {
        let source = "function main() returns int64:\n    return 1\n";
        let file = FileId::new(0);
        let parsed = jett_parser::parse(source, file);
        let resolved = jett_resolve::resolve(&parsed.module);
        let mut checked = jett_typecheck::check(&parsed.module, &resolved);
        let mut replaced = 0;
        for type_id in checked.type_map.values_mut() {
            if *type_id == TypeInterner::INT64 {
                *type_id = TypeInterner::ERROR;
                replaced += 1;
            }
        }
        assert!(replaced > 0, "the literal should have a checked type");
        let origins = HashMap::from([(file, SourceOrigin::Project)]);

        let errors = crate::lower(&parsed.module, &resolved, &checked, &origins)
            .expect_err("HIR lowering must reject backend-ineligible types");

        assert!(
            errors
                .iter()
                .any(|error| error.message.contains("unresolved `<error>` type"))
        );
    }
}
