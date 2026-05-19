# JSON Stdlib Extraction Plan

This note records the remaining compiler/tooling work needed before the
reflection JSON prototypes can become a real stdlib `json` module.

The important conclusion is that JSON itself is no longer the main blocker.
Reflection can now read fields, construct structs/bitfields/enums, inspect type
arguments, use structured kind and primitive tags, and walk native `JsonTree`
values. `JsonValue` is now visible through a narrow stdlib root alias while
keeping its legacy primitive reflection tag for one compatibility stage. The
blocker is the module and namespace path that would let ordinary `.jett` stdlib
code own the public `json.*` API.

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
- Builtin module prefixes such as `json`, `type`, `list`, `map`, `set`,
  `string`, and `bytes` are hardcoded in resolver/typechecker/interpreter
  paths. The typechecker now gives compiler-owned builtin signatures precedence
  over ordinary functions, matching the interpreter's builtin-first runtime
  dispatch. Set builtins now have compiler-owned signatures for
  `new`/`add`/`remove`/`contains`/`length`/`is_empty`/`to_list`/`union`/
  `intersection`/`difference`, so JSON set parsing fixtures typecheck through
  normal builtin signatures.

That means physical stdlib JSON fragments under `stdlib/json/` are available
to single-file builds, but ordinary `namespace json` functions still do not
automatically own policy-bearing public calls such as `json.parse[T](...)`.

## JSON-Specific State

The reflected JSON implementation has started moving into stdlib under the
`json` namespace. Prefixed hook names hold the implementation bodies:

- `stdlib/json/` fragments
- `JsonTree` as a first self-hosted raw JSON tree representation
- `json.JsonValue` as an exported namespaced alias for `JsonTree`
- bare `JsonValue` as a stdlib root alias for `json.JsonTree`, while the
  legacy `TypePrimitive.json_value_type` reflection tag is preserved
- `json_tree_serialize(value: JsonTree)`
- `json_tree_parse(raw: string)` for staged scalar, array, and object parsing
- `json_tree_*` traversal helpers for kind checks, field/index lookup, lengths,
  keys, and scalar casts
- `json_serialize_reflected[T](view value)`
- `json_serialize_public_reflected[T](view value)`
- `json_parse_reflected[T](raw: string)`
- `json_parse_exact_reflected[T](raw: string)`

The module also declares exported public wrapper names `parse`, `parse_exact`,
`serialize`, and `serialize_public`. Calls such as `json.parse[T](raw)` and
`json.parse_exact[T](raw)` still pass through the compiler-owned policy gate
first, then the interpreter delegates to the internal reflected stdlib hook when
the bundled module is registered as trusted compiler-shipped stdlib. The public
wrappers remain readable source-level declarations, but the bridge target stays
on internal hook names until the language can carry the compiler-owned JSON
policy on ordinary stdlib declarations.

The raw-string hooks remain directly exercised through their qualified stdlib
staging names, `json.json_parse_reflected[T](raw)` and
`json.json_parse_exact_reflected[T](raw)`.

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
- focused external shape fixtures for bytes, floats, sets, maps, enum payloads,
  bitfields, optionals, results, refinements, and public secret omission across
  `serialize`, `serialize_public`, `parse`, and `parse_exact`.
- malformed-input parity across `json.json_tree_parse`, `json.parse_raw`,
  `json.parse[JsonValue]`, `json.parse[json.JsonTree]`, and the exact raw-tree
  targets.
- dedicated shape coverage now lives in `json_shape_matrix`,
  `json_parse_collection_edges`, `json_parse_exact*`,
  `json_enum_bitfield_exact_edges`, `json_refinement_exact_serialize_edges`,
  and parser parity fixtures.

The compiler-known JSON bridge remains the compatibility and policy facade. It
still owns the public typechecker policy for `json.parse`, `json.parse_exact`,
`json.serialize`, and `json.serialize_public`. In the interpreter, all four public calls now
delegate to reflected stdlib hook implementations only when the current hook
registry entries came from compiler-shipped stdlib files. The old typed Rust
fallback paths for public parse/serialize have been removed; Rust-backed paths
have also been removed from raw JSON execution in `jett_comptime`. Public
`json.parse_raw` now delegates to the trusted self-hosted `JsonTree` parser,
and raw helper calls dispatch native `JsonTree` runtime values through trusted
stdlib facade wrappers backed by `json_tree_*` hooks. The remaining builtin raw
facade path is a bootstrap/no-stdlib dispatcher around those hooks, not a
separate Rust JSON implementation.
`json.parse_exact[T]` now exists as a second compiler-policy public parse
bridge backed by trusted stdlib hooks. It rejects unknown object fields
recursively for reflected closed-shape targets: structs, bitfields, enum
payloads, aliases/refinements over those shapes, and containers containing
them. Raw `JsonTree` / `JsonValue` targets remain arbitrary JSON payloads.
The existing `json.parse[T]` behavior remains lenient.

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

