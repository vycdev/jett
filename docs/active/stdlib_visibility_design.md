# Stdlib Visibility and Trusted Origins

This note records the visibility work that blocks a fully ordinary stdlib
`json` public API.

## Problem

Jett now has namespaces, explicit `export` declarations, and private-by-default
visibility inside namespaces. The interpreter also has a narrow trusted-source
identity for functions parsed from compiler-shipped stdlib files. That gives the
stdlib a workable public/private surface, but it is not yet a full module,
import, or prelude system.

That matters for JSON because the module needs both:

- public functions such as `json.parse`, `json.parse_exact`, `json.serialize`, and
  `json.serialize_public`,
- many helper functions such as `json_decode_record_reflected` and
  `json_stdlib_quote`,
- compiler-owned policy gates around public JSON calls,
- a way for the interpreter to know that a helper came from bundled stdlib code,
  not from a project file with the same qualified name.

The current staging model keeps public JSON names compiler-owned, requires
trusted internal reflected hooks, and executes trusted exported wrappers as the
runtime body boundary. That is intentionally conservative.

Two concepts must stay separate:

- `export` answers: can ordinary source code name this declaration from outside
  the namespace?
- trusted stdlib origin answers: may compiler-owned policy code delegate to this
  implementation?

Do not make trusted origin a source-level keyword. Trust is a property of how a
module was loaded by the compiler, not a promise a user file can write.

## Namespace Leakage Removed

Namespaced declarations are registered under qualified names, for example
`json.parse`. Same-namespace shorthand now resolves through lexical namespace
context instead of global flat aliases.

The old bootstrap leakage was useful early on, but it is not the export model:

- a helper from `namespace json` should not appear as an unqualified global
  name outside that namespace,
- helper visibility is represented by the presence or absence of `export`,
- trusted origin currently exists only as interpreter staging metadata, not as a
  source-level visibility or export rule.

## Requirements

The eventual module system should support:

- unqualified references within the current namespace,
- qualified references from outside the namespace,
- explicit public exports for user-facing API,
- private or module-local helpers,
- a trusted-origin marker for compiler-shipped support modules,
- no accidental global flat aliases for namespaced declarations,
- compiler policy gates that can call trusted stdlib bodies without trusting
  arbitrary user/project definitions.

For the transition, unnamespaced top-level user declarations can keep their
current public behavior. The private-by-default rule should apply first to
declarations inside an explicit `namespace`, where accidental helper leakage is
the real problem.

## Options

### 1. Export By Default, Private Helpers

All declarations are public unless marked private.

This is simple, but it fits Jett poorly: helper-heavy modules such as `json`
would expose too much by default, and LLM-generated code would need to remember
to hide helpers.

### 2. Private By Default, Explicit `export`

Top-level declarations inside a namespace are module-private unless marked with
an `export` keyword.

This is safer for stdlib and large projects. It also fits the "one canonical
form" principle: public API is mechanically searchable by `export`.

Example shape:

```jett
namespace json

export function parse[T](raw: string) returns result[T, string]:
    return json_parse_reflected[T](raw)

function json_parse_reflected[T](raw: string) returns result[T, string]:
    ...
```

This syntax is now implemented with `export` prefixing each public declaration.

### 3. Separate Trusted Stdlib Registry

Keep language visibility for later, but have the driver/interpreter register
compiler-shipped modules in a separate trusted registry.

This directly solves public JSON bridge trust, but not helper visibility. It is
a useful staging step, not the whole module story.

This stage is now implemented for interpreter function calls: file ids at or
above the reserved stdlib range are marked trusted, and public JSON builtins
only delegate to reflected hook names when the current registry entry is trusted.
An untrusted registration of the same name clears that trust.

## Recommendation

Use two stages, with source visibility and compiler trust kept orthogonal:

1. Keep the trusted stdlib identity for compiler-owned bridge hooks. Done for
   the interpreter; future codegen should use the same notion.
2. Add explicit exports, probably private-by-default within namespaces, before
   treating the stdlib `json` module as a clean public module. Done for the current
   parser/resolver/typechecker/interpreter surface; a full import/prelude model
   remains separate.

