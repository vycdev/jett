# Jett language and tooling reference

Use the compiler and nearby accepted source as the authority. `docs/open_design/` is unresolved and must not be treated as implemented behavior.

## Program shape and names

- A source file begins with `namespace name`. Namespaced declarations are private unless marked `export`.
- Declarations are visible top to bottom. Put callees before callers; use `mutual:` only for genuine recursion cycles.
- Built-in types use lowercase names. Declared types use PascalCase. Functions, variables, fields, variants, and namespaces use `snake_case`.
- Keywords and built-in type spellings are reserved, not identifiers. Do not name a parameter or local `result`, `optional`, `view`, or another language word; use a domain name such as `outcome` instead.
- Blocks use `:` plus four-space indentation. There are no braces or semicolons.
- Every local has an explicit type and initializer: `Type name = value`. Add `mutable` before the type only when rebinding is required: `mutable Type name = value`.
- Parameters and fields use `name: Type`; locals never do. Do not write `let`, `var`, or `mutable name: Type`.
- Every function spells `function`, parameter names and types, and `returns Type`. Non-`nothing` paths return explicitly.

```jett
namespace sample
struct Coordinate:
    row: int64
    column: int64
function count_positive(values: list[int64]) returns int64:
    mutable int64 count = 0
    for value in values:
        if value > 0:
            count = count + 1
    return count
function shift(point: Coordinate, row_delta: int64) returns Coordinate:
    return Coordinate(row: point.row + row_delta, column: point.column)
```

## Control flow and operators

Use `if` / `else if` / `else`, `for item in items:`, and `while condition:`. Equality operators are `==` and `!=`; boolean operators are `and`, `or`, and `not`; remainder is `modulo`. A newline ends an ordinary expression: keep each function call and constructor argument list on one physical line. When a call would be too long, bind typed intermediate values first instead of wrapping its arguments across lines.

Functions have a cyclomatic-complexity maximum of 10 and bounded nesting, statements, and parameters. Nested matches and every `and` or `or` condition add decision points, so a short-looking function can still exceed the limit. Extract small typed helpers before writing a full branch matrix, declare them before the caller, and keep each function below the limit.

## Closed data and failures

Structs use named fields and named construction. Enums are closed variants with optional typed payloads. Construct a variant as `Type.variant(...)`; match a user enum exhaustively by unqualified variant name and do not add `other:` when explicit exhaustiveness matters.

```jett
namespace state_sample
enum Lookup:
    found(value: string)
    missing
function lookup_label(state: Lookup) returns string:
    match state:
        found(value):
            return value
        missing:
            return "missing"
```

`result[T, E]`, `optional[T]`, and user enums have distinct handling:

- A result is unwrapped only with `handle error:`; the handler ends in `return` or `default`.
- An optional is constructed as `some(value)` or `none` and unwrapped only with bare `handle:`; the handler ends in `return` or `default`.
- A user enum is coarsened with exhaustive `match`.

Errors are values, not exceptions. Do not use `match` to unwrap `result` or `optional`.

## Ownership and mutation

Numbers, `bool`, `nothing`, and immutable `string` are implicitly copyable. `bytes`, `list`, `map`, `set`, structs, enums, capabilities, actor handles, task handles, and resources are move-only.

- Read a move-only value without transfer using `view` in both the parameter and call.
- Create an independent owned value only with explicit `clone`.
- Collection-transforming operations consume a collection and return its replacement.
- `mutable` permits local rebinding; it does not create a mutable reference.
- Closures capture only implicitly copyable values.

Lists are the only sequence type. Construct collections with standard-library constructors, not `[]` or `{}` literals. Collection operations are namespace-qualified generic calls, not methods or indexing. Observers borrow with `view`; transformations consume the collection and must be assigned back.

```jett
namespace collection_sample
function first_or_zero(view values: list[int64]) returns int64:
    int64 first = list.get[int64](view values, 0) handle:
        default 0
    return first
function parsed_or_zero(text: string) returns int64:
    int64 parsed = int64.from_string(text) handle error:
        default 0
    return parsed
function render_number(value: int64) returns string:
    return string.from_int64(value)
function has_count(view counts: map[string, int64], name: string) returns bool:
    return map.has[string, int64](view counts, name)
function collection_forms() returns int64:
    mutable list[int64] values = list.new[int64]()
    values = list.append[int64](values, 7)
    int64 first = first_or_zero(view values)
    mutable set[int64] seen = set.new[int64]()
    if not set.contains[int64](view seen, first):
        seen = set.add[int64](seen, first)
    mutable map[string, int64] counts = map.new[string, int64]()
    int64 prior = map.get_or[string, int64](clone counts, "items", 0)
    counts = map.set[string, int64](counts, "items", prior + 1)
    return map.get_or[string, int64](counts, "items", 0)
```

Use `list.get` rather than `values[index]`, `set.contains` rather than a method call, `map.has` (or its `map.contains_key` alias) for map membership, and `map.get_or` rather than `map[key]`. There is no `map.contains`. Query the compiler for the exact signature when uncertain.

Parse decimal integers with `int64.from_string(text) handle error:` and render them with `string.from_int64(value)`.

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
