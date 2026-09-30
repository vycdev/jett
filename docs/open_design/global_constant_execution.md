# Global constant execution

Global constants are permitted by the design and frontend, subject to declaration
order and namespace isolation. Neither execution handoff implements them today.
Mutable namespace-level declarations are forbidden and now report E0377 before
either execution handoff. Local mutable bindings are unaffected.
A same-namespace `int64 answer = 42` followed by an entry point reading `answer`
passes checking but fails reference execution with `undefined variable 'answer'`.
The interpreter registers declarations without initializing top-level variables,
and HIR accepts identifier values only for locals or checked functions.

The design already selects immutable compile-time constants baked into the
binary. It does not authorize runtime initialization or shared global runtime
storage. Implement the documented constant subset first, preserving declaration
order and namespace isolation. Both execution handoffs must receive the same
checked constant value rather than evaluate an initializer at program startup.

The frontend must enforce the documented compile-time constant initializer
restriction. Literal initializers are unambiguous. Any extension to ordinary
pure calls must be reconciled with the explicit `comptime expression` rule:
optimization must not change validity, and ordinary pure calls must not silently
acquire a required compile-time evaluation boundary. This note does not select
such an extension.

Move-only constant values still need a precise read and ownership rule before
execution support is extended to them. Do not implicitly clone a move-only
constant on every read, silently consume shared storage, or introduce mutable
references. Opaque resources, actors, pending tasks, and erased payloads require
separate scrutiny for hidden state or authority. Callable constants must retain
declaration context and their permitted captures.

The compile-time-only contract is selected by `docs/design.md`; this note records
the missing implementation and remaining subset questions. Native parity cannot
be claimed for constant reads based on successful checking alone.
