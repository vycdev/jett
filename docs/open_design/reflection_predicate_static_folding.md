# Reflection Predicate Static Folding

Status: narrowed open design. Predicate calls and detached booleans permanently
cannot carry type facts or authorize generic casts. Only reachability-only
static evaluation remains unresolved.

## Context

The completed
[reflection predicate fact contract](../completed/reflection_predicate_facts.md)
requires evidence about a generic type to remain structurally tied to that type.
Predicate calls and detached `bool` locals therefore do not authorize generic
casts.

A possible later slice would statically evaluate a narrow helper call only when
its `TypeKind` or `TypePrimitive` argument is already known. For example:

```jett
function json_type_primitive_is_integer(primitive: TypePrimitive) returns bool:
    match primitive:
        int8_type:
            return true
        int16_type:
            return true
        other:
            return false
```

Such evaluation could only answer whether a branch is reachable for one
concrete instantiation. It cannot turn an arbitrary runtime boolean into a
reusable type proof or authorize a cast. It could reduce repeated
primitive-family matches in generic code, but it would still change which
predicate-shaped branches are checked as reachable.

## Settled Type-Proof Boundary

- Helper calls returning `bool` never carry reflection facts.
- Copying a reflection comparison into a `bool` local permanently discards the
  fact.
- Purity, compiler ownership, annotations, or successful static evaluation do
  not let either form authorize a generic cast.
- Casts continue to require a visible direct reflection fact or `match` arm.

## Unresolved Questions

- Which helper bodies, calls, and argument sources would be eligible for static
  evaluation?
- Must an eligible helper be compiler-recognized, compiler-shipped, explicitly
  annotated, or merely pure and statically evaluable?
- How are mixed-carrier classifiers rejected when one predicate branch groups
  values with different runtime representations?
- What diagnostic distinguishes an ineligible predicate from an eligible helper
  that could not be evaluated?

## Required Design Work

Before reachability-only implementation, a separate decision must specify
eligibility, purity or trust, static evaluation, diagnostics, and conformance
coverage. Tests must include same-carrier cases, mixed-carrier rejection,
caller-supplied tags, and behavior for an unavailable static result. Existing
predicate-call and detached-boolean compile-fail fixtures remain permanent cast
boundaries.

Whether or not reachability-only folding is later accepted, helper calls and
detached booleans do not carry reflection facts. Generic code must retain visible
direct facts or `match` arms where a cast depends on the primitive carrier.
