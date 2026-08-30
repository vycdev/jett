# TypeScript pilot reference v0.2

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
