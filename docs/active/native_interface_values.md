# Native interface values

The full native parity audit found an accepted runtime surface absent from the
fixture inventory: a concrete implementation can be stored in an interface
binding, returned as an interface, passed to an interface parameter, and mixed
with other implementations in a `list[Interface]`. The interpreter dispatches
an interface-qualified method using the concrete runtime value. The current
native path handles concrete interface calls but rejects these dynamic calls.
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
4. Preserve the current copy/view behavior of interface bindings and release
   every owner on overwrites, failures, and normal exits. Debug observations must
   format the concrete payload with recursive secret redaction.
5. Add linked differential coverage for multiple implementations, parameters,
   returns, stored values, collections, callbacks, clone/view behavior, pending
   values, and cleanup. Only then close this release gate.

Clean host packaging checks run independently while this semantic gap is open.
