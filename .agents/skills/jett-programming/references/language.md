# Jett language and tooling reference

Use the compiler and nearby accepted source as the authority. `docs/open_design/` is unresolved and must not be treated as implemented behavior.

## Program shape and names

- A source file begins with `namespace name`. Namespaced declarations are private unless marked `export`.
- Declarations are visible top to bottom. Put callees before callers; use `mutual:` only for genuine recursion cycles.
- Built-in types use lowercase names. Declared types use PascalCase. Functions, variables, fields, variants, and namespaces use `snake_case`.
- Blocks use `:` plus four-space indentation. There are no braces or semicolons.
- Every variable has an explicit type and initializer. Add `mutable` before the type only when rebinding is required.
- Every function spells `function`, parameter names and types, and `returns Type`. Non-`nothing` paths return explicitly.

```jett
namespace sample
struct Coordinate:
    row: int64
    column: int64
function shift(point: Coordinate, row_delta: int64) returns Coordinate:
    return Coordinate(row: point.row + row_delta, column: point.column)
```

## Control flow and operators

Use `if` / `else if` / `else`, `for item in items:`, and `while condition:`. Equality operators are `==` and `!=`; boolean operators are `and`, `or`, and `not`; remainder is `modulo`. Functions have a cyclomatic-complexity maximum of 10 and bounded nesting, statements, and parameters. Extract a typed helper instead of deepening a branch ladder.

## Closed data and failures

Structs use named fields and named construction. Enums are closed variants with optional typed payloads. Construct a variant as `Type.variant(...)`; match a user enum exhaustively by unqualified variant name and do not add `other:` when explicit exhaustiveness matters.

`result[T, E]`, `optional[T]`, and user enums have distinct handling:

- A result is unwrapped only with `handle error:`; the handler ends in `return` or `default`.
- An optional is unwrapped only with bare `handle:`; the handler ends in `return` or `default`.
- A user enum is coarsened with exhaustive `match`.

Errors are values, not exceptions. Do not use `match` to unwrap `result` or `optional`.

## Ownership and mutation

Numbers, `bool`, `nothing`, and immutable `string` are implicitly copyable. `bytes`, `list`, `map`, `set`, structs, enums, capabilities, actor handles, task handles, and resources are move-only.

- Read a move-only value without transfer using `view` in both the parameter and call.
- Create an independent owned value only with explicit `clone`.
- Collection-transforming operations consume a collection and return its replacement.
- `mutable` permits local rebinding; it does not create a mutable reference.
- Closures capture only implicitly copyable values.

Lists are the only sequence type. Construct collections with standard-library constructors, not `[]` or `{}` literals. Use qualified operations such as `list.get`, `set.contains`, and `map.get_or`; do not guess method or indexing syntax. Query the compiler for the exact signature when uncertain.

## Effects and modules

Production effects enter through explicit capability parameters such as `Stdout`, `Filesystem`, `Network`, or `Clock`. Ordinary functions borrow capabilities with `view`. Put `use namespace` inside the function or block that needs it; file-level imports are forbidden. Standard-library functions stay namespace-qualified.

## Verification loop

Prefer agent-readable output when investigating:

```text
jett format --check file.jett
jett format file.jett
jett build --agent file.jett
jett test --agent file.jett
jett query --agent --symbols file.jett
jett query --agent --type-at file.jett:line:column
jett query --agent --signature qualified_name
```

`jett build` validates source; `--release` selects optimized mode. `jett test` runs `verify` and `property` blocks. Fix parse and resolution errors before ownership or policy errors, and preserve the requested public signature while repairing.

## Provenance

This reference summarizes implemented language documentation, compiler help, and accepted source. It intentionally contains no evaluation task, solution, hidden case, or failure-specific recipe.
