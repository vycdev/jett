# JsonValue To JsonTree Transition Plan

`JsonValue` should not remain a Rust-owned language feature. It was useful as a
bridge while reflection and construction were missing, but the long-term shape
should make raw JSON a native Jett value implemented in the stdlib `json`
module.

The destination is:

- `JsonTree` is the canonical raw JSON representation.
- `JsonValue` becomes a compatibility spelling for that representation, not a
  separate Rust-backed tree.
- Typed `json.parse[T]` continues to be compiler-policy-gated but stdlib-bodied.
- `json.parse_raw` and raw traversal helpers run on `JsonTree`.
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

- `JsonTree` is defined in the stdlib `json` module.
- `json_tree_parse(raw)` parses nulls, booleans, numbers, strings, arrays, and
  objects.
- `json_tree_*` traversal helpers mirror the raw `JsonValue` helper surface.
- `json_decode_tree_reflected[T](view raw)` decodes typed values from
  `JsonTree`.
- Public typed `json.parse[T]` now routes through `JsonTree`, including the
  legacy-compatible `json.parse[JsonValue]` branch and the canonical
  `json.parse[json.JsonTree]` identity path.
- `json.parse_raw` now prefers a public `JsonTree` signature whenever the
  bundled stdlib type is loaded; `JsonValue` code still compiles through the
  legacy compatibility spelling.
- `json.serialize_raw`, `json.kind`, `json.field`, `json.index`, scalar casts,
  `json.array_length`, and `json.object_keys` are `JsonTree`-first facade
  signatures with `JsonValue` compatibility. Runtime values dispatch through
  exported stdlib facade wrappers that call `json_tree_*` helpers. The old
  compiler-owned raw facade fallback dispatcher has been removed.
- `jett_comptime` no longer has `Value::Json` or a `serde_json` dependency.
- In stdlib-loaded code, the root `JsonValue` spelling now resolves and
  reflects through the stdlib alias to `json.JsonTree`. Bare `JsonValue`
  without that alias is now unknown to the typechecker and direct comptime
  interpreter; the legacy `Type::JsonValue` placeholder remains internal until
  a later removal.
- The stdlib JSON module exports `json.JsonValue` as a source alias to
  `json.JsonTree`, and exports a narrow root alias
  `JsonValue = json.JsonTree` for source visibility.
- `json_decode_reflected[T](raw: JsonValue)` has been removed; decoding now
  enters through `json_decode_tree_reflected[T](view raw)` after parsing to
  `JsonTree`.
- `json_decode_tree_reflected[T]` now treats bare `JsonValue`,
  `json.JsonValue`, and `json.JsonTree` as raw-tree identity targets, so raw
  fields nested inside typed structs/lists/options use the native tree instead
  of an unsupported primitive path.
- `json.parse_exact[JsonValue]` and `json.parse_exact[json.JsonValue]` now
  share the same raw-tree identity behavior as `json.parse_exact[json.JsonTree]`;
  exact validation does not reject unknown fields inside raw tree targets.
- `json.serialize[json.JsonTree]` and
  `json.serialize_public[json.JsonTree]` serialize the native tree as raw JSON,
  matching the legacy `JsonValue` behavior rather than exposing enum internals.

## Compatibility Principle

Do not break user code merely to rename a type.

New code should prefer the stdlib-owned raw tree spelling:

```jett
json.JsonTree raw = json.parse_raw(body) handle error:
    return fail(error)
json.JsonTree name = json.field(view raw, "name") handle:
    return fail("missing name")
string text = json.as_string(view name) handle error:
    return fail(error)
```

Existing code like this should keep compiling during the transition:

```jett
JsonValue raw = json.parse_raw(body) handle error:
    return fail(error)
JsonValue name = json.field(raw, "name") handle:
    return fail("missing name")
string text = json.as_string(name) handle error:
    return fail(error)
```

The implementation under that spelling has changed from Rust-backed
`serde_json::Value` to native `JsonTree`. New raw facade signatures should
prefer `JsonTree`; the narrow stdlib root alias now handles the unqualified
`JsonValue` compatibility spelling. The remaining migration questions are
when to retire the legacy primitive tag and whether root aliases should
generalize beyond this compatibility bridge.

## Target API Shape

Preferred final shape:

