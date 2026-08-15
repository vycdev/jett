# Explicit Struct Equality

Status: completed. User-defined structs opt into equality with an explicit
`Equatable` implementation.

## Decision

Jett does not infer equality from struct fields and does not provide derive
syntax. The canonical contract is:

```jett
implement Equatable for User:
    function equals(view self: User, view other: User) returns bool:
        return self.id == other.id
```

`==` dispatches to `equals`; `!=` negates its result. There is no
`not_equals` method. Both parameters must be views and the return type must be
`bool`, so equality observes without consuming either operand. The normal
interface implementation checker enforces the exact signature.

This contract lets a type define semantic identity rather than accidentally
equating every stored detail. Adding a cache, display label, audit field, or
other representation detail cannot silently change equality. A struct without
an implementation receives E0358 at the operator.

## Standard Interface

`Equatable` is declared in compiler-shipped `stdlib/core.jett` and exposed as
an unqualified prelude interface. Numeric primitives, `string`, and `bool`
have ordinary source-defined implementations. User structs use the same
interface; the compiler does not create a privileged structural path for them.

Interface implementation validation substitutes the concrete owner for every
parameter typed as the implemented interface. This is required for both
`self` and `other` in the binary equality contract.

The interpreter dispatches struct equality through the registered concrete
implementation. Coverage proves that custom identity semantics override field
shape, that `!=` is the exact negation, that comparison preserves both owned
values, that missing implementations report E0358, and that an inexact method
signature is rejected.

## Deliberate Boundary

Enums retain their established variant-and-payload equality. Implementing
`Equatable` does not make a struct usable as a map key or set element. That
surface remains primitive-only, as recorded in the completed
[collection hashing boundary](primitive_collection_hash_boundary.md).
