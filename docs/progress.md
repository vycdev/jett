# Jett Compiler Progress

## Statistics

- **Lines of Rust:** ~17,500
- **Tests:** 327 passing
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
| Parser (recursive descent + Pratt) | `jett_parser` | 49 | Done |
| Formatter (canonical whitespace) | `jett_fmt` | 3 | Done |
| Pipeline orchestration | `jett_driver` | 0 | Done |
| CLI (format, build, run, test, lsp) | `jett_cli` | 0 | Done |

### Phase B: Type System Core — COMPLETE

| Component | Crate | Tests | Status |
|---|---|---|---|
| Type interning, struct/enum defs | `jett_types` | 17 | Done |
| Name resolution, scoping | `jett_resolve` | 12 | Done |
| Type checking (expressions, operators, generics) | `jett_typecheck` | 49 | Done |

### Phase C: Ownership and Capabilities — COMPLETE

| Component | Crate | Tests | Status |
|---|---|---|---|
| Ownership analysis (move/view/consume) | `jett_typecheck/ownership` | 9 | Done |
| Capability purity enforcement | `jett_typecheck/capability` | 7 | Done |
| Secret taint analysis | — | — | Not started |

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
| Comptime interpreter | `jett_comptime` | 115 | Done |
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
| Pipeline operator (`into`) | Done |
| String interpolation | Done |
| Verify blocks (parsing + execution) | Done |
| Property-based testing (`property` blocks) | Done |
| Bitfield declarations | Not started |
| Actor model (`actor`, `spawn`, `send`, `ask`) | Not started |
| Structured concurrency (`run`, `join`, `cancel`) | Not started |
| Interface/implement blocks | Not started |
| `mutual` blocks | Not started |
| Secret types (`secret[T]`, `declassify`) | Not started |
| Handle blocks (`handle error:`, `handle:`, `default`) | Done |
| Generic structs | Not started |

### Phase H: Agent Tooling — PARTIAL

| Component | Status |
|---|---|
| TOON output (`--agent` flag) | Done |
| LSP server (diagnostics on save) | Done |
| LSP hover (type at cursor) | Not started |
| LSP go-to-definition | Not started |
| LSP completions | Not started |
| MCP server | Not started |
| ASP query system (type-at, signature, completions) | Not started |

### Phase I: Testing and Profiling — PARTIAL

| Component | Status |
|---|---|
| Property-based test runner | Done (basic: 100 random iterations) |
| Input shrinking on failure | Not started |
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
| `string` (basic ops in interpreter) | Partial (13 builtins) |
| `list` (basic ops in interpreter) | Partial (6 builtins) |
| `math` (basic ops in interpreter) | Partial (3 builtins) |
| `net.http` | Not started |
| `net.socket` | Not started |
| `json` | Not started |
| `csv` | Not started |
| `time` | Not started |
| `crypto` | Not started |
| `encoding` | Not started |
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