- the compiler-owned `json.parse`, `json.parse_exact`, `json.serialize`, and
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
- `map[K, V]` JSON encoding and decoding for `serialize`, `serialize_public`,
  `parse`, and `parse_exact` is restricted to `K == string`.
- `json.parse[T]` and `json.parse_exact[T]` return `result[T, string]`, so
  callers must handle errors.

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
   - `json_decode_tree_reflected[T](view raw: JsonTree)` exists in stdlib.
   - The old private `json_decode_reflected[T](raw: JsonValue)` wrapper and the
     duplicate raw decoder helper family have been removed.
   - `json_parse_reflected[T](raw: string)` now routes typed targets through the
     self-hosted `JsonTree` parser/decoder. The `JsonValue` compatibility
     branch also parses through `json_tree_parse` directly.
   - Public raw facade wrappers now exist in `stdlib/json/` for
     `parse_raw`, `serialize_raw`, tree kind/predicates, lookup, length/key
     helpers, and scalar casts.
   - The typechecker raw facade signatures now prefer the bundled
     `json.JsonTree` type only when it came from compiler-shipped stdlib files,
     while preserving the compiler-owned `JsonValue` compatibility alias.
     User/project declarations with the same qualified name do not affect that
     bridge. When compiler-shipped stdlib declarations are loaded, those raw
     facade calls now use the trusted exported stdlib signatures before falling
     back to the hardcoded bootstrap signature table.
   - `json_tree_serialize` is view-native and iterates arrays/objects through
     viewed list/map loops, so `serialize_raw(view value)` no longer clones the
     tree before serializing.
   - The JSON facade name policy is centralized in `jett_common` and reused by
     runtime dispatch plus ownership checking, including direct view-first
     `json_tree_*` helpers. The raw facade policy also records the trusted
     stdlib hook and argument shape used by runtime bootstrap dispatch.
   - The compiler-policy public bridge names (`json.parse`,
     `json.parse_exact`, `json.serialize`, and `json.serialize_public`) also
     share their trusted stdlib hook mapping through `jett_common`.
3. Maintain the public bridge handoff. `json.parse`, `json.parse_exact`,
   `json.serialize`, and `json.serialize_public` now use compiler-owned
   typechecker policy with stdlib-owned interpreter bodies. See
   `/docs/completed/json_public_bridge_handoff.md`.
4. Keep the real public wrapper names in `namespace json`, while retaining
   compiler policy checks for secrets, `view`, map keys, and handled results.
5. Keep broad bridge/parity tests before removing any Rust-backed fallback
   implementation paths. Done for typed public parse/serialize, raw
   `JsonValue` execution in `jett_comptime`, and runtime `main()` reflection
   metadata handoff for namespaced generic JSON.
6. Continue hardening the self-hosted `JsonTree` parser. Common malformed-input
   diagnostics are pinned for unterminated strings/arrays/objects, trailing
   characters, mismatched delimiters, bad number forms, bad literals,
   empty/whitespace-only roots, malformed object keys, duplicate object fields
   after key unescaping, comma/separator errors, extra closing delimiters,
   nested unterminated strings, invalid escapes, and unicode surrogate failures.
   Root and nested value trimming now follows JSON's byte-level whitespace set
   only: space, tab, line feed, and carriage return. Non-JSON whitespace such as
   form feed stays part of the token and is rejected through the same parser
   error path as other malformed input.
   Valid JSON escapes now include quote, backslash, slash, backspace, form feed,
   newline, carriage return, tab, and unicode escapes. Public entrypoint parity
   for those parser errors is pinned across
   `json.json_tree_parse`, `json.parse_raw`, `json.parse[JsonValue]`,
   `json.parse[json.JsonTree]`, `json.parse_exact[JsonValue]`, and
   `json.parse_exact[json.JsonTree]`. The remaining question is how far
   `JsonTree` should go toward replacing the raw `JsonValue` compatibility
   surface.
