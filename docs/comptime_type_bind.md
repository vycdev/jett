# Comptime Type Binding

This note sketches the next reflection primitive needed to move JSON and other
format libraries out of Rust builtins and into `.jett` stdlib code.

For the reverse direction, where reflected code needs to build a `T` from typed
field values, see `docs/type_construction_design.md`.

## Problem

Jett code can inspect a field:

```jett
list[TypeField] fields = type.fields[T]()
```

and it can read a field if the field type is already known:

```jett
string value = type.field_value[T, string](view item, view field)
```

But it cannot say "take this compiler-produced `field.type_info`, bind it as a
real type parameter, and call generic code for that type." Without that bridge,
stdlib serializers either stay flat and stringly, or drift toward an opaque
dynamic value container.

## Recommended Direction

Add a narrow comptime type-bind form that turns trusted reflection metadata into
a scoped type alias:

```jett
for field in type.fields[T]():
    comptime type Field = field.type_info:
        Field item = type.field_value[T, Field](view value, view field)
        string encoded = json.serialize_value[Field](view item)
```

The intended lowering is monomorphization: for each compiler-known field,
compile the block with `Field` substituted by that field's concrete `TypeExpr`.
The generated code remains statically typed; `TypeInfo` is metadata, not an
`any` value.

This should initially be limited to trusted metadata produced by reflection:

- `type.info[T]()`
- `type.arg[T](literal_index)`
- `field.type_info` where `field` comes from `type.fields[T]()`
- `field.type_info` where `field` comes from
  `type.variant_value[T](view value).fields`
- `field.type_info` where `field` comes from `variant.fields` inside a direct
  `for variant in type.variants[T]():` loop
- recursive `TypeInfo.args` reached from those values

User-constructed `TypeInfo` values must not be bindable as types.

## Alternatives

### Generic Field Visitor

```jett
type.visit_fields[T](view value, function[Field](view field_value: Field, view field: TypeField) returns nothing:
    ...
)
```

This is ergonomic for struct and bitfield traversal, but it makes control flow
and generic callback typing more magical than the rest of the language today.
It also helps fields specifically, while JSON needs recursive dispatch for list
elements, map values, optionals, results, aliases, refinements, and enums too.

### TypedValue Container

```jett
TypedValue item = type.field_typed_value[T](view value, view field)
```

This would be quick to prototype, but it weakens Jett's design. It creates a
dynamic reflection lane that looks like `any`, forces runtime operations to
recover type information, and makes the checker less central. Avoid this unless
the static approach proves impossible.

## Prototype Scope

Start with the smallest useful form:

```jett
comptime type Name = some_type_info:
    ...
```

Rules:

- The initializer must be a comptime-known trusted `TypeInfo`.
- `Name` is visible only inside the indented block.
- Generic calls may use `Name` as a normal type argument.
- Variable declarations may use `Name` as a normal type annotation.
- The compiler lowers the block once per concrete type binding.
- No runtime `TypeInfo -> type` conversion exists.

The current implementation supports:

- direct roots such as `comptime type Root = type.info[T]():`
- direct type arguments such as `comptime type Value = type.arg[map[string, V]](1):`
- `field.type_info` when `field` is the loop variable of a direct
  `for field in type.fields[T]():` loop
- `field.type_info` when `field` is the loop variable of a direct active enum
  payload loop such as `for field in type.variant_value[T](view value).fields:`
- `field.type_info` when `field` is the loop variable of a direct static enum
  variant payload loop nested under `for variant in type.variants[T]():`
- `TypeInfo` values produced by direct reflected `args` loops, such as
  `for arg in type.info[list[T]]().args:` or nested `for inner in arg.args:`

The field form is intentionally narrow. Binding through an intermediate list,
through reassigned metadata, or from user-constructed `TypeInfo` is rejected or
fails runtime validation. Bitfield fields share the same `type.fields[T]()` path;
bitfield-specific metadata such as width and enum annotations is still exposed
separately. The trusted `args` form currently follows direct `type.info[T]()`,
trusted field metadata, and previously trusted `args` loop variables; storing
metadata in ordinary variables still drops provenance.
The same narrow rule applies to enum payload fields: a stored `TypeVariant`
value can be inspected as data, but only direct active-value payload loops and
direct `type.variants[T]()` loops carry enough provenance to bind payload field
types.

`type.arg[T](index)` is also available as an ordinary runtime metadata helper:
it returns the indexed `TypeInfo` argument for wrappers such as `list[T]`,
`map[K, V]`, `optional[T]`, `result[T, E]`, aliases, refinements, and function
types. Only direct calls with a literal non-negative index are trusted as
`comptime type` initializers; dynamic indexes still produce metadata, not a
type.

## Tests

Run-pass:

- A reflection-driven serializer that handles primitive struct and bitfield
  fields with `comptime type Field = field.type_info:`.
- Nested fields: `list[User]`, `optional[User]`, `result[int64, string]`,
  `map[string, int64]`.
- Generic structs: `Box[list[int64]]`.
- Alias/refinement metadata: bind the base type from a trusted `TypeInfo.args`
  loop.
- Secret filtering still uses `TypeInfo.has_secret` before field reads.

Compile-fail:

- Binding from a user-constructed `TypeInfo`.
- Binding from non-`TypeInfo` values.
- Using the bound type outside the block.
- `type.field_value[T, Wrong]` still rejects mismatched metadata.
- Binding through an intermediate `list[TypeField]` rather than a direct
  `type.fields[T]()` loop.
- Binding through an intermediate `TypeInfo` variable rather than a direct
  reflected `args` loop.
- Binding through `type.arg[T](index)` with a non-literal index.
- Binding through an intermediate `TypeVariant` variable rather than a direct
  active enum payload field loop or a direct `type.variants[T]()` loop.
- Binding a field from one owner and reading it from another owner remains
  rejected.

## Open Questions

- Should broader trust provenance use hidden tags on interpreter `Value::Struct`,
  an AST provenance side table, or a dedicated internal reflection value?
- Should nested `TypeInfo.args` preserve provenance beyond the direct trusted
  loops, or is `type.arg[T](literal)` enough for the common generic wrappers?
- Should static `type.variants[T]()` field binding preserve selected-variant
  provenance in later compiler IR, or is the current direct-loop validation
  enough for stdlib JSON and similar formats?
