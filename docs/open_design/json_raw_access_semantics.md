# Raw JSON Access Semantics

This note records the open policy question around `json.field` and
`json.index`.

## Current Behavior

Raw JSON lookup is intentionally lenient today:

```jett
JsonValue field = json.field(root, "name") handle:
    return "absent"
```

Both of these cases return `none`:

- the object or array does not contain the requested field/index,
- the input value has the wrong shape, such as `json.field("text", "name")`.

Shape-requiring helpers are stricter:

- `json.object_keys(value)` returns `result[list[string], string]`,
- `json.array_length(value)` returns `result[int64, string]`,
- scalar casts such as `json.as_int64(value)` return `result[T, string]`.

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
function field(view value: JsonTree, key: string) returns optional[JsonTree]
function index(view value: JsonTree, index: int64) returns optional[JsonTree]
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
function field(view value: JsonTree, key: string) returns result[optional[JsonTree], string]
function index(view value: JsonTree, index: int64) returns result[optional[JsonTree], string]
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
function require_field(view value: JsonTree, key: string) returns result[JsonTree, string]
function require_index(view value: JsonTree, index: int64) returns result[JsonTree, string]
function object_field(view value: JsonTree, key: string) returns result[optional[JsonTree], string]
function array_index(view value: JsonTree, index: int64) returns result[optional[JsonTree], string]
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

## Recommendation

Prefer Option C.

It fits Jett's "one obvious pattern per intent" principle better than changing
`field` into a nested result/optional:

- use `json.field` / `json.index` when probing unknown raw JSON,
- use `json.require_field` / `json.require_index` when a value must be present,
- use `json.object_field` / `json.array_index` when shape must be correct but
  absence is meaningful.

The next implementation bite should add the strict helpers in
`stdlib/json.jett`, keep the existing lenient helpers unchanged, and add parity
tests that pin wrong-shape and missing-key/index diagnostics separately.
