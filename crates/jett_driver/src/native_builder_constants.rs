//! Recreate evaluated construction builders using checked layouts and typed
//! runtime operations. Source computations are never re-executed here.

use jett_common::Span;
use jett_comptime::{Interpreter, Value};
use jett_hir::{
    Block, Expression, ExpressionKind, HandleKind, IntrinsicId, Statement, StatementKind,
};
use jett_types::{ReflectionFieldInfo, ReflectionTypeInfo, Type, TypeId, TypeInterner};

use crate::native_property_cases::{
    ValueContext, evaluated_intrinsic_expression, value_expression,
};

pub(super) fn builder_expression(
    value: &Value,
    span: Span,
    context: &mut ValueContext<'_>,
) -> Result<Expression, String> {
    let Value::TypeConstruction {
        type_name,
        variant,
        state,
        fields,
    } = value
    else {
        return Err("expected evaluated construction builder".into());
    };
    let types = context.types;
    let reflection = context.reflection;
    let owner = checked_type(type_name, context)?;
    let info = reflection
        .get_source_type_info(type_name)
        .ok_or_else(|| format!("checked builder owner metadata `{type_name}` is absent"))?;
    let mut arguments = vec![info.clone()];
    let (intrinsic, metadata, selected_fields, member): (_, _, Vec<ReflectionFieldInfo>, _) =
        match (variant, state) {
            (Some(name), None) => {
                let variants = reflection
                    .get_type_variants(type_name)
                    .or_else(|| reflection.get_type_variants_for_id(owner))
                    .ok_or("checked builder variant metadata is absent")?;
                let selected = variants
                    .iter()
                    .find(|variant| variant.name == *name)
                    .ok_or("evaluated builder variant is absent from checked metadata")?;
                arguments.extend(variants.iter().flat_map(|variant| {
                    variant.fields.iter().map(|field| field.type_info.clone())
                }));
                (
                    IntrinsicId::TypeConstructVariantStart,
                    Some(Interpreter::reflection_variant_info_value(
                        type_name, selected,
                    )),
                    selected.fields.clone(),
                    Some(name.as_str()),
                )
            }
            (None, Some(name)) => {
                let owner_name = machine_owner(owner, types)?;
                let machine = reflection
                    .get_machine(type_name)
                    .or_else(|| reflection.get_machine(owner_name))
                    .ok_or("checked builder machine metadata is absent")?;
                let selected = machine
                    .states
                    .iter()
                    .find(|state| state.name == *name)
                    .ok_or("evaluated builder state is absent from checked metadata")?;
                arguments.extend(
                    machine
                        .states
                        .iter()
                        .flat_map(|state| state.fields.iter().map(|field| field.type_info.clone())),
                );
                (
                    IntrinsicId::TypeConstructMachineStart,
                    Some(Interpreter::reflection_machine_state_info_value(
                        owner_name, selected,
                    )),
                    selected.fields.clone(),
                    Some(name.as_str()),
                )
            }
            (None, None) => {
                let selected = if matches!(info.kind.as_str(), "struct" | "bitfield") {
                    reflection
                        .get_type_fields(type_name)
                        .or_else(|| reflection.get_type_fields_for_id(owner))
                        .ok_or("checked builder field metadata is absent")?
                        .to_vec()
                } else {
                    Vec::new()
                };
                arguments.extend(selected.iter().map(|field| field.type_info.clone()));
                (IntrinsicId::TypeConstructStart, None, selected, None)
            }
            _ => return Err("evaluated builder selects both a variant and a state".into()),
        };
    let mut args = Vec::new();
    if let Some(metadata) = metadata {
        let name = if state.is_some() {
            "TypeMachineState"
        } else {
            "TypeVariant"
        };
        args.push(value_expression(
            &metadata,
            checked_type(name, context)?,
            span,
            context,
        )?);
    }
    let mut current = evaluated_intrinsic_expression(
        intrinsic,
        vec![owner],
        arguments,
        args,
        if member.is_some() {
            builder_result_type(types)?
        } else {
            TypeInterner::TYPE_CONSTRUCTION
        },
        span,
        types,
    )?;
    if member.is_some() {
        current = unwrap_builder(current, span);
    }
    for (index, name, stored_type, payload) in fields {
        let declared_type = checked_type(stored_type, context)?;
        let field = selected_fields
            .get(*index)
            .filter(|field| {
                field.index == *index && field.name == *name
                    && checked_type(&field.type_name, context).ok() == Some(declared_type)
            })
            .ok_or_else(|| format!("evaluated builder field `{type_name}.{name}` at {index} disagrees with checked type `{stored_type}`"))?;
        let mut payload_type = declared_type;
        // Record, enum, and machine builders can contain unvalidated base
        // values. Finishing the builder, not baking it, validates refinements.
        if !matches!(types.resolve(owner), Type::Bitfield(_)) {
            while let Type::Refinement { base, .. } = types.resolve(payload_type) {
                payload_type = *base;
            }
        }
        let payload_info = if declared_type == payload_type {
            field.type_info.clone()
        } else {
            reflection
                .get_source_type_info(&types.type_name(payload_type))
                .ok_or("checked builder storage metadata is absent")?
                .clone()
        };
        let metadata_owner = if state.is_some() {
            machine_owner(owner, types)?
        } else {
            type_name
        };
        let metadata = Interpreter::reflection_field_info_value(metadata_owner, member, field);
        let metadata = value_expression(
            &metadata,
            checked_type("TypeField", context)?,
            span,
            context,
        )?;
        let payload = value_expression(payload, payload_type, span, context)?;
        current = unwrap_builder(
            evaluated_intrinsic_expression(
                IntrinsicId::TypeConstructPut,
                vec![owner, payload_type],
                vec![info.clone(), payload_info],
                vec![current, metadata, payload],
                builder_result_type(types)?,
                span,
                types,
            )?,
            span,
        );
    }
    Ok(current)
}

