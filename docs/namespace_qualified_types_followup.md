# Namespace-Qualified Types Follow-Up

## Current State

Jett now accepts namespace-qualified type spellings such as `models.User`,
`models.Box[int64]`, `models.Color.blue`, and `models.Header(...)`.

This is intentionally implemented as a small compatibility layer over the
existing flat declaration model:

- The resolver still declares user types by their leaf name.
- The typechecker registers both the historical leaf name and the qualified
  `namespace.Name` spelling for types in a namespace.
- The comptime interpreter registers both leaf and qualified runtime
  definitions, so reflection and JSON can use qualified type arguments.

That was the right first bite because it unblocks stdlib-style reflection and
JSON code without changing the language's broader namespace semantics.

## Known Limits

- Two namespaces still cannot define the same leaf type name in the same build.
  `namespace a; struct User` and `namespace b; struct User` collide today.
- `use models as m` does not make `m.User` a type alias. The registered spelling
  is still `models.User`.
- Interfaces, implementations, actors, and state machines still have some
  leaf-name-oriented paths. Qualified struct, enum, bitfield, generic struct,
  reflection, and JSON paths are covered first because they are the JSON
  extraction blocker.
- Typechecker canonical names are mixed: type expressions preserve the
  qualified spelling for `type.name[T]()` and reflection values, while some
  internal `TypeId` display paths still use the leaf declaration name.

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

### Option C: Qualified Canonical Symbols, No Import Aliases For Types

Adopt Option B for canonical symbols, but require external type references to
use the full namespace path. `use` can remain an expression/function convenience
or be deferred.

Pros:

- Strong canonicality for LLM generation.
- Avoids `m.User` versus `models.User` ambiguity.
- Renames and searches remain straightforward.

Cons:

- More verbose external references.
- Existing `use` semantics become asymmetrical unless documented carefully.

## Recommendation

Prefer Option C if Jett wants namespaces to become real isolation boundaries.
It matches the language's "one obvious spelling" philosophy better than alias
heavy imports, and it keeps agent search behavior clean.

The next implementation step should not be more ad hoc aliasing. It should be a
resolver-level change that stores fully qualified top-level symbols, defines
when unqualified local names are in scope, and makes typechecker/interpreter
metadata consume those canonical names.

## Suggested Next Tests

- `namespace a` and `namespace b` can both define `User`, and `a.User(...)` /
  `b.User(...)` construct distinct shapes.
- `type.name[a.User]()` returns `a.User`.
- `type.fields[a.User]()` reports field metadata with owner `a.User`.
- `json.parse[a.User]` and `json.serialize[a.User]` roundtrip only the intended
  type.
- `use a as alias` with `alias.User` is either accepted intentionally or
  rejected with a clear diagnostic.
