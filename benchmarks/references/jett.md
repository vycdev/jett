# Jett pilot reference v0.3

## Type-driven development in Jett

Start from the required function type. Give every helper, parameter, local, and
return value its precise Jett type; use those boundaries to split the algorithm
before writing its branches. Do not weaken the requested types or bypass a
compiler error. Within this pilot subset, prefer small typed helpers over
encoding several logical states in one integer or string.

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

Primitive task types include `int64`, `bool`, `string`, and `list[int64]`. Variables are
declared as `Type name = expression` and reassigned as `name = expression`.
Blocks begin with `:` and use four-space indentation. Boolean operators are
`and`, `or`, and `not`. Remainder is spelled `left modulo right`.

Loops may use `for value in values:`, `for i in range(end):`, or
`while condition:`. Use `return` explicitly. Strings use double quotes. Do not
print, read input, add tests, or use features outside the requested signature.
Return a complete source file and keep `namespace benchmark` first. Functions
have a hard cyclomatic-complexity maximum of 10; split logic into a small helper
declared before its caller when necessary.

Closed domain types use enums. Variants may carry typed payloads, and `match`
must handle every variant explicitly:

```jett
enum Outcome:
    accepted(value: int64)
    rejected(reason: string)

function label(outcome: Outcome) returns string:
    match outcome:
        accepted(value):
            return "accepted {value}"
        rejected(reason):
            return "rejected {reason}"
```

Construct variants as `Outcome.accepted(1)`. Enums are move-only, so pass a
value once or use `view` only when the requested signature permits it. Do not
add `other:` catch-all arms in tasks that require explicit exhaustiveness.
