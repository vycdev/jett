# Direct equality of collections and sums

The checker accepts direct `==` and `!=` comparisons of bytes, lists, maps,
sets, optionals, and results. The interpreter has no corresponding binary
operation: even identical values fail at runtime. Native validation or emission
rejects the checked expression with an internal MIR contract error instead.

Confirmed probes include:

| Expression | Interpreter outcome |
| --- | --- |
| `none == none` | `unsupported binary operation: none Eq none` |
| `list(1) == list(1)` | `unsupported binary operation: list(1) Eq list(1)` |
| `map("a": 1) == map("a": 1)` | `unsupported binary operation: map(a: 1) Eq map(a: 1)` |
| `set.new[int64]() != set.new[int64]()` | `unsupported binary operation: set() NotEq set()` |
| `some(1) == some(1)` | `unsupported binary operation: some(1) Eq some(1)` |
| `ok(1) == ok(1)` | `unsupported binary operation: ok(1) Eq ok(1)` |
| `bytes.from_string("a") == bytes.from_string("a")` | `unsupported binary operation: bytes(97) Eq bytes(97)` |

The requested decision is whether these unsupported operations should become
source-level compile-time errors or preserve their existing runtime failure.
Neither option introduces structural equality. Equality inside an enum payload
uses a different existing interpreter path; this decision must not silently
change it or bypass the separate user-struct, actor, and interface decisions.

Compile-time rejection needs diagnostics for both operators, generic
instantiations, refinements, and explicit comptime evaluation. Preserving runtime
errors requires native operand evaluation in source order, matching diagnostic
formatting, and cleanup on the terminal failure edge, including pending values
and failure while evaluating either operand. Secret-output policy remains
applicable to diagnostics; raw formatting must not expose hidden secrets.

Do not count these expressions as native parity coverage merely because both
execution paths fail: an internal build rejection differs from a compiled
program's runtime failure. Keep the existing native rejection until the
source-language policy is settled.
