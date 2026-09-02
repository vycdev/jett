# Capability Mocking and Deterministic Test Harness Contract

Status: accepted design. The source-owned script types, constructor surface, and
property-only checker boundary are implemented; per-attempt providers, execution,
replay, shrinking, and mismatch reporting remain staged.

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

`Filesystem`, `Network`, `Stdout`, `Stderr`, `Stdin`, `Process`, `Foreign`, and
`Log` are not mockable in the first slice. Each may opt in only after its public
capability contract defines typed requests, outcomes, failures, narrowing, and
provider ownership. `Foreign` in particular must not become a route for
simulating arbitrary native calls before its generated-binding and resource
contracts exist. `Log` remains a distinct structured-event capability; captured
logs do not make it an untyped mock event stream.

Jett capabilities remain a closed built-in set. There are no user-defined
capabilities to mock, and this contract does not add them.

## Property-Only Construction

`test.mock` script types are ordinary compiler-shipped data. Pure helper
functions may build and return those values. The constructor calls that return
`Random`, `Clock`, or `Environment` are different: they create test authority
and are source-valid only as expressions lexically inside a `property` body.
They execute only when that property is run by `jett test`.

They are rejected in:

- ordinary function bodies, including a helper called only from a property;
- `main` and application runtime code;
- `verify` blocks;
- required or opportunistic comptime evaluation;
- global constant initializers;
- any attempted build-time, query-time, LSP-time, or `jett run` evaluation.

Keeping construction lexical gives the compiler a small, local policy check
and prevents a seemingly pure helper from hiding authority creation. A helper
under test may accept the resulting capability normally as `view Clock`,
`view Random`, or `view Environment`.

Parse, resolution, type, ownership, capability, and context checks cover these
declarations in every command. A legal constructor call inside a property is
therefore accepted consistently by `jett build`, `jett query`, and the LSP even
though none of those modes executes it. A call in a forbidden source context
receives the same focused diagnostic in every mode. Only `jett
test` creates a property-attempt runtime and executes constructors; `jett build`
and `jett run` do not execute property bodies, and queries and the LSP never
execute constructors while answering semantic requests. Test-only runtime hooks
are linked or registered only for `jett test`; they are absent from production
artifacts.

Constructor authority follows the trusted-origin contract rather than spelling.
The checker and runner recognize a constructor only when its resolved
`DeclarationId` has `SourceOrigin::Stdlib` and is the exact entry in the
compiler-distribution `test.mock` constructor manifest. The private hook is
selected by that resolved identity. A matching namespace, exported name,
signature, logical path, or copied stdlib body from project or dependency
source grants no authority. A local import alias that resolves to the genuine
manifest `DeclarationId` remains authorized; an alias to a lookalike does not.
At execution, the runner also requires the current
mode to be a live `jett test` property attempt and the checked call site to be a
direct property-body expression; failure of either condition is an internal
runner error, never a fallback to production authority.

### Construction Identity and Source Sites

The checker assigns every legal constructor expression a stable construction
site before lowering:

```text
ProviderConstructionSiteV1 {
    property: DeclarationId
    source: FileKey
    span_start: uint64
    span_end: uint64
    lexical_ordinal: uint32
    capability_kind: Clock | Random | Environment
}

ProviderIdV1 {
    site: ProviderConstructionSiteV1
    occurrence: uint64
}
```

`source` is the immutable `FileKey`, including `SourceOrigin` and normalized
logical path. Physical discovery roots and host-absolute paths never enter the
identity or output. Spans are half-open UTF-8 byte offsets into the exact source
bytes and cover the complete constructor call. `lexical_ordinal` is zero-based
among constructor calls in that property after sorting by `(span_start,
span_end, capability_kind)`, where capability kinds order `Clock`, `Random`,
then `Environment`; duplicate spans are a compiler error rather than an
unstable tie.

