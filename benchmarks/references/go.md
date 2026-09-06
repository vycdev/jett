# Go pilot reference v0.5

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

Use the requested named constant types for closed states and events. When the
required outcome has different payload shapes, implement its private marker
method on the requested concrete result structs. Switch over every named
constant without a `default` branch. Go does not prove switch exhaustiveness,
so keep the explicit cases visible and retain precise result types instead of
using `any` or `interface{}`.

Recursive domains use the requested private-marker interface and concrete
structs. When starter source is supplied, return one complete replacement file
and update every type switch affected by the new concrete state.

Use structs for requested records and `map[T]struct{}` for sets. A requested
optional is represented by its exact result struct rather than a sentinel.
Use precisely typed maps and preserve explicit type-switch cases. For canonical
int64 parsing, use `strconv.ParseInt` and compare `strconv.FormatInt` with the
original text.
