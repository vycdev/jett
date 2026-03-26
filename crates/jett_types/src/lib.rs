// Type representations and interning for the Jett compiler.

mod defs;
mod interner;
mod types;

pub use defs::{
    ActorDef, ActorId, ActorMessageDef, BitfieldDef, BitfieldFieldDef, BitfieldFieldKind,
    BitfieldId, EnumDef, EnumId, FunctionSig, InterfaceDef, InterfaceId, StructDef, StructId,
    VariantDef,
};
pub use interner::TypeInterner;
pub use types::{Type, TypeId};
