# Raw JSON Values

This note records the first raw JSON surface in Jett. It is meant as a bridge
toward stdlib `json.parse[T]`, not as a replacement for typed parsing.

## Current Primitive

`JsonValue` is an opaque built-in value that stores a parsed JSON tree. It is not
an `any` type: user code can only inspect it through explicit `json.*` accessors,
and typed conversion still returns `result[T, string]`.

Implemented accessors:

- `json.parse_raw(raw: string)` returns `result[JsonValue, string]`.
- `json.serialize_raw(value: JsonValue)` returns a compact JSON string.
- `json.kind(value: JsonValue)` returns `null`, `bool`, `number`, `string`,
  `array`, or `object`.
- `json.is_null`, `json.is_bool`, `json.is_number`, `json.is_string`,
  `json.is_array`, and `json.is_object` return `bool`.
- `json.field(value: JsonValue, key: string)` returns `optional[JsonValue]`.
- `json.index(value: JsonValue, index: int64)` returns `optional[JsonValue]`.
- `json.array_length(value: JsonValue)` returns `result[int64, string]`.
- `json.object_keys(value: JsonValue)` returns `result[list[string], string]`.
- `json.as_string`, `json.as_int64`, `json.as_float64`, and `json.as_bool`
  return typed `result` values.

## Stdlib Tree Staging

`stdlib/json.jett` also defines a first self-hosted raw tree shape:

```jett
enum JsonTree:
    null
    bool_value(value: bool)
    number_value(raw: string)
    string_value(value: string)
    array_value(items: list[JsonTree])
    object_value(fields: map[string, JsonTree])
```

`json.json_tree_serialize(value)` serializes that tree using the same string
quoting helper as reflected typed serialization. `json.json_tree_parse(raw)`
now parses that staged tree shape for nulls, booleans, raw-number tokens,
strings, arrays, and objects.

The staged tree also has stdlib traversal helpers mirroring the opaque raw API:
`json_tree_kind`, `json_tree_is_*`, `json_tree_field`, `json_tree_index`,
`json_tree_array_length`, `json_tree_object_keys`, and scalar casts for string,
int64, float64, and bool. That gives future decoder work a Rust-free tree
surface to target.

This is intentionally staged alongside, not instead of, the opaque `JsonValue`
primitive. `JsonTree` gives the self-hosted parser a stdlib-owned target
without breaking existing `json.parse_raw`, raw accessors, or the reflected
decoder that currently walks `JsonValue`. The staged parser is not yet a full
replacement for `json.parse_raw`; broader malformed-input diagnostics still
need hardening before that handoff. Unicode escapes including BMP values such as
`\u0041`, `\u00e9`, `\u20ac`, and surrogate pairs such as `\ud834\udd1e` are
already decoded.

## Design Intent

Raw JSON values are format data, not reflected Jett values. They should help a
future `stdlib/json.jett` implementation decode input by walking an object tree,
checking field presence, applying `serialize_name`, and producing clear errors.

The raw API deliberately keeps these properties:

- **No unchecked construction.** A `JsonValue` does not become a `T` without a
  typed conversion path.
- **Explicit errors.** Parsing and scalar casts return `result`, so callers must
  handle malformed input and type mismatches.
- **Missing data is visible.** Field and index lookup return `optional`, which
  lets format code distinguish absent values from present JSON `null`.
- **Wrong-shape traversal is absence for lookup.** `json.field` on a non-object
  and `json.index` on a non-array return `none`; shape-requiring operations such
  as `json.object_keys`, `json.array_length`, and scalar casts return
  `result` errors.
- **Format policy remains in `json`.** Optional defaults, unknown key handling,
  enum object shape, and `serialize_name` lookup stay outside any eventual
  `type.construct[T]` primitive.

## Relationship To Reflected Construction

A future `.jett` decoder can use `JsonValue` to parse and inspect the raw tree,
then use trusted reflection to decode each field at its concrete type:

```jett
for field in type.fields[T]():
    comptime type Field = field.type_info:
        JsonValue raw_field = json.field(raw, field.serialize_name) handle:
            ...
        Field value = decode_json[Field](view raw_field) handle error:
            ...
        ...
```

For structs, bitfields, and enums, construction of the final `T` from typed
field or payload values is now available through `TypeConstruction`. The nested
decoder proof uses that path and treats absent optional fields as `none`; the
remaining work is hardening the prototype into stdlib code. That is tracked in
`docs/type_construction_design.md` and `docs/reflected_construction_staging.md`.
The first flat and nested struct proofs live in
`tests/run_pass/json_reflection_flat_decoder.jett` and
`tests/run_pass/json_reflection_nested_decoder.jett`.

## Open Questions

- Should object traversal grow a typed entry iterator, or are
  `json.object_keys` plus `json.field` enough?
- Should `json.field` and `json.index` return `result[optional[JsonValue],
  string]` to distinguish wrong-shape access from ordinary absence?
- Should `JsonValue` stay copyable for ergonomics, or eventually become a
  non-copy value that raw accessors read by `view`?
- Should `JsonTree.number_value` keep raw number text for round-tripping, or
  split into integer/float variants once numeric parsing is self-hosted?
- Should unicode escape handling live in the parser itself or in a shared
  string decoding helper used by JSON and future text formats?
