# Uint64 Runtime Value Model

## Context

Jett exposes `uint64` as a primitive type, but the current interpreter stores
all integer runtime values as `Value::Int64(i64)`. That means JSON parse,
property generation, arithmetic, comparisons, bitfield construction, and string
conversion can only exercise `uint64` values up to `i64::MAX`.

The JSON fixture intentionally pins this current ceiling:

- `json.parse[uint64]("9223372036854775807")` succeeds.
- `json.parse[uint64]("9223372036854775808")` reports a range error.
- property generation for `uint64` uses `i64::MAX`.

This is a runtime representation limit, not a JSON parser limit.

## Why Not Fix In JSON Alone

The self-hosted JSON decoder ultimately needs a runtime value of type `uint64`.
Today the only integer value carrier is signed `i64`, and `int64.from_string`
is the only integer string parser available to stdlib JSON code. Accepting the
full `0..18446744073709551615` range would therefore require a value that
cannot currently be represented by the interpreter.

Adding ad hoc JSON-only handling would make JSON more capable than the rest of
the language and would leak a special case into reflection, arithmetic, and
serialization.

## Options

1. **Add `Value::Uint64(u64)`.**

   This is direct and preserves fixed-width primitive identity. It requires
   updating numeric literals, arithmetic, comparisons, casts, interpolation,
   string conversion, property generation/shrinking, JSON decode/encode, and
   any bitfield paths that currently assume `Value::Int64`.

2. **Use one integer carrier with explicit signedness metadata.**

   A value could store a wider integer plus the checked primitive type. This is
   more invasive because many runtime values are currently type-erased after
   checking.

3. **Use arbitrary precision integers.**

   This simplifies overflow during intermediate operations but is a bigger
   semantic choice: Jett's primitive integers are explicitly fixed-width, so the
   language still needs clear overflow and conversion rules.

4. **Keep the `i64` ceiling for now.**

   This matches the current interpreter and keeps JSON honest about the runtime.
   It should remain documented and tested until the numeric value model changes.

## Recommendation

Do not lift the JSON `uint64` ceiling inside `stdlib/json/` by itself. The next
real step is a numeric runtime pass, probably starting with `Value::Uint64(u64)`
because it is the smallest change that keeps `uint64` visibly distinct from
`int64`.

Once that exists:

- add `uint64.from_string` and `string.from_uint64`,
- update property generation to cover values above `i64::MAX`,
- teach JSON number decoding to use the unsigned parser for
  `TypePrimitive.uint64_type`,
- update `json_sized_primitives.jett` to pin the full `uint64` maximum.
