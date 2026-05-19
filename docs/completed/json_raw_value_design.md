# Raw JSON Values

This note records the first raw JSON surface in Jett. It began as an opaque
`JsonValue` bridge toward stdlib `json.parse[T]`; the active implementation now
executes that surface through the self-hosted `json.JsonTree` representation.

Long term, bare `JsonValue` should become a compatibility spelling for the
native `JsonTree` representation rather than a compiler-owned legacy spelling.
See `/docs/active/json_value_transition_plan.md`.

## Current Compatibility Surface

`json.JsonTree` is the stdlib-owned raw JSON tree. `json.JsonValue` is an
exported namespaced alias for that tree, while bare `JsonValue` remains a
compiler-owned compatibility spelling for one transition stage. It is not an
`any` type: user code can only inspect raw JSON through explicit `json.*`
accessors, and typed conversion still returns `result[T, string]`.

Implemented accessors:

- `json.parse_raw(raw: string)` returns `result[json.JsonTree, string]`, with
  compatibility assignment to bare `JsonValue`.
- `json.serialize_raw(view value: json.JsonTree)` returns a compact JSON
  string.
- `json.kind(view value: json.JsonTree)` returns `null`, `bool`, `number`, `string`,
  `array`, or `object`.
- `json.is_null`, `json.is_bool`, `json.is_number`, `json.is_string`,
  `json.is_array`, and `json.is_object` return `bool`.
- `json.field(view value: json.JsonTree, key: string)` returns
  `optional[json.JsonTree]`.
- `json.index(view value: json.JsonTree, index: int64)` returns
  `optional[json.JsonTree]`.
- `json.array_length(view value: json.JsonTree)` returns `result[int64, string]`.
- `json.object_keys(view value: json.JsonTree)` returns
  `result[list[string], string]`.
- `json.as_string`, `json.as_int64`, `json.as_float64`, and `json.as_bool`
  return typed `result` values.

## Stdlib Tree Staging

`stdlib/json/` also defines a first self-hosted raw tree shape:

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

The tree also has stdlib traversal helpers mirroring the raw API:
`json_tree_kind`, `json_tree_is_*`, `json_tree_field`, `json_tree_index`,
`json_tree_array_length`, `json_tree_object_keys`, and scalar casts for string,
int64, float64, and bool. That gives the decoder a Rust-free tree surface to
target.

This was intentionally staged alongside the opaque `JsonValue` primitive first.
The current implementation now routes `json.parse_raw`, raw accessors, and
typed public `json.parse[T]` through `JsonTree`; bare `JsonValue` remains only a
compatibility spelling. The parser now has pinned diagnostics for common
malformed inputs such as unterminated strings/arrays/objects, trailing
characters, mismatched delimiters, bad number forms, bad literals, and invalid
escapes. Unicode escapes including BMP values such as `\u0041`, `\u00e9`,
`\u20ac`, and surrogate pairs such as `\ud834\udd1e` are decoded.

## Design Intent

Raw JSON values are format data, not reflected Jett values. They should help a
future stdlib JSON implementation decode input by walking an object tree,
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

The `.jett` decoder uses `JsonTree` to parse and inspect the raw tree, then uses
trusted reflection to decode each field at its concrete type:

```jett
for field in type.fields[T]():
    comptime type Field = field.type_info:
        json.JsonTree raw_field = json.field(view raw, field.serialize_name) handle:
            ...
        Field value = decode_json[Field](view raw_field) handle error:
            ...
        ...
```

For structs, bitfields, and enums, construction of the final `T` from typed
field or payload values is available through `TypeConstruction`. The stdlib
decoder uses that path and treats absent optional fields as `none`. The original
flat and nested struct proofs live in
`tests/run_pass/json_reflection_flat_decoder.jett` and
`tests/run_pass/json_reflection_nested_decoder.jett`; the active stdlib bridge
coverage now lives in the JSON run-pass and driver tests.

## Open Questions

- Should object traversal grow a typed entry iterator, or are
  `json.object_keys` plus `json.field` enough?
- Should `json.field` and `json.index` return `result[optional[JsonValue],
  string]` to distinguish wrong-shape access from ordinary absence?
- During the transition, should `JsonValue` stay copyable for ergonomics, or
  become a non-copy compatibility spelling whose raw accessors read by `view`?
- Should `JsonTree.number_value` keep raw number text for round-tripping, or
  split into integer/float variants once numeric parsing is self-hosted?
- Should unicode escape handling live in the parser itself or in a shared
  string decoding helper used by JSON and future text formats?
