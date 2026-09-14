# Generic function values

Contextual specialization of a generic function used as a value remains an open
design choice. Direct and pipeline calls already instantiate generic functions
from explicit or inferred type arguments. A bare generic declaration is only a
template, not a checked function value with a concrete signature.

## Conservative boundary

Reject an unspecialized generic function reference when used as a value. Pass a
concrete named wrapper or inline function that makes an ordinary generic call:

```jett
namespace example
function identity[T](value: T) returns T:
    return value
function integer_identity(value: int64) returns int64:
    return identity[int64](value)
```

This keeps the existing call-site body checking, borrowing rules, effect policy,
and runtime type arguments authoritative. It introduces no specialization syntax
and does not change type inference for ordinary direct or pipeline calls.

## Why this needs an explicit boundary

The earlier checker stored generic declarations as templates without assigning
a concrete function type to bare references. Those references fell through to the
error type, which is compatible with other types to suppress cascading errors.
Consequently some higher-order calls appeared to compile without checking or
instantiating the generic body. The old interpreter sometimes executed such a
body as an erased anonymous function; declaration-preserving named dispatch
instead exposed its missing type arguments. Neither behavior defines safe
polymorphic function values. Do not restore unchecked body execution or guess
generic arguments from runtime values, which erase distinctions such as integer
widths and empty-collection element types.

## Future decision

Contextual specialization could instantiate a template from an expected concrete
function signature, but it must define ambiguity, return-only type parameters,
borrowing and effects, and retain checked instantiation identity across every
backend. This is separate from passing genuinely polymorphic values. Until that
choice is made and implemented, concrete wrappers are the supported form.
