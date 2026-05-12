// Type representations and interning for the Jett compiler.

mod defs;
mod interner;
mod reflection;
mod types;

pub use defs::{
    ActorDef, ActorId, ActorMessageDef, BitfieldDef, BitfieldFieldDef, BitfieldFieldKind,
    BitfieldId, EnumDef, EnumId, FunctionSig, InterfaceDef, InterfaceId, StructDef, StructId,
    VariantDef,
};
pub use interner::TypeInterner;
pub use reflection::{
    ReflectionBitfieldFieldInfo, ReflectionBitfieldInfo, ReflectionFieldInfo, ReflectionMetadata,
    ReflectionTypeInfo, ReflectionVariantInfo,
};
pub use types::{Type, TypeId};
