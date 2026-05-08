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
- `json.is_null(value: JsonValue)` returns `bool`.
- `json.field(value: JsonValue, key: string)` returns `optional[JsonValue]`.
- `json.index(value: JsonValue, index: int64)` returns `optional[JsonValue]`.
- `json.array_length(value: JsonValue)` returns `result[int64, string]`.
- `json.object_keys(value: JsonValue)` returns `result[list[string], string]`.
- `json.as_string`, `json.as_int64`, `json.as_float64`, and `json.as_bool`
  return typed `result` values.

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

For structs, construction of the final `T` from those typed field values is now
available through `TypeConstruction`. The remaining work is integration in a
`.jett` decoder plus bitfield and enum construction support. That is tracked in
`docs/type_construction_design.md` and `docs/reflected_construction_staging.md`.

## Open Questions

- Should object traversal grow a typed entry iterator, or are
  `json.object_keys` plus `json.field` enough?
- Should `json.field` and `json.index` return `result[optional[JsonValue],
  string]` to distinguish wrong-shape access from ordinary absence?
- Should `JsonValue` stay copyable for ergonomics, or eventually become a
  non-copy value that raw accessors read by `view`?
