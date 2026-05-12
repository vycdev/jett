use std::collections::HashMap;

/// Immutable reflection metadata produced by type checking and consumed by
/// comptime reflection builtins.
#[derive(Debug, Clone, Default)]
pub struct ReflectionMetadata {
    type_infos: HashMap<String, ReflectionTypeInfo>,
    type_fields: HashMap<String, Vec<ReflectionFieldInfo>>,
    bitfields: HashMap<String, ReflectionBitfieldInfo>,
    type_variants: HashMap<String, Vec<ReflectionVariantInfo>>,
}

impl ReflectionMetadata {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert_type_info(&mut self, info: ReflectionTypeInfo) {
        self.type_infos.insert(info.type_name.clone(), info);
    }

    pub fn get_type_info(&self, type_name: &str) -> Option<&ReflectionTypeInfo> {
        self.type_infos.get(type_name)
    }

    pub fn insert_type_fields(
        &mut self,
        type_name: impl Into<String>,
        fields: Vec<ReflectionFieldInfo>,
    ) {
        self.type_fields.insert(type_name.into(), fields);
    }

    pub fn get_type_fields(&self, type_name: &str) -> Option<&[ReflectionFieldInfo]> {
        self.type_fields.get(type_name).map(Vec::as_slice)
    }

    pub fn insert_bitfield(
        &mut self,
        type_name: impl Into<String>,
        bitfield: ReflectionBitfieldInfo,
    ) {
        self.bitfields.insert(type_name.into(), bitfield);
    }

    pub fn get_bitfield(&self, type_name: &str) -> Option<&ReflectionBitfieldInfo> {
        self.bitfields.get(type_name)
    }

    pub fn insert_type_variants(
        &mut self,
        type_name: impl Into<String>,
        variants: Vec<ReflectionVariantInfo>,
    ) {
        self.type_variants.insert(type_name.into(), variants);
    }

    pub fn get_type_variants(&self, type_name: &str) -> Option<&[ReflectionVariantInfo]> {
        self.type_variants.get(type_name).map(Vec::as_slice)
    }
}

/// Canonical metadata for `TypeInfo`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReflectionTypeInfo {
    pub type_name: String,
    pub kind: String,
    pub primitive_tag: Option<String>,
    pub has_secret: bool,
    pub args: Vec<ReflectionTypeInfo>,
}

impl ReflectionTypeInfo {
    pub fn new(
        type_name: impl Into<String>,
        kind: impl Into<String>,
        primitive_tag: Option<String>,
        has_secret: bool,
        args: Vec<ReflectionTypeInfo>,
    ) -> Self {
        Self {
            type_name: type_name.into(),
            kind: kind.into(),
            primitive_tag,
            has_secret,
            args,
        }
    }
}

/// Canonical metadata for `TypeField`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReflectionFieldInfo {
    pub index: usize,
    pub name: String,
    pub type_name: String,
    pub kind: String,
    pub serialize_name: String,
    pub has_secret: bool,
    pub type_info: ReflectionTypeInfo,
}

impl ReflectionFieldInfo {
    pub fn new(
        index: usize,
        name: impl Into<String>,
        type_name: impl Into<String>,
        kind: impl Into<String>,
        serialize_name: impl Into<String>,
        has_secret: bool,
        type_info: ReflectionTypeInfo,
    ) -> Self {
        Self {
            index,
            name: name.into(),
            type_name: type_name.into(),
            kind: kind.into(),
            serialize_name: serialize_name.into(),
            has_secret,
            type_info,
        }
    }
}

/// Canonical metadata for `TypeBitfield`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReflectionBitfieldInfo {
    pub network_order: bool,
    pub fields: Vec<ReflectionBitfieldFieldInfo>,
}

impl ReflectionBitfieldInfo {
    pub fn new(network_order: bool, fields: Vec<ReflectionBitfieldFieldInfo>) -> Self {
        Self {
            network_order,
            fields,
        }
    }
}

/// Canonical metadata for `TypeBitfieldField`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReflectionBitfieldFieldInfo {
    pub index: usize,
    pub name: String,
    pub shape: String,
    pub width: i64,
    pub type_info: ReflectionTypeInfo,
    pub enum_type: Option<ReflectionTypeInfo>,
}

impl ReflectionBitfieldFieldInfo {
    pub fn new(
        index: usize,
        name: impl Into<String>,
        shape: impl Into<String>,
        width: i64,
        type_info: ReflectionTypeInfo,
        enum_type: Option<ReflectionTypeInfo>,
    ) -> Self {
        Self {
            index,
            name: name.into(),
            shape: shape.into(),
            width,
            type_info,
            enum_type,
        }
    }
}

/// Canonical metadata for `TypeVariant`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReflectionVariantInfo {
    pub index: usize,
    pub name: String,
    pub discriminant: i64,
    pub has_secret: bool,
    pub fields: Vec<ReflectionFieldInfo>,
}

impl ReflectionVariantInfo {
    pub fn new(
        index: usize,
        name: impl Into<String>,
        discriminant: i64,
        has_secret: bool,
        fields: Vec<ReflectionFieldInfo>,
    ) -> Self {
        Self {
            index,
            name: name.into(),
            discriminant,
            has_secret,
            fields,
        }
    }
}
