// Type checking for the Jett compiler.

pub mod capability;
pub mod checker;
pub mod complexity;
pub mod errors;
pub mod ownership;

pub use checker::{
    CheckOptions, CheckResult, CheckedBodyFacts, CheckedCallArgumentOrder,
    CheckedComptimeTypeBinding, CheckedComptimeTypeSelection, CheckedGenericCall,
    CheckedGenericFunctionInstantiation, CheckedGenericSpecialization, CheckedMethodCall,
    CheckedMethodDefinition, CheckedMethodValue, CheckedStaticSelection, CheckedStructConstruction,
    check, check_with_options,
};