A "direct property-body expression" may appear beneath ordinary statement,
branch, or loop nodes whose nearest declaration/execution-owner ancestor is the
property itself. It may not be beneath a helper, nested declaration, closure,
actor body, or spawned task. The property runner executes every constructor on
its owning attempt thread. `occurrence` is the zero-based execution
count for that construction site within the current attempt. Every normal,
replay, and shrink attempt starts every site's counter at zero. The counter
increments with checked overflow before a handle is minted; overflow fails the
attempt with the stable runner code
`TEST_MOCK_PROVIDER_ID_OVERFLOW`. An internal attempt generation is also stored
in each opaque handle to reject cross-attempt use, but it is deliberately absent
from `ProviderIdV1` so diagnostics and replay fingerprints remain stable.

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

These are structured test-harness failures. Every observed mismatch produces
exactly one `MockMismatchV1` record with fields in this order:

```text
schema = "jett.test.mock-mismatch.v1"
property:
    module:
        origin: SourceOrigin in the exact variant form below
        namespace: CanonicalNamespace
    name: CanonicalName
    kind: DeclarationKind
source_origin:
    Project { canonical_name: string }
  | Dependency { canonical_name: string, graph_path: list[string] }
  | Stdlib { compiler_distribution: string, stdlib_version: string }
logical_path: string
span_start: uint64
span_end: uint64
construction_ordinal: uint32
provider_occurrence: uint64
capability_kind: "clock" | "random" | "environment"
step_index: uint64
category: "wrong_step_kind" | "invalid_payload" | "exhausted" | "unconsumed_steps"
expected_request: optional[string]
actual_step: optional[string]
```

The source fields are copied from the provider construction site and never name
a physical root. `step_index` is the offending or first missing/unconsumed
zero-based index. `expected_request` is present for a primitive request and is
absent for the end-of-attempt consumption check. `actual_step` is absent at
exhaustion and otherwise names the stable step variant, not its payload.
Payload values, host errors, and secret data are never included.

Agent output always emits every field in the displayed order. An absent
`expected_request` or `actual_step` is the explicit `none` value; agent output
never omits the field. Stable request names are `random.bounded`,
`random.unit53`, `random.boolean`, and `clock.now`. Stable step names are
`random.bounded`, `random.unit53`, `random.boolean`,
`random.entropy_unavailable`, `clock.wall`, and `clock.unavailable`. No other
string is valid in version one.

After child tasks join or are cancelled, all observed mismatch records are
sorted by `(source_origin, logical_path, span_start, span_end,
construction_ordinal, provider_occurrence, capability_kind, step_index,
category_rank, expected_request_sort, actual_step_sort)`. Origin variants rank
`Project`, `Dependency`, then `Stdlib`.
Within a variant, fields compare in the displayed order by unsigned UTF-8 bytes;
`graph_path` compares component-by-component and a proper prefix sorts first.
Numeric fields compare numerically. Capability kind follows the rank above.
Category ranks are `wrong_step_kind = 0`, `invalid_payload = 1`, `exhausted =
2`, and `unconsumed_steps = 3`. This is the record order in human and agent
output.

For each optional-kind sort component, `none` ranks before every present value;
present strings compare by unsigned UTF-8 bytes. Because every valid string is
enumerated above, this is a closed backend-independent order. Records that still
tie are field-for-field identical, so exchanging them cannot change output bytes
or the primary `FailureFingerprintV1`. In particular, two concurrent request
kinds that observe the same non-advancing wrong step sort by
`expected_request_sort`, not scheduler arrival.

With placeholders substituted, the human first line is exactly:

```text
mock {capability_kind} at {logical_path}:{span_start}: {category} (step {step_index})
```

When present, following detail lines use this exact form and order:

```text
expected_request: {expected_request}
actual_step: {actual_step}
```

An absent optional field omits its whole line.

If no non-mismatch failure already owns the attempt, the first sorted mismatch
is the primary failure and later records are secondary. Any existing assertion,
cancellation, ordinary runtime fault, or matching scripted provider/platform
fault remains primary; cleanup mismatches are then secondary and cannot replace
it. A scripted `ClockStep.unavailable` or `RandomStep.entropy_unavailable` fault
is a runtime failure at the capability-operation call site, not a mock mismatch.
Canonical ordering makes an identical observed execution trace render
identically; tests with concurrently used providers must still pin or coordinate
scheduling when the set of observed requests matters.

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

