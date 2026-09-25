use std::collections::HashMap;

use jett_runtime::native_abi::values::NativeDebugTag;
use jett_types::{Type, TypeId, TypeInterner};

use crate::values::list_sort_kind;

enum DebugNode {
    Primitive(u32),
    Nothing,
    Bytes,
    List(u32),
    Set(u32),
    Map(u32, u32),
    Optional(u32),
    Result(u32, u32),
    Record(String, Vec<(String, u32)>),
    Enum(String, Vec<(String, Vec<u32>)>),
    Machine(String, Vec<(String, Vec<u32>)>),
    Capability(String),
    Alias(u32),
}

struct DebugGraph<'a> {
    types: &'a TypeInterner,
    indexes: HashMap<TypeId, u32>,
    nodes: Vec<Option<DebugNode>>,
}

impl DebugGraph<'_> {
    fn node(&mut self, ty: TypeId) -> Option<u32> {
        if let Some(&index) = self.indexes.get(&ty) {
            return Some(index);
        }
        let index = u32::try_from(self.nodes.len()).ok()?;
        self.indexes.insert(ty, index);
        self.nodes.push(None);
        let node = if let Some(kind) = list_sort_kind(self.types, ty) {
            DebugNode::Primitive(kind as u32)
        } else {
            match self.types.resolve(ty).clone() {
                Type::Nothing => DebugNode::Nothing,
                Type::Bytes => DebugNode::Bytes,
                Type::List(element) => DebugNode::List(self.node(element)?),
                Type::Set(element) => DebugNode::Set(self.node(element)?),
                Type::Map(key, value) => DebugNode::Map(self.node(key)?, self.node(value)?),
                Type::Optional(element) => DebugNode::Optional(self.node(element)?),
                Type::Result(ok, error) => DebugNode::Result(self.node(ok)?, self.node(error)?),
                Type::Struct(id) => {
                    let definition = self.types.resolve_struct(id).clone();
                    let mut fields = Vec::with_capacity(definition.fields.len());
                    for (name, ty) in definition.fields {
                        fields.push((name, self.node(ty)?));
                    }
                    DebugNode::Record(definition.name, fields)
                }
                Type::Bitfield(id) => {
                    let definition = self.types.resolve_bitfield(id).clone();
                    let mut fields = Vec::with_capacity(definition.fields.len());
                    for field in definition.fields {
                        fields.push((field.name, self.node(field.ty)?));
                    }
                    DebugNode::Record(definition.name, fields)
                }
                Type::Enum(id) => {
                    let definition = self.types.resolve_enum(id).clone();
                    let mut variants = Vec::with_capacity(definition.variants.len());
                    for variant in definition.variants {
                        let mut fields = Vec::with_capacity(variant.fields.len());
                        for (_, ty) in variant.fields {
                            fields.push(self.node(ty)?);
                        }
                        variants.push((variant.name, fields));
                    }
                    DebugNode::Enum(definition.name, variants)
                }
                Type::Machine(id) | Type::MachineState { machine: id, .. } => {
                    let definition = self.types.resolve_machine(id).clone();
                    let mut states = Vec::with_capacity(definition.states.len());
                    for state in definition.states {
                        let mut fields = Vec::with_capacity(state.fields.len());
                        for (_, ty) in state.fields {
                            fields.push(self.node(ty)?);
                        }
                        states.push((state.name, fields));
                    }
                    DebugNode::Machine(definition.name, states)
                }
                Type::Capability(_) => DebugNode::Capability(self.types.type_name(ty)),
                Type::Refinement { base, .. } => DebugNode::Alias(self.node(base)?),
                Type::Secret(_)
                | Type::TypeConstruction
                | Type::Never
                | Type::Interface(_)
                | Type::Actor(_)
                | Type::Resource(_)
                | Type::Function { .. }
                | Type::Error => return None,
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
                | Type::Bool => return None,
            }
        };
        self.nodes[index as usize] = Some(node);
        Some(index)
    }
}

