// Type checking for the Jett compiler.

pub mod checker;
pub mod errors;
pub mod ownership;

pub use checker::{check, CheckResult};
