# Math Numeric Overload Boundary

## Status

Decided for the current compiler stage.

## Current Boundary

The interpreter currently accepts both `int64` and `float64` values for a few
numeric math helpers:

- `math.abs`
- `math.min`
- `math.max`

The typechecker does not expose a general overload model. These helpers are
instead checked as a closed compiler-owned numeric intrinsic table:

- `math.abs(int64) returns int64`
- `math.abs(float64) returns float64`
- `math.min(int64, int64) returns int64`
- `math.min(float64, float64) returns float64`
- `math.max(int64, int64) returns int64`
- `math.max(float64, float64) returns float64`

Mixed numeric arguments are rejected rather than coerced. Secret arguments lift
through the pure intrinsic and taint the return type, matching ordinary pure
builtin calls.

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
