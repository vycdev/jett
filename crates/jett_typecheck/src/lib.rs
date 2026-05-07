// Type checking for the Jett compiler.

pub mod capability;
pub mod checker;
pub mod errors;
pub mod ownership;

pub use checker::{CheckResult, check};
