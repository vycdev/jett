# Go pilot reference v0.2

## Type-driven development in Go

Start from the required function signature. Use concrete parameter, local, and
helper types to make each state and conversion explicit, and keep all return
paths consistent with the declared result. Do not replace useful static types
with `any` or `interface{}` and do not hide mismatches behind conversions.

Return one formatted Go source file in `package benchmark` with the requested
function. Use `int64`, `string`, slices, `if`, and `for`; Go has no `while`
keyword, so use `for condition`. Range iteration provides index and value.
Use `%`, `&&`, and `||`. Do not add a `main`, perform I/O, add tests, or rename
the requested exported function.
