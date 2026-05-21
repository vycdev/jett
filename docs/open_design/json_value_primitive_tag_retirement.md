# JsonValue Primitive Tag Retirement

Status: open design.

`json.JsonTree` is now the canonical raw JSON representation. The stdlib also
exports `json.JsonValue = JsonTree` and the narrow root alias
`JsonValue = json.JsonTree`. In stdlib-loaded code both aliases reflect as
aliases to `json.JsonTree`, while the compiler still keeps a legacy built-in
`JsonValue` primitive for bootstrap/no-stdlib fallback paths.

This note records what must be true before removing or deprecating that legacy
primitive tag.

## Current Dependency Map

- `jett_types` still has `Type::JsonValue` and `TypeInterner::JSON_VALUE`.
- The typechecker lets the stdlib root alias win for the bare name `JsonValue`
  when the bundled stdlib is loaded. The extra compiler-owned compatibility
  table between legacy `JsonValue` and trusted `json.JsonTree` has been
  removed.
- Raw JSON facade signatures come from exported stdlib wrappers in normal
  stdlib-loaded code. In no-stdlib direct-interpreter contexts the raw public
  names are undefined rather than backed by hidden Rust semantics.
- The interpreter's direct/no-metadata fallback still reports the
  `json_value_type` primitive tag for bare `JsonValue` reflection.
- The typechecker no longer treats the bare text `JsonValue` as primitive while
  building `TypeInfo`; the tag is produced only when type resolution reaches the
  legacy `Type::JsonValue` fallback.
- The stdlib JSON serializer and decoder route raw targets by reflected type
  name: `json.JsonTree`, `json.JsonValue`, and bare `JsonValue`. They no
  longer depend on `TypePrimitive.json_value_type`.
- Tests intentionally pin both sides of the staged split:
  `JsonValue` remains a compatibility spelling, while `json.JsonValue` is an
  alias to `json.JsonTree`.

## Goal

Bare `JsonValue` should eventually be a source-compatibility spelling for
`json.JsonTree`, not a separate language primitive. New code should learn
`json.JsonTree` first. Old code should continue compiling through an explicit
compatibility window.

## Options

### Option A: Keep The Primitive Tag Longer

Keep `TypePrimitive.json_value_type` and document it as a legacy primitive.

Pros:

- zero migration risk,
- bootstrap/no-stdlib reflection stays simple,
- existing reflection tests remain stable.

Cons:

- raw JSON remains partly compiler-owned,
- users see two different reflection stories for conceptually one raw JSON
  representation,
- future backends must keep honoring a special primitive.

### Option B: Finish Deprecating The Primitive Tag, Keep The Bare Name

Keep bare `JsonValue` resolving as a root alias to trusted `json.JsonTree` in
stdlib-loaded code, then remove the remaining bootstrap/no-stdlib primitive
reflection behavior once that fallback has a replacement story.

Pros:

- matches the intended language model,
- removes a JSON-specific primitive from normal reflection,
- keeps source compatibility for old code.

Cons:

- code using `type.primitive_tag[JsonValue]()` must migrate,
- stdlib JSON must route bare raw targets without relying on the primitive tag,
- bootstrap/no-stdlib paths need a clear fallback story.

### Option C: Remove The Bare Name

Eventually require `json.JsonTree` or `json.JsonValue` in source.

Pros:

- simplest final model,
- no root alias special case.

Cons:

- breaks existing code,
- premature until the import/prelude/root-alias story is settled.

Do not do this as the next step.

## Safe Staging

1. Keep bare `JsonValue` concentrated in compatibility/parity fixtures and
   transition docs.
2. Add a stdlib JSON raw-target helper that treats `json.JsonTree`,
   `json.JsonValue`, and bare `JsonValue` as raw JSON without depending only on
   `TypePrimitive.json_value_type`.
   Status: done. `stdlib/json/` now routes reflected serialize/decode raw
   targets through `json_reflected_raw_type(...)`, and that helper recognizes
   `json.JsonTree` plus aliases that resolve to it through `TypeInfo.args`.
   The legacy primitive tag is no longer part of normal stdlib JSON dispatch.
3. Change stdlib-loaded reflection for bare `JsonValue` to alias metadata only
   after parse, parse_exact, serialize, serialize_public, raw accessors, and
   container assignment compatibility all pass through the alias path.
   Status: done. `tests/run_pass/json_value_reflection_staging.jett` and
   `tests/run_pass/json_value_reflection_container_metadata.jett` now pin alias
   metadata for bare `JsonValue` while preserving raw behavior.
4. Keep the legacy primitive fallback isolated to bootstrap/no-stdlib
   reflection paths until a later cleanup removes or deprecates
   `TypePrimitive.json_value_type`.
   Status: in progress. The typechecker now narrows `json_value_type` to the
   resolved legacy fallback type instead of a textual `JsonValue` shortcut; the
   direct interpreter fallback and enum variant remain.
5. Retire `Type::JsonValue` / `TypeInterner::JSON_VALUE` only after bootstrap
   stdlib loading and root aliases no longer need the fallback built-in.

## Retirement Checklist

The alias-table removal is done. The remaining primitive behavior is the
bootstrap/no-stdlib fallback pinned by the direct interpreter reflection tests.

Before removing `TypePrimitive.json_value_type`:

1. Decide what direct/no-stdlib reflection should report for bare `JsonValue`
   when no bundled stdlib alias has been registered.
2. Update the typechecker fallback in unresolved-name type lookup so it no
   longer maps bare `JsonValue` to `TypeInterner::JSON_VALUE`, or keep that path
   as an explicit deprecated compatibility mode.
3. Update comptime direct reflection so `type.kind[JsonValue]()` and
   `type.primitive_tag[JsonValue]()` no longer synthesize the legacy primitive
   when the alias is absent.
4. Remove the `TypePrimitive.json_value_type` variant only after all stdlib
   JSON routing, public raw facade signatures, and compatibility tests are
   source-alias based.
5. Remove `Type::JsonValue` / `TypeInterner::JSON_VALUE` last, after bootstrap
   stdlib loading no longer needs an internal placeholder for the old spelling.

## Tests That Must Stay Green

- `tests/run_pass/json_value_tree_compatibility.jett`
- `tests/run_pass/json_parse_error_parity.jett`
- `tests/run_pass/json_parse_exact.jett`
- `tests/run_pass/json_raw_tree_parity.jett`
- `tests/run_pass/json_serialize_public.jett`
- `tests/run_pass/type_info_reflection.jett`
- `tests/compile_fail/prelude_json_value_unavailable.jett`
- `tests/compile_fail/json_value_user_json_tree_incompatible.jett`

## Open Questions

- Should `TypePrimitive.json_value_type` become a deprecated enum variant for a
  while, or disappear from `TypePrimitive` in the same release as the behavior
  change?
- How should no-stdlib/bootstrap interpreter tests represent raw JSON once the
  built-in primitive is gone?
- Should `json.JsonValue` remain as the friendly public alias, or should docs
  eventually teach only `json.JsonTree`?
- Should root aliases grow beyond the allowlisted `JsonValue` bridge, or stay a
  compatibility-only mechanism?
