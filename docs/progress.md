# Jett Compiler Progress

## Statistics

- **Lines of Rust:** ~30,500
- **Tests:** 441 passing
- **Crates:** 15
- **VS Code extension:** Yes

## Implementation Status

### Phase A: Foundation — COMPLETE

| Component | Crate | Tests | Status |
|---|---|---|---|
| FileId, Span, Symbol interner | `jett_common` | 3 | Done |
| Diagnostics, error rendering, TOON output | `jett_diagnostics` | 12 | Done |
| Project discovery, jett.proj parsing | `jett_project` | 7 | Done |
| Lexer (indentation, interpolation, 90+ tokens) | `jett_lexer` | 62 | Done |
| Parser (recursive descent + Pratt) | `jett_parser` | 58 | Done |
| Formatter (canonical whitespace) | `jett_fmt` | 3 | Done |
| Pipeline orchestration | `jett_driver` | 36 | Done |
| CLI (format, build, run, test, lsp) | `jett_cli` | 0 | Done |

### Phase B: Type System Core — COMPLETE

| Component | Crate | Tests | Status |
|---|---|---|---|
| Type interning, struct/enum/interface defs | `jett_types` | 18 | Done |
| Name resolution, scoping | `jett_resolve` | 15 | Done |
| Type checking (expressions, operators, generics) | `jett_typecheck` | 96 | Done |

### Phase C: Ownership and Capabilities — COMPLETE

| Component | Crate | Tests | Status |
|---|---|---|---|
| Ownership analysis (move/view/consume) | `jett_typecheck/ownership` | 9 | Done |
| Capability purity enforcement | `jett_typecheck/capability` | 7 | Done |
| Secret taint analysis | — | — | Partial |

### Phase D: Code Generation — NOT STARTED

| Component | Crate | Tests | Status |
|---|---|---|---|
| HIR (monomorphization) | `jett_hir` | — | Not started |
| MIR (control flow graph) | `jett_mir` | — | Not started |
| LLVM native codegen | `jett_codegen_llvm` | — | Not started |
| Runtime library | `jett_runtime` | — | Not started |
| Core stdlib (.jett files) | `stdlib/` | — | Not started |

### Phase E: Comptime and Verification — COMPLETE

| Component | Crate | Tests | Status |
|---|---|---|---|
| Comptime interpreter | `jett_comptime` | 134 | Done |
| Verify blocks (compile-time) | `jett_comptime/verify` | — | Done |
| 25+ stdlib builtins in interpreter | `jett_comptime` | — | Done |

### Phase F: Interpreter — COMPLETE

| Component | Crate | Tests | Status |
|---|---|---|---|
| Tree-walking interpreter (`jett run`) | `jett_comptime` | — | Done (reuses comptime interpreter) |

### Phase G: Advanced Type Features — PARTIAL

| Feature | Status |
|---|---|
| State machines (`machine`, transitions, `at`) | Done |
| Refinement types (`type X = T where ...`, `coarsen`) | Done |
| User-defined structs (constructors, field access, methods) | Done |
| Match statements with enum destructuring | Done |
| Pipeline operator (`into`, single-line and multi-line indented form) | Done |
| String interpolation | Done |
| Verify blocks (parsing + execution) | Done |
| Property-based testing (`property` blocks) | Done |
| Bitfield declarations | Partial |
| Actor model (`actor`, `spawn`, `send`, `ask`) | Done |
| Structured concurrency (`run`, `join`, `cancel`) | Done |
| Interface/implement blocks | Done |
| `mutual` blocks | Done |
| Secret types (`secret[T]`, `declassify`) | Partial |
| Handle blocks (`handle error:`, `handle:`, `default`) | Done |
| Generic structs | Done |
| Generic functions | Done |
| String escape sequences (`\"`, `\\`, `\n`, `\t`, `\r`) | Done |
| Inline function expressions (`function(x: T) returns U: body`) | Done |
| Higher-order list functions (`filter`, `map`, `find`, `sort_by`, `all`, `any`, `count`, `sum`, `group_by`) | Done |
| Single-line `handle:` blocks | Done |
| `math.average`, `math.median` | Done |
| `string.reverse`, `string.after`, `string.before`, `string.trim_start`, `string.trim_end` | Done |
| `string.slugify`, `string.truncate`, `string.between`, `string.pad_left`, `string.is_not_empty` | Done |
| `string.chars`, `string.words`, `string.lines` (iterator builtins → `list[string]`) | Done |
| `list.reduce`, `list.chunk`, `list.sort_by_index`, `list.is_sorted`, `list.all_elements_in` | Done |
| `map.get_or`, `map.merge`, `map.set`, `map.contains_key` | Done |
| `encoding` module: `base64_encode`, `base64_decode`, `hex_encode`, `hex_decode`, `url_encode`, `url_decode` | Done |