```jett
namespace json

export enum JsonTree:
    ...
export type JsonValue = JsonTree
export root type JsonValue = json.JsonTree

export function parse_raw(raw: string) returns result[JsonTree, string]
export function serialize_raw(view value: JsonTree) returns string
export function kind(view value: JsonTree) returns string
export function is_null(view value: JsonTree) returns bool
export function is_bool(view value: JsonTree) returns bool
export function is_number(view value: JsonTree) returns bool
export function is_string(view value: JsonTree) returns bool
export function is_array(view value: JsonTree) returns bool
export function is_object(view value: JsonTree) returns bool
export function field(view value: JsonTree, key: string) returns optional[JsonTree]
export function index(view value: JsonTree, index: int64) returns optional[JsonTree]
export function object_field(view value: JsonTree, key: string) returns result[optional[JsonTree], string]
export function array_index(view value: JsonTree, index: int64) returns result[optional[JsonTree], string]
export function require_field(view value: JsonTree, key: string) returns result[JsonTree, string]
export function require_index(view value: JsonTree, index: int64) returns result[JsonTree, string]
export function array_length(view value: JsonTree) returns result[int64, string]
export function object_keys(view value: JsonTree) returns result[list[string], string]
export function as_string(view value: JsonTree) returns result[string, string]
export function as_int64(view value: JsonTree) returns result[int64, string]
export function as_uint64(view value: JsonTree) returns result[uint64, string]
export function as_float64(view value: JsonTree) returns result[float64, string]
export function as_bool(view value: JsonTree) returns result[bool, string]
```

`json.JsonValue` is now expressible as a normal exported stdlib alias, and bare
`JsonValue` is now expressible as a narrow stdlib root alias. In normal
stdlib-loaded builds it reflects as an alias to `json.JsonTree`; direct
interpreter reflection also requires that alias path now. The remaining legacy
legacy primitive placeholder is internal/bootstrap-only. See
`docs/open_design/prelude_root_aliases.md` for the recommended staged design.

`field` and `index` are probing helpers: wrong shape and absence both produce
`none`. Production validation should use `require_field` / `require_index` when
presence is mandatory, and `object_field` / `array_index` when absence is allowed
but the parent shape must be correct.

## Alias Direction

Do not implement `JsonValue` by relying on accidental namespace flattening.
Writing `type JsonValue = JsonTree` inside `namespace json` would naturally
create `json.JsonValue`, not the existing unqualified compatibility spelling.
Using the old flat alias leakage would have tied a core migration to behavior the
module system is supposed to remove.

The staged direction is:

1. Done: remove source-level `JsonValue` primitive fallback behavior. The
   typechecker and direct interpreter now require the alias path for bare
   `JsonValue`.
2. Done: normal stdlib-loaded source now uses the root alias
   `JsonValue = json.JsonTree`, without a separate compiler-owned compatibility
   table.
3. Done: the stdlib also exports `json.JsonValue = JsonTree` as the
   namespaced source-level alias.
4. Done: the stdlib exports `export root type JsonValue = json.JsonTree`, while
   project files are rejected from using `export root`.
5. Done: stdlib-loaded reflection now reports `type.info[JsonValue]()` as an
   alias to `json.JsonTree`, matching `json.JsonValue`.
6. Done for direct interpreter reflection: bare `JsonValue` without a
   registered alias is now just an unresolved named type with no primitive tag.
   A later cleanup can decide whether to deprecate or remove
   `TypePrimitive.json_value_type` after the internal placeholder has a staged
   replacement.

This keeps canonical identity clear: `json.JsonTree` is the real type;
`JsonValue` is a source-compatibility spelling.

## Staging Plan

### 1. Add Native Raw Helper Parity

Add stdlib helper names that match the current raw API behavior but operate on
`JsonTree`:

- `json_tree_serialize` serializes native tree values.
- `json_tree_kind`, `json_tree_field`, `json_tree_index`,
  `json_tree_array_length`, `json_tree_object_keys`, and scalar casts already
  exist.
- Add focused parity fixtures comparing current `JsonValue` helpers to
  `JsonTree` helpers for the same inputs.

Implementation note: view-friendly serialization now uses viewed list/map
iteration rather than materializing or cloning raw trees. Keep that path native
to `.jett` stdlib code rather than reintroducing host magic.

Status: first parity fixture is in place in
`tests/run_pass/json_raw_tree_parity.jett`, covering object traversal, array
lookup, nulls, booleans, strings, numbers, wrong-shape errors, absent lookup,
and raw serialization against the native tree helpers. The focused
`tests/run_pass/json_value_tree_compatibility.jett` fixture now also pins the
legacy raw helper surface accepting native `json.JsonTree` values directly, and
`tests/run_pass/json_raw_facade_tree_surface.jett` pins the public raw facade
API as a `JsonTree`-first surface.

### 2. Route Public Raw Functions Through Trusted Stdlib Hooks

Keep legacy `JsonValue` source compatibility stable, but make normal checked
and interpreted execution delegate to trusted stdlib functions where possible:

- `json.parse_raw(raw)` delegates to `json.json_tree_parse(raw)`.
- `json.serialize_raw(value)` delegates to native tree serialization.
- `json.kind`, `json.field`, `json.index`, scalar casts, length, and keys
  delegate to the corresponding tree helpers.

