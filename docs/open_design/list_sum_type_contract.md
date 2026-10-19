# List summation type contract

## Selected domain

The selected domain is exact numeric primitives and transparent aliases of those
primitives only.
Nominal refinements, secret wrappers, and nonnumeric types are rejected during
checking, including empty lists, inferred/generic specializations, pipelines,
concrete function wrappers, runtime code, and explicit `comptime` code. The same
rule applies to public `list.sum` and the private trusted `list.__sum` kernel.

This keeps the result locally predictable: it has the selected primitive width
and a known zero identity. A refinement is not necessarily closed under wrapping
addition and may exclude zero. Secret arithmetic needs an explicit secrecy
contract; admitting it by unwrapping its type would silently cross that boundary.
There is no public addition/identity interface and no implicit coarsening or
declassification. Callers can explicitly convert values using existing language
operations when their intended numeric domain permits it.

The checker owns this policy at the closed intrinsic signature. It checks each
concrete source-facade body rather than recognizing public call spellings, so
ordinary generic specialization cannot bypass it. Existing primitive-width,
typed-zero, pending-input, and borrowing behavior remains unchanged.
The private kernel requires one explicit type operand, already supplied by the
public facade, so an omitted operand cannot become an unchecked wildcard.

The probes below record the pre-decision mismatch, not current accepted behavior.

## Existing numeric behavior and historical boundary

The numeric-primitive behavior is selected in [the language design](../design.md)
and [the architecture](../architecture.md): `list.sum[T]` borrows its list,
integer addition wraps at the checked primitive width, floating addition rounds
at that width after each step, and empty numeric lists return zero of that type.
The implementation preserves the existing pending-container and pending-element
errors. Transparent aliases normalize to their primitive target and use the same
contract. The linked `list_sum_primitives` regression includes an `int8` alias,
all ten primitive numeric widths, typed empty zero, interface erasure, runtime
and explicit comptime values, and verify/property bodies.

The original numeric implementation left nominal refinements and nonnumeric types
without a summation contract. It selected no public addition/identity interface,
refinement validation result, or secret-wrapper admission.

The public source signature in `stdlib/list.jett` remains unconstrained:

```jett
export function sum[T](view items: list[T]) returns T:
    return list.__sum[T](view items)
```

Before this decision, the private kernel signature in
`crates/jett_typecheck/src/checker.rs` accepted `list[T]` and returned `T` without
checking its numeric domain. Native admission
in `crates/jett_codegen_cranelift/src/values.rs` accepts only an exact numeric
primitive element and an identical result type. The interpreter's selected
primitive path uses canonical alias expansion; before this decision its older
fallback remained reachable for other checked types. That fallback chooses an
integer or floating carrier from the first element and returns `Int64(0)` for any
empty list.

## Reproduced accepted gaps

These probes were run with a freshly built CLI at `8fdfa027`, after the primitive
sum and pending-aggregate fixes. Each direct runtime program passed frontend
checking. Except for the alias control, native emission rejected them with
`invalid native list signature for list.__sum` before an executable is produced.

| Checked element type and input | Reference outcome | Native outcome |
| --- | --- | --- |
| Transparent `uint64` alias, maximum value plus 2 | `1` | Same linked output |
| Transparent `float32` alias, empty list and 16777216 + 1 - 16777216 | `0` and `0` | Same linked output |
| `Positive = int64 where value > 0`, values 1 and 2 | Refined total 3 | Rejected at native admission |
| The same `Positive`, empty list | Terminal refinement constraint failure | Rejected at native admission |
| `SmallPositive = int8 where value > 0`, values 127 and 1 | Terminal `int8 value 128 is outside range -128..127` | Rejected at native admission |
| `bool`, one true element | Terminal `list.__sum: list elements must be int64 or float64` | Rejected at native admission |
| `bool`, empty list | Integer zero; trace says `bool = 0` | Rejected at native admission |
| `string`, empty list | Integer zero; trace says `string = 0` | Rejected at native admission |
| Struct with an int64 field, empty list | Integer zero; later field access reports `field access is not supported on 0` | Rejected at native admission |
| `optional[int64]`, empty list | Integer zero; later handling reports `handle block requires a result or optional value, got 0` | Rejected at native admission |
| `secret[int64]`, values 1 and 2 | Explicitly declassified total 3 | Rejected at native admission |
| `secret[int64]`, empty list | Explicitly declassified zero | Rejected at native admission |

The bool gap has a small standalone reproducer:

```jett
namespace app
function main() returns nothing:
    list[bool] values = list()
    bool total = list.sum[bool](view values)
    trace total
    println(total)
```

Its reference output is `trace total: bool = 0` followed by `0`. This is a wrong
runtime value shape, not evidence that bool summation or a bool zero is selected.
Moving the call into a closed pure function and invoking it with `comptime`
still prints `0` in the reference path. Native HIR materialization rejects it:
`cannot materialize generated Int64(0) as checked Bool`. Required compile-time
evaluation therefore does not close the malformed-value gap.

A numeric refinement can already produce a successful reference result:

```jett
namespace app
type Positive = int64 where value > 0
function main() returns nothing:
    Positive first = 1 handle error:
        return nothing
    Positive second = 2 handle error:
        return nothing
    list[Positive] values = list(first, second)
    Positive total = list.sum[Positive](view values)
    println(coarsen total)
```

This prints `3`; an empty list fails the refinement predicate at the return
boundary. A closed pure helper computing and coarsening that total also returns
3 with explicit `comptime`, while its native build still rejects the unsupported
kernel specialization. Neither success establishes a contract for arbitrary
refinements, empty refined lists, or invariant-breaking overflow.

## Deferred extensions

The selected domain closes the source-admission gap. Any future extension requires
a separate design decision; the historical fallback is not its specification:

- If nominal numeric refinements are admitted, how the result preserves or
  validates their invariant, what return type carries validation failure, and
  what happens when the empty-list zero violates the predicate. Primitive-width
  wrapping alone cannot prove a refinement is closed under addition.
- Whether secret-wrapped numeric types are admitted, and how their selected
  arithmetic width and result secrecy are retained across runtime/comptime
  boundaries.
- Whether any nonnumeric summation is intended. A general extension would need
  an explicit addition and identity contract; the current unconstrained `T`
  signature does not provide one.

The implementation must not infer these choices from the fallback's successful
zero or from the primitive carrier used by a refinement. In particular, applying
`primitive_base_type_name` to all summation types would silently erase refinement
and secret boundaries while leaving the return/invariant questions unresolved.
The numeric-only frontend admission makes the old fallback unreachable from these
checked source specializations. Native admission remains conservative; a rejected
specialization is not an implemented numeric or refinement conversion.
