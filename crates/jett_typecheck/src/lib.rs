// Type checking for the Jett compiler.

pub mod caller_ownership;
pub mod capability;
pub mod checker;
pub mod complexity;
pub mod errors;
pub mod ownership;
pub mod resource_hooks;
mod resource_program;

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

pub use resource_program::{CheckedResourceProgram, ResourceProgramError};

pub use caller_ownership::{
    CheckedArgumentOwnership, CheckedBindingFact, CheckedCallOwnership, CheckedCalleeAccess,
    CheckedCallerEffect, CheckedCallerOrigin, CheckedCallerSyntax, CheckedIntrinsicOperandRole,
    CheckedInvocationShape, CheckedInvocationTarget, CheckedOwnershipContext,
    intrinsic_operand_access,
};
