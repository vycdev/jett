# Jett Compiler Progress

## Statistics

- **Lines of Rust:** ~33,000
- **Tests:** 470+ passing
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
| Parser (recursive descent + Pratt) | `jett_parser` | 67 | Done |
| Formatter (canonical whitespace) | `jett_fmt` | 8 | Done |
| Pipeline orchestration | `jett_driver` | 36 | Done |
| CLI (format, build, run, test, lsp) | `jett_cli` | 0 | Done |

### Phase B: Type System Core — COMPLETE

| Component | Crate | Tests | Status |
|---|---|---|---|
| Type interning, struct/enum/interface defs | `jett_types` | 18 | Done |
| Name resolution, scoping, namespace export visibility | `jett_resolve` | 27 | Done |
| Type checking (expressions, operators, generics) | `jett_typecheck` | 100 | Done |

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
| Core stdlib (.jett files) | `stdlib/` | — | Partial (bootstrap loader plus marker module and draft `json` module; many modules still Rust-backed) |

### Phase E: Comptime and Verification — COMPLETE

| Component | Crate | Tests | Status |
|---|---|---|---|
| Comptime interpreter | `jett_comptime` | 137 | Done |
| Verify blocks (compile-time) | `jett_comptime/verify` | — | Done |
| Comptime type reflection (`type.name`, `type.kind`, `type.kind_tag`, `type.primitive_tag`, `type.has_secret`, `type.info`, `type.arg`, `type.fields`, `type.bitfield_layout`, `type.bitfield_fields`, `type.machine_layout`, `type.machine_states`, `type.machine_transitions`, `type.machine_state_value`, `type.machine_field_value`, `type.variants`, `type.variant_value`, `type.field_value`, `type.variant_field_value`, `type.construct_start`, `type.construct_variant_start`, `type.construct_machine_start`, `type.construct_put`, `type.construct_finish`, trusted `comptime type` binding for roots, type args, struct/enum/machine field loops, `TypeInfo.args`, and `TypeInfo.primitive_tag`) | `jett_comptime` | — | Partial (struct/bitfield/enum/machine construction via `TypeConstruction`; structured `TypeKind` and `TypePrimitive` tags available; checked `ReflectionMetadata` snapshot now feeds metadata-only reflection builtins, value-sensitive field/variant/machine access, construction validation/finish, checked bitfield layout, checked machine state/transition metadata, trusted `comptime type` bindings for args/field/variant/machine-state loops, branch specialization for direct, immutable-local, and immutable helper-parameter `TypeInfo` / `TypeKind` / `TypePrimitive` generic reflection checks, selected-branch and direct top-level `type.arg[T]()` plus direct `type.fields[T]()` / `type.variants[T]()` / `type.machine_states[T]()` / `type.machine_state_value[T](...).fields` loop checking under concrete generic instantiations, top-level variant/state selection and `TypeConstruction` start/finish checking, and the `json.serialize` secret-containing type gate; AST fallback paths remain for bootstrap/direct-interpreter compatibility while metadata identity moves toward canonical checked records) |
| 25+ stdlib builtins in interpreter | `jett_comptime` | — | Done |

### Phase F: Interpreter — COMPLETE

| Component | Crate | Tests | Status |
|---|---|---|---|
| Tree-walking interpreter (`jett run`) | `jett_comptime` | — | Done (reuses comptime interpreter) |

### Phase G: Advanced Type Features — PARTIAL

