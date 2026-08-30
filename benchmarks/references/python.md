# Python pilot reference v0.2

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
