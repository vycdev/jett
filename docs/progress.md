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
| Parser (recursive descent + Pratt, direct source-spanned AST) | `jett_parser` | 67 | Done; lossless CST-to-AST frontend is [planned later](active/frontend_syntax_tree_staging.md) |
| Formatter (canonical whitespace) | `jett_fmt` | 8 | Done |
| Pipeline orchestration | `jett_driver` | 36 | Done |
| CLI (format, build, run, test, lsp) | `jett_cli` | 0 | Done |

### Phase B: Type System Core — COMPLETE

| Component | Crate | Tests | Status |
|---|---|---|---|
| Type interning, struct/enum/interface defs | `jett_types` | 18 | Done |
| Name resolution, scoping, namespace export visibility | `jett_resolve` | 27 | Done |
| Type checking (expressions, operators, generics) | `jett_typecheck` | 117 | Done; checked output now includes an ordered, deduplicated concrete generic-instantiation manifest with per-instantiation expression and nested-call facts |
| Module/import/prelude registry and backend-neutral trusted origin | `jett_project`, `jett_resolve`, later IR | n/a | Design selected by the [module and trusted-origin contract](completed/module_import_trusted_origin_contract.md); current block-local `use`, stdlib loading, and interpreter trust paths remain transitional |

### Phase C: Ownership and Capabilities — COMPLETE