| Feature | Status |
|---|---|
| State machines (`machine`, transitions, `at`) | Done (checked state-qualified types, branch narrowing, namespaced machines, reflection metadata, reflected machine construction, and JSON parse/serialize through the explicit state/payload envelope) |
| Refinement types (`type X = T where ...`, `coarsen`) | Done |
| User-defined structs (constructors, field access, methods) | Done |
| Match statements with enum destructuring | Done |
| Pipeline operator (`into`, single-line and multi-line indented form) | Done (qualified/generic steps, `view` steps, and step-local `handle error:` / `handle:` are covered) |
| String interpolation | Done |
| Verify blocks (parsing + execution) | Done |
| Property-based testing (`property` blocks) | Done |
| Bitfield declarations | Partial (constructors, field access, enum-annotated fields, payload fields, 64-bit `uint64` fields, reflection, and interpreter `to_bytes`/`from_bytes` roundtrips are covered; native codegen still pending) |
| Actor model (`actor`, `spawn`, `send`, `ask`) | Done (capability args, message args, state initializers, and `responds` values typecheck against declared types; exported namespaced actors support qualified and `use`-alias spawn) |
| Structured concurrency (`run`, `join`, `cancel`) | Done |
| Interface/implement blocks | Done |
| `mutual` blocks | Done |
| Secret types (`secret[T]`, `declassify`) | Partial |
| Handle blocks (`handle error:`, `handle:`, `default`) | Done |
| Generic structs | Done |
| Generic functions | Done |
| Comptime struct/enum/bitfield/machine introspection (`TypeInfo`, `TypeField`, `TypeBitfield`, `TypeBitfieldField`, `TypeMachine`, `TypeMachineState`, `TypeMachineTransition`, `TypeVariant`, alias/refinement base metadata, field `serialize` names, checked field/variant/machine value access) | Partial |
| String escape sequences (`\"`, `\\`, `\n`, `\t`, `\r`) | Done |
| Inline function expressions (`function(x: T) returns U: body`) | Done |
| Higher-order list functions (`filter`, `map`, `find`, `sort_by`, `all`, `any`, `count`, `sum`, `group_by`) | Done |
| Single-line `handle:` blocks | Done |
| `math.average`, `math.median` | Done |
| `string.reverse`, `string.after`, `string.before`, `string.trim_start`, `string.trim_end` | Done |
| `string.slugify`, `string.truncate`, `string.between`, `string.pad_left`, `string.is_not_empty` | Done |
| `string.chars`, `string.words`, `string.lines` (iterator builtins → `list[string]`) | Done |
| `string.index_of`, `string.count`, `string.to_upper_first`, `string.to_lower_first` | Done (search/count results are grapheme-boundary aware) |
| `list.reduce`, `list.flat_map`, `list.chunk`, `list.sort_by_index`, `list.is_sorted`, `list.all_elements_in` | Done (higher-order callback return types are checked for explicit typed calls) |
| `list.enumerate` | Done |
| `map.get_or`, `map.merge`, `map.set`, `map.contains_key` | Done |
| `encoding` module: `base64_encode`, `base64_decode`, `hex_encode`, `hex_decode`, `url_encode`, `url_decode` | Done |
| Byte-native hex helpers (`bytes.to_hex`, `bytes.from_hex`) | Done |
| Closure captures (inline functions capture immutable enclosing scope) | Done |
| Function type expressions (`function(T) returns U` in type annotations) | Done |
| Dotted `use` paths (`use net.http`) | Done |
| Namespace-qualified user function calls (`helpers.f()`, `helpers.f[T]()`) | Done |
| Namespace-private declarations with explicit `export` for public APIs, including `mutual` declarations and qualified-only external access | Done |
| Multi-file compilation (project-aware build/run with `jett.proj`) | Done |
| `range()` builtin (1, 2, or 3 args) | Done |
| For-in over strings, maps (with `key, value` destructuring), sets | Done |
| `and`/`or` keyword operators for logical expressions | Done |
| Unhandled result/optional detection (E0341, E0342) | Done |
| Set value type and 12 set builtins (`new`, `add`, `remove`, `contains`, `union`, `intersection`, `difference`) | Done |
| `print`/`println` builtins | Done (current secret-blocked debug helpers; stable capability policy remains open) |
| Type conversions: `float64.from_string`, `string.from_float64`, `string.from_bool` | Done |
| `time.now_ms`, `time.now_s` | Done |
| `os.env`, `os.args` | Done |
| Math: `pi`, `e`, `sin`, `cos`, `tan`, `mod`, `is_even`, `is_odd`, `sum` | Done |

### Phase H: Agent Tooling — PARTIAL

| Component | Status |
|---|---|
| TOON output (`--agent` flag) | Partial (build diagnostics include ok/error status plus diagnostic/error/warning counts, format status, run stdout/debug output, verify/property test summaries, and namespace/symbol/type-at/definition-at/references-at/completion/signature query results are structured) |
| LSP server (diagnostics on save) | Done |
| LSP hover (type at cursor) | Done |
| LSP go-to-definition | Done |
| LSP completions | Done |
| MCP server | Not started |
| ASP query system | Partial (`jett query --agent --namespaces`, `--symbols`, `--type-at`, `--definition-at`, `--references-at`, prefix-filtered `--complete-at`, and `--signature` are implemented; function completion rows include source-level signatures where available, while richer context/ranking metadata remains open) |

### Phase I: Testing and Profiling — PARTIAL

| Component | Status |
|---|---|
| Property-based test runner | Done (basic: 100 generated iterations; all numeric primitives, bool/string/bytes/nothing, aliases/refinements, structs including generic structs, bitfields, enums, plus generic list/set/map/optional/result pools) |
| Input shrinking on failure | Done (shrinking for int64, float64, string, bytes, list, set, map, optional, result, struct fields, enum payloads) |
| CPU profiler (`--profile`) | Not started |
| Memory profiler (`--profile-memory`) | Not started |
| `trace` keyword | Partial (parses, typechecks, runtime type-tagged current-value output in `jett run`) |
| `breakpoint` keyword | Partial (parses, typechecks, conditional runtime debug output with visible binding types in `jett run`) |

### Phase J: Cross-Platform and Interop — NOT STARTED

| Component | Status |
|---|---|
| Cross-compilation (`--target`) | Not started |
| C binding generator (`jett bind`) | Not started |
| `jett bundle` | Not started |

### Phase K: Full Standard Library — NOT STARTED

