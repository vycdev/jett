# Python pilot reference v0.5

## Type-driven development in Python

Start from the required annotated signature. Fully annotate any helper and
collection, keep each variable within the narrowest useful type, and structure
branches so every path returns the declared type. Do not use `Any`, untyped
containers, ignored type errors, or casts merely to silence a mismatch.

Return a complete Python 3.12 module containing the requested typed function.
Use ordinary `if`/`elif`/`else`, `for`, and `while` statements. Python integers
are sufficient for the bounded inputs. Lists use `list[int]`; iterate with
`for value in values` or `enumerate(values)`. Use `%` for remainder, `and`/`or`
for boolean expressions, and an explicit `return`. Do not print, read input,
add tests, or rename the requested function.

For a closed domain, use the requested `Enum` members and frozen dataclasses.
A union such as `type Outcome = Accepted | Rejected` preserves which payload
belongs to each case. Match every enum member explicitly; do not use `case _`,
`Any`, `cast`, or ignored type diagnostics. Pyright runs in strict mode before
the hidden tests.

Recursive domains use postponed annotations and the exact requested union.
When starter source is supplied, return one complete replacement module and
let strict checking reveal every branch affected by the type change.

Use frozen dataclasses for requested record shapes, `set[T]` for membership,
and precisely typed `dict[K, V]` values for maps. Express an optional as
`T | None`. When parsing bounded integers, validate the int64 range and any
canonical-text requirement explicitly; Python integers themselves are not
limited to int64.