The first failing attempt records a `FailureFingerprintV1`. Every form begins
with the property `DeclarationId`, the property's `FileKey`, and exactly one
failure-class tag: `assertion`, `runtime`, `mock_mismatch`, or `cancelled`. An
assertion fingerprint adds the assertion's half-open source span. A runtime
fingerprint adds the compiler-owned stable error code and either the originating
expression's `FileKey` plus half-open span or, when no source expression exists,
the compiler-owned `RunnerFaultSiteId` registered for that exact runner fault.
`RunnerFaultSiteId` is a stable ASCII identifier, not rendered host text or a
stack location. Matching scripted provider/platform faults use the `runtime`
form and the capability-operation call expression as their origin. A mock
fingerprint adds the complete `ProviderIdV1`, mismatch category, step index, and
optional expected and actual kind names from the primary `MockMismatchV1`.
Cancellation adds the
checkpoint callee `DeclarationId`, `FileKey`, and half-open call-expression
span. Values, rendered messages, host paths, provider payloads, task scheduling
IDs, and secondary cleanup mismatches are excluded. A shrink candidate is
retained only when its complete primary fingerprint equals the original
field-for-field. Provider exhaustion or a cleanup mismatch therefore cannot
replace the assertion or runtime failure being minimized.

A version-one replay token contains these fields:

```text
ReplayTokenV1 {
    schema: "jett.test.replay.v1"
    property: DeclarationId
    source: FileKey
    source_digest: SHA-256
    property_digest: SHA-256
    checked_compilation_digest: SHA-256
    generator_version: string
    shrinker_version: string
    generated_input_state: runner-defined versioned bytes
    failure_fingerprint: FailureFingerprintV1
}
```

`source_digest` is SHA-256 over the exact UTF-8 bytes of the owning source file.
`property_digest` is SHA-256 over the ASCII domain
`jett-property-declaration-v1`, a zero byte, the big-endian `uint64` byte length,
and the exact half-open UTF-8 source slice for the complete property declaration.
`checked_compilation_digest` binds every checked source and configuration input,
including helpers that build scripts or are called by the property. Its preimage
is this canonical record:

```text
CheckedTestCompilationV1:
    magic = ASCII "jett-checked-test-compilation-v1" followed by NUL
    compiler_compatibility_id: 32 bytes
    test_policy_revision: u32 = 1
    semantic_option_count: u32
    semantic_options sorted by key UTF-8 bytes:
        key_length: u64
        key_utf8: byte[key_length]
        value_length: u64
        value_utf8: byte[value_length]
    input_count: u64
    inputs sorted by (kind, encoded origin, logical_path UTF-8 bytes):
        kind: u8                 # 1 = source, 2 = manifest/lock/config
        origin: EncodedSourceOriginV1
        logical_path_length: u64
        logical_path_utf8: byte[logical_path_length]
        content_length: u64
        content_sha256: byte[32]

EncodedSourceOriginV1:
    tag: u8                     # 1 = Project, 2 = Dependency, 3 = Stdlib
    Project:
        canonical_name: length-prefixed UTF-8
    Dependency:
        canonical_name: length-prefixed UTF-8
        graph_path_count: u32
        graph_path_segments: graph_path_count length-prefixed UTF-8 values
    Stdlib:
        compiler_distribution: length-prefixed UTF-8
        stdlib_version: length-prefixed UTF-8
```

Every unspecified integer and every `length-prefixed UTF-8` length in this
record is unsigned little-endian; the latter uses `u64`. The input set contains
every discovered project, dependency, and stdlib source read by the checked test
compilation plus the exact manifest, lock, and configuration files that selected
that graph. Compiler-synthesized code and policy are covered by
`compiler_compatibility_id` and `test_policy_revision`; they do not invent a
fourth `SourceOrigin`. Semantic options include the target and every feature,
mode, or compiler option that can affect parse, resolution, type, ownership,
capability, secret, property, or lowering semantics. Duplicate option keys,
duplicate `(kind, origin, logical_path)` inputs, invalid logical paths, unknown
kinds or tags, invalid UTF-8, count/length overflow, and trailing bytes are
rejected.
SHA-256 of the entire record is `checked_compilation_digest`.

