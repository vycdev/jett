# Global constant execution

The design selects immutable compile-time constants baked into the binary,
subject to declaration order and namespace isolation. Runtime initialization
and mutable global storage are forbidden. E0377 rejects mutable namespace-level
declarations; local mutable bindings remain unaffected.

## Implemented contract

The frontend accepts literals (including signed numeric spellings), earlier
same-namespace constant references, parentheses around these forms, or explicit
`comptime expression`. All required calculations, including operators,
interpolation, and calls, use that canonical explicit spelling. An ordinary
calculation initializer reports E0378 even when its operands are known. Every
semantically pure function remains eligible at an explicit site. Ordinary pure
expressions outside required evaluation keep their runtime behavior, and optional
optimization cannot change source validity.

A literal spelling intrinsically denotes a value, and a constant reference reads
an earlier value. Recognizing a negative numeric literal is part of its spelling,
not a general authorization to evaluate unary expressions. Computing `base * 2`,
`not enabled`, `-base`, or an interpolated string executes a calculation. Making
the destination immutable does not authorize that execution implicitly. Write
`int64 doubled = comptime (base * 2)`; a direct copy may use `int64 copied = base`.
This distinction preserves the unconditional explicit-evaluation rule in
`docs/design.md` and does not add an implicit operator or function boundary.

Compilation materializes permitted namespace constants in declaration order,
evaluating calculations only at explicit sites, including unused declarations.
Their checked values are retained by declaration
span and supplied to reference execution and verify/property drivers. Native HIR
uses declaration-keyed `Constant` reads and replaces them with typed baked values
before MIR lowering. Both backends therefore receive the same immutable value,
without a startup initializer or a mutable global store. Generic and inline
function bodies retain the constant declaration identity rather than capturing
ambient caller state.

The supported subset is fixed-width integers, `float32`, `float64`, `bool`,
`string`, `nothing`, and transparent aliases of those primitives. Evaluated
values must match their declared primitive shape and width. A `run` expression
can retain hidden pending state despite having a primitive checked type; such a
value reports E9001 instead of entering the baked-value table. Evaluation errors
also report E9001 before execution or native publication.

Debug `trace` reads supported constants with their declared type spelling.
Native lowering uses a scoped observation temporary that does not enter later
breakpoint snapshots; release builds remove the trace.

## Remaining ownership policy

Move-only constant values still need a precise read and ownership rule before
execution support is extended to them. Do not implicitly clone a move-only
constant on every read, silently consume shared storage, or introduce mutable
references. Opaque resources, actors, pending tasks, and erased payloads require
separate scrutiny for hidden state or authority. Callable constants must retain
declaration context and their permitted captures.

Other constant types, including aggregates and nominal refinements, resources,
actor/task values, function values, and erased payloads, conservatively report
E9001. This is an implementation boundary while ownership remains unresolved,
not a decision to narrow the language's constant contract permanently. Local
values and supported explicit `comptime` results outside namespace declarations
keep their existing behavior.

Full native acceptance still requires closing this ownership policy and checking
the final revision against the independent inventory, workspace, and distribution
gates. The primitive handoff alone does not establish complete native parity.
