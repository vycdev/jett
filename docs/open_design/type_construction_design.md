# Reflected Type Construction

This note records the design space for reflected construction. The first
builder form has already moved `json.parse[T]` onto the `.jett` stdlib path;
the remaining design question is how this primitive should evolve for future
decoders, generators, and possible syntax cleanup.

## Problem

Jett can now reflect a type's shape and read fields by trusted metadata:

```jett
list[TypeField] fields = type.fields[T]()
value = type.field_value[T, Field](view item, view field)
```

Deserialization needed the reverse operation: given checked field values, build
a `T` while preserving the exact same rules as ordinary source constructors.
That first form now exists as the opaque `TypeConstruction` builder for
structs, bitfields, enum variants, and selected state-machine snapshots; the
final syntax story is still open.

The construction primitive is not JSON-specific. JSON happened to be the first
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

State-machine construction must enforce:

- selecting a declared state by checked `TypeMachineState` metadata,
- state-local payload field lookup,
- duplicate and missing payload-field errors,
- payload field type checking,
- refinement boundary validation for payload values,
- preserving `Machine at state` static precision when the target type is
  state-qualified.

A reflected constructor must not be a weaker back door around these checks. It
should reuse the same typechecker and interpreter rules where possible.
The opaque builder also remembers the aggregate target it was started for:
`type.construct_finish[T](builder)` must use that same target `T`. A builder
started for one struct, bitfield, enum, machine, or state-qualified machine
cannot be finished as a lookalike owner with the same field shape.

## Design Constraints

- **Static types stay central.** Avoid an `any`-like container where values carry
  only runtime `TypeInfo`. Jett's purpose is to make the checker do useful work.
- **Trusted metadata matters.** Compiler-produced `TypeField`, `TypeVariant`,
  and `TypeInfo` can drive construction. User-created structs that merely look
  like metadata must not be trusted. The current typechecker rejects direct
  source constructors for compiler-owned reflection metadata records, with
  `reflection_type_info_constructor.jett`,
  `reflection_field_metadata_constructor.jett`, and
  `reflection_machine_state_metadata_constructor.jett` pinning the type-info,
  field-metadata, and machine-state cases used by construction. Runtime
  construction still validates the supplied metadata contents against the
  selected target.
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
with `/docs/completed/comptime_type_bind.md`.

Verdict: preferred direction, pending syntax.

## Recommended Staging

1. Implement the trusted `comptime type Name = info:` binding from
   `/docs/completed/comptime_type_bind.md`.
2. Done for structs: prototype the no-syntax opaque builder described in
   `/docs/completed/reflected_construction_staging.md`, with `construct_put` returning
   `result[TypeConstruction, string]` and `construct_finish` returning
   `result[T, string]`.
3. Done for bitfields: reuse reflected `TypeField` values, require their
   owner metadata to match the construction target, and validate bit widths on
   `construct_finish`.
4. Done for enums: start from checked `TypeVariant` metadata with
   `type.construct_variant_start[T](variant)`, require its `owner_type` to
   match `T`, then reuse `construct_put` and `construct_finish` for
   variant-local payload fields whose `TypeField.owner_member` matches the
   selected variant.
5. Done for JSON records/enums: public typed parse now routes through the stdlib `JsonTree`
   parser/decoder. Remaining construction work is about hardening syntax and
   reuse for future decoders, not replacing a Rust-backed `json.parse[T]`.
6. Done for machines: add an explicit machine starter that selects checked
   `TypeMachineState` metadata, require its `owner_type` to match the target
   machine, then reuse the existing typed put/finish path for fields whose
   `TypeField.owner_member` matches the selected state.

## Implementation Notes

The current code already has most of the semantic checks in one place:

- typechecking ordinary struct constructors: `check_struct_constructor`,
- typechecking ordinary bitfield constructors: `check_bitfield_constructor`,
- runtime/interpreter struct construction: `construct_struct`,
- runtime/interpreter bitfield construction: `construct_bitfield`,
- runtime/interpreter enum construction: `Value::Enum` construction and
  `reflected_construct_finish`,
- JSON behavior oracle after replacing the typed Rust bridge:
  `stdlib/json/` plus the JSON run-pass/parity fixtures.

A reflected construction implementation should route through these same paths
or share their core helpers. Duplicating the checks risks subtle drift, especially
around refinement result types and bitfield range validation.
The public `type.construct_put[T, Field]` surface is intentionally narrow: it
requires exactly the construction target and field-value type parameters, and a
compiler-produced `TypeField` value at the call site. The compile-fail fixtures
`type_construct_put_wrong_arity.jett` and
`type_construct_put_wrong_field_arg.jett` pin that callers cannot replace
trusted metadata with strings or omit the concrete field type.

The construction primitive should receive values that are already decoded into
their field types. Format modules remain responsible for:

- matching external names such as JSON `serialize_name`,
- deciding whether missing optional fields become `none`,
- choosing whether unknown external keys are ignored or rejected,
- producing format-specific error messages.

This keeps `type.construct` from becoming `json.construct` in disguise.

## Machine Construction Decision

Machine construction should extend the existing builder with a distinct state
starter:

```jett
result[TypeConstruction, string] builder = type.construct_machine_start[T](view state)
```

`T` may be either a bare machine type or a state-qualified `Machine at state`
type. The `state` argument must be a compiler-reflected `TypeMachineState` for
the same machine. After the starter succeeds, callers provide state payload
fields with the existing typed operation:

```jett
builder = type.construct_put[T, Field](builder, view field, decoded) handle error:
    return fail(error)
```

`type.construct_finish[T](builder)` then produces `result[T, string]`. For a
bare machine target, any declared state selected by the starter is valid. For a
state-qualified target, the selected state must exactly match the static state
in `T`; constructing a different state is an error at the finish boundary.

This keeps state selection separate from enum variant selection. Overloading
`type.construct_variant_start` for machines would make two different language
concepts share one spelling, which is especially costly for agents trying to
repair code from local context. A dedicated `construct_machine_start` also lets
diagnostics say "state" instead of "variant" and keeps future transition-aware
construction room to grow without changing enum behavior.

This path is implemented for the current opaque builder. The builder remains
format-agnostic. JSON owns the envelope shape, unknown-key policy,
`serialize_name` matching, and missing optional-field defaults. The construction
primitive receives only checked machine-state metadata and decoded payload
values. Like struct and enum builders, a machine builder records its selected
target owner; finishing it as another machine owner is a construction error.

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
- Should machine construction eventually validate transition effects when the
  input represents an event rather than a full machine snapshot? Ordinary
  `json.parse[Machine]` should not, because JSON snapshots do not carry
  history.

See `/docs/completed/reflected_construction_staging.md` for the current builder surface and
`/docs/open_design/type_construction_block_syntax.md` for the recommended long-term block
syntax direction.
