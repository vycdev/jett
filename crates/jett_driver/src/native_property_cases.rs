//! Translate deterministic property inputs into typed, compiler-owned HIR.
//! The interpreter chooses cases; only compiled Jett code executes their bodies.

use jett_common::{FileId, Span};
use jett_comptime::value::Value;
use jett_comptime::verify::{PROPERTY_DEFAULT_ITERATIONS, PropertyCase};
use jett_hir::{
    Block, DeclarationId, DeclarationKind, Expression, ExpressionKind, Function, FunctionId,
    FunctionIdentity, HandleKind, IntrinsicId, Local, LocalId, MapEntry, ParamMode, StateId,
    Statement, StatementKind, VariantId,
};
use jett_parser::ast::{Item, Module};
use jett_types::{Type, TypeId, TypeInterner};

pub(super) fn append_property_suite(
    hir: &mut jett_hir::Program,
    module: &Module,
    entry_file: FileId,
    cases: &[PropertyCase],
    types: &TypeInterner,
) -> Result<Option<FunctionId>, Vec<jett_hir::LowerError>> {
    let function_values = function_value_candidates(&hir.functions);
    let mut statements = Vec::new();
    let mut locals = Vec::new();
    let mut first_identity = None;
    let mut first_span = None;
    for item in &module.items {
        let Item::Property(property) = item else {
            continue;
        };
        if property.span.file != entry_file {
            continue;
        }
        let mut matches = hir.functions.iter().filter(|function| {
            function.span == property.span
                && function.identity.declaration.kind == DeclarationKind::Property
        });
        let Some(function) = matches.next() else {
            return Err(error(
                property.span,
                "checked property has no exact HIR function",
            ));
        };
        if matches.next().is_some() {
            return Err(error(
                property.span,
                "checked property has multiple HIR functions",
            ));
        }
        if first_identity.is_none() {
            first_identity = Some(function.identity.declaration.clone());
            first_span = Some(property.span);
        }
        let mut count = 0;
        for case in cases
            .iter()
            .filter(|case| case.property_span == property.span)
        {
            if case.iteration != count || case.arguments.len() != function.params.len() {
                return Err(error(
                    property.span,
                    "generated property cases disagree with checked parameters or iteration order",
                ));
            }
            let mut context = ValueContext {
                types,
                functions: &function_values,
                locals: &mut locals,
                bindings: &mut statements,
            };
            let args = case
                .arguments
                .iter()
                .zip(&function.params)
                .map(|(value, param)| {
                    value_expression(value, param.ty, property.span, &mut context)
                })
                .collect::<Result<Vec<_>, _>>()
                .map_err(|message| error(property.span, message))?;
            statements.push(Statement {
                kind: StatementKind::Expression(Expression {
                    kind: ExpressionKind::Call {
                        function: function.id,
                        evaluation_order: (0..args.len()).collect(),
                        args,
                    },
                    ty: TypeInterner::NOTHING,
                    span: property.span,
                }),
                span: property.span,
            });
            count += 1;
        }
        if count != PROPERTY_DEFAULT_ITERATIONS {
            return Err(error(
                property.span,
                format!(
                    "checked property has {count} generated cases; expected {PROPERTY_DEFAULT_ITERATIONS}"
                ),
            ));
        }
    }
    let Some(span) = first_span else {
        return Ok(None);
    };
    let declaration = first_identity.expect("a suite span has a checked declaration");
    let id = FunctionId::new(hir.functions.len() as u32);
    hir.functions.push(Function {
        id,
        identity: FunctionIdentity {
            declaration: DeclarationId {
                origin: declaration.origin,
                namespace: declaration.namespace,
                name: format!("__native_property_suite:{}", span.start),
                kind: DeclarationKind::Property,
            },
            type_arguments: Vec::new(),
            specialization: jett_typecheck::CheckedGenericSpecialization::default(),
        },
        source_definition: None,
        params: Vec::new(),
        capture_count: 0,
        return_type: TypeInterner::NOTHING,
        locals,
        body: Block { statements, span },
        span,
    });
    Ok(Some(id))
}

fn error(span: Span, message: impl Into<String>) -> Vec<jett_hir::LowerError> {
    vec![jett_hir::LowerError {
        span,
        message: message.into(),
    }]
}

pub(super) struct FunctionValueCandidate {
    name: String,
    kind: DeclarationKind,
    body_span: Span,
    params: Vec<TypeId>,
    view_params: Vec<bool>,
    captures: Vec<(String, TypeId)>,
    return_type: TypeId,
    id: FunctionId,
}

