use std::collections::HashSet;

use jett_common::{SourceOrigin, Span};
use jett_hir::{BinaryOp, Expression, ExpressionKind, FunctionId, UnaryOp};
use jett_mir::{
    Function, Program, SequenceSource, Statement, StatementKind, Terminator, TerminatorKind,
};
use jett_types::{BitfieldFieldKind, Type, TypeId, TypeInterner};

use crate::reachability::reachable_function_ids;
use crate::{CodegenError, symbol_name};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ScalarKind {
    SignedInteger(u16),
    UnsignedInteger(u16),
    Float(u16),
    Bool,
    Nothing,
    String,
    Bytes,
    Sum,
    List,
    Set,
    Map,
    Struct,
    Enum,
    Bitfield,
    Machine,
    Construction,
    Function,
    Actor,
    Stdout,
    Clock,
    Random,
    Environment,
    Graphics,
}

pub(crate) fn known_unit_enum_variant(expression: &Expression) -> bool {
    match &expression.kind {
        ExpressionKind::EnumConstruct { payloads, .. } => payloads.is_empty(),
        ExpressionKind::View(inner) | ExpressionKind::Clone(inner) => {
            known_unit_enum_variant(inner)
        }
        _ => false,
    }
}

impl ScalarKind {
    pub(crate) fn is_integer(self) -> bool {
        matches!(self, Self::SignedInteger(_) | Self::UnsignedInteger(_))
    }

    fn is_numeric(self) -> bool {
        self.is_integer() || matches!(self, Self::Float(_))
    }
}

fn native_constructible_struct(types: &TypeInterner, ty: TypeId) -> bool {
    matches!(types.resolve(ty), Type::Struct(_))
}

fn native_constructible_bitfield(types: &TypeInterner, ty: TypeId) -> bool {
    let Type::Bitfield(id) = types.resolve(ty) else {
        return false;
    };
    types
        .resolve_bitfield(*id)
        .fields
        .iter()
        .all(|field| match field.kind {
            BitfieldFieldKind::Bits { width } if width <= 64 => match types.resolve(field.ty) {
                Type::Int64 | Type::Uint64 => true,
                Type::Enum(enum_id) => types
                    .resolve_enum(*enum_id)
                    .variants
                    .iter()
                    .all(|variant| variant.fields.is_empty()),
                _ => false,
            },
            BitfieldFieldKind::Payload => {
                !matches!(types.resolve(field.ty), Type::Refinement { .. })
            }
            _ => false,
        })
}

fn native_constructible_record(types: &TypeInterner, ty: TypeId) -> bool {
    native_constructible_struct(types, ty) || native_constructible_bitfield(types, ty)
}

fn native_builder_value_type_supported(types: &TypeInterner, owner: TypeId, value: TypeId) -> bool {
    let Type::Struct(id) = types.resolve(owner) else {
        return true;
    };
    let fields = &types.resolve_struct(*id).fields;
    if fields
        .iter()
        .any(|(_, field_ty)| matches!(types.resolve(*field_ty), Type::Refinement { .. }))
        && !fields.iter().any(|(_, field_ty)| *field_ty == value)
    {
        return false;
    }
    // The native builder receives already validated refinement values. A base
    // value can only be accepted once finish can invoke the refinement predicate.
    fields.iter().all(|(_, field_ty)| {
        let mut current = *field_ty;
        while let Type::Refinement { base, .. } = types.resolve(current) {
            if *base == value {
                return false;
            }
            current = *base;
        }
        true
    })
}

fn native_constructible_enum(types: &TypeInterner, ty: TypeId) -> bool {
    let Type::Enum(id) = types.resolve(ty) else {
        return false;
    };
    types.resolve_enum(*id).variants.iter().all(|variant| {
        variant
            .fields
            .iter()
            .all(|(_, field_ty)| !matches!(types.resolve(*field_ty), Type::Refinement { .. }))
    })
}

fn native_constructible_machine(types: &TypeInterner, ty: TypeId) -> bool {
    let id = match types.resolve(ty) {
        Type::Machine(id) | Type::MachineState { machine: id, .. } => *id,
        _ => return false,
    };
    types.resolve_machine(id).states.iter().all(|state| {
        state
            .fields
            .iter()
            .all(|(_, field_ty)| !matches!(types.resolve(*field_ty), Type::Refinement { .. }))
    })
}

