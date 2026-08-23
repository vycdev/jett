# Initial Incremental Query and Invalidation Boundary

Status: selected design for issue
[#147](https://github.com/vycdev/jett/issues/147); the first bounded
implementation slice is tracked by
[#166](https://github.com/vycdev/jett/issues/166).

## Decision Summary

The first incremental-compilation slice will add a `jett_query` crate as the
sole owner of an in-process Salsa database. Its first memoized compiler query
is deliberately conservative:

```text
parse_file(file: FileKey) -> ParsedFile
```

`ParsedFile` owns the current parser's immutable, source-spanned direct AST and
its lexer/parser diagnostics for one source revision. The first implementation
does not wait for a lossless CST, invent stable AST `NodeId` values, or split a
function signature from its body. Existing resolver, typechecker, comptime,
and driver operations initially remain whole-project computations layered over
memoized per-file parsing.

This boundary is useful on its own: unchanged files are not lexed and parsed
again within a database, and it establishes stable source identity, revision
ownership, deterministic diagnostics, and client snapshot rules before finer
semantic queries are introduced. It is also intentionally honest about the
current compiler. Item-level body reuse is a later stage and must not be
claimed until stable declaration identities and cache-observability tests
exist.

Parallel query execution, persistent or content-addressed caches, HIR, MIR,
native lowering, and the deferred CST are outside this first slice.

## Current Constraints

The supported frontend parses tokens directly into an AST. AST nodes carry
`Span` values but no stable `NodeId`, and the driver calls parser, resolver,
typechecker, and interpreter APIs directly. Project discovery currently assigns
`FileId` values from each file's position in a sorted list. Inserting an
earlier path can therefore change the numeric IDs of otherwise unchanged
files.

Those properties rule out copying the future architecture literally:

- a positional `FileId` is not a durable query key;
- a source offset is not stable declaration identity after an edit;
- a whole direct AST cannot provide item-local arena ownership by assertion;
- Salsa accumulators, cycle recovery, and cancellation behavior depend on the
  selected Salsa API and must be implemented and tested rather than presumed;
- the current whole-project semantic passes cannot promise caller-level reuse
  for a body-only edit.

The design below preserves current correctness while creating explicit seams
for finer granularity.

## Database Ownership

A future `jett_query` workspace crate owns the Salsa database, input
setters, query implementations, and test-only recomputation observer. No other
crate declares Salsa inputs or tracked values.

The dependency direction is:

```text
jett_common / jett_project / jett_lexer / jett_parser
                         |
                         v
                     jett_query
                         |
                         v
                     jett_driver
                    /           \
               jett_lsp       jett_cli / ASP
```

As semantic queries are added, `jett_query` may depend on `jett_resolve`,
`jett_typecheck`, and later checked-program/HIR layers. Those compiler phase
crates must not depend back on `jett_query`. `jett_driver` remains the public
orchestration adapter so existing CLI, LSP, and ASP call shapes can migrate
without exposing Salsa types.

The first implementation pins Salsa `=0.28.2`, the reviewed current release at
the time of this decision. The initial slice relies only on that version's
documented input, interned-value, tracked-query, and event-reporting APIs. The
implementation uses `salsa::Event` for test-only query observation and evaluates
`salsa::CancellationToken` / `salsa::Cancelled` in the gated LSP cancellation
stage. Cooperative cancellation and cycle recovery are not enabled merely
because the dependency exists: each is a later gated stage with focused tests
against this pinned API. Upgrading Salsa requires rerunning the recomputation,
revision, cycle, and cancellation matrices; this design does not require
compatibility shims for other Salsa releases.

## Ground-Truth Inputs

The database has explicit inputs for facts obtained outside compiler queries:

- `source_text(FileKey)`: the complete UTF-8 text for each project, dependency,
  or compiler-shipped stdlib file;
- `project_files(ProjectKey)`: the ordered set of source `FileKey` values and
  the entry file;
- `project_config(ProjectKey)`: parsed `jett.proj` fields and compiler options
  that affect discovery or semantic checking;
- `stdlib_files(StdlibKey)`: the ordered bundled stdlib file set and its source
  texts, with compiler-shipped provenance retained;
- `compiler_mode`: only semantic or execution policy that genuinely changes a
  result, such as build versus test evaluation.

Presentation-only settings are not semantic inputs. Human versus `--agent`
rendering, terminal color, and output destination select a renderer after a
query returns and must not invalidate parsing or typechecking. A target or
release setting becomes an input to the first query whose checked behavior or
lowering actually depends on it; it is not added to `parse_file` preemptively.

File discovery and filesystem reads occur before input setters are called.
Derived queries never read ambient files, environment variables, clocks, or
network state. A client updates the file-set input when a file is added,
removed, or changes provenance, and updates only the affected source-text input
for an ordinary edit.

## Stable File Identity

`FileKey` is the cache identity of source text. It consists semantically of:

```text
SourceOrigin + normalized logical path
```

`SourceOrigin` distinguishes the root project, each vendored dependency, and
the compiler-shipped stdlib. The logical path is normalized relative to that
origin, uses `/` separators, removes `.` components, rejects paths escaping the
origin, and follows the project's existing case-sensitive identity policy.
Absolute checkout paths are not identities: moving a checkout must not turn
every file into a different logical source.

The database interns `FileKey` values. Current `FileId` remains a compact handle
used by `Span`, source maps, and diagnostics, but it is derived from the
interned file key for a database lifetime rather than from the file's current
sorted-list position. Adding or removing another file must not change existing
file handles in that database. Numeric handles need not match across processes
or persistent sessions, so externally visible ordering and cache tests use the
logical `FileKey`, never the integer.

A project and a dependency may contain the same relative path because their
origins differ. Compiler-shipped provenance is part of the key and cannot be
claimed by project source; this preserves stdlib namespace and trusted-hook
policy.

## First Query Slice

The selected first query is:

```text
parse_file(file: FileKey) -> Arc<ParsedFile>

ParsedFile:
    file: FileKey
    ast: direct parser-owned AST
    diagnostics: ordered lexer/parser diagnostics
    source_revision: query-owned revision marker
```

The exact Rust fields may differ, but the ownership and dependency boundary may
not. `parse_file` depends only on the file's source text and the minimum lexical
policy actually used by the parser. It does not depend on project configuration,
compiler mode, namespace registries, or unrelated files.

The current AST derives `Clone` but not structural equality. The Salsa 0.28.2
tracked parse function therefore uses its documented `no_eq` option initially;
the project must not add broad AST equality solely to make Salsa backdating
compile. Unchanged inputs still reuse the memo without executing the query.
Later compact declaration/signature summaries implement explicit equality so
backdating can stop unchanged facts from propagating invalidation.

The result owns all AST allocations reachable from it. It contains no borrowed
slice into a mutable document buffer and no index into a global body arena.
Consumers may hold an `Arc` while using one database snapshot, but they must not
publish AST nodes, spans, or arena indices as cross-revision identities.

The initial project-check adapter requests `parse_file` for every member in the
project and stdlib manifests, then runs the current resolver/typechecker path.
That semantic adapter may recompute for any changed parsed file. This is a
correctness-preserving migration step, not the final granularity target.

Current driver paths prepend compiler-shipped support modules to a mutable
parsed module. They must not mutate a cached `ParsedFile`. The adapter instead
builds a separate owned `SemanticModuleSet` (or equivalent aggregate) from the
requested parse results and support modules, and mutates only that disposable
aggregate. A later resolver boundary may consume immutable per-file modules
directly, but shared query results always remain unchanged.

Before a parse result is cached, every lexer/parser-generated span must use the
requested file's diagnostic handle. In particular, the parser's current
synthetic EOF fallback must stop using a fixed `FileId(0)` or be normalized at
the parse-query boundary. A malformed non-entry file ending at EOF must report
that file, not the first file in the database.

## Declaration Identity and Ordered Semantics

The first slice does not manufacture durable declaration identity from spans,
source offsets, vector indices, or AST addresses. All of those change under
ordinary edits. `FileKey` is the only stable frontend identity selected now.

Before item-level semantic memoization lands, a second design/implementation
stage must introduce a `DeclarationKey` with tests for malformed and duplicate
declarations. Its semantic identity will include at least:

- the owning `FileKey` and namespace;
- declaration kind and declared source name;
- enclosing declaration or `mutual:` group identity where applicable;
- a revision-local disambiguator for invalid duplicate declarations that is
  never persisted or exposed as a stable public ID.

Strict top-to-bottom visibility is part of the dependency model, not an
optimization detail. A declaration's semantic query depends on the ordered
signature state visible before it. Inserting, deleting, exporting, or changing
a predecessor may invalidate later declarations even if their own text did not
change. Appending a declaration may leave earlier declarations reusable.

A `mutual:` block is one ordered signature-collection unit. It contains member
declarations/signatures, not their function bodies. All member signatures
become visible together according to language rules; the separate top-level
function definitions associated with those declarations may be checked
individually only after the group signature result is complete. Legal recursion
inside the group is not treated as an accidental query cycle.

## Namespace and Stdlib Registry Invariants

Incremental registries preserve the same language policy as a clean build:

- declarations inside a namespace are private unless explicitly marked
  `export`;
- project and vendored-dependency namespace ownership remains unique, and
  project/dependency source cannot reopen a compiler-shipped stdlib namespace;
- only compiler-shipped stdlib files may contribute multiple fragments to one
  namespace, and duplicate declarations in that merged namespace still fail;
- compiler-shipped provenance is checked before private trusted hooks or
  trusted exported stdlib wrappers are honored;
- removing or changing `export`, namespace ownership, or trusted provenance
  invalidates every lookup or policy query that consumed that fact.

No cached namespace success may survive a file-set change that introduces a
duplicate owner or declaration. These rules are also summarized in
[How the Compiler Locates the Stdlib](../architecture.md#how-the-compiler-locates-the-stdlib)
and remain resolver/compiler policy rather than properties inferred from a
qualified string alone.

## Invalidation Contract

The first slice and the later item-level target are distinguished explicitly:

| Change | First direct-AST slice | Required item-level target |
|---|---|---|
| Function body only | Reparse the changed file; rerun the current project semantic adapter | Recheck that body and its local verify/comptime dependents; callers that depend only on an unchanged signature are reused |
| Declaration signature or type shape | Reparse the file; rerun project semantics | Recompute the declaration signature, its body, callers, type users, and affected reflection metadata |
| Namespace, `export`, or `use` visibility | Reparse the file; rerun project semantics | Recompute the namespace/declaration index and all consumers whose visible scope or qualified lookup can change |
| Insert/delete/reorder before a declaration | Reparse the file; rerun project semantics | Recompute affected later declarations under top-to-bottom visibility; unrelated earlier declarations remain eligible for reuse |
| Edit a declaration inside `mutual:` | Reparse the file; rerun project semantics | Recompute the group signature and dependents of the changed group result |
| Edit a separate function definition declared by `mutual:` | Reparse the file; rerun project semantics | Recheck that function body; reuse group signatures and callers when its declared signature is unchanged |
| Add/remove/rename a project file | Update the file manifest and affected source inputs | Recompute discovery, namespace uniqueness, project ordering, and semantic consumers; unchanged independent parse results remain reusable |
| Change `jett.proj` | Update only parsed configuration and manifest inputs that changed | Parsing is reused; discovery/entry/dependency and semantic queries depending on the changed field recompute |
| Change one stdlib fragment | Update that stdlib source input | Reparse that fragment; recompute merged stdlib namespace/declaration results and only consumers of changed exported or trusted policy facts at item granularity |
| Change renderer or `--agent` | No compiler-query invalidation | No compiler-query invalidation |

The item-level column is an acceptance target for later stages, not a claim
about the first PR that adds Salsa. Salsa backdating may preserve a dependent
when a recomputed signature summary compares equal, but correctness must not
rely on undocumented equality or a benchmark-only observation.

The bundled stdlib manifest preserves the existing depth-then-lexical order:
root foundational files precede nested namespace fragments, and paths at the
same depth sort lexically. Declaration order is currently semantic, especially
for numbered JSON fragments. A manifest-order change is therefore a semantic
input change even when all fragment texts are unchanged.

## Diagnostics and Cycle Handling

Initial queries return diagnostics as ordinary owned values. The first slice
does not use Salsa accumulators. Keeping diagnostics in query results makes the
current phase boundary explicit and avoids choosing accumulator behavior before
cross-file aggregation and cancellation are tested.

Before rendering, diagnostics are deterministically ordered by:

1. source origin and logical path;
2. primary span start and end;
3. severity rank;
4. stable error code;
5. message, label, and suggested-fix text as final tie-breakers.

No ordering depends on hash-map iteration, task scheduling, numeric `FileId`, or
which client requested the result. Existing source maps remain attached so
cross-file labels and agent-mode envelopes preserve their paths. Human and
agent renderers consume the same ordered values.

Query cycles must produce a deterministic compiler diagnostic and an explicit
error/sentinel result that allows safe aggregation where possible. A cycle must
not panic, deadlock, or return a partially initialized semantic value. Legal
language recursion is represented through the existing `mutual:` signature
unit and should not enter generic cycle recovery. The first parse-only query is
acyclic; cycle recovery is required before declaration/type queries are enabled.

## Cancellation, Revisions, and Clients

The first LSP integration keeps the current full-document input and
version-based stale-result suppression. Each `didOpen` or `didChange` updates
`source_text(FileKey)` and requests diagnostics or an interactive result against
one consistent database revision. Before publishing, the server confirms that
the requested document version is still current.

Cooperative cancellation is a later implementation step after the pinned Salsa
API is selected. When added, cancellation is checked at phase/query boundaries,
returns no partial diagnostics as a successful result, and never commits a
partially built externally observable cache entry. Starting a newer revision
may cancel older work, but stale-result suppression remains the final publish
guard.

LSP keeps one database per workspace session so revisions can reuse unchanged
results. ASP and ordinary CLI commands are one-shot processes: they create one
database per invocation and may reuse results among commands performed in that
invocation, but there is no cross-process cache in this design. Persistent and
content-addressed caches remain separate Phase L work.

Current ASP/LSP operation names and result envelopes stay stable. The driver
adapts `file_symbols`, `type_at`, `signature`, completion, namespace,
definition, reference, and diagnostic requests to a database snapshot. Query
migration must not discard known source context or make agent and human modes
compute different compiler facts.

## Future CST, HIR, and MIR Compatibility

The deferred lossless CST is not required by `FileKey`, input, revision, or
client snapshot policy. When introduced, it adds a syntax query and keeps the
semantic AST boundary:

```text
initial: source_text -> parse_file -> direct AST
later:   source_text -> parse_cst -> lower_ast -> semantic AST
```

Semantic consumers depend on `lower_ast`, not on rowan node types. CST node
identity may improve edit correlation, but CST IDs do not silently become
semantic declaration IDs without the declaration-key tests above.

Future checked-program/HIR and MIR queries consume stable file/declaration
keys, ordered diagnostics, and immutable query-owned results. They do not retain
AST/body arena indices across revisions. HIR/MIR may introduce their own local
arenas whose indices remain valid only within one owning query result.

## Bounded Implementation Sequence

The database-and-parse-reuse slice in stage 1 is tracked by
[#166](https://github.com/vycdev/jett/issues/166). Later stages require their
own independently bounded implementation tracking after this first slice is
measured.

1. **Database and parse reuse**
   - add `jett_query`, pin Salsa `=0.28.2`, and define `ProjectKey`, `StdlibKey`,
     and interned `FileKey` inputs;
   - adapt the direct parser to return an owned `ParsedFile`;
   - fix or normalize synthetic EOF and other generated spans to the requested
     file handle;
   - build a separate owned semantic aggregate instead of prepending support
     modules into cached parse results;
   - route one driver build/query path through memoized `parse_file` without
     changing language behavior;
   - retain diagnostic vectors and current whole-project semantic passes.
2. **Client snapshots**
   - keep one database in the LSP workspace, update full-text inputs, and retain
     version-based stale-result suppression;
   - create one fresh database for each CLI/ASP invocation;
   - migrate interactive driver operations without changing output envelopes.
3. **Declaration summaries**
   - implement and test `DeclarationKey`, ordered file summaries, namespace
     registries, duplicate recovery, and `mutual:` signature groups;
   - separate signature facts from body data only after identities survive the
     required edit matrix.
4. **Item-level semantic queries**
   - migrate resolution and body checking behind immutable summaries;
   - prove body-only caller reuse and correct signature/export invalidation;
   - add deterministic cycle recovery before enabling recursive semantic
     queries.
5. **Later independent stages**
   - add the CST/lowering query when source-tooling pressure justifies it;
   - add checked-program/HIR and MIR queries through #20 and #22;
   - evaluate parallel execution and persistent/content-addressed caching only
     after in-process recomputation is measured.

Each stage must preserve a working driver and can land independently. A stage
that only wraps the whole project must describe itself that way; it cannot
claim item-level incremental typechecking.

## Cache Observability and Required Tests

Incremental tests use a test-only query event recorder or per-query execution
counter exposed by `jett_query`. They assert recomputation sets, not elapsed
wall-clock time. The observer is unavailable to Jett source and normal CLI
output.

The first stage requires tests that:

- two unchanged requests in one database execute `parse_file` once;
- editing file A reparses A but not unchanged file B;
- adding or removing a file keeps existing `FileKey` identities and cached
  parses while recomputing the manifest-dependent adapter;
- changing project configuration does not reparse unchanged source;
- changing one stdlib fragment reparses that fragment but not unrelated project
  files;
- stdlib manifest ordering remains depth-first and then lexical, including
  root versus numbered nested fragments;
- an EOF parse error in a non-entry file retains that file's key and path;
- semantic checking and support-module aggregation do not mutate a cached
  `ParsedFile`;
- diagnostics have identical order across repeated requests and fresh database
  instances;
- an older LSP document version never publishes after a newer version;
- cancellation, once implemented, cannot publish or cache partial successful
  results.

The declaration and item stages additionally require fixtures for:

- body-only edits versus signature edits;
- namespace and `export` changes across files;
- insertion, deletion, and reordering under strict top-to-bottom visibility;
- appending an unrelated declaration without invalidating earlier items;
- `mutual:` declaration/signature changes and separate associated function-body
  changes;
- duplicate/malformed declarations and deterministic identity recovery;
- private/exported namespace changes, duplicate project/dependency namespace
  owners, stdlib fragment merging, duplicate merged declarations, and trusted
  hook/wrapper provenance;
- project configuration, file-set, and stdlib-fragment changes;
- legal recursion versus a diagnosed illegal query/type cycle;
- identical ASP/LSP compiler facts from the same revision.

A cache-hit claim is valid only when these counters show the expected query was
reused and the ordinary correctness suite still passes.

## Deferred Scope

This design does not select or implement:

- parallel query execution or namespace scheduling;
- serialized Salsa state, persistent caches, or content-addressed artifacts;
- distributed or remote builds;
- a lossless CST library or incremental reparsing algorithm;
- final declaration identity for malformed syntax before its tests exist;
- HIR, MIR, LLVM, interpreter bytecode, or native-runtime lowering;
- broad performance targets beyond observable in-process reuse;
- hot code reloading.

Those concerns must build on measured, deterministic in-process behavior rather
than expanding the first database slice.
