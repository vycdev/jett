# JsonValue To JsonTree Transition Plan

`JsonValue` should not remain a Rust-owned language feature. It was useful as a
bridge while reflection and construction were missing, but the long-term shape
should make raw JSON a native Jett value implemented in `stdlib/json.jett`.

The destination is:

- `JsonTree` is the canonical raw JSON representation.
- `JsonValue` becomes a compatibility spelling for that representation, not a
  separate Rust-backed tree.
- Typed `json.parse[T]` continues to be compiler-policy-gated but stdlib-bodied.
- `json.parse_raw` and raw traversal helpers eventually run on `JsonTree`.
- The interpreter/compiler should not depend on `serde_json::Value` for normal
  JSON language behavior.

## Why This Direction

Jett's standard library should be real Jett code wherever the language has
enough expressive power. JSON parsing and tree traversal are language-facing
features, not host-runtime conveniences. Keeping a Rust-only raw JSON value
would create two JSON models:

- `JsonTree`, used by typed parsing and stdlib code.
- `JsonValue`, used by raw accessors and Rust internals.

That split would leak into tests, docs, reflection metadata, and future native
backends. It also makes the raw JSON API harder for users and LLMs to reason
about: two names, two traversal surfaces, one conceptual data model.

## Current State

- `JsonTree` is defined in `stdlib/json.jett`.
- `json_tree_parse(raw)` parses nulls, booleans, numbers, strings, arrays, and
  objects.
- `json_tree_*` traversal helpers mirror the raw `JsonValue` helper surface.
- `json_decode_tree_reflected[T](view raw)` decodes typed values from
  `JsonTree`.
- Public typed `json.parse[T]` now routes through `JsonTree`, except
  `json.parse[JsonValue]`.
- `json.parse_raw` keeps its public `JsonValue` signature but delegates to the
  trusted stdlib `json_tree_parse` hook.
- `json.serialize_raw`, `json.kind`, `json.field`, `json.index`, scalar casts,
  `json.array_length`, and `json.object_keys` dispatch native `JsonTree`
  runtime values through trusted stdlib `json_tree_*` hooks.
- `jett_comptime` no longer has `Value::Json` or a `serde_json` dependency.
- The type system still reports `JsonValue` as a built-in primitive for
  reflection compatibility, but it accepts `JsonValue` and `json.JsonTree` as
  compatible source types for assignments, calls, fields, and container wrappers.
- `json_decode_reflected[T](raw: JsonValue)` is now only a compatibility wrapper
  around `json_decode_tree_reflected[T](view raw)`.

## Compatibility Principle

Do not break user code merely to rename a type.

Existing code like this should keep compiling during the transition:

```jett
JsonValue raw = json.parse_raw(body) handle error:
    return fail(error)
JsonValue name = json.field(raw, "name") handle:
    return fail("missing name")
string text = json.as_string(name) handle error:
    return fail(error)
```

The implementation under that spelling can change from Rust-backed
`serde_json::Value` to native `JsonTree`. The source-level migration can happen
later, with docs preferring `JsonTree` once visibility/export and aliasing are
clean enough.

## Target API Shape

Preferred final shape:

```jett
type JsonValue = JsonTree

function parse_raw(raw: string) returns result[JsonTree, string]
function serialize_raw(view value: JsonTree) returns string
function kind(view value: JsonTree) returns string
function field(view value: JsonTree, key: string) returns optional[JsonTree]
function index(view value: JsonTree, index: int64) returns optional[JsonTree]
function array_length(view value: JsonTree) returns result[int64, string]
function object_keys(view value: JsonTree) returns result[list[string], string]
function as_string(view value: JsonTree) returns result[string, string]
function as_int64(view value: JsonTree) returns result[int64, string]
function as_float64(view value: JsonTree) returns result[float64, string]
function as_bool(view value: JsonTree) returns result[bool, string]
```

Open syntax detail: Jett may not yet have the exact alias/export mechanics to
write `type JsonValue = JsonTree` in the right namespace while preserving the
current unqualified `JsonValue` spelling. Until that exists, `JsonValue` can stay
compiler-recognized as a compatibility name whose runtime representation is
`JsonTree`.

