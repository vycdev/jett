// Type checking for the Jett compiler.

pub mod checker;
pub mod errors;

pub use checker::{check, CheckResult};
