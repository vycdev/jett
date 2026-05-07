# JSON Reflection Plan

This note tracks the remaining design work for moving JSON serialization and
deserialization out of Rust builtins and into Jett stdlib code.

## Current State

Implemented reflection primitives:

- `type.name[T]()` returns a stable display name.
- `type.kind[T]()` returns a broad kind string.
- `type.has_secret[T]()` reports whether a type contains secret data.
- `type.info[T]()` returns recursive `TypeInfo` metadata with `type_name`,
  `kind`, `has_secret`, and nested `args`.
- `type.fields[T]()` returns ordered `TypeField` metadata for structs.
- `TypeField` includes `index`, `name`, `type_name`, `kind`,
  `serialize_name`, `has_secret`, and `type_info`.
- `type.field_value[T, U](view value, view field)` reads a field by metadata
  after checking that the metadata belongs to `T` and that `U` matches the
  reflected field type.

JSON serialization is still Rust-backed, but it now consumes the same field
metadata exposed to Jett code. The active tests cover nested structs, lists,
maps with string keys, optionals, results, `serialize` names, public secret
omission, and valid JSON string escaping for control characters.

`json.parse[T](raw)` has a Rust-backed bridge: it requires one type argument,
accepts a string, returns `result[T, string]`, and supports core primitives,
structs with `serialize` names, lists, sets, `map[string, V]`, optionals,
results, generic structs, and refinement validation. The long-term goal is
still to replace this bridge with stdlib code plus a general construction
primitive.

## Design Pressure

A real `stdlib/json.jett` serializer needs more than field metadata. It needs a
way to turn reflected field type metadata into typed work.

Today, this is possible for a small fixed set of known field types:

```jett
if field.type_name == "string":
    string value = type.field_value[T, string](view item, view field)
```

That is enough for a flat proof of concept, but not enough for arbitrary nested
structs. Jett code cannot currently say "call `serialize_value[FieldType]` where
`FieldType` comes from this `TypeField` metadata." The next primitive should
close that gap without becoming JSON-specific.

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
reflection.

### 2. Constructing `T` During `json.parse[T]`

Deserialization currently has a Rust bridge that mirrors field access: given
parsed field values, it builds `T` in declaration order while validating missing
fields, `serialize` names, secret wrappers, optionals, results, refinements, and
nested structs. The missing language feature is the general primitive that would
let stdlib code do this itself.

Options:

- `type.construct[T](fields)` where `fields` is a checked collection of
  reflected field values.
- Generated constructor code from a comptime `for field in type.fields[T]()` loop
  once the language can instantiate per-field type work.
- Keep parse as a compiler intrinsic.

Recommendation: avoid a permanent JSON-specific intrinsic. A
`type.construct[T]` or comptime-generated constructor path would also help CSV,
binary formats, config loaders, and test data generation.

### 3. JSON Map Keys

JSON object keys are strings. Jett maps are `map[K, V]`.

Options:

- Reject `json.serialize[map[K, V]]` unless `K == string`.
- Serialize non-string keys by applying a display conversion.
- Encode maps as arrays of key/value pairs when `K != string`.

Recommendation: reject non-string keys first. It is the simplest safe contract,
matches `json.parse[map[string, V]]`, and avoids lossy display conversions. A
future explicit API can support pair-array encoding if needed.

### 4. `view` Enforcement For JSON Builtins

The design says JSON serialization reads by view:

```jett
json.serialize[User](view user)
```

The current builtin checker accepts owned arguments too, and active primitive
tests still use literals. Tightening this should be done deliberately:

- allow literals and copy primitives without explicit `view`, or
- require `view` uniformly and update tests/docs together.

Recommendation: enforce `view` for non-copy compound values first, then decide
whether literals and primitives should remain ergonomic exceptions.

## Suggested Next Steps

1. Add compile-fail coverage for `json.serialize[map[int64, V]]` once the
   non-string-key policy is accepted.
2. Design the comptime type-switch or typed field visitor primitive.
3. Implement a nested `.jett` serializer prototype for primitives, structs,
   lists, maps with string keys, optionals, and results.
4. Design `type.construct[T]` before implementing `json.parse[T]`.
