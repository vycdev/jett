# Canonical Reflection Metadata Plan

This note records the migration from AST-shaped reflection metadata toward
checked, canonical type metadata.

## Why This Matters

The reflected JSON stdlib now depends on rich metadata:

- `TypeInfo` and structured `TypeKind` / `TypePrimitive`,
- `TypeField` with `serialize_name`, `type_info`, and `has_secret`,
- bitfield layout metadata,
- enum variants and payload fields,
- trusted type-argument and field-type bindings,
- `TypeConstruction` validation.

Most of that works today, but the comptime interpreter still reconstructs much
of it from parsed AST declarations plus generic substitutions. That is good
enough for staging, but it is not the final source of truth.

Reflection should eventually describe exactly the type the checker accepted,
after namespace qualification, alias/refinement resolution, generic
substitution, and any lowering/monomorphization.

## Current Risks

- AST declarations and checked type definitions can drift.
- Namespace-qualified and flat alias names can disagree in subtle cases.
- Generic substitutions are recreated in the interpreter rather than reused
  from a canonical checked instantiation.
- Reflection can expose shape details before there is a single authoritative
  monomorphized type record.
- Future HIR/MIR lowering could change type shape without reflection seeing the
  same view.

JSON is the first feature large enough to feel this pressure, but the same
issue will affect CSV, binary decoding, config loading, doc generation, and
agent queries.

## Target Shape

Reflection should read from a canonical checked metadata table:

```text
CheckedProgram
    types: TypeInterner
    structs: StructDef records keyed by canonical TypeId
    enums: EnumDef records keyed by canonical TypeId
    bitfields: BitfieldDef records keyed by canonical TypeId
    aliases/refinements: canonical base TypeId plus constraint metadata
    namespaces: canonical qualified names
    generic instantiations: substituted canonical TypeId records
```

The comptime interpreter should receive this checked metadata alongside the AST
it executes. Reflection builtins should prefer the checked metadata and only
fall back to AST declarations during early bootstrap tests.

## Staged Migration

### Stage 1: Preserve Check Metadata

Extend the driver/check result path so the interpreter can receive the same
`TypeInterner` and named-type maps produced by typechecking.

Important constraint: this should not make the interpreter mutate checker
state. It should receive an immutable snapshot or a purpose-built reflection
metadata struct.

Status: started. `CheckResult` now carries an immutable
`ReflectionMetadata` snapshot, and driver verify/test paths pass it into the
comptime interpreter. Direct interpreter construction still works without a
snapshot and uses the old fallback path.

### Stage 2: Build A Reflection Metadata Snapshot

Introduce a small type, likely in `jett_types` or `jett_comptime`, that contains
only what reflection needs:

- canonical type display names,
- kind and primitive tags,
- type arguments,
- struct fields,
- bitfield fields and layout,
- enum variants and payload fields,
- alias/refinement base links,
- secret-containment facts.

This keeps the interpreter from depending directly on every checker internal.

Status: started for `TypeInfo` and `TypeField` metadata: display name, kind,
primitive tag, secret-containment, nested type arguments, field order, field
serialize names, and field `TypeInfo` records are captured from the checked
type state. Bitfield layout and variant metadata still use the AST-shaped
interpreter registries.

### Stage 3: Route Reflection Builtins Through The Snapshot

Move these APIs first:

- `type.info`
- `type.arg`
- `type.fields`
- `type.bitfield_layout`
- `type.bitfield_fields`
- `type.variants`

Status: `type.info[T]()`, `type.arg[T](index)`, and `type.fields[T]()` now
prefer the checked snapshot when metadata for the requested type name is
present, and fall back to the previous AST path during bootstrap and direct
interpreter tests.

Then move value-sensitive APIs:

- `type.field_value`
- `type.variant_value`
- `type.variant_field_value`
- `type.construct_*`

Value-sensitive APIs need both canonical metadata and runtime value layout to
agree, so they should move after metadata-only APIs are stable.

### Stage 4: Make Generic Instantiations Canonical

Generic structs and enums should reflect through instantiated canonical records
rather than ad hoc substitutions at each reflection call.

That does not require full native codegen monomorphization first, but it does
require a stable checked representation of `Box[string]` as more than
`Box[T] + {T = string}` in interpreter-local code.

### Stage 5: Remove AST Metadata Fallbacks

Once fixtures and comptime tests are passing through the checked snapshot, remove
the AST reconstruction paths that duplicate typechecker behavior.

## Non-Goals For This Stage

- Do not redesign public JSON shapes while doing this.
- Do not add dynamic `TypedValue` or `any`.
- Do not make reflection depend on HIR/MIR before those phases exist.
- Do not remove trusted metadata checks; canonical metadata should strengthen
  provenance, not replace validation.

## Recommended Next Bite

Continue the metadata-only migration:

1. Add checked `TypeField` records to `ReflectionMetadata`, then move
   `type.fields[T]()` over while preserving trusted loop provenance. Done.
2. Route `TypeInfo.args` trusted loop binding through checked metadata once
   runtime metadata values and typechecker provenance agree on the same source.
3. Add checked bitfield layout/field metadata and enum variant metadata.
4. Only after metadata-only APIs are stable, revisit value-sensitive APIs such
   as `type.field_value` and `type.construct_*`.
