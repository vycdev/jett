# Time and Clock Capability Contract

Status: accepted and implemented for the interpreter-backed compiler.
Future backend lowering remains handoff work.

## Context

Jett previously exposed zero-argument `time.now_ms()` and `time.now_s()`
builtins backed by direct host-clock reads. That legacy surface allowed hidden
time effects, mishandled pre-epoch values, and performed unchecked range
conversion. The implementation now uses the explicit injected boundary below.

The language design already names `Clock` as the capability for reading current
time and uses both `Time` and `Timestamp` for the returned value. This record
selects one spelling and one capability boundary before the implementation is
extended.

## Decision

The canonical current-time operation is:

```jett
Clock.now(view clock) returns time.Timestamp
```

`Clock.now` reads the wall clock. It is effectful because its result may change
between calls, so the caller must visibly borrow a `Clock` capability. The
runtime gives a real `Clock` only to `main`; ordinary functions receive
`view Clock` through their parameters.

`time.Timestamp` is the only spelling for an absolute point on the wall-clock
timeline. The ambiguous bare `Time` spelling is retired rather than kept as an
alias. Calendar values such as a local date or a zoned date-time are separate,
future types and must not reuse `Timestamp`.

## Timestamp and Duration Model

The first value model has two distinct exported types:

- `time.Timestamp` is a signed `int64` count of milliseconds from the Unix epoch,
  `1970-01-01T00:00:00Z`.
- `time.Duration` is a signed `int64` count of milliseconds between two
  timestamps.

Both are exported nominal structs, not aliases or compiler-only opaque types:

```jett
namespace time

export struct Timestamp:
    unix_milliseconds: int64

export struct Duration:
    milliseconds: int64
```

The current language has no private struct fields, so direct construction and
field reads remain valid. The nominal struct types still prevent accidentally
adding two timestamps or passing a duration where an absolute time is required.
The named `time` helpers are the canonical construction and arithmetic surface;
future private-field support may narrow direct carrier access without changing
the types or units. This contract does not define a stable JSON or other wire
encoding for either wrapper. External protocols should select an explicit
representation and use the epoch conversion helpers.

The initial source-facing operations are intentionally small. Inside
`namespace time`, their exported declarations are:

```jett
export function from_unix_milliseconds(value: int64) returns Timestamp
export function to_unix_milliseconds(value: Timestamp) returns int64
export function to_unix_seconds(value: Timestamp) returns int64

export function duration_milliseconds(value: int64) returns Duration
export function duration_seconds(value: int64) returns result[Duration, string]
export function duration_to_milliseconds(value: Duration) returns int64

export function difference(start: Timestamp, end: Timestamp) returns result[Duration, string]
export function add(value: Timestamp, delta: Duration) returns result[Timestamp, string]
export function subtract(value: Timestamp, delta: Duration) returns result[Timestamp, string]
export function before(left: Timestamp, right: Timestamp) returns bool
```

The `Clock.now` declaration lives in the compiler-shipped `Clock` namespace and
is specified in the source/runtime boundary below.

`time.difference(start, end)` computes `end - start`, so its result may be
negative. `time.to_unix_seconds` uses Euclidean floor division: every instant in
the millisecond interval `[-1000, 0)` maps to Unix second `-1`. This keeps
pre-epoch ordering consistent instead of truncating toward zero.

Arithmetic that cannot fit in signed `int64` returns a deterministic
`result[..., string]` failure. The messages are
`time.duration_seconds: integer overflow`,
`time.difference: integer overflow`, `time.add: integer overflow`, and
`time.subtract: integer overflow`. The source helpers must use checked
arithmetic; they must not wrap, saturate, or inherit host-language overflow
behavior.

## Precision, Range, and Host Conversion

Milliseconds are the canonical precision for the first contract. A backend may
sample a clock with finer precision, but it floors to the preceding whole
millisecond before constructing `time.Timestamp`. It must not promise
microsecond or nanosecond precision through this API.

Negative timestamps are valid and represent instants before the Unix epoch.
The supported range is exactly `int64.MIN..int64.MAX` milliseconds around that
epoch. A runtime must convert its native clock representation with checked
arithmetic. If the host value cannot be represented, `Clock.now` reports the
stable runtime error `Clock.now: timestamp is outside int64 millisecond range`;
it must not return zero, wrap, or saturate.

