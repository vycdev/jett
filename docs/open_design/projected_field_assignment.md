# Projected field assignment

Field assignment has no implemented execution contract. A checked probe with a
struct containing an integer and string accepts `item.value = 9` and
`item.label = "after"`. Reference execution then fails with
`only simple variable assignment is supported in comptime`, including during
ordinary runtime execution. MIR ownership planning rejects the same shape as a
nonlocal assignment requiring place ownership/materialization.

The checker also accepts a write through an immutable struct local and through
a `view` parameter. Its ownership pass enforces mutability only when the target
is an identifier; it treats other targets as ordinary expressions. Acceptance
does not establish safe mutation behavior. Neither probe changes a value in an
executing program today.

The design currently has competing statements: mutable bindings provide local
consume-and-rebind, and views are read-only; the bitfield operation table also
promises direct field assignment. Select a contract before implementing writes
in the interpreter or native runtime:

- Implement projected updates rooted in an owned mutable local, preserving
  read-only views and rejecting writes through immutable or temporary roots.
- Reject projected assignment at compile time and express changes by constructing
  a new value and rebinding an owned mutable local. Update the bitfield promise
  to match that selected language surface.

An implementation of updates additionally needs explicit rules for nested
projections, an already-consumed root, outstanding views, replacement of owned
fields, RHS evaluation order, pending values, and failure cleanup. Refinements
of an enclosing aggregate must remain valid; bitfield values must still obey
their declared widths. Do not silently bypass either validation or introduce
masking/truncation as a new assignment rule. These requirements must be resolved
before the feature can count toward native parity.

No option is selected by this note. In particular, runtime failure in both
backends is not evidence that projected updates are supported.
