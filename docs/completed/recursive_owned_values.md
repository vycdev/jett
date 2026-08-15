# Recursive Owned Values

Status: completed. Self-recursive owned values use compiler-managed
indirection and ordinary Jett ownership syntax.

## Decision

Jett source names a recursive type directly:

```jett
struct Node:
    value: int64
    next: optional[Node]

enum IntList:
    empty
    item(value: int64, next: IntList)
```

There is no public `box[T]`, pointer type, allocation call, or fourth ownership
mode. A recursive struct or enum is an ordinary move-only value. Reading uses
`view`; independent duplication uses explicit deep `clone`. The compiler owns
the representation boundary and inserts indirection where native layout needs
it.

## Finite-Value Rule

Every recursive declaration must admit a finite value:

- `optional[T]`, `list[T]`, `map[K, V]`, and `set[T]` can terminate with
  `none` or an empty collection.
- `result[T, E]` is finite when either branch is finite.
- `secret[T]` changes visibility rather than shape, so its inner value must be
  finite.
- a recursive enum needs at least one variant whose fields can all terminate.
- direct required self-containment has no base and reports E0357.

Generic recursion must preserve the declared parameters exactly. For
`struct Chain[T]`, `Chain[T]` is a canonical recursive reference;
`Chain[list[T]]` is rejected because it creates an unbounded sequence of new
monomorphizations.

## Graph Boundary

This decision applies to self-recursive ownership trees and linked shapes. It
does not introduce mutually recursive named declarations or implicit shared
references. Jett's top-to-bottom rule remains unchanged for different types,
and `mutual:` remains function-only. Shared nodes, parent links, and cycles are
represented by explicit node IDs in lists or maps.

## Implementation

The type checker validates recursive declarations before later phases and
emits E0357 for shapes without a finite base. Generic struct monomorphization
installs a placeholder instance before resolving fields, allowing a canonical
self-reference to reuse the same concrete type rather than recursing forever.

Coverage includes recursive structs, enums, generic structs, ownership and
clone behavior, reflection, JSON round-tripping, property generation, direct
infinite containment, recursive enums without a base, non-terminating result
branches, and changing generic arguments.