Both stages now exist for the current JSON interpreter path, but they do not
make public JSON policy ordinary library behavior yet. Keep compiler-owned JSON
policy checks until the broader import/prelude story and future backends can
carry the same trusted-origin guarantees.

The JSON bridge uses a compiler-owned hook table, not public function name
guessing:

```text
json.parse            requires json.json_parse_reflected
json.parse_exact      requires json.json_parse_exact_reflected
json.serialize        requires json.json_serialize_reflected
json.serialize_public requires json.json_serialize_public_reflected
```

The right-hand side may be private source code as long as it came from a
trusted compiler-shipped stdlib module. Ordinary user/project code cannot
satisfy that table merely by declaring the same qualified name. At runtime the
interpreter also requires the exported public wrapper itself to be trusted, then
executes that wrapper as the body boundary.

## JSON-Specific Staging

For now:

- public `json.parse`, `json.parse_exact`, `json.serialize`, and
  `json.serialize_public` remain compiler-owned policy gates,
- the interpreter verifies both the trusted private hook and the trusted
  exported public wrapper, then executes the public wrapper from `stdlib/json/`,
- parser walkers, reflected decoders, quoting helpers, and bridge hooks are now
  namespace-private source declarations. Their `json_` prefixes remain useful
  for readability, but privacy no longer depends on naming convention.

Before removing compiler-owned JSON policy gates, finish the broader
module/import/prelude story and decide how trusted private hooks should be
represented outside the current interpreter registry.

## Current Implementation Status

Implemented:

- `export` syntax for top-level `function`, `struct`, `enum`, `bitfield`,
  `type`, and `interface` declarations.
- Per-declaration `export function` syntax inside `mutual` blocks, allowing a
  mixed public/private recursive API surface.
- Resolver visibility metadata on definitions, including namespace identity.
- Private-by-default resolver diagnostics for references to namespaced
  declarations from outside their namespace.
- Namespace-private `mutual` declarations are enforced for qualified, aliased,
  and old flat compatibility references.
- External unqualified access to exported namespaced declarations is rejected;
  users must write the qualified path or a namespace alias.
- The resolver no longer creates root-scope leaf bindings for namespaced
  declarations; same-namespace shorthand resolves through the canonical
  namespace path.
- The typechecker now registers namespaced functions, types, aliases, generic
  struct templates, and reflection metadata under canonical namespace keys
  instead of leaf aliases.
- The comptime/runtime interpreter now registers namespaced functions,
  structs, actors, bitfields, enums, machines, and type aliases under canonical
  namespace keys instead of leaf aliases.
- Verify/property blocks and runtime `main` execute with their lexical
  namespace context, so same-namespace private helpers still resolve without
  flat aliases.
- Parameter, return, and assignment refinement checks resolve aliases in the
  active lexical namespace.
- Same-namespace private helper access remains allowed.
- Existing user/project namespace fixtures now mark public cross-namespace API
  with `export`.
- The public raw/tree helper surface in `stdlib/json/` fragments is explicitly
  exported while parser span scanners, reflected decoders, serializers, and
  bridge hooks stay private.
- The stdlib JSON module exports `json.JsonValue = JsonTree` as the namespaced
  source-level alias and `export root type JsonValue = json.JsonTree` as the
  narrow root compatibility alias. In stdlib-loaded code the unqualified
  spelling now reflects as an alias to `json.JsonTree`; the legacy
  `TypePrimitive.json_value_type` tag remains only for bootstrap/no-stdlib
  fallback paths.
- Driver-level `build_source` coverage now verifies that in-memory/LSP-style
  validation sees the exported `json.JsonTree` raw facade surface, not only the
  marker stdlib module.
- Driver hover coverage now verifies that editor type queries see the
  `json.parse_raw` facade as `result[json.JsonTree, string]`.
- Driver hover coverage also pins the `JsonTree`-first return types for the
  raw facade surface, including lenient lookups, strict lookup helpers, scalar
  casts, predicates, key/length helpers, and raw serialization.