| Component | Crate | Tests | Status |
|---|---|---|---|
| Ownership analysis (move/view/consume) | `jett_typecheck/ownership` | 9 | Done |
| Capability purity enforcement | `jett_typecheck/capability` | 7 | Done |
| Secret taint analysis | — | — | Partial (refined-secret taint fixed by [#18](https://github.com/vycdev/jett/issues/18); content-constant interpreter comparison for string/bytes secrets defined by [#33](https://github.com/vycdev/jett/issues/33), with native lowering pending [#20](https://github.com/vycdev/jett/issues/20) and [#22](https://github.com/vycdev/jett/issues/22)) |

### Phase D: Code Generation — STARTED

| Component | Crate | Tests | Status |
|---|---|---|---|
| HIR (monomorphization) | `jett_hir` | 12 | Typed ordinary and generic function lowering implemented for the core subset, with canonical identity, deterministic IDs, separate per-instantiation facts, named-argument normalization, concrete source-method targets, struct/list/map construction, field access, unhandled pipeline normalization, generic calls, and core structured control flow; pipeline handles and remaining constructs are staged by the [HIR lowering plan](active/hir_lowering_plan.md) and [#20](https://github.com/vycdev/jett/issues/20) |
| MIR (control flow graph) | `jett_mir` | — | Not started ([Tracked by #22](https://github.com/vycdev/jett/issues/22)) |
| LLVM native codegen | `jett_codegen_llvm` | — | Not started |
| Runtime library | `jett_runtime` | — | Not started |
| Core stdlib (.jett files) | `stdlib/` | — | Partial (bootstrap loader plus extracted `json`, complete source-owned `list`, `map`, `set`, `string`, `math`, `random`, and `time` public APIs, and other modules that remain Rust-backed) |

### Phase E: Comptime and Verification — COMPLETE

| Component | Crate | Tests | Status |
|---|---|---|---|
| Comptime interpreter | `jett_comptime` | 137 | Done |
| Explicit `comptime expression` evaluation | `jett_comptime`, `jett_driver` | — | Done (closed pure expressions are evaluated during the build and their values are consumed by runtime interpretation) |
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
| State machines (`machine`, transitions, `at`) | Done (checked state-qualified types with explicit bare-annotation erasure, canonical `Machine.transition(...)` calls without generated alternatives, exact-state-only and local-variable-only branch narrowing, no implicit union-state or path facts, namespaced machines, reflection metadata, reflected machine construction, and JSON parse/serialize through the explicit state/payload envelope) |
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
| Complete source-owned `list` API with typed `list.Pair[A, B]` and `list.Indexed[T]` | Done |
| Complete source-owned `map` API and typed `map.Entry[K, V]` | Done |
| Complete source-owned `set` API | Done |
| Complete source-owned `encoding` API: byte-native Base64/hex plus URL/form components | Done |
| Byte-native hex helpers (`bytes.to_hex`, `bytes.from_hex`) | Done |
| Complete source-owned `bytes` API with move-only/view semantics | Done |
| Closure captures (inline functions capture immutable enclosing scope) | Done |
| Function type expressions (`function(T) returns U` in type annotations) | Done |
| Dotted `use` paths (`use net.http`) | Done |
| Strict inline imports for project/vendored namespaces (`E0210`; same-namespace and declaration-signature type access remain direct; compiler stdlib remains implicit pending prelude policy) | Done |
| Namespace-local global constant initializers (`E0211`; cross-namespace project/vendored dependencies are forbidden) | Done |
| Conservative generic reflection facts (predicate calls and detached booleans never authorize casts or skip branch checking; post-check folding is optimization only) | Done |
| Namespace-qualified user function calls (`helpers.f()`, `helpers.f[T]()`) | Done |
| Namespace-private declarations with explicit `export` for public APIs, including `mutual` declarations and qualified-only external access | Done |
| Multi-file compilation (project-aware build/run with `jett.proj`) | Done |
| `range()` builtin (1, 2, or 3 args) | Done |
| For-in over strings, maps (with `key, value` destructuring), sets | Done |
| `and`/`or` keyword operators for logical expressions | Done |
| Unhandled result/optional detection (E0341, E0342) | Done |
| Set value type and 12 set builtins (`new`, `add`, `remove`, `contains`, `union`, `intersection`, `difference`) | Done |
| `print`/`println` builtins | Partial (interpreter support, secret blocking, and E0362 release diagnostics are done; [debug-only capability policy](open_design/print_debug_builtin_policy.md) is decided, while debug-event isolation and future-backend conformance remain pending) |
| Type conversions: `int64.from_float64`, `float64.from_string`, `string.from_float64`, `string.from_bool` | Done |
| `time.now_ms`, `time.now_s` | Removed (replaced by explicit `Clock.now`) |
| `os.env`, `os.args` | Removed (replaced by explicit `Environment.get` and `Environment.args`, with focused migration diagnostics) |
| Math: `pi`, `e`, `sin`, `cos`, `tan`, `mod`, `is_even`, `is_odd`, `sum` | Done |

### Phase H: Agent Tooling — PARTIAL

> Tracked by [#35](https://github.com/vycdev/jett/issues/35) for the remaining
> structured diagnostic context and agent-mode failure envelope.

| Component | Status |
|---|---|
| TOON output (`--agent` flag) | Partial (build diagnostics include file, ok/error status, severity counts, ranged diagnostics/labels, and tabular suggested fixes; format status, run stdout/typed debug output, ranged verify/property test summaries, namespace/symbol/type-at/definition-at/references-at/completion/signature query results, file-symbol parse failures, type-at compiler failures, and definition-at/references-at parse/resolution failures with known source context and cross-file labels are structured) |
| LSP server (diagnostics on save and whole-document formatting) | Done |
| LSP hover (type at cursor) | Done |
| LSP go-to-definition | Done |
| LSP find references | Done (current document, with optional declaration inclusion) |
| LSP completions | Done |
| LSP document symbols | Done (top-level file outline from the latest in-memory document, with declaration kinds, signatures, and UTF-16 ranges) |
| MCP server | Not started; initial transport, tool/resource, and ASP handoff boundary tracked by [#37](https://github.com/vycdev/jett/issues/37) |
| ASP query system | Partial (`jett query --agent --namespaces`, `--symbols`, `--type-at`, `--definition-at`, `--references-at`, prefix-filtered `--complete-at`, and `--signature` are implemented; namespace, type, symbol, definition, reference, and completion rows include source ranges; file-symbol parse failures, type-at parse/resolution/type-check failures, and definition-at/references-at parse/resolution failures with known source context preserve structured diagnostics and cross-file labels; completion rows also include deterministic rank, match kind, namespace, visibility, and source-level signatures where available) |

### Phase I: Testing and Profiling — PARTIAL

| Component | Status |
|---|---|
| Property-based test runner | Done (basic: 100 generated iterations; all numeric primitives, bool/string/bytes/nothing, aliases/refinements, structs including generic structs, bitfields, enums, plus generic list/set/map/optional/result pools) |
| Input shrinking on failure | Done (shrinking for int64, float64, string, bytes, list, set, map, optional, result, struct fields, enum payloads) |
| CPU profiler (`--profile`) | Initial backend-neutral configuration, sample aggregation, exact thresholding, deterministic ranking, and suggestion rules implemented in `jett_profiler`; CLI, rendering, and runtime sampling remain staged by the [profiling contract](completed/cpu_memory_profiling_contract.md) |
| Memory profiler (`--profile-memory`) | Design complete; implementation not started ([allocation, resize/free, retention, peak-memory, attribution, and runtime contract](completed/cpu_memory_profiling_contract.md)) |
| `trace` keyword | Partial (parses, typechecks, runtime type-tagged current-value output in `jett run`) |
| `breakpoint` keyword | Partial (parses, typechecks, and emits conditional runtime debug snapshots with visible binding types in `jett run`; the pause/inspection protocol is [decided](completed/breakpoint_pause_inspection_protocol.md), while its interpreter and future native-runtime stages remain unimplemented) |

### Phase J: Cross-Platform and Interop — PARTIAL

| Component | Status |
|---|---|
| Cross-compilation (`--target`) | Not started |
| C binding generator (`jett bind`) | Not started (initial syntax, safety boundary, supported subset, and staged implementation specified by the [C FFI binding contract](open_design/c_ffi_binding_contract.md) from [#53](https://github.com/vycdev/jett/issues/53); the foreign declaration frontend prerequisite is tracked by [#173](https://github.com/vycdev/jett/issues/173), while generator/CLI work remains pending) |
| `jett bundle` | Working (resolver-derived whole-file dependency ordering, stable lexical tie-breaking, structured cycle and namespace-boundary diagnostics, line manifests, and validation-before-write) |

### Phase K: Full Standard Library — NOT STARTED

| Module | Status |
|---|---|
| `string` | Done (all public declarations and compositional behavior are source-owned in `stdlib/string.jett`; only private trusted conversion, Unicode, grapheme, search, and text primitive kernels remain, and project code cannot call them; count/index/search/extraction helpers avoid partial grapheme matches) |
| `list` | Done (all public declarations and compositional behavior are source-owned; observers use views, transformations consume inputs, `zip`/`enumerate` return typed records, global `range` is canonical, and only private trusted allocation/indexing/mutation/sorting/sum/callback kernels remain) |
| `set` | Done (all public declarations and set algebra are source-owned; only private trusted storage/equality/cardinality kernels remain) |
| `map` | Done (all public declarations, conversions, and higher-order operations are source-owned with typed `map.Entry[K, V]`; only private trusted storage/equality/lookup-update kernels remain) |
| `math` | Done (all public declarations are source-owned in `stdlib/math.jett`; compositional helpers have Jett bodies, integer operations share the language's wrapping semantics and nonzero-divisor proof rule, while private trusted kernels preserve floating-point primitives, constants, exact numeric collection behavior, and remaining domain failures; project code cannot call the kernels, and the closed `abs`/`min`/`max` `int64`/`float64` call policy remains compiler-enforced without creating general overloading) |
| `json` | Partial (json.serialize, json.serialize_public, json.parse_exact, json.parse_raw and raw-tree accessors, compiler-owned public policy for parse/serialization; interpreter entrypoints require trusted stdlib-loaded reflected `.jett` hooks under `namespace json`; typed parsing routes through the stdlib `json.JsonTree` parser/decoder and exact parsing rejects unknown object fields recursively; reflected construction covers nested structs, enum-annotated bitfields, enums, machines, collections, wrappers, bytes, sized numeric primitives, null, secrets, aliases/refinements, and missing optional-field defaults; `json.JsonTree` is the sole raw representation and both former `JsonValue` aliases are rejected; checked reflection metadata feeds direct reflection, construction, runtime execution, and secret serialization policy) |
| `random` | Done for the interpreter-backed compiler (all 5 public declarations are source-owned in `stdlib/random.jett`; every call borrows an explicit `Random`, integer sampling is half-open and unbiased, choice/shuffle preserve borrowed inputs, production state is runtime-injected, typed scripted samples make tests deterministic, and only private trusted sampling kernels remain; concurrent cancellation/clone sharing and later backends retain handoff obligations in the [random contract](completed/random_capability_entropy_contract.md)) |
| `crypto` | Done for the implemented text-digest surface (`sha256`, `sha512`, and legacy-only `md5` are source-owned in `stdlib/crypto.jett`; wrappers use exact UTF-8 bytes and lowercase byte hex, only private raw-digest kernels remain, secret lifting is preserved, and HMAC stays reserved and undiscoverable per the [crypto contract](completed/crypto_hashing_security_contract.md)) |
| `encoding` | Done for the interpreter-backed compiler (all 8 public declarations are source-owned in `stdlib/encoding.jett`; Base64/hex operate on arbitrary bytes, all decoders return stable handled errors, URL and form component semantics are distinct, project code cannot call private kernels, and the future-backend handoff is recorded in the [encoding contract](completed/encoding_representation_failure_contract.md)) |
| `bytes` | Done (all 9 public declarations are source-owned in `stdlib/bytes.jett`; observers use read-only views, `slice` returns independent owned bytes, `concat` consumes both inputs, and only private trusted raw-byte and UTF-8/hex kernels remain) |
| `uuid` | Partial (`uuid.new`; generation and entropy contract [tracked by #73](https://github.com/vycdev/jett/issues/73)) |
| `time` | Done for the interpreter-backed compiler (`Clock.now(view clock)` reads an injected wall clock; `time.Timestamp` and `time.Duration` plus conversions, comparison, difference, and checked arithmetic are source-owned in `stdlib/time.jett`; deterministic raw clock samples cover pre-epoch flooring, backward movement, provider failure, exhaustion, and range checks; ambient clock builtins are removed with migration diagnostics; later backends retain the [time/Clock contract](completed/time_clock_capability_contract.md)) |
| `Environment` / `os` | Done for the interpreter-backed compiler (`Environment.get(view env, key)` and `Environment.args(view env)` are source-owned over an immutable launch snapshot; production and deterministic test contexts inject launch data; missing, invalid-name, invalid-value, argument ordering/empties, independent-list, and capability restrictions are covered; private kernels cannot be called by project code; ambient `os.env`/`os.args` are removed with migration diagnostics; later backends retain the [Environment and argument contract](open_design/environment_argv_capability_contract.md), implemented by [#170](https://github.com/vycdev/jett/issues/170)) |
| `net.http` | Not started (initial outbound client and `Network` capability contract [tracked by #101](https://github.com/vycdev/jett/issues/101)) |
| `net.socket` | Partial (TCP-first transport contract completed by [#104](https://github.com/vycdev/jett/issues/104); compiler-shipped `resource Name` declarations now have lexer/parser/AST, trusted-stdlib resolver, nominal type, reflection, formatter, query, and LSP support, including project-origin and clone rejection; runtime handle storage, exactly-once cleanup, interpreter registry, authority provenance, and backend handoff remain pending under the [opaque runtime resource contract](completed/opaque_runtime_resource_contract.md) from [#175](https://github.com/vycdev/jett/issues/175); see [`docs/open_design/net_socket_transport_contract.md`](open_design/net_socket_transport_contract.md)) |
| `csv` | Done for the interpreter-backed compiler (all 3 public declarations are source-owned in `stdlib/csv.jett`, with only private trusted parse/stringify kernels in the interpreter; parsing returns `result[..., string]`, ignores one leading UTF-8 BOM, treats empty input as zero records, preserves blank records, whitespace, quoted data, Unicode, LF/CRLF endings, and ragged raw rows; malformed quoting, bare CR record endings, invalid headers, and header/data width mismatches fail explicitly; `stringify` emits canonical LF-separated records with no final newline; future backends retain the [CSV format and failure contract](completed/csv_format_failure_contract.md)) |
| `regex` | Design selected, implementation not started (pure one-shot `is_match`, `find`, `captures`, and non-overlapping `find_all`; an exact grammar/error map, Unicode 17.0.0 grapheme/fold manifest, grapheme-indexed spans, canonical checked NFA sizing, a deterministic execution-work cap, structured values, and private trusted kernels are fixed by the [regular expression contract](completed/regex_matching_extraction_contract.md) from [#140](https://github.com/vycdev/jett/issues/140); replacement, splitting, Unicode property classes, and public compiled patterns remain deferred) |
| `log` | Design selected, implementation not started (dedicated `Log` capability; source-owned event, level, field, and error types; eager runtime filtering; checked non-wrapping sequence allocation; complete `FileKey` source identity without physical-root leakage; ordered deterministic JSON records; compiler-enforced secret rejection; isolated injected sinks and captures; exact `RunOutput`/TOON channel composition; see the [structured logging contract](completed/structured_logging_contract.md) from [#143](https://github.com/vycdev/jett/issues/143)) |
| `test.mock` | Not started (the property-only source facade, typed provider adapters, exact scripts, isolation, replay/shrinking boundary, and future-backend obligations are defined by the [capability mocking and deterministic test harness contract](completed/capability_mocking_test_harness_contract.md) from [#145](https://github.com/vycdev/jett/issues/145)) |

### Phase L: Incremental Compilation — NOT STARTED

| Component | Status |
|---|---|
| Salsa integration | Initial whole-file parse-query slice implemented (the first `jett_query` boundary memoizes parser-owned direct ASTs by stable logical file identity; see the [initial query and invalidation boundary](open_design/incremental_query_boundary.md) from [#147](https://github.com/vycdev/jett/issues/147), with implementation tracked by [#166](https://github.com/vycdev/jett/issues/166)) |
| Parallel compilation | Design selected, implementation not started (bounded parallel parsing first; namespace/body scheduling follows stable declaration facts; see the [deterministic parallel compilation boundary](open_design/parallel_compilation_boundary.md), tracked by [#151](https://github.com/vycdev/jett/issues/151)) |
| Content-addressed caching | Initial canonical parse-key codec implemented (SHA-256 identity, exact v1 binary records, strict decoding, and current-source validation); parse artifact serialization, authenticated storage, read-through integration, atomic publication, and bounded cleanup remain pending under the [content-addressed compilation cache contract](completed/content_addressed_compilation_cache_contract.md) from [#153](https://github.com/vycdev/jett/issues/153) |

## VS Code Extension

| Feature | Status |
|---|---|
| Syntax highlighting (TextMate grammar) | Done |
| Language configuration (brackets, indentation) | Done |
| LSP integration (diagnostics and formatting) | Done (via `jett lsp`) |

## CLI Commands

| Command | Status |
|---|---|
| `jett format [--agent] [--check] file.jett` | Working |
| `jett build [--agent] [--release] [--target T] file.jett` | Working (validates, no binary output) |
| `jett run [--agent] file.jett` | Working (tree-walking interpreter; `--agent` captures stdout plus typed trace/breakpoint debug rows) |
| `jett test [--agent] [file.jett]` | Working (verify + property blocks; `--agent` emits compact block tables) |
| `jett lsp` | Working (diagnostics on save) |
| `jett bind header.h` | Not started (contract specified in the [C FFI binding contract](open_design/c_ffi_binding_contract.md) from [#53](https://github.com/vycdev/jett/issues/53); the foreign declaration frontend prerequisite is tracked by [#173](https://github.com/vycdev/jett/issues/173), while generator/CLI work remains pending) |
| `jett bundle` | Working (resolver-derived whole-file dependency ordering, stable lexical tie-breaking, structured cycle and namespace-boundary diagnostics, line manifests, and validation-before-write) |
| `jett mcp` | Not started |
| `jett query --agent --namespaces` / `--symbols file.jett` / `--type-at file:line:column` / `--definition-at file:line:column` / `--references-at file:line:column` / `--complete-at file:line:column` / `--signature name` | Partial (ranged namespace registry, file-local symbols with declaration ranges and function signatures, ranged type lookup with structured compiler failures and cross-file labels when source context is known, ranged definition lookup, ranged reference lookup, ranked prefix-filtered completion candidates with context metadata and ranges, and source-level function signatures) |
