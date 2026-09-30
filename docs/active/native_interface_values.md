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

- Resolve equality of erased interface values; the checker accepts it without
  a uniform runtime contract. See
  [interface value equality](../open_design/interface_value_equality.md).
- Resolve erased calls whose non-receiver arguments require the implementation's
  concrete owner. See
  [same-owner arguments](../open_design/interface_same_owner_arguments.md).
- Audit interpreter source instances of compiler-owned facades that use
  different native helpers and lack checked source bodies.
- Audit remaining comptime combinations; reflected interface fields and lists
  now have record, enum, and machine round-trip coverage.
- Resolve actor handles escaping comptime evaluation; a saved interpreter actor
  ID currently has no corresponding runtime instance. See
  [comptime actor values](../open_design/comptime_actor_values.md).
- Audit refinement compositions beyond the covered primitive, list, map, set,
  optional, result, record, enum, machine, bitfield, function, builder, and
  capability bases.

Generic struct interface dispatch uses the concrete instantiated owner retained
on the value. Implementations for `Box[int64]`, `Box[string]`, and
`Box[secret[string]]` must coexist without overwriting each other. Namespace
qualification and transparent aliases normalize before matching. Concrete method
aliases use the fully instantiated owner, including through a transparent type
alias such as `type IntBox = Box[int64]`. An inherent method keeps its own source
identity. A bare generic owner such as `Box.name` has no instantiated type and must
receive the same missing-type-arguments diagnostic as other incomplete generic
types. Use a concrete type alias for a method call or value; inferring the owner
from a receiver would introduce a new inference rule. The checker must reject
this source before HIR lowering instead of silently returning an error type. Pending interface receivers retain the
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
normalization failure. Boxing the captured type context and separating actor and
closure construction helpers preserve the stack budget for recursive JSON fixtures.

Named function invocation resolves explicit type arguments in the caller, then
executes in the callee's own type-parameter scope. A non-generic function sees
its declaration's nominal types even when a generic caller uses the same type
parameter name. Closures created by that callee capture only its lexical type
bindings. Return and argument failures must restore the caller's type scope.

Interpreter arithmetic normalization must use the checked expression map for the
active generic specialization. The driver must retain those maps and their type
argument, alias-reflection, and reflection-parameter identities instead of
collapsing all instantiations into one span map. A named call selects its checked
map before evaluation; a closure retains that map with its lexical context.
Calls and failures restore the caller's map. Matching must reject conflicting
checked identities rather than select whichever specialization was registered
last. Reflected loop body expansions still require their own scoped selection.

The `generic_integer_wrapping` linked regression now covers signed and unsigned
closure arithmetic, named generic calls, and explicit comptime results. Checked
maps survive closure capture; argument failures restore the caller's map.
Generic spans are absent from the global fallback, including spans checked for
only one concrete instance. Compiler-owned facade instances without matching
source facts retain source interpretation without borrowing another instance's
map. Conflicting matching maps are rejected in either registration order.

Comptime closures retain the complete selected checked function context through
shared ownership. Native materialization compares reflection-parameter facts as
well as type arguments and alias metadata. The linked
`comptime_reflection_callbacks` fixture distinguishes closures sharing one body,
signature, and type arguments using `TypeKind`, `TypePrimitive`, and both kind
and primitive facts from `TypeInfo` parameters. Captured ordinary values survive
selection, and moving a reflection parameter before closure creation is valid.

Every concrete generic invocation is now checked regardless of where reflection
expressions occur. Interpolation, nested call arguments, and runtime or baked
closures retain checked bodies for native lowering. The user selected a
compile-time error for invalid `comptime type` bindings behind runtime guards;
`generic_reflection_runtime_guard_deferral` is now a compile-fail fixture.
`generic_reflection_body_type_error` rejects a mismatched generic return type,
and `generic_reflection_expressions` compares interpreter and native output.

Reflected-loop callbacks now carry their lexical `comptime type` binding chain
through inline HIR identity, evaluated closures, and native symbol encoding.
Previously a list of baked callbacks returning `type.name[Field]()` for different
record fields failed native materialization as ambiguous. Named calls start
their own lexical chain; nested closures retain their defining chain. The
`comptime_reflected_callbacks` differential fixture covers runtime and baked
callbacks, repeated field types, a nested direct binding inside a generic
reflected loop, and callbacks that produce another callback.