`Clock.now` is otherwise infallible at the source signature. An unavailable or
unrepresentable runtime clock is a runtime/platform contract failure rather than
an ordinary domain result that every call site must handle. These failures abort
the current interpreter/run invocation through its ordinary runtime-error path;
they are not `result` values and cannot be handled in Jett source. An unavailable
host clock reports the separate stable error `Clock.now: wall clock
unavailable`.

## Wall Clock and Monotonic Time

`time.Timestamp` is wall-clock data. It can be compared with an external
timestamp or converted to an epoch count. Wall time may move backward or forward
when the operating system clock is adjusted, so subtracting two `Clock.now`
values is not a reliable elapsed-time measurement.

Monotonic time is a distinct use case and is not part of this first API. A
future timer/scheduler design should introduce a separate opaque
`time.Instant` value and an explicitly monotonic clock operation. An `Instant`
must be comparable only with values from the same runtime clock domain; it must
not convert to Unix time or serialize as a wall timestamp. `Timestamp`,
`Duration`, and a future `Instant` are not interchangeable.

This split keeps one canonical wall-clock operation now without prematurely
selecting timer, sleep, deadline, or scheduler semantics.

## Capability, Purity, and Determinism

`Clock` follows the ordinary capability ownership rules:

- `main` owns the production runtime-provided `Clock`; a property body may own
  the narrow test capability described below.
- Other functions borrow it as `view Clock`; sampling never consumes it.
- A function that samples time declares `view Clock` directly or transitively.
- Capability-free functions cannot call `Clock.now` and remain pure.
- `verify` blocks and comptime evaluation cannot access `Clock`, including
  indirectly through a helper.
- Constant evaluation must never substitute build-host time into an artifact.

The interpreter and later backends receive the clock through an injected runtime
interface. The production runner supplies a system wall clock. The interface
returns either a wide raw sample (`i128` signed Unix seconds plus normalized
subsecond nanoseconds) or an unavailable-clock fault; checked conversion into
milliseconds happens after that boundary. This lets driver tests cover
pre-epoch flooring, target-range overflow, and host failure instead of injecting
an already-valid `int64` result.

A fake clock owns a sequence per property-attempt runtime context. Each
`Clock.now` consumes one entry. Under `test.mock`, exhaustion emits
`MockMismatchV1` category `exhausted` under the exact schema and output rules in
the [capability mocking contract](capability_mocking_test_harness_contract.md);
it never reuses a value silently. A matching `ClockStep.unavailable` remains the
ordinary stable wall-clock runtime fault and is not a mismatch. The existing
low-level Rust driver adapter may temporarily retain `Clock.now: test clock
exhausted` as a private host-test diagnostic, but that string never appears in
property human/agent output, a shrink fingerprint, or a replay token and is not
a backend compatibility surface. Cloned
`Clock` capabilities, including clones passed to actors, share that runtime
provider. Tests involving concurrent actors should use repeated equal samples
unless the scheduler order is itself pinned. The fake remains a private
test-harness provider. Its sole source facade is the compiler-shipped
`test.mock.clock(list[test.mock.ClockStep])` constructor defined by the
[capability mocking contract](capability_mocking_test_harness_contract.md).
That declaration is discoverable and statically checked in every compiler mode,
but it executes only as a direct property-body expression under `jett test`.
The resolved constructor must have `SourceOrigin::Stdlib` and the manifest
`DeclarationId`; lookalike project/dependency declarations cannot mint a
`Clock`. There is no production or ordinary-function source constructor.

Pure calendar and arithmetic tests should construct `time.Timestamp` values
with `time.from_unix_milliseconds`. Capability integration tests belong in the
runner/driver harness, where a fake `Clock` can pin repeated calls, backward
wall-clock adjustments, pre-epoch values, and range failures.

## Compatibility Policy

The zero-argument `time.now_ms()` and `time.now_s()` builtins were removed when
`Clock.now` landed. They cannot remain as compatibility wrappers because a
zero-argument clock read would preserve the hidden effect this contract removes.

Migrations are mechanical:

```jett
# Before
int64 milliseconds = time.now_ms()
int64 seconds = time.now_s()

# After
time.Timestamp now = Clock.now(view clock)
int64 milliseconds = time.to_unix_milliseconds(now)
int64 seconds = time.to_unix_seconds(now)
```

