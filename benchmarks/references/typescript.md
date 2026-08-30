# TypeScript pilot reference v0.3

## Type-driven development in TypeScript

Start from the required strict signature and let it drive the implementation.
Keep inputs readonly when requested, narrow values through control flow, give
helpers precise input and output types, and make branches return one coherent
type. Do not escape through `any`, unchecked assertions, `@ts-ignore`, or broad
types that discard useful information.

Return one strict TypeScript ES2022 module exporting the requested function.
Use `bigint` and `n`-suffixed literals where the signature uses bigint; do not
mix bigint and number arithmetic. Arrays may be read through indexing or
`for...of`. Use `===`, `&&`, `||`, `%`, and explicit returns. Do not print,
read input, add tests, or rename/remove the requested export.

Model closed domains as discriminated unions with literal `kind` fields. A
`switch (value.kind)` narrows the payload in each case. Handle every literal
case and return directly; do not add `default`, use `any`, suppress a diagnostic,
or erase the union through assertions. Strict checking and implicit-return
checking run before hidden tests.