Scoped expression facts now follow the checker's recursive `comptime type`
body snapshots. Each binding selects facts for its concrete type; nested
bindings remain under their parent, and flat fallback maps exclude their
expression spans. Closures retain the selected scope through shared ownership,
while named calls isolate it and restore it on return or failure. Conflicting
matching scope facts fail rather than choosing an expansion by registration
order. `reflected_integer_wrapping` checks signed and unsigned field widths
through ordinary and generic functions, nested bindings, and runtime or baked
callbacks, including repeated field types.

The scoped alias audit found and fixed native dispatch merging `Label` (an
alias of `string`) with `string`: it printed `alias`, `alias` instead of
`alias`, `primitive`. Checked bindings now export their source-visible
reflection metadata. Scoped expression selection, callback materialization,
native symbols, and runtime dispatch preserve that metadata. The dedicated
alias-preserving reflection match leaf keeps the earlier alias-transparent
identity operation available. Argument-loop facts retain alias metadata from
direct `TypeInfo` sources, fields, and other argument loops.
`reflected_alias_callbacks` exercises runtime and baked callbacks, repeated
aliases, distinct aliases of the same base, nested container aliases, direct
`type.arg` bindings, and nested argument sources.

The alias regression also covers a generic callee receiving a scoped
`list[Small]` argument where `Small` aliases `int8`. Nested source spellings
may have no global reflection-table entry, so argument selection now retains
metadata from the lexical scope or from the interpreter's existing type
reflection representation. This prevents a missing callee context from
evaluating `127 + 1` as 128 instead of -128.

Settled reflection guard policy: a valid `comptime type` binding is allowed
under an unknown runtime guard when every branch typechecks. An invalid binding
still fails at compile time, regardless of the runtime condition. Unknown tags,
predicate calls, and detached boolean locals do not provide type evidence.
The former `generic_reflection_guarded_unknown_fact` and
`generic_reflection_match_unknown_fact` negative fixtures become compile-pass
fixtures; runtime coverage exercises both outcomes of each guard.

Known reflection branch selection also applies inside closures and after
interpolation or other reflection expressions. The checker no longer uses a
whole-body placement classifier to decide whether these facts are available.
`closure_reflection_guards` compares runtime and baked callbacks with scalar
and list instantiations. `closure_reflection_boolean_boundaries` keeps
predicate-call and detached-bool casts rejected inside callbacks. The
`generic_reflection_unknown_guard_invalid_binding` negative fixture checks
invalid lookups in unknown match arms and closures, complementing the existing
runtime-if regression.

Primitive interface identity implementation: the interpreter must retain the
checked concrete type when a primitive carrier would erase it (integer widths,
float32, and nominal refinements). This is value metadata, preserved by cloning,
containers, pending results, captures, and comptime constants. Primitive
operations inspect the payload; interface dispatch and comptime materialization
inspect the retained identity. Transparent aliases canonicalize to their base,
and an interface-typed boundary must preserve the incoming concrete identity.
Typed debug output must keep recursive secret redaction. This restores the
already checked dispatch contract without changing source typing or arithmetic.

The primitive identity regression now covers every signed and unsigned width,
float32 versus float64, nominal integer/string refinements, and a transparent
primitive alias in a namespace. Runtime and baked lists, literal element types,
callbacks, cloning, pending values, list/optional/map conversions, and reflected
interface fields retain the checked identity. Native materialization resolves
that retained name to a unique checked type instead of guessing from the carrier.
Primitive operations borrow the payload; container insertion and cloning keep
the whole value. The broader fixture suite caught and pinned byte-list consumers
that also need to inspect their element payloads.

Collection identity implementation follows the same checked contract as the
primitive fix: implementation owners use the full canonical type, and erased
collections retain that identity. Concrete collection conversions retag the
outer value for the destination type while preserving element identities;
interface boundaries preserve the incoming identity. Register concrete owners
explicitly so unrelated runtime collections need no extra wrapper. Collection
operations and sum/aggregate projections inspect payload storage without changing
stored element values. Non-primitive refinements with interface implementations
must retain their nominal identity by the same mechanism.

