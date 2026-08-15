# Reflection Predicate Facts

Status: completed. The conservative reflection-fact and generic branch-checking
policies are selected and pinned by conformance fixtures.

## Decision

Jett keeps a conservative reflection-fact boundary for generic specialization.
A fact must remain structurally tied to the reflected generic type through one
of these forms:

- a direct `type.kind_tag[T]()` or `type.info[T]().kind_tag` comparison,
- a direct `type.primitive_tag[T]()` or `TypeInfo.primitive_tag` comparison,
- an immutable local carrying a `TypeKind` or `TypePrimitive` value,
- a typed helper parameter carrying that value from the same generic
  instantiation,
- a `match` arm over `TypeKind` or `TypePrimitive`.

An arbitrary tag supplied by a caller is not evidence about `T`. The checker may
use valid facts to prove branch reachability and validate casts for the concrete
generic instantiation.

A function call that returns `bool` never creates a reflection fact, even when
the function body is pure or compares only reflection tags. Copying a reflection
comparison into an arbitrary `bool` local also discards the fact. This rule keeps
type evidence local and inspectable: a boolean cannot be detached from the
reflected value or generic parameter that it was intended to describe, and
mixed runtime carriers cannot be hidden behind a broad classifier.

Small same-carrier classifiers remain useful for organizing runtime logic, but
their result does not authorize a generic cast. Code that needs a cast must use
a visible direct fact or `match` arm. This is a deliberate language and checker
contract, not merely an implementation gap.

## Branch Checking

A helper call or detached boolean also cannot make the checker skip a generic
branch. Both branches must typecheck, even when a pure helper could be evaluated
for the concrete instantiation. This keeps source validity independent from
optimizer sophistication and prevents helper implementation details from
silencing diagnostics.

A later optimization pass may fold a known helper result and remove unreachable
runtime code only after checking succeeds. Such folding does not carry a type
fact and cannot change whether the source program is accepted.

## Conformance Boundaries

- `generic_reflection_branch_specialization.jett` covers direct comparisons,
  immutable tag locals, and helper parameters receiving reflection values tied
  to the same `T`.
- `generic_reflection_match_specialization.jett` covers direct and helper-local
  `TypeKind` and `TypePrimitive` matches.
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

## Stdlib Boundary

The selected facts are enough for the current stdlib JSON implementation to
keep casts such as `int64 item = value`, `float64 item = value`, and
`bytes item = value` inside small primitive-specific helpers. Keep direct
`TypePrimitive` matches in JSON helpers that select different runtime carriers,
such as the separate `uint64` path.

Same-carrier classifier helpers may organize runtime logic when every selected
primitive shares one safe cast target, such as the int64-backed path for signed
integers and narrower unsigned integers. The classifier result still does not
authorize a generic cast.

This record resolves the policy decision requested by
[#6](https://github.com/vycdev/jett/issues/6). It rejects trusted predicate
annotations and general flow-sensitive boolean refinement as type-proof or
branch-checking mechanisms; ordinary post-check optimization remains an
implementation detail.
