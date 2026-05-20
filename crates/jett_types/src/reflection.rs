use std::collections::HashMap;

use crate::TypeId;

/// Immutable reflection metadata produced by type checking and consumed by
/// comptime reflection builtins.
#[derive(Debug, Clone, Default)]
pub struct ReflectionMetadata {
    type_ids_by_name: HashMap<String, TypeId>,
    type_infos_by_id: HashMap<TypeId, ReflectionTypeInfo>,
    type_fields_by_id: HashMap<TypeId, Vec<ReflectionFieldInfo>>,
    bitfields_by_id: HashMap<TypeId, ReflectionBitfieldInfo>,
    type_variants_by_id: HashMap<TypeId, Vec<ReflectionVariantInfo>>,
    type_infos: HashMap<String, ReflectionTypeInfo>,
    type_fields: HashMap<String, Vec<ReflectionFieldInfo>>,
    bitfields: HashMap<String, ReflectionBitfieldInfo>,
    type_variants: HashMap<String, Vec<ReflectionVariantInfo>>,
}

impl ReflectionMetadata {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn bind_type_name(&mut self, type_name: impl Into<String>, type_id: TypeId) {
        let type_name = type_name.into();
        self.type_ids_by_name.insert(type_name.clone(), type_id);
        if let Some(info) = self.type_infos.get(&type_name) {
            self.type_infos_by_id.insert(type_id, info.clone());
        }
        if let Some(fields) = self.type_fields.get(&type_name) {
            self.type_fields_by_id.insert(type_id, fields.clone());
        }
        if let Some(bitfield) = self.bitfields.get(&type_name) {
            self.bitfields_by_id.insert(type_id, bitfield.clone());
        }
        if let Some(variants) = self.type_variants.get(&type_name) {
            self.type_variants_by_id.insert(type_id, variants.clone());
        }
    }

    pub fn type_id_for_name(&self, type_name: &str) -> Option<TypeId> {
        self.type_ids_by_name.get(type_name).copied()
    }

    pub fn insert_type_info_for_id(&mut self, type_id: TypeId, info: ReflectionTypeInfo) {
        self.bind_type_name(info.type_name.clone(), type_id);
        self.type_infos_by_id.insert(type_id, info.clone());
        self.type_infos.insert(info.type_name.clone(), info);
    }

    pub fn insert_type_info(&mut self, info: ReflectionTypeInfo) {
        if let Some(type_id) = self.type_ids_by_name.get(&info.type_name) {
            self.type_infos_by_id.insert(*type_id, info.clone());
        }
        self.type_infos.insert(info.type_name.clone(), info);
    }

    pub fn get_type_info_for_id(&self, type_id: TypeId) -> Option<&ReflectionTypeInfo> {
        self.type_infos_by_id.get(&type_id)
    }

    pub fn get_type_info(&self, type_name: &str) -> Option<&ReflectionTypeInfo> {
        if let Some(type_id) = self.type_id_for_name(type_name)
            && let Some(info) = self.get_type_info_for_id(type_id)
        {
            return Some(info);
        }
        self.type_infos.get(type_name)
    }

    pub fn insert_type_fields(
        &mut self,
        type_name: impl Into<String>,
        fields: Vec<ReflectionFieldInfo>,
    ) {
        let type_name = type_name.into();
        if let Some(type_id) = self.type_ids_by_name.get(&type_name) {
            self.type_fields_by_id.insert(*type_id, fields.clone());
        }
        self.type_fields.insert(type_name, fields);
    }

    pub fn insert_type_fields_for_id(
        &mut self,
        type_id: TypeId,
        type_name: impl Into<String>,
        fields: Vec<ReflectionFieldInfo>,
    ) {
        let type_name = type_name.into();
        self.bind_type_name(type_name.clone(), type_id);
        self.type_fields_by_id.insert(type_id, fields.clone());
        self.type_fields.insert(type_name, fields);
    }

    pub fn get_type_fields_for_id(&self, type_id: TypeId) -> Option<&[ReflectionFieldInfo]> {
        self.type_fields_by_id.get(&type_id).map(Vec::as_slice)
    }

    pub fn get_type_fields(&self, type_name: &str) -> Option<&[ReflectionFieldInfo]> {
        if let Some(type_id) = self.type_id_for_name(type_name)
            && let Some(fields) = self.get_type_fields_for_id(type_id)
        {
            return Some(fields);
        }
        self.type_fields.get(type_name).map(Vec::as_slice)
    }

    pub fn insert_bitfield(
        &mut self,
        type_name: impl Into<String>,
        bitfield: ReflectionBitfieldInfo,
    ) {
        let type_name = type_name.into();
        if let Some(type_id) = self.type_ids_by_name.get(&type_name) {
            self.bitfields_by_id.insert(*type_id, bitfield.clone());
        }
        self.bitfields.insert(type_name, bitfield);
    }

    pub fn insert_bitfield_for_id(
        &mut self,
        type_id: TypeId,
        type_name: impl Into<String>,
        bitfield: ReflectionBitfieldInfo,
    ) {
        let type_name = type_name.into();
        self.bind_type_name(type_name.clone(), type_id);
        self.bitfields_by_id.insert(type_id, bitfield.clone());
        self.bitfields.insert(type_name, bitfield);
    }

