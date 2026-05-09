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
- `json.parse_raw`, `json.serialize_raw`, `json.kind`, `json.field`,
  `json.index`, scalar casts, `json.array_length`, and `json.object_keys` still
  operate on the Rust-backed `JsonValue`.
- The comptime interpreter still has `Value::Json(serde_json::Value)`.
- The type system still treats `JsonValue` as a built-in primitive.

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

### 2. Route Public Raw Functions Through Trusted Stdlib Hooks

Keep compiler/typechecker signatures stable, but make interpreter execution
delegate to trusted stdlib functions where possible:

- `json.parse_raw(raw)` delegates to `json.json_tree_parse(raw)`.
- `json.serialize_raw(value)` delegates to native tree serialization.
- `json.kind`, `json.field`, `json.index`, scalar casts, length, and keys
  delegate to the corresponding tree helpers.

At this stage the typechecker may still say the parameter/return type is
`JsonValue`, but the interpreter should hold a native tree value.

### 3. Change Runtime Representation

Replace `Value::Json(serde_json::Value)` with a representation that stores the
same value shape as `JsonTree`.

Reasonable intermediate options:

- Reuse the existing enum/struct `Value` shape produced by `JsonTree` values.
- Introduce `Value::JsonTree(...)` only as an interpreter implementation detail.
- Keep `Value::Json(...)` temporarily but change its payload away from
  `serde_json::Value`.

The important invariant is that raw JSON operations no longer depend on
`serde_json::Value`.

### 4. Make `JsonValue` A Compatibility Name

Once the runtime representation is native:

- Treat `JsonValue` and `json.JsonTree` as the same type for assignment and
  calls, or make `JsonValue` an alias to `JsonTree`.
- Preserve `type.info[JsonValue]()` enough for existing reflection tests.
- Decide whether `TypePrimitive.json_value_type` should remain for
  compatibility or become an alias/wrapper tag around `TypeKind.enum_type`.

Recommendation: keep `TypePrimitive.json_value_type` for one compatibility
stage, but document it as legacy once `JsonTree` is the preferred spelling.

### 5. Move Raw Decoder Code Off `JsonValue`

`stdlib/json.jett` still contains the older `json_decode_reflected[T](raw:
JsonValue)` path. After raw APIs use `JsonTree`:

- Replace internal uses with `json_decode_tree_reflected[T]`.
- Keep a thin `json_decode_reflected[T]` compatibility wrapper only if needed.
- Remove duplicate `JsonValue` decoder helpers once no tests or public bridges
  rely on them.

This should substantially reduce `stdlib/json.jett` duplication.

### 6. Remove Rust JSON Fallbacks

After parity tests pass with native representation:

- Remove `serde_json::Value` from `jett_comptime::value::Value`.
- Remove Rust-backed `json.parse_raw` and raw accessor implementations.
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

## Risks And Open Questions

- **Alias mechanics:** the language may need better stdlib export/alias support
  before `JsonValue = JsonTree` can be expressed cleanly.
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

Start with parity, not representation surgery:

1. Add a `json_raw_tree_parity.jett` fixture comparing `json.parse_raw` and
   `json_tree_parse` behavior on objects, arrays, nulls, numbers, strings, and
   wrong-shape access.
2. Make `json.serialize_raw` parity explicit against `json_tree_serialize`.
3. Add a small design note or issue for view-friendly `JsonTree` serialization.

Once parity is pinned, route one raw helper at a time through native `JsonTree`
while preserving the current public signatures.
