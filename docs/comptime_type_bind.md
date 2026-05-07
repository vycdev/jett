# Comptime Type Binding

This note sketches the next reflection primitive needed to move JSON and other
format libraries out of Rust builtins and into `.jett` stdlib code.

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
- `field.type_info` where `field` comes from `type.fields[T]()`
- recursive `TypeInfo.args` reached from those values

User-constructed `TypeInfo` values must not be bindable as types.

## Alternatives

### Generic Field Visitor

```jett
type.visit_fields[T](view value, function[Field](view field_value: Field, view field: TypeField) returns nothing:
    ...
)
```

This is ergonomic for struct traversal, but it makes control flow and generic
callback typing more magical than the rest of the language today. It also helps
fields specifically, while JSON needs recursive dispatch for list elements, map
values, optionals, results, aliases, refinements, and enums too.

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

Initial implementation can support only `field.type_info` inside loops over
`type.fields[T]()` plus `type.info[T]()` for root tests. Support for nested
`args` can follow once the trust provenance is represented cleanly.

## Tests

Run-pass:

- A reflection-driven serializer that handles primitive struct fields without
  comparing `field.type_name` strings.
- Nested fields: `list[User]`, `optional[User]`, `result[int64, string]`,
  `map[string, int64]`.
- Generic structs: `Box[list[int64]]`.
- Alias/refinement metadata: bind the base type from `TypeInfo.args[0]`.
- Secret filtering still uses `TypeInfo.has_secret` before field reads.

Compile-fail:

- Binding from a user-constructed `TypeInfo`.
- Binding from non-`TypeInfo` values.
- Using the bound type outside the block.
- `type.field_value[T, Wrong]` still rejects mismatched metadata.
- Binding a field from one owner and reading it from another owner remains
  rejected.

## Open Questions

- What is the exact syntax? `comptime type Field = info:` is readable and
  matches existing indentation, but it reserves a new statement form.
- How should trust provenance be represented? A hidden runtime tag on
  interpreter `Value::Struct`, an AST provenance side table, or a dedicated
  internal reflection value could all work.
- Should nested `TypeInfo.args` preserve provenance automatically, or should
  there be explicit helper functions such as `type.arg(info, index)`?
- Can the same primitive bind enum variant payload fields from
  `type.variants[T]()` without adding a second visitor-specific mechanism?