pub(super) fn function_value_candidates(functions: &[Function]) -> Vec<FunctionValueCandidate> {
    functions
        .iter()
        .filter(|function| {
            matches!(
                function.identity.declaration.kind,
                DeclarationKind::Function | DeclarationKind::Verify | DeclarationKind::Property
            )
        })
        .map(|function| {
            let declaration = &function.identity.declaration;
            let name = if declaration.namespace.is_empty() {
                declaration.name.clone()
            } else {
                format!("{}.{}", declaration.namespace, declaration.name)
            };
            FunctionValueCandidate {
                name,
                kind: declaration.kind,
                body_span: function.body.span,
                params: function
                    .params
                    .iter()
                    .skip(function.capture_count)
                    .map(|param| param.ty)
                    .collect(),
                view_params: function
                    .params
                    .iter()
                    .skip(function.capture_count)
                    .map(|param| param.mode == ParamMode::View)
                    .collect(),
                captures: function
                    .params
                    .iter()
                    .take(function.capture_count)
                    .map(|param| (param.name.clone(), param.ty))
                    .collect(),
                return_type: function.return_type,
                id: function.id,
            }
        })
        .collect()
}

pub(super) struct ValueContext<'a> {
    pub(super) types: &'a TypeInterner,
    pub(super) functions: &'a [FunctionValueCandidate],
    pub(super) locals: &'a mut Vec<Local>,
    pub(super) bindings: &'a mut Vec<Statement>,
}

