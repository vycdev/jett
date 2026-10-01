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

Pending interface receivers still fail dispatch. Their diagnostic names the
registered interface method, including its declaration namespace, consistently
with generated dispatchers. The failure regression evaluates an earlier owned
interpolation segment before a temporary pending interface receiver containing
a secret-bearing record. Both paths preserve the earlier output, omit the
secret, and terminate with the same error; native cleanup must release all
partially evaluated owners.

Implicit display results have a separate exact-string readiness boundary.
The reference interpreter rejects a pending string returned by the selected
method with `Displayable.display returned pending(...) instead of string`.
HIR retains this distinction with `DisplayResult` around only the implicit
call; direct method calls and ordinary pending-string interpolation are
unaffected. MIR keeps the check in each staged segment initializer, before
later handlers or effects. Native checking borrows the returned handle without
joining or consuming it, leaving its existing owning slot available for use
or terminal cleanup.
