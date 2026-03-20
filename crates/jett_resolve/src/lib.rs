// Name resolution for the Jett compiler.

pub mod errors;
pub mod resolver;
pub mod scope;

pub use resolver::{resolve, ResolveResult};
pub use scope::{DefId, DefInfo, DefKind, Scope, ScopeId, ScopeTable};