fn native_constructible_builder_kind(types: &TypeInterner, ty: TypeId) -> Option<&'static str> {
    match types.resolve(ty) {
        Type::Struct(_) if native_constructible_struct(types, ty) => Some("struct"),
        Type::Bitfield(_) if native_constructible_bitfield(types, ty) => Some("bitfield"),
        Type::Enum(_) if native_constructible_enum(types, ty) => Some("enum"),
        Type::Machine(_) if native_constructible_machine(types, ty) => Some("machine"),
        Type::MachineState { .. } if native_constructible_machine(types, ty) => {
            Some("machine_state")
        }
        _ => None,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct VerifiedFunction {
    pub(crate) mir_id: FunctionId,
    pub(crate) symbol: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct VerifiedProgram {
    functions: Vec<VerifiedFunction>,
    by_mir_index: Vec<Option<usize>>,
}

impl VerifiedProgram {
    pub(crate) fn functions(&self) -> &[VerifiedFunction] {
        &self.functions
    }

    pub(crate) fn get(&self, id: FunctionId) -> Option<&VerifiedFunction> {
        let index = usize::try_from(id.index()).ok()?;
        let verified_index = self.by_mir_index.get(index).copied().flatten()?;
        self.functions
            .get(verified_index)
            .filter(|function| function.mir_id == id)
    }
}

pub(crate) fn verify_program(
    program: &Program,
    types: &TypeInterner,
) -> Result<VerifiedProgram, CodegenError> {
    jett_mir::validate(program).map_err(CodegenError::InvalidMir)?;

    let reachable = reachable_function_ids(program)?;
    let mut functions = Vec::with_capacity(reachable.len());
    let mut by_mir_index = vec![None; program.functions.len()];
    let mut unique_symbols = HashSet::with_capacity(reachable.len());
    for function_id in reachable {
        let (function_index, function) = function_by_id(program, function_id)?;
        let symbol = symbol_name(&function.identity, types)?;
        if !unique_symbols.insert(symbol.clone()) {
            return Err(CodegenError::DuplicateSymbol(symbol));
        }
        let verified_index = functions.len();
        by_mir_index[function_index] = Some(verified_index);
        functions.push(VerifiedFunction {
            mir_id: function_id,
            symbol,
        });
    }

    let verified = VerifiedProgram {
        functions,
        by_mir_index,
    };

    let verifier = Verifier {
        program,
        types,
        verified: &verified,
    };
    for verified_function in verified.functions() {
        let (_, function) = function_by_id(program, verified_function.mir_id)?;
        verifier.function(function)?;
    }
    Ok(verified)
}

fn function_by_id(program: &Program, id: FunctionId) -> Result<(usize, &Function), CodegenError> {
    let index = usize::try_from(id.index()).map_err(|_| {
        CodegenError::Backend("reached MIR function index does not fit this host".to_string())
    })?;
    let function = program
        .functions
        .get(index)
        .filter(|function| function.id == id)
        .ok_or_else(|| {
            CodegenError::Backend(
                "verified reachable function is absent from the original MIR table".to_string(),
            )
        })?;
    Ok((index, function))
}

pub(crate) fn scalar_kind(
    types: &TypeInterner,
    ty: TypeId,
    context: impl Into<String>,
) -> Result<ScalarKind, CodegenError> {
    scalar_kind_inner(types, ty, context.into(), &mut HashSet::new())
}
fn scalar_kind_inner(
    types: &TypeInterner,
    ty: TypeId,
    context: String,
    seen: &mut HashSet<TypeId>,
) -> Result<ScalarKind, CodegenError> {
    let type_count = u32::try_from(types.len()).unwrap_or(u32::MAX);
    if ty.index() >= type_count {
        return Err(CodegenError::UnsupportedType {
            type_name: format!("<invalid type {}>", ty.index()),
            context,
        });
    }
    let kind = match types.resolve(ty) {
        Type::Int8 => ScalarKind::SignedInteger(8),
        Type::Int16 => ScalarKind::SignedInteger(16),
        Type::Int32 => ScalarKind::SignedInteger(32),
        Type::Int64 => ScalarKind::SignedInteger(64),
        Type::Uint8 => ScalarKind::UnsignedInteger(8),
        Type::Uint16 => ScalarKind::UnsignedInteger(16),
        Type::Uint32 => ScalarKind::UnsignedInteger(32),
        Type::Uint64 => ScalarKind::UnsignedInteger(64),
        Type::Float32 => ScalarKind::Float(32),
        Type::Float64 => ScalarKind::Float(64),
        Type::Bool => ScalarKind::Bool,
        Type::Nothing => ScalarKind::Nothing,
        Type::String => ScalarKind::String,
        Type::Bytes => ScalarKind::Bytes,
        Type::Secret(inner) | Type::Refinement { base: inner, .. } => {
            return scalar_kind_inner(types, *inner, context, seen);
        }
        Type::Struct(id) => {
            if seen.insert(ty) {
                for (_, field) in &types.resolve_struct(*id).fields {
                    scalar_kind_inner(types, *field, "struct field".into(), seen)?;
                }
            }
            ScalarKind::Struct
        }
        Type::Enum(id) => {
            if seen.insert(ty) {
                for variant in &types.resolve_enum(*id).variants {
                    for (_, field) in &variant.fields {
                        scalar_kind_inner(types, *field, "enum payload".into(), seen)?;
                    }
                }
            }
            ScalarKind::Enum
        }
        Type::Bitfield(id) => {
            if seen.insert(ty) {
                for field in &types.resolve_bitfield(*id).fields {
                    scalar_kind_inner(types, field.ty, "bitfield field".into(), seen)?;
                }
            }
            ScalarKind::Bitfield
        }
        Type::Machine(id) | Type::MachineState { machine: id, .. } => {
            if seen.insert(ty) {
                for state in &types.resolve_machine(*id).states {
                    for (_, field) in &state.fields {
                        scalar_kind_inner(types, *field, "machine payload".into(), seen)?;
                    }
                }
            }
            ScalarKind::Machine
        }
        Type::Function {
            params,
            return_type,
            ..
        } => {
            if seen.insert(ty) {
                for param in params {
                    scalar_kind_inner(types, *param, "function value parameter".into(), seen)?;
                }
                scalar_kind_inner(types, *return_type, "function value result".into(), seen)?;
            }
            ScalarKind::Function
        }
        Type::Actor(id) => {
            if seen.insert(ty) {
                let actor = types.resolve_actor(*id);
                for (_, field) in actor.capability_params.iter().chain(&actor.state_fields) {
                    scalar_kind_inner(types, *field, "actor state field".into(), seen)?;
                }
                for message in &actor.messages {
                    for (_, parameter) in &message.params {
                        scalar_kind_inner(
                            types,
                            *parameter,
                            "actor message parameter".into(),
                            seen,
                        )?;
                    }
                    scalar_kind_inner(
                        types,
                        message.responds,
                        "actor message response".into(),
                        seen,
                    )?;
                }
            }
            ScalarKind::Actor
        }
        Type::List(inner) => {
            if *inner != TypeInterner::NEVER {
                scalar_kind_inner(types, *inner, "list element".into(), seen)?;
            }
            ScalarKind::List
        }
        Type::Set(inner) => {
            scalar_kind_inner(types, *inner, "set element".into(), seen)?;
            ScalarKind::Set
        }
        Type::Map(key, value) => {
            scalar_kind_inner(types, *key, "map key".into(), seen)?;
            scalar_kind_inner(types, *value, "map value".into(), seen)?;
            ScalarKind::Map
        }
        Type::TypeConstruction => ScalarKind::Construction,
        Type::Optional(inner) => {
            scalar_kind_inner(types, *inner, "optional payload".into(), seen)?;
            ScalarKind::Sum
        }
        Type::Result(ok, error) => {
            scalar_kind_inner(types, *ok, "result success payload".into(), seen)?;
            scalar_kind_inner(types, *error, "result failure payload".into(), seen)?;
            ScalarKind::Sum
        }
        Type::Capability(jett_types::CapabilityKind::Stdout) => ScalarKind::Stdout,
        Type::Capability(jett_types::CapabilityKind::Clock) => ScalarKind::Clock,
        Type::Capability(jett_types::CapabilityKind::Random) => ScalarKind::Random,
        Type::Capability(jett_types::CapabilityKind::Environment) => ScalarKind::Environment,
        Type::Capability(jett_types::CapabilityKind::Graphics) => ScalarKind::Graphics,
        unsupported => {
            return Err(CodegenError::UnsupportedType {
                type_name: types.type_name(ty),
                context: format!("{context} ({unsupported:?})"),
            });
        }
    };
    Ok(kind)
}

struct Verifier<'a> {
    program: &'a Program,
    types: &'a TypeInterner,
    verified: &'a VerifiedProgram,
}

impl Verifier<'_> {
    fn function(&self, function: &Function) -> Result<(), CodegenError> {
        self.reject_entry_predecessors(function)?;

        let name = self.function_name(function);
        for param in &function.params {
            scalar_kind(
                self.types,
                param.ty,
                format!("parameter `{}` of `{name}`", param.name),
            )?;
        }
        scalar_kind(
            self.types,
            function.return_type,
            format!("return type of `{name}`"),
        )?;
        for local in &function.locals {
            scalar_kind(
                self.types,
                local.ty,
                format!("local `{}` of `{name}`", local.name),
            )?;
        }
        for block in &function.blocks {
            for statement in &block.statements {
                self.statement(function, statement)?;
            }
            self.terminator(function, &block.terminator)?;
        }
        jett_mir::move_values::MoveValuePlan::analyze(self.program, function, self.types)
            .map_err(|message| self.contract_error(function, function.span, message))?;
        Ok(())
    }

    fn reject_entry_predecessors(&self, function: &Function) -> Result<(), CodegenError> {
        for block in &function.blocks {
            let targets_entry = match &block.terminator.kind {
                TerminatorKind::Goto(target) => *target == function.entry,
                TerminatorKind::Branch {
                    then_block,
                    else_block,
                    ..
                } => *then_block == function.entry || *else_block == function.entry,
                TerminatorKind::Switch {
                    variants,
                    otherwise,
                    ..
                } => {
                    variants
                        .iter()
                        .any(|(_, target, _)| *target == function.entry)
                        || otherwise.is_some_and(|target| target == function.entry)
                }
                TerminatorKind::ForEach { body, exit, .. } => {
                    *body == function.entry || *exit == function.entry
                }
                TerminatorKind::ReflectedTypeDispatch {
                    arms, otherwise, ..
                } => {
                    arms.iter().any(|arm| arm.target == function.entry)
                        || *otherwise == function.entry
                }
                TerminatorKind::Return(_)
                | TerminatorKind::Respond(_)
                | TerminatorKind::Unreachable => false,
            };
            if targets_entry {
                return Err(self.contract_error(
                    function,
                    block.terminator.span,
                    format!(
                        "control-flow edge from block {} targets ABI entry block {}",
                        block.id.index(),
                        function.entry.index()
                    ),
                ));
            }
        }
        Ok(())
    }

    fn statement(&self, function: &Function, statement: &Statement) -> Result<(), CodegenError> {
        match &statement.kind {
            StatementKind::IterationBorrow { source, token, .. } => {
                if !matches!(
                    self.types.resolve(self.sequence_source_type(
                        function,
                        source,
                        statement.span
                    )?),
                    Type::List(_) | Type::Set(_) | Type::Map(..)
                ) || function.local(*token).unwrap().ty != TypeInterner::INT64
                {
                    return Err(self.contract_error(
                        function,
                        statement.span,
                        "invalid iteration loan",
                    ));
                }
                Ok(())
            }
            StatementKind::SequenceLength { source, target }
            | StatementKind::SequenceGet { source, target, .. } => {
                if matches!(
                    statement.kind,
                    StatementKind::SequenceGet { consume: true, .. }
                ) && matches!(source, SequenceSource::Projected { .. })
                {
                    return Err(self.contract_error(
                        function,
                        statement.span,
                        "cannot consume a projected sequence field",
                    ));
                }
                let sequence_ty = self.sequence_source_type(function, source, statement.span)?;
                let element = match self.types.resolve(sequence_ty) {
                    Type::List(element) | Type::Set(element) => {
                        if matches!(statement.kind, StatementKind::SequenceGet { part, .. } if part != jett_mir::SequencePart::Element)
                        {
                            return Err(self.contract_error(
                                function,
                                statement.span,
                                "invalid sequence projection",
                            ));
                        }
                        *element
                    }
                    Type::Map(key, value) => match statement.kind {
                        StatementKind::SequenceGet {
                            part: jett_mir::SequencePart::Key,
                            ..
                        } => *key,
                        StatementKind::SequenceGet {
                            part: jett_mir::SequencePart::Value,
                            ..
                        } => *value,
                        StatementKind::SequenceLength { .. } => *key,
                        _ => {
                            return Err(self.contract_error(
                                function,
                                statement.span,
                                "invalid map projection",
                            ));
                        }
                    },
                    Type::String => {
                        if matches!(statement.kind, StatementKind::SequenceGet { part, .. } if part != jett_mir::SequencePart::Element)
                        {
                            return Err(self.contract_error(
                                function,
                                statement.span,
                                "invalid string iteration projection",
                            ));
                        }
                        TypeInterner::STRING
                    }
                    _ => {
                        return Err(self.contract_error(
                            function,
                            statement.span,
                            "sequence requires list or set",
                        ));
                    }
                };
                let expected = if let StatementKind::SequenceGet { index, .. } = statement.kind {
                    self.require_same_type(
                        function,
                        statement.span,
                        TypeInterner::INT64,
                        function.local(index).unwrap().ty,
                        "sequence index must be int64",
                    )?;
                    element
                } else {
                    TypeInterner::INT64
                };
                self.require_same_type(
                    function,
                    statement.span,
                    expected,
                    function.local(*target).unwrap().ty,
                    &format!(
                        "sequence target type mismatch: expected {}, got {}",
                        self.types.type_name(expected),
                        self.types.type_name(function.local(*target).unwrap().ty)
                    ),
                )
            }

            StatementKind::SumTag { source, target }
            | StatementKind::SumTake { source, target, .. } => {
                let source = function.local(*source).ok_or_else(|| {
                    self.contract_error(function, statement.span, "missing sum place")
                })?;
                let target = function.local(*target).ok_or_else(|| {
                    self.contract_error(function, statement.span, "missing payload place")
                })?;
                let payload = match (self.types.resolve(source.ty), &statement.kind) {
                    (Type::Result(..) | Type::Optional(_), StatementKind::SumTag { .. }) => {
                        TypeInterner::BOOL
                    }
                    (Type::Result(ok, error), StatementKind::SumTake { success, .. }) => {
                        if *success { *ok } else { *error }
                    }
                    (Type::Optional(inner), StatementKind::SumTake { success: true, .. }) => *inner,
                    _ => {
                        return Err(self.contract_error(
                            function,
                            statement.span,
                            "invalid sum projection",
                        ));
                    }
                };
                if matches!(self.types.resolve(target.ty), Type::Secret(inner) if *inner == payload)
                {
                    return Ok(());
                }
                self.require_same_type(
                    function,
                    statement.span,
                    payload,
                    target.ty,
                    "sum payload type mismatch",
                )
            }

            StatementKind::Let { local, value } => {
                let local = function.local(*local).ok_or_else(|| {
                    self.contract_error(
                        function,
                        statement.span,
                        "let target is absent from the local table",
                    )
                })?;
                self.expression(function, value)?;
                self.require_same_type(
                    function,
                    statement.span,
                    local.ty,
                    value.ty,
                    "let initializer type does not match its local",
                )
            }
            StatementKind::Assign { target, value } => {
                let ExpressionKind::Local(local_id) = target.kind else {
                    return Err(self.unsupported(
                        function,
                        statement.span,
                        "assignment target other than a local",
                    ));
                };
                let local = function.local(local_id).ok_or_else(|| {
                    self.contract_error(
                        function,
                        statement.span,
                        "assignment target is absent from the local table",
                    )
                })?;
                if !local.mutable {
                    return Err(self.contract_error(
                        function,
                        statement.span,
                        "assignment targets an immutable local",
                    ));
                }
                self.expression(function, target)?;
                self.expression(function, value)?;
                self.require_same_type(
                    function,
                    statement.span,
                    local.ty,
                    value.ty,
                    "assigned value type does not match its local",
                )
            }
            StatementKind::Evaluate(value) => self.expression(function, value),
            StatementKind::HandleDefault(_) => {
                Err(self.unsupported(function, statement.span, "handle default"))
            }
            StatementKind::Assert { condition, message } => {
                self.expression(function, condition)?;
                if condition.ty != TypeInterner::BOOL {
                    return Err(self.contract_error(
                        function,
                        statement.span,
                        "assert condition is not bool",
                    ));
                }
                if let Some(message) = message {
                    self.expression(function, message)?;
                    self.require_same_type(
                        function,
                        statement.span,
                        TypeInterner::STRING,
                        message.ty,
                        "assert message must be string",
                    )?;
                }
                Ok(())
            }
            StatementKind::Trace(local) => {
                let Some(local) = function.local(*local) else {
                    return Err(self.contract_error(
                        function,
                        statement.span,
                        "trace target is absent from the local table",
                    ));
                };
                if super::emit::debug::debug_layout(self.types, local.ty).is_some() {
                    Ok(())
                } else {
                    Err(self.unsupported(function, statement.span, "aggregate trace value"))
                }
            }
            StatementKind::Breakpoint {
                condition,
                bindings,
            } => {
                if let Some(condition) = condition {
                    self.expression(function, condition)?;
                    self.require_same_type(
                        function,
                        statement.span,
                        TypeInterner::BOOL,
                        condition.ty,
                        "breakpoint condition must be bool",
                    )?;
                }
                for binding in bindings {
                    let local = function.local(*binding).ok_or_else(|| {
                        self.contract_error(
                            function,
                            statement.span,
                            "breakpoint binding is absent from local table",
                        )
                    })?;
                    if super::emit::debug::debug_layout(self.types, local.ty).is_none() {
                        return Err(self.unsupported(
                            function,
                            statement.span,
                            "aggregate breakpoint binding",
                        ));
                    }
                }
                Ok(())
            }
        }
    }

    fn sequence_source_type(
        &self,
        function: &Function,
        source: &SequenceSource,
        span: Span,
    ) -> Result<TypeId, CodegenError> {
        match source {
            SequenceSource::Local(local) => function
                .local(*local)
                .map(|local| local.ty)
                .ok_or_else(|| self.contract_error(function, span, "missing sequence source")),
            SequenceSource::Projected { owner, path, ty } => {
                if path.is_empty() {
                    return Err(self.contract_error(
                        function,
                        span,
                        "projected sequence has no field path",
                    ));
                }
                let mut current =
                    function
                        .local(*owner)
                        .map(|local| local.ty)
                        .ok_or_else(|| {
                            self.contract_error(function, span, "missing projected sequence owner")
                        })?;
                for step in path {
                    if current != step.owner_type {
                        return Err(self.contract_error(
                            function,
                            span,
                            "projected sequence owner type mismatch",
                        ));
                    }
                    let Type::Struct(struct_id) = self.types.resolve(current) else {
                        return Err(self.contract_error(
                            function,
                            span,
                            "projected sequence owner is not a struct",
                        ));
                    };
                    current = self
                        .types
                        .resolve_struct(*struct_id)
                        .fields
                        .get(step.field.index() as usize)
                        .map(|(_, field_ty)| *field_ty)
                        .ok_or_else(|| {
                            self.contract_error(
                                function,
                                span,
                                "projected sequence field is absent",
                            )
                        })?;
                }
                if current != *ty {
                    return Err(self.contract_error(
                        function,
                        span,
                        "projected sequence field type mismatch",
                    ));
                }
                Ok(*ty)
            }
        }
    }

    fn terminator(&self, function: &Function, terminator: &Terminator) -> Result<(), CodegenError> {
        match &terminator.kind {
            TerminatorKind::Return(value) => match value {
                Some(value) => {
                    self.expression(function, value)?;
                    self.require_same_type(
                        function,
                        terminator.span,
                        function.return_type,
                        value.ty,
                        "returned value does not match the function return type",
                    )
                }
                None => {
                    if scalar_kind(self.types, function.return_type, "return without a value")?
                        == ScalarKind::Nothing
                    {
                        Ok(())
                    } else {
                        Err(self.contract_error(
                            function,
                            terminator.span,
                            "non-nothing function returns without a value",
                        ))
                    }
                }
            },
            TerminatorKind::Goto(_) | TerminatorKind::Unreachable => Ok(()),
            TerminatorKind::Branch { condition, .. } => {
                self.expression(function, condition)?;
                if scalar_kind(self.types, condition.ty, "branch condition")? == ScalarKind::Bool {
                    Ok(())
                } else {
                    Err(self.contract_error(
                        function,
                        terminator.span,
                        "branch condition is not bool",
                    ))
                }
            }
            TerminatorKind::Respond(value) => {
                if function.identity.declaration.kind != jett_hir::DeclarationKind::ActorHandler {
                    return Err(self.contract_error(
                        function,
                        terminator.span,
                        "response outside actor handler",
                    ));
                }
                self.expression(function, value)?;
                self.require_same_type(
                    function,
                    terminator.span,
                    function.return_type,
                    value.ty,
                    "actor response type does not match handler return type",
                )
            }
            TerminatorKind::Switch {
                scrutinee,
                variants,
                otherwise,
            } => {
                self.expression(function, scrutinee)?;
                let Type::Enum(enum_id) = self.types.resolve(scrutinee.ty) else {
                    return Err(self.contract_error(
                        function,
                        terminator.span,
                        "variant switch scrutinee is not an enum",
                    ));
                };
                let definition = self.types.resolve_enum(*enum_id);
                let mut seen = std::collections::BTreeSet::new();
                for (variant, _, bindings) in variants {
                    let index = usize::try_from(variant.index()).map_err(|_| {
                        self.contract_error(function, terminator.span, "variant index overflow")
                    })?;
                    let Some(variant_definition) = definition.variants.get(index) else {
                        return Err(self.contract_error(
                            function,
                            terminator.span,
                            "variant switch has an invalid arm",
                        ));
                    };
                    if !seen.insert(index) {
                        return Err(self.contract_error(
                            function,
                            terminator.span,
                            "variant switch has a duplicate arm",
                        ));
                    }
                    if bindings.len() != variant_definition.fields.len() && !bindings.is_empty() {
                        return Err(self.contract_error(
                            function,
                            terminator.span,
                            "variant binding count mismatch",
                        ));
                    }
                    let mut unique_bindings = std::collections::BTreeSet::new();
                    for (binding, (_, expected)) in bindings.iter().zip(&variant_definition.fields)
                    {
                        let Some(local) = function.local(*binding) else {
                            return Err(self.contract_error(
                                function,
                                terminator.span,
                                "variant binding is not a local",
                            ));
                        };
                        if local.ty != *expected || !unique_bindings.insert(binding.index()) {
                            return Err(self.contract_error(
                                function,
                                terminator.span,
                                "variant binding has an invalid type or duplicate local",
                            ));
                        }
                    }
                }
                if otherwise.is_none() && seen.len() != definition.variants.len() {
                    return Err(self.contract_error(
                        function,
                        terminator.span,
                        "variant switch is not exhaustive",
                    ));
                }
                Ok(())
            }
            TerminatorKind::ForEach { .. } => {
                Err(self.unsupported(function, terminator.span, "for-each loop"))
            }
            TerminatorKind::ReflectedTypeDispatch {
                type_info, arms, ..
            } => {
                let valid_type_info = matches!(self.types.resolve(type_info.ty), Type::Struct(id)
                    if self.types.resolve_struct(*id).name == "TypeInfo");
                let identities = arms
                    .iter()
                    .map(|arm| arm.canonical_identity.as_str())
                    .collect::<std::collections::HashSet<_>>();
                if valid_type_info
                    && !arms.is_empty()
                    && arms.iter().all(|arm| !arm.canonical_identity.is_empty())
                    && identities.len() == arms.len()
                {
                    Ok(())
                } else {
                    Err(self.contract_error(
                        function,
                        terminator.span,
                        "invalid checked reflected type dispatch",
                    ))
                }
            }
        }
    }

    fn expression(&self, function: &Function, expression: &Expression) -> Result<(), CodegenError> {
        let kind = scalar_kind(
            self.types,
            expression.ty,
            format!("expression in `{}`", self.function_name(function)),
        )?;
        match &expression.kind {
            ExpressionKind::Int(value) => self.integer_literal(function, expression, *value, kind),
            ExpressionKind::Float(_) => {
                if matches!(kind, ScalarKind::Float(_)) {
                    Ok(())
                } else {
                    Err(self.expression_kind_error(function, expression, "float literal"))
                }
            }
            ExpressionKind::Bool(_) => {
                if kind == ScalarKind::Bool {
                    Ok(())
                } else {
                    Err(self.expression_kind_error(function, expression, "bool literal"))
                }
            }
            ExpressionKind::Nothing => {
                if kind == ScalarKind::Nothing {
                    Ok(())
                } else {
                    Err(self.expression_kind_error(function, expression, "nothing literal"))
                }
            }
            ExpressionKind::Local(local) => {
                let local = function.local(*local).ok_or_else(|| {
                    self.contract_error(
                        function,
                        expression.span,
                        "expression local is absent from the local table",
                    )
                })?;
                if let (
                    Type::Machine(machine),
                    Type::MachineState {
                        machine: narrowed, ..
                    },
                ) = (
                    self.types.resolve(local.ty),
                    self.types.resolve(expression.ty),
                ) && machine == narrowed
                {
                    return Ok(());
                }
                if let (
                    Type::MachineState {
                        machine: narrowed, ..
                    },
                    Type::Machine(machine),
                ) = (
                    self.types.resolve(local.ty),
                    self.types.resolve(expression.ty),
                ) && machine == narrowed
                {
                    return Ok(());
                }
                if matches!(self.types.resolve(expression.ty), Type::Secret(inner) if *inner == local.ty)
                {
                    return Ok(());
                }
                self.require_same_type(
                    function,
                    expression.span,
                    local.ty,
                    expression.ty,
                    "local expression type does not match local metadata",
                )
            }
            ExpressionKind::FunctionRef(target) => {
                let Some(callee) = self
                    .program
                    .functions
                    .get(target.index() as usize)
                    .filter(|callee| callee.id == *target)
                else {
                    return Err(self.contract_error(
                        function,
                        expression.span,
                        "function value target is absent",
                    ));
                };
                let Type::Function {
                    params,
                    view_params,
                    return_type,
                } = self.types.resolve(expression.ty)
                else {
                    return Err(self.expression_kind_error(function, expression, "function value"));
                };
                if params.len() != callee.params.len()
                    || view_params.len() != params.len()
                    || params.iter().zip(view_params).zip(&callee.params).any(
                        |((expected, view), actual)| {
                            *expected != actual.ty
                                || *view != (actual.mode == jett_mir::ParamMode::View)
                        },
                    )
                    || *return_type != callee.return_type
                {
                    return Err(self.contract_error(
                        function,
                        expression.span,
                        "function value signature does not match target",
                    ));
                }
                Ok(())
            }
            ExpressionKind::ClosureRef {
                function: target,
                captures,
            } => {
                let Some(callee) = self
                    .program
                    .functions
                    .get(target.index() as usize)
                    .filter(|callee| callee.id == *target)
                else {
                    return Err(self.contract_error(
                        function,
                        expression.span,
                        "closure target is absent",
                    ));
                };
                let Type::Function {
                    params,
                    view_params,
                    return_type,
                } = self.types.resolve(expression.ty)
                else {
                    return Err(self.expression_kind_error(function, expression, "closure value"));
                };
                if callee.capture_count == 0
                    || callee.capture_count != captures.len()
                    || callee.params.len() != captures.len() + params.len()
                    || view_params.len() != params.len()
                    || *return_type != callee.return_type
                    || callee
                        .params
                        .iter()
                        .skip(callee.capture_count)
                        .zip(params.iter().zip(view_params))
                        .any(|(actual, (expected, view))| {
                            actual.ty != *expected
                                || (actual.mode == jett_mir::ParamMode::View) != *view
                        })
                {
                    return Err(self.contract_error(
                        function,
                        expression.span,
                        "closure signature does not match target",
                    ));
                }
                for (capture, parameter) in captures.iter().zip(&callee.params) {
                    let Some(local) = function.local(*capture) else {
                        return Err(self.contract_error(
                            function,
                            expression.span,
                            "closure capture local is absent",
                        ));
                    };
                    if local.ty != parameter.ty {
                        return Err(self.contract_error(
                            function,
                            expression.span,
                            "closure capture type does not match target",
                        ));
                    }
                    if !matches!(
                        self.types.resolve(local.ty),
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
                            | Type::Nothing
                    ) {
                        return Err(self.contract_error(
                            function,
                            expression.span,
                            "closure capture is not an implicitly copyable source value",
                        ));
                    }
                }
                Ok(())
            }
            ExpressionKind::Unary { op, value } => {
                self.expression(function, value)?;
                self.require_same_type(
                    function,
                    expression.span,
                    expression.ty,
                    value.ty,
                    "unary operand and result types differ",
                )?;
                let supported = match op {
                    UnaryOp::Not => kind == ScalarKind::Bool,
                    UnaryOp::Negate => {
                        matches!(kind, ScalarKind::SignedInteger(_) | ScalarKind::Float(_))
                    }
                };
                if supported {
                    Ok(())
                } else {
                    Err(self.expression_kind_error(function, expression, "unary operation"))
                }
            }
            ExpressionKind::Binary { left, op, right } => {
                self.binary(function, expression, left, *op, right)
            }
            ExpressionKind::Call {
                function: callee,
                args,
                ..
            } => {
                let callee_index = usize::try_from(callee.index()).map_err(|_| {
                    self.contract_error(
                        function,
                        expression.span,
                        "direct call target index does not fit this host",
                    )
                })?;
                let Some(callee) = self.program.functions.get(callee_index) else {
                    return Err(self.contract_error(
                        function,
                        expression.span,
                        "direct call target is absent from the function table",
                    ));
                };
                if args.len() != callee.params.len() {
                    return Err(self.contract_error(
                        function,
                        expression.span,
                        "direct call argument count does not match its signature",
                    ));
                }
                for (argument, parameter) in args.iter().zip(&callee.params) {
                    self.expression(function, argument)?;
                    self.require_same_type(
                        function,
                        argument.span,
                        parameter.ty,
                        argument.ty,
                        "direct call argument type does not match its parameter",
                    )?;
                }
                if matches!(self.types.resolve(expression.ty), Type::Secret(inner) if *inner == callee.return_type)
                {
                    return Ok(());
                }
                self.require_same_type(
                    function,
                    expression.span,
                    callee.return_type,
                    expression.ty,
                    "direct call result type does not match its signature",
                )
            }
            ExpressionKind::Comptime(_) => {
                Err(self.unsupported(function, expression.span, "unbaked comptime expression"))
            }
            ExpressionKind::View(value) | ExpressionKind::Clone(value) => {
                self.expression(function, value)?;
                self.require_same_type(
                    function,
                    expression.span,
                    value.ty,
                    expression.ty,
                    "scalar wrapper changes its value type",
                )
            }
            ExpressionKind::String(_) if kind == ScalarKind::String => Ok(()),
            ExpressionKind::String(_) => {
                Err(self.expression_kind_error(function, expression, "string literal"))
            }
            ExpressionKind::Intrinsic {
                intrinsic,
                args,
                type_arguments,
                reflection_arguments,
                ..
            } => {
                for arg in args {
                    self.expression(function, arg)?;
                }
                if matches!(
                    intrinsic,
                    jett_hir::IntrinsicId::TypeName
                        | jett_hir::IntrinsicId::TypeKind
                        | jett_hir::IntrinsicId::TypeHasSecret
                ) {
                    let expected = if *intrinsic == jett_hir::IntrinsicId::TypeHasSecret {
                        TypeInterner::BOOL
                    } else {
                        TypeInterner::STRING
                    };
                    if !args.is_empty()
                        || type_arguments.len() != 1
                        || reflection_arguments.len() != 1
                        || expression.ty != expected
                    {
                        return Err(self.contract_error(
                            function,
                            expression.span,
                            "invalid checked reflection intrinsic operands",
                        ));
                    }
                    return Ok(());
                }
                if *intrinsic == jett_hir::IntrinsicId::TypeConstructStart {
                    let unsupported_kind = reflection_arguments.len() == 1
                        && !matches!(reflection_arguments[0].kind.as_str(), "struct" | "bitfield");
                    let record_fields =
                        type_arguments
                            .first()
                            .and_then(|ty| match self.types.resolve(*ty) {
                                Type::Struct(id) => {
                                    Some(("struct", self.types.resolve_struct(*id).fields.len()))
                                }
                                Type::Bitfield(id) => Some((
                                    "bitfield",
                                    self.types.resolve_bitfield(*id).fields.len(),
                                )),
                                _ => None,
                            });
                    let valid = args.is_empty()
                        && type_arguments.len() == 1
                        && (unsupported_kind
                            || (record_fields.is_some_and(|(kind, count)| {
                                reflection_arguments.len() == count + 1
                                    && reflection_arguments[0].kind == kind
                            }) && native_constructible_record(self.types, type_arguments[0])))
                        && expression.ty == TypeInterner::TYPE_CONSTRUCTION;
                    return if valid {
                        Ok(())
                    } else {
                        Err(self.unsupported(
                            function,
                            expression.span,
                            "type.construct_start target requiring unsupported construction validation",
                        ))
                    };
                }
                if *intrinsic == jett_hir::IntrinsicId::TypeConstructVariantStart {
                    let fields =
                        type_arguments
                            .first()
                            .and_then(|ty| match self.types.resolve(*ty) {
                                Type::Enum(id) => Some(
                                    self.types
                                        .resolve_enum(*id)
                                        .variants
                                        .iter()
                                        .map(|variant| variant.fields.len())
                                        .sum::<usize>(),
                                ),
                                _ => None,
                            });
                    let valid = args.len() == 1
                        && type_arguments.len() == 1
                        && fields.is_some_and(|count| reflection_arguments.len() == count + 1)
                        && reflection_arguments[0].kind == "enum"
                        && native_constructible_enum(self.types, type_arguments[0])
                        && matches!(self.types.resolve(args[0].ty), Type::Struct(id)
                            if self.types.resolve_struct(*id).name == "TypeVariant")
                        && matches!(self.types.resolve(expression.ty), Type::Result(ok, err)
                            if *ok == TypeInterner::TYPE_CONSTRUCTION && *err == TypeInterner::STRING);
                    return if valid {
                        Ok(())
                    } else {
                        Err(self.unsupported(function, expression.span,
                            "type.construct_variant_start target requiring unsupported construction validation"))
                    };
                }
                if *intrinsic == jett_hir::IntrinsicId::TypeConstructMachineStart {
                    let fields = type_arguments.first().and_then(|ty| {
                        let id = match self.types.resolve(*ty) {
                            Type::Machine(id) | Type::MachineState { machine: id, .. } => *id,
                            _ => return None,
                        };
                        Some(
                            self.types
                                .resolve_machine(id)
                                .states
                                .iter()
                                .map(|state| state.fields.len())
                                .sum::<usize>(),
                        )
                    });
                    let valid = args.len() == 1
                        && type_arguments.len() == 1
                        && fields.is_some_and(|count| reflection_arguments.len() == count + 1)
                        && native_constructible_builder_kind(self.types, type_arguments[0])
                            == Some(reflection_arguments[0].kind.as_str())
                        && matches!(self.types.resolve(args[0].ty), Type::Struct(id)
                            if self.types.resolve_struct(*id).name == "TypeMachineState")
                        && matches!(self.types.resolve(expression.ty), Type::Result(ok, err)
                            if *ok == TypeInterner::TYPE_CONSTRUCTION && *err == TypeInterner::STRING);
                    return if valid {
                        Ok(())
                    } else {
                        Err(self.unsupported(function, expression.span,
                            "type.construct_machine_start target requiring unsupported construction validation"))
                    };
                }
                if *intrinsic == jett_hir::IntrinsicId::TypeConstructPut {
                    // The trusted decoder binds its value type from each
                    // reflected field. Runtime BuilderPut still checks the
                    // field index, name, and exact canonical type, so an
                    // unrelated refinement field sharing this value's base
                    // must not reject the whole record specialization.
                    let checked_json_record_decoder = function.identity.declaration.origin
                        == SourceOrigin::Stdlib
                        && function.identity.declaration.namespace == "json"
                        && function.identity.declaration.name
                            == "json_decode_tree_record_reflected";
                    let unsupported_kind = reflection_arguments.first().is_some_and(|info| {
                        !matches!(
                            info.kind.as_str(),
                            "struct" | "bitfield" | "enum" | "machine" | "machine_state"
                        )
                    });
                    let valid = args.len() == 3
                        && type_arguments.len() == 2
                        && reflection_arguments.len() == 2
                        && (unsupported_kind
                            || native_constructible_builder_kind(self.types, type_arguments[0])
                                == Some(reflection_arguments[0].kind.as_str()))
                        && (unsupported_kind
                            || checked_json_record_decoder
                            || native_builder_value_type_supported(
                                self.types,
                                type_arguments[0],
                                type_arguments[1],
                            ))
                        && args[0].ty == TypeInterner::TYPE_CONSTRUCTION
                        && matches!(self.types.resolve(args[1].ty), Type::Struct(id)
                            if self.types.resolve_struct(*id).name == "TypeField")
                        && args[2].ty == type_arguments[1]
                        && matches!(self.types.resolve(expression.ty), Type::Result(ok, err)
                            if *ok == TypeInterner::TYPE_CONSTRUCTION && *err == TypeInterner::STRING);
                    return if valid {
                        Ok(())
                    } else {
                        Err(self.unsupported(
                            function,
                            expression.span,
                            "type.construct_put target requiring unsupported construction validation",
                        ))
                    };
                }
                if *intrinsic == jett_hir::IntrinsicId::TypeConstructFinish {
                    let unsupported_kind = reflection_arguments.first().is_some_and(|info| {
                        !matches!(
                            info.kind.as_str(),
                            "struct" | "bitfield" | "enum" | "machine" | "machine_state"
                        )
                    });
                    let valid = args.len() == 1
                        && type_arguments.len() == 1
                        && reflection_arguments.len() == 1
                        && (unsupported_kind
                            || native_constructible_builder_kind(self.types, type_arguments[0])
                                == Some(reflection_arguments[0].kind.as_str()))
                        && args[0].ty == TypeInterner::TYPE_CONSTRUCTION
                        && matches!(self.types.resolve(expression.ty), Type::Result(ok, err)
                            if *ok == type_arguments[0] && *err == TypeInterner::STRING);
                    return if valid {
                        Ok(())
                    } else {
                        Err(self.unsupported(
                            function,
                            expression.span,
                            "type.construct_finish target requiring unsupported construction validation",
                        ))
                    };
                }
                if *intrinsic == jett_hir::IntrinsicId::TypeVariantValue {
                    let Some(&owner_ty) = type_arguments.first() else {
                        return Err(self.contract_error(
                            function,
                            expression.span,
                            "type.variant_value has no checked enum type",
                        ));
                    };
                    let Type::Enum(enum_id) = self.types.resolve(owner_ty) else {
                        return Err(self.unsupported(
                            function,
                            expression.span,
                            "type.variant_value of non-enum type",
                        ));
                    };
                    let variants = &self.types.resolve_enum(*enum_id).variants;
                    let valid = type_arguments.len() == 1
                        && reflection_arguments.len() == 1
                        && reflection_arguments[0].kind == "enum"
                        && !variants.is_empty()
                        && args.len() == variants.len() + 1
                        && args[0].ty == owner_ty
                        && args[1..].iter().all(|arg| arg.ty == expression.ty)
                        && matches!(self.types.resolve(expression.ty), Type::Struct(id)
                            if self.types.resolve_struct(*id).name == "TypeVariant");
                    return if valid {
                        Ok(())
                    } else {
                        Err(self.contract_error(
                            function,
                            expression.span,
                            "invalid checked type.variant_value operands",
                        ))
                    };
                }
                if *intrinsic == jett_hir::IntrinsicId::TypeArg {
                    let valid = type_arguments.len() == 1
                        && reflection_arguments.len() == 1
                        && args.len() == reflection_arguments[0].args.len() + 1
                        && args[0].ty == TypeInterner::INT64
                        && args[1..].iter().all(|arg| arg.ty == expression.ty)
                        && matches!(self.types.resolve(expression.ty), Type::Struct(id)
                            if self.types.resolve_struct(*id).name == "TypeInfo");
                    return if valid {
                        Ok(())
                    } else {
                        Err(self.contract_error(
                            function,
                            expression.span,
                            "invalid checked type.arg operands",
                        ))
                    };
                }
                if *intrinsic == jett_hir::IntrinsicId::TypeMachineStateValue {
                    let Some(&owner_ty) = type_arguments.first() else {
                        return Err(self.contract_error(
                            function,
                            expression.span,
                            "type.machine_state_value has no checked machine type",
                        ));
                    };
                    let (machine_id, owner_kind) = match self.types.resolve(owner_ty) {
                        Type::Machine(id) => (*id, "machine"),
                        Type::MachineState { machine, .. } => (*machine, "machine_state"),
                        _ => {
                            return Err(self.unsupported(
                                function,
                                expression.span,
                                "type.machine_state_value of non-machine type",
                            ));
                        }
                    };
                    let states = &self.types.resolve_machine(machine_id).states;
                    let valid = type_arguments.len() == 1
                        && reflection_arguments.len() == 1
                        && reflection_arguments[0].kind == owner_kind
                        && !states.is_empty()
                        && args.len() == states.len() + 1
                        && args[0].ty == owner_ty
                        && args[1..].iter().all(|arg| arg.ty == expression.ty)
                        && matches!(self.types.resolve(expression.ty), Type::Struct(id)
                            if self.types.resolve_struct(*id).name == "TypeMachineState");
                    return if valid {
                        Ok(())
                    } else {
                        Err(self.contract_error(
                            function,
                            expression.span,
                            "invalid checked type.machine_state_value operands",
                        ))
                    };
                }
                if *intrinsic == jett_hir::IntrinsicId::TypeFieldValue {
                    let Some(&owner_ty) = type_arguments.first() else {
                        return Err(self.contract_error(
                            function,
                            expression.span,
                            "type.field_value has no checked owner type",
                        ));
                    };
                    let (field_count, owner_kind) = match self.types.resolve(owner_ty) {
                        Type::Struct(id) => (self.types.resolve_struct(*id).fields.len(), "struct"),
                        Type::Bitfield(id) => {
                            (self.types.resolve_bitfield(*id).fields.len(), "bitfield")
                        }
                        _ => {
                            return Err(self.unsupported(
                                function,
                                expression.span,
                                "type.field_value of non-struct type",
                            ));
                        }
                    };
                    let field_ty = args.get(1).map(|arg| arg.ty);
                    let valid_field_ty = field_ty.is_some_and(|ty| {
                        matches!(self.types.resolve(ty), Type::Struct(id)
                        if self.types.resolve_struct(*id).name == "TypeField")
                    });
                    let valid = type_arguments.len() == 2
                        && type_arguments[1] == expression.ty
                        && reflection_arguments.len() == 2
                        && reflection_arguments[0].kind == owner_kind
                        && args.len() == field_count + 2
                        && args[0].ty == owner_ty
                        && valid_field_ty
                        && args[2..].iter().all(|arg| Some(arg.ty) == field_ty);
                    return if valid {
                        Ok(())
                    } else {
                        Err(self.contract_error(
                            function,
                            expression.span,
                            "invalid checked type.field_value operands",
                        ))
                    };
                }
                if *intrinsic == jett_hir::IntrinsicId::TypeVariantFieldValue {
                    let Some(&owner_ty) = type_arguments.first() else {
                        return Err(self.contract_error(
                            function,
                            expression.span,
                            "type.variant_field_value has no checked enum type",
                        ));
                    };
                    let Type::Enum(enum_id) = self.types.resolve(owner_ty) else {
                        return Err(self.unsupported(
                            function,
                            expression.span,
                            "type.variant_field_value of non-enum type",
                        ));
                    };
                    let field_count = self
                        .types
                        .resolve_enum(*enum_id)
                        .variants
                        .iter()
                        .map(|variant| variant.fields.len())
                        .sum::<usize>();
                    let field_ty = args.get(1).map(|arg| arg.ty);
                    let valid_field_ty = field_ty.is_some_and(|ty| {
                        matches!(self.types.resolve(ty), Type::Struct(id)
                        if self.types.resolve_struct(*id).name == "TypeField")
                    });
                    let valid = type_arguments.len() == 2
                        && type_arguments[1] == expression.ty
                        && reflection_arguments.len() == 2
                        && reflection_arguments[0].kind == "enum"
                        && args.len() == field_count + 2
                        && args[0].ty == owner_ty
                        && valid_field_ty
                        && args[2..].iter().all(|arg| Some(arg.ty) == field_ty);
                    return if valid {
                        Ok(())
                    } else {
                        Err(self.contract_error(
                            function,
                            expression.span,
                            "invalid checked type.variant_field_value operands",
                        ))
                    };
                }
                if *intrinsic == jett_hir::IntrinsicId::TypeMachineFieldValue {
                    let Some(&owner_ty) = type_arguments.first() else {
                        return Err(self.contract_error(
                            function,
                            expression.span,
                            "type.machine_field_value has no checked machine type",
                        ));
                    };
                    let (machine_id, owner_kind) = match self.types.resolve(owner_ty) {
                        Type::Machine(id) => (*id, "machine"),
                        Type::MachineState { machine, .. } => (*machine, "machine_state"),
                        _ => {
                            return Err(self.unsupported(
                                function,
                                expression.span,
                                "type.machine_field_value of non-machine type",
                            ));
                        }
                    };
                    let field_count = self
                        .types
                        .resolve_machine(machine_id)
                        .states
                        .iter()
                        .map(|state| state.fields.len())
                        .sum::<usize>();
                    let field_ty = args.get(1).map(|arg| arg.ty);
                    let valid_field_ty = field_ty.is_some_and(|ty| {
                        matches!(self.types.resolve(ty), Type::Struct(id)
                        if self.types.resolve_struct(*id).name == "TypeField")
                    });
                    let valid = type_arguments.len() == 2
                        && type_arguments[1] == expression.ty
                        && reflection_arguments.len() == 2
                        && reflection_arguments[0].kind == owner_kind
                        && args.len() == field_count + 2
                        && args[0].ty == owner_ty
                        && valid_field_ty
                        && args[2..].iter().all(|arg| Some(arg.ty) == field_ty);
                    return if valid {
                        Ok(())
                    } else {
                        Err(self.contract_error(
                            function,
                            expression.span,
                            "invalid checked type.machine_field_value operands",
                        ))
                    };
                }
                let numeric_generic = matches!(
                    intrinsic,
                    jett_hir::IntrinsicId::MathKernelAbs
                        | jett_hir::IntrinsicId::MathKernelMin
                        | jett_hir::IntrinsicId::MathKernelMax
                );
                if numeric_generic
                    && type_arguments.as_slice()
                        != [args.first().map_or(TypeInterner::ERROR, |a| a.ty)]
                {
                    return Err(self.contract_error(
                        function,
                        expression.span,
                        "numeric intrinsic type argument differs from operand",
                    ));
                }
                let list_generic = crate::values::list_intrinsic(*intrinsic);
                let math_aggregate_generic = matches!(
                    intrinsic,
                    jett_hir::IntrinsicId::MathAverage | jett_hir::IntrinsicId::MathMedian
                );
                if math_aggregate_generic
                    && type_arguments.as_slice()
                        != [args
                            .first()
                            .and_then(|arg| match self.types.resolve(arg.ty) {
                                Type::List(element) => Some(*element),
                                _ => None,
                            })
                            .unwrap_or(TypeInterner::ERROR)]
                {
                    return Err(self.contract_error(
                        function,
                        expression.span,
                        "numeric list intrinsic type argument differs from element",
                    ));
                }
                let list_element =
                    crate::values::list_element(*intrinsic, args, expression.ty, self.types)
                        .unwrap_or(TypeInterner::ERROR);
                let list_type_argument = if *intrinsic == jett_hir::IntrinsicId::ListSortByIndex {
                    match self.types.resolve(list_element) {
                        Type::List(inner) => *inner,
                        _ => TypeInterner::ERROR,
                    }
                } else {
                    list_element
                };
                if list_generic && type_arguments.as_slice() != [list_type_argument] {
                    return Err(self.contract_error(
                        function,
                        expression.span,
                        "list intrinsic type argument differs from element",
                    ));
                }
                let set_generic = crate::values::set_intrinsic(*intrinsic);
                if set_generic
                    && type_arguments.as_slice()
                        != [
                            crate::values::set_element(*intrinsic, args, expression.ty, self.types)
                                .unwrap_or(TypeInterner::ERROR),
                        ]
                {
                    return Err(self.contract_error(
                        function,
                        expression.span,
                        "set intrinsic type argument differs from element",
                    ));
                }
                let map_generic = crate::values::map_intrinsic(*intrinsic);
                if map_generic
                    && type_arguments.as_slice()
                        != crate::values::map_types(*intrinsic, args, expression.ty, self.types)
                            .map(|(k, v)| [k, v])
                            .unwrap_or([TypeInterner::ERROR; 2])
                {
                    return Err(self.contract_error(
                        function,
                        expression.span,
                        "map intrinsic type arguments differ from key and value",
                    ));
                }
                let graphics_generic = *intrinsic == jett_hir::IntrinsicId::GraphicsRun;
                if graphics_generic
                    && (args.len() != 5 || type_arguments.as_slice() != [args[2].ty])
                {
                    return Err(self.contract_error(
                        function,
                        expression.span,
                        "graphics state type argument differs from its initial value",
                    ));
                }
                if !numeric_generic
                    && !list_generic
                    && !math_aggregate_generic
                    && !set_generic
                    && !map_generic
                    && !graphics_generic
                    && !type_arguments.is_empty()
                {
                    return Err(self.unsupported(
                        function,
                        expression.span,
                        format!("generic native intrinsic {intrinsic}"),
                    ));
                }
                crate::values::verify_intrinsic(*intrinsic, args, expression.ty, self.types)
                    .map_err(|message| self.contract_error(function, expression.span, message))
            }
            ExpressionKind::IndirectCall { callee, args, .. } => {
                self.expression(function, callee)?;
                let Type::Function {
                    params,
                    view_params,
                    return_type,
                } = self.types.resolve(callee.ty)
                else {
                    return Err(self.expression_kind_error(function, expression, "indirect call"));
                };
                if params.len() != args.len() || view_params.len() != params.len() {
                    return Err(self.contract_error(
                        function,
                        expression.span,
                        "indirect call argument count does not match function type",
                    ));
                }
                for ((argument, expected), view) in args.iter().zip(params).zip(view_params) {
                    self.expression(function, argument)?;
                    if !view && matches!(argument.kind, ExpressionKind::View(_)) {
                        return Err(self.contract_error(
                            function,
                            argument.span,
                            "owned indirect call parameter cannot accept a view",
                        ));
                    }
                    self.require_same_type(
                        function,
                        argument.span,
                        *expected,
                        argument.ty,
                        "indirect call argument type mismatch",
                    )?;
                }
                self.require_same_type(
                    function,
                    expression.span,
                    *return_type,
                    expression.ty,
                    "indirect call result type mismatch",
                )
            }
            ExpressionKind::StructConstruct {
                struct_type,
                fields,
                validates_refinements,
                ..
            } => {
                if *validates_refinements {
                    let valid_result = matches!(self.types.resolve(expression.ty),
                        Type::Result(ok, err) if *ok == *struct_type && *err == TypeInterner::STRING);
                    if !valid_result {
                        return Err(self.contract_error(
                            function,
                            expression.span,
                            "refinement-validating struct construction must return result[Struct, string]",
                        ));
                    }
                } else {
                    self.require_same_type(
                        function,
                        expression.span,
                        *struct_type,
                        expression.ty,
                        "struct construction type mismatch",
                    )?;
                }
                let Type::Struct(id) = self.types.resolve(*struct_type) else {
                    return Err(self.expression_kind_error(
                        function,
                        expression,
                        "struct construction",
                    ));
                };
                let layout = &self.types.resolve_struct(*id).fields;
                if *validates_refinements
                    && !layout
                        .iter()
                        .any(|(_, ty)| matches!(self.types.resolve(*ty), Type::Refinement { .. }))
                {
                    return Err(self.contract_error(
                        function,
                        expression.span,
                        "refinement-validating struct has no refinement field",
                    ));
                }
                if fields.len() != layout.len() {
                    return Err(self.contract_error(
                        function,
                        expression.span,
                        "struct field count mismatch",
                    ));
                }
                for (field, (_, ty)) in fields.iter().zip(layout) {
                    self.expression(function, field)?;
                    // An exact refinement value passed its predicate at the
                    // earlier boundary. Base values still need native predicate
                    // invocation and must not silently acquire that type.
                    if *validates_refinements
                        && matches!(self.types.resolve(*ty), Type::Refinement { .. })
                        && field.ty != *ty
                    {
                        return Err(self.unsupported(
                            function,
                            field.span,
                            "struct refinement validation from a base value",
                        ));
                    }
                    self.require_same_type(
                        function,
                        field.span,
                        *ty,
                        field.ty,
                        "struct field type mismatch",
                    )?;
                }
                Ok(())
            }
            ExpressionKind::BitfieldConstruct {
                bitfield_type,
                fields,
                validates_widths,
                ..
            } => {
                if *validates_widths {
                    let Type::Result(ok, error) = self.types.resolve(expression.ty) else {
                        return Err(self.expression_kind_error(
                            function,
                            expression,
                            "validated bitfield construction result",
                        ));
                    };
                    self.require_same_type(
                        function,
                        expression.span,
                        *bitfield_type,
                        *ok,
                        "bitfield construction success type mismatch",
                    )?;
                    self.require_same_type(
                        function,
                        expression.span,
                        TypeInterner::STRING,
                        *error,
                        "bitfield construction failure type mismatch",
                    )?;
                } else {
                    self.require_same_type(
                        function,
                        expression.span,
                        *bitfield_type,
                        expression.ty,
                        "bitfield construction type mismatch",
                    )?;
                }
                let Type::Bitfield(id) = self.types.resolve(*bitfield_type) else {
                    return Err(self.expression_kind_error(
                        function,
                        expression,
                        "bitfield construction",
                    ));
                };
                let layout = &self.types.resolve_bitfield(*id).fields;
                if fields.len() != layout.len() {
                    return Err(self.contract_error(
                        function,
                        expression.span,
                        "bitfield field count mismatch",
                    ));
                }
                for (field, definition) in fields.iter().zip(layout) {
                    self.expression(function, field)?;
                    self.require_same_type(
                        function,
                        field.span,
                        definition.ty,
                        field.ty,
                        "bitfield field type mismatch",
                    )?;
                }
                Ok(())
            }
            ExpressionKind::MachineConstruct {
                state_type,
                state,
                payloads,
            } => {
                self.require_same_type(
                    function,
                    expression.span,
                    *state_type,
                    expression.ty,
                    "machine construction type mismatch",
                )?;
                let Type::MachineState {
                    machine,
                    state: declared_state,
                } = self.types.resolve(*state_type)
                else {
                    return Err(self.expression_kind_error(
                        function,
                        expression,
                        "machine construction",
                    ));
                };
                if declared_state.index() != state.index() {
                    return Err(self.contract_error(
                        function,
                        expression.span,
                        "machine constructor state mismatch",
                    ));
                }
                let definition = self
                    .types
                    .resolve_machine(*machine)
                    .state(*declared_state)
                    .ok_or_else(|| {
                        self.contract_error(
                            function,
                            expression.span,
                            "machine constructor state is missing",
                        )
                    })?;
                if payloads.len() != definition.fields.len() {
                    return Err(self.contract_error(
                        function,
                        expression.span,
                        "machine constructor payload count mismatch",
                    ));
                }
                for (payload, (_, expected)) in payloads.iter().zip(&definition.fields) {
                    self.expression(function, payload)?;
                    self.require_same_type(
                        function,
                        payload.span,
                        *expected,
                        payload.ty,
                        "machine constructor payload type mismatch",
                    )?;
                }
                Ok(())
            }
            ExpressionKind::MachineTransition {
                source,
                state_type,
                target,
                payloads,
            } => {
                self.expression(function, source)?;
                self.require_same_type(
                    function,
                    expression.span,
                    *state_type,
                    expression.ty,
                    "machine transition type mismatch",
                )?;
                let Type::MachineState {
                    machine,
                    state: declared_target,
                } = self.types.resolve(*state_type)
                else {
                    return Err(self.expression_kind_error(
                        function,
                        expression,
                        "machine transition",
                    ));
                };
                if declared_target.index() != target.index() {
                    return Err(self.contract_error(
                        function,
                        expression.span,
                        "machine transition target mismatch",
                    ));
                }
                let Type::MachineState {
                    machine: source_machine,
                    state: source_state,
                } = self.types.resolve(source.ty)
                else {
                    return Err(self.unsupported(
                        function,
                        expression.span,
                        "transition from unqualified machine state",
                    ));
                };
                let definition = self.types.resolve_machine(*machine);
                if source_machine != machine
                    || !definition.has_transition(*source_state, *declared_target)
                {
                    return Err(self.contract_error(
                        function,
                        expression.span,
                        "machine transition edge is not declared",
                    ));
                }
                let target_definition = definition.state(*declared_target).ok_or_else(|| {
                    self.contract_error(
                        function,
                        expression.span,
                        "machine transition target is missing",
                    )
                })?;
                if payloads.len() != target_definition.fields.len() {
                    return Err(self.contract_error(
                        function,
                        expression.span,
                        "machine transition payload count mismatch",
                    ));
                }
                for (payload, (_, expected)) in payloads.iter().zip(&target_definition.fields) {
                    self.expression(function, payload)?;
                    self.require_same_type(
                        function,
                        payload.span,
                        *expected,
                        payload.ty,
                        "machine transition payload type mismatch",
                    )?;
                }
                Ok(())
            }
            ExpressionKind::ListConstruct { elements } => {
                let Type::List(element) = self.types.resolve(expression.ty) else {
                    return Err(self.expression_kind_error(
                        function,
                        expression,
                        "list construction",
                    ));
                };
                for value in elements {
                    self.expression(function, value)?;
                    self.require_same_type(
                        function,
                        value.span,
                        *element,
                        value.ty,
                        "list element type mismatch",
                    )?;
                }
                Ok(())
            }
            ExpressionKind::MapConstruct { entries } => {
                let Type::Map(key, value) = self.types.resolve(expression.ty) else {
                    return Err(self.expression_kind_error(
                        function,
                        expression,
                        "map construction",
                    ));
                };
                for entry in entries {
                    self.expression(function, &entry.key)?;
                    self.require_same_type(
                        function,
                        entry.key.span,
                        *key,
                        entry.key.ty,
                        "map key type mismatch",
                    )?;
                    self.expression(function, &entry.value)?;
                    self.require_same_type(
                        function,
                        entry.value.span,
                        *value,
                        entry.value.ty,
                        "map value type mismatch",
                    )?;
                }
                Ok(())
            }
            ExpressionKind::ResultOk(value)
            | ExpressionKind::ResultFail(value)
            | ExpressionKind::OptionalSome(value) => {
                let payload = match (&expression.kind, self.types.resolve(expression.ty)) {
                    (ExpressionKind::ResultOk(_), Type::Result(ok, _)) => *ok,
                    (ExpressionKind::ResultFail(_), Type::Result(_, error)) => *error,
                    (ExpressionKind::OptionalSome(_), Type::Optional(inner)) => *inner,
                    _ => {
                        return Err(self.expression_kind_error(
                            function,
                            expression,
                            "sum constructor",
                        ));
                    }
                };
                self.expression(function, value)?;
                self.require_same_type(
                    function,
                    expression.span,
                    payload,
                    value.ty,
                    "sum constructor payload type mismatch",
                )
            }
            ExpressionKind::OptionalNone => {
                if matches!(self.types.resolve(expression.ty), Type::Optional(_)) {
                    Ok(())
                } else {
                    Err(self.expression_kind_error(function, expression, "optional none"))
                }
            }
            ExpressionKind::Handle { .. } => {
                Err(self.unsupported(function, expression.span, "failure handler"))
            }
            ExpressionKind::EnumConstruct {
                enum_type,
                variant,
                payloads,
            } => {
                self.require_same_type(
                    function,
                    expression.span,
                    expression.ty,
                    *enum_type,
                    "enum construction type mismatch",
                )?;
                let Type::Enum(enum_id) = self.types.resolve(*enum_type) else {
                    return Err(self.expression_kind_error(
                        function,
                        expression,
                        "enum construction",
                    ));
                };
                let index = usize::try_from(variant.index()).map_err(|_| {
                    self.contract_error(function, expression.span, "variant index overflow")
                })?;
                let Some(definition) = self.types.resolve_enum(*enum_id).variants.get(index) else {
                    return Err(self.contract_error(
                        function,
                        expression.span,
                        "enum constructor variant is missing",
                    ));
                };
                if payloads.len() != definition.fields.len() {
                    return Err(self.contract_error(
                        function,
                        expression.span,
                        "enum constructor payload count mismatch",
                    ));
                }
                for (payload, (_, expected)) in payloads.iter().zip(&definition.fields) {
                    self.expression(function, payload)?;
                    self.require_same_type(
                        function,
                        payload.span,
                        *expected,
                        payload.ty,
                        "enum constructor payload type mismatch",
                    )?;
                }
                Ok(())
            }
            ExpressionKind::StringInterpolation(segments) => {
                if kind != ScalarKind::String {
                    return Err(self.expression_kind_error(function, expression, "interpolation"));
                }
                for segment in segments {
                    if let jett_hir::StringSegment::Value(value) = segment {
                        self.expression(function, value)?;
                        if !crate::values::is_formattable(self.types, value.ty) {
                            return Err(self.unsupported(function, value.span, "format value"));
                        }
                    }
                }
                Ok(())
            }
            ExpressionKind::Declassify(value) => {
                self.expression(function, value)?;
                if matches!(self.types.resolve(value.ty), Type::Secret(inner) if *inner == expression.ty)
                {
                    Ok(())
                } else {
                    Err(self.expression_kind_error(function, expression, "declassification"))
                }
            }
            ExpressionKind::Coarsen(value) => {
                self.expression(function, value)?;
                let mut ancestor = value.ty;
                let mut valid = false;
                while let Type::Refinement { base, .. } = self.types.resolve(ancestor) {
                    ancestor = *base;
                    if ancestor == expression.ty {
                        valid = true;
                        break;
                    }
                }
                if valid {
                    Ok(())
                } else {
                    Err(self.expression_kind_error(function, expression, "coarsen"))
                }
            }
            ExpressionKind::RefinementValidated(value) => {
                self.expression(function, value)?;
                let mut current = expression.ty;
                let mut valid = false;
                while let Type::Refinement { base, .. } = self.types.resolve(current) {
                    if *base == value.ty
                        || matches!(self.types.resolve(*base), Type::Secret(inner) if *inner == value.ty)
                    {
                        valid = true;
                        break;
                    }
                    current = *base;
                }
                if valid {
                    Ok(())
                } else {
                    Err(self.expression_kind_error(function, expression, "validated refinement"))
                }
            }
            ExpressionKind::StateIs { value, state } => {
                self.expression(function, value)?;
                let machine = match self.types.resolve(value.ty) {
                    Type::Machine(machine) | Type::MachineState { machine, .. } => *machine,
                    _ => {
                        return Err(self.expression_kind_error(
                            function,
                            expression,
                            "machine state test",
                        ));
                    }
                };
                if kind != ScalarKind::Bool
                    || self
                        .types
                        .resolve_machine(machine)
                        .states
                        .get(state.index() as usize)
                        .is_none()
                {
                    return Err(self.contract_error(
                        function,
                        expression.span,
                        "machine state test has an invalid result or state",
                    ));
                }
                Ok(())
            }
            ExpressionKind::Run(value) => {
                self.expression(function, value)?;
                self.require_same_type(
                    function,
                    expression.span,
                    value.ty,
                    expression.ty,
                    "run result type mismatch",
                )
            }
            ExpressionKind::Join(value) => {
                self.expression(function, value)?;
                // A bare `nothing` means cancellation to the interpreter,
                // while `run` of a nothing-returning call is still pending.
                // MIR has no pending marker on a nothing-typed local yet.
                if value.ty == TypeInterner::NOTHING {
                    return Err(self.unsupported(
                        function,
                        expression.span,
                        "join of nothing-typed task",
                    ));
                }
                let Type::Result(ok, error) = self.types.resolve(expression.ty) else {
                    return Err(self.expression_kind_error(function, expression, "task join"));
                };
                if value.ty == expression.ty || (*ok == value.ty && *error == TypeInterner::STRING)
                {
                    Ok(())
                } else {
                    Err(self.contract_error(
                        function,
                        expression.span,
                        "join result does not match task value",
                    ))
                }
            }
            ExpressionKind::Cancel(value) => {
                self.expression(function, value)?;
                self.require_same_type(
                    function,
                    expression.span,
                    TypeInterner::NOTHING,
                    expression.ty,
                    "cancel must return nothing",
                )
            }
            ExpressionKind::InlineFunction { .. } => {
                Err(self.unsupported(function, expression.span, "inline function"))
            }
            ExpressionKind::ActorSpawn {
                actor_type,
                args,
                constructor,
                ..
            } => {
                let Type::Actor(actor_id) = self.types.resolve(expression.ty) else {
                    return Err(self.expression_kind_error(function, expression, "actor spawn"));
                };
                let actor = self.types.resolve_actor(*actor_id);
                let expected = if let Some(constructor) = constructor {
                    let (_, target) = function_by_id(self.program, *constructor)?;
                    if target.identity.declaration.kind
                        != jett_hir::DeclarationKind::ActorConstructor
                        || target.return_type != expression.ty
                    {
                        return Err(self.contract_error(
                            function,
                            expression.span,
                            "actor spawn target is not its checked constructor",
                        ));
                    }
                    target
                        .params
                        .iter()
                        .map(|param| param.ty)
                        .collect::<Vec<_>>()
                } else {
                    if function.identity.declaration.kind
                        != jett_hir::DeclarationKind::ActorConstructor
                        || function.return_type != expression.ty
                        || actor_type != &actor.name
                    {
                        return Err(self.contract_error(
                            function,
                            expression.span,
                            "actor allocation is outside its checked constructor",
                        ));
                    }
                    actor
                        .capability_params
                        .iter()
                        .chain(&actor.state_fields)
                        .map(|(_, ty)| *ty)
                        .collect::<Vec<_>>()
                };
                if args.len() != expected.len() {
                    return Err(self.contract_error(
                        function,
                        expression.span,
                        "actor spawn argument count does not match its checked fields",
                    ));
                }
                for (argument, expected) in args.iter().zip(expected) {
                    self.expression(function, argument)?;
                    self.require_same_type(
                        function,
                        argument.span,
                        expected,
                        argument.ty,
                        "actor spawn argument type mismatch",
                    )?;
                }
                Ok(())
            }
            ExpressionKind::ActorMessage {
                actor,
                message,
                handler,
                args,
                kind: message_kind,
                ..
            } => {
                self.expression(function, actor)?;
                let Type::Actor(actor_id) = self.types.resolve(actor.ty) else {
                    return Err(self.expression_kind_error(function, expression, "actor message"));
                };
                let definition = self.types.resolve_actor(*actor_id);
                let Some(checked_message) = definition.messages.iter().find(|m| m.name == *message)
                else {
                    return Err(self.contract_error(
                        function,
                        expression.span,
                        "actor message is absent from its checked actor type",
                    ));
                };
                let (_, target) = function_by_id(self.program, *handler)?;
                let capture_count =
                    definition.capability_params.len() + definition.state_fields.len();
                let (expected_namespace, actor_name) = definition
                    .name
                    .rsplit_once('.')
                    .unwrap_or(("", &definition.name));
                let expected_name = format!("{actor_name}.{message}");
                if target.identity.declaration.kind != jett_hir::DeclarationKind::ActorHandler
                    || target.identity.declaration.name != expected_name
                    || target.identity.declaration.namespace != expected_namespace
                    || target.capture_count != capture_count
                    || target.return_type != checked_message.responds
                    || target.params.len() != capture_count + args.len()
                    || args.len() != checked_message.params.len()
                    || target.params[..capture_count]
                        .iter()
                        .zip(
                            definition
                                .capability_params
                                .iter()
                                .chain(&definition.state_fields),
                        )
                        .any(|(parameter, (_, expected))| parameter.ty != *expected)
                {
                    return Err(self.contract_error(
                        function,
                        expression.span,
                        "actor message target does not match its checked handler",
                    ));
                }
                for ((argument, parameter), (_, checked_type)) in args
                    .iter()
                    .zip(&target.params[capture_count..])
                    .zip(&checked_message.params)
                {
                    self.expression(function, argument)?;
                    self.require_same_type(
                        function,
                        argument.span,
                        parameter.ty,
                        argument.ty,
                        "actor message argument type mismatch",
                    )?;
                    self.require_same_type(
                        function,
                        argument.span,
                        *checked_type,
                        parameter.ty,
                        "actor handler parameter type mismatch",
                    )?;
                }
                let expected = if *message_kind == jett_hir::ActorMessageKind::Send {
                    TypeInterner::NOTHING
                } else {
                    checked_message.responds
                };
                self.require_same_type(
                    function,
                    expression.span,
                    expected,
                    expression.ty,
                    "actor message result type mismatch",
                )
            }
            ExpressionKind::Field {
                base,
                owner_type,
                field,
            } => {
                self.expression(function, base)?;
                self.require_same_type(
                    function,
                    expression.span,
                    base.ty,
                    *owner_type,
                    "field owner mismatch",
                )?;
                let ty = match self.types.resolve(*owner_type) {
                    Type::Struct(id) => self
                        .types
                        .resolve_struct(*id)
                        .fields
                        .get(field.index() as usize)
                        .map(|(_, ty)| *ty),
                    Type::Bitfield(id) => self
                        .types
                        .resolve_bitfield(*id)
                        .fields
                        .get(field.index() as usize)
                        .map(|field| field.ty),
                    Type::MachineState { machine, state } => self
                        .types
                        .resolve_machine(*machine)
                        .state(*state)
                        .and_then(|state| state.fields.get(field.index() as usize))
                        .map(|(_, ty)| *ty),
                    _ => {
                        return Err(self.unsupported(
                            function,
                            expression.span,
                            "non-aggregate field access",
                        ));
                    }
                }
                .ok_or_else(|| {
                    self.contract_error(function, expression.span, "invalid aggregate field index")
                })?;
                self.require_same_type(
                    function,
                    expression.span,
                    ty,
                    expression.ty,
                    "projected field type mismatch",
                )
            }
        }
    }

    fn integer_literal(
        &self,
        function: &Function,
        expression: &Expression,
        value: i128,
        kind: ScalarKind,
    ) -> Result<(), CodegenError> {
        let in_range = match kind {
            ScalarKind::SignedInteger(bits) => {
                let magnitude = 1_i128 << (bits - 1);
                (-magnitude..magnitude).contains(&value)
            }
            ScalarKind::UnsignedInteger(bits) => {
                let limit = 1_i128 << bits;
                (0..limit).contains(&value)
            }
            _ => false,
        };
        if in_range {
            Ok(())
        } else {
            Err(self.expression_kind_error(function, expression, "integer literal"))
        }
    }

    fn binary(
        &self,
        function: &Function,
        expression: &Expression,
        left: &Expression,
        op: BinaryOp,
        right: &Expression,
    ) -> Result<(), CodegenError> {
        self.expression(function, left)?;
        self.expression(function, right)?;
        let operand = scalar_kind(self.types, left.ty, "binary operand")?;
        let right_operand = scalar_kind(self.types, right.ty, "binary operand")?;
        let refined_divisor = operand.is_integer()
            && matches!(op, BinaryOp::Divide | BinaryOp::Modulo)
            && matches!(
                self.types.resolve(right.ty),
                Type::Refinement { base, .. } if *base == left.ty
            )
            && operand == right_operand;
        if left.ty != right.ty && !refined_divisor {
            return Err(self.contract_error(
                function,
                expression.span,
                "binary operand types differ",
            ));
        }
        let result = scalar_kind(self.types, expression.ty, "binary result")?;
        if operand == ScalarKind::Enum
            && matches!(op, BinaryOp::Equal | BinaryOp::NotEqual)
            && !known_unit_enum_variant(left)
            && !known_unit_enum_variant(right)
            && let Type::Enum(id) = self.types.resolve(left.ty)
            && self
                .types
                .resolve_enum(*id)
                .variants
                .iter()
                .flat_map(|variant| &variant.fields)
                .any(|(_, field_ty)| {
                    !matches!(
                        scalar_kind(self.types, *field_ty, "enum equality payload"),
                        Ok(ScalarKind::SignedInteger(_)
                            | ScalarKind::UnsignedInteger(_)
                            | ScalarKind::Float(32 | 64)
                            | ScalarKind::Bool
                            | ScalarKind::String)
                    )
                })
            && super::emit::debug::equality_layout(self.types, left.ty).is_none()
        {
            return Err(self.unsupported(function, expression.span, "enum equality payload type"));
        }
        if matches!(op, BinaryOp::Divide | BinaryOp::Modulo)
            && operand.is_integer()
            && integer_expression_is_statically_zero(right)
        {
            return Err(self.contract_error(
                function,
                right.span,
                "integer division or modulo has a statically zero divisor",
            ));
        }
        let supported = match op {
            BinaryOp::Add | BinaryOp::Subtract | BinaryOp::Multiply | BinaryOp::Divide => {
                operand.is_numeric() && result == operand
            }
            BinaryOp::Modulo => operand.is_integer() && result == operand,
            BinaryOp::Equal | BinaryOp::NotEqual => {
                !matches!(
                    operand,
                    ScalarKind::Nothing
                        | ScalarKind::Stdout
                        | ScalarKind::Clock
                        | ScalarKind::Random
                        | ScalarKind::Environment
                        | ScalarKind::Graphics
                        | ScalarKind::Bytes
                        | ScalarKind::Sum
                        | ScalarKind::List
                        | ScalarKind::Map
                        | ScalarKind::Struct
                        | ScalarKind::Bitfield
                        | ScalarKind::Machine
                        | ScalarKind::Function
                        | ScalarKind::Actor
                ) && result == ScalarKind::Bool
            }
            BinaryOp::Less | BinaryOp::Greater | BinaryOp::LessEqual | BinaryOp::GreaterEqual => {
                operand.is_numeric() && result == ScalarKind::Bool
            }
            BinaryOp::And | BinaryOp::Or => {
                operand == ScalarKind::Bool && result == ScalarKind::Bool
            }
        };
        if supported {
            Ok(())
        } else {
            Err(self.expression_kind_error(function, expression, "binary operation"))
        }
    }

    fn require_same_type(
        &self,
        function: &Function,
        span: Span,
        expected: TypeId,
        actual: TypeId,
        message: &str,
    ) -> Result<(), CodegenError> {
        if expected == actual
            || matches!(
                (self.types.resolve(expected), self.types.resolve(actual)),
                (Type::Machine(expected), Type::MachineState { machine: actual, .. })
                    if expected == actual
            )
        {
            Ok(())
        } else {
            Err(self.contract_error(function, span, message))
        }
    }

    fn expression_kind_error(
        &self,
        function: &Function,
        expression: &Expression,
        construct: &str,
    ) -> CodegenError {
        self.contract_error(
            function,
            expression.span,
            format!(
                "{construct} is inconsistent with checked type `{}`",
                self.types.type_name(expression.ty)
            ),
        )
    }

    fn function_name(&self, function: &Function) -> &str {
        self.verified
            .get(function.id)
            .map(|function| function.symbol.as_str())
            .unwrap_or("<invalid-function>")
    }

    fn contract_error(
        &self,
        function: &Function,
        span: Span,
        message: impl Into<String>,
    ) -> CodegenError {
        CodegenError::InvalidMirContract {
            function: self.function_name(function).to_string(),
            span,
            message: message.into(),
        }
    }

    fn unsupported(
        &self,
        function: &Function,
        span: Span,
        construct: impl Into<String>,
    ) -> CodegenError {
        CodegenError::UnsupportedMir {
            function: self.function_name(function).to_string(),
            span,
            construct: construct.into(),
        }
    }
}

fn integer_expression_is_statically_zero(expression: &Expression) -> bool {
    match &expression.kind {
        ExpressionKind::Int(0) => true,
        ExpressionKind::Unary {
            op: UnaryOp::Negate,
            value,
        }
        | ExpressionKind::Comptime(value)
        | ExpressionKind::View(value)
        | ExpressionKind::Clone(value) => integer_expression_is_statically_zero(value),
        _ => false,
    }
}