pub(super) fn value_expression(
    value: &Value,
    ty: TypeId,
    span: Span,
    context: &mut ValueContext<'_>,
) -> Result<Expression, String> {
    let types = context.types;
    let kind = match (types.resolve(ty), value) {
        (
            Type::Int8
            | Type::Int16
            | Type::Int32
            | Type::Int64
            | Type::Uint8
            | Type::Uint16
            | Type::Uint32
            | Type::Uint64,
            Value::Int64(number),
        ) => ExpressionKind::Int(i128::from(*number)),
        (Type::Uint8 | Type::Uint16 | Type::Uint32 | Type::Uint64, Value::Uint64(number)) => {
            ExpressionKind::Int(i128::from(*number))
        }
        (Type::Float32 | Type::Float64, Value::Float64(number)) => ExpressionKind::Float(*number),
        (Type::String, Value::String(text)) => ExpressionKind::String(text.clone()),
        (Type::Bool, Value::Bool(flag)) => ExpressionKind::Bool(*flag),
        (Type::Nothing, Value::Nothing) => ExpressionKind::Nothing,
        (
            Type::Function {
                params,
                view_params,
                return_type,
            },
            Value::NamedFunction(name),
        ) => {
            let mut matches = context.functions.iter().filter(|function| {
                function.kind == DeclarationKind::Function
                    && function.name == *name
                    && function.params == *params
                    && function.view_params == *view_params
                    && function.return_type == *return_type
                    && function.captures.is_empty()
            });
            let function = matches
                .next()
                .ok_or_else(|| format!("checked function value `{name}` is absent"))?;
            if matches.next().is_some() {
                return Err(format!("checked function value `{name}` is ambiguous"));
            }
            ExpressionKind::FunctionRef(function.id)
        }
        (
            Type::Function {
                params,
                view_params,
                return_type,
            },
            Value::Function { body, captures, .. },
        ) => {
            let mut matches = context.functions.iter().filter(|function| {
                function.body_span == body.span
                    && function.params == *params
                    && function.view_params == *view_params
                    && function.return_type == *return_type
                    && function
                        .captures
                        .iter()
                        .all(|(name, _)| captures.contains_key(name))
            });
            let function = matches
                .next()
                .ok_or("checked inline function value is absent")?;
            if matches.next().is_some() {
                return Err("checked inline function value is ambiguous".into());
            }
            let function_id = function.id;
            let capture_types = function.captures.clone();
            if capture_types.is_empty() {
                ExpressionKind::FunctionRef(function_id)
            } else {
                let mut capture_locals = Vec::with_capacity(capture_types.len());
                for (name, capture_type) in capture_types {
                    let captured = captures
                        .get(&name)
                        .ok_or_else(|| format!("checked closure capture `{name}` is absent"))?;
                    let expression = value_expression(captured, capture_type, span, context)?;
                    let id = LocalId::new(context.locals.len() as u32);
                    context.locals.push(Local {
                        id,
                        name: format!("__native_baked_capture_{}", id.index()),
                        ty: capture_type,
                        mutable: false,
                        span,
                    });
                    context.bindings.push(Statement {
                        kind: StatementKind::Let {
                            local: id,
                            value: expression,
                        },
                        span,
                    });
                    capture_locals.push(id);
                }
                ExpressionKind::ClosureRef {
                    function: function_id,
                    captures: capture_locals,
                }
            }
        }
        (Type::Bytes, Value::Bytes(bytes)) => return bytes_expression(bytes, span, types),
        (Type::List(element), Value::List(values)) => ExpressionKind::ListConstruct {
            elements: values
                .iter()
                .map(|value| value_expression(value, *element, span, context))
                .collect::<Result<_, _>>()?,
        },
        (Type::Map(key, mapped), Value::Map(entries)) => ExpressionKind::MapConstruct {
            entries: entries
                .iter()
                .map(|(key_value, mapped_value)| {
                    Ok(MapEntry {
                        key: value_expression(key_value, *key, span, context)?,
                        value: value_expression(mapped_value, *mapped, span, context)?,
                    })
                })
                .collect::<Result<_, String>>()?,
        },
        (Type::Set(element), Value::Set(values)) => {
            return set_expression(values, *element, ty, span, context);
        }
        (Type::Optional(_), Value::OptionalNone) => ExpressionKind::OptionalNone,
        (Type::Optional(inner), Value::OptionalSome(value)) => {
            ExpressionKind::OptionalSome(Box::new(value_expression(value, *inner, span, context)?))
        }
        (Type::Result(ok, _), Value::ResultOk(value)) => {
            ExpressionKind::ResultOk(Box::new(value_expression(value, *ok, span, context)?))
        }
        (Type::Result(_, failure), Value::ResultFail(value)) => {
            ExpressionKind::ResultFail(Box::new(value_expression(value, *failure, span, context)?))
        }
        (Type::Refinement { base, .. }, _) => ExpressionKind::RefinementValidated(Box::new(
            value_expression(value, *base, span, context)?,
        )),
        (Type::Struct(id), Value::Struct { fields, .. }) => {
            let definition = types.resolve_struct(*id);
            if fields.len() != definition.fields.len() {
                return Err(format!(
                    "generated struct has {} fields; checked type expects {}",
                    fields.len(),
                    definition.fields.len()
                ));
            }
            ExpressionKind::StructConstruct {
                struct_type: ty,
                fields: definition
                    .fields
                    .iter()
                    .map(|(name, field_type)| {
                        let (_, value) = fields
                            .iter()
                            .find(|(candidate, _)| candidate == name)
                            .ok_or_else(|| format!("generated struct field `{name}` is absent"))?;
                        value_expression(value, *field_type, span, context)
                    })
                    .collect::<Result<_, _>>()?,
                evaluation_order: (0..definition.fields.len()).collect(),
                validates_refinements: false,
                refinement_predicates: Vec::new(),
            }
        }
        (Type::Bitfield(id), Value::Struct { fields, .. }) => {
            let definition = types.resolve_bitfield(*id);
            if fields.len() != definition.fields.len() {
                return Err(format!(
                    "generated bitfield has {} fields; checked type expects {}",
                    fields.len(),
                    definition.fields.len()
                ));
            }
            ExpressionKind::BitfieldConstruct {
                bitfield_type: ty,
                fields: definition
                    .fields
                    .iter()
                    .map(|field| {
                        let (_, value) = fields
                            .iter()
                            .find(|(name, _)| name == &field.name)
                            .ok_or_else(|| {
                                format!("generated bitfield field `{}` is absent", field.name)
                            })?;
                        value_expression(value, field.ty, span, context)
                    })
                    .collect::<Result<_, _>>()?,
                evaluation_order: (0..definition.fields.len()).collect(),
                // The property generator already bounds every bitfield field.
                validates_widths: false,
            }
        }
        (
            Type::Enum(id),
            Value::Enum {
                variant, fields, ..
            },
        ) => {
            let definition = types.resolve_enum(*id);
            let (index, variant_def) = definition
                .variants
                .iter()
                .enumerate()
                .find(|(_, candidate)| candidate.name == *variant)
                .ok_or_else(|| format!("generated enum variant `{variant}` is absent"))?;
            if fields.len() != variant_def.fields.len() {
                return Err(format!(
                    "generated enum variant `{variant}` has wrong arity"
                ));
            }
            let index = u32::try_from(index)
                .map_err(|_| "generated enum variant index exceeds u32".to_owned())?;
            ExpressionKind::EnumConstruct {
                enum_type: ty,
                variant: VariantId::new(index),
                payloads: fields
                    .iter()
                    .zip(&variant_def.fields)
                    .map(|(value, (_, field_type))| {
                        value_expression(value, *field_type, span, context)
                    })
                    .collect::<Result<_, _>>()?,
            }
        }
        (
            Type::Machine(machine) | Type::MachineState { machine, .. },
            Value::Machine { state, fields, .. },
        ) => {
            let definition = types.resolve_machine(*machine);
            let (index, state_def) = definition
                .states
                .iter()
                .enumerate()
                .find(|(_, candidate)| candidate.name == *state)
                .ok_or_else(|| format!("generated machine state `{state}` is absent"))?;
            let index = u32::try_from(index)
                .map_err(|_| "generated machine state index exceeds u32".to_owned())?;
            if let Type::MachineState {
                state: expected, ..
            } = types.resolve(ty)
                && expected.index() != index
            {
                return Err(format!(
                    "generated machine state `{state}` disagrees with checked state"
                ));
            }
            if fields.len() != state_def.fields.len() {
                return Err(format!("generated machine state `{state}` has wrong arity"));
            }
            let state_type = if matches!(types.resolve(ty), Type::MachineState { .. }) {
                ty
            } else {
                types
                    .type_ids()
                    .find(|candidate| {
                        matches!(
                            types.resolve(*candidate),
                            Type::MachineState { machine: owner, state: state_id }
                                if owner == machine && state_id.index() == index
                        )
                    })
                    .ok_or("checked machine state type is absent")?
            };
            return Ok(Expression {
                kind: ExpressionKind::MachineConstruct {
                    state_type,
                    state: StateId::new(index),
                    payloads: fields
                        .iter()
                        .zip(&state_def.fields)
                        .map(|(value, (_, field_type))| {
                            value_expression(value, *field_type, span, context)
                        })
                        .collect::<Result<_, _>>()?,
                },
                ty: state_type,
                span,
            });
        }
        (expected, actual) => {
            return Err(format!(
                "cannot materialize generated `{actual:?}` as checked `{expected:?}`"
            ));
        }
    };
    Ok(Expression { kind, ty, span })
}

