# Reflection Predicate Facts

Status: conservative reflection-fact policy selected and pinned; broader
predicate semantics are deferred rather than implied by this contract.

Jett now propagates several reflection facts through generic functions:

- direct `type.kind_tag[T]()` and `type.info[T]().kind_tag` comparisons,
- direct `type.primitive_tag[T]()` and `TypeInfo.primitive_tag` comparisons,
- immutable local `TypeKind` / `TypePrimitive` values,
- helper parameters that receive those direct reflection values for the same
  generic instantiation,
- `match` arms over `TypeKind` and `TypePrimitive`.

## Decision

Jett keeps this conservative boundary as the reflection-fact policy for generic
specialization. A fact must remain structurally tied to the reflected generic
type through a direct comparison, an immutable `TypeKind` / `TypePrimitive`
value, a typed helper parameter carrying that value from the same generic
instantiation, or a matching arm. An arbitrary tag supplied by a caller is not
evidence about `T`. The checker may use valid facts to prove branch reachability
and validate casts for the concrete generic instantiation.

A function call that returns `bool` does not create a reflection fact, even
when the function body is pure or compares only reflection tags. Copying a
reflection comparison into an arbitrary `bool` local also discards the fact.
This rule keeps type evidence local and inspectable: a boolean cannot be
detached from the reflected value or generic parameter that it was intended to
describe, and mixed runtime carriers cannot be hidden behind a broad
classifier.

Small same-carrier classifiers remain useful for organizing runtime logic, but
their result does not authorize a generic cast. Code that needs a cast must use
a visible direct fact or `match` arm. This is a deliberate language/checker
contract, not merely an implementation gap.

## Conformance Boundaries

- `generic_reflection_branch_specialization.jett` covers direct comparisons,
  immutable tag locals, and helper parameters receiving reflection values tied
  to the same `T`.
- `generic_reflection_match_specialization.jett` covers direct and helper-local
  `TypeKind` / `TypePrimitive` matches.
- `generic_reflection_local_fact_specialization.jett` pins local fact use for a
  concrete instantiation.
- `generic_reflection_predicate_fact_boundary.jett` rejects a predicate call as
  a generic cast guard.
- `generic_reflection_boolean_fact_boundary.jett` rejects detached boolean
  evidence.
- `generic_reflection_helper_kind_fact_cache.jett` and
  `generic_reflection_helper_primitive_fact_cache.jett` reject arbitrary
  caller-supplied tags as facts about `T`, including cases that could hide a
  mixed runtime carrier.

That is enough for the current stdlib JSON implementation to keep casts such as
`int64 item = value`, `float64 item = value`, and `bytes item = value` inside
small primitive-specific helpers. The JSON stdlib also now uses narrow
classifier helpers for primitive families only when every selected primitive
shares the same safe runtime carrier, such as the int64-backed path for signed
integers and narrower unsigned integers.

## Deferred Shape

Under the selected policy, the checker does not derive facts from predicates:

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
a predicate result does not retain which `TypePrimitive` fact made the branch
reachable. Treating that boolean as a cast guard could therefore hide a
mixed-carrier family from the checker.

Keep direct `TypePrimitive` matches in JSON helpers that choose different
runtime carriers, such as the separate `uint64` path. Small classifier helpers
are acceptable when all selected primitives share one safe cast target, but the
classifier result does not authorize the cast. The compile-fail fixture
`generic_reflection_predicate_fact_boundary.jett` pins that a boolean helper
over `TypePrimitive` is not a generic cast guard.
`generic_reflection_boolean_fact_boundary.jett` similarly pins that assigning a
reflection comparison to a `bool` local does not preserve the fact. Changing
either rule requires a separate design decision because arbitrary booleans are
easier for agents to detach from the carrier they were meant to prove.

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
typechecks. A trusted predicate annotation would change the contract as well.
Either extension requires a new design decision that specifies eligibility,
purity or trust, static evaluation, reachability, diagnostics, and mixed-carrier
conformance tests. Until then, neither helper calls nor detached booleans carry
reflection facts.

This note addresses the policy decision requested by
[#6](https://github.com/vycdev/jett/issues/6). The selected policy does not
pre-approve static folding, trusted predicate annotations, or general
flow-sensitive boolean refinement.
