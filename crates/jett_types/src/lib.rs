// Type representations and interning for the Jett compiler.

mod defs;
mod interner;
mod types;

pub use defs::{EnumDef, EnumId, FunctionSig, StructDef, StructId, VariantDef};
pub use interner::TypeInterner;
pub use types::{Type, TypeId};
