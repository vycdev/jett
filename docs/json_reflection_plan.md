# JSON Reflection Plan

This note tracks the remaining design work for moving JSON serialization and
deserialization out of Rust builtins and into Jett stdlib code.

## Current State

Implemented reflection primitives:

- `type.name[T]()` returns a stable display name.
- `type.kind[T]()` returns a broad kind string.
- `type.kind_tag[T]()` returns a structured `TypeKind` enum tag while keeping
  the string kind API available for compatibility.
- `type.has_secret[T]()` reports whether a type contains secret data.
- `type.info[T]()` returns recursive `TypeInfo` metadata with `type_name`,
  `kind`, `kind_tag`, `has_secret`, and nested `args`.
- `type.arg[T](index)` returns the indexed type argument as `TypeInfo`, and
  direct literal-index calls can be used as trusted `comptime type` binders.
- Alias and refinement `TypeInfo` values expose their base type as the first
  `args` entry, so stdlib code can peel a refinement such as `NonEmpty` back to
  `string`, or an alias such as `NameList` back to `list[string]`.
- `type.fields[T]()` returns ordered `TypeField` metadata for structs and
  bitfields.
- `TypeField` includes `index`, `name`, `type_name`, `kind`, `kind_tag`,
  `serialize_name`, `has_secret`, and `type_info`. For bitfields,
  `serialize_name` currently equals `name`; bitfield field renaming is not
  supported.
- `type.bitfield_fields[T]()` returns ordered `TypeBitfieldField` metadata for
  bitfields, including `shape`, `shape_tag`, `width`, semantic `type_info`, and
  optional enum annotation metadata.
- `type.bitfield_layout[T]()` returns `TypeBitfield` metadata with
  `network_order` and the same ordered `list[TypeBitfieldField]` layout.
- `type.variants[T]()` returns ordered `TypeVariant` metadata for enums.
  `TypeVariant` includes `index`, `name`, `discriminant`, `has_secret`, and
  payload `fields` as `list[TypeField]`.
- `type.variant_value[T](view value)` returns the active `TypeVariant`
  metadata for an enum value.
- `type.variant_field_value[T, U](view value, view field)` reads an active
  enum payload field by metadata after checking that `U` matches the reflected
  field type.
- `type.field_value[T, U](view value, view field)` reads a field by metadata
  after checking that the metadata belongs to `T` and that `U` matches the
  reflected field type.
- `type.construct_start[T]()`, `type.construct_put[T, Field](builder, field,
  value)`, and `type.construct_finish[T](builder)` provide an opaque
  construction builder for turning checked struct and bitfield field values back
  into `T`.
- `type.construct_variant_start[T](variant)` starts the same opaque builder for
  a checked enum `TypeVariant`; `construct_put` then accepts variant-local
  payload fields and `construct_finish` produces the enum value.
- `comptime type Name = type.info[T]():` binds a direct reflected root as a
  scoped type, and `comptime type Field = field.type_info:` works inside direct
  `for field in type.fields[T]():` loops and direct active enum payload loops
  over `type.variant_value[T](view value).fields`, plus static enum payload
  loops nested under `for variant in type.variants[T]():`.
- Trusted `TypeInfo.args` loops can bind nested type arguments, enabling
  recursive dispatch for wrappers such as `list[T]` and `optional[T]` without
  trusting user-constructed metadata.

JSON serialization is still Rust-backed, but it now consumes the same field
metadata exposed to Jett code. The active tests cover nested structs, bitfields,
lists, sets, maps with string keys, optionals, results, `serialize` names,
public secret omission, and valid JSON string escaping for control characters.

There is also a `.jett` public serializer prototype now staged in
`stdlib/json_reflection.jett` as
`json_serialize_public_reflected[T](view value)`. It recursively handles
primitives, structs, lists, sets, `map[string, V]`, optionals, result ok/fail
objects, alias/refinement base serialization, bitfields, and enums using the
same string/object shape as the Rust-backed JSON bridge. It relies on reflection
primitives, trusted `comptime type` binding, and `type.arg[T](index)` for wrapper
element/value types. The prototype also covers basic string escaping for quotes,
backslashes, common named control characters, and all other ASCII control bytes
as `\u00xx` sequences while preserving non-ASCII UTF-8.
`tests/run_pass/json_reflection_nested_serializer.jett` exercises that stdlib
function against nested user-defined types. This is still not the public
`json.serialize_public` replacement; namespace-qualified user functions and API
policy still need to land first.

There are also `.jett` decoder prototypes:
`tests/run_pass/json_reflection_flat_decoder.jett` covers the first
flat-struct path, and `stdlib/json_reflection.jett` now stages the nested
decoder as `json_decode_reflected[T](raw)`, plus
`json_parse_reflected[T](raw: string)` as a thin staged wrapper over
`json.parse_raw` and the reflected decoder. It recursively handles core
primitives (`string`, `int64`, `float64`, `bool`,
`bytes` as hex strings, and JSON null as `nothing`), nested structs, lists,
sets, maps with string keys, optionals, results, aliases/refinements, and
`serialize_name` by walking raw `JsonValue` and finishing structs, bitfields,
and enums with `TypeConstruction`.
It treats absent optional fields as `none`, validates top-level refinements
including refinements over generic shapes, and decodes enum-annotated bitfields
through the ordinary enum branch plus bitfield finish validation. It also
decodes `secret[T]` by parsing the inner `T` and assigning the result into the
secret wrapper, matching the Rust-backed `json.parse[T]` input policy while
keeping output exposure blocked by the ordinary secret type rules.
`tests/run_pass/json_reflection_nested_decoder.jett` exercises the decoder
against nested user-defined types, while
`tests/run_pass/json_reflection_parse_wrapper.jett` checks the raw-string
wrapper path through the qualified staging name
`json.json_parse_reflected[T](raw)`. See
`docs/json_public_parse_policy.md` for the current
recommendation: avoid a public parse API for now, and keep public JSON as an
output projection unless a future audit-oriented API rejects secret-containing
targets outright.

