# Display interpolation uses the checked owner

String interpolation selects the exact `Displayable.display` implementation for
the expression's checked type. If that type is an interface, its implementation
receives the erased value and may dispatch through the original interface.
Selecting the concrete payload's display method instead changes this explicit
contract. Native HIR selects the checked owner; the interpreter now does the
same instead of selecting only the payload's runtime owner.

Implementation registration must retain the interface's declaration namespace,
which can differ from the namespace containing its implementation. In particular,
a root `Displayable` implemented by a namespaced type remains that root interface.
This is ordinary declaration identity, not a new method search rule.

Differential coverage includes namespaced implementations, an erased
primitive and record with distinct concrete display behavior, projected and
returned interface values, generic and scoped bindings, and closed comptime
formatting. Interpolation evaluates each expression once and keeps view operands
available afterward.

An explicit checked display implementation also takes precedence over the
built-in formatter for primitive types. Both the interpreter and native HIR
select the exact method before their primitive fallback.
