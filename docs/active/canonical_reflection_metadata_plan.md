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

Status: started for `TypeInfo`, `TypeField`, bitfield metadata, and enum
variant metadata: display name, kind, primitive tag, secret-containment, nested
type arguments, field order, field serialize names, field `TypeInfo` records,
bitfield network order, bitfield shapes, widths, bitfield enum annotations,
variant discriminants, and variant payload fields are captured from the checked
type state.

### Stage 3: Route Reflection Builtins Through The Snapshot

Move these APIs first:

- `type.info`
- `type.arg`
- `type.fields`
- `type.bitfield_layout`
- `type.bitfield_fields`
- `type.variants`

Status: `type.name[T]()`, `type.kind[T]()`, `type.kind_tag[T]()`,
`type.primitive_tag[T]()`, `type.has_secret[T]()`, `type.info[T]()`,
`type.arg[T](index)`, `type.fields[T]()`, `type.bitfield_layout[T]()`,
`type.bitfield_fields[T]()` and `type.variants[T]()` now prefer the checked
snapshot when metadata for the requested type name is present, and fall back to
the previous AST path during bootstrap and direct interpreter tests. Trusted
`comptime type` bindings over direct `type.arg[T](index)`, `TypeInfo.args`,
`type.fields[T]()` loops, and `type.variants[T]()` / variant payload loops also
prefer checked metadata when constructing the compile-time type binding scope.
When checked `TypeInfo` identifies an owner as field- or variant-bearing, those
loop-scope bindings now surface missing checked owner metadata instead of
silently reconstructing trusted scopes from AST.
The interpreter's public `json.serialize[T]` secret-containing type gate also
uses the checked secret-containment fact before falling back to the older
interpreter registry helper.

Then move value-sensitive APIs:

- `type.field_value`
- `type.variant_value`
- `type.variant_field_value`
- `type.construct_*`

Value-sensitive APIs need both canonical metadata and runtime value layout to
agree, so they should move after metadata-only APIs are stable.

Status: `type.field_value[T, Field]` now prefers checked `TypeField` metadata
for field provenance and type validation, `type.variant_value[T]` now prefers
checked `TypeVariant` metadata for active variant lookup, and
`type.variant_field_value[T, Field]` now prefers checked variant payload
metadata for payload field provenance and type validation. All retain the AST
fallback for bootstrap/direct interpreter tests. `type.construct_variant_start`
and `type.construct_put` now prefer checked metadata for construction kind,
field, and variant payload validation while preserving the existing
`TypeConstruction` builder layout. `type.construct_finish` now uses checked
metadata to materialize structs, enums, and bitfields, including bitfield width
checks and enum annotation discriminants.

### Stage 4: Make Generic Instantiations Canonical

Generic structs and enums should reflect through instantiated canonical records
rather than ad hoc substitutions at each reflection call.

That does not require full native codegen monomorphization first, but it does
require a stable checked representation of `Box[string]` as more than
`Box[T] + {T = string}` in interpreter-local code.

Status: started. `ReflectionMetadata` now has a canonical `TypeId` lookup
scaffold alongside its legacy string maps. The typechecker seeds canonical
`TypeInfo` entries by checked `TypeId`, including real named owners and
refinements while keeping simple aliases string-only so they do not collapse
into their base type's reflection identity. The checker now stores
struct/generic-struct fields, bitfield layout metadata, and enum variants by the
known owner `TypeId` at the construction sites. The string-shaped
`ReflectionMetadata` API remains as a compatibility facade, but owner metadata
lookups can resolve through canonical id maps after a name is bound. For
field-bearing checked types, the interpreter no longer silently reconstructs
field, bitfield, or enum variant metadata from AST when a checked `TypeInfo`
says the owner should have that metadata but the checked table is missing. The
same completeness rule now applies to value-sensitive field/variant access and
reflected construction paths, so missing checked owner metadata is surfaced
instead of masked by interpreter-local AST reconstruction. Bitfield enum
annotations now follow the same rule: if checked metadata says the referenced
enum exists but variant metadata is absent, enum discriminant lookup reports the
metadata gap instead of falling back to the interpreter enum registry.
Duplicate-leaf reflected construction is pinned in
`tests/run_pass/reflection_type_id_duplicate_construction.jett`, which builds
`accounts.Record` and `audit.Record` through `type.construct_*` using their
own checked field metadata.
Duplicate-leaf built-in container arguments are pinned in
`tests/run_pass/reflection_type_id_duplicate_container_args.jett`, covering
`TypeInfo.args`, direct `type.arg[...]`, nested struct field container args,
and JSON parse/parse_exact/serialize paths through `map`, `optional`, and
`result` shapes whose payload types share leaf names across namespaces.
Duplicate-leaf enum payloads and bitfield enum annotations are also covered by
value-sensitive reflection reads in
`tests/run_pass/reflection_type_id_duplicate_enum_payloads.jett` and
`tests/run_pass/reflection_type_id_duplicate_named_owners.jett`.
`tests/run_pass/json_tree_reflection_variant_metadata.jett` pins the
stdlib-owned `json.JsonTree` enum variant metadata, including variant names,
payload field names, container type names, nested `json.JsonTree` arguments,
primitive tags, and value-sensitive `type.variant_value` /
`type.variant_field_value` payload reads.
`tests/run_pass/json_tree_reflection_construction.jett` pins the same stdlib
enum as a construction target through `type.construct_variant_start`,
`type.construct_put`, and `type.construct_finish`, including null, string,
number, array, and object variants.
`ReflectionMetadata::insert_type_info` now also refreshes an existing canonical
`TypeId` binding when the inserted type name is already bound, matching the
owner-metadata insertion behavior.
`ReflectionMetadata::bind_type_name` now promotes any existing string-keyed type
info, fields, bitfield layout, or variant metadata into the canonical `TypeId`
tables when the binding is added after the metadata. The metadata API also
exposes explicit `type_id_for_name` and `get_*_for_id` accessors, and the
comptime interpreter's checked reflection helpers use those id accessors first
before preserving the legacy string fallback. The typechecker no longer keeps
parallel string-only owner metadata maps for checked struct fields, bitfield
layouts, or enum variants; production owner metadata now flows through the
`TypeId` keyed path and relies on `ReflectionMetadata` to preserve legacy name
lookup compatibility.
Monomorphized generic struct `TypeInfo` now also gets concrete type arguments
from the checker-owned `(generic owner, TypeId args) -> concrete TypeId` cache
during canonical TypeId metadata construction. The old ambiguous string-only
leaf insertion for names such as `Box[int64]` has been removed from checked
metadata; namespaced generic owners keep their qualified identities such as
`accounts.Box[int64]` and `audit.Box[int64]`.

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

Move metadata identity from display-string keys toward canonical checked type
records or `TypeId`s, while preserving documented bootstrap/direct-interpreter
fallback paths.

The remaining AST fallback users have been audited. They are intentionally kept
for direct interpreter tests and bootstrap runs without a checked
`ReflectionMetadata` snapshot. The tightening target is no longer "remove every
AST path"; it is "when checked metadata exists for a program, avoid silently
masking missing owner metadata with AST reconstruction."

Concrete follow-up:

1. Keep auditing future `ReflectionMetadata` insertions: simple aliases should
   stay string-only unless they get their own source-level identity; refinements
   and canonical owners should stay id-bound.
2. Keep no-metadata interpreter mode working for unit tests and bootstrap.
3. Apply the same "checked owner metadata must be complete" rule to any future
   value-sensitive reflection operations as they are added.