`interface_collection_identity` now covers distinct list/map/set/optional/result
implementations, empty and failed sums, list and record refinements, nested
collection conversion, typed insertion, loops, sorting, alias methods and method
values, reflected collection fields, callbacks, cloning, pending values, and
baked values. Secret element types survive erasure and remain redacted.

The expanded regression exposed two checker handoff gaps. A namespace-local
alias method must resolve its canonical owner before recording its source
method target; otherwise valid calls and method values reach HIR without their
checked target. A `run` initializer must preserve its expected type context;
otherwise `run ok(17)` leaves a local with `result[int64, <never>]` while later
uses carry the declared `result[int64, string]`. Dedicated checker tests pin
both behaviors, including nested pending values, empty containers, and narrow
integer literals. Interpreter registration also retains interface declarations
for namespace qualification inside canonical collection type arguments.

Function interface identity follows the checked function signature, including
parameter view modes and the result type. Runtime and baked callbacks must retain
that signature across erasure without losing their evaluated source declaration,
captures, or generic context. A checked function conversion changes the exposed
signature while preserving the source body used by the adapter. Calling the
value unwraps identity metadata and invokes that body; interface dispatch uses
the retained signature. Canonical type parsing must preserve nested function and
container arguments rather than treating a whole signature as an identifier.

`interface_function_identity` covers distinct scalar widths, string and list
returns, owned and view parameters, nested function parameters, named functions,
captured closures, and generic closure specializations. Runtime and baked mixed
interface lists, cloning, pending functions, reflected function fields, and
contravariant parameter/covariant return adapters retain the exposed signature
and evaluated source body. Its native stdout and trace output match the
interpreter, including explicit comptime erasure after a callback conversion.

Machine interface identity must distinguish the checked bare machine type from
each state-qualified type when both have implementations. A value checked as
`Session at active` dispatches to that implementation; converting it to `Session`
uses the bare implementation. Keep the actual state and fields in the payload,
and preserve the exposed owner through erasure and explicit comptime evaluation.
The interpreter retains the qualification so a state-qualified value selects
its own implementation.
Checked flow narrowing inside a state guard must also supply the exposed state
type even when the binding was declared with the bare machine type. Native
materialization must construct the actual state and then bind it at the requested
bare type before wrapping a refinement or interface; returning a state-typed
constructor in place of the requested bare value changes dispatch and violates
the refinement base contract.

`interface_nominal_identity` covers bare machines and distinct state-qualified
implementations, aliases, transitions, guarded narrowing, pending values,
reflected state-qualified fields, and enum/machine/bitfield refinements. Runtime
and baked mixed interface lists retain the same dispatch and recursive secret
redaction. Materialized bare machines use an explicit typed local after state
construction, preserving both the requested owner and validated refinement base.

Explicit comptime construction builders are accepted pure values. Native
materialization must recreate the evaluated owner, selected variant or state,
provided field order, and stored payloads from checked reflection metadata. It
must retain incomplete builders and defer refinement/width validation until the
same runtime finish boundary as an ordinary builder. Reconstruction may use the
existing typed builder intrinsics, but must not execute the source computation
or invent layouts from source spellings. Nested, pending, and erased builders
must retain the same ownership and recursive secret redaction contracts.

`comptime_builders` covers empty and partially filled records, generic and
alias-bearing owners, nested builders, variant/state builders, bitfields,
cloning, lists, stored values, pending and erased builders, and provided fields
in a different order from the declaration. Missing-field, refinement, and width
errors remain deferred to runtime finish. Source reflection operands are
exported separately from canonical metadata; equivalent aliases compare by
their checked type ID without discarding the builder's source owner name.

Generated locals holding baked bare-machine values are constant seeds. Each
evaluation clones its seed so an expression inside a runtime loop creates an
independent owned value on every iteration. Moving the seed consumed it after
the first iteration and incorrectly failed native ownership validation.

