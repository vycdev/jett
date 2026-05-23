# Reflected Construction Block Syntax

This note sketches the eventual surface syntax that could replace the current
opaque `TypeConstruction` builder. The builder has proved the semantics for
structs, bitfields, enum variants, and state-machine snapshots; the remaining
question is what syntax best fits Jett's language principles.

## Goals

- Keep field values statically typed. The syntax must not introduce an
  `any`-like container for decoded values.
- Keep format policy outside reflection. JSON, CSV, binary, and config loaders
  should decide names, missing values, unknown keys, and error text before
  providing fields.
- Make generated work explicit. A reader should be able to see when code is
  iterating over reflected fields and when a concrete type is bound.
- Preserve ordinary construction checks: missing fields, duplicate fields,
  field type mismatch, refinement validation, bitfield width validation, and
  enum payload arity.
- Avoid magic callbacks unless they buy a large readability win.

## Current Low-Level Shape

The current prototype is intentionally explicit:

```jett
mutable TypeConstruction builder = type.construct_start[T]()
for field in type.fields[T]():
    comptime type Field = field.type_info:
        Field decoded = decode_field[Field](raw, view field) handle error:
            return fail(error)
        builder = type.construct_put[T, Field](builder, view field, decoded) handle error:
            return fail(error)
return type.construct_finish[T](builder)
```

Enum payloads use the same builder after selecting a checked variant:

```jett
mutable TypeConstruction builder = type.construct_variant_start[T](view variant) handle error:
    return fail(error)
for field in variant.fields:
    comptime type Field = field.type_info:
        Field decoded = decode_payload[Field](payload, field.index) handle error:
            return fail(error)
        builder = type.construct_put[T, Field](builder, view field, decoded) handle error:
            return fail(error)
return type.construct_finish[T](builder)
```

Machine snapshots use the same builder after selecting a checked state:

```jett
mutable TypeConstruction builder = type.construct_machine_start[T](view state) handle error:
    return fail(error)
for field in state.fields:
    comptime type Field = field.type_info:
        Field decoded = decode_payload[Field](payload, field.serialize_name) handle error:
            return fail(error)
        builder = type.construct_put[T, Field](builder, view field, decoded) handle error:
            return fail(error)
return type.construct_finish[T](builder)
```

This is verbose but valuable as a semantic baseline. It exposes exactly which
operations need to be preserved by future syntax.

## Current Decision

The reflected JSON decoder now uses the builder path in real stdlib code for
records, bitfields, enums, and machines, and the old typed Rust `json.parse[T]`
fallback has been removed from public JSON dispatch. That gives the builder
enough production pressure to keep hardening it, but it does not yet justify
adding new parser syntax.

Keep `TypeConstruction` as the implementation surface until block-expression
syntax and lowering are mature enough to justify a new construction block:

- block expressions, so `type.construct[T]:` has a natural AST home,
- tooling and diagnostics for a new `provide` statement can explain reflected
  field provenance clearly.

Candidate A remains the preferred final surface once those pieces exist.

## Candidate A: `provide` Statement

```jett
result[T, string] built = type.construct[T]:
    for field in type.fields[T]():
        comptime type Field = field.type_info:
            Field decoded = decode_field[Field](raw, view field) handle error:
                return fail(error)
            provide field = decoded
```

For enum variants:

```jett
result[T, string] built = type.construct_variant[T](view variant):
    for field in variant.fields:
        comptime type Field = field.type_info:
            Field decoded = decode_payload[Field](payload, field.index) handle error:
                return fail(error)
            provide field = decoded
```

For machine snapshots:

```jett
result[T, string] built = type.construct_machine[T](view state):
    for field in state.fields:
        comptime type Field = field.type_info:
            Field decoded = decode_payload[Field](payload, field.serialize_name) handle error:
                return fail(error)
            provide field = decoded
```

Pros:

- The syntax says exactly what is happening: the block provides a value for a
  reflected field.
- It avoids an explicit mutable builder in stdlib code.
- It keeps the field metadata visible at the point where the value is supplied.
- It maps cleanly to the existing builder lowering.

