# Reflected Construction Staging

This note records the construction step after field/variant reflection and raw
`json.JsonTree` support. The construction primitive stays format-agnostic and
statically typed.

## Final Source Form: Opaque Builder

The explicit builder that began as the no-syntax staging bridge is now the sole
canonical reflected-construction source form:

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

The builder is intentionally explicit and opaque: user code can add typed
values, but cannot read heterogeneous values back out. Jett does not also
provide a construction block or contextual `provide` statement.

## Why The Builder Is The Canonical Form

- `type.construct_put[T, Field]` typechecks the provided value as the concrete
  `Field` type, so user code does not get an `any` lane.
- The builder receives already-decoded values; JSON-specific policy such as
  `serialize_name`, missing optional defaults, and unknown-key handling stays in
  `json`.
- `type.construct_finish[T]` can return `result[T, string]` uniformly, giving
  callers the same explicit `handle error:` boundary used by `json.parse[T]`.
- Builder creation, mutation, failure handling, and finalization remain visible
  and searchable, preserving one canonical spelling.

## Guardrails For The Prototype

The current implementation is still deliberately narrow:

- Supports structs, bitfields, and enum variants. Enums require
  `type.construct_variant_start[T](variant)` because a variant must be selected
  before payload fields can be provided.
- Machine construction uses `type.construct_machine_start[T](state)` rather
  than the enum starter; a state must be selected before payload fields can be
  provided, and state-qualified targets must finish in that exact state.
- Require `construct_put` field metadata to match `T`, the selected enum
  variant, or the selected machine state by owner, index, name, and reflected
  type.
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

## Recommendation

Keep hardening the explicit builder as future decoders reuse it. Do not add a
parallel `type.construct[T]:` block, contextual `provide` statement, or callback
spelling. See [Reflected construction syntax policy](type_construction_block_syntax.md)
for the completed canonical-form decision.
