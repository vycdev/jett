# Math Numeric Overload Boundary

## Status

Open.

## Current Boundary

The interpreter currently accepts both `int64` and `float64` values for a few
numeric math helpers:

- `math.abs`
- `math.min`
- `math.max`

The typechecker does not have an overload model yet, and these helpers are used
with both integer and float expectations in existing fixtures. Treating them as
only `int64` or only `float64` would silently choose a language rule before the
numeric generic story is settled.

## Architectural Preference

For an LLM-oriented language, the eventual rule should keep one canonical
spelling at the call site and avoid return-type guessing. Good candidates are:

- constrained generic builtins such as `math.abs[T implements Numeric](value:
  T) returns T`, once builtin generic constraints can be expressed cleanly;
- explicit typed spellings such as `math.abs_int64` and `math.abs_float64`, if
  overload-like generics remain too implicit;
- a small compiler-owned numeric builtin table that can dispatch on checked
  argument types while still reporting exact signatures.

Until that choice is made, the checker should keep adding exact signatures for
monomorphic math helpers and leave these overloaded helpers as an intentional
boundary rather than widening `ERROR` wildcard use further.
