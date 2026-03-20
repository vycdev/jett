use std::collections::HashMap;

use crate::defs::{EnumDef, EnumId, StructDef, StructId};
use crate::types::{Type, TypeId};

/// Type interner that deduplicates types and provides stable [`TypeId`] handles.
///
/// All primitive types are pre-registered at construction time and have
/// constant IDs that never change.
#[derive(Debug)]
pub struct TypeInterner {
    /// Map from type to its assigned TypeId (for deduplication).
    map: HashMap<Type, TypeId>,
    /// All interned types, indexed by TypeId.
    types: Vec<Type>,
    /// User-defined struct definitions.
    structs: Vec<StructDef>,
    /// User-defined enum definitions.
    enums: Vec<EnumDef>,
}

// Constant TypeId values for every primitive type.
// The order here must match the order of pre-registration in `new()`.
impl TypeInterner {
    pub const INT8: TypeId = TypeId(0);
    pub const INT16: TypeId = TypeId(1);
    pub const INT32: TypeId = TypeId(2);
    pub const INT64: TypeId = TypeId(3);
    pub const UINT8: TypeId = TypeId(4);
    pub const UINT16: TypeId = TypeId(5);
    pub const UINT32: TypeId = TypeId(6);
    pub const UINT64: TypeId = TypeId(7);
    pub const FLOAT32: TypeId = TypeId(8);
    pub const FLOAT64: TypeId = TypeId(9);
    pub const STRING: TypeId = TypeId(10);
    pub const BOOL: TypeId = TypeId(11);
    pub const BYTES: TypeId = TypeId(12);
    pub const NOTHING: TypeId = TypeId(13);
    pub const ERROR: TypeId = TypeId(14);
}

impl TypeInterner {
    /// Creates a new interner with all primitive types pre-registered.
    pub fn new() -> Self {
        let primitives = vec![
            Type::Int8,    // 0
            Type::Int16,   // 1
            Type::Int32,   // 2
            Type::Int64,   // 3
            Type::Uint8,   // 4
            Type::Uint16,  // 5
            Type::Uint32,  // 6
            Type::Uint64,  // 7
            Type::Float32, // 8
            Type::Float64, // 9
            Type::String,  // 10
            Type::Bool,    // 11
            Type::Bytes,   // 12
            Type::Nothing, // 13
            Type::Error,   // 14
        ];

        let mut map = HashMap::with_capacity(primitives.len());
        for (i, ty) in primitives.iter().enumerate() {
            map.insert(ty.clone(), TypeId(i as u32));
        }

        Self {
            types: primitives,
            map,
            structs: Vec::new(),
            enums: Vec::new(),
        }
    }

    /// Intern a type, returning an existing [`TypeId`] if the type was already
    /// interned, or creating a new one.
    pub fn intern(&mut self, ty: Type) -> TypeId {
        if let Some(&id) = self.map.get(&ty) {
            return id;
        }
        let id = TypeId(self.types.len() as u32);
        self.map.insert(ty.clone(), id);
        self.types.push(ty);
        id
    }

    /// Look up a type by its [`TypeId`].
    ///
    /// # Panics
    ///
    /// Panics if the ID was not produced by this interner.
    pub fn resolve(&self, id: TypeId) -> &Type {
        &self.types[id.0 as usize]
    }

    /// Register a new struct definition and return its [`StructId`].
    pub fn add_struct(&mut self, def: StructDef) -> StructId {
        let id = StructId(self.structs.len() as u32);
        self.structs.push(def);
        id
    }

    /// Look up a struct definition by its [`StructId`].
    ///
    /// # Panics
    ///
    /// Panics if the ID was not produced by this interner.
    pub fn resolve_struct(&self, id: StructId) -> &StructDef {
        &self.structs[id.0 as usize]
    }

    /// Register a new enum definition and return its [`EnumId`].
    pub fn add_enum(&mut self, def: EnumDef) -> EnumId {
        let id = EnumId(self.enums.len() as u32);
        self.enums.push(def);
        id
    }

    /// Look up an enum definition by its [`EnumId`].
    ///
    /// # Panics
    ///
    /// Panics if the ID was not produced by this interner.
    pub fn resolve_enum(&self, id: EnumId) -> &EnumDef {
        &self.enums[id.0 as usize]
    }

    /// Returns the total number of interned types.
    pub fn len(&self) -> usize {
        self.types.len()
    }

    /// Returns true if no types have been interned (always false after `new()`).
    pub fn is_empty(&self) -> bool {
        self.types.is_empty()
    }
}

