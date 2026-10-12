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

Before selection, the language tradeoff was whether to:

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

## Selected runtime contract: exact concrete arguments

The native-codegen goal selects runtime validation rather than banning an erased
call whose static interface signature contains another occurrence of the owner.
Selection was recorded before implementation on 2026-10-08. Source accepted by
the checker remains accepted; an erased interface is not a proof that two values
have the same nominal implementation owner.

Resolve the receiver's exact checked concrete type and selected method first.
Before converting any argument to that concrete signature or entering the method
body, validate each concrete argument requirement against the argument's retained
checked concrete identity. Different nominal structs remain different even when
their fields have identical names and representations. This also applies to
checked generic arguments, refinements, primitive widths, and nested occurrences
of the owner in parameter types. Matching owners retain ordinary view semantics;
validation grants neither cloning nor ownership transfer.

A mismatch fails dispatch before the implementation body. It follows existing
runtime failure propagation and argument cleanup, with no user-defined body
execution. Pending receiver/argument failures retain their existing evaluation
and error precedence; validation must not inspect an unavailable payload or
replace an earlier failure. Native unboxing and handle checks remain mandatory.
This decision does not define equality operators on erased interfaces.

The reference/native implementation and local MSVC Debug/Release gates are
recorded in [exact concrete argument verification](../active/native_erased_same_owner_arguments.md).
That bounded acceptance includes optimized production matching/mismatched calls;
full refinement/pending Source, GNU, installed and coherent native parity remain
open. No recursive Self substitution or interface equality rule is introduced.