# Reflection Predicate Static Folding

Status: open design. No predicate-call extension to the completed reflection-fact
policy has been selected.

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

Such evaluation might answer whether a branch is reachable for one concrete
instantiation without treating an arbitrary runtime boolean as a reusable type
proof. It could reduce repeated primitive-family matches in generic stdlib code,
but it would still change which predicate-shaped code typechecks.

## Unresolved Questions

- Which helper bodies, calls, and argument sources would be eligible for static
  evaluation?
- Must an eligible helper be compiler-recognized, compiler-shipped, explicitly
  annotated, or merely pure and statically evaluable?
- Does the result affect only branch reachability, or can it carry a fact that
  authorizes a generic cast?
- How are mixed-carrier classifiers rejected when one predicate branch groups
  values with different runtime representations?
- What diagnostic distinguishes an ineligible predicate from an eligible helper
  that could not be evaluated?
- Do copied boolean locals remain outside the fact model?

## Required Design Work

Before implementation, a separate decision must specify eligibility, purity or
trust, static evaluation, reachability, diagnostics, and conformance coverage.
Tests must include sound same-carrier cases, mixed-carrier rejection, detached
booleans, caller-supplied tags, and behavior for an unavailable static result.

Until that decision is accepted, helper calls and detached booleans do not carry
reflection facts. JSON code must retain visible direct facts or `match` arms
where a generic cast depends on the primitive carrier.
