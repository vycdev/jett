# Reflected Type Construction

This note records the design space for the construction primitive needed before
`json.parse[T]`, binary decoding, CSV loading, and test-data generation can move
out of Rust bridges and into `.jett` stdlib code.

## Problem

Jett can now reflect a type's shape and read fields by trusted metadata:

```jett
list[TypeField] fields = type.fields[T]()
value = type.field_value[T, Field](view item, view field)
```

Deserialization needs the reverse operation: given checked field values, build a
`T` while preserving the exact same rules as ordinary source constructors. The
first form now exists as the opaque `TypeConstruction` builder for structs,
bitfields, and enum variants; the final syntax story is still open.

The missing operation is not JSON-specific. JSON happens to be the current
pressure point, but the same primitive should serve:

- `json.parse[T]`,
- `csv.parse_rows[T]`,
- `T.from_bytes` for structs and bitfields,
- schema-driven config loaders,
- generators used by `property` tests.

## Existing Semantics To Preserve

Ordinary struct construction already enforces:

- declaration order for positional fields,
- named field lookup,
- duplicate-field rejection,
- missing required field errors,
- field type checking,
- refinement boundary validation.

When any struct field has a refinement type, the constructor typechecks as
`result[Struct, string]`; otherwise it returns `Struct` directly. At runtime,
invalid refinement values become `fail(message)`.

Bitfield construction similarly enforces:

- field order and named lookup,
- duplicate and missing field errors,
- field type checking,
- fixed-width integer range validation,
- enum-annotated bit fields,
- result-returning construction when runtime validation is required.

Enum construction enforces:

- selecting a declared variant by checked `TypeVariant` metadata,
- variant-local payload field lookup,
- duplicate and missing payload-field errors,
- payload field type checking,
- refinement boundary validation for payload values.

A reflected constructor must not be a weaker back door around these checks. It
should reuse the same typechecker and interpreter rules where possible.

## Design Constraints

- **Static types stay central.** Avoid an `any`-like container where values carry
  only runtime `TypeInfo`. Jett's purpose is to make the checker do useful work.
- **Trusted metadata matters.** Compiler-produced `TypeField`, `TypeVariant`,
  and `TypeInfo` can drive construction. User-created structs that merely look
  like metadata must not be trusted.
- **Errors are explicit.** Construction from parsed or decoded data should
  surface as `result[T, string]`, forcing a `handle error:` boundary.
- **Format policy stays outside construction.** JSON `serialize_name`, missing
  optional fields, unknown key handling, and enum object shape are format-module
  decisions. The construction primitive should receive final field values.
- **Secret types remain type-system facts.** Constructing a value with
  `secret[T]` fields should not declassify anything or make serialization
  easier; it should simply produce a value whose type still carries secret
  taint.

## Options

### 1. Dynamic Field Bag

```jett
result[T, string] built = type.construct[T](fields)
```

`fields` would be some runtime collection of names, metadata, and values.

This looks simple, but Jett does not currently have a heterogenous collection
type that can hold field values without erasing their types. Adding one would
pull reflection toward a dynamic `any` lane, which fights the language's main
principle.

Verdict: avoid as the primary design.

### 2. Generic Builder Callback

```jett
result[T, string] built = type.construct[T](function[Field](view field: TypeField) returns result[Field, string]:
    return json.decode_field[Field](view raw, field.serialize_name)
)
```

The compiler would call the builder once per reflected field, with `Field`
bound to that field's concrete type. It would collect the returned values and
invoke the normal constructor path.

This keeps field values statically typed and gives formats control over missing
data and error messages. The cost is new generic-callback machinery, which Jett
does not otherwise have yet.

Verdict: promising, but probably after the `comptime type` binding mechanism is
implemented and proven.

### 3. Comptime Construction Block

```jett
result[T, string] built = type.construct[T]:
    for field in type.fields[T]():
        comptime type Field = field.type_info:
            provide field = json.decode_field[Field](view raw, field.serialize_name)
```

The exact syntax is open, but the shape is deliberate: the block runs at
compile time over trusted metadata, each `provide` expression is typechecked
with the concrete field type, and the compiler lowers the result to a normal
constructor call.

This is closest to Jett's principles. It is explicit, indentation-based,
compile-time-specialized, and avoids opaque values. It also composes naturally
with `docs/comptime_type_bind.md`.

Verdict: preferred direction, pending syntax.

## Recommended Staging

1. Implement the trusted `comptime type Name = info:` binding from
   `docs/comptime_type_bind.md`.
2. Done for structs: prototype the no-syntax opaque builder described in
   `docs/reflected_construction_staging.md`, with `construct_put` returning
   `result[TypeConstruction, string]` and `construct_finish` returning
   `result[T, string]`.
3. Done for bitfields: reuse reflected `TypeField` values and validate bit
   widths on `construct_finish`.
4. Done for enums: start from checked `TypeVariant` metadata with
   `type.construct_variant_start[T](variant)`, then reuse `construct_put` and
   `construct_finish` for variant-local payload fields.
5. Replace pieces of Rust-backed `json.parse[T]` with `.jett` code as each
   construction case becomes expressible.

## Implementation Notes

The current code already has most of the semantic checks in one place:

- typechecking ordinary struct constructors: `check_struct_constructor`,
- typechecking ordinary bitfield constructors: `check_bitfield_constructor`,
- runtime/interpreter struct construction: `construct_struct`,
- runtime/interpreter bitfield construction: `construct_bitfield`,
- runtime/interpreter enum construction: `Value::Enum` construction and
  `json_to_enum_value`,
- JSON behavior oracle while replacing the Rust bridge:
  `json_to_value_typed`, `json_to_struct_value`, `json_to_bitfield_value`, and
  `json_to_enum_value`.

A reflected construction implementation should route through these same paths
or share their core helpers. Duplicating the checks risks subtle drift, especially
around refinement result types and bitfield range validation.

The construction primitive should receive values that are already decoded into
their field types. Format modules remain responsible for:

- matching external names such as JSON `serialize_name`,
- deciding whether missing optional fields become `none`,
- choosing whether unknown external keys are ignored or rejected,
- producing format-specific error messages.

This keeps `type.construct` from becoming `json.construct` in disguise.

## Open Questions

- Should the eventual block syntax always return `result[T, string]`, matching
  the builder, or should it mirror ordinary constructors and return plain `T`
  when no validation can fail?
- What exact syntax should bind a generated field value to a reflected
  `TypeField`? `provide field = ...`, `field field = ...`, and a callback API
  are all still candidates.
- How should trusted provenance be represented internally: hidden flags on
  reflection values, dedicated internal value variants, or typed side tables?
- Should construction accept only `TypeField` metadata, or should there be a
  distinct trusted `TypeConstructorField` that cannot be confused with fields
  used only for reading?
- How should extra input fields be handled? The construction primitive should
  probably ignore that policy and let each format module decide before calling
  construction.

See `docs/reflected_construction_staging.md` for the current builder surface and
`docs/type_construction_block_syntax.md` for the recommended long-term block
syntax direction.
