# Uint64 Runtime Value Model

## Context

Jett exposes `uint64` as a primitive type. The interpreter now has a real
`Value::Uint64(u64)` carrier for values produced by `uint64.from_string`,
property generation, and reflected JSON parsing. This lets JSON decode and
serialize the full unsigned range through the stdlib path:

- `json.parse[uint64]("18446744073709551615")` succeeds.
- `json.parse[uint64]("18446744073709551616")` reports a range error.
- property generation for `uint64` can include values above `i64::MAX`.

The remaining ceiling is source literal parsing: integer literals are still
stored in the AST as `i64`, so a source literal such as
`18446744073709551615` is rejected before the checker can use the expected
`uint64` type.

## Why Not Fix In JSON Alone

The self-hosted JSON decoder needs a runtime value of type `uint64`. The first
staged fix added that runtime carrier and ordinary conversion builtins instead
of special-casing JSON. This keeps JSON aligned with the rest of the language:
`json.parse[uint64]` uses `uint64.from_string`, and JSON serialization uses
`string.from_uint64`.

Do not solve the remaining source-literal limit inside JSON. Literal widening
belongs in the parser/AST/typechecker pipeline so all language features see the
same numeric model.

## Options

1. **Add `Value::Uint64(u64)`.**

   Status: partially implemented. The carrier exists, display/equality,
   conversion builtins, property generation/shrinking, core uint64 arithmetic
   and comparisons, and JSON decode/encode have been updated. Source literals
   above `i64::MAX` and bitfield paths that assume signed runtime integers are
   still open.

2. **Use one integer carrier with explicit signedness metadata.**

   A value could store a wider integer plus the checked primitive type. This is
   more invasive because many runtime values are currently type-erased after
   checking.

3. **Use arbitrary precision integers.**

   This simplifies overflow during intermediate operations but is a bigger
   semantic choice: Jett's primitive integers are explicitly fixed-width, so the
   language still needs clear overflow and conversion rules.

4. **Keep the `i64` ceiling for now.**

   This was the previous staging choice. It no longer applies to JSON parsing
   or `uint64.from_string`, but it still applies to oversized source literals.

## Recommendation

Keep the JSON path on ordinary unsigned runtime values. The next real step is
not more JSON work; it is literal widening:

- parse integer literal text into a representation that can hold `u64::MAX`,
- let expected-type checking choose signed or unsigned interpretation,
- keep overflow diagnostics precise for each fixed-width primitive,
- update `integer_literal_overflow.jett` and add pass/fail fixtures around
  `u64::MAX`.
