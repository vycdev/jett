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

## Current Prototype: Opaque Builder

The first no-syntax bridge uses existing reflection and `comptime type` binding:

```jett
mutable TypeConstruction builder = type.construct_start[T]()

for field in type.fields[T]():
    comptime type Field = field.type_info:
        Field decoded = decode_json[Field](view raw, view field) handle error:
            return fail(error)
        builder = type.construct_put[T, Field](builder, view field, decoded) handle error:
            return fail(error)

return type.construct_finish[T](builder)
```

Enum construction uses the same builder after selecting a variant:

```jett
for variant in type.variants[T]():
    if variant.name == variant_name:
        mutable TypeConstruction builder = type.construct_variant_start[T](view variant) handle error:
            return fail(error)
        for field in variant.fields:
            comptime type Field = field.type_info:
                Field decoded = decode_payload[Field](view raw, view field) handle error:
                    return fail(error)
                builder = type.construct_put[T, Field](builder, view field, decoded) handle error:
                    return fail(error)
        return type.construct_finish[T](builder)
```

State-machine construction uses a separate starter:

```jett
for state in type.machine_states[T]():
    if state.name == state_name:
        mutable TypeConstruction builder = type.construct_machine_start[T](view state) handle error:
            return fail(error)
        for field in state.fields:
            comptime type Field = field.type_info:
                Field decoded = decode_payload[Field](view raw, view field) handle error:
                    return fail(error)
                builder = type.construct_put[T, Field](builder, view field, decoded) handle error:
                    return fail(error)
        return type.construct_finish[T](builder)
```

The dedicated starter matters because machine states are not enum variants:
state-qualified targets such as `Session at logged_in` carry static precision
that `construct_finish` must preserve, and transition edges are separate
metadata rather than construction-time proof obligations.

This is less elegant than the eventual block syntax, but it proves the
construction semantics without adding new syntax. The builder is opaque: user
code can add typed values, but cannot read heterogeneous values back out.

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

The current implementation is still deliberately narrow:

- Supports structs, bitfields, and enum variants. Enums require
  `type.construct_variant_start[T](variant)` because a variant must be selected
  before payload fields can be provided.
- Machine construction uses `type.construct_machine_start[T](state)` rather
  than the enum starter; a state must be selected before payload fields can be
  provided, and state-qualified targets must finish in that exact state.
- Require `construct_put` field metadata to match `T` or the selected enum
  variant by index, name, and reflected type. This is not full provenance, but
  it matches the current `type.field_value` safety model.
- Reject duplicate fields and missing fields in `construct_finish`.
- Validate `TypeVariant` metadata by index, name, discriminant, and payload
  field metadata before starting enum construction.
- Validate `TypeVariant.owner_type` and `TypeMachineState.owner_type` before
  starting enum or state-machine construction so metadata from a different
  owner cannot be accepted by shape alone.
- Return `result[TypeConstruction, string]` from `construct_put` and
  `result[T, string]` from `construct_finish`.
- Do not parse `TypeInfo.type_name` or trust arbitrary user-created `TypeInfo`.

Longer term, trusted metadata provenance should become an internal value fact
rather than a convention reconstructed from public `TypeField` contents.

## Code Touch Points

For the no-syntax builder:

- `crates/jett_types`: added an opaque `TypeConstruction` built-in type.
- `crates/jett_resolve`: pre-registered `TypeConstruction`.
- `crates/jett_typecheck`: added builtin signatures for
  `type.construct_start`, `type.construct_variant_start`,
  `type.construct_machine_start`, `type.construct_put`, and
  `type.construct_finish`.
- `crates/jett_comptime`: added `Value::TypeConstruction` and
  dispatch the construction builtins.
- Refinement validation is checked on `construct_finish`; longer term, the
  builder should share more of the ordinary constructor helper path directly.
- Fixture tests cover successful struct construction, missing fields,
  duplicate fields, wrong field metadata, wrong value type, generic structs, and
  refinement fields. They also cover bitfield construction and bit-width
  validation, plus enum unit and payload variant construction.

For the eventual block syntax:

- `crates/jett_parser::ast`: add a construct-block expression and contextual
  provide statement.
- `crates/jett_parser::parser`: parse `type.construct[T]:` as a block
  expression and ensure postfix `handle error:` still composes.
- `crates/jett_resolve`, `crates/jett_typecheck`, `crates/jett_comptime`, and
  ownership traversal all need to visit the new nodes.

## Recommendation

Keep hardening the `.jett` decoder prototype now that structs, bitfields, and
enums can all be constructed through reflection. The opaque builder is not the
final language shape, but it proves the hard semantic part while staying inside
existing syntax. As the decoder grows through the remaining cases, the case for
a cleaner `type.construct[T]:` block will be grounded in real usage instead of
guesswork.
