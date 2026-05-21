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
small primitive-specific helpers.

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
`stdlib/json/70_reflect_decode.jett` both spell out the same integer primitive
sets. But replacing the direct `match primitive:` arms with boolean predicates
would hide the exact primitive fact from the current checker.

For now, keep direct `TypePrimitive` matches in JSON helpers that perform
primitive-specific generic casts. Small helper extraction is still fine when the
call remains guarded by a direct primitive comparison or match arm.

## Possible Directions

1. Keep the current rule and accept some repeated primitive match arms.
2. Add a small language-level predicate mechanism for reflection facts, for
   example a trusted stdlib predicate annotation.
3. Teach the checker to inline/evaluate simple pure predicates over
   `TypeKind` and `TypePrimitive` values.

Option 1 is the current behavior. Options 2 and 3 need a separate design pass
because they affect generic specialization beyond JSON.
