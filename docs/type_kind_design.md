# Structured Type Kind Metadata

This note sketches a migration path from stringly reflection kinds toward a
structured representation.

## Problem

Today reflection exposes broad type categories as strings:

- `type.kind[T]()` returns values such as `"primitive"`, `"struct"`, `"list"`,
  `"optional"`, `"enum"`, and `"bitfield"`.
- `TypeInfo.kind` and `TypeField.kind` repeat those strings.
- `TypeBitfieldField.shape` is also a string, currently `"bits"` or
  `"payload"`.

This is easy to bootstrap and good for readable tests, but it has the usual
stringly downsides:

- misspellings are ordinary string bugs,
- callers compare against magic literals,
- adding a new kind has no exhaustiveness signal,
- bitfield field shape is a different string namespace from type kind.

JSON reflection is now large enough that those string comparisons show up in
real generic code. That is the right pressure to design the replacement, but
not necessarily to break the current API immediately.

## Goals

- Keep reflection values statically typed and checker-friendly.
- Preserve readable string names for diagnostics, logs, and compatibility.
- Make stdlib code branch on structured values where possible.
- Avoid introducing dynamic `any` or untrusted metadata.
- Keep the migration incremental; current `.jett` fixtures should not need a
  flag day rewrite.

## Surface

Add builtin reflected enums:

```jett
enum TypeKind:
    primitive_type
    alias_type
    refinement_type
    struct_type
    bitfield_type
    enum_type
    list_type
    set_type
    map_type
    optional_type
    result_type
    secret_type
    function_type
    unknown_type

enum TypeBitfieldFieldShape:
    bits_field
    payload_field
```

Metadata structs are extended without removing existing string fields:

```jett
struct TypeInfo:
    type_name: string
    kind: string
    kind_tag: TypeKind
    has_secret: bool
    args: list[TypeInfo]

struct TypeField:
    index: int64
    name: string
    type_name: string
    kind: string
    kind_tag: TypeKind
    serialize_name: string
    has_secret: bool
    type_info: TypeInfo

struct TypeBitfieldField:
    index: int64
    name: string
    shape: string
    shape_tag: TypeBitfieldFieldShape
    width: int64
    type_info: TypeInfo
    enum_type: optional[TypeInfo]
```

`type.kind[T]()` keeps returning `string` for compatibility.
`type.kind_tag[T]()` returns `TypeKind`.

## Why Add Fields Instead Of Replacing `kind`

Replacing `kind: string` with `kind: TypeKind` is cleaner in the abstract, but
it would churn every existing reflected fixture and every early stdlib
prototype. Keeping both fields has a practical advantage:

- existing code keeps compiling,
- new code can switch to `kind_tag`,
- diagnostics can still print the exact stable string,
- tests can migrate gradually.

The long-term API can deprecate string comparisons once enough code uses the
structured fields.

## Exhaustiveness

Jett already has enum `match` exhaustiveness checking. Once `kind_tag` exists,
stdlib code can use:

```jett
match info.kind_tag:
    primitive_type:
        ...
    list_type:
        ...
    other:
        return fail("unsupported type kind {info.kind}")
```

For generic JSON code, using a catch-all branch is still useful because not
every type kind is serializable. The difference is that typos become impossible
and new variants can be reviewed deliberately.

## Open Compatibility Questions

- Should the structured field be named `kind_tag`, `kind_value`, or `tag`?
  Recommendation: `kind_tag`, because it reads clearly beside existing `kind`.
- Why do enum variants use names such as `struct_type` and `list_type` instead
  of `struct` and `list`?
  Because many natural kind names are Jett keywords or builtin type names.
  Variant names must be source-friendly.
- Should `type.kind_tag[T]()` exist immediately?
  Recommendation: not required for the first migration; `type.info[T]().kind_tag`
  is enough, but the helper is harmless if call-site ergonomics matter.
- Should primitive sub-kinds be represented? Today `int64`, `string`, `bytes`,
  and `bool` all report `primitive`.
  Recommendation: keep `TypeKind.primitive` broad and use `type_name` for exact
  primitive dispatch until there is a clear need for `PrimitiveKind`.
- Should bitfield `shape` reuse `TypeKind`?
  Recommendation: no. Field shape answers a different question from semantic
  value type. Keep `TypeBitfieldFieldShape`.

## Staging

1. Done: add `TypeKind` and `TypeBitfieldFieldShape` as builtin metadata enums
   in the checker/interpreter metadata installation path.
2. Done: add `type.kind_tag[T]()`, `kind_tag`, and `shape_tag` while preserving
   the existing string fields.
3. Done: extend `type_info_reflection.jett` with structured-kind assertions.
4. Migrate the JSON reflection fixtures to branch on structured tags where it
   improves clarity.
5. Decide later whether `type.kind[T]()` should stay forever as a string helper
   now that the parallel `type.kind_tag[T]()` API exists.