    pub fn get_bitfield_for_id(&self, type_id: TypeId) -> Option<&ReflectionBitfieldInfo> {
        self.bitfields_by_id.get(&type_id)
    }

    pub fn get_bitfield(&self, type_name: &str) -> Option<&ReflectionBitfieldInfo> {
        if let Some(type_id) = self.type_id_for_name(type_name)
            && let Some(bitfield) = self.get_bitfield_for_id(type_id)
        {
            return Some(bitfield);
        }
        self.bitfields.get(type_name)
    }

    pub fn insert_type_variants(
        &mut self,
        type_name: impl Into<String>,
        variants: Vec<ReflectionVariantInfo>,
    ) {
        let type_name = type_name.into();
        if let Some(type_id) = self.type_ids_by_name.get(&type_name) {
            self.type_variants_by_id.insert(*type_id, variants.clone());
        }
        self.type_variants.insert(type_name, variants);
    }

    pub fn insert_type_variants_for_id(
        &mut self,
        type_id: TypeId,
        type_name: impl Into<String>,
        variants: Vec<ReflectionVariantInfo>,
    ) {
        let type_name = type_name.into();
        self.bind_type_name(type_name.clone(), type_id);
        self.type_variants_by_id.insert(type_id, variants.clone());
        self.type_variants.insert(type_name, variants);
    }

    pub fn get_type_variants_for_id(&self, type_id: TypeId) -> Option<&[ReflectionVariantInfo]> {
        self.type_variants_by_id.get(&type_id).map(Vec::as_slice)
    }

