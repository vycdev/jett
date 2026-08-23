# Deterministic Parallel Compilation Boundary

Status: selected design for issue
[#151](https://github.com/vycdev/jett/issues/151); implementation not started.

## Decision Summary

Jett will parallelize compiler queries only after a deterministic build plan has
identified independent work. Worker timing, worker count, and hash-table
iteration are never observable compiler inputs.

The first parallel slice is intentionally narrow:

```text
ordered project manifest
    -> parallel parse_file(FileKey) requests
    -> canonical ParsedFile ordering
    -> existing sequential resolver and typechecker
```

Each whole-file parse is independent because it consumes only one immutable
`SourceFile` input. The coordinator submits files by canonical `FileKey`, but
correctness does not depend on start or finish order. It collects all results
and publishes them in manifest order after every requested parse finishes.

Parallel namespace and declaration checking is a later gated slice. It requires
the `DeclarationKey`, ordered signature summaries, and namespace dependency
facts selected by the
[initial incremental query boundary](incremental_query_boundary.md). The
compiler must not describe whole-project resolution or typechecking as parallel
until those query boundaries and their recomputation tests exist.

Persistent caches, distributed builds, HIR/MIR lowering, and Jett source-level
`run` tasks are separate concerns.

## Why Planning and Execution Are Separate

Parallelism changes throughput, not compiler meaning. The coordinator first
constructs an immutable `BuildPlan` from one database revision. A plan contains:

```text
WorkKey:
    stage
    stable logical source or declaration identity
    canonical ordinal within that stage

WorkNode:
    key
    dependency WorkKeys
    immutable query inputs
```

`WorkKey` never contains a numeric `FileId`, pointer, arena index, worker ID,
thread ID, random value, or completion sequence. A plan is equal across clean
runs when semantic inputs are equal. The scheduler may execute any ready node,
but the coordinator publishes only complete stage results in canonical order.

No compiler query discovers new ambient filesystem, environment, clock, or
network inputs while running. Discovery and input updates finish before the
plan is built. A query may request another declared query dependency, but it
may not enqueue untracked side work whose completion order affects output.

## Work Graph and First Parallel Unit

### Stage 1: whole-file parsing

The first work node is:

```text
Parse(FileKey) -> Arc<ParsedFile>
```

All parse nodes for one manifest revision are independent. Project,
dependency, and compiler-shipped stdlib origins remain part of `FileKey`.
Stdlib manifest order remains depth-then-lexical, including numbered JSON
fragments. Parsing may finish out of order, but the returned vector, source map,
and diagnostic aggregation use manifest order and the ordinary diagnostic sort.

This stage does not mutate cached ASTs, prepend support modules into them, or
share a parser arena. Every `ParsedFile` owns its AST and diagnostics. A worker
receives only immutable text and its assigned diagnostic handle.

### Stage 2: declaration and namespace facts

This stage cannot start until stable `DeclarationKey` and ordered declaration
summary queries land. The coordinator then builds namespace facts in two
parts:

1. collect file and namespace headers in canonical source order;
2. validate ownership, compiler-shipped provenance, duplicates, exports, and
   ordered declaration visibility before body work is scheduled.

Project and dependency namespaces have one owner. Compiler-shipped stdlib
fragments may merge, but their declarations form one depth-then-lexical ordered
stream. Fragments of the same stdlib namespace are not independent semantic
units merely because they are separate files.

The namespace dependency graph records exact consumed public signatures and
policy facts. An edge `consumer -> provider` means the provider summary must be
complete before the consumer body may be checked. Independent ready namespace
bodies may run concurrently. A body does not become independent merely because
its source file differs.

Invalid duplicate owners, illegal forward references, and dependency cycles are
diagnosed while building canonical facts. The scheduler never breaks a cycle
by choosing whichever worker happens to run first.

### Stage 3: declaration bodies

After namespace and ordered signature facts are available, individual function,
method, verify, property, actor-handler, and other executable bodies may become
work nodes. A body depends on:

- the ordered signature state visible before its declaration;
- its namespace imports and exact exported provider summaries;
- its owning type, interface, actor, or machine metadata;
- its enclosing `mutual:` signature group when applicable;
- semantic compiler options used by that query.

Bodies with equal ready dependency sets may execute concurrently. This remains
a future stage until tests prove body-only reuse and exact invalidation.

## Ordered Language Semantics

Parallel scheduling does not weaken Jett's strict top-to-bottom rules.

- A declaration sees only preceding declarations in its ordered semantic
  stream, plus signatures made visible by its enclosing `mutual:` group.
- Inserting, deleting, exporting, or changing an earlier declaration
  invalidates later consumers even if their workers could otherwise run.
- A `mutual:` block is one signature-collection node. All member signatures are
  validated and published together; associated bodies may run independently
  only after that node succeeds.
- Legal recursion represented by `mutual:` is not a scheduler cycle.
- Namespace ownership and `export` facts are settled before dependent body
  checks. A worker may not speculate that a private or duplicate declaration
  will later become usable.
- Compiler-shipped stdlib fragments preserve their lexical semantic order and
  trusted provenance. Parallel parsing does not authorize parallel declaration
  publication within a merged namespace.

The same source must therefore accept or reject identically with one worker or
many.

## Ownership and Thread Safety

Query outputs crossing worker boundaries are immutable owned values, normally
shared through `Arc`. A result owns every AST, summary, diagnostic, and local
arena reachable through it. No worker keeps a borrow into a mutable document,
coordinator vector, or another worker's scratch storage.

Process-local identities have these boundaries:

- `FileKey` and future `DeclarationKey` are stable query identities;
- `FileId` remains a database-lifetime diagnostic handle and is assigned before
  workers start;
- AST offsets, vector positions, and arena indices stay within one owning query
  result and revision;
- worker IDs and operating-system thread IDs are never compiler identities.

A shared mutable `SymbolInterner`, type interner, namespace map, or diagnostics
vector must not sit behind a lock merely to make existing sequential code run
concurrently. Lock acquisition order would become hidden semantics and make
identity assignment timing-dependent.

Before semantic parallelism, globally shared symbols and canonical type shapes
are collected and assigned in one deterministic coordinator phase, sorted by
their canonical structural keys. Workers receive immutable snapshots. New
body-local values use query-owned local arenas and are merged by stable keys,
not by local numeric indices. If a later stage needs a new global shape, it
returns the structural request to a deterministic barrier rather than mutating
the global interner from the worker.

All types and query results crossing a worker boundary must be honestly
`Send + Sync`; unsafe adapters require a written lifetime argument and focused
concurrency tests. Adding `unsafe impl Send` or `unsafe impl Sync` to bypass an
ownership problem is not an implementation stage.

## Deterministic Diagnostics and Publication

Workers return diagnostics as ordinary owned query results. They do not print,
write files, send LSP notifications, populate persistent caches, or append to a
shared diagnostics vector.

The coordinator waits for the complete requested stage, then sorts diagnostics
by the existing compiler key:

1. source origin and normalized logical path;
2. primary span start and end;
3. severity rank;
4. stable diagnostic code;
5. message, labels, and suggested-fix text as final tie-breakers.

Human and `--agent` renderers consume the same ordered values. Diagnostic order
must be identical across worker counts and repeated runs. Any deterministic
deduplication uses the full diagnostic key after sorting; “first worker wins” is
forbidden.

A stage publishes atomically. Success values, diagnostics, LSP responses,
command summaries, and future persistent artifacts become externally visible
only when every required dependency completed for the same revision. Internal
Salsa memo entries may be reused according to Salsa's tested rules, but an
incomplete aggregate is never represented as a successful compiler result.

## Failures, Cycles, Cancellation, and Revisions

Every work node has one of four coordinator outcomes:

```text
Complete(value, diagnostics)
Failed(diagnostics)
Cancelled
Stale(revision)
```

A worker panic, poisoned synchronization primitive, or task-join failure becomes
one deterministic internal compiler failure associated with its `WorkKey`. The
coordinator cancels dependent work and publishes no partial success. Panic text,
thread names, and backtraces are not stable agent output.

Query or namespace cycles are found from the canonical dependency graph and
reported using sorted `WorkKey` paths. Worker waiting is not cycle detection.
No worker blocks synchronously on another worker while holding a compiler lock.

Cancellation is cooperative at query and phase boundaries:

- each build plan belongs to one immutable revision token;
- starting a newer LSP revision may cancel older work;
- a cancelled or stale plan publishes no diagnostics, completion result, or
  cache artifact;
- current LSP document-version checking remains the final publication guard;
- CLI interruption cancels outstanding work, waits for worker teardown, emits
  no successful build/run/test payload, and exits with status 130;
- cancellation is not a compiler diagnostic and does not become a cached
  failure.

A newer revision may reuse completed immutable dependencies when Salsa proves
they remain valid. It cannot reuse an aggregate that was only partly assembled
for an old revision.

## Worker and Resource Policy

Parallelism is an orchestration setting, not a semantic compiler input.
Changing it must not invalidate parsing or semantic queries.

The initial controls are:

- CLI compiler commands accept `--jobs N`, where `N >= 1`;
- absent an explicit value, CLI uses available hardware parallelism capped at
  eight workers;
- LSP exposes a workspace `jett.jobs` setting and otherwise uses available
  hardware parallelism capped at four workers so background analysis is
  bounded;
- ordinary unit and fixture tests default to one worker inside the compiler,
  while dedicated parallel tests explicitly exercise 1, 2, 4, and 8 workers;
- no ambient `JETT_JOBS` variable is read, because hidden environment policy
  would make invocation behavior harder to reproduce;
- each plan uses one bounded worker pool rather than creating a thread per file
  or nested pools per query.

`--jobs 1` uses the same build plan, query implementations, merge, and
publication paths as larger values. It is the debugging baseline, not a
separate sequential compiler implementation.

The pool limits simultaneously ready CPU work. Later memory-heavy phases may
add deterministic per-stage admission weights, but may not inspect transient
host memory pressure to alter compiler meaning. Resource exhaustion becomes a
normal deterministic command failure; it does not silently retry with a
semantically different phase or publish partial output.

## CLI, LSP, ASP, and Test Behavior

CLI, LSP, and ASP request the same compiler facts from one consistent database
snapshot. Only lifecycle and presentation differ.

- One-shot CLI and ASP commands create one database and worker pool for the
  invocation, render once after publication, and tear both down before exit.
- LSP owns one database and bounded pool per workspace. Interactive requests use
  revision snapshots; stale responses are discarded even if their workers
  finished successfully.
- Human versus `--agent` rendering occurs after deterministic aggregation and
  never changes scheduling dependencies.
- Test harnesses inject a worker count and cancellation points directly. They do
  not use sleeps or elapsed-time assertions to prove concurrency.

Program execution, verify/property evaluation, and Jett actor scheduling remain
sequential unless a separate contract explicitly parallelizes them. Compiler
workers do not grant runtime capabilities and never execute impure comptime
work.

## Relationship to Caching and Future Backends

In-process Salsa memoization and parallel execution share immutable query
results, but neither implies persistent caching. A future content-addressed
cache may consume only a fully published canonical result and must follow
[#153](https://github.com/vycdev/jett/issues/153) for identity, serialization,
trust, atomic writes, and concurrent process behavior.

Cancelled, stale, failed, or partially merged stages cannot publish a persistent
artifact. Two compiler processes racing to cache the same successful result are
outside this in-process worker contract.

Future CST, HIR, MIR, LLVM, and interpreter-bytecode phases may add work-node
kinds. They inherit stable keys, immutable ownership, deterministic barriers,
cancellation, and atomic publication. Backend-specific parallelism may not
reorder user-visible diagnostics or make target output depend on worker count.

## Bounded Implementation Sequence

1. **Parallel parse requests**
   - land and stabilize the selected `jett_query` parse database;
   - add a bounded coordinator pool and request independent `parse_file` nodes;
   - collect results in canonical manifest order;
   - keep resolver, typechecker, and diagnostic rendering sequential;
   - add worker-count, repeated-run, panic, and cancellation tests.
2. **Deterministic declaration facts**
   - implement `DeclarationKey`, ordered summaries, namespace ownership,
     duplicate recovery, export facts, and `mutual:` signature groups;
   - preassign shared symbol/type identities in canonical coordinator barriers;
   - keep body checking sequential until invalidation tests pass.
3. **Namespace and body queries**
   - build the exact dependency graph from consumed signature and policy facts;
   - parallelize only ready independent nodes;
   - prove identical diagnostics and results for worker counts 1, 2, 4, and 8;
   - add cooperative cancellation and stale-result suppression at every publish
     boundary.
4. **Client integration**
   - add `--jobs` and `jett.jobs` without making them semantic query inputs;
   - reuse one pool per CLI invocation or LSP workspace;
   - preserve existing human, agent, and LSP envelopes.
5. **Later phases**
   - add HIR/MIR/backend work nodes only after their ownership contracts land;
   - coordinate persistent publication separately with #153;
   - measure speedup only after correctness and recomputation gates pass.

Each stage keeps a working compiler and may land independently. Parallel parse
throughput is not evidence of parallel typechecking.

## Required Verification Matrix

Tests use barriers, injected cancellation tokens, deterministic fake work, and
query execution counters. They never require a task to finish first because of
a sleep.

The first stage requires:

- identical parsed modules and diagnostic sequences with 1, 2, 4, and 8 workers;
- repeated clean runs with deliberately varied completion order;
- project, dependency, and stdlib files returned in canonical manifest order;
- numbered stdlib fragments preserving semantic order;
- editing one file re-executing only its parse while unrelated parses are reused;
- malformed files producing stable file paths, spans, and diagnostic order;
- one worker panic cancelling dependents and producing one stable internal
  failure with no successful aggregate;
- cancellation before dispatch, during parsing, and before publication;
- an older LSP revision never publishing after a newer revision;
- `--jobs 1` and the default pool using the same coordinator path.

Semantic parallel stages additionally require:

- independent namespaces and bodies demonstrably overlapping behind a test
  barrier without output-order assertions;
- dependency edges preventing a consumer from checking before its provider
  summary exists;
- insertion, deletion, and reordering under top-to-bottom visibility;
- `mutual:` signatures publishing as one unit while eligible bodies run
  independently;
- merged stdlib fragments, duplicate declarations, namespace ownership,
  `export`, and trusted provenance retaining sequential semantics;
- shared symbol and type identities matching across worker counts;
- stable diagnostics for multiple workers failing simultaneously;
- cancellation and stale revisions publishing neither partial facts nor
  diagnostics;
- race-oriented tests under Miri or an equivalent applicable checker for any
  custom unsafe ownership adapter.

Performance tests may report speedup, but elapsed time never proves correctness.

## Deferred Scope

This design does not select or implement:

- persistent, content-addressed, remote, or distributed caching;
- cross-process worker coordination;
- HIR, MIR, native code generation, or backend-specific work decomposition;
- lossless CST parsing or incremental text reparsing;
- Jett source-level task, actor, property-test, or program-runtime scheduling;
- speculative checking against unvalidated namespace or signature facts;
- work stealing behavior as a public contract;
- dynamic worker changes based on ambient machine load;
- partial diagnostic streaming from an unfinished revision.

Those features must preserve the deterministic plan and atomic publication
boundary rather than making worker timing part of the language.
