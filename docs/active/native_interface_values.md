# Native interface values

The full native parity audit found an accepted runtime surface absent from the
fixture inventory: a concrete implementation can be stored in an interface
binding, returned as an interface, passed to an interface parameter, and mixed
with other implementations in a `list[Interface]`. The interpreter dispatches
an interface-qualified method using the concrete runtime value. The current
native path originally handled concrete interface calls but rejected these dynamic
calls. The first implementation stage below now supports direct erased values.
This is an implementation gap, not a proposed language feature or exclusion.

## Existing behavior to preserve

An interface `Named` with `name(view self: Named) returns string`, implemented
by `User` and `Group`, accepts a function `show(view item: Named)` whose body
calls `Named.name(view item)`. A function returning `Named` may select either
implementation, and a `list[Named]` may contain both. The interpreter prints
the concrete record in a trace while labeling the binding `Named`.

No new source spelling, cast, implementation search, or interpreter fallback
belongs in the native backend. The checker remains the authority for interface
membership and method signatures. Exact checked implementation bodies supply
native dispatch targets, including when an inherent method shares a name.

## Implementation handoff

1. Distinguish a concrete checked method call from an interface dispatch slot
   in the checker handoff, including generic and reflected specializations.
2. Preserve interface coercions explicitly through HIR and MIR at declarations,
   assignments, arguments, returns, aggregates, collection elements, and sums.
   The existing compatibility relation also admits nested container and function
   types; those boundaries must be covered, rather than supporting parameters
   alone.
3. Use a context-owned erased value carrying the concrete payload, its checked
   type identity, ownership, pending depth, and typed debug layout. Dispatch
   selects compiler-generated targets from that identity. Native code must not
   rediscover implementations by source spelling at runtime.
4. Preserve the current move/view behavior of interface bindings and release
   every owner on overwrites, failures, and normal exits. Debug observations must
   format the concrete payload with recursive secret redaction.
5. Add linked differential coverage for multiple implementations, parameters,
   returns, stored values, collections, callbacks, clone/view behavior, pending
   values, and cleanup. Only then close this release gate.

Clean host packaging checks run independently while this semantic gap is open.

## Implemented foundation and remaining work

HIR inserts interface coercions at typed boundaries and synthesizes dispatchers
from exact checked implementation records. Native boxes own a cloned concrete
payload plus its type identity and debug layout; pending depth belongs to the
outer box. Clones own independent payloads, and allocation failure releases
partially initialized boxes. The interpreter now uses concrete nominal field
types when debugging a value declared as an interface, so secret fields remain
redacted on both paths.

Linked differential coverage includes primitive and nominal payloads, mixed list
literals, record/enum/machine fields, actor state and messages, parameters and
returns, explicit clones, nested pending values, typed callbacks, generic bodies,
consumed-binding snapshots and reassignment, and terminal dispatch failure.
Existing list/map/optional/result values convert through compiler-described trees,
including nested containers, pending payloads, empty values, and later insertion.
Allocation-failure tests verify cleanup of partial lists, maps, sums, and boxes;
malformed conversion descriptors fail before touching source storage.
Explicit comptime values with unambiguous checked primitive or nominal payloads
materialize as typed boxes, including mixed lists, secret fields, and pending
wrappers. Generic structs retain their instantiated identity for typed observation
and comptime materialization; other ambiguous erased payloads still fail closed.

Generic struct values must retain their instantiated type separately from their
base display name. Interface erasure must not discard secret type arguments:
`Box[secret[string]]` still redacts its field when observed as `Named`, including
after cloning, pending tasks, and explicit comptime evaluation. Construction
resolves the current invocation's type arguments rather than consulting a shared
expression-span map, which can describe a different generic instantiation.
This metadata supplies typed observation and comptime materialization; it does
not introduce structural equality or change the printed struct name.

This does **not** close the full interface gate. Still required:

- Audit remaining comptime callback specialization keys, particularly
  reflection-valued parameters and reflected-loop bindings. Generic type
  arguments and their alias reflection metadata are retained.
- Remaining comptime payload shapes and reflected interface fields. Generic
  struct metadata and exact implementation identity are retained.