fn checked_type(name: &str, context: &ValueContext<'_>) -> Result<TypeId, String> {
    if let Some(ty) = context.reflection.source_type_id_for_name(name) {
        return Ok(ty);
    }
    let identity = context
        .reflection
        .get_source_type_info(name)
        .map(ReflectionTypeInfo::canonical_identity);
    let mut candidates = context.types.type_ids().filter(|ty| {
        context.types.type_name(*ty) == name
            || identity.as_ref().is_some_and(|identity| {
                context
                    .reflection
                    .get_type_info_for_id(*ty)
                    .is_some_and(|info| info.canonical_identity() == *identity)
            })
    });
    match (candidates.next(), candidates.next()) {
        (Some(ty), None) => Ok(ty),
        _ => Err(format!(
            "builder type `{name}` has no unambiguous checked identity"
        )),
    }
}

fn machine_owner(owner: TypeId, types: &TypeInterner) -> Result<&str, String> {
    match types.resolve(owner) {
        Type::Machine(id) | Type::MachineState { machine: id, .. } => {
            Ok(&types.resolve_machine(*id).name)
        }
        _ => Err("evaluated machine builder has no checked machine owner".into()),
    }
}

fn builder_result_type(types: &TypeInterner) -> Result<TypeId, String> {
    types
        .type_ids()
        .find(|ty| {
            matches!(types.resolve(*ty), Type::Result(ok, error)
        if *ok == TypeInterner::TYPE_CONSTRUCTION && *error == TypeInterner::STRING)
        })
        .ok_or("checked builder result type is absent".into())
}

fn unwrap_builder(target: Expression, span: Span) -> Expression {
    Expression {
        kind: ExpressionKind::Handle {
            target: Box::new(target),
            kind: HandleKind::Result,
            error_local: None,
            failure: Block {
                statements: vec![Statement {
                    kind: StatementKind::Expression(Expression {
                        kind: ExpressionKind::RuntimeFailure(
                            "cannot reconstruct evaluated construction builder".into(),
                        ),
                        ty: TypeInterner::NOTHING,
                        span,
                    }),
                    span,
                }],
                span,
            },
        },
        ty: TypeInterner::TYPE_CONSTRUCTION,
        span,
    }
}