The typechecker should prefer the bundled `json.JsonTree` type for raw facade
parameters and returns when the stdlib is loaded from compiler-shipped files.
User/project declarations with the same qualified name must not seed that
bridge. The stdlib root alias keeps older `JsonValue` code working during the
transition.

Status: implemented. Exported stdlib facade wrappers now exist for
`parse_raw`, `serialize_raw`, `kind`, `field`, `index`, the shape predicates,
length/key helpers, and scalar casts. The interpreter prefers those trusted
stdlib wrappers when they are registered, and the old raw builtin path has
been removed. The public typed `json.parse[JsonValue]` compatibility branch
also calls `json_tree_parse` directly instead of bouncing through the raw
builtin surface. The shared JSON facade name set now lives in `jett_common` for
runtime wrapper precedence and ownership's implicit-view rule across raw
facades and view-first `json_tree_*` accessors. The typechecker now gets raw
facade signatures from the exported stdlib wrapper declarations rather than a
hardcoded signature table; no-stdlib/bootstrap contexts no longer have a
separate compiler-owned raw facade signature fallback.

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
- Update `type.info[JsonValue]()` to follow the stdlib root alias when the
  bundled stdlib is loaded.
- Decide whether `TypePrimitive.json_value_type` should remain as an inert
  deprecated enum variant or disappear with the internal placeholder.

Recommendation: keep `TypePrimitive.json_value_type` only while staged cleanup
needs it for compatibility, and document it as legacy now that `JsonTree` is
the preferred spelling.

Status: implemented through a narrow source-level root alias. Normal
stdlib-loaded code resolves `JsonValue` through that alias, so it shares the
same checked type as the bundled `json.JsonTree`; user-defined enums named
`JsonTree` remain unrelated. The runtime raw facade checks only accept enum
values whose owner is the bundled `json.JsonTree`, not a bare or user-defined
`JsonTree`. Typechecker raw facade signatures also follow the same
trusted-origin rule instead of trusting qualified name text alone.
Reflection metadata now follows source aliases in stdlib-loaded code:
`type.info[JsonValue]()` and `type.info[json.JsonValue]()` report aliases to
`json.JsonTree`, while `type.info[json.JsonTree]()` reports enum metadata. The
direct interpreter and typechecker no longer expose a legacy primitive when the
alias is absent. Ownership follows the same source model: `JsonValue` is no
longer treated as an
implicitly copyable primitive, and raw read facades keep their ergonomics by
borrowing the raw tree argument implicitly.

### 5. Move Raw Decoder Code Off `JsonValue`

The stdlib JSON module used to contain an older `json_decode_reflected[T](raw:
JsonValue)` path. After raw APIs use `JsonTree`:

- Replace internal uses with `json_decode_tree_reflected[T]`.
- Remove the thin `json_decode_reflected[T]` compatibility wrapper once nothing
  calls it.
- Remove duplicate `JsonValue` decoder helpers once no tests or public bridges
  rely on them.

This should substantially reduce stdlib JSON duplication.

Status: implemented. The old duplicate `JsonValue` decoder helper family was
removed, and the unused private `json_decode_reflected[T](raw: JsonValue)`
wrapper was removed as well. Public typed parse enters through the `JsonTree`
parser/decoder, with only the `JsonValue` type spelling kept as compatibility.
The flat reflected decoder proof fixture now follows the same shape with
`view json.JsonTree` inputs and strict `json.require_field` lookup;
`JsonValue` remains covered by explicit compatibility tests rather than new
decoder examples.

### 6. Remove Rust JSON Fallbacks

After parity tests pass with native representation:

- Done: remove `serde_json::Value` from `jett_comptime::value::Value`.
- Done: remove Rust-backed `json.parse_raw` and raw accessor implementations.
- Done: remove the runtime raw facade fallback dispatcher. Public raw calls now
  use exported stdlib wrappers in normal stdlib-loaded execution.
- Keep Rust only for tests/dev tooling if useful, not as language semantics.

## Required Tests

Add tests before each behavior change:

- `json.parse_raw` returns a value that raw accessors can traverse.
- Raw facade signatures accept and return `json.JsonTree` directly while
  preserving `JsonValue` compatibility.
- `json.parse[JsonValue]` still works during compatibility.
- `json.parse[json.JsonValue]`, `json.serialize[json.JsonValue]`, and
  `json.serialize_public[json.JsonValue]` treat the namespaced alias as raw
  `JsonTree`, not as enum internals.
- `json.parse[json.JsonTree]` returns the native raw tree directly.
- `JsonValue` and `json.JsonTree` assignment/container alias behavior stays
  pinned.
- Raw object lookup preserves absence semantics.
- Wrong-shape lookup returns `none`; wrong-shape shape-requiring access returns
  `result` errors.
