# JSON Stdlib Extraction Plan

This note records the remaining compiler/tooling work needed before the
reflection JSON prototypes can become a real `stdlib/json.jett` module.

The important conclusion is that JSON itself is no longer the main blocker.
Reflection can now read fields, construct structs/bitfields/enums, inspect type
arguments, use structured kind and primitive tags, and walk native `JsonTree`
values. `JsonValue` remains only as the legacy compatibility spelling for that
tree.
The blocker is the module and namespace path that would let ordinary `.jett`
stdlib code own the public `json.*` API.

## Current Architecture

- `stdlib/` now has a bootstrap loader, marker module, and draft JSON module.
- `build_file` parses the requested file, then prepends stdlib modules and, when
  a `jett.proj` is discovered, sibling project modules.
- `run_file` validates through `build_file`, then registers stdlib modules,
  sibling project modules, and the entry module in the interpreter.
- `build_source` and `test_file` also prepend stdlib modules, so LSP-style
  validation and `jett test file` exercise the same prelude path.
- `test_file` now also loads sibling project modules for cross-file verify
  blocks, while stripping verify/property blocks from support modules so a
  single-file test report stays focused on the requested file.
- Namespaces are parsed and resolved as declarations. Top-level functions and
  named types are checked and interpreted through canonical `namespace.name`
  names, with scoped lookup preserving same-namespace ergonomics. This is still
  lighter than a full module registry, but it no longer relies on runtime flat
  leaf aliases for namespaced declarations.
- `use` declarations bind aliases or final path segments, but they do not yet
  import a namespace registry.
- Builtin module prefixes such as `json`, `type`, `list`, `string`, and `bytes`
  are hardcoded in resolver/typechecker/interpreter paths. The typechecker now
  gives compiler-owned builtin signatures precedence over ordinary functions,
  matching the interpreter's builtin-first runtime dispatch.

That means a physical `stdlib/json.jett` file is available to single-file
builds, but ordinary `namespace json` functions still do not automatically own
policy-bearing public calls such as `json.parse[T](...)`.

## JSON-Specific State

The reflected JSON implementation has started moving into stdlib under the
`json` namespace. Prefixed hook names hold the implementation bodies:

- `stdlib/json.jett`
- `JsonTree` as a first self-hosted raw JSON tree representation
- `json_tree_serialize(value: JsonTree)`
- `json_tree_parse(raw: string)` for staged scalar, array, and object parsing
- `json_tree_*` traversal helpers for kind checks, field/index lookup, lengths,
  keys, and scalar casts
- `json_serialize_reflected[T](view value)`
- `json_serialize_public_reflected[T](view value)`
- `json_decode_reflected[T](raw: JsonValue)`
- `json_parse_reflected[T](raw: string)`

The module also declares exported public wrapper names `parse`, `serialize`,
and `serialize_public`. Calls such as `json.parse[T](raw)` still pass through
the compiler-owned policy gate first, then the interpreter delegates to the
internal reflected stdlib hook when the bundled module is registered as trusted
compiler-shipped stdlib. The public wrappers remain readable source-level
declarations, but the bridge target stays on internal hook names until the
language can carry the compiler-owned JSON policy on ordinary stdlib
declarations.

The raw-string hook remains directly exercised through its qualified stdlib
staging name, `json.json_parse_reflected[T](raw)`.

The run-pass fixtures still own the test-specific type definitions and the
flat decoder prototype:

- `tests/run_pass/json_reflection_nested_serializer.jett`
- `tests/run_pass/json_reflection_nested_decoder.jett`

Together they cover the shape needed for a future stdlib module:

- recursive structured type dispatch through `TypeKind` and `TypePrimitive`,
- primitive values, bytes, null, aliases, refinements, secrets,
- structs, bitfields, enum variants and enum payloads,
- lists, sets, `map[string, V]`, optionals, and results,
- field `serialize_name`,
- missing optional defaults,
- public secret omission for the serializer prototype,
- all-control-character JSON string escaping in `.jett`.
- self-hosted `JsonTree` construction, serialization, staged scalar/array/object
  parsing, and traversal helpers for the future raw parser target.
