# Bitfield Reflection Metadata

This note records the proposed shape for exposing bitfield-specific metadata to
Jett comptime code. The current `type.fields[T]()` bridge intentionally treats
bitfields like structs: each bitfield member is a `TypeField`, and
`type.field_value[T, U]` can read it by metadata. That is enough for JSON object
serialization, but not enough to reimplement every bitfield bridge in `.jett`
stdlib code.

## Problem

Bitfields carry layout facts that ordinary struct fields do not:

- fixed-width fields have a bit width,
- some fixed-width fields are annotated with `as EnumType`,
- payload fields are variable-length `list[uint8]`,
- payload fields must be final and byte-aligned,
- the bitfield as a whole may use network byte order.

Today these facts live in parser/typechecker/interner structures and in the
Rust interpreter bridge. They are not visible to `type.fields[T]()` users.

## Principles

- Keep `TypeField` as the shared field metadata for structs, enum payloads, and
  bitfields. Code that only needs names, types, secret state, and field reads
  should not learn a second field type.
- Put bitfield layout facts behind a bitfield-specific primitive rather than
  bloating every `TypeField`.
- Reflect checked metadata, not raw syntax. Invalid payload placement, invalid
  enum annotations, and width overflow should already be rejected before
  reflection observes the type.
- Preserve semantic field types. For `protocol: 8 bits as IpProtocol`,
  `TypeField.type_info` should remain `IpProtocol`, not `int64`, because user
  code reads and serializes the named enum.

## Proposed Surface

Implemented first-stage builtin metadata struct:

```jett
struct TypeBitfieldField:
    index: int64
    name: string
    shape: string
    shape_tag: TypeBitfieldFieldShape
    width: int64
    type_info: TypeInfo
    enum_type: optional[TypeInfo]
```

Implemented first-stage reflection primitive:

```jett
type.bitfield_fields[T]() returns list[TypeBitfieldField]
```

Rules:

- `type.bitfield_fields[T]()` returns metadata only when `T` is a bitfield.
  For non-bitfields it should return an empty list, matching the current
  forgiving behavior of `type.fields[T]()` on non-field-bearing types.
- `index` and `name` match the corresponding `TypeField`.
- `shape == "bits"` for fixed-width bitfields, including single-bit fields.
- `shape == "payload"` for variable payload fields.
- `shape_tag` provides the structured equivalent of `shape`.
- `width` is the declared width for fixed fields and `0` for payload fields.
- `type_info` is the semantic field type: `int64` for unannotated 1..63-bit
  fields, `uint64` for unannotated 64-bit fields, the enum type for
  `bits as EnumType`, and `list[uint8]` for payloads.
- `enum_type` is `some(type.info[EnumType]())` only for `bits as EnumType`;
  otherwise it is `none`.

Implemented second-stage owning-type metadata struct:

```jett
struct TypeBitfield:
    network_order: bool
    fields: list[TypeBitfieldField]
```

It is exposed as `type.bitfield_layout[T]()` so future `.jett` binary encoding
code can observe the whole bitfield layout, including byte order, without
special Rust knowledge.

## Why Not Extend `TypeField`?

Adding `width`, `shape`, and `enum_type` directly to `TypeField` would make
every struct field carry irrelevant defaults. That weakens the simple mental
model: `TypeField` tells you what value exists and how to read it; a
bitfield-specific view tells you how that value is packed.

This separation also keeps enum payload fields clean. Enum payloads are
represented as `TypeField` metadata, but they are not bit-packed and should not
pretend to have bit widths.

## JSON Impact

The current JSON bridge only needs `TypeField`:

- field name,
- semantic field type,
- `type.field_value`.

That is why `json.serialize[Header](view header)` can emit
`{"version":4,"protocol":"tcp"}` without bit widths. `json.parse[Header](raw)`
does need width validation, so a future `.jett` parser replacement would use
`type.bitfield_fields[T]()` to validate numeric ranges and enum annotations
before constructing the final bitfield value.

## Binary Impact

`to_bytes` and `from_bytes` need the full layout:

- bit widths in declaration order,
- payload position,
- payload byte alignment,
- byte order.

The staged approach is:

1. Add `type.bitfield_fields[T]()` for field-level layout. Done.
2. Add `type.bitfield_layout[T]()` for bitfield-level byte-order metadata.
   Done.
3. Design construction before replacing Rust-backed parse/from-bytes behavior.

## Tests

Run-pass:

- `type.bitfield_fields[Header]()` returns two fixed fields with widths `4` and
  `8`.
- An enum-annotated field reports `type_info.kind == "enum"` and `enum_type`
  present.
- A payload field reports `shape == "payload"`, `width == 0`, and
  `type_info.type_name == "list[uint8]"`.
- `type.fields[Header]()` and `type.bitfield_fields[Header]()` agree on field
  order and names.
- `type.bitfield_layout[Header]()` exposes `network_order` and the same
  `TypeBitfieldField` list.

Compile-fail:

- Wrong arity for `type.bitfield_layout`.
- Wrong arity for `type.bitfield_fields`.
- Attempting to use user-constructed metadata as trusted bitfield layout should
  fail once trusted reflection provenance exists.

## Open Questions

- Should `shape` be a string, or should Jett grow a small reflected enum such
  as `TypeBitfieldFieldShape.bits_field` and `.payload_field`? Current recommendation:
  keep the string for compatibility and add `shape_tag`; see
  `/docs/completed/type_kind_design.md`.
- Should `enum_type` be `optional[TypeInfo]`, or should enum annotation be
  encoded only through `type_info.kind == "enum"`?
- Should `TypeBitfield` also expose derived facts such as total fixed bit width,
  payload index, or byte alignment, or should those remain easy stdlib folds
  over `fields`?