The facade identity audit includes JSON parsing and round trips through numeric,
collection, refinement, generic record, enum, bitfield, and machine helpers, plus
the compiler-owned math facades. Its nested explicit comptime calls expose a
caller-stack dependency: native compilation on a normal Rust test thread can
overflow even when the same program builds from the CLI. Explicit evaluation
must use the existing 8 MiB interpreter stack budget on its own scoped worker,
without increasing that budget or moving any evaluation to runtime. Modules
without explicit comptime expressions need no evaluation worker.

`interface_facade_identity` now matches native stdout and recursive secret
redaction for runtime and baked JSON parser results, enum/bitfield/machine JSON
round trips, and math facade results. It passes from the ordinary native test
thread after explicit evaluation adopts the shared interpreter stack budget.

The `interface_math_facade_contexts` probe covers both allowed numeric owners
for every abs/min/max facade, secret taint from either operand, generic wrappers,
captured public-value callbacks, entirely baked collections, and baked callbacks
called at runtime. All preserve integer/fraction/secret interface dispatch in
linked debug and release artifacts after source removal. This probe exposed an
independent MIR capacity error: owned indirect-call results were omitted from
temporary planning. The planner now reserves each result in addition to the
callee descriptor and its arguments. A companion matrix covers the other owned
return families and terminal failure after earlier callbacks produced owners.

Opaque carriers must also retain their checked concrete owners when erased to
interfaces. Distinct actor declarations and refinements of `TypeConstruction`
already dispatch independently in native code. Interpreter metadata must preserve
those owners while actor messages and reflected construction operate on the
underlying handle or builder payload. Converting a refined builder to its base
selects the base implementation; erasure preserves the exposed refinement.
Capability-backed refinements use the same checked owner rule. Explicit comptime
builders nested inside a validated refinement still need their reconstruction
handlers extracted into MIR control flow; the validated wrapper must not hide
those handlers from lowering.

`interface_opaque_identity` exercises distinct actor declarations, cloned and
stored actor references, messages after annotation, pending actors and refined
builders, runtime and baked refined builder erasure, and capability refinements.
Builder completion explicitly coarsens the refinement as required by the checked
intrinsic signature. MIR extracts reconstruction handlers through
`RefinementValidated` so baked refined builders remain ordinary native values.

Machine state field types follow ordinary declaration-site name resolution,
including namespace qualification, visibility, and declaration order. The
resolver must visit them just as it visits struct and enum fields; otherwise
an interface field accepted at the root fails inside a namespace before native
lowering. Reflected fields preserve erased owners through builder round trips,
including stored lists, secrets, and explicit comptime construction.

`interface_reflected_fields` checks direct interface fields and interface lists
in records, enum payloads, and machine states. It compares ordinary round trips,
fully baked results, and baked builders completed at runtime; all preserve
integer-width dispatch and recursively redact secret elements. Resolver tests
also pin nested field type qualification and reject private or forward field
type references.

Function-backed refinements retain their nominal interface owner just like
collection-backed refinements. Alias-base classification can report the simple
carrier name `function`; that carrier must be treated as concrete, while the
value metadata keeps the full exposed refinement name. Coarsening restores the
base function signature and its implementation without changing the callable
body or captures.

`interface_refined_containers` covers nominal map, set, optional, result, and
function owners, including both sum branches, named and captured functions,
cloning, pending callbacks, and coarsening back to the base function signature.
Runtime and baked heterogeneous interface lists match in calls and traces.

`interface_facade_results` extends the facade audit to erased result values on
success and failure paths. Typed JSON parsing covers numeric widths, refinement
failure, secret-bearing records, field errors, and exact unknown-field checks;
raw JSON access covers malformed input, wrong-kind errors, missing items, and
successful absent optionals. Runtime and baked lists retain their full result
owner even when the active payload is only an error string, and traces preserve
recursive secret redaction.

`reflected_opaque_fields` checks plain and doubly pending actor/capability fields
through typed field reads and builder reconstruction. Messages through rebuilt
actor references still update the original actor, and joined capabilities retain
the authority required by their provider. Exact stdout and debug comparisons
also require reconstruction to succeed; failures cannot return the original
record as an unnoticed fallback.

