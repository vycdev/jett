# Stdlib Visibility and Trusted Origins

This note records the visibility work that blocks a fully ordinary
`stdlib/json.jett` public API.

## Problem

Jett currently has namespaces, but it does not yet have public/private exports.
The interpreter now has a narrow trusted-source identity for functions parsed
from compiler-shipped stdlib files, but that identity is not yet a full module
visibility system.

That matters for JSON because the module needs both:

- public functions such as `json.parse`, `json.serialize`, and
  `json.serialize_public`,
- many helper functions such as `json_decode_record_reflected` and
  `json_stdlib_quote`,
- compiler-owned policy gates around public JSON calls,
- a way for the interpreter to know that a helper came from bundled stdlib code,
  not from a project file with the same qualified name.

The current staging model keeps public JSON names compiler-owned and routes
runtime bodies through internal reflected hook names. That is intentionally
conservative.

Two concepts must stay separate:

- `export` answers: can ordinary source code name this declaration from outside
  the namespace?
- trusted stdlib origin answers: may compiler-owned policy code delegate to this
  implementation?

Do not make trusted origin a source-level keyword. Trust is a property of how a
module was loaded by the compiler, not a promise a user file can write.

## Current Namespace Leakage

Namespaced declarations are registered under qualified names, for example
`json.parse`, but they are also staged with a flat alias in several places so
unqualified calls inside the same namespace keep working.

That is useful during bootstrap, but it is not the long-term export model:

- a helper from `namespace json` can appear as an unqualified global name,
- helpers are public by convention rather than by declaration,
- helper visibility is not represented in the language,
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

Open syntax decision: whether `export` prefixes declarations, or whether a
namespace has an explicit export list.

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
   treating `stdlib/json.jett` as a clean public module.

Do not remove compiler-owned JSON policy checks until both stages exist.

The long-term JSON bridge should use a compiler-owned hook table, not public
function name guessing:

```text
json.parse            -> json.json_parse_reflected
json.serialize        -> json.json_serialize_reflected
json.serialize_public -> json.json_serialize_public_reflected
```

The right-hand side may be private source code as long as it came from a
trusted compiler-shipped stdlib module. Ordinary user/project code cannot
satisfy that table merely by declaring the same qualified name.

## JSON-Specific Staging

For now:

- public `json.parse`, `json.serialize`, and `json.serialize_public` remain
  compiler-owned policy gates,
- the interpreter delegates to `json.json_parse_reflected`,
  `json.json_serialize_reflected`, and
  `json.json_serialize_public_reflected` only when those registry entries came
  from compiler-shipped stdlib files,
- public wrappers in `stdlib/json.jett` stay as readable declarations and a
  preview of the eventual API,
- helper names remain prefixed to reduce collisions, but that is convention,
  not real privacy.

Before changing the hook names into ordinary public wrappers, implement
exports/private helpers.

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
- Same-namespace private helper access remains allowed.
- Existing user/project namespace fixtures now mark public cross-namespace API
  with `export`.
- The public `JsonTree` API in `stdlib/json.jett` is explicitly exported while
  parser, decoder, serializer, and reflected bridge helpers stay private.

Still staged:

- The typechecker and interpreter still keep temporary flat compatibility
  registrations. The resolver now rejects external private flat access, but the
  registries still need to stop creating those aliases.
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
5. Keep compiler-owned JSON facades builtin-first in the typechecker and
   interpreter. Export controls whether users can name a declaration; it does
   not decide whether compiler policy routes through it.
6. Add the compiler-owned trusted hook table for public JSON facades. The table
   maps public facade names to private trusted stdlib implementations and is not
   expressible from source.
7. Mark only the real stdlib JSON surface as exported. Keep parser walkers,
   reflected decoders, quoting helpers, and bridge hooks private unless there is
   a user-facing reason to expose them. Done for the current `JsonTree` surface.

## Required Tests

- Outside a namespace, code can call exported qualified declarations and cannot
  call private qualified helpers.
- Inside the same namespace, private helpers are callable unqualified and, if
  the resolver supports it, through their qualified local name.
- Namespaced helpers are no longer visible through accidental flat aliases.
- Public JSON policy remains compiler-owned: `view`, secret, map-key, and
  handled-result rules still fire even if public wrapper declarations exist.
- A project-defined `namespace json function json_parse_reflected...` cannot
  satisfy the trusted public JSON bridge, and an untrusted later registration of
  a hook name clears trust.
- `jett build`, `jett run`, `jett test file`, project tests, and LSP-style
  `build_source` all see the same stdlib exports.
