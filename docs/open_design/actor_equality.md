# Actor handle equality

The checker currently accepts `first == first` for actor handles, but the
interpreter rejects it at runtime as an unsupported binary operation. An enum
whose payload is an actor handle is also accepted; the interpreter's recursive
enum equality compares the actor IDs, while native aggregate enum equality
rejects that payload type. These paths need one language rule before native
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
