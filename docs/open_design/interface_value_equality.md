# Equality of erased interface values

The checker currently accepts `==` and `!=` when both operands have the same
interface type. Native verification rejects the binary expression because the
erased handle has no equality contract. The interpreter removes concrete type
metadata and attempts its ordinary value operation: two erased `int64` values
compare successfully, while an erased integer and string fail at runtime.

This is separate from comparing concrete structs through their exact
`Equatable.equals` implementation. An interface may contain unrelated concrete
types, including types without equality, actor handles, and secret-bearing
values. The current accidental acceptance does not define a uniform equality
rule or justify structural comparison of user structs.

Possible contracts:

- Reject equality operators on interface-typed values. Code compares before
  erasure or exposes an explicit domain comparison function or interface method.
- Define dynamic equality of concrete payloads. This needs rules for different
  concrete types, unsupported payloads, secret results, pending operands, and
  exact user-struct equality methods, including how those methods enter native
  dispatch and reachability.

Do not compare erased handle addresses or introduce a structural fallback.
After the decision, add frontend and linked native/interpreter regressions for
same and different implementations, supported and unsupported equality payloads,
and ownership of both operands.