fn set_expression(
    values: &[Value],
    element: TypeId,
    ty: TypeId,
    span: Span,
    context: &mut ValueContext<'_>,
) -> Result<Expression, String> {
    let mut current = Expression {
        kind: ExpressionKind::Intrinsic {
            intrinsic: IntrinsicId::SetNew,
            type_arguments: vec![element],
            reflection_arguments: Vec::new(),
            args: Vec::new(),
            evaluation_order: Vec::new(),
        },
        ty,
        span,
    };
    for value in values {
        current = Expression {
            kind: ExpressionKind::Intrinsic {
                intrinsic: IntrinsicId::SetAdd,
                type_arguments: vec![element],
                reflection_arguments: Vec::new(),
                args: vec![current, value_expression(value, element, span, context)?],
                evaluation_order: vec![0, 1],
            },
            ty,
            span,
        };
    }
    Ok(current)
}

fn bytes_expression(bytes: &[u8], span: Span, types: &TypeInterner) -> Result<Expression, String> {
    let empty = Expression {
        kind: ExpressionKind::Intrinsic {
            intrinsic: IntrinsicId::BytesNew,
            type_arguments: Vec::new(),
            reflection_arguments: Vec::new(),
            args: Vec::new(),
            evaluation_order: Vec::new(),
        },
        ty: TypeInterner::BYTES,
        span,
    };
    if bytes.is_empty() {
        return Ok(empty);
    }
    let result_type = types
        .type_ids()
        .find(|id| {
            matches!(types.resolve(*id), Type::Result(ok, failure) if *ok == TypeInterner::BYTES && *failure == TypeInterner::STRING)
        })
        .ok_or("checked `result[bytes, string]` type is absent")?;
    let hex = bytes.iter().map(|byte| format!("{byte:02x}")).collect();
    Ok(Expression {
        kind: ExpressionKind::Handle {
            target: Box::new(Expression {
                kind: ExpressionKind::Intrinsic {
                    intrinsic: IntrinsicId::BytesFromHex,
                    type_arguments: Vec::new(),
                    reflection_arguments: Vec::new(),
                    args: vec![Expression {
                        kind: ExpressionKind::String(hex),
                        ty: TypeInterner::STRING,
                        span,
                    }],
                    evaluation_order: vec![0],
                },
                ty: result_type,
                span,
            }),
            kind: HandleKind::Result,
            error_local: None,
            failure: Block {
                statements: vec![
                    Statement {
                        kind: StatementKind::Assert {
                            condition: Expression {
                                kind: ExpressionKind::Bool(false),
                                ty: TypeInterner::BOOL,
                                span,
                            },
                            message: None,
                        },
                        span,
                    },
                    Statement {
                        kind: StatementKind::HandleDefault(empty),
                        span,
                    },
                ],
                span,
            },
        },
        ty: TypeInterner::BYTES,
        span,
    })
}
