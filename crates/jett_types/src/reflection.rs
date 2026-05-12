use std::collections::HashMap;

/// Immutable reflection metadata produced by type checking and consumed by
/// comptime reflection builtins.
#[derive(Debug, Clone, Default)]
pub struct ReflectionMetadata {
    type_infos: HashMap<String, ReflectionTypeInfo>,
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
