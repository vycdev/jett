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
        errors: Vec::new(),
    };
    for function in &program.functions {
        validator.function(function);
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
        let function_name = &function.identity.declaration.name;
        for (index, type_argument) in function.identity.type_arguments.iter().enumerate() {
            self.type_id(
                *type_argument,
                function.span,
                format!("function `{function_name}` type argument {index}"),
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
    }

    fn block(&mut self, block: &Block, function_name: &str) {
        for statement in &block.statements {
            self.statement(statement, function_name);
        }
    }

    fn statement(&mut self, statement: &Statement, function_name: &str) {
        match &statement.kind {
            StatementKind::Let { value, .. }
            | StatementKind::HandleDefault(value)
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
            ExpressionKind::Binary { left, right, .. } => {
                self.expression(left, function_name);
                self.expression(right, function_name);
            }
            ExpressionKind::Unary { value, .. }
            | ExpressionKind::ResultOk(value)
            | ExpressionKind::ResultFail(value)
            | ExpressionKind::OptionalSome(value)
            | ExpressionKind::Comptime(value)
            | ExpressionKind::Declassify(value)
            | ExpressionKind::Coarsen(value)
            | ExpressionKind::RefinementValidated(value)
            | ExpressionKind::Run(value)
            | ExpressionKind::Join(value)
            | ExpressionKind::Cancel(value)
            | ExpressionKind::View(value)
            | ExpressionKind::Clone(value) => self.expression(value, function_name),
            ExpressionKind::Call { args, .. } => {
                for argument in args {
                    self.expression(argument, function_name);
                }
            }
            ExpressionKind::Intrinsic {
                type_arguments,
                args,
                ..
            } => {
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
            ExpressionKind::InlineFunction { body, .. } => self.block(body, function_name),
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
            ExpressionKind::Int(_)
            | ExpressionKind::Float(_)
            | ExpressionKind::String(_)
            | ExpressionKind::Bool(_)
            | ExpressionKind::Nothing
            | ExpressionKind::Local(_)
            | ExpressionKind::FunctionRef(_)
            | ExpressionKind::ClosureRef { .. }
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
            functions: vec![Function {
                id: FunctionId(0),
                identity: FunctionIdentity {
                    declaration: DeclarationId {
                        origin: SourceOrigin::Project,
                        namespace: "test".to_string(),
                        name: "main".to_string(),
                        kind: DeclarationKind::Function,
                    },
                    type_arguments: Vec::new(),
                    specialization: Default::default(),
                },
                source_definition: None,
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
