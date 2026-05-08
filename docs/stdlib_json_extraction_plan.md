# JSON Stdlib Extraction Plan

This note records the remaining compiler/tooling work needed before the
reflection JSON prototypes can become a real `stdlib/json.jett` module.

The important conclusion is that JSON itself is no longer the main blocker.
Reflection can now read fields, construct structs/bitfields/enums, inspect type
arguments, use structured kind tags, and walk raw `JsonValue`. The blocker is
the module and namespace path that would let ordinary `.jett` stdlib code own
the public `json.*` API.

## Current Architecture

- `stdlib/` now has a bootstrap loader and marker module, but real stdlib JSON
  extraction has not happened yet.
- `build_file` parses the requested file, then prepends stdlib modules and, when
  a `jett.proj` is discovered, sibling project modules.
- `run_file` validates through `build_file`, then registers stdlib modules,
  sibling project modules, and the entry module in the interpreter.
- `build_source` and `test_file` also prepend stdlib modules, so LSP-style
  validation and `jett test file` exercise the same prelude path.
- Namespaces are parsed and resolved as declarations. Top-level functions are
  now registered under both their historical flat name and a
  `namespace.name` qualified name for typechecking and interpretation; types
  are still effectively registered by flat names.
- `use` declarations bind aliases or final path segments, but they do not yet
  import a namespace registry.
- Builtin module prefixes such as `json`, `type`, `list`, `string`, and `bytes`
  are hardcoded in resolver/typechecker/interpreter paths.

That means a physical `stdlib/json.jett` file would not currently become
available to single-file builds, and a `namespace json` function would not
automatically be callable as `json.parse[T](...)`.

## JSON-Specific State

The reflected public serializer prototype has started moving into stdlib under
the `json` namespace while keeping prefixed staging function names so it does
not collide with the compiler-owned public bridge:

- `stdlib/json_reflection.jett`
- `json_serialize_reflected[T](view value)`
- `json_serialize_public_reflected[T](view value)`
- `json_decode_reflected[T](raw: JsonValue)`
- `json_parse_reflected[T](raw: string)`

The raw-string wrapper is also exercised through its qualified stdlib staging
name, `json.json_parse_reflected[T](raw)`.

The run-pass fixtures still own the test-specific type definitions and the
flat decoder prototype:

- `tests/run_pass/json_reflection_nested_serializer.jett`
- `tests/run_pass/json_reflection_nested_decoder.jett`

Together they cover the shape needed for a future stdlib module:

- recursive structured type dispatch through `TypeKind`,
- primitive values, bytes, null, aliases, refinements, secrets,
- structs, bitfields, enum variants and enum payloads,
- lists, sets, `map[string, V]`, optionals, and results,
- field `serialize_name`,
- missing optional defaults,
- public secret omission for the serializer prototype,
- all-control-character JSON string escaping in `.jett`.
- bridge checks that keep the reflected prototypes aligned with the public JSON
  facade for representative full serialization, public serialization, and
  typed parse.

The compiler-known JSON bridge remains the compatibility and policy facade. It
still owns the public typechecker policy for `json.parse`, `json.serialize`,
and `json.serialize_public`. In the interpreter, all three public calls now
delegate to reflected stdlib implementations when the bundled stdlib functions
are registered, while Rust-backed fallback paths remain for raw JSON access and
bootstrap compatibility.

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

- the Rust-backed `json.parse`, `json.serialize`, and
  `json.serialize_public` paths still carry compiler-enforced policy checks,
- `use` still resolves only a namespace-looking binding rather than importing a
  real namespace registry,
- types are not yet registered under qualified namespace names.

This is larger than JSON and should be staged carefully.

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
`decode_value[T]`, and enum/field helpers. Jett does not yet have explicit
public/private module visibility.

Until visibility exists, extraction should avoid pretending helper names are
private. A first stdlib experiment can use internal-looking names, but shipping
the module cleanly probably needs an export rule or another visibility story.

## Minimal Staging Plan

1. Keep the compiler-known public JSON bridge for policy checks and bootstrap
   fallback paths.
2. Continue staging extracted prototype code under flat, non-conflicting names:
   - `json_serialize_reflected[T](view value: T)` exists in stdlib.
   - `json_serialize_public_reflected[T](view value: T)` exists in stdlib.
   - `json_decode_reflected[T](raw: JsonValue)` exists in stdlib.
   - `json_parse_reflected[T](raw: string)` exists in stdlib as the raw-string
     wrapper around `json.parse_raw` and the reflected decoder.
3. Continue the public bridge handoff. `json.parse`, `json.serialize`, and
   `json.serialize_public` now use compiler-owned typechecker policy with
   stdlib-owned interpreter bodies. See `docs/json_public_bridge_handoff.md`.
4. Move the extracted functions into `namespace json` behind the real public
   names, while retaining compiler policy checks for secrets, `view`, map keys,
   and handled results.
5. Keep broad bridge/parity tests before removing any Rust-backed fallback
   implementation paths.

## Recommended Shape For `stdlib/json.jett`

The eventual module should keep these layers distinct:

- `quote(raw: string) returns string`
- `serialize_value[T](view value: T) returns result[string, string]`
- `serialize_public_value[T](view value: T) returns result[string, string]`
- `decode_value[T](raw: JsonValue) returns result[T, string]`
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
  `export` or `private` rule?
- How much abstraction is allowed around trusted reflection loops before
  `comptime type Field = field.type_info:` loses provenance?
