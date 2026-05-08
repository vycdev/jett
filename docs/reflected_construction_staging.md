# Reflected Construction Staging

This note narrows the next step after field/variant reflection and raw
`JsonValue` support. The goal is still a stdlib `json.parse[T]`, but the
construction primitive must stay format-agnostic and statically typed.

## Endpoint: Construction Block

The cleanest language shape is a dedicated block expression:

```jett
T value = type.construct[T]:
    for field in type.fields[T]():
        comptime type Field = field.type_info:
            Field decoded = decode_json[Field](view raw, view field) handle error:
                return fail(error)
            provide field = decoded
handle error:
    return fail(error)
```

This matches Jett's indentation-first design and keeps construction explicit.
It also lets the compiler lower the block to the same path as an ordinary
constructor call, preserving missing-field, duplicate-field, refinement, and
bitfield validation rules.

The cost is that this is not a normal builtin. `type.construct[T]:` needs parser
and AST support for a block expression, plus a contextual `provide` statement
inside that block.

## Near-Term Prototype: Opaque Builder

A no-syntax bridge can use existing reflection and `comptime type` binding:

```jett
TypeConstruction builder = type.construct_start[T]()

for field in type.fields[T]():
    comptime type Field = field.type_info:
        Field decoded = decode_json[Field](view raw, view field) handle error:
            return fail(error)
        builder = type.construct_put[T, Field](builder, view field, decoded) handle error:
            return fail(error)

return type.construct_finish[T](builder)
```

This is less elegant, but it avoids adding syntax before the construction
semantics are proven. The builder is opaque: user code can add typed values, but
cannot read heterogeneous values back out.

## Why The Builder Is Acceptable As A Step

- `type.construct_put[T, Field]` typechecks the provided value as the concrete
  `Field` type, so user code does not get an `any` lane.
- The builder receives already-decoded values; JSON-specific policy such as
  `serialize_name`, missing optional defaults, and unknown-key handling stays in
  `json`.
- `type.construct_finish[T]` can return `result[T, string]` uniformly, giving
  callers the same explicit `handle error:` boundary used by `json.parse[T]`.
- The same API can later be lowered away or replaced by the block syntax once
  the compiler has a better block-expression story.

## Guardrails For The Prototype

The first implementation should be deliberately narrow:

- Support structs first; add bitfields only after the struct path is stable.
- Require `construct_put` field metadata to match `T` by index, name, and
  reflected type. This is not full provenance, but it matches the current
  `type.field_value` safety model.
- Reject duplicate fields and missing fields in `construct_finish`.
- Return `result[T, string]` from both `construct_put` and `construct_finish`.
- Do not support enum construction in the first slice.
- Do not parse `TypeInfo.type_name` or trust arbitrary user-created `TypeInfo`.

Longer term, trusted metadata provenance should become an internal value fact
rather than a convention reconstructed from public `TypeField` contents.

## Code Touch Points

For the no-syntax builder:

- `crates/jett_types`: add an opaque `TypeConstruction` built-in type.
- `crates/jett_resolve`: pre-register `TypeConstruction`.
- `crates/jett_typecheck`: add builtin signatures for
  `type.construct_start`, `type.construct_put`, and `type.construct_finish`.
- `crates/jett_comptime`: add a `Value::TypeConstruction` or equivalent and
  dispatch the three builtins.
- `crates/jett_comptime::construct_struct`: reuse the existing runtime
  constructor path where possible so refinement behavior does not drift.
- Fixture tests should cover successful struct construction, missing fields,
  duplicate fields, wrong field metadata, wrong value type, generic structs, and
  refinement fields.

For the eventual block syntax:

- `crates/jett_parser::ast`: add a construct-block expression and contextual
  provide statement.
- `crates/jett_parser::parser`: parse `type.construct[T]:` as a block
  expression and ensure postfix `handle error:` still composes.
- `crates/jett_resolve`, `crates/jett_typecheck`, `crates/jett_comptime`, and
  ownership traversal all need to visit the new nodes.

## Recommendation

Implement the opaque builder as the next compiler slice. It is not the final
language shape, but it proves the hard semantic part while staying inside
existing syntax. Once a `.jett` decoder can walk `JsonValue`, decode each field
with trusted `comptime type` binding, and finish a typed struct, the case for a
cleaner `type.construct[T]:` block will be grounded in real usage instead of
guesswork.