impl Default for TypeInterner {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::defs::VariantDef;

    // -- Primitive constant IDs -----------------------------------------------

    #[test]
    fn primitive_type_ids_are_stable() {
        let interner = TypeInterner::new();
        assert_eq!(*interner.resolve(TypeInterner::INT8), Type::Int8);
        assert_eq!(*interner.resolve(TypeInterner::INT16), Type::Int16);
        assert_eq!(*interner.resolve(TypeInterner::INT32), Type::Int32);
        assert_eq!(*interner.resolve(TypeInterner::INT64), Type::Int64);
        assert_eq!(*interner.resolve(TypeInterner::UINT8), Type::Uint8);
        assert_eq!(*interner.resolve(TypeInterner::UINT16), Type::Uint16);
        assert_eq!(*interner.resolve(TypeInterner::UINT32), Type::Uint32);
        assert_eq!(*interner.resolve(TypeInterner::UINT64), Type::Uint64);
        assert_eq!(*interner.resolve(TypeInterner::FLOAT32), Type::Float32);
        assert_eq!(*interner.resolve(TypeInterner::FLOAT64), Type::Float64);
        assert_eq!(*interner.resolve(TypeInterner::STRING), Type::String);
        assert_eq!(*interner.resolve(TypeInterner::BOOL), Type::Bool);
        assert_eq!(*interner.resolve(TypeInterner::BYTES), Type::Bytes);
        assert_eq!(*interner.resolve(TypeInterner::NOTHING), Type::Nothing);
        assert_eq!(*interner.resolve(TypeInterner::ERROR), Type::Error);
    }

    // -- Basic interning ------------------------------------------------------

    #[test]
    fn intern_same_type_returns_same_id() {
        let mut interner = TypeInterner::new();
        let a = interner.intern(Type::List(TypeInterner::INT64));
        let b = interner.intern(Type::List(TypeInterner::INT64));
        assert_eq!(a, b);
    }

    #[test]
    fn intern_different_types_returns_different_ids() {
        let mut interner = TypeInterner::new();
        let a = interner.intern(Type::List(TypeInterner::INT64));
        let b = interner.intern(Type::List(TypeInterner::STRING));
        assert_ne!(a, b);
    }

    #[test]
    fn intern_primitive_returns_constant_id() {
        let mut interner = TypeInterner::new();
        let id = interner.intern(Type::Int64);
        assert_eq!(id, TypeInterner::INT64);
    }

    // -- Generic type interning -----------------------------------------------

    #[test]
    fn generic_list_interning() {
        let mut interner = TypeInterner::new();
        let list_int = interner.intern(Type::List(TypeInterner::INT64));
        let list_int_again = interner.intern(Type::List(TypeInterner::INT64));
        assert_eq!(list_int, list_int_again);

        // Resolve and verify
        assert_eq!(
            *interner.resolve(list_int),
            Type::List(TypeInterner::INT64)
        );
    }

    #[test]
    fn generic_map_interning() {
        let mut interner = TypeInterner::new();
        let map_type = interner.intern(Type::Map(TypeInterner::STRING, TypeInterner::INT64));
        let map_type_again = interner.intern(Type::Map(TypeInterner::STRING, TypeInterner::INT64));
        assert_eq!(map_type, map_type_again);

        let map_different = interner.intern(Type::Map(TypeInterner::INT64, TypeInterner::STRING));
        assert_ne!(map_type, map_different);
    }

    #[test]
    fn generic_set_interning() {
        let mut interner = TypeInterner::new();
        let set_a = interner.intern(Type::Set(TypeInterner::STRING));
        let set_b = interner.intern(Type::Set(TypeInterner::STRING));
        assert_eq!(set_a, set_b);
    }

    #[test]
    fn generic_optional_interning() {
        let mut interner = TypeInterner::new();
        let opt_a = interner.intern(Type::Optional(TypeInterner::INT64));
        let opt_b = interner.intern(Type::Optional(TypeInterner::INT64));
        assert_eq!(opt_a, opt_b);

        let opt_str = interner.intern(Type::Optional(TypeInterner::STRING));
        assert_ne!(opt_a, opt_str);
    }

    #[test]
    fn generic_result_interning() {
        let mut interner = TypeInterner::new();
        let res = interner.intern(Type::Result(TypeInterner::INT64, TypeInterner::STRING));
        let res_again = interner.intern(Type::Result(TypeInterner::INT64, TypeInterner::STRING));
        assert_eq!(res, res_again);
    }

