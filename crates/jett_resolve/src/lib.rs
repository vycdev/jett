// Name resolution for the Jett compiler.

pub mod errors;
pub mod resolver;
pub mod resource_hooks;
pub mod scope;

pub use resolver::{ResolveResult, resolve, resolve_with_resource_kernels};
pub use resource_hooks::{
    ResolvedResourceKernel, ResolvedResourceKernels, ResourceKernelError, ResourceKernelSpec,
    validate_resource_kernels,
};
pub use scope::{DefId, DefInfo, DefKind, DefVisibility, Scope, ScopeId, ScopeTable};

#[cfg(test)]
mod resource_hook_tests;
