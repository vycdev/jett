use crate::defs::{EnumId, InterfaceId, StructId};

/// A unique handle to an interned type. Cheap to copy and compare.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TypeId(pub(crate) u32);

impl TypeId {
    /// Returns the raw index. Useful for serialization/debugging.
    pub fn index(self) -> u32 {
        self.0
    }
}

/// The core type representation in the Jett compiler.
///
/// Primitive types are simple unit variants; generic and user-defined types
/// carry [`TypeId`] handles back into the interner so the type graph stays
/// flat and easily comparable.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Type {
    // -- Integer types -------------------------------------------------------
    Int8,
    Int16,
    Int32,
    Int64,
    Uint8,
    Uint16,
    Uint32,
    Uint64,

    // -- Floating-point types ------------------------------------------------
    Float32,
    Float64,

    // -- Other primitives ----------------------------------------------------
    String,
    Bool,
    Bytes,
    Nothing,

    // -- Generic built-in types ----------------------------------------------
    /// `list[T]`
    List(TypeId),
    /// `map[K, V]`
    Map(TypeId, TypeId),
    /// `set[T]`
    Set(TypeId),
    /// `optional[T]`
    Optional(TypeId),
    /// `result[T, E]`
    Result(TypeId, TypeId),
    /// `secret[T]`
    Secret(TypeId),

    // -- User-defined types --------------------------------------------------
    /// A user-defined struct.
    Struct(StructId),
    /// A user-defined enum.
    Enum(EnumId),
    /// A user-defined interface.
    Interface(InterfaceId),

    // -- Function type -------------------------------------------------------
    /// `function(T, U) returns V`
    Function {
        params: Vec<TypeId>,
        return_type: TypeId,
    },

    // -- Refinement types ----------------------------------------------------
    /// A refinement type: a base type with a constraint expression.
    /// The `String` holds the name of the type alias (e.g. "Port").
    /// The `TypeId` is the base type.  The constraint is stored externally
    /// (in the AST or a side table) since `Type` must be `Hash + Eq`.
    Refinement {
        name: String,
        base: TypeId,
    },

    // -- Error sentinel ------------------------------------------------------
    /// Placeholder for type-check errors so compilation can continue.
    Error,
}