### Phase H: Agent Tooling — PARTIAL

| Component | Status |
|---|---|
| TOON output (`--agent` flag) | Done |
| LSP server (diagnostics on save) | Done |
| LSP hover (type at cursor) | Done |
| LSP go-to-definition | Done |
| LSP completions | Done |
| MCP server | Not started |
| ASP query system (type-at, signature, completions) | Not started |

### Phase I: Testing and Profiling — PARTIAL

| Component | Status |
|---|---|
| Property-based test runner | Done (basic: 100 random iterations) |
| Input shrinking on failure | Done (binary shrinking for int64, float64, string, list) |
| CPU profiler (`--profile`) | Not started |
| Memory profiler (`--profile-memory`) | Not started |
| `trace` keyword | Not started |
| `breakpoint` keyword | Not started |

### Phase J: Cross-Platform and Interop — NOT STARTED

| Component | Status |
|---|---|
| Cross-compilation (`--target`) | Not started |
| C binding generator (`jett bind`) | Not started |
| `jett bundle` | Not started |

### Phase K: Full Standard Library — NOT STARTED

| Module | Status |
|---|---|
| `string` (basic ops in interpreter) | Partial (23 builtins: length/char_count, contains, trim, upper, lower, replace, split, join, starts_with, ends_with, is_empty, is_not_empty, slice, repeat, pad_left, pad_end, from_int64, from_float64, from_bool, slugify, truncate, between) |
| `list` (basic ops in interpreter) | Partial (34 builtins: 19 basic + filter, map, find, sort_by, all, any, count, sum, group_by, reduce, chunk, sort_by_index, is_sorted, all_elements_in) |
| `map` (basic ops in interpreter) | Partial (12 builtins: new, length, has/contains_key, get, get_or, insert/set, remove, keys, values, is_empty, merge) |
| `math` (basic ops in interpreter) | Partial (15 builtins: abs, sqrt, pow, floor, ceil, round, clamp, log, log2, log10, min, max, average, median, pi constant) |
| `json` (serialize) | Partial (json.serialize, json.serialize_public) |
| `random` (basic ops in interpreter) | Partial (5 builtins: int64, float64, bool, choice, shuffle) |
| `net.http` | Not started |
| `net.socket` | Not started |
| `json` | Not started |
| `csv` | Not started |
| `time` | Not started |
| `crypto` | Not started |
| `encoding` (base64, hex, URL) | Partial (6 builtins: base64_encode, base64_decode, hex_encode, hex_decode, url_encode, url_decode) |
| `uuid` | Partial (1 builtin: uuid.new) |
| `validate` | Not started |
| `regex` | Not started |
| `random` | Not started |
| `uuid` | Not started |
| `log` | Not started |
| `format` | Not started |
| `os` | Not started |
| `test.mock` | Not started |

### Phase L: Incremental Compilation — NOT STARTED

| Component | Status |
|---|---|
| Salsa integration | Not started |
| Parallel compilation | Not started |
| Content-addressed caching | Not started |

## VS Code Extension

| Feature | Status |
|---|---|
| Syntax highlighting (TextMate grammar) | Done |
| Language configuration (brackets, indentation) | Done |
| LSP integration (diagnostics) | Done (via `jett lsp`) |

## CLI Commands

| Command | Status |
|---|---|
| `jett format [--check] file.jett` | Working |
| `jett build [--agent] [--release] [--target T] file.jett` | Working (validates, no binary output) |
| `jett run file.jett` | Working (tree-walking interpreter) |
| `jett test [file.jett]` | Working (verify + property blocks) |
| `jett lsp` | Working (diagnostics on save) |
| `jett bind header.h` | Not started |
| `jett bundle` | Not started |
| `jett mcp` | Not started |
| `jett query --agent` | Not started |
