//! Exact checked type graph; recursion uses canonical TypeId backreferences.
use super::*;

fn id(types: &TypeInterner, ty: TypeId) -> Result<ResourceCarrierShapeId, String> {
    let index =
        usize::try_from(ty.index()).map_err(|_| "carrier TypeId index is not representable")?;
    if index >= types.len() {
        return Err("carrier child type is outside its checked interner".into());
    }
    Ok(ResourceCarrierShapeId(index))
}
fn fields(
    types: &TypeInterner,
    fields: &[(String, TypeId)],
) -> Result<Vec<ResourceCarrierField>, String> {
    fields
        .iter()
        .enumerate()
        .map(|(ordinal, (name, ty))| {
            Ok(ResourceCarrierField {
                ordinal,
                name: name.clone(),
                shape: id(types, *ty)?,
            })
        })
        .collect()
}
fn arguments(types: &TypeInterner, ty: TypeId) -> Result<Vec<ResourceCarrierShapeId>, String> {
    types
        .nominal_type_arguments(ty)
        .iter()
        .map(|ty| id(types, *ty))
        .collect()
}
fn machine_type(
    types: &TypeInterner,
    owner: jett_types::MachineId,
) -> Result<ResourceCarrierShapeId, String> {
    let ty = types
        .type_ids()
        .find(|ty| matches!(types.resolve(*ty), Type::Machine(id) if *id == owner))
        .ok_or("qualified carrier machine lost its unqualified checked owner")?;
    id(types, ty)
}
fn node(
    manifest: &hir::ResourceManifest,
    types: &TypeInterner,
    ty: TypeId,
) -> Result<ResourceCarrierNode, String> {
    Ok(match types.resolve(ty) {
        Type::Resource(_) => ResourceCarrierNode::Resource {
            kind: manifest
                .kind_for_type(ty)
                .ok_or("carrier Resource node has no exact manifest kind")?,
        },
        Type::Optional(payload) => ResourceCarrierNode::Optional {
            payload: id(types, *payload)?,
        },
        Type::Result(success, failure) => ResourceCarrierNode::Result {
            success: id(types, *success)?,
            failure: id(types, *failure)?,
        },
        Type::List(element) => ResourceCarrierNode::List {
            element: id(types, *element)?,
        },
        Type::Map(key, value) => ResourceCarrierNode::Map {
            key: id(types, *key)?,
            value: id(types, *value)?,
        },
        Type::Struct(owner) => ResourceCarrierNode::Struct {
            owner: *owner,
            arguments: arguments(types, ty)?,
            fields: fields(types, &types.resolve_struct(*owner).fields)?,
        },
        Type::Enum(owner) => ResourceCarrierNode::Enum {
            owner: *owner,
            arguments: arguments(types, ty)?,
            variants: types
                .resolve_enum(*owner)
                .variants
                .iter()
                .enumerate()
                .map(|(ordinal, variant)| {
                    Ok(ResourceCarrierVariant {
                        ordinal,
                        name: variant.name.clone(),
                        discriminant: variant.discriminant,
                        fields: fields(types, &variant.fields)?,
                    })
                })
                .collect::<Result<_, String>>()?,
        },
        Type::Machine(owner) => {
            let definition = types.resolve_machine(*owner);
            ResourceCarrierNode::Machine {
                owner: *owner,
                states: definition
                    .states
                    .iter()
                    .enumerate()
                    .map(|(ordinal, state)| {
                        Ok(ResourceCarrierState {
                            ordinal,
                            name: state.name.clone(),
                            fields: fields(types, &state.fields)?,
                        })
                    })
                    .collect::<Result<_, String>>()?,
                transitions: definition
                    .transitions
                    .iter()
                    .map(|edge| {
                        Ok::<_, String>((
                            usize::try_from(edge.from.index())
                                .map_err(|_| "carrier state index is not representable")?,
                            usize::try_from(edge.to.index())
                                .map_err(|_| "carrier state index is not representable")?,
                        ))
                    })
                    .collect::<Result<_, String>>()?,
            }
        }
        Type::MachineState { machine, state } => ResourceCarrierNode::MachineState {
            machine: machine_type(types, *machine)?,
            state: usize::try_from(state.index())
                .map_err(|_| "carrier state index is not representable")?,
        },
        Type::Refinement { name, base } => ResourceCarrierNode::Refinement {
            name: name.clone(),
            base: id(types, *base)?,
        },
        // Ordinary graph terminals are metadata only. In particular, descriptor
        // signatures never become leaf owners by mentioning Resource types.
        Type::Function { .. } => ResourceCarrierNode::Ordinary,
        _ if !resource_type_pending(types, ty) => ResourceCarrierNode::Ordinary,
        _ => {
            return Err(
                "pending Resource carrier: this checked type family needs its dedicated graph node"
                    .into(),
            );
        }
    })
}
pub(super) fn capture(
    source: &hir::ResourceSourceArchive,
    manifest: &hir::ResourceManifest,
    types: &TypeInterner,
) -> Result<Arc<Vec<ResourceCarrierShape>>, String> {
    source.validate_types(types)?;
    manifest.validate(types)?;
    if source.manifest() != Some(manifest) {
        return Err("carrier shape graph belongs to another checked manifest".into());
    }
    let mut rows = Vec::with_capacity(types.len());
    for ty in types.type_ids() {
        let shape = id(types, ty)?;
        if shape.index() != rows.len() {
            return Err("carrier type graph lost canonical dense ordinals".into());
        }
        rows.push(ResourceCarrierShape {
            id: shape,
            ty,
            node: node(manifest, types, ty)?,
            contains_resource: !matches!(types.resolve(ty), Type::Function { .. })
                && resource_type_pending(types, ty),
            refinement_predicate: predicate(source, types, ty)?,
        });
    }
    Ok(Arc::new(rows))
}

fn predicate(
    source: &hir::ResourceSourceArchive,
    types: &TypeInterner,
    ty: TypeId,
) -> Result<Option<hir::Function>, String> {
    let Type::Refinement { name, base } = types.resolve(ty) else {
        return Ok(None);
    };
    let mut input = *base;
    while let Type::Refinement { base, .. } = types.resolve(input) {
        input = *base;
    }
    if let Type::Secret(inner) = types.resolve(input) {
        input = *inner;
    }
    let mut candidates = source.functions().iter().filter(|function| {
        hir::refinement_predicate_declaration_matches(&function.identity.declaration, name)
            && function.source_definition.is_some()
            && function.capture_count == 0
            && function.params.len() == 1
            && function.params[0].ty == input
            && function.params[0].mode == ParamMode::Owned
            && !function.params[0].mutable
            && function.return_type == TypeInterner::BOOL
    });
    let selected = candidates
        .next()
        .ok_or("carrier refinement graph lost its original checked predicate association")?;
    if candidates.next().is_some() {
        return Err(
            "carrier refinement graph has an ambiguous original predicate association".into(),
        );
    }
    Ok(Some(selected.clone()))
}
