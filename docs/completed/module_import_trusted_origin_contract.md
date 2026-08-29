# Module, Import, Prelude, and Trusted-Origin Contract

Status: design selected. The current compiler implements canonical namespaces,
block-local `use`, namespace export visibility, stdlib loading, and an
interpreter-only trusted-file convention. The module registry, dependency graph,
and backend-neutral origin propagation defined here remain staged.

Tracked by [#3](https://github.com/vycdev/jett/issues/3).

## Decision Summary

Jett has one module identity model:

- A namespace is the source-level module boundary.
- Every project or dependency namespace has exactly one owning source file.
- Only compiler-shipped stdlib sources may contribute ordered fragments to the
  same namespace.
- A canonical declaration identity contains its source origin, namespace, and
  declaration name. Textual spelling alone is never identity.
- `use` binds one existing namespace into a lexical block. It does not load
  files, execute code, copy declarations, or grant trust.
- Project and vendored dependency namespaces require a local `use` before
  executable references. Canonical qualified types remain valid in declaration
  signatures, which have no block-local import scope.
- Compiler-shipped stdlib namespaces are addressable by canonical qualification
  without `use`. Their members remain namespaced and obey `export`.
- The prelude is a small compiler-owned manifest of foundational declarations,
  not wildcard namespace injection. It cannot create root type aliases.
- Trusted stdlib origin is compiler-assigned metadata that survives every
  compiler phase. It is independent of `export` and cannot be written in Jett
  source.

This preserves Jett's canonical spelling and local-context rules while removing
incidental file order and reserved `FileId` ranges as long-term authorization
mechanisms.

## Vocabulary and Identity

### Source origins

Every discovered source has one immutable `SourceOrigin`:

```text
SourceOrigin =
    Project(ProjectKey)
    Dependency(DependencyKey)
    Stdlib(StdlibKey)
```

The keys are compiler records, not source strings:

```text
ProjectKey {
    canonical_name
}

DependencyKey {
    canonical_name
    graph_path
}

StdlibKey {
    compiler_distribution
    stdlib_version
}

DiscoveryRoot {
    origin: SourceOrigin
    physical_root
}
```

There is one `ProjectKey` for the selected build root. A dependency's
`graph_path` is its normalized sequence of vendored dependency-directory
segments from that project, so two dependency manifests with the same display
name remain distinct origins. `physical_root` is a separately held,
canonicalized host path used only for containment, duplicate-root, and cycle
checks. It is not semantic identity, is never source-visible, and never enters
diagnostic ordering or compiler cache keys. Relocating an unchanged checkout
therefore preserves origin and file identities, while paths and symlink aliases
that resolve to the same physical root are still detected and rejected rather
than silently collapsed.

Only the driver may construct `Stdlib(StdlibKey)`, and only for files discovered
beneath the stdlib directory selected by the compiler installation. A project
path, command-line flag, dependency manifest, namespace name, or source
annotation cannot request stdlib origin.

The compiler installation is the trust boundary. This contract does not claim
that compiler files are cryptographically authenticated. Packaging and host
integrity are outside the language model.

### File identity

A stable file key is:

```text
FileKey {
    origin: SourceOrigin
    logical_path: normalized relative path
}
```

Logical paths use `/`, reject absolute paths and `..`, and are unique within an
origin. Physical paths may be retained for I/O and local source lookup, but
semantic identity, portable diagnostics, and persistent cache records use
`FileKey`. Host-absolute roots must not affect compiler output.

The narrow syntax-only parse-object exception is defined by the
[content-addressed cache contract](content_addressed_compilation_cache_contract.md).
That object may deduplicate exact source bytes without putting `FileKey` in its
object key or payload because parsing does not consume origin or path: lookup
already owns the current `FileKey`, cached spans contain no authority, and
decode rebinds every span and diagnostic to that caller identity. Any artifact
whose fact, diagnostic, namespace, or policy depends on source identity must
include `FileKey` or a canonical derivative. Content equality must never upgrade
origin or carry trust across callers.

`FileId` remains a compact diagnostic handle allocated for one compiler
session. Its numeric range must not decide source authority after discovery.
The current reserved stdlib range is a bootstrap representation that must be
translated into `SourceOrigin::Stdlib` at the driver boundary.

### Module and declaration identity

A module is identified by origin and canonical namespace:

```text
ModuleId {
    origin: SourceOrigin
    namespace: CanonicalNamespace
}
```

A declaration is identified by its owning module and canonical declared name:

```text
DeclarationId {
    module: ModuleId
    name: CanonicalName
    kind: DeclarationKind
}
```

Generic instantiations, implementations, and later HIR/MIR symbols derive from
`DeclarationId`; they do not reconstruct authority by comparing strings.
Namespace aliases affect only local lookup. Reflection, diagnostics, caches,
linkage, and policy-hook selection retain canonical identities.

## Discovery and Module Registry

### Discovery roots

A build has three ordered origin classes:

1. compiler-shipped stdlib,
2. vendored dependencies,
3. the project.

The project root is the directory containing the selected `jett.proj`.
Dependencies remain vendored source tracked with the project. Each immediate
child under `deps/` that contains its own `jett.proj` is a dependency root;
that dependency may have its own `deps/` directory. There is no package registry,
network fetch, implicit global cache, or lock-file lookup during compilation.

Dependency roots are canonicalized and containment-checked. A root may appear
only once in the graph. Cycles and duplicate dependency identities are errors,
not lexical tie-breaks.

The initial registry does not add a second dependency declaration syntax to
`jett.proj`. If a future package manager needs explicit source locations, it
must extend this same graph rather than create another import mechanism.

### Namespace ownership

Discovery parses namespace declarations and builds the complete registry before
name resolution. These invariants apply:

- A project or dependency namespace has one owning `FileKey`.
- A source file may contain more than one namespace, but each namespace still
  has that one file as its owner.
- The same namespace cannot be declared by two project files, two dependency
  files, or a project and a dependency.
- A project or dependency cannot declare a namespace owned by stdlib.
- Two dependencies cannot expose the same canonical namespace, even when their
  package names differ.
- Only files with `SourceOrigin::Stdlib` may contribute multiple fragments to
  one stdlib namespace.
- Duplicate declarations inside merged stdlib fragments remain errors.

Global namespace uniqueness keeps `use reports` deterministic without adding a
package qualifier or overload-like lookup rule. A collision must name both
origins and declaration locations.

### Deterministic order

Dependency roots form a directed acyclic graph. The compiler processes
dependencies before dependents. Independent roots are ordered by canonical
origin name and then normalized dependency `graph_path`. Physical discovery
roots are deliberately excluded so checkout location cannot change declaration
or diagnostic order.

Within a project or dependency, source files are ordered by normalized logical
path. Since a namespace has one owner, this order is for deterministic discovery,
diagnostics, and whole-project orchestration rather than namespace merging.
Declarations within each file retain source order and Jett's top-to-bottom rule.

Stdlib loading keeps its existing deliberate exception:

1. root stdlib files before nested files,
2. shallower paths before deeper paths,
3. lexical logical-path order at equal depth.

Numbered fragments such as `json/10_*.jett` therefore retain their explicit
order. Fragment order is part of the compiler-shipped stdlib build, not a feature
available to project modules.

A later query engine may evaluate independent parses or modules concurrently.
It must publish declarations, diagnostics, and artifacts in this canonical
order and must not make source validity depend on scheduling.

### Module dependencies

Discovery creates the namespace registry; resolution creates module-dependency
edges from canonical qualified references and `use` targets. The graph records
which module interfaces a module consumes. It does not execute imports.

Cross-module resolution reads an already checked exported interface. Inside a
module, declarations remain strictly top-to-bottom, with `mutual:` as the only
source-level forward-reference mechanism. The first implementation may reject
module dependency cycles with a focused diagnostic. It must not silently merge
cyclic modules or treat a cycle as a global `mutual:` block.

## `use` and Name Resolution

### One source form

The only import spelling remains:

```jett
use reports
use net.http
use reports as r
```

`use path` binds the final segment in the current lexical block. `use path as
alias` binds the explicit alias. The target is always a canonical namespace in
the registry.

A `use` statement:

- appears only at the beginning of a function or nested block,
- is visible from that statement through the end of that block,
- may be shadowed only according to ordinary local duplicate-name rules,
- has no runtime representation or initialization effect,
- does not expose private declarations,
- does not change declaration or origin identity,
- does not import one member or use a wildcard.

A missing namespace, duplicate local binding, namespace collision, or private
member access is reported at the relevant source span.

### Project and dependency access

Executable references from one project/dependency namespace to another require
a visible `use`, including references written with the full canonical namespace.
This keeps dependencies local to the function or block being read.

Declaration signatures have no block body in which to place `use`. They may use
canonical qualified project or dependency types directly. They may not use a
function-local alias, and their qualified references create module-dependency
edges.

Same-namespace declarations remain available through local shorthand according
to source order. Their canonical identity is still qualified.

### Stdlib access

Compiler-shipped stdlib namespace names are available for canonical qualified
access without a required `use`:

```jett
json.parse[Config](raw)
list.length[string](view values)
```

A local `use json as j` may shorten repeated references inside one block, but it
never changes the canonical `json.*` identity. Stdlib members are not injected
as unqualified names merely because their namespace is implicit.

This is a language convenience for the shipped standard surface, not trust.
Untrusted source cannot gain implicit status by choosing a stdlib namespace,
and namespace ownership rejects that collision before ordinary lookup.

## Export and Prelude Policy

### Export remains visibility only

Namespaced declarations are private by default. `export` allows callers outside
the owning namespace to name a declaration. It does not:

- add the declaration to the prelude,
- make a namespace implicit,
- mark code trusted,
- permit compiler policy delegation,
- create a root alias,
- weaken project/dependency boundaries.

Private declarations stay available inside their owning namespace and may be
selected by a compiler policy hook only when their resolved origin is trusted
stdlib.

### Fixed prelude manifest

The prelude is a versioned compiler-owned manifest. It contains only
foundational declarations that Jett source is designed to spell unqualified,
such as the implemented `Equatable` interface, plus language-provided primitive
and generic types and intrinsic constructors.

Each manifest entry points to one canonical declaration identity. It does not
copy a declaration into a synthetic root namespace. The manifest is loaded
before project resolution and is identical for every source file in the build.

Rules for the prelude:

- no wildcard namespace imports,
- no project or dependency entries,
- no source-controlled additions,
- no per-file or per-project variation,
- no ambiguity fallback,
- no root type aliases,
- no alternate reflected or linkage identity.

Public modules such as `json`, `time`, `list`, and `map` are implicit namespace
names, not sets of unqualified prelude members. `json.JsonTree` remains
canonical; bare `JsonTree` and retired `JsonValue` spellings remain invalid.

Changing the prelude is a language compatibility change. Additions require an
explicit design decision, collision analysis, diagnostics, and conformance tests.

## Trusted Stdlib Origin

### Authorization rule

Trusted origin answers whether compiler-owned code may delegate to a declaration
or runtime kernel. Only resolved declarations whose immutable origin is
`SourceOrigin::Stdlib` may satisfy that authorization.

The following are never evidence of trust:

- namespace or function spelling,
- `export`,
- local or canonical import aliases,
- a path containing `stdlib`,
- a reserved `FileId` observed outside discovery,
- matching source text or signatures,
- project configuration,
- runtime registration order.

Trust attaches during discovery and follows the declaration identity. If a
collision occurs, compilation fails. A later declaration cannot replace a
trusted registry entry or inherit its authority.

### Policy hooks and private kernels

Compiler policy gates use a compiler-owned registry keyed by the public
operation identity. Each entry identifies the expected trusted wrapper and, when
needed, a private trusted hook or runtime kernel.

For the current JSON bridge, the shared mapping from public operations to private
hooks remains the single source of truth. The checker validates policy, then the
interpreter or backend invokes the resolved trusted exported wrapper. A private
hook is eligible only when its `DeclarationId` has stdlib origin and matches the
registry entry.

The same mechanism may later cover private kernels for collections, encoding,
crypto, time, random, environment, sockets, or other stdlib modules. Generalizing
the registry must not expose a source-level `trusted` annotation.

### Compiler-phase propagation

The following records must preserve or reference the canonical identity rather
than retain only a display name:

- discovery and query inputs,
- namespace and export registries,
- resolved definitions and references,
- typechecker function/type records,
- comptime and runtime interpreter registrations,
- reflection metadata where an owning declaration exists,
- HIR functions and generic instantiations,
- MIR call targets,
- bytecode/native symbols and runtime-kernel bindings,
- cache keys and serialized compiler artifacts from the first layer whose fact
  depends on source identity.

Serialized and cached origin records contain only the logical identities above;
they never persist `DiscoveryRoot.physical_root` or reconstruct identity from a
host-absolute path.

Backends may erase metadata after final call targets are selected, but policy
selection must happen from trusted identities before erasure. Native or bytecode
codegen must not rediscover trust from mangled symbol text.

Compiler cache layers that consume stdlib declarations, namespaces, or policy
include the stdlib identity/version and source origin in their keys. The
syntax-only parse object contains no authority and is rebound beneath current
discovery before those layers run. An artifact built from project source cannot
be replayed as a trusted stdlib artifact because its text or logical path
matches.

## JSON and Stdlib Handoff

The selected module model removes the identity ambiguity that currently keeps
parts of the public JSON API compiler-owned. It does not remove JSON policy by
itself.

The staged JSON rule remains:

1. resolve the public `json.*` wrapper from the stdlib module registry,
2. enforce compiler-owned secret, ownership, target-shape, and handled-result
   policy,
3. require the wrapper and mapped private hook to have trusted stdlib origin,
4. execute or lower the exported wrapper as the body boundary,
5. keep helpers private unless they are intentional public API.

Moving these policy gates into ordinary constraints is a separate language
decision. Module registry implementation is a prerequisite, not permission to
weaken the existing checks.

## Diagnostics and Tooling

Diagnostics use source spelling at the primary location and include canonical
namespace/origin information when ambiguity or trust is relevant. Suggested
messages should distinguish:

- namespace not discovered,
- namespace collision across origins,
- missing local `use`,
- declaration not exported,
- import alias collision,
- dependency cycle,
- untrusted compiler-hook target.

LSP and ASP namespace queries expose canonical namespace names and export
visibility. Private declarations remain visible only where lexical namespace
context permits. Completions may show a local alias while retaining the
canonical declaration target for definition, references, and rename.

Bundle ordering consumes the same resolved module-dependency graph. It must not
re-infer imports by scanning source text or change declaration order inside a
file.

## Implementation Stages

### Stage 1: Shared origin identity

- Move `SourceOrigin`, `FileKey`, and normalized logical-path rules to a shared
  compiler boundary usable by project discovery, driver, query, and later IR.
- Translate current stdlib `FileId` allocation into explicit stdlib origin at
  discovery.
- Keep `FileId` only as a session-local source handle.
- Add tests proving project files cannot request or inherit stdlib origin.

### Stage 2: Registry and ownership

- Discover project, nested vendored dependency, and stdlib roots separately.
- Build `ModuleId` and namespace-owner records before semantic resolution.
- Enforce global namespace uniqueness and the stdlib-only fragment exception.
- Produce deterministic origin, file, and fragment ordering diagnostics.

### Stage 3: Registry-backed `use`

- Resolve every `use` against the module registry rather than the merged lexical
  scope or a builtin-name list.
- Preserve block-local alias behavior and project/dependency import gates.
- Record module-dependency edges and reject cycles initially.
- Feed bundle, LSP, ASP, and completion paths from the same identities.

### Stage 4: Prelude and policy registry

- Materialize the fixed prelude manifest from canonical compiler declarations.
- Remove ad hoc root fallbacks where a manifest entry or canonical namespace is
  required.
- Generalize trusted hook/kernel selection around `DeclarationId` and
  `SourceOrigin::Stdlib` without changing public policy.
- Keep stdlib namespace access implicit and members namespaced.

### Stage 5: Lowering and caches

- Carry declaration and origin identities into HIR, MIR, interpreter bytecode,
  native codegen, runtime bindings, and cache keys.
- Add backend parity tests before removing interpreter-specific trust checks.
- Remove numeric `FileId` authorization once all consumers use explicit origin.

Each stage is independently testable and must preserve current source behavior
unless that stage explicitly replaces a documented bootstrap path.

## Required Conformance Coverage

### Discovery and ordering

- project, dependency, and stdlib files receive distinct origins,
- absolute and escaping logical paths are rejected,
- dependency cycles and duplicate roots fail deterministically,
- project/dependency namespace collisions name both origins,
- reopening a stdlib namespace from project source fails,
- stdlib fragments merge only in canonical path order,
- duplicate declarations across stdlib fragments still fail.

### Imports and visibility

- local `use`, dotted `use`, and explicit aliases resolve canonical modules,
- missing and duplicate aliases fail at their declaration,
- project/dependency executable access without `use` fails,
- canonical qualified signature types work without block-local aliases,
- private members fail externally and work in the owning namespace,
- aliases do not change reflection, definition, reference, or linkage identity,
- import cycles fail rather than weakening declaration order.

### Prelude

- only manifest entries are available unqualified,
- ordinary stdlib members remain namespaced,
- project source cannot add or shadow a prelude entry,
- no root type alias is created,
- `json.JsonTree` remains canonical and retired spellings remain rejected.

### Trusted origin

- a trusted wrapper/private hook pair is accepted,
- a project lookalike with the same namespace, name, and signature is rejected,
- aliases and exports do not grant trust,
- cache roundtrips preserve origin and cannot upgrade project artifacts,
- interpreter, future bytecode, and native lowering select the same policy target,
- tooling never presents private hooks as public completions.

## Deferred Work

This contract does not add:

- a network package registry or lock file,
- package-qualified source names,
- wildcard, member, file-level, or re-export imports,
- source syntax for trusted declarations,
- user-defined namespace fragments,
- cyclic modules,
- root type aliases,
- automatic removal of compiler-owned JSON policy,
- HIR, MIR, bytecode, or native code generation.

Those features require separate pressure and design. The identities and
invariants here are the boundary they must preserve.
