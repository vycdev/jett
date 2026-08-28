# Structured Logging Contract

Status: accepted design for [#143](https://github.com/vycdev/jett/issues/143).
Implementation has not started. Interpreter, source extraction, and later backend
work must preserve this contract.

## Context

Jett reserves the `log` namespace and already treats the bare name `log` as a
secret-output boundary, but it has no public logging declarations, capability,
sink, runtime dispatch, or focused fixtures. Older capability examples use an
illustrative `log(view stdout: Stdout, msg: string)` spelling, while the standard
library inventory promises structured logging with levels. Neither sketch
selects an event shape, filtering policy, failure behavior, or deterministic
test path.

This record selects one initial application-logging surface. It keeps logging
separate from semantic `Stdout`, compiler diagnostics, compatibility
`print`/`println`, `trace`, breakpoint snapshots, and ASP protocol output. It
does not implement logging, select a production logging framework, or define
file rotation, remote telemetry, asynchronous batching, metrics, or tracing.

## Public Surface

The canonical module is lowercase `namespace log`. The runtime-provided
capability is PascalCase `Log`. Public declarations belong in a trusted
compiler-shipped `stdlib/log.jett` file with this shape:

```jett
namespace log

export enum Level:
    debug
    info
    warn
    error

export enum Error:
    sink_unavailable
    write_failed

export struct Field:
    name: string
    value: string

export struct Event:
    level: Level
    message: string
    fields: list[Field]

export function emit(view output: Log, event: Event) returns result[nothing, Error]
export function debug(view output: Log, message: string, fields: list[Field]) returns result[nothing, Error]
export function info(view output: Log, message: string, fields: list[Field]) returns result[nothing, Error]
export function warn(view output: Log, message: string, fields: list[Field]) returns result[nothing, Error]
export function error(view output: Log, message: string, fields: list[Field]) returns result[nothing, Error]
```

The four level-named functions are source-owned convenience wrappers. Each
constructs one `log.Event` and calls `log.emit`; they do not have separate
runtime hooks or subtly different behavior. Callers pass `list()` when an event
has no fields. This keeps the declaration set explicit and avoids a second
variadic, template, macro, or map-literal logging syntax.

`log.Event` is an owned value. `log.emit` consumes it, including its field list.
A caller that needs the event after emission must write `clone` explicitly.
`Log` is borrowed and remains usable after success, filtering, or failure.
Ordinary source cannot construct or mint a `Log` capability.

## Levels and Filtering

Level order is fixed:

```text
debug < info < warn < error
```

Each runtime context configures one minimum accepted level for its `Log`
provider, or disables the provider entirely. This configuration belongs to the
runner or embedding API, not to Jett source. The initial module has no
`set_level`, global mutable filter, per-namespace override, or environment
variable convention.

An event is enabled when its level is greater than or equal to the configured
minimum. A disabled provider filters every level. Filtering has these exact
semantics:

- Jett uses ordinary eager argument evaluation. The message, field names, field
  values, list, and `log.Event` are fully evaluated before `log.emit` decides
  whether to filter.
- A filtered event returns `ok(nothing)`.
- A filtered event does not contact the sink, allocate a sequence number, write
  bytes, or consume a scripted sink outcome.
- Filtering cannot hide a type error, move error, failed argument expression,
  explicit declassification, or other source behavior.
- Compilers must not remove an argument expression merely because a build's
  current runner configuration would filter the event.

These rules make filter changes affect observations only, not source validity or
program computation. A future lazy field API would need a separate design and
must not change these operations retroactively.

## Event and Field Model

The source-owned event contains exactly a level, a message, and an ordered list
of fields. Field values are strings in the initial contract. This intentionally
avoids an open-ended runtime `any`, implicit reflection, automatic `Displayable`
conversion, or a logging-specific JSON value hierarchy.

Field order is source data. The sink preserves the list order exactly. Empty
names, repeated names, and Unicode names are allowed and preserved because
fields are encoded as an ordered list rather than a JSON object. A sink must not
silently deduplicate, sort, trim, normalize, case-fold, or reinterpret names.
Empty messages, empty values, and an empty field list are valid.

Callers convert non-string public values explicitly before constructing a
field. For example, ordinary scalar interpolation is visible in source, while a
public projection of a struct may use `json.serialize_public`. The logging API
does not inspect arbitrary nested objects or choose a display format on the
caller's behalf.

The initial API has no message-template language, positional substitutions,
implicit exception field, logger name, target/category, span identifier, or
context inheritance. A message is literal runtime string data, not a format
string interpreted by the sink. Future helpers may construct `log.Event`
values, but `log.emit` remains the one canonical emission boundary.

## Runtime Metadata

The runtime adds metadata after a source event passes filtering:

- `sequence`: a zero-based unsigned logical counter scoped to one runtime
  context and shared by every clone of that context's `Log` capability;
- `source.path`: the compiler's normalized logical path for the call site;
- `source.line`: the one-based line of the `log.emit` or level-wrapper call;
- `source.column`: the one-based column of that call.

Logical paths use `/` separators and never contain a host-absolute project root.
Project files use their project-relative path. Dependency and compiler-shipped
stdlib paths use the same stable logical identities already carried by source
maps. Synthetic source without a stable path uses the literal `<generated>`.

The call site for `log.debug`, `log.info`, `log.warn`, or `log.error` is the
user's wrapper call, not the internal line in `stdlib/log.jett`. A direct
`log.emit` uses its own call site. This requires trusted wrapper source mapping
or equivalent compiler metadata; it is not permission for arbitrary functions
to forge source locations.

There is no semantic timestamp in the first event contract. Reading wall time
would otherwise add a second hidden effect or force every log call to borrow a
`Clock`. Production adapters may add an ingestion timestamp outside the
canonical Jett record, but that timestamp is sink metadata and is absent from
captured events and canonical serialization. Programs that need a business
timestamp add an explicit field derived through `Clock.now(view clock)`.

Process IDs, thread IDs, actor IDs, build-host paths, dependency versions, and
machine names are also absent from the canonical record. This keeps captures
portable and avoids accidental environment disclosure.

## Ordering and Concurrency

`log.emit` is synchronous with respect to its provider attempt. Within one
task, enabled attempts retain program order. The provider serializes attempts
from capabilities that share a runtime context and assigns each exactly one
sequence value in that attempt order.

Concurrent tasks therefore produce one valid total order chosen by the runtime
scheduler. The exact interleaving is not deterministic unless the test also
pins scheduling. Sequence values are contiguous across provider attempts in the
context, but successful captured records may show a gap where an attempt failed.
Filtered events receive no sequence. A failed attempt consumes its assigned
sequence so a later record cannot be mistaken for the failed record.

Returning `ok(nothing)` means the sink accepted the complete canonical record.
It does not promise disk durability, remote delivery, or an external consumer's
acknowledgement. The initial operation does not queue work that can fail after
returning success.

A capability-use cancellation checkpoint occurs immediately before provider
acceptance. Cancellation before acceptance emits nothing and surfaces the
task's normal `CancelledError` through `join`; it is not `log.Error`.
Acceptance and one provider write are atomic from Jett's point of view. Once
accepted, cancellation does not turn a successful or failed write into a
cancelled call. The current sequential interpreter has no externally
cancellable primitive write, so this is a future concurrent-runtime obligation.

## Canonical Serialization

A text sink receives one compact UTF-8 JSON record followed by one LF. Keys use
this exact order:

```json
{"sequence":0,"level":"info","message":"started","fields":[{"name":"port","value":"8080"}],"source":{"path":"src/main.jett","line":12,"column":5}}
```

The level strings are exactly `debug`, `info`, `warn`, and `error`. `sequence`,
`line`, and `column` are non-negative decimal integers without leading zeroes,
except zero itself. There is no trailing space and exactly one terminating LF.
Field array order is source order.

Strings use canonical JSON escaping: `"`, `\\`, backspace, form feed, LF, CR,
and tab use their short escapes; other U+0000 through U+001F controls use a
lowercase four-digit `\u00xx` escape; all other Unicode scalar values are
written directly as UTF-8 without normalization or ASCII-only escaping. Jett
strings cannot contain unpaired surrogates. The serializer never includes host
error text or debug formatting.

The runtime may build a `json.JsonTree` and use a trusted canonical encoder, or
use an equivalent bounded encoder. It must not route public logging through
`json.serialize`, because logging has its own secret boundary, capability,
metadata, and failure contract. A binary, system-journal, or hosted adapter may
consume the typed record directly, but deterministic conformance tests must
also prove it would produce the canonical JSON line above.

## Capability and Effect Boundary

`Log` is a dedicated semantic-output capability. It is not an alias for
`Stdout`, `Stderr`, `Filesystem`, `Network`, or `Clock`:

- `main` may receive a runtime-provided `Log` capability.
- Ordinary functions borrow it as `view Log` directly or transitively.
- A capability-free function cannot emit an application log and remain pure.
- Borrowing `Stdout` does not authorize logging, and borrowing `Log` does not
  authorize arbitrary stdout or stderr writes.
- Cloning `Log` for actor handoff creates another handle to the same provider,
  filter, and sequence state; it does not duplicate the sink.
- Source cannot narrow, replace, flush, close, or reconfigure `Log` in the first
  surface.

This dedicated capability makes application logging visible in signatures and
keeps machine-readable program output independent. A runner may adapt both
`Log` and `Stdout` to the same host destination only when it still preserves
separate logical channels and capture APIs. It must never interleave log JSON
into the byte stream returned as semantic Jett stdout.

## Failure Contract

An enabled event may return one of two source-level errors:

- `log.Error.sink_unavailable`: the configured provider is absent, closed, or
  cannot begin accepting records;
- `log.Error.write_failed`: the provider accepted the attempt but could not
  accept the complete canonical record.

The runtime maps host-specific failures into these variants. It does not expose
paths, URLs, operating-system codes, dependency messages, or partial event data.
There is no automatic retry, fallback to stdout/stderr, panic, or silent
success. A caller decides whether to propagate, handle, or deliberately ignore
the result.

Filtering occurs before availability checks, so a filtered call succeeds even
when the underlying sink is unavailable. Serialization of a well-typed event is
an internal invariant. An impossible encoder failure is a compiler/runtime
defect rather than a third public error variant. Resource exhaustion follows
the runtime's ordinary fatal resource policy and must not be mislabeled as a
malformed log event.

On `write_failed`, production transports may have performed an unobservable
partial host write, but the provider must not expose a partial record through
Jett's captured-event interface. The deterministic test provider records either
one complete event or none. `emit` makes at most one provider attempt per call.

## Secret Safety

Every `log.emit` and level-wrapper argument is a compiler-enforced secret-output
boundary. The checker applies the existing structural secret rules before
runtime filtering or dispatch:

- `secret[string]` cannot become a message, field name, or field value;
- interpolation or concatenation that includes a secret remains secret and is
  rejected;
- a struct, enum, collection, alias, or refinement containing secret data
  cannot be converted implicitly for logging;
- nested secret-bearing values cannot bypass the rule by being placed inside a
  list or event;
- filtering does not make a secret argument legal;
- sink errors and runtime metadata never quote source values.

There is no logging-specific auto-redaction and no field-name heuristic for
`password`, `token`, or similar text. Such heuristics are incomplete and make
output depend on spelling rather than types.

A caller may use `secret.redact` to produce the fixed public masked value, or
`json.serialize_public` to omit secret fields from a reflected value. An
explicit `declassify` is also accepted under Jett's existing auditable policy;
logging does not add a second declassification operation. These choices remain
visible at the event construction site.

Because `log.Field.value` is only `string`, the first surface does not recursively
serialize arbitrary data. This is the conservative nested-field policy: a
caller must first cross an existing checked public-projection or explicit
redaction/declassification boundary.

## Comptime, Verify, and Build Modes

Every `log.*` emission function is effectful because it borrows `Log`.
Comptime evaluation never receives runtime capabilities, so direct and
transitive log emission is rejected before evaluation. A compiler must never
write a build-host log while evaluating a required `comptime` expression.

`verify` also receives no production `Log` capability. Verification code that
needs to assert logging behavior runs through the runner test harness with an
injected capture; it does not emit into the compiler process's own diagnostics
or test-runner protocol. Ordinary pure helper functions used by logging wrappers
remain eligible for comptime only when they neither borrow `Log` nor dispatch a
trusted sink hook.

Application logging is semantic in both debug and release modes. Release builds
do not strip `debug` events, skip argument evaluation, or rewrite logging to
`print`. Runtime filtering is the sole initial level suppression mechanism.
This differs from compatibility `print`/`println`, `trace`, and `breakpoint`,
whose debug-tooling policies may reject or omit them in release builds.

## Channel Isolation

The runner and embedding API expose separate channels for:

1. semantic application stdout/stderr;
2. accepted application log records;
3. compiler diagnostics;
4. `print`/`println` compatibility debug events;
5. trace and breakpoint events;
6. ASP or agent protocol payloads.

No channel is parsed out of another channel's text after execution. Capturing
logs does not consume stdout, and capturing stdout does not include log JSON.
Compiler diagnostics and protocol envelopes must not be sent through `Log`.
Likewise, application code cannot forge compiler diagnostics by choosing a
field name or message prefix.

A CLI may render channels to one terminal for a human, but its framing must
remain unambiguous and the structured runner result must keep them separate.
Machine-oriented modes never mix canonical log lines into TOON, JSON, or other
protocol output without a dedicated structured field.

## Deterministic Test Provider

The test runner can inject a `Log` provider with:

- a fixed minimum level or disabled state;
- an initially empty capture list;
- an optional scripted outcome for each enabled attempt: `accept`,
  `sink_unavailable`, or `write_failed`.

The provider is a harness facility, not a source constructor and not a general
mocking API. Each enabled call consumes one scripted outcome after receiving its
sequence. Filtered calls consume none. If no outcome is scripted, `accept` is
the default. Tests inspect typed captured records containing sequence, level,
message, ordered fields, and logical source metadata; they do not scrape process
stdout or depend on wall-clock time.

Independent runtime contexts have independent filters, scripts, captures, and
zero-based sequences. Cloned capabilities in one context share all four.
Provider state must not leak between tests.

Focused tests must cover:

- every level at, below, and above each minimum filter;
- eager evaluation and ownership for filtered calls;
- ordered, empty, duplicate-name, and Unicode fields;
- empty and escaped messages and values;
- exact source path, line, column, sequence, and canonical JSON line;
- successful ordering across repeated calls;
- both failure variants, sequence consumption, and no partial capture;
- capability propagation and rejection from capability-free functions;
- rejection in comptime and direct production `verify` contexts;
- direct, nested, alias/refinement-wrapped, and filtered secret attempts;
- explicit redaction, public serialization, and visible declassification;
- isolation from stdout, diagnostics, print, trace, breakpoint, and agent output;
- debug and release builds preserving the same semantic calls;
- shared cloned-provider state and independent runtime isolation.

Concurrent backend tests additionally pin one schedule when asserting exact
interleaving. Other concurrency tests assert only uniqueness, contiguity, and
record completeness.

## Source and Runtime Boundary

Trusted compiler-shipped `.jett` source owns all public `log` types, function
names, signatures, wrappers, and event construction. The checker retains only
general capability/secret enforcement plus the metadata needed to identify an
emission call site.

The minimum private runtime hook is conceptually:

```text
log_emit_kernel(view Log, event: log.Event, source: SourceLocation)
    returns result[nothing, log.Error]
```

This is pseudocode, not a public declaration. Only `log.emit` from trusted
compiler-shipped origin may dispatch it. Project and dependency code cannot
claim or merge into `namespace log`, spoof the private hook, supply source
locations, or gain trust by matching a qualified name. This remains aligned
with the trusted stdlib-origin work tracked by
[#3](https://github.com/vycdev/jett/issues/3).

The runtime owns provider injection, filtering, sequence allocation, canonical
encoding, sink adaptation, and source-level failure mapping. Public wrappers,
level selection, event and field types, and any future compositional helpers
remain source-owned wherever Jett can express them safely.

The current checker's bare `"log"` secret-boundary placeholder must be replaced
by resolved trusted `log.emit` and wrapper call handling. It must not accidentally
bless arbitrary user functions named `log` or miss qualified wrapper calls.
The resolver's reserved `log` module entry remains bootstrap behavior until
compiler-shipped source origin supplies the namespace normally.

## Future Backend Handoff

HIR and MIR must preserve the explicit `Log` operand, source call-site identity,
event value, and fallible result. Bytecode and native runtimes may use different
providers, but they must preserve:

- capability visibility and transitive effect checks;
- eager argument evaluation before filtering;
- level order, filtering, sequence, and ordering rules;
- exact source metadata without host-absolute path leakage;
- secret rejection before dispatch;
- result variants without host-specific details;
- channel isolation and no fallback to stdout/stderr;
- deterministic typed capture and canonical JSON conformance;
- release-mode preservation of application logging;
- cancellation and shared-clone behavior.

Backend implementations may integrate with a system journal or hosted logging
service behind the provider. Adapter-specific metadata is outside the canonical
record and cannot change captured conformance values. Future lowering remains
downstream of [#20](https://github.com/vycdev/jett/issues/20) and
[#22](https://github.com/vycdev/jett/issues/22).

## Deferred Work

This first contract deliberately defers:

- file sinks, rotation, retention, and filesystem path policy;
- network export, telemetry protocols, retries, and delivery guarantees;
- asynchronous queues, batching, backpressure, and flush/shutdown APIs;
- dynamic or hierarchical filters and source-level reconfiguration;
- logger names, inherited context, spans, metrics, and trace correlation;
- automatic wall-clock or monotonic timestamps;
- typed heterogeneous field values or implicit reflected serialization;
- source-level custom sink implementations;
- sampling, rate limiting, deduplication, and secret-name heuristics.

Each item changes observable effects, authority, or failure behavior and needs a
separate contract rather than an optional parameter hidden in this API.

## Implementation Stages

1. **Public source and checker boundary**
   - add `Log` as a runtime-provided capability;
   - add `stdlib/log.jett` with the selected types and wrappers;
   - resolve trusted calls and enforce capabilities and structural secret
     rejection, including comptime and verify restrictions.
2. **Interpreter provider and capture**
   - inject filter and provider state per runtime context;
   - add source metadata, sequence allocation, typed captures, failure scripts,
     and exact canonical serialization;
   - keep stdout, diagnostics, and debugging channels separate.
3. **Driver and release conformance**
   - expose deterministic harness injection and structured capture;
   - add the focused regression matrix in debug and release modes;
   - prove trusted-origin enforcement and remove bare-name placeholder logic.
4. **Later backend handoff**
   - carry calls through HIR/MIR and backend runtime interfaces;
   - reuse the same capture scenarios and canonical serialization vectors;
   - add concurrent cancellation and shared-clone conformance.

Implementation may split these stages into multiple PRs. No stage may introduce
an ambient capability-free logging alias or treat `print` as a temporary public
logging implementation.