- Raw serialization matches `JsonTree` serialization.
- Malformed-input diagnostics stay pinned.
- Reflection metadata remains intentional for `JsonValue` and `JsonTree`.
- A user-defined top-level `enum JsonTree` remains incompatible with
  `JsonValue`.
- The stdlib root `JsonValue` compatibility alias works recursively in
  `list`, `map`, `set`, `optional`, `result`, `secret`, struct fields, and
  function arguments.
- `type.info[JsonValue]()`, `type.info[json.JsonValue]()` and
  `type.info[json.JsonTree]()` stay pinned during the compatibility stage.

Status: the recursive compatibility surface is pinned in
`tests/run_pass/json_value_tree_compatibility.jett`, including bidirectional
list, map, set, optional, result, secret, direct assignment, function argument,
return, and struct-field compatibility between `JsonValue` and `json.JsonTree`.
Malformed-input error parity is pinned in
`tests/run_pass/json_parse_error_parity.jett` across `json.json_tree_parse`,
`json.parse_raw`, `json.parse[JsonValue]`, and `json.parse[json.JsonTree]`.
Exact-parse raw target compatibility is pinned in
`tests/run_pass/json_parse_exact.jett` for `json.JsonTree`, bare `JsonValue`,
and `json.JsonValue`. Public bridge compatibility for the namespaced alias is
pinned in `tests/run_pass/json_parse.jett`,
`tests/run_pass/json_serialize.jett`, and
`tests/run_pass/json_serialize_public.jett`. Strict raw accessors are pinned in
`tests/run_pass/json_raw_strict_accessors.jett` for `json.JsonTree`, bare
`JsonValue`, and `json.JsonValue`, while lenient raw probing edge cases are
pinned in `tests/run_pass/json_raw_value_access_edges.jett`. The namespaced
`json.JsonValue` alias facade surface is pinned in
`tests/run_pass/json_raw_alias_facade_surface.jett`, covering kind checks,
predicates, object keys, scalar casts, array indexing, optional lookup, and raw
serialization. Strict accessor argument-shape diagnostics are pinned in
`tests/compile_fail/json_raw_strict_accessor_argument_shapes.jett`, and the
broader raw facade shape diagnostics are pinned in
`tests/compile_fail/json_raw_facade_argument_shapes.jett`.
Bare-handle diagnostics for optional-returning probing facades are pinned in
`tests/compile_fail/json_raw_probe_facades_require_handle.jett`.
Unhandled-result diagnostics for raw result-returning facades are pinned in
`tests/compile_fail/json_raw_result_facades_require_handle.jett`.
Serialization ownership diagnostics for raw tree values are pinned in
`tests/compile_fail/json_serialize_json_value_requires_view.jett` and
`tests/compile_fail/json_serialize_public_json_value_requires_view.jett` across
bare `JsonValue`, `json.JsonValue`, and direct `json.JsonTree`.
General JSON examples now prefer `json.JsonTree` / `json.JsonValue`; bare
`JsonValue` remains concentrated in compatibility, parity, and transition
fixtures.

## Risks And Open Questions

- **Alias mechanics:** the first stdlib root alias exists for `JsonValue`. The
  open question is whether this stays an allowlisted compatibility-only feature
  or grows into a broader prelude policy.
- **Reflection metadata:** because `JsonValue` is now a stdlib root alias in
  stdlib-loaded code, `type.info[JsonValue]()` alias metadata must stay pinned
  while the no-stdlib primitive fallback remains documented separately.
- **View iteration:** native raw serialization over `view JsonTree` now uses
  viewed list/map iteration; the remaining question is whether the broader
  language should make view-parameter calls implicitly non-consuming for
  ordinary source functions, or keep requiring explicit `view` at call sites.
- **Performance:** the self-hosted parser is correctness-first today. Native
  backends can optimize later, but we should avoid introducing needless
  materialization in the language semantics.
- **API naming:** `JsonTree` is honest for implementation; `JsonValue` may be
  friendlier for users. The final public spelling can be decided separately from
  the representation.

## Recommended Next Decision

Continue the legacy primitive-tag retirement:

1. Keep bare `JsonValue` in compatibility fixtures and design notes that explain
   the transition.
2. Done: stdlib-loaded `type.info[JsonValue]()` now reports alias metadata for
   `json.JsonTree`.
3. Done: direct/no-stdlib interpreter reflection rejects the legacy primitive
   shortcut by treating bare `JsonValue` without a registered alias as an
   unresolved named type with no primitive tag.
4. Done: remove the typechecker fallback for bare `JsonValue` without the
   stdlib root alias.
5. Next: remove `Type::JsonValue`, `TypeInterner::JSON_VALUE`, and
   `TypePrimitive.json_value_type` once no bootstrap/internal path needs the
   placeholder.
