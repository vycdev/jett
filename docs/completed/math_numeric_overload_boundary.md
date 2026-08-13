# Math Numeric Overload Boundary

## Status

Completed for the current compiler stage. Broader generic numeric constraints
remain deferred until the language has a general model for them.

## Current Boundary

The source-owned public API accepts both `int64` and `float64` values for three
numeric math helpers:

- `math.abs`
- `math.min`
- `math.max`

The typechecker does not expose a general overload model. These source facades
are checked by a closed compiler-owned numeric call-policy table:

- `math.abs(int64) returns int64`
- `math.abs(float64) returns float64`
- `math.min(int64, int64) returns int64`
- `math.min(float64, float64) returns float64`
- `math.max(int64, int64) returns int64`
- `math.max(float64, float64) returns float64`

Mixed numeric arguments are rejected rather than coerced. Secret arguments lift
through the pure call and taint the return type, matching ordinary pure calls.
Execution resolves through the declarations in `stdlib/math.jett`; only private
trusted kernels perform the primitive numeric operation.

## Architectural Preference

For an LLM-oriented language, the rule should keep one canonical spelling at
the call site and avoid return-type guessing. The current compiler-owned table
is intentionally small and exact. Future candidates remain:

- constrained generic builtins such as `math.abs[T implements Numeric](value:
  T) returns T`, once builtin generic constraints can be expressed cleanly;
- explicit typed spellings such as `math.abs_int64` and `math.abs_float64`, if
  overload-like generics remain too implicit;
- a small compiler-owned numeric builtin table that can dispatch on checked
  argument types while still reporting exact signatures.

Until a broader numeric constraint model exists, do not generalize this into
user-defined overloads or implicit numeric coercion. Keep adding exact
signatures for monomorphic math helpers, and keep this table limited to helpers
whose runtime behavior is already fixed and homogeneous by argument type.
