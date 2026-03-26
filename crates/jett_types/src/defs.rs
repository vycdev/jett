use crate::types::TypeId;

/// Handle to a struct definition in the type registry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct StructId(pub(crate) u32);

impl StructId {
    pub fn index(self) -> u32 {
        self.0
    }
}

/// Handle to an enum definition in the type registry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct EnumId(pub(crate) u32);

impl EnumId {
    pub fn index(self) -> u32 {
        self.0
    }
}

/// Handle to an interface definition in the type registry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct InterfaceId(pub(crate) u32);

impl InterfaceId {
    pub fn index(self) -> u32 {
        self.0
    }
}

/// Describes a user-defined struct type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StructDef {
    pub name: String,
    /// (field_name, field_type)
    pub fields: Vec<(String, TypeId)>,
    pub methods: Vec<FunctionSig>,
}

/// Describes a user-defined enum type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnumDef {
    pub name: String,
    pub variants: Vec<VariantDef>,
}

/// Describes a user-defined interface type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InterfaceDef {
    pub name: String,
    pub methods: Vec<FunctionSig>,
}

/// A single variant of an enum.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VariantDef {
    pub name: String,
    /// (field_name, field_type) — empty for unit variants.
    pub fields: Vec<(String, TypeId)>,
}

/// A function signature (used for methods on structs, etc.).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunctionSig {
    pub name: String,
    /// (param_name, param_type, is_view)
    pub params: Vec<(String, TypeId, bool)>,
    pub return_type: TypeId,
    /// True when the function takes no capability (view) parameters.
    pub is_pure: bool,
}
