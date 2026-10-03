// Type checking for the Jett compiler.

pub mod capability;
pub mod checker;
pub mod complexity;
pub mod errors;
pub mod ownership;
pub mod resource_hooks;

pub use checker::{
    CheckOptions, CheckResult, CheckedBindingMode, CheckedBodyFacts, CheckedCallArgumentOrder,
    CheckedComptimeTypeBinding, CheckedComptimeTypeSelection, CheckedGenericCall,
    CheckedGenericFunctionInstantiation, CheckedGenericSpecialization, CheckedInterfaceCall,
    CheckedMethodCall, CheckedMethodDefinition, CheckedMethodValue, CheckedStaticSelection,
    CheckedStructConstruction, CheckedViewSource, check, check_with_options,
    check_with_resource_kernels,
};

pub use resource_hooks::{CheckedResourceHook, ResourceHookError, validate_resource_hooks};

#[cfg(test)]
mod resource_hook_tests;
