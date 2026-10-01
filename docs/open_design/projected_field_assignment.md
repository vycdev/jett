# Projected field assignment

Field assignment has no implemented execution contract. A checked probe with a
owned mutable struct containing an integer and string accepts `item.value = 9` and
`item.label = "after"`. Reference execution then fails with
`only simple variable assignment is supported in comptime`, including during
ordinary runtime execution. MIR ownership planning rejects the same shape as a
nonlocal assignment requiring place ownership/materialization.

The checker enforces the prerequisites shared by both possible contracts:
projected writes through immutable bindings or temporary values report E0404,
and writes through known views report E0401. A mutable flag cannot authorize a
write through a view. These checks also apply to actor handlers, inline
functions, explicit comptime evaluation, verify blocks, and property bodies.

Assignment facts use resolved declaration identity, so an expired mutable local
does not authorize a later immutable binding with the same name. Explicit view
initializers and aliases of known move-only views preserve read-only status.
Ordinary field initializers and clones produce owned copies in the current
implementation; the checker does not treat a fresh field copy as an assignment
to its parent. These facts enforce assignment prerequisites, not a complete
borrow-provenance analysis.
The facts do not infer borrowing from calls or returned values, or interpret a
named alias whose declaration wraps a view type. Those ownership combinations
require a separate audit before extending projected updates.

Rebinding an owned mutable local, including a parenthesized target, from a known
move-only view also reports
E0401; an explicit clone is required. Otherwise an assignment could erase the
read-only fact established at declaration and later appear to authorize a field
write. This check follows the existing prohibition on consuming views, rather
than updating declaration facts as though every branch executed. Copyable view
values and ordinary field copies retain their existing behavior. Native ownership
planning already rejects a move-only view escaping into an owning assignment;
the interpreter's incidental value cloning is not an ownership conversion.

Owned mutable roots still pass these checks, but neither backend executes their
projected updates. Acceptance does not establish safe mutation behavior.

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

No execution option is selected by this note. The implemented read-only checks
follow existing immutable-binding and view rules; they do not select projected
updates. Runtime failure in both backends is not evidence that those updates
are supported.
