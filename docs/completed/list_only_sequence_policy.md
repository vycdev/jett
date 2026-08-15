# List-Only Sequence Policy

Status: completed. `list[T]` is Jett's sole source-level sequence type.

## Decision

Jett does not provide `array[T, N]` or another fixed-size sequence alongside
`list[T]`. A program that needs an exact-length value expresses that invariant
with a refinement:

```jett
type EncryptionBlock = list[uint8] where list.length(value) == 16
```

The refinement checks length when the refined value is created. It does not
change the underlying list representation and gives no source-level guarantee
of inline storage, stack allocation, fixed ABI layout, or bounds-check removal.

Native code generation may optimize a list when it can prove a length, but
that remains an implementation detail. C interop must marshal the canonical
list representation or use an opaque binding boundary rather than introducing
a second source sequence type.

## Rationale

A separate array creates two ways to model an ordered sequence and forces an
agent to choose based on representation details. Keeping one type preserves
Jett's one-canonical-form rule. Refinements already express the correctness
property—exact length—without exposing storage policy in ordinary source.

## Enforcement

The type checker recognizes the rejected `array[...]` spelling explicitly and
reports E0360 with the canonical `list[T]` plus refinement direction. Keeping
this case explicit prevents a future generic-type addition from accidentally
reviving the rejected form.

Focused coverage accepts a fixed-length list refinement and rejects
`array[...]`. Existing list fixtures cover construction, mutation, iteration,
reflection, JSON, ownership, and runtime behavior.