The refinement audit found that an interface-backed refinement loses its
nominal method owner. `type Selected = Named where true` with its own `Named`
implementation dispatches as the underlying concrete value after erasure;
the interpreter also loses the owner on direct calls. The established nominal
refinement rule requires preserving both layers: exposed `Selected` dispatches
to its implementation, while explicit `coarsen` restores the original erased
`Named` value and that value's concrete implementation.

Interface-backed refinements use the interface handle as their underlying
representation. Erasing one must box that handle with the refinement identity;
dispatch must unbox it before entering the refinement method. Equal underlying
representations alone cannot justify omitting those conversions. Interpreter
type metadata must retain the underlying concrete identity instead of flattening
both layers, including through comptime values, collections, and pending tasks.

The sum-value regression also exposes a checker ordering problem: when both a
sum and its payload implement the expected interface, the checker requests a
payload `handle` before checking whether the whole sum already satisfies the
destination. Existing whole-value compatibility must take precedence. Erasing
that value preserves the sum and its implementation; only explicit `handle`
extracts its active payload.

`interface_refined_interfaces` now covers direct and erased dispatch, intermediate
and fully coarsened ancestors, narrow primitive and secret-bearing record
payloads, sums whose payloads also implement the interface, whole-container
conversions, signature adapters, nested pending tasks, and runtime/baked values.
Its predicate calls the base interface implementation, and rejection cases
exercise both ordinary values and an unvalidated pending constructor field.

Fresh constructor inputs keep their ancestor type until MIR runs the checked
predicate chain. Interpreter normalization similarly attaches the destination
identity after validation. Already validated pending values retain their
invariant; a new pending value cannot acquire that invariant from a type label.

Validation for this change passed 616 compiler-phase tests, all 583 frontend
fixtures, 30 native interface/comptime/refinement tests, and workspace
documentation tests. Actor handles escaping comptime and the remaining semantic
audit still prevent a full native parity claim.

`interface_refined_actors` covers actor-backed refinements and inherited
refinements with distinct interface implementations. Cloning, coarsening, direct
and reflected record storage, and nested pending handles preserve the original
actor's mutable state. Erased pending handles retain the exposed refinement
for dispatch after joining, while a fresh pending actor still fails a rejected
refinement constructor. A scalar-returning computation uses a refined actor
internally at runtime and comptime; no actor handle escapes evaluation.
Exact stdout and traces match, including actor identity through reconstruction.

`interface_display_contexts` checks the exact display contract of interface
values against their concrete records, including namespaced implementations,
generic and reflected scoped types, fields, returned values, and comptime
formatting. A side-effecting receiver is evaluated once, view operands remain
usable, and explicit primitive display implementations override the default
formatter. See [display contexts](native_display_context.md).

`interface_method_returns` checks dynamic methods returning interface values,
interface lists, and captured callbacks returning interfaces. Distinct string,
`int64`, and `int8` implementations retain their method owners, including
wrapping narrow arithmetic inside a returned collection. Callback lists created
at runtime and comptime preserve captures after the source receiver's scope
ends. The accepted method signatures explicitly return the interface types;
substituting concrete return types is rejected by the existing checker.

Property generation uses the same concrete generic owner metadata as ordinary
record construction. A generated `list[Boxed[Tiny]]`, where `Tiny` aliases `int8`,
must dispatch through the `Boxed[int8]` implementation after interface erasure.
Shrinking a field preserves the record's instantiated identity. The
`property_generic_owners` fixture and linked replay test distinguish narrow and
string owners and require shrinking to preserve successful dispatch before the
property's intended length assertion fails.

`interface_refined_small_values` completes linked coverage for `nothing`,
inherited unit refinements, boolean refinements, and byte-backed refinements.
Runtime and baked heterogeneous lists select each nominal implementation;
doubly pending unit values preserve their refinement through interface joins.
Coarsening and declassification retain borrowed byte storage through direct
calls, callbacks, field projections, and handlers in later arguments. Debug and
release binaries match the interpreter after source removal. A companion gate
checks terminal failure in a later argument for both call forms and wrapper
kinds, requiring exit 71 and exact diagnostics after owned-value cleanup.