The compatibility ID is for the checked-test artifact kind and includes the
compiler source, locked dependencies/features, applicable bundled stdlib, schema,
and policy components required by the content-addressed-cache contract; a
parse-file compatibility ID is not reused. Semantic-option keys come from a
versioned compiler registry, and each value is that option's exact canonical
UTF-8 spelling. A semantic option without such a spelling makes replay-token
issuance unavailable rather than being omitted.

This record uses the same relocation-independent `SourceOrigin`, `FileKey`, and
tagged/length-prefixed identity principles as the trusted-origin and
content-addressed-cache contracts. It never contains a physical root,
timestamp, directory order, or presentation-only host metadata. A runner that
cannot enumerate the complete checked input graph and semantic option set must
not issue a replay token.

The version strings select one documented generation/shrinking algorithm; the
input bytes are interpreted only by that exact version. The token does not
serialize capability handles, provider cursors, production entropy, host
environment data, mock scripts, or secret payloads.

Replay first resolves the property and requires exact equality of its
`DeclarationId`, `FileKey`, source digest, property digest, checked-compilation
digest, and both runner versions. Any mismatch fails with
`TEST_REPLAY_INCOMPATIBLE` and names only the mismatched field. A compatible
replay starts a fresh attempt, evaluates the
checked-in property body, reconstructs explicit scripts at step zero, and
requires the reproduced primary `FailureFingerprintV1` to match the token.

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
claim compiler-shipped origin. The checker does not retain a second hardcoded
copy of public signatures after source extraction, but it does own the exact
trusted-constructor manifest keyed by `DeclarationId` and the narrow context
policy that only a direct property-body call can construct a capability.

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
   - identify constructors by trusted `DeclarationId`, not qualified-name text;
   - statically allow constructors only in direct property bodies and execute
     them only under `jett test`;
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
   - add canonical structured mismatch output, stable failure fingerprints, and
     source-bound versioned replay integration;
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
- Script-building data is ordinary, but capability constructors are source-legal
  only directly in property bodies and execute only under `jett test`.
- Build, query, and LSP paths parse, resolve, and check legal property calls
  without executing them; ordinary functions, `main`, verify, comptime, global
  initializers, and run-time application code reject construction.
- Only the manifest `DeclarationId` with stdlib origin can select a constructor
  hook; an alias to that identity remains valid, while project/dependency
  lookalikes (including aliases to them) cannot.
- Provider IDs use `DeclarationId`, `FileKey`, half-open UTF-8 spans, lexical
  ordinals, and checked per-site occurrences without physical paths.
- Functions under test receive ordinary capability types and cannot detect the
  provider kind.
- Capability arguments still require explicit `view` in property bodies.
- Random preserves normalized request kinds, request-domain validation, and
  no-consumption behavior.
- Clock millisecond scripts preserve positive, epoch, and pre-epoch values;
  host-level raw tests retain unavailable, invalid, and out-of-range coverage.
- Environment scripts preserve argument order, empty arguments, lookup policy,
  immutable repeated reads, and independent returned lists.
- Wrong kinds, invalid payloads, exhaustion, and unconsumed suffixes emit the
  exact `MockMismatchV1` schema in canonical provider/category order.
- Concurrent mismatches tied at one provider and step use optional request/step
  tie-breaks; field-identical duplicates cannot change the primary fingerprint.
- Separate capabilities have independent FIFO scripts and no accidental global
  ordering dependency.
- Explicit clones and actor handoffs share one provider cursor; independent
  attempts do not.
- Every normal iteration, replay, and shrink candidate begins with fresh
  providers at step zero.
- Shrinking changes generated values only and retains a candidate only when the
  complete stable primary `FailureFingerprintV1` is preserved.
- Replay binds the property identity, `FileKey`, exact source and property
  digests, the canonical full checked-compilation digest, runner versions,
  generated-input state, and failure fingerprint; it never serializes handles,
  cursor state, mock scripts, host data, or secrets.
- Mock providers never fall back to production entropy, clocks, environment, or
  I/O.
- Later backends satisfy the same typed request, exact-consumption, isolation,
  authority, and diagnostic rules.

Future capability lowering remains downstream of
[#20](https://github.com/vycdev/jett/issues/20) and
[#22](https://github.com/vycdev/jett/issues/22). The initial provider adapters
must not wait for those backends because the interpreter and property runner
already need the selected isolation boundary.