`json.parse[T](raw)` has a Rust-backed bridge: it requires one type argument,
accepts a string, returns `result[T, string]`, and supports core primitives,
structs with `serialize` names, lists, sets, `map[string, V]`, optionals,
results, `secret[T]` wrappers, enums using the serializer's string/object
payload shape, bitfields from JSON objects with width and enum-annotation
validation, generic structs, aliases, and refinement validation, including
refinements over generic shapes such as `list[string]`. The long-term goal is
still to replace this bridge with stdlib code; struct construction is now
available through `TypeConstruction`, bitfield construction is available through
the same builder, and enum construction is available through
`type.construct_variant_start`; the final syntax remains future work.
Moving the prototypes into an actual public `json.*` API now mostly needs the
handoff from the hardcoded builtin bridge; see
`docs/stdlib_json_extraction_plan.md`.

`json.parse_raw(raw)` now exposes an opaque `JsonValue` tree with explicit
accessors for kind checks, object fields, array indexes, scalar casts, object
keys, array length, null checks, and compact raw serialization. This gives a
future `.jett` decoder a safe raw input representation without adding an
unchecked `any` lane. See `docs/json_raw_value_design.md`.

## Design Pressure

A real `stdlib/json.jett` implementation needs more than shape metadata. It
needs ways to turn reflected type metadata into typed recursive work and then
construct final values from decoded fields.

The earliest version of that problem could only handle a small fixed set of
known field types:

```jett
if field.type_name == "string":
    string value = type.field_value[T, string](view item, view field)
```

That was enough for a flat proof of concept, but not enough for arbitrary nested
structs. Trusted `comptime type` binding now closes much of the typed-dispatch
gap for compiler-owned reflection flows. The remaining pressure is to harden
the `.jett` decoder prototype, identify less-direct metadata flows that still
need language support, and keep format policy out of type construction.

## Open Decisions

### 1. Recursive Field Access

Options:

- Keep `type.field_value[T, U]` only, and add a comptime type switch that can
  instantiate generic code from `TypeInfo`.
- Add a reflected value container, for example `TypedValue`, that pairs
  `TypeInfo` with an opaque value and supports safe operations.
- Add a generic visitor primitive, for example `type.visit_fields[T](value,
  visitor)`, where the compiler typechecks each visitor instantiation.

Recommendation: prefer a comptime type switch or visitor over an opaque
`TypedValue`. The goal of Jett is to keep type information precise and useful
to the checker; opaque dynamic values would pull the design toward untyped
reflection. See `docs/comptime_type_bind.md` for the current type-bind proposal.
For replacing stringly kind checks with structured tags, see
`docs/type_kind_design.md`.

### 2. Constructing `T` During `json.parse[T]`

Deserialization currently has a Rust bridge that mirrors field access: given
parsed field values, it builds `T` in declaration order while validating missing
fields, `serialize` names, secret wrappers, optionals, results, refinements, and
nested structs. Stdlib-shaped code can now build nested structs, bitfields, and
enums with primitive, list/set/map, optional, result, alias, and refinement
fields through `TypeConstruction`, but a full replacement still needs the
remaining format policy.

Refinement fields are validated at `construct_finish`; direct top-level
refinement targets validate through the decoder's refinement branch.

Options:

- `type.construct[T](fields)` where `fields` is a checked collection of
  reflected field values.
- Generated constructor code from a comptime `for field in type.fields[T]()` loop
  once the language can instantiate per-field type work.
- Keep parse as a compiler intrinsic.

Recommendation: avoid a permanent JSON-specific intrinsic. A
`type.construct[T]` or comptime-generated constructor path would also help CSV,
binary formats, config loaders, and test data generation. See
`docs/type_construction_design.md` for the current options and staging plan.

### 3. JSON Map Keys

JSON object keys are strings. Jett maps are `map[K, V]`.

Current policy: `json.serialize[map[K, V]]` and `json.parse[map[K, V]]` require
`K == string`. This matches JSON object semantics and avoids lossy display
conversions. A future explicit API can support pair-array encoding if needed.

### 4. `view` Enforcement For JSON Builtins

The design says JSON serialization reads by view:

```jett
json.serialize[User](view user)
```

The builtin checker now requires `view` for non-copy compound values passed to
`json.serialize[T]` and `json.serialize_public[T]`, while copy primitive
conveniences remain allowed.

- allow literals and copy primitives without explicit `view`, or
- require `view` uniformly and update tests/docs together.

Remaining decision: whether to keep copy primitives ergonomic or require `view`
uniformly for every JSON serialization call.

### 5. Bitfield-Specific Metadata

`type.bitfield_fields[T]()` exposes field-level layout metadata: bit width,
bits-vs-payload shape, semantic field type, and enum annotations.
`type.bitfield_layout[T]()` wraps that with bitfield-level byte-order metadata.
A full stdlib replacement for binary packing still needs a construction story
for the decoded value. See `docs/bitfield_reflection_metadata.md`.

## Suggested Next Steps

1. Add a staged stdlib loader so extracted `.jett` helpers can be tested outside
   single-file fixtures.
2. Resolve the public `json.*` bridge/handoff, then move the reflected
   JSON prototypes behind real `json.*` APIs.
3. Keep hardening format policy with parity tests against the Rust bridge:
   unknown fields, enum/bitfield shape, public/secret modes, and error messages.
