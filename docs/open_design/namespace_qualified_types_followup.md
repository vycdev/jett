# Namespace-Qualified Types Follow-Up

## Current State

Jett now accepts namespace-qualified type spellings such as `models.User`,
`models.Box[int64]`, `models.Color.blue`, and `models.Header(...)`.

The implementation has moved from the early flat-alias compatibility layer
toward canonical qualified declarations:

- The resolver declares namespaced top-level values and types by their
  qualified `namespace.Name` symbol. External unqualified flat access is
  rejected; same-namespace lookup remains ergonomic inside the namespace.
- The typechecker uses the qualified symbol as the canonical interner metadata
  name for namespaced structs, enums, bitfields, generic structs, aliases,
  interfaces, and actors.
- The comptime interpreter registers namespaced runtime definitions under their
  qualified symbols and resolves local unqualified names through the current
  namespace when executing a namespaced body.
- Function-local namespace aliases from `use models as m` now expand qualified
  references such as `m.User`, `m.make[T](...)`, and `m.Color.active` back to
  the canonical `models.*` symbols. Reflection still reports canonical names.
- `comptime type` binding through namespace aliases is covered for exported
  aliases, refinements, and generic field metadata.
- Qualified interface implementations such as
  `implement contracts.Named for models.User` now parse, typecheck, and run
  through qualified interface method calls like `contracts.Named.name(view user)`.
- Duplicate-leaf interfaces are covered with both direct qualified calls and
  function-local namespace aliases.
- Exported actors can be spawned through qualified names such as
  `spawn workers.Counter(...)` and function-local namespace aliases such as
  `use workers as w; spawn w.Counter(...)`; namespaced actor bodies are checked
  against their canonical owner metadata.
- Duplicate-leaf actors can now coexist when callers use fully qualified names
  or explicit namespace aliases; actor runtime registration and message dispatch
  preserve the canonical owner identity.

That was the right first bite because it unblocks stdlib-style reflection and
JSON code without changing the language's broader namespace semantics.

## Known Limits

- Two namespaces can now define the same leaf type name when callers use fully
  qualified names such as `a.User` and `b.User`.
- Ordinary typechecker and interpreter registries now resolve namespaced
  declarations through canonical qualified names, namespace aliases, or the
  current lexical namespace. The remaining leaf-oriented pressure is limited to
  explicitly deferred areas such as state machines and bootstrap compatibility
  paths.
- `use models as m` is a local namespace alias, not a type alias. The registered
  spelling and reflected metadata remain `models.User`.
- State machines still have some leaf-name-oriented paths. Qualified actor
  spawn and duplicate-leaf actor owners are covered for exported actors.
  Qualified struct, enum, bitfield, generic struct, reflection, JSON, and
  interface implementation paths are covered first because they were the JSON
  extraction blocker.
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

The next implementation step is to leave ordinary type/function paths on
canonical qualified symbols and avoid extending the remaining state-machine
fallback behavior until the machine type model is explicit.

## Deferred Machine Work

Actor spawn now accepts qualified and `use`-alias names for exported actors,
namespaced actor bodies are checked through their canonical `namespace.Actor`
metadata, and duplicate-leaf actor owners are covered by
`tests/run_pass/namespace_duplicate_leaf_actors.jett`.

State machines have more runtime namespace support than actor spawn, but their
type-system story is still thin. Treat namespaced machine tests as design probes
until the machine type model is made explicit. See
`docs/open_design/state_machine_type_model.md`.

## Suggested Next Tests

- State machines: add a small namespaced machine fixture once the machine type
  model is explicit enough for diagnostics and reflection to agree on canonical
  names.
- Interfaces: the direct qualified, `use`-alias, and duplicate-leaf interface
  paths are now covered by run-pass fixtures.
- Structs, enums, generic structs, bitfields, same-leaf functions, type names,
  field metadata, `comptime type` bindings, and JSON parse/serialize are now
  covered by the namespace and reflection run-pass fixtures.
