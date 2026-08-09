# Random Capability and Entropy Contract

Status: proposed decision for [#67](https://github.com/vycdev/jett/issues/67).
Implementation and stdlib extraction remain pending.

## Context

Jett currently exposes five hardcoded `random.*` operations without a visible
`Random` capability:

```text
random.int64(int64, int64) returns int64
random.float64() returns float64
random.bool() returns bool
random.choice[T](list[T]) returns optional[T]
random.shuffle[T](list[T]) returns list[T]
```

The checker therefore treats calls as pure, even though the interpreter draws
from Rust `rand::thread_rng()` on every call. There is no injected generator,
seed/state model, deterministic test path, or documented security guarantee.
This conflicts with Rule Set 16: a function with no capability parameter must
not use randomness.

Current runtime details are useful compatibility input, but they are not yet a
stable API. `random.int64` samples a half-open range and aborts when its lower
bound is not below its upper bound, `choice` returns `none` for an empty list,
and `shuffle` clones its input before shuffling. No focused fixture currently
pins these behaviors.

This record selects an explicit capability surface, defines deterministic
runtime injection and non-cryptographic security policy, and separates public
source declarations from private generator kernels. It does not implement a new
RNG, expose probability distributions, redesign property testing, or select a
native runtime ABI.

## Proposed Public Surface

The five existing qualified names remain canonical. In signature notation (not
complete Jett declarations or bodies), every operation visibly borrows the
runtime-provided `Random` capability as its first argument:

```text
random.int64(view rng: Random, lower: int64, upper: int64) returns result[int64, string]
random.float64(view rng: Random) returns float64
random.bool(view rng: Random) returns bool
random.choice[T](view rng: Random, view items: list[T]) returns optional[T]
random.shuffle[T](view rng: Random, view items: list[T]) returns list[T]
```

Keeping the functions in `namespace random` preserves one discoverable module
for random algorithms. `Random` is the capability value, not a second namespace
containing duplicate operation names. The explicit first parameter makes the
effect visible without renaming every existing operation.

`main` may own a `Random` supplied by the runner:

```jett
function main(random_source: Random) returns nothing:
    int64 die = random.int64(view random_source, 1, 7) handle error:
        return nothing
```

Ordinary functions receive only `view Random`. Sampling mutates opaque generator
state inside the capability, but does not consume or replace the source-level
capability value. A caller may reuse the capability after any operation.

The old capability-free signatures are transitional technical debt and are
removed when this surface lands. They cannot remain as overloads or wrappers:
zero-argument ambient randomness would preserve the hidden effect this contract
is intended to eliminate.

## Integer Sampling

`random.int64(view rng, lower, upper)` samples uniformly from the half-open range
`[lower, upper)`. The lower bound is included and the upper bound is excluded.
Every representable `int64` interval with `lower < upper` is supported, including
ranges that cross zero and the widest expressible half-open interval
`[int64.MIN, int64.MAX)`. Because the upper bound is an exclusive `int64`, this
operation cannot produce `int64.MAX`; a future full-width operation would need a
different signature rather than a special-case interpretation of the bound.

Implementations must use unbiased bounded sampling. They must not use a simple
remainder operation when the generator range is not an exact multiple of the
requested interval size. Backend implementations may use different rejection
sampling or wide-arithmetic strategies as long as every value in the requested
range has equal probability.

Range arithmetic must not overflow signed `int64`. An implementation computes
the width in an unsigned or signed type at least 128 bits wide, obtains an
unbiased unsigned offset in `0..width`, adds that offset to `lower` in the wide
type, and only then converts the proven in-range result to `int64`. An equivalent
overflow-safe algorithm is allowed, but `upper - lower` in signed `int64` is not.

When `lower >= upper`, the operation returns:

```text
fail("random.int64: lower bound must be less than upper bound")
```

The error does not include argument values, so secret-derived bounds cannot leak
through error text. Invalid bounds are rejected before generator state advances.
Provider failures are runtime contract failures described below, not additional
source-level `result` variants.

Changing the current runtime abort into an explicit `result` is intentional. An
invalid dynamic range is a recoverable domain error and must not depend on host
panic or interpreter behavior.

## Floating-Point and Boolean Sampling

`random.float64(view rng)` returns `k / 2^53`, where `k` is sampled uniformly
from the integers `0..2^53`. The result is therefore a finite exactly
representable value in `[0.0, 1.0)`. It never returns `1.0`, a negative value,
negative zero, infinity, or NaN. The provider and raw-bit algorithm may change,
but this 53-bit grid and mapping are cross-backend compatibility requirements.

`random.bool(view rng)` returns `true` and `false` with equal probability. It is
not defined by a particular bit position or by a particular call to
`random.int64`; implementations may share a private primitive while preserving
the same observable probability.

## Choice and Shuffle

`random.choice[T](view rng, view items)` behaves as follows:

- an empty list returns `none` without advancing generator state;
- a non-empty list chooses every position with equal probability;
- duplicate values retain the probability contributed by each position;
- `items` is borrowed and remains usable after the call;
- `some(value)` contains an owned clone of the selected element.

`random.shuffle[T](view rng, view items)` returns a new list and never changes or
consumes `items`. Empty and one-element inputs return an owned copy without
advancing generator state. For a list of distinct positional elements, every
permutation has equal probability. Duplicate values are still shuffled by
position; no equality-based deduplication or canonicalization occurs.

The source implementation should use an unbiased Fisher-Yates-style algorithm
over positions. It may delegate index sampling to the same private bounded
integer kernel as `random.int64`. Tests must check permutation preservation and
ownership deterministically; they must not assert that two ordinary production
shuffles differ, because chance can legitimately preserve the original order.

Both generic operations require `T` to support Jett's ordinary explicit
`clone` operation. Capability elements are rejected even though capability
handles can be cloned explicitly for actor handoff: random selection must not
silently duplicate authority. Any future non-cloneable linear value is rejected
for the same reason. Until generic constraints can spell this requirement in
the source signature, the checker reports a focused error at the call.

## Capability, Purity, and Comptime

`Random` follows Jett's ordinary capability rules:

- only `main` owns the runtime-provided capability;
- ordinary functions borrow it as `view Random`;
- a helper using randomness propagates that capability requirement to callers;
- capability-free functions cannot call any `random.*` operation;
- `verify` blocks and comptime evaluation cannot access `Random`, directly or
  through a helper;
- compilation and constant evaluation must never sample the build host's RNG.

The compiler must reject an unavailable capability before interpreter dispatch.
It must not classify the functions as pure merely because their public bodies
are compiler-shipped source wrappers.

In the target concurrent runtime, every random operation is a capability-use
cancellation checkpoint. Cancellation is a task-control outcome, not the `E` in
the operation's declared `result[T, E]`: it terminates the pending task before
argument validation or generator-state advancement, and `join` surfaces the
task's `CancelledError`. A direct synchronous call cannot be externally
cancelled. After the checkpoint, the operation runs to completion without
suspending or checking cancellation again. It may advance generator state more
than once internally—for example, once per Fisher-Yates swap—but those advances
form one atomic capability operation from the task's perspective. Cancellation
does not interrupt a draw halfway through or expose partially shuffled output.
The current sequential
interpreter's `run`/`cancel` simulation does not implement this boundary yet, so
it belongs in the capability-runtime implementation slice rather than being
claimed as current behavior.

## Production Entropy and Generator State

A production runner creates one opaque generator state when it supplies
`Random` to `main`. Initialization obtains entropy through a platform/runtime
provider rather than from Jett source. Public calls advance that state; they do
not call an ambient host RNG by qualified-name dispatch.

The stable contract requires:

- independent process invocations are not deliberately given the same fixed
  state;
- bounded integer, unit-float, and boolean sampling meet the distribution rules
  above;
- provider state is not serialized, reflected, printed, or constructible from
  ordinary source;
- a provider or algorithm may change without changing the public API;
- runtime state is isolated between independent interpreter/run contexts.

If the production provider cannot initialize, the runner fails before entering
`main` with the stable runtime error:

```text
Random: entropy unavailable
```

If a backend provider can fail while advancing or reseeding, the same runtime
error aborts the current run. It is a platform/runtime contract failure, not a
normal random domain result that every operation returns. Implementations must
not expose operating-system codes or dependency-specific wording.

The target concurrent runtime treats `clone Random` for actor handoff as another
capability handle to the same synchronized generator state; it must not duplicate
state and create identical child streams. Concurrent draws have a valid total
order chosen by the runtime, so their assignment to actors is intentionally
schedule-dependent. The current interpreter does not yet carry capability
objects or concurrent generator state; implementing and testing that shared
state is part of the future runtime handoff. Programs requiring reproducible
simulation streams need the separate future value API described below.

## Deterministic Testing and Seeding

The initial public surface has no seed parameter, `Random.seed`, or source-level
constructor. Ordinary code cannot mint capabilities, and exposing a seed on the
production capability would mix permission to consume entropy with a
reproducible pure generator model.

Interpreter, bytecode, and native test harnesses instead inject the same
backend-neutral scripted-provider interface when creating a runtime context. It
is not callable or discoverable from Jett source. The script contains typed
entries for three normalized requests:

```text
bounded(offset: uint64)  # 1 <= request width <= uint64.MAX; 0 <= offset < width
unit53(bits: uint64)     # bits must be < 2^53; result is bits / 2^53
boolean(value: bool)
```

A valid `int64` call and a non-empty `choice` consume one matching `bounded`
entry. `float64` consumes one `unit53` entry and `bool` consumes one `boolean`
entry. Fisher-Yates shuffle consumes one `bounded` entry per swap, in descending
upper-index order. Invalid integer bounds, empty choice, and empty/singleton
shuffle consume no entry. A missing entry reports:

```text
Random: test provider exhausted
```

A wrong entry kind or an offset/bits value outside the request domain reports
`Random: invalid test sample`. That invalid entry is not advanced, and the run
terminates. Production rejection sampling occurs behind the normalized
`bounded` request; kernel unit tests use controlled raw words to pin rejection
and retry behavior separately. This keeps public deterministic scenarios
identical across backends without making a production RNG's raw state or seeded
sequence part of compatibility.

The harness must support deterministic coverage of minimum and maximum accepted
integer offsets (including `[int64.MIN, int64.MAX)`), float grid edges, both
booleans, each choice position, shuffle swaps, provider failure, and
non-advancement for rejected/empty/trivial operations. Tests assert outcomes
selected by explicit scripted samples, not the sequence of Rust's current RNG
implementation.

A future reproducible simulation API may introduce an ordinary owned value such
as `random.Generator` with an explicit seed and specified state-transition
contract. Such a value would be separate from the `Random` capability and would
require a design decision about algorithm/version stability, splitting streams,
serialization, and secret seeds. It is intentionally unsupported by this first
contract rather than being hidden behind an optional seed argument.

Property-test generation remains separate. Its corpus generation, shrinking,
iteration count, and replay format are compiler/test-runner policy and must not
silently depend on the public production `Random` capability.

## Security and Secret Non-Claims

No operation in this contract is a cryptographic API. The production provider
should be initialized from appropriate platform entropy, but Jett does not yet
guarantee a named cryptographically secure generator, forward secrecy,
backtracking resistance, fork safety, key erasure, or a stable amount of entropy
per output.

This is documentation and code-generation policy, not a type-system prohibition:
the compiler cannot infer the security purpose of an ordinary integer or list.
Agents, examples, and official docs must not present `random.*` as suitable for:

- passwords, authentication tokens, API keys, or session secrets;
- encryption keys, nonces whose security requires unpredictability, or salts;
- UUID security claims;
- any protocol that requires a cryptographically reviewed random byte source.

Random outputs are ordinary public values, not `secret[T]`. A future
cryptographic byte-generation API needs a distinct security contract and an
explicit secret result policy. UUID generation remains a separate surface and
must coordinate with this entropy model rather than calling capability-free
`thread_rng()`; this contract does not choose UUID versions or representations.

## Source and Runtime Boundary

Every public `random.*` declaration belongs in trusted compiler-shipped `.jett`
source. Public names, parameter order, `view` ownership, result types, validation,
and collection behavior must not remain in a permanent checker/interpreter name
table.

Private trusted runtime kernels own only opaque generator access and primitive
unbiased sampling that Jett cannot yet express safely. The first extraction may
use kernels equivalent to:

```text
random_int64_kernel(view Random, lower, upper) returns int64
random_float64_kernel(view Random) returns float64
random_bool_kernel(view Random) returns bool
```

These are descriptive placeholders, not public names or source signatures. The
implementation may reduce the kernel set when the language can express safe
wide arithmetic and bit conversion. `choice`, `shuffle`, empty/trivial input
handling, public errors, and other compositional policy belong in real `.jett`
bodies.

Trusted hook dispatch must depend on compiler-shipped origin, not on matching a
qualified name. Project code cannot claim `namespace random`, replace public
wrappers, or spoof private kernels. The current hardcoded public signatures and
`rand::thread_rng()` interpreter arms are transitional technical debt.

## Future Backend Handoff

HIR and MIR must retain the explicit `Random` capability operand and the selected
operation. A native or bytecode backend may use a different internal provider,
but it must preserve:

- capability visibility and transitive effect checking;
- cancellation before state advancement;
- half-open unbiased integer ranges and deterministic invalid-range errors;
- finite `[0.0, 1.0)` floats and unbiased booleans;
- positional choice/shuffle distributions and non-consuming collection views;
- isolated runtime contexts and shared-state capability clones;
- deterministic provider injection for conformance tests;
- the absence of cryptographic security claims.

Backend libraries, state formats, and host errors remain implementation details.
Future lowering follows the HIR and MIR boundaries tracked by
[#20](https://github.com/vycdev/jett/issues/20) and
[#22](https://github.com/vycdev/jett/issues/22), but those phases do not block
the interpreter-facing capability and injection work.

## Implementation Slices

1. **Pin current value semantics**
   - add focused tests for half-open integer ranges, invalid bounds, float
     interval, both booleans, empty/non-empty choice, and shuffle permutation
     preservation;
   - add ownership tests that reuse lists after `choice` and `shuffle`.
2. **Add capability enforcement and deterministic injection**
   - add the explicit `view Random` signatures and runtime-provided `main`
     capability;
   - classify random calls as effects and reject them from pure, verify, and
     comptime contexts;
   - inject production and scripted providers into each runtime context;
   - add cancellation and provider-failure tests.
3. **Extract the public declarations**
   - add compiler-shipped `stdlib/random.jett` wrappers;
   - make invalid integer ranges an explicit stable `result` failure;
   - implement choice and shuffle compositionally over private sampling kernels;
   - remove hardcoded public signature knowledge and ambient `thread_rng()`
     dispatch.
4. **Preserve the contract in later backends**
   - carry the capability and operation through HIR/MIR;
   - share deterministic conformance scenarios across interpreter, bytecode, and
     native runners;
   - audit provider isolation, clone sharing, cancellation, and unbiased bounds.

## Required Regression Matrix

- Each public operation requires a visible `view Random` capability.
- Transitive callers must accept the capability; capability-free calls fail.
- `verify` and comptime calls fail without sampling a host RNG.
- `main` receives a production capability; ordinary source cannot construct one.
- Integer lower/upper edge samples stay in `[lower, upper)` without bias or
  overflow, and invalid bounds fail before state advancement.
- Float samples are finite and in `[0.0, 1.0)`; boolean samples cover both values.
- Empty choice returns `none`; each non-empty position is selectable.
- Empty/singleton shuffle does not advance state; nontrivial shuffle preserves
  all positional elements and leaves the input reusable.
- Scripted providers make outcomes and exhaustion deterministic.
- Cancelled pending tasks surface `CancelledError` through `join` before the
  random operation advances state; cancellation is not a declared result error.
- Independent runtime contexts do not share state; cloned actor capabilities do.
- Project declarations cannot claim the stdlib namespace or trusted hooks.
- Interpreter, bytecode, and native implementations satisfy the same scenarios
  without promising the same production sequence or RNG algorithm.
