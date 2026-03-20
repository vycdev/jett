//! Capability-based purity enforcement.
//!
//! A function is **pure** if none of its parameters have a capability type.
//! Pure functions cannot call impure functions.  Verify blocks can only call
//! pure functions.
//!
//! The capability types are:
//!   Stdout, Stderr, Stdin, Filesystem, Network, Clock, Random, Process, Environment

use jett_parser::ast::TypeExpr;

/// The set of built-in capability type names.
const CAPABILITY_TYPES: &[&str] = &[
    "Stdout",
    "Stderr",
    "Stdin",
    "Filesystem",
    "Network",
    "Clock",
    "Random",
    "Process",
    "Environment",
];

/// Returns `true` if `name` is one of the built-in capability types.
pub fn is_capability_type(name: &str) -> bool {
    CAPABILITY_TYPES.contains(&name)
}

/// Returns `true` if the given [`TypeExpr`] refers to a capability type.
/// Handles `view Stdout` as well as plain `Stdout`.
pub fn type_expr_is_capability(ty: &TypeExpr) -> bool {
    match ty {
        TypeExpr::Named(ident) => is_capability_type(&ident.name),
        TypeExpr::View(inner, _) => type_expr_is_capability(inner),
        TypeExpr::Generic(_, _, _) => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capability_names_recognised() {
        assert!(is_capability_type("Stdout"));
        assert!(is_capability_type("Filesystem"));
        assert!(is_capability_type("Environment"));
        assert!(!is_capability_type("int64"));
        assert!(!is_capability_type("string"));
        assert!(!is_capability_type("User"));
    }
}
