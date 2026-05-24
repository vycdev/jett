# Raw JSON Access Semantics

Status: implemented for the helper split; long-term naming/default guidance
remains open.

This note records the staged policy decision around `json.field` and
`json.index`.

## Current Behavior

Raw JSON lookup is intentionally lenient today:

```jett
json.JsonTree field = json.field(view root, "name") handle:
    return "absent"
```

Both of these cases return `none`:

- the object or array does not contain the requested field/index,
- the input raw JSON value has the wrong shape, such as probing a
  `json.JsonTree.string_value("text")` as though it were an object.

Shape-requiring helpers are stricter:

- `json.object_keys(value)` returns `result[list[string], string]`,
- `json.array_length(value)` returns `result[int64, string]`,
- scalar casts such as `json.as_int64(value)` and `json.as_uint64(value)`
  return `result[T, string]`.
- strict raw lookup helpers now exist for production-style validation:
  `json.object_field`, `json.array_index`, `json.require_field`, and
  `json.require_index`.

So the current split is:

- lookup helpers are probing operations,
- shape/cast helpers are validation operations.

## Problem

The lenient lookup shape is convenient for exploratory JSON traversal, but it
collapses two different facts:

- "this object does not have key `name`",
- "this value is not an object".

That can hide bugs in production parsers. It is also less teachable for LLMs:
an agent may accidentally call `json.field` on the wrong value and handle the
failure as though a key were merely optional.

## Options

### Option A: Keep `field` / `index` Lenient

Keep:

```jett
function field(view value: json.JsonTree, key: string) returns optional[json.JsonTree]
function index(view value: json.JsonTree, index: int64) returns optional[json.JsonTree]
```

Pros:

- preserves existing source behavior,
- simple for optional probing,
- mirrors common dynamic JSON APIs.

Cons:

- wrong-shape and absent lookup stay indistinguishable,
- production decoding must remember to validate shape separately.

### Option B: Change `field` / `index` To Result-Wrapped Optional

Change to:

```jett
function field(view value: json.JsonTree, key: string) returns result[optional[json.JsonTree], string]
function index(view value: json.JsonTree, index: int64) returns result[optional[json.JsonTree], string]
```

Pros:

- separates wrong shape from absence,
- keeps optional field semantics explicit.

Cons:

- breaks existing code,
- nested `result[optional[T], string]` is wordy at every call site,
- makes casual raw traversal noisy.

### Option C: Keep Lenient Lookup, Add Strict Helpers

Keep `field` / `index` as probing helpers and add strict helpers:

```jett
function require_field(view value: json.JsonTree, key: string) returns result[json.JsonTree, string]
function require_index(view value: json.JsonTree, index: int64) returns result[json.JsonTree, string]
function object_field(view value: json.JsonTree, key: string) returns result[optional[json.JsonTree], string]
function array_index(view value: json.JsonTree, index: int64) returns result[optional[json.JsonTree], string]
```

`require_*` would fail on both wrong shape and absence. `object_field` /
`array_index` would fail on wrong shape but return `none` for absence.

Pros:

- preserves compatibility,
- gives production code a precise API,
- lets the typed decoder use strict helpers internally,
- keeps the simple probing API available for exploratory code.

Cons:

- adds more names to the JSON surface,
- requires clear documentation so LLMs pick the strict helper for validation.

## Decision

Option C is implemented.

It fits Jett's "one obvious pattern per intent" principle better than changing
`field` into a nested result/optional:

- use `json.field` / `json.index` when probing unknown raw JSON,
- use `json.require_field` / `json.require_index` when a value must be present,
- use `json.object_field` / `json.array_index` when shape must be correct but
  absence is meaningful.

The implemented path keeps the existing lenient helpers unchanged and adds the
strict helpers for code that needs shape validation. Treat the future question
as documentation and compatibility guidance, not as an unresolved type or
stdlib surface gap.

Status: implemented. The strict helper surface is pinned in
`tests/run_pass/json_raw_strict_accessors.jett`, while lenient probing edge
cases are pinned in `tests/run_pass/json_raw_value_access_edges.jett`.
Argument-shape diagnostics are pinned in
`tests/compile_fail/json_raw_strict_accessor_argument_shapes.jett` and
`tests/compile_fail/json_raw_facade_argument_shapes.jett`. The public strict
wrappers now share private `JsonTree`-level helpers with reflected decoding and
exact validation, so the stdlib has one shape-vs-absence vocabulary internally.
Handle-policy diagnostics for the optional probing helpers and strict
result-returning helpers are pinned in
`tests/compile_fail/json_raw_probe_facades_require_handle.jett` and
`tests/compile_fail/json_raw_result_facades_require_handle.jett`.
The remaining design question is whether the lenient `json.field` /
`json.index` names should stay as the primary public spelling forever, or
whether a later compatibility stage should guide users toward the stricter
helpers for most production code. The current JSON transition docs explicitly
describe `field` / `index` as probing helpers and the strict helpers as the
production validation surface.