fn number(bytes: &mut Vec<u8>, value: usize) -> Option<()> {
    bytes.extend_from_slice(&u32::try_from(value).ok()?.to_le_bytes());
    Some(())
}

fn name(bytes: &mut Vec<u8>, value: &str) -> Option<()> {
    number(bytes, value.len())?;
    bytes.extend_from_slice(value.as_bytes());
    Some(())
}

fn encode_node(bytes: &mut Vec<u8>, node: DebugNode) -> Option<()> {
    match node {
        DebugNode::Primitive(kind) => {
            bytes.push(NativeDebugTag::Primitive as u8);
            bytes.extend_from_slice(&kind.to_le_bytes());
        }
        DebugNode::Nothing => bytes.push(NativeDebugTag::Nothing as u8),
        DebugNode::Bytes => bytes.push(NativeDebugTag::Bytes as u8),
        DebugNode::List(child) => child_node(bytes, NativeDebugTag::List, child),
        DebugNode::Set(child) => child_node(bytes, NativeDebugTag::Set, child),
        DebugNode::Optional(child) => child_node(bytes, NativeDebugTag::Optional, child),
        DebugNode::Alias(child) => child_node(bytes, NativeDebugTag::Alias, child),
        DebugNode::Map(key, value) => pair_node(bytes, NativeDebugTag::Map, key, value),
        DebugNode::Result(ok, error) => pair_node(bytes, NativeDebugTag::Result, ok, error),
        DebugNode::Record(type_name, fields) => {
            bytes.push(NativeDebugTag::Record as u8);
            name(bytes, &type_name)?;
            number(bytes, fields.len())?;
            for (field_name, child) in fields {
                name(bytes, &field_name)?;
                bytes.extend_from_slice(&child.to_le_bytes());
            }
        }
        DebugNode::Enum(type_name, variants) => {
            variants_node(bytes, NativeDebugTag::Enum, &type_name, variants)?;
        }
        DebugNode::Machine(type_name, variants) => {
            variants_node(bytes, NativeDebugTag::Machine, &type_name, variants)?;
        }
        DebugNode::Capability(type_name) => {
            bytes.push(NativeDebugTag::Capability as u8);
            name(bytes, &type_name)?;
        }
    }
    Some(())
}

fn child_node(bytes: &mut Vec<u8>, tag: NativeDebugTag, child: u32) {
    bytes.push(tag as u8);
    bytes.extend_from_slice(&child.to_le_bytes());
}

fn pair_node(bytes: &mut Vec<u8>, tag: NativeDebugTag, first: u32, second: u32) {
    bytes.push(tag as u8);
    bytes.extend_from_slice(&first.to_le_bytes());
    bytes.extend_from_slice(&second.to_le_bytes());
}

fn variants_node(
    bytes: &mut Vec<u8>,
    tag: NativeDebugTag,
    type_name: &str,
    variants: Vec<(String, Vec<u32>)>,
) -> Option<()> {
    bytes.push(tag as u8);
    name(bytes, type_name)?;
    number(bytes, variants.len())?;
    for (variant_name, fields) in variants {
        name(bytes, &variant_name)?;
        number(bytes, fields.len())?;
        for child in fields {
            bytes.extend_from_slice(&child.to_le_bytes());
        }
    }
    Some(())
}

pub(crate) fn debug_layout(types: &TypeInterner, ty: TypeId) -> Option<Vec<u8>> {
    let mut graph = DebugGraph {
        types,
        indexes: HashMap::new(),
        nodes: Vec::new(),
    };
    let root = graph.node(ty)?;
    let mut bytes = b"JD\x01".to_vec();
    number(&mut bytes, graph.nodes.len())?;
    bytes.extend_from_slice(&root.to_le_bytes());
    for node in graph.nodes {
        let mut encoded = Vec::new();
        encode_node(&mut encoded, node?)?;
        number(&mut bytes, encoded.len())?;
        bytes.extend_from_slice(&encoded);
    }
    Some(bytes)
}
