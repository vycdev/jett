# Rust pilot reference v0.2

## Type-driven development in Rust

Start from the required function type. Use precise local and helper types,
borrowing, and control-flow narrowing to make invalid states difficult to
express; make every branch agree with the declared return type. Do not bypass
the type system with unchecked casts, unnecessary cloning, or panic-based
shortcuts.

Return one Rust 2024 source file containing the requested `pub fn`. Inputs use
`i64`, slices use `&[i64]`, and fixed outputs may use `&'static str`. Use
`if`/`else`, `for`, `while`, `%`, `&&`, and `||`. Mutable locals require
`let mut`. Do not add `main`, perform I/O, add tests, or change the requested
function signature.
