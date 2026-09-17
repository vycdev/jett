use std::fmt::Write;

use jett_common::SourceOrigin;
use jett_hir::{DeclarationKind, FunctionIdentity};
use jett_types::{Type, TypeId, TypeInterner};
use sha2::{Digest, Sha256};

use crate::CodegenError;

/// Produce the stable native symbol for one concrete Jett function.
///
/// The encoding contains canonical declaration and structural type identities;
/// it never embeds session-local definition or type indexes. A versioned
/// SHA-256 digest keeps adversarially nested generic names within object-format
/// and linker symbol-length limits.
pub fn symbol_name(
    identity: &FunctionIdentity,
    types: &TypeInterner,
) -> Result<String, CodegenError> {
    let mut canonical = String::new();
    encode_origin(&mut canonical, &identity.declaration.origin);
    push_component(&mut canonical, &identity.declaration.namespace);
    push_component(&mut canonical, &identity.declaration.name);
    canonical.push(match identity.declaration.kind {
        DeclarationKind::Function => 'f',
        DeclarationKind::Method => 'm',
        DeclarationKind::ActorHandler => 'h',
    });
    canonical.push_str(&identity.type_arguments.len().to_string());
    canonical.push('_');
    for argument in &identity.type_arguments {
        let encoded = canonical_type(*argument, types)?;
        push_component(&mut canonical, &encoded);
    }
    push_category(
        &mut canonical,
        "type-argument-kinds",
        identity.specialization.type_argument_kinds.len(),
    );
    for kind in &identity.specialization.type_argument_kinds {
        push_component(&mut canonical, kind);
    }
    push_category(
        &mut canonical,
        "type-info-kinds",
        identity.specialization.type_info_kinds.len(),
    );
    for (index, kind) in &identity.specialization.type_info_kinds {
        push_component(&mut canonical, &index.to_string());
        push_component(&mut canonical, kind);
    }
    push_category(
        &mut canonical,
        "type-info-primitives",
        identity.specialization.type_info_primitives.len(),
    );
    for (index, primitive) in &identity.specialization.type_info_primitives {
        push_component(&mut canonical, &index.to_string());
        match primitive {
            Some(primitive) => {
                push_component(&mut canonical, "some");
                push_component(&mut canonical, primitive);
            }
            None => push_component(&mut canonical, "none"),
        }
    }
    push_category(
        &mut canonical,
        "type-kind-values",
        identity.specialization.type_kind_values.len(),
    );
    for (index, kind) in &identity.specialization.type_kind_values {
        push_component(&mut canonical, &index.to_string());
        push_component(&mut canonical, kind);
    }
    push_category(
        &mut canonical,
        "type-primitive-values",
        identity.specialization.type_primitive_values.len(),
    );
    for (index, primitive) in &identity.specialization.type_primitive_values {
        push_component(&mut canonical, &index.to_string());
        push_component(&mut canonical, primitive);
    }

    let digest = Sha256::digest(canonical.as_bytes());
    let mut symbol = String::from("jett_v0_");
    for byte in digest {
        write!(&mut symbol, "{byte:02x}").expect("writing to a String cannot fail");
    }
    Ok(symbol)
}

fn encode_origin(output: &mut String, origin: &SourceOrigin) {
    match origin {
        SourceOrigin::Project => output.push_str("project_"),
        SourceOrigin::Dependency(name) => {
            output.push_str("dependency_");
            push_component(output, name);
        }
        SourceOrigin::Stdlib => output.push_str("stdlib_"),
    }
}

fn push_component(output: &mut String, component: &str) {
    output.push_str(&component.len().to_string());
    output.push('_');
    output.push_str(component);
    output.push('_');
}

fn push_category(output: &mut String, name: &str, item_count: usize) {
    push_component(output, name);
    push_component(output, &item_count.to_string());
}

fn canonical_type(id: TypeId, types: &TypeInterner) -> Result<String, CodegenError> {
    let Some(ty) = resolve_type(types, id) else {
        return Err(CodegenError::UnsupportedType {
            type_name: format!("<invalid type {}>", id.index()),
            context: "native symbol mangling".to_string(),
        });
    };
    let encoded = match ty {
        Type::Int8 => "i8".to_string(),
        Type::Int16 => "i16".to_string(),
        Type::Int32 => "i32".to_string(),
        Type::Int64 => "i64".to_string(),
        Type::Uint8 => "u8".to_string(),
        Type::Uint16 => "u16".to_string(),
        Type::Uint32 => "u32".to_string(),
        Type::Uint64 => "u64".to_string(),
        Type::Float32 => "f32".to_string(),
        Type::Float64 => "f64".to_string(),
        Type::String => "string".to_string(),
        Type::Bool => "bool".to_string(),
        Type::Bytes => "bytes".to_string(),
        Type::Nothing => "nothing".to_string(),
        Type::TypeConstruction => "type-construction".to_string(),
        Type::Never => "never".to_string(),
        Type::Capability(kind) => format!("capability({})", kind.name()),
        Type::List(inner) => format!("list({})", canonical_type(*inner, types)?),
        Type::Map(key, value) => format!(
            "map({},{})",
            canonical_type(*key, types)?,
            canonical_type(*value, types)?
        ),
        Type::Set(inner) => format!("set({})", canonical_type(*inner, types)?),
        Type::Optional(inner) => format!("optional({})", canonical_type(*inner, types)?),
        Type::Result(ok, error) => format!(
            "result({},{})",
            canonical_type(*ok, types)?,
            canonical_type(*error, types)?
        ),
        Type::Secret(inner) => format!("secret({})", canonical_type(*inner, types)?),
        Type::Struct(id) => format!("struct({})", types.resolve_struct(*id).name),
        Type::Bitfield(id) => format!("bitfield({})", types.resolve_bitfield(*id).name),
        Type::Enum(id) => format!("enum({})", types.resolve_enum(*id).name),
        Type::Interface(id) => format!("interface({})", types.resolve_interface(*id).name),
        Type::Actor(id) => format!("actor({})", types.resolve_actor(*id).name),
        Type::Resource(name) => format!("resource({name})"),
        Type::Machine(id) => format!("machine({})", types.resolve_machine(*id).name),
        Type::MachineState { machine, state } => {
            let machine = types.resolve_machine(*machine);
            let state = machine
                .state(*state)
                .map(|state| state.name.as_str())
                .unwrap_or("<invalid-state>");
            format!("machine-state({},{state})", machine.name)
        }
        Type::Function {
            params,
            return_type,
        } => {
            let mut result = String::from("function(");
            for param in params {
                push_component(&mut result, &canonical_type(*param, types)?);
            }
            result.push_str("returns-");
            result.push_str(&canonical_type(*return_type, types)?);
            result.push(')');
            result
        }
        Type::Refinement { name, base } => {
            format!("refinement({name},{})", canonical_type(*base, types)?)
        }
        Type::Error => {
            return Err(CodegenError::UnsupportedType {
                type_name: "<error>".to_string(),
                context: "native symbol mangling".to_string(),
            });
        }
    };
    Ok(encoded)
}

fn resolve_type(types: &TypeInterner, id: TypeId) -> Option<&Type> {
    let type_count = u32::try_from(types.len()).unwrap_or(u32::MAX);
    (id.index() < type_count).then(|| types.resolve(id))
}
