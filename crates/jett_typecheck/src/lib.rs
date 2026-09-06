// Type checking for the Jett compiler.

pub mod capability;
pub mod checker;
pub mod complexity;
pub mod errors;
pub mod ownership;

pub use checker::{
    CheckOptions, CheckResult, CheckedCallArgumentOrder, CheckedGenericCall,
    CheckedGenericFunctionInstantiation, CheckedMethodCall, CheckedMethodDefinition,
    CheckedStructConstruction, check, check_with_options,
};
