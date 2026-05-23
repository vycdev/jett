# Uint64 Runtime Value Model

## Context

Jett exposes `uint64` as a primitive type. The interpreter now has a real
`Value::Uint64(u64)` carrier for values produced by full-range source literals,
direct local declarations and function parameter/return boundaries annotated as
`uint64`, direct struct constructor fields annotated as `uint64`,
`uint64.from_string`, property generation, and reflected JSON parsing. This lets
ordinary source and JSON decode/serialize the full unsigned range:

- `uint64 value = 18446744073709551615` succeeds.
- `int64 value = 9223372036854775808` is rejected.
- `json.parse[uint64]("18446744073709551615")` succeeds.
- `json.parse[uint64]("18446744073709551616")` reports a range error.
- property generation for `uint64` can include values above `i64::MAX`.

Integer literals are now stored in the AST wide enough for `u64::MAX`, while
enum discriminants remain signed `i64` in this stage. Unannotated 64-bit
bitfield fields now use `uint64` values, while 1..63-bit fields keep the
existing `int64` surface.

## Why Not Fix In JSON Alone

The self-hosted JSON decoder needs a runtime value of type `uint64`. The first
staged fix added that runtime carrier and ordinary conversion builtins instead
of special-casing JSON. This keeps JSON aligned with the rest of the language:
`json.parse[uint64]` uses `uint64.from_string`, and JSON serialization uses
`string.from_uint64`.

Do not solve numeric limits inside JSON alone. Literal widening belongs in the
parser/AST/typechecker pipeline so all language features see the same numeric
model; that path now exists for ordinary `uint64` literals.

## Options

1. **Add `Value::Uint64(u64)`.**

   Status: partially implemented. The carrier exists, display/equality,
   conversion builtins, property generation/shrinking, source literals through
   `u64::MAX`, direct local declarations and named function parameter/return
   boundaries, direct struct constructor fields, core uint64 arithmetic and
   comparisons, JSON decode/encode, and unannotated 64-bit bitfield
   construction/field access/to-bytes/from-bytes have been updated.

2. **Use one integer carrier with explicit signedness metadata.**

   A value could store a wider integer plus the checked primitive type. This is
   more invasive because many runtime values are currently type-erased after
   checking.

3. **Use arbitrary precision integers.**

   This simplifies overflow during intermediate operations but is a bigger
   semantic choice: Jett's primitive integers are explicitly fixed-width, so the
   language still needs clear overflow and conversion rules.

4. **Keep the `i64` ceiling for now.**

   This was the previous staging choice. It no longer applies to source
   literals, JSON parsing, or `uint64.from_string`.

## Recommendation

Keep the JSON path on ordinary unsigned runtime values. The next real step is
not more JSON work; it is finishing the remaining numeric surfaces:

- decide whether bitfield syntax should ever support fields wider than 64 bits
  or stay explicitly tied to fixed-width primitive carriers,
- decide whether the interpreter should receive checked expression type maps so
  assignments, enum payload construction, reflected construction, inline
  function calls, interface dispatch, and other expression-only sites can mirror
  the checked primitive type instead of relying on explicit-boundary
  normalization plus the current value-range fallback,
- keep overflow diagnostics precise for each fixed-width primitive.
