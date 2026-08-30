# Opaque Runtime Resource Type Contract

Status: accepted for the first compiler-shipped runtime resource implementation.
The frontend/type boundary and the generation-checked `jett_runtime` registry
substrate are implemented. Trusted-hook dispatch, interpreter control-flow
cleanup integration, sockets, HIR/MIR drop elaboration, and native-runtime
support remain pending. This document closes the design scope tracked by
[#175](https://github.com/vycdev/jett/issues/175).

## Purpose And Scope

Jett needs nominal values that own runtime objects whose representation cannot
be expressed or safely exposed in source. The first users are
`net.socket.TcpStream` and `net.socket.TcpListener`. These values must follow
ordinary move and `view` rules while also running one trusted cleanup operation
when their ownership ends.

This contract selects:

- one source declaration for a compiler-shipped opaque resource type;
- construction and operation access through private trusted kernels;
- the allowed type-level metadata and the rejected value operations;
- exact ownership, close, scope cleanup, error, cancellation, and transfer
  behavior;
- a generation-checked interpreter carrier and capability-provenance boundary;
- the required HIR, MIR, and native-runtime handoff.

It does not implement sockets, add user-defined destructors, expose raw
pointers or descriptors, define a public resource registry, or merge C FFI
pointer semantics into the general runtime-resource model.

## Canonical Source Declaration

A compiler-shipped source file declares a resource with the `resource` keyword:

```jett
namespace net.socket

export resource TcpStream
export resource TcpListener
```

The canonical grammar is:

```text
resource_decl := ["export"] "resource" IDENT NEWLINE
```

A resource declaration has no body, fields, variants, constructor, generic
parameters, inheritance clause, cleanup expression, runtime symbol string, or
user-selected representation. Adding a colon or body is an error rather than a
second spelling.

The declaration follows the ordinary top-level rules:

- its name uses strict PascalCase;
- it belongs to the current namespace and is private unless marked `export`;
- it becomes visible only after its declaration;
- it participates in ordinary duplicate and namespace collision checks;
- it cannot appear in `mutual:`, a function, a type, an interface, an
  implementation, a foreign block, or another declaration.

Only a compiler-shipped stdlib source may declare `resource`. Project and
vendored dependency files may name an exported resource but cannot declare one.
This is a conservative first boundary, not a reserved path to general
user-defined destructors. If user-defined resources are ever needed, their
construction, cleanup authority, package trust, and backend contract require a
separate design.

The parser and checked program retain a distinct resource declaration and type
kind. A resource must never be represented as a fieldless struct because that
would make ordinary construction, struct reflection, and struct interface rules
plausibly applicable.

## Trusted Construction And Operations

Ordinary source has no expression that constructs a resource. An owned value
can enter source execution only as the successful result of a checked private
trusted kernel associated with a compiler-shipped resource declaration.

Public names and signatures remain in `.jett` source. For example,
`net.socket.connect` is a normal source-owned wrapper returning `TcpStream`;
the private kernel performs host work and creates the carrier. Project code
cannot import, call, shadow, or spoof the kernel.

Trusted dispatch uses declaration identity, not a qualified-name string:

1. the stdlib loader proves compiler-shipped origin;
2. resolution binds a private kernel declaration to a checked definition;
3. checking validates the kernel's resource argument and result modes;
4. the checked program records a backend-neutral trusted-hook identity;
5. the interpreter or backend dispatches that identity.

Matching `net.socket.TcpStream`, `__connect`, or another spelling is never enough
to obtain trusted behavior. The broader stdlib origin and module policy remains
tracked by [#3](https://github.com/vycdev/jett/issues/3), but resource support
must not introduce a new public-name dispatch exception while that work is
pending.

A construction hook either returns one fresh owned carrier or returns its
ordinary domain failure without creating a live source value. A kernel must not
publish a handle and then report failure. Resource operations state ownership in
normal signatures:

- `view resource: Resource` borrows without taking cleanup responsibility;
- `resource: Resource` transfers ownership to the callee;
- a returned `Resource` is a fresh owner transferred to the caller.

There is no hidden borrowed-return form. A view cannot escape through a return,
field, collection, actor message, or task result under the existing ownership
rules.

## Type And Value Restrictions

A resource is nominal, move-only, and opaque. By default:

- no literal, constructor, field access, destructuring, pattern payload, or
  refinement can expose or create its representation;
- `clone` is rejected;
- `==`, `!=`, ordering, and hashing are rejected;
- a resource cannot implement `Equatable`, `Orderable`, `Displayable`,
  `Serializable`, or another user interface;
- JSON, binary serialization, string interpolation, ordinary printing, and
  declassification-style extraction are rejected;
- `type.construct_start` and every reflected construction path reject it;
- aggregate reflection returns no fields, variants, state, layout, or value
  access;
- property generation and comptime value evaluation cannot produce or inspect a
  resource.

Type-level reflection remains total and may reveal only the canonical qualified
type name and a distinct `TypeKind.resource_type` tag. This is enough for generic
code to reject or route around resources without exposing carrier, authority,
open/closed state, drop hook, or backend layout. `type.fields[T]()` and other
shape probes return empty metadata for a resource. No value-reflection operation
accepts one.

Debugger and diagnostic surfaces may report the declared type and ownership
availability (`owned`, `view`, or `consumed`). They must not render a slot,
generation, host descriptor, endpoint hidden in the carrier, capability
provenance, or provider state.

A resource is not implicitly copyable when its carrier happens to fit in a
machine word. Bitwise carrier copies are a backend defect unless they implement
one semantic ownership transfer and invalidate the source owner.

## Ownership And Exactly-Once Cleanup

Resource values use Jett's existing `Owned`, `Viewed`, and `Consumed` states.
They add a compiler-known cleanup obligation to each live owner.

The rules are:

1. Moving a resource transfers its single cleanup obligation and consumes the
   source binding.
2. A `view` borrows the live owner and never gains cleanup responsibility.
3. An explicit close operation consumes the owner and invokes the same trusted
   finalizer used by implicit cleanup.
4. Leaving a scope with a live owner invokes its finalizer exactly once.
5. Return, `handle` propagation, loop exit, and other control-flow edges drop
   every live local they exit in reverse acquisition order.
6. Moving, explicitly closing, returning, or sending the value removes the
   former binding's cleanup obligation, so that binding is not dropped again.
7. Cleanup for `optional[Resource]`, `result[Resource, E]`, enums, structs, and
   collections descends only into the live owned resource values selected by
   the container's ordinary shape.

A source-level explicit close is preferred when lifetime matters, but scope
cleanup is mandatory rather than a leak fallback. Explicit close is not a
separate user destructor protocol. The operation consumes the handle even if a
host close reports a late platform error. The trusted finalizer is infallible to
source, non-suspending, and non-cancellable; a backend may record an internal
close failure in debug telemetry but cannot restore a consumed owner or expose a
portable retry result.

The runtime makes finalization defensively idempotent, but the source type system
does not expose idempotent close. A second close or use is rejected as use after
consume. Runtime idempotence protects against backend cleanup bugs and
cancellation races; it is not permission to duplicate owners.

Process abort, forced host termination, and hardware failure are outside the
exactly-once language guarantee. All normal Jett control flow, handled domain
errors, task cancellation, actor shutdown, and runtime-context teardown are
inside it.

## Errors, Pending Operations, And Cancellation

A borrowed operation that returns a domain error leaves the resource live and
owned by its existing owner unless the public operation contract explicitly
consumes it. A consuming operation documents whether successful completion
transfers or retires the resource; it cannot silently return ownership through
an error value.

Cancellation is observed at trusted capability-operation checkpoints:

- cancellation before dispatch starts no host operation and changes no resource;
- cancellation of a pending borrowed operation unregisters its readiness or
  provider work before `join` reports `CancelledError`;
- cancelling borrowed work never closes the caller's resource;
- if the cancelled task owns a resource, task unwinding finalizes it after
  pending work is detached and before `join` completes;
- finalizers themselves cannot be interrupted by another cancellation check.

A provider callback must not access a carrier after cancellation has detached
that operation or after finalization has retired its registry entry. The
operation state and registry generation jointly reject late completions.

The initial socket rule allowing at most one pending operation per handle is a
socket contract, not a universal property of every resource type. The generic
resource carrier supports an operation state, while each trusted provider
defines which overlaps are accepted and reports its documented domain failure
when they are not.

## Actor And Task Transfer

Passing a resource without `view` to a task or actor transfers the single owner.
The queue or task frame owns the cleanup obligation while the value is in
transit. Delivery transfers that obligation again; dropping an undelivered
message or cancelled task finalizes the value.

Resources cannot be cloned for fan-out. A future resource may expose an explicit
source operation that creates a second independently owned runtime object, but
that operation is ordinary effectful behavior and not `clone`.

A task may borrow a resource with `view` under the existing structured
concurrency rule. The parent owner remains borrowed until `join` or `cancel` and
cannot move, close, or rebind the value during that interval. Actor messages
cannot contain views.

Transfer preserves resource type, registry identity, open/closed state, pending
operation state, and authority provenance. It does not duplicate a host handle
or create new authority. Cross-process serialization and transfer between
independent runtime contexts are unsupported.

## Interpreter Carrier And Stale-Handle Defense

The interpreter stores resources in a runtime-context-owned registry. A source
value carries only an unforgeable internal key equivalent to:

```text
ResourceKey {
    context_id,
    resource_type_id,
    slot,
    generation,
}
```

The addressed registry entry contains:

```text
ResourceEntry {
    resource_type_id,
    generation,
    lifecycle_state,
    provider_payload,
    authority_provenance,
    finalizer_hook,
}
```

These are implementation records, not source-visible structs or stable ABI.
Every operation validates the context, type, occupied slot, generation, and live
state before reaching a provider. Finalization marks the entry closing, detaches
pending work, runs the finalizer once, retires the slot, and advances its
generation before that slot can be reused. A late callback or stale copied key
therefore cannot operate on a newer resource in the same slot.

An impossible stale, wrong-context, wrong-type, or already-retired carrier is a
stable runtime contract failure, not a socket `SocketError` and not arbitrary
provider behavior. Ordinary checked source cannot trigger it; deterministic
harness tests must be able to inject malformed internal keys to verify the
boundary.

The registry is owned by one run context. Context teardown finalizes every live
entry in deterministic reverse-creation order after stopping new dispatch and
detaching pending work. Tests create isolated registries so one test's handles,
slots, fake provider, or cleanup log cannot affect another.

## Capability And Authority Provenance

A resource identifies one runtime object. It is not a capability and does not
grant ambient permission to create or operate another object. Public operations
still require every capability parameter selected by their module contract.

When construction depends on narrowed authority, the registry entry records a
backend-neutral provenance identity and the relevant restriction snapshot. A
later operation validates the supplied capability against that provenance
before host work. Moving a resource preserves the metadata exactly. It neither
widens authority nor stores a source-usable clone of the capability.

Cleanup is different from ordinary operation authority. The finalizer may use
the registry's internal provider state to release the object after the source
capability is unavailable. Otherwise scope cleanup could leak merely because a
capability owner moved first. This internal release permission authorizes only
finalization; it cannot be recovered as `Network`, used for new I/O, or passed to
source.

Capabilities remain a separate built-in family. They are injected into `main`,
may support explicit authority-preserving clones for actor handoff, and may be
narrowed. A resource is returned by an operation, cannot be cloned, and owns one
specific cleanup obligation. This contract does not reclassify capabilities as
`resource` declarations.

## Relationship To C FFI Opaque Pointers

The C FFI `opaque pointer` declaration remains distinct:

- it is generated dependency syntax tied to one C target, ABI spelling, policy
  digest, pointer carrier, and named C release function;
- a stdlib `resource` is compiler-shipped source tied to a backend-neutral
  trusted provider and has no source-visible pointer or ABI metadata;
- foreign calls require `Foreign`; a runtime resource operation requires the
  capability selected by its public module, such as `Network`;
- FFI policy may transfer ownership to or from C, while a runtime resource stays
  inside the Jett runtime's provider boundary.

Both use move/view analysis and exactly-once drop elaboration. Their declaration
syntax, trust source, carrier validation, authority, and backend lowering must
not be unified merely because both are opaque and linear.

## HIR, MIR, And Native Runtime Handoff

HIR must retain the nominal resource type, trusted-hook identity, ownership mode,
and capability operands. It must not lower a resource to a fieldless aggregate
or ordinary integer.

MIR adds an explicit resource drop obligation. Drop elaboration inserts one
finalizer on every edge where a live owner dies, including fallthrough, return,
handled error propagation, loop exit, cancellation unwind, actor message drop,
and runtime shutdown. Move dataflow suppresses the source drop after transfer.
Borrowed places never receive drops. Verification rejects a path with a leaked
owner, a double drop, or a use after a consuming close.

A native carrier may be a pointer, index, platform handle wrapper, or another
backend representation. It must preserve nominal type checks, non-forgeability,
exactly-once cleanup, pending-operation cancellation, authority provenance, and
stale-handle protection. Raw descriptors and provider pointers never become
part of the Jett ABI or reflection surface.

Interpreter, bytecode, and native conformance tests use the same scripted
provider scenarios and cleanup log. Backend-specific loopback or OS tests are
additional coverage, not a substitute for deterministic lifecycle tests.

## Deterministic Verification Contract

A fake provider records construction, operation start/completion/cancellation,
and finalization events without opening a host resource. Required scenarios
include:

- fresh construction and one normal explicit close;
- implicit cleanup on fallthrough, return, handled failure, and loop exit;
- reverse-order cleanup for multiple owners;
- move, return, task transfer, actor-message transfer, and dropped messages;
- view borrow without cleanup transfer;
- use-after-move and clone rejection at checking time;
- cancellation of borrowed work without close;
- cancellation of owned work with one close before `join` completes;
- late completion after cancellation;
- explicit close followed by scope exit without a second finalizer;
- stale generation, wrong type, and wrong runtime-context rejection;
- authority-provenance match, mismatch, and preservation across transfer;
- empty type/shape reflection and rejection by construction/serialization paths;
- isolation and deterministic teardown across independent run contexts.

Socket implementation tests add endpoint, DNS, partial I/O, timeout, and
`Network.allow` behavior from the socket contract. Those do not replace the
generic ownership and registry matrix above.

## Implementation Stages

1. **Frontend and type boundary:** reserve `resource`, parse and format the sole
   declaration form, enforce compiler-shipped origin/order/visibility/name
   rules, add the resource type kind, and reject construction, cloning,
   interfaces, equality, hashing, serialization, value reflection, and
   comptime/property values.
2. **Checked trusted-hook boundary:** resolve stdlib-origin private kernels to
   typed hook identities and preserve resource ownership modes without public
   qualified-name dispatch.
3. **Interpreter registry:** add generation-checked carriers, fake-provider
   injection, explicit close, scope/error cleanup, stale-handle failures,
   provenance validation, and isolated context teardown.
4. **Task and actor lifecycle:** transfer cleanup obligations through task frames
   and messages; pin borrowed cancellation, owned cancellation, undelivered
   message, and late-completion behavior.
5. **Socket implementation:** add source-owned `net.socket` declarations and the
   fake provider before host TCP adapters, following the completed socket
   behavior contract.
6. **HIR/MIR/native handoff:** preserve resource operations and elaborate drops
   on the CFG before any native socket API is exposed.

No stage may expose a public Rust builtin for `TcpStream` or `TcpListener`, treat
a fieldless struct as opaque, rely only on a qualified-name match, or claim
exactly-once cleanup without the relevant control-flow and cancellation tests.