Cons:

- It likely requires a new `provide` keyword.
- `provide field = decoded` is a new statement form, not a normal assignment.
- The block must define how early `return`, `handle error:`, duplicate
  providers, and missing providers interact.

## Candidate B: Block-Scoped `type.provide`

```jett
result[T, string] built = type.construct[T]:
    for field in type.fields[T]():
        comptime type Field = field.type_info:
            Field decoded = decode_field[Field](raw, view field) handle error:
                return fail(error)
            type.provide[T, Field](view field, decoded)
```

Pros:

- No new keyword.
- Looks like the existing reflection namespace.
- The type arguments remain explicit, which matches Jett's generic style.

Cons:

- It creates an implicit block-local receiver for `type.provide`, which is less
  visible than the explicit builder.
- It is easy to mistake for an ordinary function call with no side effect.
- The syntax is not much shorter than the current builder.

## Candidate C: Generated Constructor Expression

```jett
result[T, string] built = type.construct[T] with fields:
    field.id = decode_id(raw)
    field.name = decode_name(raw)
```

Pros:

- Pleasant for hand-written structs.
- Could lower almost directly to ordinary named constructors.

Cons:

- It does not serve the reflection use case well because field names are not
  statically written in recursive format code.
- It encourages a second constructor syntax beside `T(field: value)`.
- It does not help enum payload construction unless another variant-specific
  form is added.

## Recommendation

Keep the explicit builder as the implementation primitive for now. For the
eventual language surface, prefer Candidate A: a construction block with a
`provide` statement.

The reason is not brevity. The reason is that `provide` cleanly separates three
things that should stay separate:

- reflection chooses the set of fields,
- format code decodes each `Field`,
- construction validates and assembles `T`.

That maps closely to Jett's core style: explicit, typed, indentation-shaped,
and easy for an agent to inspect.

## Lowering Model

The block can lower mechanically to today's builder:

1. `type.construct[T]:` creates a hidden `TypeConstruction` with
   `type.construct_start[T]()`.
2. `type.construct_variant[T](view variant):` creates a hidden builder with
   `type.construct_variant_start[T](view variant)`.
3. `type.construct_machine[T](view state):` creates a hidden builder with
   `type.construct_machine_start[T](view state)`.
4. Each `provide field = value` lowers to
   `type.construct_put[T, Field](builder, view field, value)`.
5. The block result lowers to `type.construct_finish[T](builder)`.
6. The result type should be `result[T, string]` even when ordinary
   construction could not fail. Reflection-driven construction is primarily for
   decoded data, and explicit error handling is worth the small uniformity cost.

The hidden builder must stay compiler-generated. User code should not be able to
access it or pass it around from the block surface.

## Static Rules

- `provide` is only valid directly inside a `type.construct[...]`,
  `type.construct_variant[...]`, or `type.construct_machine[...]` block.
- The left side must be trusted compiler-produced `TypeField` metadata.
- The right side must typecheck as the concrete field type bound by the nearest
  enclosing `comptime type Field = field.type_info:` block, or an equivalent
  compiler-recognized reflected field binding.
- Providing the same field twice is a compile-time error when statically known,
  and otherwise a construction error.
- Missing fields are reported by construction finish, not by the format module.
- Unknown input keys remain the format module's responsibility.
- `provide` cannot declassify or bypass secret types; it only supplies a value
  whose type is already known.

## Open Questions

- Is a new `provide` keyword acceptable, or should it stay an internal lowering
  until another feature also wants the word?
- Should the construct block always return `result[T, string]`, or mirror
  ordinary constructors by returning plain `T` when no validation can fail?
- Should `provide` accept a variant payload field and a struct field through the
  same `TypeField` type, or should enum payloads eventually get a distinct
  metadata type?
- Should construction blocks support an optional `else` or `missing` clause for
  per-field defaults, or should defaults stay in ordinary format code before
  `provide`?
- How much of the missing/duplicate-field checking can eventually move from
  runtime to compile time once HIR/monomorphization exists?
