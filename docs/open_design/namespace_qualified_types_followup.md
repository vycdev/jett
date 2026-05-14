# Namespace-Qualified Types Follow-Up

## Current State

Jett now accepts namespace-qualified type spellings such as `models.User`,
`models.Box[int64]`, `models.Color.blue`, and `models.Header(...)`.

This is intentionally implemented as a small compatibility layer over the
existing flat declaration model:

- The resolver now declares namespaced top-level values and types by their
  qualified `namespace.Name` symbol, while preserving a first unambiguous flat
  alias for legacy single-namespace fixtures.
- The typechecker uses the qualified symbol as the canonical interner metadata
  name for namespaced structs, enums, bitfields, generic structs, aliases,
  interfaces, and actors.
- The comptime interpreter registers both leaf and qualified runtime
  definitions, so reflection and JSON can use qualified type arguments.
- Function-local namespace aliases from `use models as m` now expand qualified
  references such as `m.User`, `m.make[T](...)`, and `m.Color.active` back to
  the canonical `models.*` symbols. Reflection still reports canonical names.
- Qualified interface implementations such as
  `implement contracts.Named for models.User` now parse, typecheck, and run
  through qualified interface method calls like `contracts.Named.name(view user)`.

That was the right first bite because it unblocks stdlib-style reflection and
JSON code without changing the language's broader namespace semantics.

## Known Limits

- Two namespaces can now define the same leaf type name when callers use fully
  qualified names such as `a.User` and `b.User`.
- Unqualified flat aliases remain a compatibility path and should not be treated
  as the final namespace model.
- `use models as m` is a local namespace alias, not a type alias. The registered
  spelling and reflected metadata remain `models.User`.
- Interfaces, actors, and state machines still have some leaf-name-oriented
  paths. Qualified struct, enum, bitfield, generic struct, reflection, JSON,
  and explicitly qualified interface implementation paths are covered first
  because they are the JSON extraction blocker.
- Typechecker canonical names for namespaced user types now use the qualified
  spelling in reflection and `TypeId` display paths covered by the current
  tests.

## Design Pressure

Jett's core design favors deterministic searchability and easy agent edits. A
namespace system should preserve those strengths:

- Fully qualified names should be canonical when crossing module boundaries.
- Local shorthand should not create multiple equivalent ways to write public
  APIs.
- Renames should remain grep-friendly.
- Diagnostics and reflection should agree on the name a user wrote, or clearly
  choose one canonical form.

## Options

### Option A: Keep Flat Leaf Names Globally Unique

Continue treating namespaces mostly as callable/type prefixes, while requiring
leaf type names to be unique across a build.

Pros:

- Smallest semantic change.
- Preserves the current "flat query" model for agents.
- Avoids ambiguous local shorthand and import alias questions.

Cons:

- Namespaces are less useful for large codebases.
- `models.User` and `auth.User` cannot coexist.
- The surface looks more namespace-rich than the compiler actually is.

### Option B: Fully Qualified Canonical Symbols

Make every top-level declaration's canonical symbol `namespace.name`, with
unqualified names available only inside the current namespace or through an
explicit import/use mechanism.

Pros:

- Real namespace isolation.
- Reflection, diagnostics, and runtime values can use one canonical name.
- Duplicate leaf names across namespaces become possible.

Cons:

- Requires resolver/scope-table changes, not only typechecker aliases.
- Needs a clear rule for local shorthand and `use`.
- Existing tests and diagnostics that assume leaf names may need updates.

### Option C: Qualified Canonical Symbols, Local Namespace Aliases

Adopt Option B for canonical symbols, while allowing function-local namespace
aliases to shorten repeated references. Aliases expand back to canonical symbols
before typechecking and runtime lookup.

Pros:

- Strong canonicality for LLM generation.
- Keeps reflection and diagnostics on canonical names.
- Renames and searches remain straightforward.

Cons:

- Introduces a second local spelling inside functions.
- Requires alias expansion to stay local and explicit.

## Recommendation

Prefer Option C if Jett wants namespaces to become real isolation boundaries.
Local aliases are acceptable as a function-scoped readability tool because they
do not change canonical symbol names, reflection metadata, or public API
identity.

The next implementation step should be to keep replacing flat fallback paths
with canonical qualified symbols, while treating alias expansion as a local
front-end convenience.

## Suggested Next Tests

- Duplicate-leaf bitfields and `use`-alias bitfield references are now covered
  by the namespace run-pass fixtures.
- Explicitly qualified interface implementations and qualified interface method
  calls are now covered by a namespace run-pass fixture.
- Add more duplicate-leaf coverage beyond structs, enums, generic structs,
  bitfields, and same-leaf functions. Same-leaf functions are now covered both
  by direct qualified calls and `use`-alias calls, so actors and duplicate-leaf
  interfaces remain.
- `type.name[a.User]()` returns `a.User`.
- `type.fields[a.User]()` reports field metadata with owner `a.User`.
- `json.parse[a.User]` and `json.serialize[a.User]` roundtrip only the intended
  type.
- Broader `use a as alias` coverage for interfaces and actors.