    #[test]
    fn generic_secret_interning() {
        let mut interner = TypeInterner::new();
        let sec = interner.intern(Type::Secret(TypeInterner::STRING));
        let sec_again = interner.intern(Type::Secret(TypeInterner::STRING));
        assert_eq!(sec, sec_again);
    }

    #[test]
    fn nested_generic_interning() {
        let mut interner = TypeInterner::new();
        // list[optional[int64]]
        let opt_int = interner.intern(Type::Optional(TypeInterner::INT64));
        let list_opt = interner.intern(Type::List(opt_int));
        let list_opt_again = interner.intern(Type::List(opt_int));
        assert_eq!(list_opt, list_opt_again);
    }

    #[test]
    fn function_type_interning() {
        let mut interner = TypeInterner::new();
        let fn_type = interner.intern(Type::Function {
            params: vec![TypeInterner::INT64, TypeInterner::STRING],
            return_type: TypeInterner::BOOL,
        });
        let fn_type_again = interner.intern(Type::Function {
            params: vec![TypeInterner::INT64, TypeInterner::STRING],
            return_type: TypeInterner::BOOL,
        });
        assert_eq!(fn_type, fn_type_again);

        // Different param order -> different type
        let fn_different = interner.intern(Type::Function {
            params: vec![TypeInterner::STRING, TypeInterner::INT64],
            return_type: TypeInterner::BOOL,
        });
        assert_ne!(fn_type, fn_different);
    }

    // -- Struct and enum definitions ------------------------------------------

    #[test]
    fn struct_definition_storage() {
        let mut interner = TypeInterner::new();

        let struct_def = StructDef {
            name: "Point".to_string(),
            fields: vec![
                ("x".to_string(), TypeInterner::FLOAT64),
                ("y".to_string(), TypeInterner::FLOAT64),
            ],
            methods: vec![],
        };

        let sid = interner.add_struct(struct_def);
        let resolved = interner.resolve_struct(sid);
        assert_eq!(resolved.name, "Point");
        assert_eq!(resolved.fields.len(), 2);
        assert_eq!(resolved.fields[0].0, "x");
        assert_eq!(resolved.fields[0].1, TypeInterner::FLOAT64);

        // Intern the struct type and check round-trip
        let struct_type_id = interner.intern(Type::Struct(sid));
        assert_eq!(*interner.resolve(struct_type_id), Type::Struct(sid));
    }

    #[test]
    fn enum_definition_storage() {
        let mut interner = TypeInterner::new();

        let enum_def = EnumDef {
            name: "Color".to_string(),
            variants: vec![
                VariantDef {
                    name: "Red".to_string(),
                    fields: vec![],
                },
                VariantDef {
                    name: "Green".to_string(),
                    fields: vec![],
                },
                VariantDef {
                    name: "Custom".to_string(),
                    fields: vec![
                        ("r".to_string(), TypeInterner::UINT8),
                        ("g".to_string(), TypeInterner::UINT8),
                        ("b".to_string(), TypeInterner::UINT8),
                    ],
                },
            ],
        };

        let eid = interner.add_enum(enum_def);
        let resolved = interner.resolve_enum(eid);
        assert_eq!(resolved.name, "Color");
        assert_eq!(resolved.variants.len(), 3);
        assert_eq!(resolved.variants[2].name, "Custom");
        assert_eq!(resolved.variants[2].fields.len(), 3);

        // Intern the enum type and check round-trip
        let enum_type_id = interner.intern(Type::Enum(eid));
        assert_eq!(*interner.resolve(enum_type_id), Type::Enum(eid));
    }

    // -- Type resolution ------------------------------------------------------

    #[test]
    fn resolve_returns_correct_type() {
        let mut interner = TypeInterner::new();
        let list_str = interner.intern(Type::List(TypeInterner::STRING));
        match interner.resolve(list_str) {
            Type::List(inner) => assert_eq!(*inner, TypeInterner::STRING),
            other => panic!("expected List, got {:?}", other),
        }
    }

    #[test]
    fn interner_len_includes_primitives() {
        let interner = TypeInterner::new();
        // 15 primitives: int8..uint64 (8) + float32/64 (2) + string, bool, bytes, nothing, error (5)
        assert_eq!(interner.len(), 15);
    }

    #[test]
    fn interner_len_grows_with_new_types() {
        let mut interner = TypeInterner::new();
        let before = interner.len();
        interner.intern(Type::List(TypeInterner::INT64));
        assert_eq!(interner.len(), before + 1);
        // Re-interning doesn't grow
        interner.intern(Type::List(TypeInterner::INT64));
        assert_eq!(interner.len(), before + 1);
    }
}
