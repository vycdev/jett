# Actor handle equality

Decision status: live instance identity implemented in the reference evaluator and native backend; local Debug/Release native acceptance passes. See [the active selected contract](../active/actor_identity_equality.md). The observations and alternatives below record the prior unresolved state.

## Historical divergence and alternatives

Before this implementation, the checker accepted `first == first` for actor handles, but the
interpreter rejected it at runtime as an unsupported binary operation. An enum
whose payload is an actor handle was also accepted; the interpreter's recursive
enum equality compared the actor IDs, while native aggregate enum equality
rejected that payload type. These paths needed one language rule before native
codegen can claim parity.

Two coherent choices are:

- Give actor handles identity equality. Direct `==` and `!=`, and actor values
  nested in enums and collections, compare the same live actor identity. This
  requires aligning the interpreter's direct operation with its existing
  recursive comparison and extending native aggregate equality.
- Reject actor equality, directly and when an actor occurs inside a compared
  enum or collection. This requires checker diagnostics for the nested forms
  and removal of the interpreter's incidental recursive comparison.

The decision should preserve user-defined struct equality's separate exact
`Equatable.equals` rule. Differential tests should cover the same cloned actor
handle, distinct spawned actors, nested enum and list payloads, and `!=`.

## Selected contract

Actor equality now selects live instance identity. Exact checked nominal actor types must match; runtime IDs must belong to the owning context. Cloning preserves identity, distinct spawns remain unequal even with equal state, and != negates the same comparison. State is never structurally compared. See [the selected implementation note](../active/actor_identity_equality.md). Escaping comptime actor graph import remains a separate contract.