## Alias Direction

Do not implement `JsonValue` by relying on accidental namespace flattening.
Writing `type JsonValue = JsonTree` inside `namespace json` would naturally
create `json.JsonValue`, not the existing unqualified compatibility spelling.
Using the current flat alias leakage would tie a core migration to behavior the
module system is supposed to remove.

The staged direction is:

1. Keep the current narrow compatibility rule while visibility/export work is
   still open. `JsonValue` remains a compiler-known legacy spelling and only the
   bundled `json.JsonTree` enum is compatible with it.
2. Add an explicit compiler-seeded compatibility alias table entry:
   `JsonValue -> json.JsonTree`. This should model a future prelude/exported
   alias without depending on namespace leakage.
3. Preserve legacy reflection during the compatibility window:
   `type.info[JsonValue]()` may continue to report
   `TypePrimitive.json_value_type`, while `type.info[json.JsonTree]()` reports
   enum metadata.
4. Once explicit stdlib exports or prelude imports exist, express the
   compatibility alias as an exported/prelude stdlib symbol.
5. In a later breaking cleanup, decide whether to deprecate or remove
   `TypePrimitive.json_value_type` and make `JsonValue` fully identical to
   `json.JsonTree` in reflection.

This keeps canonical identity clear: `json.JsonTree` is the real type;
`JsonValue` is a source-compatibility spelling.

## Staging Plan

### 1. Add Native Raw Helper Parity

Add stdlib helper names that match the current raw API behavior but operate on
`JsonTree`:

- `json_tree_serialize_raw(view value: JsonTree)` or make
  `json_tree_serialize` view-friendly.
- `json_tree_kind`, `json_tree_field`, `json_tree_index`,
  `json_tree_array_length`, `json_tree_object_keys`, and scalar casts already
  exist.
- Add focused parity fixtures comparing current `JsonValue` helpers to
  `JsonTree` helpers for the same inputs.

Important implementation note: view-friendly serialization currently needs
either iteration over viewed lists/maps or a deliberate materialization/cloning
primitive. Do not paper over that with host magic.

Status: first parity fixture is in place in
`tests/run_pass/json_raw_tree_parity.jett`, covering object traversal, array
lookup, nulls, booleans, strings, numbers, wrong-shape errors, absent lookup,
and raw serialization against the native tree helpers.

### 2. Route Public Raw Functions Through Trusted Stdlib Hooks

Keep compiler/typechecker signatures stable, but make interpreter execution
delegate to trusted stdlib functions where possible:

- `json.parse_raw(raw)` delegates to `json.json_tree_parse(raw)`.
- `json.serialize_raw(value)` delegates to native tree serialization.
- `json.kind`, `json.field`, `json.index`, scalar casts, length, and keys
  delegate to the corresponding tree helpers.

At this stage the typechecker may still say the parameter/return type is
`JsonValue`, but the interpreter should hold a native tree value.

Status: implemented. The interpreter delegates `json.parse_raw` to trusted
`json.json_tree_parse`, and raw helpers dispatch native `JsonTree` values
through trusted `json.json_tree_*` hooks.

### 3. Change Runtime Representation

Runtime raw JSON now reuses the existing enum/struct `Value` shape produced by
`JsonTree` values. No separate `Value::JsonTree` wrapper was needed, and
`Value::Json(serde_json::Value)` was removed outright.

The important invariant now holds: raw JSON operations no longer depend on
`serde_json::Value` in the comptime interpreter.

### 4. Make `JsonValue` A Compatibility Name

Once the runtime representation is native:

- Treat `JsonValue` and `json.JsonTree` as the same type for assignment and
  calls, or make `JsonValue` an alias to `JsonTree`.
- Preserve `type.info[JsonValue]()` enough for existing reflection tests.
- Decide whether `TypePrimitive.json_value_type` should remain for
  compatibility or become an alias/wrapper tag around `TypeKind.enum_type`.