The checker should diagnose the removed names with a direct replacement hint.
There is no permanent overload of `time.now_ms` or `time.now_s` that accepts a
`Clock`; keeping a single current-time entry point is easier for agents to
query, generate, and review.

## Source and Runtime Boundary

The minimum runtime-backed kernel is only the operation that samples an injected
wall clock and returns checked signed milliseconds to `Clock.now`.

The intended public declaration shape is a trusted compiler-shipped source
wrapper, with the runtime hook kept private:

```jett
namespace Clock

export function now(view clock: Clock) returns time.Timestamp:
    int64 milliseconds = clock_now_kernel(view clock)
    return time.Timestamp(unix_milliseconds: milliseconds)
```

`clock_now_kernel` is pseudocode for a private trusted runtime hook, not a public
source API. The first implementation may need loader/resolver support for the
compiler-shipped `Clock` namespace, analogous to other stdlib namespace
fragments. The end state keeps the public declaration and signature in source;
only trusted-origin metadata may connect its private hook to interpreter,
bytecode, or native runtime dispatch. Project code cannot provide or spoof that
hook.

Compiler-shipped `.jett` source owns the public `time` declarations and the
bodies for epoch conversions, checked timestamp/duration arithmetic,
comparison, and difference. The compiler does not retain a hardcoded table of
those public helper names or signatures. The hardcoded `time.now_ms` and
`time.now_s` checker and interpreter arms have been removed.

The first extraction does not include:

- calendar fields, leap-year logic, parsing, or formatting;
- time-zone databases, local-time conversion, or daylight-saving policy;
- monotonic instants, sleep, timers, deadlines, or scheduler integration;
- locale data;
- HIR, MIR, bytecode, LLVM, or platform ABI lowering beyond preserving this
  contract.

Those features should be tracked separately. Calendar and time-zone helpers may
be source-defined over vetted private runtime/data kernels where platform or
bundled database access is required, but those kernels remain implementation
details rather than public compiler-owned functions.

## Implementation Status

1. **Capability enforcement and deterministic injection — complete**
   - add the checked public `Clock.now(view Clock) -> time.Timestamp` boundary;
   - inject the clock into the interpreter instead of calling
     `SystemTime::now()` inside builtin dispatch;
   - reject `Clock.now` from pure functions, `verify`, and comptime contexts;
   - add driver tests with fixed, repeated, backward, pre-epoch, unavailable,
     out-of-range, and exhausted sequences.
2. **Source-owned value helpers — complete**
   - add compiler-shipped `time.Timestamp` and `time.Duration` declarations;
   - implement conversions, comparison, difference, and checked arithmetic in
     `.jett` source;
   - add signature/query and ownership regressions proving the public surface
     resolves from compiler-shipped source.
3. **Remove ambient compatibility builtins — complete**
   - remove zero-argument `time.now_ms` and `time.now_s` checker/interpreter
     dispatch;
   - add focused diagnostics with the `Clock.now` conversion replacements;
   - replace the host-clock-dependent fixture with injected-clock tests.
4. **Future backend handoff — pending**
   - carry the `Clock` capability operation explicitly through HIR and MIR;
   - lower it through one runtime ABI that returns checked signed milliseconds;
   - mirror the interpreter tests for native and bytecode backends, including
     pre-epoch flooring, injected sequences, and range failure.

## Required Regression Matrix

- `Clock.now` requires a visible `Clock` capability.
- Calling a helper that samples time propagates the capability requirement.
- `verify` and comptime calls are rejected without reading the host clock.
- The fake runtime clock makes repeated and backward-moving samples
  deterministic.
- Epoch, one millisecond before epoch, and ordinary positive timestamps convert
  consistently.
- Second conversion floors negative sub-second values.
- Timestamp and duration arithmetic report overflow without wrapping.
- `time.Timestamp` and `time.Duration` cannot be mixed accidentally.
- Removed `time.now_ms` and `time.now_s` calls receive migration diagnostics.
- Future native/bytecode implementations match the interpreter contract.

Deterministic capability test infrastructure should coordinate with
[#67](https://github.com/vycdev/jett/issues/67). Explicit capability lowering
for future native backends remains downstream of
[#20](https://github.com/vycdev/jett/issues/20) and
[#22](https://github.com/vycdev/jett/issues/22).
