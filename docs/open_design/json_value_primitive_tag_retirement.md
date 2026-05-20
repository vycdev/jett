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
- current stdlib JSON raw routing stays simple,
- existing reflection tests remain stable.

Cons:

- raw JSON remains partly compiler-owned,
- users see two different reflection stories for conceptually one raw JSON
  representation,
- future backends must keep honoring a special primitive.

### Option B: Deprecate The Primitive Tag, Keep The Bare Name

Make bare `JsonValue` resolve as a root alias to trusted `json.JsonTree`, and
change `type.info[JsonValue]()` to report alias/enum metadata rather than
`json_value_type`.

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
   only the source-level raw tree spellings. The legacy primitive tag is no
   longer part of normal stdlib JSON dispatch.
3. Change stdlib-loaded reflection for bare `JsonValue` to alias metadata only
   after parse, parse_exact, serialize, serialize_public, raw accessors, and
   container assignment compatibility all pass through the alias path.
   Status: done. `tests/run_pass/json_value_reflection_staging.jett` and
   `tests/run_pass/json_value_reflection_container_metadata.jett` now pin alias
   metadata for bare `JsonValue` while preserving raw behavior.
4. Keep the legacy primitive fallback isolated to bootstrap/no-stdlib
   reflection paths until a later cleanup removes or deprecates
   `TypePrimitive.json_value_type`.
5. Retire `Type::JsonValue` / `TypeInterner::JSON_VALUE` only after bootstrap
   stdlib loading and root aliases no longer need the fallback built-in.

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