Recommendation: keep `TypePrimitive.json_value_type` for one compatibility
stage, but document it as legacy once `JsonTree` is the preferred spelling.

Status: implemented as a narrow typechecker compatibility rule rather than a
source-level alias. Only the stdlib enum `json.JsonTree` is compatible with the
built-in `JsonValue`; user-defined enums named `JsonTree` remain unrelated.
Reflection metadata is intentionally split for now:
`type.info[JsonValue]()` reports `TypePrimitive.json_value_type`, while
`type.info[json.JsonTree]()` reports `TypeKind.enum_type`.

Next status target: replace the ad hoc compatibility rule with a
compiler-seeded legacy alias record once the typechecker has a small place to
store compatibility/prelude aliases separately from ordinary source aliases.

### 5. Move Raw Decoder Code Off `JsonValue`

`stdlib/json.jett` still contains the older `json_decode_reflected[T](raw:
JsonValue)` path. After raw APIs use `JsonTree`:

- Replace internal uses with `json_decode_tree_reflected[T]`.
- Keep a thin `json_decode_reflected[T]` compatibility wrapper only if needed.
- Remove duplicate `JsonValue` decoder helpers once no tests or public bridges
  rely on them.

This should substantially reduce `stdlib/json.jett` duplication.

Status: implemented. The old duplicate `JsonValue` decoder helper family was
removed; the public compatibility entrypoint delegates to the `JsonTree`
decoder.

### 6. Remove Rust JSON Fallbacks

After parity tests pass with native representation:

- Done: remove `serde_json::Value` from `jett_comptime::value::Value`.
- Done: remove Rust-backed `json.parse_raw` and raw accessor implementations.
- Keep Rust only for tests/dev tooling if useful, not as language semantics.

## Required Tests

Add tests before each behavior change:

- `json.parse_raw` returns a value that raw accessors can traverse.
- `json.parse[JsonValue]` still works during compatibility.
- `JsonValue` and `JsonTree` assignment/alias behavior once enabled.
- Raw object lookup preserves absence semantics.
- Wrong-shape lookup returns `none`; wrong-shape shape-requiring access returns
  `result` errors.
- Raw serialization matches `JsonTree` serialization.
- Malformed-input diagnostics stay pinned.
- Reflection metadata remains intentional for `JsonValue` and `JsonTree`.
- A user-defined top-level `enum JsonTree` remains incompatible with
  `JsonValue`.
- A compiler-seeded `JsonValue` compatibility alias works recursively in
  `list`, `map`, `set`, `optional`, `result`, `secret`, struct fields, and
  function arguments.
- `type.info[JsonValue]()` and `type.info[json.JsonTree]()` stay pinned during
  the compatibility stage.

## Risks And Open Questions

- **Alias mechanics:** ordinary source aliases are not enough for the current
  unqualified compatibility spelling. We likely need a compiler-seeded prelude
  alias first, then a real exported/prelude stdlib alias later.
- **Reflection metadata:** if `JsonValue` is an alias, `type.info[JsonValue]`
  must not surprise existing code.
- **View iteration:** native raw serialization over `view JsonTree` needs a
  principled solution for iterating viewed lists/maps.
- **Performance:** the self-hosted parser is correctness-first today. Native
  backends can optimize later, but we should avoid introducing needless
  materialization in the language semantics.
- **API naming:** `JsonTree` is honest for implementation; `JsonValue` may be
  friendlier for users. The final public spelling can be decided separately from
  the representation.

## Recommended Next Implementation Bite

Finish the public surface decision:

1. Keep the current compatibility rule until namespace exports exist, but
   design a small compiler-seeded legacy alias table that can represent
   `JsonValue -> json.JsonTree` explicitly.
2. Decide whether raw helper signatures should remain `JsonValue` for source
   stability or move to `view JsonTree` in a breaking cleanup pass.
3. Once explicit exports/prelude imports exist, move the compatibility alias
   out of compiler special cases and into the stdlib/prelude surface.
4. Later, update reflection metadata so the legacy
   `TypePrimitive.json_value_type` is either formally deprecated or removed.
