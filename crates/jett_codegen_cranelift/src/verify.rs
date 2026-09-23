use std::collections::HashSet;

use jett_common::Span;
use jett_hir::{BinaryOp, Expression, ExpressionKind, FunctionId, UnaryOp};
use jett_mir::{Function, Program, Statement, StatementKind, Terminator, TerminatorKind};
use jett_types::{Type, TypeId, TypeInterner};

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
    Stdout,
}

impl ScalarKind {
    pub(crate) fn is_integer(self) -> bool {
        matches!(self, Self::SignedInteger(_) | Self::UnsignedInteger(_))
    }

    fn is_numeric(self) -> bool {
        self.is_integer() || matches!(self, Self::Float(_))
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
                    self.types.resolve(function.local(*source).unwrap().ty),
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
                let element = match self.types.resolve(function.local(*source).unwrap().ty) {
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
                    "sequence target type mismatch",
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
            StatementKind::Assert { .. } => {
                Err(self.unsupported(function, statement.span, "assert"))
            }
            StatementKind::Trace(local) => {
                let Some(local) = function.local(*local) else {
                    return Err(self.contract_error(
                        function,
                        statement.span,
                        "trace target is absent from the local table",
                    ));
                };
                if local.ty == TypeInterner::INT64 {
                    Ok(())
                } else {
                    Err(self.unsupported(function, statement.span, "non-int64 trace"))
                }
            }
            StatementKind::Breakpoint(_) => {
                Err(self.unsupported(function, statement.span, "breakpoint"))
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
            TerminatorKind::Respond(_) => {
                Err(self.unsupported(function, terminator.span, "actor response"))
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
            TerminatorKind::ReflectedTypeDispatch { .. } => {
                Err(self.unsupported(function, terminator.span, "reflected type dispatch"))
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
                self.require_same_type(
                    function,
                    expression.span,
                    local.ty,
                    expression.ty,
                    "local expression type does not match local metadata",
                )
            }
            ExpressionKind::FunctionRef(_) => {
                Err(self.unsupported(function, expression.span, "function value"))
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
                ..
            } => {
                for arg in args {
                    self.expression(function, arg)?;
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
                if list_generic
                    && type_arguments.as_slice()
                        != [crate::values::list_element(
                            *intrinsic,
                            args,
                            expression.ty,
                            self.types,
                        )
                        .unwrap_or(TypeInterner::ERROR)]
                {
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
                if !numeric_generic
                    && !list_generic
                    && !math_aggregate_generic
                    && !set_generic
                    && !map_generic
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
            ExpressionKind::IndirectCall { .. } => {
                Err(self.unsupported(function, expression.span, "indirect call"))
            }
            ExpressionKind::StructConstruct {
                struct_type,
                fields,
                validates_refinements,
                ..
            } => {
                if *validates_refinements {
                    return Err(self.unsupported(
                        function,
                        expression.span,
                        "struct refinement validation",
                    ));
                }
                self.require_same_type(
                    function,
                    expression.span,
                    *struct_type,
                    expression.ty,
                    "struct construction type mismatch",
                )?;
                let Type::Struct(id) = self.types.resolve(*struct_type) else {
                    return Err(self.expression_kind_error(
                        function,
                        expression,
                        "struct construction",
                    ));
                };
                let layout = &self.types.resolve_struct(*id).fields;
                if fields.len() != layout.len() {
                    return Err(self.contract_error(
                        function,
                        expression.span,
                        "struct field count mismatch",
                    ));
                }
                for (field, (_, ty)) in fields.iter().zip(layout) {
                    self.expression(function, field)?;
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
                    return Err(self.unsupported(
                        function,
                        expression.span,
                        "bitfield width validation",
                    ));
                }
                self.require_same_type(
                    function,
                    expression.span,
                    *bitfield_type,
                    expression.ty,
                    "bitfield construction type mismatch",
                )?;
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
            ExpressionKind::MachineConstruct { .. } => {
                Err(self.unsupported(function, expression.span, "machine construction"))
            }
            ExpressionKind::MachineTransition { .. } => {
                Err(self.unsupported(function, expression.span, "machine transition"))
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
            ExpressionKind::Coarsen(_) => {
                Err(self.unsupported(function, expression.span, "coarsen"))
            }
            ExpressionKind::StateIs { .. } => {
                Err(self.unsupported(function, expression.span, "machine state test"))
            }
            ExpressionKind::Run(_) | ExpressionKind::Join(_) | ExpressionKind::Cancel(_) => {
                Err(self.unsupported(function, expression.span, "task operation"))
            }
            ExpressionKind::InlineFunction { .. } => {
                Err(self.unsupported(function, expression.span, "inline function"))
            }
            ExpressionKind::ActorSpawn { .. } | ExpressionKind::ActorMessage { .. } => {
                Err(self.unsupported(function, expression.span, "actor operation"))
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
            && let Type::Enum(id) = self.types.resolve(left.ty)
            && self
                .types
                .resolve_enum(*id)
                .variants
                .iter()
                .any(|variant| !variant.fields.is_empty())
        {
            return Err(self.unsupported(function, expression.span, "payload enum equality"));
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
                        | ScalarKind::Bytes
                        | ScalarKind::Sum
                        | ScalarKind::List
                        | ScalarKind::Map
                        | ScalarKind::Struct
                        | ScalarKind::Bitfield
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
        if expected == actual {
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
