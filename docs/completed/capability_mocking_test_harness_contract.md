# Capability Mocking and Deterministic Test Harness Contract

Status: accepted as the design for `test.mock`; implementation is pending.

Tracked by [#145](https://github.com/vycdev/jett/issues/145).

## Context

Jett makes semantic effects visible by requiring closed, compiler-known
capabilities such as `Clock`, `Random`, and `Environment`. That rule makes an
effectful function easy to identify, but the language does not yet provide a
source-facing way for a property test to supply deterministic capability
behavior.

The interpreter-backed compiler already has three useful test seams:

- `RandomTestSample` supplies typed normalized bounded, unit-float, and boolean
  samples;
- `ClockTestSample` supplies raw wall-clock samples and unavailable-clock
  faults;
- `EnvironmentTestSnapshot` supplies isolated launch arguments and environment
  entries.

Those Rust APIs are separate driver helpers. They are not visible in Jett
source, cannot be coordinated by a property block, and do not yet define a
common isolation, exhaustion, replay, shrinking, ownership, or future-backend
contract. The current property runner also reuses one interpreter across
iterations and shrinking attempts, which is incompatible with stateful test
providers unless each attempt receives a fresh provider registry.

This record selects a test-only source facade over typed harness providers. It
does not introduce general dependency injection, user-defined capabilities,
function monkey-patching, or production capability constructors.

## Decision Summary

`test.mock` is a compiler-shipped, test-only namespace. It exposes ordinary
source-defined script records and capability-specific constructor functions.
A constructor may mint one capability only while directly executing a
`property` block under `jett test`.

The selected model is a coordinated split:

1. Jett source owns discoverable typed script values and the public
   `test.mock.*` constructor declarations.
2. The checker treats capability construction as a test-only effect and rejects
   it outside a property block.
3. The test runner installs one private provider registry for each property
   attempt and gives constructed capabilities opaque handles into that
   registry.
4. Each capability keeps its own typed FIFO script. There is no implicit global
   ordering between different capabilities.
5. Existing Clock, Random, and Environment providers adapt to this registry;
   they are not replaced by an untyped callback or map of names to values.

A function under test receives the resulting capability through its normal
signature. It cannot detect whether the provider is production or scripted.
No new mock parameter, interface, annotation, or dynamic dispatch rule is
added to application code.

## Initial Public Surface

The first source-owned declarations live under `namespace test.mock`. The
following notation selects names and shapes; implementation bodies and exact
constructor syntax should use normal Jett declarations in
`stdlib/test/mock.jett`.

```jett
namespace test.mock

export enum RandomStep:
    bounded(offset: uint64)
    unit53(bits: uint64)
    boolean(value: bool)
    entropy_unavailable

export enum ClockStep:
    wall(unix_milliseconds: int64)
    unavailable

export struct EnvironmentEntry:
    name: string
    value: string

export function random(steps: list[RandomStep]) returns Random
export function clock(steps: list[ClockStep]) returns Clock
export function environment(
    arguments: list[string],
    entries: list[EnvironmentEntry],
) returns Environment
```

These are capability-specific constructors, not overloads of a generic
`mock[T]`. A generic constructor would hide which provider contract validates
the script and would encourage unsupported capabilities to appear usable.
Likewise, `test.mock` does not accept function names, namespace strings,
interfaces, arbitrary error strings, or callback closures.

The constructor consumes its script inputs and freezes an independent provider
inside the current property attempt. Mutating or consuming a source list later
cannot alter provider behavior. Constructor calls do not consume an operation
step themselves.

The initial surface covers the implemented provider contracts:

- `Random` uses an ordered `list[RandomStep]`;
- `Clock` uses an ordered `list[ClockStep]`;
- `Environment` uses one immutable snapshot rather than an operation script.

`Filesystem`, `Network`, `Stdout`, `Stderr`, `Stdin`, `Process`, and `Foreign`
are not mockable in the first slice. Each may opt in only after its public
capability contract defines typed requests, outcomes, failures, narrowing, and
provider ownership. `Foreign` in particular must not become a route for
simulating arbitrary native calls before its generated-binding and resource
contracts exist.

Jett capabilities remain a closed built-in set. There are no user-defined
capabilities to mock, and this contract does not add them.

## Property-Only Construction

`test.mock` script types are ordinary compiler-shipped data. Pure helper
functions may build and return those values. The constructor calls that return
`Random`, `Clock`, or `Environment` are different: they create test authority
and are legal only as expressions lexically inside a `property` body executed
by `jett test`.

They are rejected in:

- ordinary function bodies, including a helper called only from a property;
- `main` and application runtime code;
- `verify` blocks;
- required or opportunistic comptime evaluation;
- global constant initializers;
- `jett build` and `jett run` execution paths.

Keeping construction lexical gives the compiler a small, local policy check
and prevents a seemingly pure helper from hiding authority creation. A helper
under test may accept the resulting capability normally as `view Clock`,
`view Random`, or `view Environment`.

The declarations remain discoverable to source queries so tools can explain
the API and produce a focused diagnostic in a forbidden context. Test-only
runtime hooks are linked or registered only for `jett test`; they are not
included in production artifacts.

Example:

```jett
function sampled_label(view clock: Clock, view rng: Random) returns string:
    time.Timestamp now = Clock.now(view clock)
    bool enabled = random.bool(view rng)
    int64 milliseconds = time.to_unix_milliseconds(now)
    return "{milliseconds}:{enabled}"

property sampled_label_uses_both_capabilities:
    Clock clock = test.mock.clock(list(
        test.mock.ClockStep.wall(unix_milliseconds: 1250),
    ))
    Random rng = test.mock.random(list(
        test.mock.RandomStep.boolean(value: true),
    ))

    string label = sampled_label(view clock, view rng)
    assert label == "1250:true"
```

Property `given` declarations continue to generate ordinary data only.
Capability types and `test.mock` script types are not automatically generated.
A property may compute an explicit script from generated inputs, but the script
is re-evaluated for every attempt.

## Capability Authority and Ownership

A mock capability has the same source type and operation surface as a
production capability. The test-only constructor changes the provider, not the
type.

The ordinary ownership rules remain in force:

- the property block owns the constructed capability;
- functions under test borrow it with `view`;
- a capability cannot be inserted into an implicitly copyable aggregate,
  reflected, serialized, compared, printed, or converted into script data;
- the capability and private cursor cannot escape the property attempt;
- no operation reveals whether a provider is scripted.

The testing rule that ordinary values are implicitly reusable in property and
verify bodies does not copy capability authority. Capability arguments still
require explicit `view`, moves remain visible, and duplication still requires
an explicit capability `clone` where that capability permits cloning.

An explicit clone shares the same provider identity and cursor. It never copies
remaining steps to create an identical branch. Actor handoff follows the same
rule: handles passed to actors refer to the attempt's shared provider. Provider
storage must remain alive until all tasks in that attempt have joined or been
cancelled.

A capability narrowed by a future capability-specific operation retains the
same root provider and gains only the narrower authority selected by that
operation. A mock constructor cannot bypass or widen the narrowing policy.

## Script Execution and Ordering

Each sequential provider consumes steps in FIFO order. Every primitive
capability request consumes at most one step:

- a successful matching request consumes its step;
- a matching scripted platform fault consumes its step, then reports the
  capability contract's fault;
- a kind mismatch or invalid payload fails the attempt without advancing;
- requesting a step after exhaustion fails the attempt;
- operations that the underlying capability contract says do not consult the
  provider consume nothing.

Random preserves the normalized request model from its accepted contract:

- `random.int64` and non-empty `random.choice` request `bounded`;
- `random.float64` requests `unit53`;
- `random.bool` requests `boolean`;
- a nontrivial source-composed shuffle requests one `bounded` step per swap;
- invalid integer bounds, empty choice, and empty or singleton shuffle consume
  no step.

A `bounded` offset is checked against the width of the actual request when it
is consumed. A `unit53` payload must be below `2^53`. Wrong kinds and values
outside the request domain are invalid scripts, not values silently reduced
modulo a range. `entropy_unavailable` represents the Random provider fault
already reserved by the Random contract; it is not a source-level
`result` failure.

A source-facing `ClockStep.wall` names exact signed Unix milliseconds. The
adapter converts it to the raw seconds-and-nanoseconds provider shape using
flooring that preserves negative timestamps. This keeps source scripts simple
while continuing to test `Clock.now` through the injected provider boundary.
Host-side conformance tests retain raw `i128` seconds, invalid nanoseconds, and
out-of-range samples because Jett has no `int128` source type. `unavailable`
selects the existing stable wall-clock fault.

`Environment` is an immutable snapshot. Calls do not consume entries. Argument
order, empty arguments, duplicate-name behavior, platform-specific name
comparison, and returned list independence follow the Environment contract.
Jett strings can represent only valid Unicode; host-side conformance tests
retain invalid-Unicode snapshot cases.

There is deliberately no total order across separate Clock, Random, and
Environment providers. Each provider's local FIFO is contractual, while the
relative order of a Clock request and a Random request is not observed by the
harness. Tests should assert application outcomes rather than couple unrelated
capabilities through an invisible global event stream. A future protocol that
requires cross-capability ordering needs an explicit typed scenario design; it
must not be inferred from this contract.

Within one provider, calls from concurrent tasks are linearized through the
shared cursor. Which actor receives which step may be schedule-dependent.
Tests that require a particular actor assignment must first pin the scheduler
or coordinate calls in source. Equal repeated steps are appropriate when only
shared-provider behavior matters.

## Exhaustion, Unused Steps, and Invalid Scripts

Scripts are exact expectations. At the successful end of each property
attempt, every sequential provider constructed in that attempt must have no
remaining steps. This catches both extra calls and missing calls:

- a call after the final step reports exhaustion at the next zero-based step
  index;
- an unconsumed suffix reports the capability kind and first unconsumed index;
- a wrong step kind reports the expected request kind and actual script kind;
- an invalid payload reports the capability kind and step index.

These are structured test-harness failures. Human rendering may include stable
plain text, while agent output includes at least property name, capability kind,
step index, expected request kind, actual step kind, and failure category.
Payload values are omitted when they may contain secrets.

Environment snapshots have no consumption check. Construction still validates
what can be validated without a request, while capability-specific operation
rules validate names and values at the same point as production.

A scripted operation failure must preserve the operation's declared behavior.
If a future filesystem operation returns `result[T, FsError]`, its typed
scripted failure becomes that exact `fail` value. A platform/runtime fault for
an otherwise infallible operation terminates the attempt through the ordinary
runtime-fault path. `test.mock` cannot inject an undeclared arbitrary error or
turn an infallible API into a `result` only for tests.

## Runtime Isolation and Lifecycle

Every execution attempt receives a fresh `TestRuntimeContext` containing a
private provider registry. An attempt means:

- one normal property iteration;
- one replay execution; or
- one candidate execution during shrinking.

No provider, capability handle, cursor, invocation record, or captured
snapshot survives into another attempt, property block, test file, or `jett
test` invocation. Production providers are never used as a fallback when a
mock script is missing or exhausted.

The runner must wait for structured child tasks before checking unused steps.
A task still running at property completion is a test failure. Cancellation
uses the capability contract's checkpoint rule: a request completed before
cancellation keeps its consumed step; a request not started consumes nothing.
A primitive provider request is atomic.

Cleanup runs even when an assertion, provider fault, panic-equivalent runtime
error, or cancellation fails the attempt. Cleanup must not hide the original
failure. If cleanup also finds unconsumed steps, agent output records the
secondary mismatch without replacing the primary assertion or runtime fault.

## Property Generation, Replay, and Shrinking

Mock scripts and property-generated inputs are separate inputs with different
owners:

- the property runner owns generation order, seeds, replay tokens, and shrinking
  for `given` values;
- source code owns explicit `test.mock` scripts;
- each candidate execution re-evaluates the property body and reconstructs its
  providers from step zero.

The shrinker never mutates a live provider and never independently deletes,
reorders, or rewrites script steps. It shrinks only generated `given` values,
then re-runs the complete property. If a script is derived from those values,
normal source evaluation may produce a different script for the candidate.
The candidate is retained only when it reproduces the same primary failure
category. Provider exhaustion must not accidentally replace the assertion that
the shrinker is trying to minimize.

A replay token identifies the property, generated-input state, and runner
version needed to reproduce the input. It does not serialize capability
handles, provider cursors, production entropy, host environment data, or secret
script payloads. Replaying starts fresh, evaluates the checked-in property body,
and reconstructs the same explicit scripts. If source or runner versions no
longer match, replay fails clearly instead of attempting a best-effort run.

Failure reports include enough public data to reproduce the generated inputs
and the provider mismatch location. Secret-bearing generated values or future
scripted capability payloads follow the ordinary secret-output policy: the
report records type and location, not secret contents.

## Verify and Comptime Boundary

`verify` remains closed and pure. It cannot construct a mock capability, accept
one from the runner, or call a capability-using helper. The existence of a
deterministic provider does not make an effect pure and does not permit build
artifacts to depend on scripted time, randomness, environment, filesystem, or
network behavior.

Required `comptime expression` evaluation follows the same rule. Optimizers may
not execute mock constructors or capability calls while folding ordinary code.

Property blocks run only under `jett test`, after normal parse, resolution,
type, ownership, capability, and secret checks. Their ability to construct test
capabilities is a narrow runner authority, not a general comptime allowlist.

## Public Source and Private Harness Boundary

The permanent public surface belongs in compiler-shipped `.jett` source:

- script enums and records;
- constructor names, argument order, ownership, and return types;
- source-level validation that Jett can express without inspecting a runtime
  request;
- documentation examples.

Private trusted hooks own only behavior that ordinary source cannot perform:

- minting an opaque capability handle for the current property attempt;
- registering a typed provider in that attempt's registry;
- converting a Clock millisecond step to the raw provider representation;
- consuming and validating a step against a primitive runtime request;
- exact-consumption checks and structured mismatch metadata.

Project and dependency code cannot reopen `test.mock`, spoof a private hook, or
claim compiler-shipped origin. The checker does not retain a permanent
hardcoded table of public signatures after source extraction, but it does own
the narrow context policy that only a property body under `jett test` may call
a capability constructor.

Test-only hooks are not compiler intrinsics available to normal Jett programs.
They are runner services reached through trusted stdlib declarations. Future
bytecode and native test runners must implement the same provider protocol
rather than lower constructors to production OS capabilities.

## Existing Provider Adaptation

The first implementation consolidates the current independent driver options
without weakening their typed contracts:

1. `RandomTestSample` becomes or adapts to the Random portion of the common test
   provider registry. Its normalized bounded, unit53, and boolean requests stay
   unchanged.
2. `ClockTestSample` remains the host-level raw wall-clock conformance type. The
   source `ClockStep` adapter produces valid raw samples, while host tests retain
   unavailable, malformed, pre-epoch, and out-of-range coverage.
3. `EnvironmentTestSnapshot` becomes or adapts to the Environment portion of
   the registry. Host-only invalid-Unicode entries remain available.
4. Existing focused Rust driver helpers may remain temporarily as thin setup
   adapters, but they must create the same isolated context and run the same
   provider validation as `test.mock`.
5. Production Random, Clock, and Environment initialization remains separate and
   is never selected inside a property attempt that constructed a mock.

This common registry is an internal architecture, not a public untyped provider
trait. Each capability keeps a typed adapter so future backends can share
conformance scenarios without sharing Rust enums or implementation libraries.

## Deferred Capabilities

A future capability joins `test.mock` only after its own contract answers:

- the exact typed request and response shapes;
- declared operation failures versus runtime/platform faults;
- request ordering and which operations consume provider state;
- authority narrowing and provenance;
- cloning, actor handoff, cancellation, and cleanup;
- secret-bearing request and diagnostic redaction;
- deterministic backend-neutral representation.

Filesystem and Network mocks must enforce the same narrowed roots, hosts,
ports, protocols, and other authority as production handles. A script is not
permission to perform a request outside the mocked capability's authority.
Mocks never perform real fallback I/O.

Captured Stdout/Stderr and structured logging may use sink-specific test
providers, but they do not justify a generic event stream in this contract.
Their ordering and secret policy belong to their own public capability or
logging contracts.

## Implementation Slices

1. **Source surface and context gate**
   - add `stdlib/test/mock.jett` with the initial script types and constructors;
   - reserve trusted `test.mock` namespace fragments without allowing project
     code to reopen them;
   - allow constructors only in property bodies under `jett test`;
   - keep capability arguments, moves, and explicit views enforced in tests.
2. **Isolated provider registry**
   - introduce one test runtime context per property attempt;
   - adapt Random, Clock, and Environment providers without changing production
     initialization;
   - share provider cursors across explicit capability clones and actor handles;
   - check exact script consumption after joined tasks finish.
3. **Property runner integration**
   - stop reusing provider state across iterations and shrink candidates;
   - distinguish primary assertion/runtime failures from secondary script
     mismatches;
   - add structured mismatch output and versioned replay integration;
   - keep scripts separate from generated inputs and shrinking.
4. **Focused interpreter regressions**
   - cover valid calls, wrong kinds, invalid payloads, exhaustion, unused steps,
     no-consumption operations, and independent capabilities;
   - cover per-attempt, per-property, and per-run isolation;
   - cover clone sharing, actor handoff, cancellation, and cleanup as concurrent
     runtime behavior becomes available;
   - prove verify, comptime, ordinary functions, build, and run cannot construct
     mocks.
5. **Future backend conformance**
   - define a backend-neutral provider request/result protocol at HIR/MIR and
     runner boundaries;
   - run the same exact-consumption and isolation scenarios for interpreter,
     bytecode, and native test runners;
   - add capabilities only through typed adapters after their contracts land.

## Required Regression Matrix

- `test.mock` declarations resolve from compiler-shipped source.
- Project and dependency namespaces cannot replace public declarations or
  private hooks.
- Script-building data is ordinary, but capability constructors are legal only
  directly in property bodies under `jett test`.
- Ordinary functions, `main`, verify, comptime, global initializers, build, and
  run reject mock capability construction.
- Functions under test receive ordinary capability types and cannot detect the
  provider kind.
- Capability arguments still require explicit `view` in property bodies.
- Random preserves normalized request kinds, request-domain validation, and
  no-consumption behavior.
- Clock millisecond scripts preserve positive, epoch, and pre-epoch values;
  host-level raw tests retain unavailable, invalid, and out-of-range coverage.
- Environment scripts preserve argument order, empty arguments, lookup policy,
  immutable repeated reads, and independent returned lists.
- Wrong kinds, invalid payloads, exhaustion, and unconsumed suffixes report the
  capability and step index deterministically.
- Separate capabilities have independent FIFO scripts and no accidental global
  ordering dependency.
- Explicit clones and actor handoffs share one provider cursor; independent
  attempts do not.
- Every normal iteration, replay, and shrink candidate begins with fresh
  providers at step zero.
- Shrinking changes generated values only and retains a candidate only when the
  primary failure category is preserved.
- Replay never serializes capability handles, cursor state, host data, or secret
  payloads.
- Mock providers never fall back to production entropy, clocks, environment, or
  I/O.
- Later backends satisfy the same typed request, exact-consumption, isolation,
  authority, and diagnostic rules.

Future capability lowering remains downstream of
[#20](https://github.com/vycdev/jett/issues/20) and
[#22](https://github.com/vycdev/jett/issues/22). The initial provider adapters
must not wait for those backends because the interpreter and property runner
already need the selected isolation boundary.