    pub fn get_type_variants(&self, type_name: &str) -> Option<&[ReflectionVariantInfo]> {
        if let Some(type_id) = self.type_id_for_name(type_name)
            && let Some(variants) = self.get_type_variants_for_id(type_id)
        {
            return Some(variants);
        }
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

#[cfg(test)]
mod tests {
    use super::*;

    fn info(type_name: &str, kind: &str) -> ReflectionTypeInfo {
        ReflectionTypeInfo::new(type_name, kind, None, false, Vec::new())
    }

    fn field(name: &str, type_name: &str) -> ReflectionFieldInfo {
        ReflectionFieldInfo::new(
            0,
            name,
            type_name,
            "primitive",
            name,
            false,
            info(type_name, "primitive"),
        )
    }

    #[test]
    fn type_id_lookup_preserves_legacy_string_lookup() {
        let type_id = TypeId(42);
        let mut metadata = ReflectionMetadata::new();

        metadata.insert_type_info_for_id(type_id, info("models.Box[int64]", "struct"));
        metadata.insert_type_info(info("Box[int64]", "struct"));

        assert_eq!(
            metadata
                .get_type_info("models.Box[int64]")
                .expect("canonical info should exist")
                .kind,
            "struct"
        );
        assert_eq!(
            metadata
                .get_type_info("Box[int64]")
                .expect("legacy string info should exist")
                .kind,
            "struct"
        );
    }

    #[test]
    fn string_type_info_insertion_updates_existing_type_id_binding() {
        let type_id = TypeId(45);
        let mut metadata = ReflectionMetadata::new();

        metadata.insert_type_info_for_id(type_id, info("models.Box", "struct"));
        metadata.insert_type_info(info("models.Box", "checked_struct"));

        assert_eq!(
            metadata
                .get_type_info("models.Box")
                .expect("bound name should still resolve")
                .kind,
            "checked_struct"
        );
    }

    #[test]
    fn string_owner_metadata_insertions_update_existing_type_id_bindings() {
        let type_id = TypeId(47);
        let mut metadata = ReflectionMetadata::new();

        metadata.bind_type_name("models.Packet", type_id);
        metadata.insert_type_fields("models.Packet", vec![field("old", "int64")]);
        metadata.insert_bitfield(
            "models.Packet",
            ReflectionBitfieldInfo::new(
                false,
                vec![ReflectionBitfieldFieldInfo::new(
                    0,
                    "old",
                    "bits",
                    1,
                    info("int64", "primitive"),
                    None,
                )],
            ),
        );
        metadata.insert_type_variants(
            "models.Packet",
            vec![ReflectionVariantInfo::new(
                0,
                "old",
                0,
                false,
                vec![field("old_payload", "int64")],
            )],
        );

        metadata.insert_type_fields("models.Packet", vec![field("version", "int64")]);
        metadata.insert_bitfield(
            "models.Packet",
            ReflectionBitfieldInfo::new(
                true,
                vec![ReflectionBitfieldFieldInfo::new(
                    0,
                    "version",
                    "bits",
                    4,
                    info("int64", "primitive"),
                    None,
                )],
            ),
        );
        metadata.insert_type_variants(
            "models.Packet",
            vec![ReflectionVariantInfo::new(
                0,
                "data",
                0,
                false,
                vec![field("payload", "bytes")],
            )],
        );

        assert_eq!(
            metadata
                .get_type_fields_for_id(type_id)
                .expect("type id should see refreshed fields")[0]
                .name,
            "version"
        );
        assert!(
            metadata
                .get_bitfield_for_id(type_id)
                .expect("type id should see refreshed bitfield")
                .network_order
        );
        assert_eq!(
            metadata
                .get_type_variants_for_id(type_id)
                .expect("type id should see refreshed variants")[0]
                .fields[0]
                .name,
            "payload"
        );
    }

    #[test]
    fn type_id_lookup_can_share_fields_across_bound_names() {
        let type_id = TypeId(43);
        let mut metadata = ReflectionMetadata::new();
        let value_info = info("string", "primitive");
        let fields = vec![ReflectionFieldInfo::new(
            0,
            "value",
            "string",
            "primitive",
            "value",
            false,
            value_info,
        )];

        metadata.insert_type_info_for_id(type_id, info("models.Box[string]", "struct"));
        metadata.insert_type_fields("models.Box[string]", fields);
        metadata.bind_type_name("box_alias", type_id);

        assert_eq!(
            metadata
                .get_type_fields("box_alias")
                .expect("bound alias should see canonical fields")[0]
                .name,
            "value"
        );
    }

    #[test]
    fn type_id_owner_metadata_insertion_shares_later_bound_names() {
        let type_id = TypeId(44);
        let mut metadata = ReflectionMetadata::new();

        metadata.insert_type_fields_for_id(
            type_id,
            "models.Packet",
            vec![field("version", "int64")],
        );
        metadata.insert_bitfield_for_id(
            type_id,
            "models.Packet",
            ReflectionBitfieldInfo::new(
                true,
                vec![ReflectionBitfieldFieldInfo::new(
                    0,
                    "version",
                    "bits",
                    4,
                    info("int64", "primitive"),
                    None,
                )],
            ),
        );
        metadata.insert_type_variants_for_id(
            type_id,
            "models.Packet",
            vec![ReflectionVariantInfo::new(
                0,
                "data",
                0,
                false,
                vec![field("payload", "bytes")],
            )],
        );

        metadata.bind_type_name("PacketAlias", type_id);

        assert_eq!(
            metadata
                .get_type_fields("PacketAlias")
                .expect("alias should see id-keyed fields")[0]
                .name,
            "version"
        );
        assert!(
            metadata
                .get_bitfield("PacketAlias")
                .expect("alias should see id-keyed bitfield")
                .network_order
        );
        assert_eq!(
            metadata
                .get_type_variants("PacketAlias")
                .expect("alias should see id-keyed variants")[0]
                .fields[0]
                .name,
            "payload"
        );
    }

    #[test]
    fn type_name_binding_promotes_existing_string_metadata_to_type_id() {
        let type_id = TypeId(46);
        let mut metadata = ReflectionMetadata::new();

        metadata.insert_type_info(info("models.Packet", "struct"));
        metadata.insert_type_fields("models.Packet", vec![field("version", "int64")]);
        metadata.insert_bitfield(
            "models.Packet",
            ReflectionBitfieldInfo::new(
                true,
                vec![ReflectionBitfieldFieldInfo::new(
                    0,
                    "version",
                    "bits",
                    4,
                    info("int64", "primitive"),
                    None,
                )],
            ),
        );
        metadata.insert_type_variants(
            "models.Packet",
            vec![ReflectionVariantInfo::new(
                0,
                "data",
                0,
                false,
                vec![field("payload", "bytes")],
            )],
        );

        metadata.bind_type_name("models.Packet", type_id);
        metadata.bind_type_name("PacketAlias", type_id);
        let alias_type_id = metadata
            .type_id_for_name("PacketAlias")
            .expect("alias should be bound to a type id");
        assert_eq!(alias_type_id, type_id);

        assert_eq!(
            metadata
                .get_type_info("PacketAlias")
                .expect("alias should see promoted type info")
                .kind,
            "struct"
        );
        assert_eq!(
            metadata
                .get_type_info_for_id(alias_type_id)
                .expect("type id should see promoted type info")
                .kind,
            "struct"
        );
        assert_eq!(
            metadata
                .get_type_fields("PacketAlias")
                .expect("alias should see promoted fields")[0]
                .name,
            "version"
        );
        assert_eq!(
            metadata
                .get_type_fields_for_id(alias_type_id)
                .expect("type id should see promoted fields")[0]
                .name,
            "version"
        );
        assert!(
            metadata
                .get_bitfield("PacketAlias")
                .expect("alias should see promoted bitfield")
                .network_order
        );
        assert!(
            metadata
                .get_bitfield_for_id(alias_type_id)
                .expect("type id should see promoted bitfield")
                .network_order
        );
        assert_eq!(
            metadata
                .get_type_variants("PacketAlias")
                .expect("alias should see promoted variants")[0]
                .fields[0]
                .name,
            "payload"
        );
        assert_eq!(
            metadata
                .get_type_variants_for_id(alias_type_id)
                .expect("type id should see promoted variants")[0]
                .fields[0]
                .name,
            "payload"
        );
    }
}