7. Keep the staged `JsonValue` migration narrow. The current implementation
   exposes `json.JsonValue` as an exported namespaced source alias and bare
   `JsonValue` as an allowlisted stdlib root alias for `json.JsonTree`. It also
   preserves the compiler-owned legacy compatibility relation and separate
   `TypePrimitive.json_value_type` reflection metadata for one compatibility
   stage. See `/docs/active/json_value_transition_plan.md` and
   `/docs/open_design/prelude_root_aliases.md`. The remaining decision is when
   to deprecate or remove the legacy primitive reflection tag.
8. Keep reflection-specialized generic helpers staged carefully. The typechecker
   now checks ordinary generic function bodies per concrete instantiation, and
   it can specialize the narrow direct-branch form
   `type.kind_tag[T]()` / `type.info[T]().kind_tag` compared to `TypeKind.*`
   inside `if`/`else if` conditions. It also carries immutable block-local
   `TypeInfo`, `TypeKind`, and `TypePrimitive` facts derived from those direct
   reflection calls, so helpers may name `TypeInfo info = type.info[T]()` and
   branch on `info.kind_tag`, `info.primitive_tag handle default ...`, or local
   tag variables. Immutable generic helper parameters of type `TypeInfo`,
   `TypeKind`, or `TypePrimitive` also receive those facts from known call
   arguments, which lets helper split points like
   `json_decode_tree_structured_reflected[T](..., kind, ...)` and primitive
   decoder helpers specialize on caller-reflected tags. Branches selected by a
   statically known reflection guard may now contain deeper `type.arg[T](...)`
   bindings, while unknown/runtime guards are rejected instead of falling back
   to checking all branches. Direct top-level `type.arg[T](...)` bindings are
   also checked per concrete generic instantiation, which brings shape-specific
   helpers such as list/map/optional/result JSON helpers into the typed path.
   Direct reflected `type.fields[T]()` and `type.variants[T]()` loops are also
   checked per concrete owner in top-level helpers or selected reflection
   branches, which pulls record and enum JSON helper bodies further into the
   checked path. Direct top-level value-sensitive reflection statements for
   variant selection and `TypeConstruction` start/finish are checked per
   concrete instantiation as well. Reflection-specialized `match` statements
   over known `TypeKind` and `TypePrimitive` values now select only the
   reachable arm, matching Jett's canonical enum-dispatch form rather than
   forcing all generic reflection code through `if` ladders. Primitive JSON
   dispatch still keeps exact `TypePrimitive.*` comparisons visible at each
   generic helper split point, using small optional-returning helpers rather
   than opaque predicates, so the checker can prune invalid primitive casts
   while the function complexity checker still applies to stdlib code.
   Predicate-derived facts, diagnostic string facts, and other value-sensitive
   reflection helper shapes remain deferred.

## Recommended Shape For The Stdlib JSON Module

The eventual module should keep these layers distinct:

- `quote(raw: string) returns string`
- `serialize_value[T](view value: T) returns result[string, string]`
- `serialize_public_value[T](view value: T) returns result[string, string]`
- `decode_value[T](view raw: JsonTree) returns result[T, string]`
- `parse[T](raw: string) returns result[T, string]`
- `parse_exact[T](raw: string) returns result[T, string]`

The public API can return plain `string` for serialization only if the compiler
continues to prove that the chosen policy cannot fail. The internal reflected
functions should probably use `result` while format policy is still evolving.

## Open Questions

- Should `json.serialize[T]` stay a compiler-checked secret exposure boundary
  even after its implementation body moves to stdlib code?
- Should unknown object fields be ignored, rejected, or configurable? See
  `docs/open_design/json_unknown_field_policy.md` for the staged
  `parse_exact` path, now implemented while keeping `json.parse[T]` lenient.
- Should `json.field` and `json.index` keep returning `optional[JsonTree]`, or
  should wrong-shape access be distinguishable from absence? See
  `docs/open_design/json_raw_access_semantics.md` for the current options and
  recommendation. Strict additive helpers now exist for shape-sensitive lookup;
  the remaining question is long-term naming/default guidance.
- Representative shape is pinned for bytes, `float32`/`float64`, sets, maps,
  optionals/results, unit and payload enums, bitfields, aliases/refinements,
  serialize names, and nested matrix combinations. The remaining shape
  questions are narrower: whether map/set ordering should ever become a
  documented contract, whether to add configurable strictness beyond
  `parse_exact`, and whether more parser diagnostics deserve similarly specific
  wording.
- How should stdlib helper visibility evolve beyond namespace-private
  `export` syntax, especially once a full import/prelude model exists?
- How much abstraction is allowed around trusted reflection loops before
  `comptime type Field = field.type_info:` loses provenance?