| Module | Status |
|---|---|
| `string` | Partial (30+ builtins: length/char_count, contains, trim, upper, lower, replace, split, join, starts_with, ends_with, is_empty, slice, repeat, pad_left, pad_end, from_int64, from_float64, from_bool, slugify, truncate, between, reverse, after, before, chars, words, lines, index_of, count, to_upper_first, to_lower_first; count/index/search/extraction helpers avoid partial grapheme matches) |
| `list` | Partial (40+ builtins: new, length, append, get, first, last, is_empty, skip, take, reverse, sort, contains, index_of, remove, concat, flatten, unique, zip, chunk, sort_by_index, is_sorted, all_elements_in, enumerate, from_set, repeat, range, last_index_of, insert_at, remove_at, swap + higher-order: filter, map, find, sort_by, all, any, count, sum, group_by, reduce, flat_map) |
| `set` | Partial (12 builtins: new, add, remove, contains, length, is_empty, to_list, union, intersection, difference) |
| `map` | Partial (17+ builtins: new, length, has/contains_key, get, get_or, insert/set, remove, keys, values, is_empty, merge, from_lists, entries + higher-order filter, map_values, for_each) |
| `math` | Partial (20+ builtins: abs, sqrt, pow, floor, ceil, round, clamp, log, log2, log10, min, max, average, median, pi, e, sin, cos, tan, mod, is_even, is_odd, sum) |
| `json` | Partial (json.serialize, json.serialize_public, json.parse_exact, json.parse_raw/JsonValue accessors, compiler-owned public policy for parse/serialization; interpreter `json.parse`, `json.parse_exact`, `json.serialize`, and `json.serialize_public` require trusted stdlib-loaded reflected `.jett` hooks under `namespace json`; typed `json.parse[T]` routes through the stdlib `JsonTree` parser/decoder, including the `json.parse[JsonValue]` compatibility branch, while `json.parse_exact[T]` rejects unknown object fields recursively; stdlib `JsonTree` has a view-native serializer, scalar/array/object parser, exported raw facade wrappers, traversal/scalar-cast helpers, reflected parse/serialize for machine state/payload envelopes, and reflected decoding via `TypeConstruction` for nested structs, enum-annotated bitfields, enums, machines, lists/maps/sets, optionals/results, bytes, sized integer/float primitives, null, secret wrappers, aliases/refinements, and missing optional-field defaults; raw `JsonValue` parsing/access now runs on native `JsonTree` values through trusted stdlib facades with a shared `jett_common` JSON-facade policy for runtime dispatch, trusted hook mapping, and implicit view ownership, typechecker raw facade signatures now come from the exported `json.JsonTree` stdlib surface, with bare `JsonValue` preserved by the stdlib-only root alias to bundled `json.JsonTree`, using checked reflection metadata for direct type reflection builtins, `TypeInfo`, `type.arg`, trusted `comptime type` bindings for args/field/variant/machine-state loops, `TypeField`, `type.field_value`, bitfield metadata, machine metadata, active machine state/field access, enum variant metadata, `type.variant_value`, `type.variant_field_value`, `type.construct_variant_start`, `type.construct_machine_start`, `type.construct_put`, `type.construct_finish`, the runtime `main()` interpreter, and the `json.serialize` secret-containing type gate, with fallback-path audit complete, canonical `TypeId` metadata lookup scaffolding expanded to owner fields/bitfields/machines/variants, and missing checked owner metadata now surfaced for fields/bitfields/machines/variants) |
| `random` | Partial (5 builtins: int64, float64, bool, choice, shuffle) |
| `crypto` | Partial (sha256, md5) |
| `encoding` | Partial (6 builtins: base64_encode, base64_decode, hex_encode, hex_decode, url_encode, url_decode) |
| `bytes` | Partial (9 builtins: new, length, slice, concat, from_string, to_string, get, to_hex, from_hex) |
| `uuid` | Partial (uuid.new) |
| `time` | Partial (time.now_ms, time.now_s) |
| `os` | Partial (os.env, os.args) |
| `net.http` | Not started |
| `net.socket` | Not started |
| `csv` | Partial (interpreter builtins for `csv.parse`, `csv.stringify`, and `csv.parse_with_header`; quoted commas, quotes, and multiline fields are covered) |
| `regex` | Not started |
| `log` | Not started |
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
| `jett format [--agent] [--check] file.jett` | Working |
| `jett build [--agent] [--release] [--target T] file.jett` | Working (validates, no binary output) |
| `jett run [--agent] file.jett` | Working (tree-walking interpreter; `--agent` captures stdout plus trace/breakpoint debug rows) |
| `jett test [--agent] [file.jett]` | Working (verify + property blocks; `--agent` emits compact block tables) |
| `jett lsp` | Working (diagnostics on save) |
| `jett bind header.h` | Not started |
| `jett bundle` | Not started |
| `jett mcp` | Not started |
| `jett query --agent --namespaces` / `--symbols file.jett` / `--type-at file:line:column` / `--definition-at file:line:column` / `--references-at file:line:column` / `--complete-at file:line:column` / `--signature name` | Partial (namespace registry, file-local symbols with function signatures, type lookup, definition lookup, reference lookup, prefix-filtered completion candidates with source signatures, and source-level function signatures) |
