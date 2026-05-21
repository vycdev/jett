# Reflection Predicate Facts

Status: open.

Jett now propagates several reflection facts through generic functions:

- direct `type.kind_tag[T]()` and `type.info[T]().kind_tag` comparisons,
- direct `type.primitive_tag[T]()` and `TypeInfo.primitive_tag` comparisons,
- immutable local `TypeKind` / `TypePrimitive` values,
- helper parameters that receive those direct reflection values,
- `match` arms over `TypeKind` and `TypePrimitive`.

That is enough for the current stdlib JSON implementation to keep casts such as
`int64 item = value`, `float64 item = value`, and `bytes item = value` inside
small primitive-specific helpers. The JSON stdlib also now uses narrow
classifier helpers for primitive families only when every selected primitive
shares the same safe runtime carrier, such as the int64-backed path for signed
integers and narrower unsigned integers.

## Deferred Shape

The checker does not currently understand predicate-derived facts:

```jett
function json_type_primitive_is_integer(primitive: TypePrimitive) returns bool:
    match primitive:
        int8_type:
            return true
        # ...
        other:
            return false

function reflected[T](value: T, primitive: TypePrimitive) returns string:
    if json_type_primitive_is_integer(primitive):
        int64 item = value
        return "{item}"
    return ""
```

This is tempting for JSON because `stdlib/json/60_reflect_serialize.jett` and
`stdlib/json/70_reflect_decode.jett` both need the same primitive families. But
using a predicate to guard a generic cast can hide the exact primitive fact from
the checker unless the called helper is itself narrow enough to be checked for
each concrete instantiation.

For now, keep direct `TypePrimitive` matches in JSON helpers that choose a
different runtime carrier, such as the separate `uint64` path. Small classifier
helpers are acceptable only when all selected primitives share one safe cast
target. Avoid broader predicates that mix carriers or imply facts the checker
cannot see.

## Possible Directions

1. Keep the current rule and accept some repeated primitive match arms.
2. Add a small language-level predicate mechanism for reflection facts, for
   example a trusted stdlib predicate annotation.
3. Teach the checker to inline/evaluate simple pure predicates over
   `TypeKind` and `TypePrimitive` values.

Option 1 plus the narrow same-carrier classifier rule is the current behavior.
Options 2 and 3 need a separate design pass because they affect generic
specialization beyond JSON.

## Narrow Candidate For Later

A plausible later slice is static-folding trivial helper calls over already
known `TypeKind` / `TypePrimitive` values without inferring general facts from
the returned `bool`.

For example, a stdlib helper could classify primitive families:

```jett
function json_type_primitive_is_integer(primitive: TypePrimitive) returns bool:
    match primitive:
        int8_type:
            return true
        int16_type:
            return true
        # ...
        other:
            return false
```

The checker would evaluate such a helper only when the argument is already a
static reflection value. It would answer "is this branch reachable for this
instantiation?", not "what type facts does this arbitrary boolean imply?".

This could reduce repeated primitive-family checks beyond the current
same-carrier cases, but it still changes which predicate-shaped generic code
typechecks. Keep it as a design follow-up rather than slipping it into JSON
cleanup.
