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
| Secret taint analysis | — | — | Partial (refined-secret taint fixed by [#18](https://github.com/vycdev/jett/issues/18); content-constant interpreter comparison for string/bytes secrets defined by [#33](https://github.com/vycdev/jett/issues/33), with native lowering pending [#20](https://github.com/vycdev/jett/issues/20) and [#22](https://github.com/vycdev/jett/issues/22)) |

### Phase D: Code Generation — NOT STARTED

| Component | Crate | Tests | Status |
|---|---|---|---|
| HIR (monomorphization) | `jett_hir` | — | Not started ([Tracked by #20](https://github.com/vycdev/jett/issues/20)) |
| MIR (control flow graph) | `jett_mir` | — | Not started ([Tracked by #22](https://github.com/vycdev/jett/issues/22)) |
| LLVM native codegen | `jett_codegen_llvm` | — | Not started |
| Runtime library | `jett_runtime` | — | Not started |
| Core stdlib (.jett files) | `stdlib/` | — | Partial (bootstrap loader plus marker module and extracted `json` module; `math.is_even`, `math.is_odd`, `math.sign`, `math.to_radians`, `math.to_degrees`, and consuming `math.sum(list[int64])` are source-defined in `stdlib/math.jett`; `string.is_not_empty`, `string.reverse`, `string.after`, `string.before`, and `string.between` are source-defined in `stdlib/string.jett`; basic list, set, and map extraction slices are [tracked by #57](https://github.com/vycdev/jett/issues/57), [#59](https://github.com/vycdev/jett/issues/59), and [#61](https://github.com/vycdev/jett/issues/61); many other modules and operations remain Rust-backed) |

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
| Secret types (`secret[T]`, `declassify`) | Partial (refined-secret taint fixed by [#18](https://github.com/vycdev/jett/issues/18); `secret.compare` now accepts only compatible string/bytes secrets and avoids content-dependent early exits in the interpreter per [#33](https://github.com/vycdev/jett/issues/33); native lowering remains pending) |
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
| `print`/`println` builtins | Partial (interpreter support and secret blocking are done; [debug-only capability policy](open_design/print_debug_builtin_policy.md) is decided, while debug-event isolation, release diagnostics, and future-backend conformance remain pending) |
| Type conversions: `float64.from_string`, `string.from_float64`, `string.from_bool` | Done |
| `time.now_ms`, `time.now_s` | Done |
| `os.env`, `os.args` | Done |
| Math: `pi`, `e`, `sin`, `cos`, `tan`, `mod`, `is_even`, `is_odd`, `sum` | Done |

### Phase H: Agent Tooling — PARTIAL

> Tracked by [#35](https://github.com/vycdev/jett/issues/35) for the remaining
> structured diagnostic context and agent-mode failure envelope.

| Component | Status |
|---|---|
| TOON output (`--agent` flag) | Partial (build diagnostics include file, ok/error status, severity counts, ranged diagnostics/labels, and tabular suggested fixes; format status, run stdout/typed debug output, ranged verify/property test summaries, namespace/symbol/type-at/definition-at/references-at/completion/signature query results, file-symbol parse failures, and type-at compiler failures with known source context and cross-file labels are structured) |
| LSP server (diagnostics on save) | Done |
| LSP hover (type at cursor) | Done |
| LSP go-to-definition | Done |
| LSP completions | Done |
| MCP server | Not started; initial transport, tool/resource, and ASP handoff boundary tracked by [#37](https://github.com/vycdev/jett/issues/37) |
| ASP query system | Partial (`jett query --agent --namespaces`, `--symbols`, `--type-at`, `--definition-at`, `--references-at`, prefix-filtered `--complete-at`, and `--signature` are implemented; namespace, type, symbol, definition, reference, and completion rows include source ranges; file-symbol parse failures and type-at parse, resolution, and type-check failures with known source context preserve structured diagnostics and cross-file labels; completion rows also include deterministic rank, match kind, namespace, visibility, and source-level signatures where available) |

### Phase I: Testing and Profiling — PARTIAL

| Component | Status |
|---|---|
| Property-based test runner | Done (basic: 100 generated iterations; all numeric primitives, bool/string/bytes/nothing, aliases/refinements, structs including generic structs, bitfields, enums, plus generic list/set/map/optional/result pools) |
| Input shrinking on failure | Done (shrinking for int64, float64, string, bytes, list, set, map, optional, result, struct fields, enum payloads) |
| CPU profiler (`--profile`) | Not started |
| Memory profiler (`--profile-memory`) | Not started |
| `trace` keyword | Partial (parses, typechecks, runtime type-tagged current-value output in `jett run`) |
| `breakpoint` keyword | Partial (parses, typechecks, conditional runtime debug output with visible binding types in `jett run`; pause/inspection protocol tracked by [#41](https://github.com/vycdev/jett/issues/41)) |

### Phase J: Cross-Platform and Interop — PARTIAL

| Component | Status |
|---|---|
| Cross-compilation (`--target`) | Not started |
| C binding generator (`jett bind`) | Not started (initial FFI and generated binding contract tracked by [#53](https://github.com/vycdev/jett/issues/53)) |
| `jett bundle` | Working (resolver-derived whole-file dependency ordering, stable lexical tie-breaking, structured cycle and namespace-boundary diagnostics, line manifests, and validation-before-write) |

### Phase K: Full Standard Library — NOT STARTED

| Module | Status |
|---|---|
| `string` | Partial (`is_not_empty`, `reverse`, `after`, `before`, and `between` are source-defined in `stdlib/string.jett`; the complete public `string.*` API remains the source-owned target, while hardcoded public signatures and Rust dispatch for the other operations are transitional bootstrap debt pending follow-up extraction into source declarations backed only as needed by private trusted Unicode/grapheme kernels; count/index/search/extraction helpers avoid partial grapheme matches) |
| `list` | Partial (40+ compiler-backed public operations are transitional technical debt; every public declaration must ultimately move to compiler-shipped `.jett` source, with only private trusted allocation, indexing, mutation, sorting, and callback kernels retained; the first `is_empty`, `first`, and `last` source extraction slice, including collection-view ownership regressions, is [tracked by #57](https://github.com/vycdev/jett/issues/57)) |
| `set` | Partial (10 compiler-backed public operations are transitional technical debt; every public declaration must ultimately move to compiler-shipped `.jett` source, with only private trusted equality, storage, cardinality, iteration, and conversion kernels retained; the first `is_empty`, `union`, `intersection`, and `difference` source extraction slice is [tracked by #59](https://github.com/vycdev/jett/issues/59), with follow-up required for `new`, `add`, `remove`, `contains`, `length`, and `to_list`) |
| `map` | Partial (17+ compiler-backed public operations are transitional technical debt; the first compositional source-extraction slice is [tracked by #61](https://github.com/vycdev/jett/issues/61), with follow-up required until every public declaration is source-owned and only private storage, equality, lookup/update, and iteration kernels remain runtime-backed) |
| `math` | Partial (`is_even`, `is_odd`, `sign`, `to_radians`, `to_degrees`, and consuming monomorphic `sum(list[int64])` are source-defined in `stdlib/math.jett`; `sum` uses checked source `int64` addition; `mod` and `pi` remain primitive Rust kernels used by source helpers; other supported operations such as abs, sqrt, pow, floor, ceil, round, clamp, log, log2, log10, min, max, average, median, e, sin, cos, and tan remain Rust-backed) |
| `json` | Partial (json.serialize, json.serialize_public, json.parse_exact, json.parse_raw/JsonValue accessors, compiler-owned public policy for parse/serialization; interpreter `json.parse`, `json.parse_exact`, `json.serialize`, and `json.serialize_public` require trusted stdlib-loaded reflected `.jett` hooks under `namespace json`; typed `json.parse[T]` routes through the stdlib `JsonTree` parser/decoder, including the `json.parse[JsonValue]` compatibility branch, while `json.parse_exact[T]` rejects unknown object fields recursively; stdlib `JsonTree` has a view-native serializer, scalar/array/object parser, exported raw facade wrappers, traversal/scalar-cast helpers, reflected parse/serialize for machine state/payload envelopes, and reflected decoding via `TypeConstruction` for nested structs, enum-annotated bitfields, enums, machines, lists/maps/sets, optionals/results, bytes, sized integer/float primitives, null, secret wrappers, aliases/refinements, and missing optional-field defaults; raw `JsonValue` parsing/access now runs on native `JsonTree` values through trusted stdlib facades with a shared `jett_common` JSON-facade policy for runtime dispatch, trusted hook mapping, and implicit view ownership, typechecker raw facade signatures now come from the exported `json.JsonTree` stdlib surface, with bare `JsonValue` preserved by the stdlib-only root alias to bundled `json.JsonTree`, using checked reflection metadata for direct type reflection builtins, `TypeInfo`, `type.arg`, trusted `comptime type` bindings for args/field/variant/machine-state loops, `TypeField`, `type.field_value`, bitfield metadata, machine metadata, active machine state/field access, enum variant metadata, `type.variant_value`, `type.variant_field_value`, `type.construct_variant_start`, `type.construct_machine_start`, `type.construct_put`, `type.construct_finish`, the runtime `main()` interpreter, and the `json.serialize` secret-containing type gate, with fallback-path audit complete, canonical `TypeId` metadata lookup scaffolding expanded to owner fields/bitfields/machines/variants, and missing checked owner metadata now surfaced for fields/bitfields/machines/variants) |
| `random` | Partial (5 hardcoded capability-free builtins: int64, float64, bool, choice, shuffle; the proposed contract adds explicit `view Random`, unbiased value/collection semantics, deterministic runtime-provider injection, no public seed or cryptographic claim, and source-owned public declarations over private generator kernels; see [#67](https://github.com/vycdev/jett/issues/67) and the [random contract](open_design/random_capability_entropy_contract.md)) |
| `crypto` | Partial (hardcoded UTF-8-to-lowercase-hex `sha256` and legacy-only `md5`; the proposed contract pins their compatibility/security policy, reserves SHA-512 and key-first binary HMAC shapes, and requires source-owned public declarations over private runtime kernels; see [#69](https://github.com/vycdev/jett/issues/69) and the [crypto contract](open_design/crypto_hashing_security_contract.md)) |
| `encoding` | Partial (6 hardcoded `string -> string` builtins: base64_encode, base64_decode, hex_encode, hex_decode, url_encode, url_decode; the proposed contract migrates Base64/hex to strict byte-native codecs, makes every decoder fallible, separates URL and form components, and requires source-owned public declarations over private runtime kernels; see [#71](https://github.com/vycdev/jett/issues/71) and the [encoding contract](open_design/encoding_representation_failure_contract.md)) |
| `bytes` | Partial (9 builtins: new, length, slice, concat, from_string, to_string, get, to_hex, from_hex) |
| `uuid` | Partial (`uuid.new`; generation and entropy contract [tracked by #73](https://github.com/vycdev/jett/issues/73)) |
| `time` | Partial (transitional ambient `time.now_ms` and `time.now_s` builtins; the proposed contract selects capability-backed `Clock.now(view clock) -> time.Timestamp`, distinct signed-millisecond `Timestamp`/`Duration` values, deterministic clock injection, and removal of the ambient builtins; see [#75](https://github.com/vycdev/jett/issues/75) and the [time/Clock contract](open_design/time_clock_capability_contract.md)) |
| `os` | Partial (`os.env`, `os.args`; `Environment`/argv capability and public stdlib/runtime boundary [tracked by #94](https://github.com/vycdev/jett/issues/94)) |
| `net.http` | Not started (initial outbound client and `Network` capability contract [tracked by #101](https://github.com/vycdev/jett/issues/101)) |
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
| `jett run [--agent] file.jett` | Working (tree-walking interpreter; `--agent` captures stdout plus typed trace/breakpoint debug rows) |
| `jett test [--agent] [file.jett]` | Working (verify + property blocks; `--agent` emits compact block tables) |
| `jett lsp` | Working (diagnostics on save) |
| `jett bind header.h` | Not started (tracked by [#53](https://github.com/vycdev/jett/issues/53)) |
| `jett bundle` | Working (resolver-derived whole-file dependency ordering, stable lexical tie-breaking, structured cycle and namespace-boundary diagnostics, line manifests, and validation-before-write) |
| `jett mcp` | Not started |
| `jett query --agent --namespaces` / `--symbols file.jett` / `--type-at file:line:column` / `--definition-at file:line:column` / `--references-at file:line:column` / `--complete-at file:line:column` / `--signature name` | Partial (ranged namespace registry, file-local symbols with declaration ranges and function signatures, ranged type lookup with structured compiler failures and cross-file labels when source context is known, ranged definition lookup, ranged reference lookup, ranked prefix-filtered completion candidates with context metadata and ranges, and source-level function signatures) |