- bridge checks that keep the reflected prototypes aligned with the public JSON
  facade for representative full serialization, public serialization, and
  typed parse.

The compiler-known JSON bridge remains the compatibility and policy facade. It
still owns the public typechecker policy for `json.parse`, `json.serialize`,
and `json.serialize_public`. In the interpreter, all three public calls now
delegate to reflected stdlib hook implementations only when the current hook
registry entries came from compiler-shipped stdlib files. The old typed Rust
fallback paths for public parse/serialize have been removed; Rust-backed paths
have also been removed from raw JSON execution in `jett_comptime`. Public
`json.parse_raw` now delegates to the trusted self-hosted `JsonTree` parser, and
raw helper calls dispatch native `JsonTree` runtime values through trusted
stdlib facade wrappers backed by `json_tree_*` hooks. The Rust builtin cases
remain as bootstrap fallbacks for direct interpreter use without loaded stdlib.

## Blockers

### 1. Stdlib Loading

The driver has a first bootstrap path for loading stdlib `.jett` files before
user modules for:

- `jett build`,
- `jett run`,
- `jett test file`,
- `jett test` over a project,
- LSP validation paths,
- future query/agent tooling.

This is still intentionally simple: files are collected from repo-local
`stdlib/` and prepended before project modules. Dependency ordering is lexical
for now.

### 2. Public `json.*` Handoff

Generic and non-generic user functions can now be called through a qualified
namespace name such as `helpers.wrap[T](value)` or
`json.json_parse_reflected[T](raw)`.

Moving JSON into `.jett` still needs the public API handoff:

- the compiler-owned `json.parse`, `json.serialize`, and
  `json.serialize_public` paths still carry compiler-enforced policy checks,
- `use` still resolves only a namespace-looking binding rather than importing a
  real namespace registry,
- qualified names are still an alias-based staging model rather than a full
  namespace/export registry.

This is larger than JSON and should be staged carefully.
See `/docs/active/stdlib_visibility_design.md` for the current visibility and
trusted-stdlib recommendation.

The important design boundary is now explicit: `export` and trusted stdlib
origin are separate. `export` controls what ordinary source code can name.
Trusted origin controls whether compiler-owned JSON policy may delegate to a
stdlib implementation. A private trusted hook is valid as a compiler target; an
exported user/project function with the same name is not.

### 3. Builtin Policy Boundaries

Some JSON rules are typechecker policy, not only implementation:

- `json.serialize[T]` and `json.serialize_public[T]` require `view` for
  non-copy compound values.
- full serialization rejects secret-containing values.
- public serialization rejects top-level secret wrappers and omits
  secret-containing fields.
- `map[K, V]` JSON encoding is currently restricted to `K == string`.
- `json.parse[T]` returns `result[T, string]`, so callers must handle errors.

Those checks can remain compiler-known while the body moves to `.jett`, or they
can be redesigned as ordinary typed stdlib constraints later. The safer staging
choice is to keep the checks until namespace and stdlib loading are stable.

### 4. Visibility

The serializer and decoder need helper functions such as `quote`,
`decode_value[T]`, and enum/field helpers. Jett now has explicit `export`
visibility for namespaced declarations, but it does not yet have a separate
`private` keyword or a full module registry/import story.

Until private helper visibility exists, extraction should avoid pretending
helper names are hidden by naming convention alone. The current stdlib uses
`export` for the intended public surface and leaves internal hooks unexported.

The first trusted-origin staging piece is in place: interpreter function
registry entries parsed from the reserved stdlib file-id range are marked
trusted, and public JSON bridges require trusted hook entries. This solves
bridge spoofing, not helper visibility.

## Minimal Staging Plan