- Audit primitive-width/refinement dispatch identity against interpreter runtime
  identity rather than assuming every checked TypeId has a distinct runtime name.

Generic struct interface dispatch uses the concrete instantiated owner retained
on the value. Implementations for `Box[int64]`, `Box[string]`, and
`Box[secret[string]]` must coexist without overwriting each other. Namespace
qualification and transparent aliases normalize before matching. Concrete method
aliases use the fully instantiated owner, including through a transparent type
alias such as `type IntBox = Box[int64]`. An inherent method keeps its own source
identity. A bare generic owner such as `Box.name` currently escapes checking with
an error type and must receive a source diagnostic; it is not a concrete method
identity. This checker diagnostic remains an open gate. Pending interface receivers retain the
existing failure behavior until joined.

Comptime callback materialization must reconstruct the evaluated source function
with its own checked signature before adapting it to the expression's result
type. Re-labeling the source descriptor with the target signature is invalid.
Exact signature selection remains authoritative for specialized candidates; when
erasure leaves only one source candidate, materialize that candidate and retain
the conversion explicitly. Ambiguous source identity must fail closed rather
than pick an arbitrary specialization. Generated materialization conversions
must run through the same HIR adapter pass before ownership lowering, reusing
existing adapters and processing newly generated adapter bodies.

Actor constructor parity follows the interpreter's existing two environments:
constructor parameters remain captured by the actor, while state initializers
receive independent working values. Native lowering must establish retained
copies before evaluating initializers, so an initializer may consume its working
binding without invalidating the later generated actor allocation. This is
compiler-owned capture storage, not implicit source-level copying of interfaces.

## Callback compatibility decision

The checker previously compared function parameter types covariantly: it accepted
`function(User)` as `function(Named)`. Calling the resulting value with another
implementation can expose interpreter behavior for a value that violates the
callee's checked nominal type. The user selected safe callback substitution:
compare parameters contravariantly and returns covariantly. Reject unsafe
parameter widening and accept safe narrowing. Native adapters now preserve
borrowed/owned arguments, covariant returns, captured closures, stored callbacks,
higher-order callback parameters, debug names, and pending depth. A context-owned
adapter descriptor retains the original function and invokes a generated typed
body; no interpreter or source-name lookup participates in native execution.

Whole-container callback conversions use the same generated signature adapters.
HIR retains each required source/target signature and its generated function ID
on the conversion. Reachability and verification include these functions. The
native conversion descriptor contains link-time function-address relocations;
runtime conversion wraps each callback while preserving captures, debug labels,
pending state, and allocation-failure cleanup. Nested list/map/optional/result
conversions do not perform runtime type inference or implementation lookup.

Linked differential coverage now exercises container callbacks with captures,
contravariant parameters, covariant returns, secret lifting, pending functions
and containers, empty insertion, and adapters whose own parameters need nested
container conversion. Runtime tests walk every allocation failure through list,
map, and optional callback conversion; object tests reject missing or mismatched
adapter metadata and require native data relocations to reachable adapter bodies.

Comptime callback materialization now retains an unambiguous source function's
checked signature and applies explicit adapters afterward. Linked regressions
cover named choices, captured closures, callbacks in lists and records, pending
callbacks, covariant returns, and secret parameters/results. Completing generated
conversions is idempotent and shares already emitted adapters, avoiding duplicate
native symbols when comptime evaluation recreates an existing conversion.

Generic closures must capture their lexical type bindings and the enclosing
function's concrete type arguments, including reflection-visible alias metadata.
The source body span and callable signature alone do not identify a specialization:
two instances may differ only in capture types or a type-dependent operation.
Calling a closure restores its captured type context, and materialization matches
that context against checked HIR identity before considering signature adapters.

Generic closure regressions now cover multiple concrete instantiations sharing a
body and callable signature, aliases of an existing type, captured values, empty
capture environments with type-dependent calls, nested factories, and calls from
a different generic context. Interpreter restoration also runs after argument
normalization failure. Boxing the captured type context preserves the established
stack budget for recursive JSON fixtures.
