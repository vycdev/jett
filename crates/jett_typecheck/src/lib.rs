// Type checking for the Jett compiler.

pub mod actor_messages;
pub use actor_messages::{
    CheckedActorBindingRole, CheckedActorHandler, CheckedActorHandlerBinding,
    CheckedActorHandlerExit, CheckedActorHandlerExitKind, CheckedActorHandlerTarget,
    CheckedActorMessageCall, CheckedActorMessageKind, CheckedActorMessageRoot,
};
pub mod assignment_places;
pub use assignment_places::{
    CheckedAssignmentPlace, CheckedAssignmentPlaceError, CheckedFieldOwner, CheckedFieldProjection,
    CheckedFieldUpdateResourceRequirement, checked_field_owner, field_update_resource_requirement,
    validate_assignment_place_shape,
};
pub mod caller_ownership;
pub mod capability;
pub mod checker;
pub mod complexity;
pub mod errors;
pub mod ownership;
mod reflected_return;
pub mod resource_hooks;
mod resource_program;

pub use reflected_return::{
    CheckedReflectedCallablePlan, ProducedRefinementRequirement, ProducedRefinementRequirementPlan,
    ProducedRefinementUnsupported, produced_refinement_requirement,
};

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
