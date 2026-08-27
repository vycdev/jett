# CPU and Memory Profiling Contract

## Status

Decided for the first interpreter-backed profiler. This document closes the
design scope tracked by [#164](https://github.com/vycdev/jett/issues/164).
Implementation remains staged, and future native runtimes must preserve the
same event, aggregation, security, and reporting semantics.

## Goals and Boundaries

Jett profiling turns bounded runtime observations into compact bottleneck
summaries. The first contract provides:

- one explicit CPU mode and one explicit memory mode on `jett run`;
- backend-neutral samples and allocation events;
- stable attribution to Jett functions, source spans, call chains, and runtime
  execution contexts;
- deterministic human and TOON summaries;
- partial-profile and capability reporting instead of silent approximation;
- no program values, heap contents, or unsanitized source in reports.

This contract does not define flamegraph export, always-on telemetry, production
allocator selection, native unwinding machinery, operating-system process
profiling, optimizer behavior, or profile-guided optimization. It profiles the
Jett runtime selected by `jett run`, not the compiler phases that precede it and
not arbitrary child processes or foreign libraries.

## Command-Line Surface

The canonical modes are:

```text
jett run app.jett --profile
jett run app.jett --profile-memory
```

The shared controls are:

```text
--profile-threshold <percent>  minimum impact, default 5.00
--profile-limit <count>        maximum bottlenecks, default 10
```

CPU profiling additionally accepts:

```text
--profile-rate <hertz>         requested sample rate, default 1000
```

`percent` is parsed as a decimal percentage from `0` through `100`, with at
most two fractional digits, and is stored as integer basis points. `count` is
an integer from `1` through `100`. `hertz` is an integer from `1` through
`1000`. Invalid or out-of-range values are CLI errors before compilation or
execution.

The two modes are mutually exclusive in v1. Running both at once would combine
two sources of overhead and make each result incomparable with a single-mode
run. A profile-only option without its corresponding mode is also a CLI error.
The same options work with `--agent`; option order has no semantic effect.

No source-level capability, declaration, annotation, or import enables the
profiler. It is launcher-owned observation selected only at the command line.

## Lifecycle and Exit Behavior

The driver validates, compiles, and prepares the requested profiler before it
enters `main`:

1. A compile failure emits ordinary compiler diagnostics and no profile.
2. An unsupported mode or setup failure emits a stable profiler failure and
   does not execute user code.
3. A normal return finalizes a complete profile.
4. A handled or unhandled Jett runtime failure finalizes the observations that
   precede the failure and marks `termination: runtime_error`.
5. An interrupt that reaches the runtime's cooperative shutdown path finalizes
   a partial profile with `termination: interrupted`. The process retains the
   platform's conventional interrupted exit status, such as `130` for SIGINT.
6. An uncatchable kill, process abort, host crash, or power loss is not promised
   a final report.

Profiling never changes a Jett program's source-visible return value or runtime
error. If collection fails after user code starts, the runner disables the
failed collector, lets the program reach its ordinary termination when safe,
and emits `status: unavailable` or `status: partial` with a stable reason. A
profiler failure produces the profiler CLI failure status only when the program
would otherwise succeed; an existing compile, runtime, or interrupt failure
keeps its own status.

## Shared Attribution Model

All backends adapt observations to the following logical identities. These are
schema concepts, not required Rust layouts:

```text
SourceIdentity = {
    origin: project | dependency | stdlib,
    logical_path: normalized relative path,
    source_id: stable identity within the run manifest,
}

FrameIdentity = {
    namespace: qualified namespace or "<root>",
    function: source-level function or stable runtime bucket,
    source: SourceIdentity | unavailable,
    declaration_span: byte range | unavailable,
    call_site_span: byte range | unavailable,
}

ExecutionIdentity = {
    task_id: runtime-assigned id | unavailable,
    actor_id: runtime-assigned id | unavailable,
    runtime_thread_id: run-local id | unavailable,
}
```

Numeric parser `FileId`, pointer identity, native addresses, mangled symbols,
absolute paths, host thread IDs, and allocator addresses never appear in the
report or participate in deterministic tie-breaking. Runtime IDs are run-local
metadata only. They are not compared across runs.

A stack is ordered from the root entry frame to the observed leaf frame. The
maximum recorded depth is 128. A deeper stack ends with a stable
`<truncated-stack>` frame and increments `truncated_stacks`. Missing native
symbols use `<unavailable-symbol>`; runtime work with no Jett frame uses a named
bucket such as `<runtime>` rather than being assigned to the nearest user
function.

Project paths are relative to the canonical project root. Dependency paths use
the dependency's logical name and vendored-relative path. Standard-library
paths use `stdlib/`. If no authorized manifest-relative path exists, the source
location is unavailable. Backends must not fall back to an absolute path.

## CPU Sampling Semantics

### Collection

A CPU sample event contains:

```text
CpuSample = {
    sequence: monotonic run-local integer,
    requested_tick: monotonic run-local tick,
    execution: ExecutionIdentity,
    state: jett | runtime | waiting | unavailable,
    stack: list[FrameIdentity],
}
```

The configured rate is a requested opportunity rate, not a promise that every
tick becomes a stack. Sampling uses a monotonic clock. The timer keeps at most
one pending request per runtime execution thread; another tick while one is
pending is coalesced and counted. Collection must not queue an unbounded number
of stack requests or stop a thread at an unsafe point.

The tree-walking interpreter acknowledges pending requests at bounded statement,
call, return, and loop safe points on its dedicated `jett-runtime` thread. This
makes stack capture memory-safe while still using elapsed monotonic ticks. It
must not substitute statement counts for elapsed-time sampling. A future native
runtime may use platform stack sampling or scheduler safe points, but it emits
the same logical events and reports which mechanism it used.

The timer path is constant-space and allocation-free: it updates a tick counter
and one pending bit per registered runtime thread. Stack capture visits at most
128 frames, and aggregation is online under a 32 MiB CPU-profiler metadata
budget. If new function, line, or call-chain buckets would exceed that budget,
the collector keeps total/state counters, stops adding new detail buckets, and
emits `status: partial` with `partial_reason: cpu_metadata_limit`. It does not
block indefinitely, grow an event queue, or terminate the program. The report
does not estimate a percentage overhead because that would require a second,
unprofiled execution with potentially different program behavior; it reports
the requested/observed rates and all coalesced or dropped opportunities instead.

A sample in `state: jett` has at least one Jett frame. `runtime` covers runtime
work outside a Jett frame, `waiting` covers a requested tick while the execution
context is blocked or idle when the backend can identify that state, and
`unavailable` covers a tick whose state or stack could not be recovered. These
states remain separate; blocked wall time is never charged to the last Jett
function.

For a future concurrent runtime, every Jett runtime worker participates. Samples
carry the active task, actor, and run-local runtime thread when known. Migration
between workers does not split a function identity. External threads not
registered with the Jett runtime and work performed in child processes are out
of scope. A capability call that blocks may produce `waiting` observations but
does not make the called Jett frame consume CPU samples.

### Aggregation

CPU totals report at least:

- requested ticks;
- recorded samples by state;
- coalesced and collector-dropped ticks;
- attributed and unavailable stack counts;
- requested and observed effective rates;
- monotonic wall duration;
- process CPU duration when the platform supplies it, otherwise `unavailable`.

Percentages for Jett bottlenecks use the count of `state: jett` samples as their
denominator. They are sampling estimates, not CPU utilization and not a claim
that inclusive percentages across callers sum to 100. Runtime, waiting, and
unavailable samples remain visible in totals but do not inflate a user's
function percentage.

For each function:

- `inclusive_samples` counts samples containing that function;
- `self_samples` counts samples where that function is the leaf Jett frame;
- hot-line counts use the leaf frame's call-site or current-statement span;
- call-chain counts use the complete normalized root-to-function chain.

Each function's displayed call chains are sorted by descending sample count,
then lexically by the sequence of stable frame identities. At most three are
reported. Hot lines are sorted by descending self-sample count, then source
identity, line, and column; at most three are reported.

A backend that cannot obtain a monotonic clock or safely observe the active Jett
stack reports CPU profiling as unsupported before `main`. It must not silently
replace elapsed-time sampling with statement counting, instrumentation call
counts, a different rate, or whole-process CPU percentages.

## Memory Profiling Semantics

### Coverage and Events

Memory mode covers the Jett-managed runtime heap. It excludes compiler memory,
profiler metadata, stacks, executable code, native library internals, foreign
allocators, memory-mapped files, child processes, and operating-system cache.
The report says `coverage: jett_heap`; it never labels the result as resident set
size or total process memory.

Backends normalize allocator hooks into:

```text
AllocationEvent = {
    sequence: monotonic run-local integer,
    operation: allocate | resize | free,
    allocation_id: opaque run-local identity,
    old_size: integer bytes | not_applicable,
    new_size: integer bytes | not_applicable,
    execution: ExecutionIdentity,
    allocation_stack: list[FrameIdentity] | unavailable,
}
```

Only successful operations emit events. A new allocation creates one ID. A
resize preserves that ID. A free retires it. An ID cannot be reused during one
run. Addresses never leave the collector.

The normalized counters are:

- `allocation_count`: successful creation of new allocation IDs;
- `resize_count`: successful resizes;
- `allocated_bytes`: initial allocation sizes plus positive resize deltas;
- `freed_bytes`: freed live sizes plus negative resize deltas;
- `live_bytes`: allocated bytes minus freed bytes for tracked allocations;
- `peak_live_bytes`: the greatest live byte total after any event;
- `retained_bytes`: live bytes at finalization.

A zero-size allocation may have an ID and increments `allocation_count`, but adds
zero bytes. A failed allocation emits no event and follows the runtime's ordinary
allocation-failure behavior. A resize from or to zero follows the same delta
rules, so host allocator conventions do not change Jett metrics.

Allocation count and initial bytes are attributed to the creation event's leaf
Jett frame and normalized call chain. Positive resize growth is allocation
pressure at the resize event's active leaf frame. The live allocation still
retains its original creation site through resizes and task migration, so freed
and retained bytes are reported against that creation site, not the resizing or
releasing frame. At the instant of the global peak, the collector snapshots
live bytes grouped by original creation site as `live_at_peak_bytes`.

The default memory bottleneck impact is allocated bytes. Its percentage uses
total attributed allocated bytes as the denominator. Each entry also reports
allocation count, resize count, freed bytes, retained bytes, and bytes live at
the global peak when available. This keeps allocation churn, end-of-run
retention, and peak pressure distinct. Retained memory is not automatically
called a leak; reachable caches and intentionally long-lived data are valid.

### Bounded Overhead

Memory aggregation is online and profiler-owned allocations are excluded to
avoid recursion. Exact retention requires one metadata record per live Jett
allocation. V1 gives those records a fixed 64 MiB metadata budget per run. The
backend may use a more compact representation, but it may not exceed that budget
without an explicit future option.

If the budget is exhausted, the collector continues exact aggregate allocation
count and byte-pressure counters where possible, stops claiming exact live,
peak, freed, and retained values, and marks those fields unavailable with
`partial_reason: retention_metadata_limit`. It must not terminate the program,
attribute unknown frees to a guessed site, or silently present a lower bound as
exact.

A backend without hooks for Jett-managed allocations reports memory profiling as
unsupported before `main`. Sampling host RSS or replacing missing free events
with end-of-process guesses is not an allowed fallback.

## Thresholds, Ordering, and Truncation

A bottleneck is eligible when its primary impact is greater than or equal to
`--profile-threshold`. CPU uses inclusive Jett sample percentage. Memory uses
attributed allocated-byte percentage. Percentages are calculated from integer
counts, rounded half away from zero to two decimal places only for display, and
compared in unrounded rational form.

Eligible entries are ordered by:

1. descending primary impact count;
2. descending self samples for CPU or allocation count for memory;
3. qualified namespace and function;
4. source origin, logical path, declaration start, and declaration end.

The first `--profile-limit` entries are emitted and assigned ranks starting at
one. The summary reports `eligible_bottlenecks`, `emitted_bottlenecks`, and
`truncated_bottlenecks`. An empty or entirely below-threshold profile is valid
and emits an empty bottleneck list with totals intact.

Unavailable-symbol and runtime buckets are included in totals. An unavailable
Jett symbol may be an emitted bottleneck if it crosses the threshold, with
source and suggestions marked unavailable. The output never fabricates a source
location or merges two unknown native addresses into a source function merely
because their display names match.

## Deterministic Suggestions

Suggestions are optional compiler-owned records, not generated prose. Every
suggestion has a stable rule ID and a fixed template filled only with reported
counts, percentages, qualified names, and source locations. V1 rules are:

- `CPU_HIGH_SELF`: inspect the reported hot lines when self samples are at least
  half of a function's inclusive samples;
- `CPU_CALLEE_DOMINATED`: inspect the dominant reported call chain when self
  samples are less than half of inclusive samples;
- `MEM_ALLOCATION_PRESSURE`: reduce, reuse, or batch work at the hottest
  allocation site;
- `MEM_RETAINED`: review the lifetime of allocations from a site when its
  retained bytes cross the same configured threshold.

Ties use the bottleneck ordering above. At most two suggestions are emitted per
bottleneck, ordered by rule ID. A rule is omitted when required data is partial,
unavailable, or below its exact predicate.

The first implementation does not claim that an allocation occurs in a loop,
that a value can move to comptime, that a standard-library replacement is
semantically equivalent, or that retained memory is a leak. Such advice may be
added only when checked compiler facts prove the predicate and a versioned rule
with deterministic tests owns the wording. Runtime data is never sent to an
external model to generate suggestions.

## Source and Secret Safety

Profiles record control-flow location and aggregate sizes only. They never
record arguments, locals, return values, capability contents, allocation bytes,
dynamic strings, environment values, or secret values.

A hot-line `code` excerpt is optional. When source and checked metadata are
available, the compiler derives it from the loaded run manifest, not from a new
filesystem read, and applies all of these rules:

- comments are omitted;
- string and byte literal contents become a fixed `<redacted-literal>` token;
- a span whose checked type contains `secret` becomes `<secret-expression>`;
- control characters are escaped and output is limited to 160 UTF-8 bytes at a
  character boundary;
- source outside the authorized project, dependency, or stdlib manifest is
  unavailable.

The row includes `source_redacted: true` whenever any rule changed the excerpt.
If safe tokenization or checked metadata is unavailable, `code` is unavailable;
the profiler does not fall back to the raw line. Function and namespace names,
relative paths, line numbers, counts, and sizes remain visible because they are
the profiling subject, but suggestions never interpolate source text.

## Output Channels and Schemas

In human mode, user stdout remains stdout and user stderr remains stderr. The
profile summary is written to stderr after runtime finalization, with a clear
`CPU profile` or `Memory profile` heading. Human wording may improve, but it
must carry the same status, coverage, totals, partial reasons, and bottleneck
ordering as structured output.

With `--agent`, the CLI continues to emit one structured run envelope on stdout.
Program stdout and stderr are captured as escaped fields or ordered stream rows;
they are never concatenated with the profile object as raw text. The profile is
an optional typed object in that same envelope. Compiler and profiler setup
failures use the ordinary structured failure envelope. This preserves stdout as
one parseable TOON document while preventing program output from impersonating
profile fields.

The stable structured object begins:

```toon
profile:
  schema: jett.profile.v1
  mode: cpu
  status: complete
  termination: returned
  backend: interpreter_safe_point
  coverage: jett_runtime_threads
  config:
    requested_rate_hz: 1000
    threshold_basis_points: 500
    limit: 10
    cpu_metadata_budget_bytes: 33554432
  totals:
    requested_ticks: 48000
    recorded_samples: 47200
    attributed_samples: 46800
    runtime_samples: 200
    waiting_samples: 100
    unavailable_samples: 100
    coalesced_ticks: 800
  bottlenecks[1]:
    rank: 1
    namespace: pipeline.transform
    function: process_image
    path: src/transform.jett
    line: 142
    inclusive_samples: 16000
    self_samples: 13000
    cpu_percent: 34.19
    suggestions[1]{rule,text}:
      CPU_HIGH_SELF,"Inspect the reported hot lines in pipeline.transform.process_image; self work accounts for at least half of its samples."
```

Memory mode uses the same envelope and ordering fields, with
`schema: jett.profile.v1`, `mode: memory`, `coverage: jett_heap`, and memory
totals and entries:

```toon
profile:
  schema: jett.profile.v1
  mode: memory
  status: complete
  termination: returned
  backend: interpreter_alloc_hooks
  coverage: jett_heap
  config:
    threshold_basis_points: 500
    limit: 10
    retention_metadata_budget_bytes: 67108864
  totals:
    allocation_count: 2400000
    resize_count: 120
    allocated_bytes: 891289600
    freed_bytes: 757071872
    peak_live_bytes: 134217728
    retained_bytes: 134217728
  bottlenecks[1]:
    rank: 1
    namespace: search.indexer
    function: build_index
    path: src/indexer.jett
    line: 88
    allocation_count: 1010400
    allocated_bytes: 375272960
    allocation_percent: 42.11
    freed_bytes: 300000000
    retained_bytes: 75272960
    live_at_peak_bytes: 80000000
```

Required unavailable numeric fields are rendered as `unavailable`, not zero and
not omitted. `status` is `complete`, `partial`, or `unavailable`. A partial or
unavailable profile includes one stable reason such as `interrupted`,
`collector_failure`, `cpu_metadata_limit`, `retention_metadata_limit`,
`stack_capture_limit`, or `backend_unsupported`. Renderer tests pin field names,
order, escaping, fixed-precision percentages, unavailable values, and empty
arrays.

## Platform and Backend Capabilities

A profiler backend declares capabilities before execution:

```text
cpu_tick_source
safe_stack_capture
jett_runtime_thread_registration
task_attribution
source_spans
process_cpu_time
jett_heap_allocation_hooks
free_and_resize_events
```

The selected mode's required capabilities are checked before `main`. CPU
requires a monotonic tick source and safe stack capture. Memory requires Jett
heap allocation hooks plus free and resize events. Task attribution, source
spans, and process CPU duration may be unavailable and are reported as such
without invalidating function-level data when the remaining identity is sound.

The current interpreter stage owns only one registered runtime OS thread and can
use AST statement/call spans. It reports task and actor IDs only where its
simulation has a real active runtime identity. Future bytecode and native
runtimes may use different sampling and allocator mechanisms, but no backend may
change counter definitions, silently broaden coverage to host allocations, or
invent a separate output schema.

## Ownership and Implementation Stages

`jett_profiler` owns backend-neutral event types, aggregation, deterministic
sorting, thresholding, source sanitization, suggestions, status, and human/TOON
rendering. It depends only on foundational source/span and diagnostic data, not
on the CLI or a concrete interpreter.

The driver owns profiler selection, capability negotiation, run-manifest and
checked-source metadata, lifecycle finalization, and composition with `RunOutput`.
The CLI owns argument validation and final channel/exit behavior. A runtime owns
only safe event production and excludes profiler allocations from observation.

Implementation proceeds in independently testable stages:

1. **Pure profile model and renderer.** Add `jett_profiler` with injected CPU and
   allocation event streams. Pin counters, resize/free accounting, peak and
   retained snapshots, thresholds, ties, truncation, suggestions, redaction,
   partial fields, and both renderers.
2. **CLI and run envelope.** Add the mutually exclusive flags, validation, human
   stderr report, structured profile object, and explicit captured program
   stdout/stderr fields. Setup failures occur before `main`.
3. **Interpreter CPU adapter.** Add the bounded pending-tick mechanism and safe
   stack snapshots to the existing dedicated runtime thread. Test coalescing,
   runtime/waiting buckets, interruption finalization, recursive stack limits,
   source attribution, and sampler unavailability.
4. **Interpreter memory adapter.** First introduce an explicit interpreter-owned
   allocation boundary for semantic Jett heap objects, then normalize its
   allocate/resize/free events. Rust container capacity changes, temporary
   clones, AST storage, and other interpreter implementation allocations are not
   Jett events. Exclude collector metadata and test zero-size operations, failed
   allocation, task migration, peak snapshots, retained ownership, and
   metadata-budget degradation.
5. **Concurrent and native handoff.** After HIR, MIR, allocator, and runtime work
   exists, register all runtime workers, preserve source identities through
   lowering, adapt safe native stack samples and Jett heap hooks, and run the
   same injected-event and end-to-end conformance scenarios.

CPU and memory may land as separate implementation PRs after stages 1 and 2.
Neither mode is marked implemented merely because its CLI flags parse.

## Required Verification Matrix

- CLI rejects both modes together, orphaned controls, malformed percentages,
  rates outside `1..1000`, and limits outside `1..100` before execution.
- Compile and setup failures execute no user code and emit no fabricated profile.
- Injected CPU events pin inclusive/self counts, hot lines, dominant chains,
  unavailable/runtime/waiting buckets, coalescing, stack truncation, and ties.
- The timer callback allocates nothing, pending requests stay bounded per
  worker, and the 32 MiB CPU metadata limit yields explicit partial detail.
- Percent thresholds compare exact counts; display rounding cannot admit or
  reject a bottleneck.
- Concurrent events aggregate functions across workers while preserving
  run-local task attribution and deterministic output.
- Injected allocation events pin allocation and resize counts, positive and
  negative deltas, frees, zero sizes, peak snapshots, retained sites, and IDs.
- Collector metadata is excluded, recursion is impossible, and the 64 MiB limit
  yields explicit partial fields without changing program behavior.
- Project, dependency, and stdlib paths are normalized; absolute and
  unauthorized paths never appear.
- Comments, string/byte literals, secret-bearing expressions, control
  characters, and long excerpts follow the source-redaction policy.
- Suggestion rule IDs, predicates, template wording, ordering, and omission on
  partial data are snapshot tested.
- Human and TOON output contain the same totals and ordering; program output
  cannot alter structured profile fields.
- Normal return, runtime error, collector failure, cooperative interrupt, and
  backend unsupported states preserve the selected exit policy.
- Interpreter, bytecode, and native adapters consume the same deterministic
  event fixtures and produce byte-identical `jett.profile.v1` objects.

Future native attribution depends on the HIR and MIR boundaries tracked by
[#20](https://github.com/vycdev/jett/issues/20) and
[#22](https://github.com/vycdev/jett/issues/22). Those phases do not block the
pure aggregation, CLI, or interpreter adapter stages.
