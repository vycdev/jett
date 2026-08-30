# Jett pilot reference v0.1

Use one namespace and declare functions before callers or `verify` blocks.

```jett
namespace benchmark

function example(values: list[int64], limit: int64) returns int64:
    int64 total = 0
    for value in values:
        if value > limit:
            total = total + limit
        else:
            total = total + value
    return total
```

Types used here are `int64`, `bool`, `string`, and `list[int64]`. Variables are
declared as `Type name = expression` and reassigned as `name = expression`.
Blocks begin with `:` and use four-space indentation. Boolean operators are
`and`, `or`, and `not`. Remainder is spelled `left modulo right`.

Loops may use `for value in values:`, `for i in range(end):`, or
`while condition:`. Use `return` explicitly. Strings use double quotes. Do not
print, read input, add tests, or use features outside the requested signature.
Return a complete source file and keep `namespace benchmark` first. Functions
have a hard cyclomatic-complexity maximum of 10; split logic into a small helper
declared before its caller when necessary.
