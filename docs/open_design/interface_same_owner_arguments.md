# Concrete argument requirements in erased interface calls

An interface method may mention its interface type in more than one parameter:

```jett
interface Pairable:
    function combine(view self: Pairable, view other: Pairable) returns string
```

The implementation contract substitutes the concrete owner for those parameters.
Thus `implement Pairable for Left` declares both parameters as `Left`. This is
also the shape of the standard `Equatable.equals` implementation contract.

The checker nevertheless accepts a dynamic call with two `Pairable` operands
whose concrete owners differ. A probe with `Left(name: "one")` and
`Right(name: "two")` enters the `Left` implementation in the interpreter and
prints `left:one:two`, despite the second argument's nominal type. Native code
checks the concrete owner when unboxing and fails with the internal diagnostic
`invalid native struct handle or field`.

The remaining language decision is whether to:

- Permit erased calls and validate every concrete argument requirement at
  runtime, reporting a typed dispatch error before entering the implementation;
  or
- Reject erased calls whose implementation requires another argument of the
  receiver's concrete type, unless a checked static fact establishes that owner.

Do not reinterpret a different nominal struct as the selected owner's layout,
remove native unboxing checks, or add structural compatibility. The chosen rule
must cover view and owned parameters, nested occurrences of the owner type,
pending arguments, matching owners, and consistent cleanup/error precedence.
This is independent of the decision about equality operators on interfaces.