1. Keep the compiler-known public JSON bridge for policy checks.
2. Continue staging extracted prototype code under flat, non-conflicting names:
   - `json_serialize_reflected[T](view value: T)` exists in stdlib.
   - `json_serialize_public_reflected[T](view value: T)` exists in stdlib.
   - `json_decode_reflected[T](raw: JsonValue)` exists in stdlib.
   - `json_decode_tree_reflected[T](view raw: JsonTree)` exists in stdlib.
   - `json_decode_reflected[T](raw: JsonValue)` is now a thin compatibility
     wrapper over the `JsonTree` decoder; the duplicate raw decoder helper
     family has been removed.
   - `json_parse_reflected[T](raw: string)` now routes typed targets through the
     self-hosted `JsonTree` parser/decoder. The `JsonValue` compatibility
     branch also parses through `json_tree_parse` directly.
   - Public raw facade wrappers now exist in `stdlib/json.jett` for
     `parse_raw`, `serialize_raw`, tree kind/predicates, lookup, length/key
     helpers, and scalar casts.
   - `json_tree_serialize` is view-native and iterates arrays/objects through
     viewed list/map loops, so `serialize_raw(view value)` no longer clones the
     tree before serializing.
   - The raw facade name policy is centralized in `jett_common` and reused by
     runtime dispatch plus ownership checking.
3. Maintain the public bridge handoff. `json.parse`, `json.serialize`, and
   `json.serialize_public` now use compiler-owned typechecker policy with
   stdlib-owned interpreter bodies. See `/docs/completed/json_public_bridge_handoff.md`.
4. Keep the real public wrapper names in `namespace json`, while retaining
   compiler policy checks for secrets, `view`, map keys, and handled results.
5. Keep broad bridge/parity tests before removing any Rust-backed fallback
   implementation paths. Done for typed public parse/serialize, raw
   `JsonValue` execution in `jett_comptime`, and runtime `main()` reflection
   metadata handoff for namespaced generic JSON.
6. Continue hardening the self-hosted `JsonTree` parser. Common malformed-input
   diagnostics are pinned for unterminated strings/arrays/objects, trailing
   characters, mismatched delimiters, bad number forms, bad literals, and
   invalid escapes; the remaining question is how far `JsonTree` should go
   toward replacing the raw `JsonValue` compatibility surface.
7. Decide whether `JsonValue` becomes a source-level type alias/replacement or
   remains a compiler-recognized compatibility spelling. The current
   implementation seeds a compiler-owned legacy compatibility alias from
   built-in `JsonValue` to stdlib `json.JsonTree`, while preserving separate
   reflection metadata for one compatibility stage. See
   `/docs/active/json_value_transition_plan.md`. The remaining decision is
   whether and when that alias moves into the exported stdlib/prelude surface.

## Recommended Shape For `stdlib/json.jett`

The eventual module should keep these layers distinct:

- `quote(raw: string) returns string`
- `serialize_value[T](view value: T) returns result[string, string]`
- `serialize_public_value[T](view value: T) returns result[string, string]`
- `decode_value[T](view raw: JsonTree) returns result[T, string]`
- `parse[T](raw: string) returns result[T, string]`

The public API can return plain `string` for serialization only if the compiler
continues to prove that the chosen policy cannot fail. The internal reflected
functions should probably use `result` while format policy is still evolving.

## Open Questions

- Should `json.serialize[T]` stay a compiler-checked secret exposure boundary
  even after its implementation body moves to stdlib code?
- Should unknown object fields be ignored, rejected, or configurable?
- Should `json.field` and `json.index` keep returning `optional[JsonValue]`, or
  should wrong-shape access be distinguishable from absence?
- What is the stable external shape for enums, bitfields, bytes, floats, sets,
  and maps?
- How should stdlib helper visibility work before the language has a general
  `private` rule and a full import/prelude model?
- How much abstraction is allowed around trusted reflection loops before
  `comptime type Field = field.type_info:` loses provenance?