- Driver completion coverage now filters out private namespaced stdlib JSON
  bridge hooks while still exposing the exported `json.JsonTree`,
  `json.JsonValue`, parse/serialize wrappers, and raw facade surface. The
  exported JSON surface remains namespaced in source and completions; bare
  `JsonValue` is the narrow root-compatibility exception.
- LSP completions are now cursor-position aware for namespace visibility:
  private helpers are offered inside their own namespace but hidden from
  external namespaces.

Still staged:

- Public JSON policy remains compiler-owned; source `export` is not trusted
  origin.

## Staged Implementation Plan

1. Add visibility metadata to declarations in AST/resolver/typechecker.
   Prefer `export function`, `export struct`, `export enum`, `export bitfield`,
   `export type`, and `export interface` over export lists for the first pass;
   that keeps the public API mechanically searchable.
2. Preserve current behavior for declarations outside a namespace.
3. Make namespace declarations private by default outside their namespace.
   Within the same namespace, unqualified helper calls continue to resolve
   locally.
4. Stop registering every namespaced declaration as a global flat alias. Keep
   namespace-local lookup instead.
5. Keep compiler-owned JSON facades policy-first in the typechecker and
   wrapper-bodied in the interpreter. Export controls whether users can name a
   declaration; it does not decide whether compiler policy routes through it.
6. Add the compiler-owned trusted hook table for public JSON facades. The table
   maps public facade names to private trusted stdlib implementations and is not
   expressible from source.
7. Mark only the real stdlib JSON surface as exported. Keep parser walkers,
   reflected decoders, quoting helpers, and bridge hooks private unless there is
   a user-facing reason to expose them. Done for the current `JsonTree` surface.

## Covered Tests

- Outside a namespace, code can call exported qualified declarations and cannot
  call private qualified helpers.
- Inside the same namespace, private helpers are callable unqualified and, if
  the resolver supports it, through their qualified local name.
- Namespaced helpers are no longer visible through accidental flat aliases.
- Exported namespaced declarations are not visible through accidental flat
  aliases from outside their namespace.
- Stdlib JSON exports follow the same rule: `json.parse_raw` and
  `json.JsonTree` are available, while flat `parse_raw` and `JsonTree` are
  rejected; bare `JsonValue` remains the explicit root alias.
- Public JSON policy remains compiler-owned: `view`, secret, map-key, and
  handled-result rules still fire even if public wrapper declarations exist.
- Project code cannot reopen compiler-shipped stdlib namespaces such as
  `json`; the prepended stdlib namespace declaration wins and later project
  declarations collide instead of replacing trusted symbols. Pinned by
  `tests/compile_fail/stdlib_namespace_collision.jett`, with driver completion
  coverage ensuring invalid in-memory `namespace json` files do not receive
  same-namespace access to private stdlib hooks.
- A project-defined `namespace json function json_parse_reflected...` cannot
  satisfy the trusted public JSON bridge, and untrusted later registration of
  JSON hook or public wrapper names clears trust. Interpreter unit coverage pins
  parse, parse_exact, serialize, serialize_public, and public raw facade wrapper
  dispatch.
- Ordinary source access to private JSON hooks is rejected for tree parsing,
  reflected parsing, exact reflected parsing, reflected decoding, and reflected
  serialization fixtures under `tests/compile_fail/json_private_*.jett`.
- `jett build`, `jett run`, `jett test file`, project tests, and LSP-style
  `build_source` all see the same stdlib exports. The marker stdlib module and
  JSON raw facade are covered for `build_source`; file/project paths are covered
  by the run-pass and `test_file` fixtures. Hover/type-query coverage pins the
  JSON raw facade type through the same stdlib-loading path. Completion coverage
  pins exported namespaced declarations without leaking private JSON bridge
  hooks, while position-aware completions keep same-namespace private helpers
  available.

## Remaining Test Gaps

- Codegen should eventually get equivalent trusted-origin coverage once JSON
  bridge delegation moves beyond the comptime/runtime interpreter.
- The future import/prelude design should decide whether additional root aliases
  are ever allowed and when the legacy `TypePrimitive.json_value_type`
  compatibility tag can retire.
