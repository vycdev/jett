# Reflected Construction Syntax Policy

Status: completed. The explicit `TypeConstruction` builder family is the sole
source form for reflected construction.

## Decision

Jett does not add a `type.construct[T]:` block expression, a contextual
`provide` statement, or another generated-constructor spelling. Reflected code
constructs structs and bitfields explicitly:

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

Enums select a variant with `type.construct_variant_start[T](variant)`, and
machines select a state with `type.construct_machine_start[T](state)`. Both
then use the same `construct_put` and `construct_finish` operations.

## Rationale

- The builder is already the canonical checked operation. A construction block
  would create two source spellings for the same work.
- Builder creation, mutation, failure handling, and finalization remain visible
  and searchable.
- A contextual `provide` keyword would modify hidden compiler-managed state and
  mean something only inside one specialized block.
- Reflection is advanced infrastructure code; saving a few lines there does
  not justify expanding the grammar for ordinary programs.
- The explicit form preserves Jett's preference for one canonical spelling and
  mechanically understandable control flow.

The builder remains opaque and target-bound. A builder started for one owner,
enum variant, or machine state cannot be finished as another target, and
`construct_finish` continues to enforce missing fields, duplicates, refinement
validation, bit widths, and exact selected-state precision.

## Rejected Alternatives

- `type.construct[T]: ... provide field = value`
- a block-scoped `type.provide(...)` call with an implicit receiver
- a generic builder callback that hides the builder lifecycle
- a second generated constructor expression such as `construct with fields`
- heterogeneous dynamic field bags that erase concrete field types

These either duplicate the builder, hide state, or weaken static typing.

## Future Compatibility

A future general block-expression feature does not automatically reopen this
decision. Reflected construction should gain new syntax only if a broader
language-wide abstraction clearly replaces the builder as the sole public
source form in the same migration. The language should not retain both forms.
