# Native Code Generation Parity Plan

Status: accepted implementation strategy; checked-program, HIR, and MIR
prerequisite work is in progress.

This plan defines when Jett may claim native-code parity with the current
interpreter-backed language. It fixes the initial backend, the prerequisite IR
contracts, and measurable acceptance gates. Native execution must implement
accepted Jett semantics; it may not become a second source-language policy
layer or silently delegate runtime behavior to the interpreter.

## Scope and Baseline

The current fixture inventory establishes three separate denominators:

| Obligation | Denominator | Acceptance condition |
| --- | ---: | --- |
| Native lowering | 182 | Every `tests/run_pass/*.jett` fixture reaches validated HIR and MIR and is accepted by native object generation. |
| `main` execution | 30 | Every run-pass fixture that declares `main` links and runs on the supported host with its interpreter-equivalent expected outcome and observable behavior. One scripted graphics fixture intentionally returns a runtime error. |
| Runtime contracts | 25 | Every `tests/runtime_fail/*.jett` fixture links and matches its interpreter contract: 17 wrapping-success cases and 8 runtime-failure cases, including failure class, message contract, and cleanup behavior where applicable. |

These numbers are denominators, not a sample or a percentage target. Fixtures
added before parity is declared extend the applicable denominator. A fixture
may leave it only when the corresponding language or runtime feature is
explicitly marked unimplemented, with the reason recorded in the same change.

Run-pass files without `main`, including verification and property fixtures,
count toward the 182-fixture lowering obligation. The initial executable
harness does not execute their `verify` blocks natively, so they do not count
toward the 30-fixture execution obligation. Passing them through frontend
verification is not evidence that their bodies executed as native code. A
future native verification harness may add a separate execution denominator.

Compile-fail fixtures remain frontend contracts. Native work must not change
their diagnostics, but rejected programs do not enter a backend denominator.
Required comptime evaluation remains a compile-time operation; a generated
program may not fall back to the interpreter for runtime semantics.

The only language or runtime feature exclusions are features that the current
project records explicitly as unimplemented. Implemented portions of partial
features remain in scope, even when finishing their native representation is
difficult. Host-only output, optimization level, and debug-information quality
are backend staging limits rather than semantic feature exclusions.

## Backend Strategy

The first production path is a host ahead-of-time object backend using
Cranelift. The Cranelift crates are pinned exactly to `0.132.3`, whose Rust
version requirement is compatible with the repository's current Rust
`1.93.1` toolchain:

```toml
cranelift-codegen = { version = "=0.132.3", default-features = false, features = ["std", "unwind", "x86", "arm64"] }
cranelift-frontend = { version = "=0.132.3", default-features = false, features = ["std"] }
cranelift-module = { version = "=0.132.3", default-features = false, features = ["std"] }
cranelift-object = { version = "=0.132.3", default-features = false }
target-lexicon = "=0.13.3"
```

The selected feature set covers the first x86-64 host and the intended AArch64
host follow-up without enabling unrelated ISA backends. The implementation
change will record the same exact versions and features in `Cargo.toml`; this
plan does not itself add dependencies.

Cranelift produces a target object, not an in-memory-only JIT result. Jett then
links that object with a target-matched Jett runtime and the host system
libraries to produce the executable. This keeps ordinary builds independent of
an installed LLVM development package and gives the first backend a bounded,
portable Rust dependency surface.

LLVM remains the later optimizing backend. It must consume the same validated
MIR, layouts, intrinsic identities, runtime ABI, and conformance suite rather
than reimplementing language decisions. LLVM is not on the critical path for
the first full-parity claim. Adding it later must not weaken the Cranelift
denominators or make backend selection observable in program semantics.

A C transpiler may be useful as a diagnostic oracle, but it is not the accepted
production path. Direct instruction and object emission is also out of scope;
Jett will not independently implement instruction selection, register
allocation, unwind metadata, and each platform ABI.

## Host-Only First Slice

The first slice supports only the compiler host target, initially
`x86_64-pc-windows-msvc`. An omitted target selects that host. An explicit
target is accepted only when it is exactly the supported host target.

Any other target must produce an explicit unsupported-target diagnostic before
object emission or linking. Jett must never ignore `--target`, substitute host
layouts for a requested target, or emit an object that the selected linker
cannot consume. Target validation covers the Cranelift ISA, pointer width,
data layout, calling convention, object format, runtime library, linker, and
host SDK. Cross-compilation is added only after all of those inputs are
target-specific and tested.

## Required Compiler Handoff

Native code generation begins only after the checked-program, HIR, and MIR
gates below pass. A backend adapter may reject a genuinely unsupported target;
it may not recover missing semantic facts by inspecting source spellings or
re-running frontend policy.

### Checked-program gate

The driver must publish one coherent checked-program artifact that keeps the
facts needed by every downstream phase together. At minimum it carries:

- explicit `SourceOrigin` and canonical declaration and function identities;
- deterministic concrete generic and method call targets;
- function parameters, locals, checked expression and definition types, and
  the type interner or a closed canonical type table;
- aggregate, enum, machine, bitfield, recursive-value, and resource layout
  inputs;
- trusted-stdlib and compiler-intrinsic identities as checked identities, not
  source-name tests; and
- reflection, capability, and required comptime results used by runtime
  lowering.

Session-local IDs may join facts inside one checked program. Persistent
artifacts and symbols use canonical identities and structural types, never raw
`FileId`, `DefId`, or `TypeId` indices. The gate fails on an unresolved call,
missing type, missing origin, or ambiguous trusted operation.

### HIR identity gate

Validated HIR must represent every accepted runtime construct and preserve the
checked identity of every direct call, method, intrinsic, resource operation,
and trusted hook. It makes lexical evaluation order and concrete generic
instantiation explicit. It does not retain an AST fallback that a backend must
reinterpret.

The 182-fixture lowering denominator first passes through this gate. Any
unsupported-HIR error for one of those fixtures is a parity failure, not an
allowed skip.

### MIR control-flow and ownership gate

Validated MIR is the sole semantic input to native code generation. Before it
is backend-ready, it must make all branches, loops, handlers, failure paths,
calls, and returns explicit basic-block control flow. Operands and locals have
complete types and layouts, and each runtime operation has a typed intrinsic or
function identity.

Definitive ownership dataflow and drop elaboration also belong here. The MIR
validator must prove moves, copies, and views follow the checked value category
and that every owned value and resource is cleaned up exactly once on normal,
early-return, handled-failure, and runtime-failure edges. Code generation is
blocked when an ownership state, cleanup edge, or layout remains implicit.

## Versioned Native Runtime ABI

The native runtime is packaged behind a versioned C ABI. It is an internal
compiler/runtime contract, but version mismatches must fail deterministically
rather than rely on Rust layout or symbol compatibility.

- Exported runtime symbols carry an ABI version namespace or are accompanied by
  a required ABI-version symbol. Objects and runtime libraries record the same
  version.
- The boundary uses fixed-width integers, C-compatible records, pointers with
  explicit lengths, opaque handles, status codes, and out parameters.
- Rust `String`, `Vec`, `Result`, trait objects, closures, `Any`, enum layout,
  and other Rust-ABI values never cross the boundary.
- Ownership is explicit for every pointer and handle: borrowed inputs have a
  bounded lifetime, transferred inputs identify the new owner, and created
  values have one matching retain, release, or drop contract.
- Runtime context is passed explicitly for capabilities, allocation, resource
  registries, scheduling, diagnostics, and other per-program state. Native
  code must not acquire undeclared ambient authority.
- The Cranelift ABI realizes that rule as one backend-private, pointer-sized
  first parameter on every emitted Jett function. It is absent from source,
  HIR, and MIR arity; direct Jett calls forward the caller's exact pointer,
  and the exported `jett_aot_v1_entry(void *context)` wrapper passes its input
  to the checked program entry without reconstructing or inspecting it.
- Panics and Rust unwinding may not cross the ABI. Runtime operations translate
  failure to the stable status and error-value contract; native cleanup paths
  remain responsible for owned Jett values. If entry execution and final
  runtime-context destruction both fail, the cleanup/infrastructure failure is
  the terminal process outcome, while diagnostics retain both failures.
- Host layout tests pin every shared record's size, alignment, field offsets,
  tag values, and calling convention. Backend and runtime use one canonical
  definition of those layouts.

The compiler maintains one typed intrinsic registry shared by type checking,
the interpreter mapping, HIR/MIR lowering, and runtime ABI adapters. The
closed `IntrinsicId` set is the sole acceptance boundary for compiler-owned
call spellings. Checked call sites carry that ID into HIR and MIR; downstream
dispatchers match it exhaustively so adding an intrinsic requires an explicit
decision in every execution path. Source names remain only diagnostic and
symbol metadata. This prevents duplicated string dispatch and preserves
trusted operation identity through codegen.

## Artifacts, Linking, and Packaging

The host AOT pipeline is:

```text
checked program -> validated HIR -> validated MIR -> host object
                -> host linker + versioned runtime -> executable
```

The driver carries the primary source file's checked `main` as an exact
`FunctionId` in its backend-lowering result. Object emission and native test
harnesses consume that identity directly; they never rediscover the entry by
scanning source names or native symbols.

The implementation must provide all of the following before the pipeline is a
supported `jett build` result:

- deterministic native symbol mangling from canonical function identities;
- a host object with the correct target triple, object format, relocations,
  unwind information, and profile settings;
- a small entry shim that initializes runtime context, passes process inputs
  through declared capabilities, invokes Jett `main`, renders failures, and
  performs final cleanup;
- a target- and profile-matched runtime static library, its ABI metadata, and
  all required host system-library declarations;
- explicit linker discovery or configuration, safe argument construction,
  host SDK validation, and actionable diagnostics that identify a missing
  linker, SDK, runtime archive, or system library;
- an artifact layout that separates target and profile outputs, publishes only
  successful final files, and removes incomplete temporary products; and
- redistribution notices and runtime artifacts needed to run the executable on
  a clean supported host without the compiler source tree.

On the initial Windows host, the linker contract must be tested with the
documented MSVC-compatible linker and Windows SDK/UCRT inputs. Finding Clang or
LLVM utilities on one developer machine is not sufficient packaging. Clean-host
CI must exercise compiler build, object emission, runtime archive selection,
linking, and execution.

## Staged Acceptance Gates

Progress is monotonic: a later gate includes all preceding checks, and a
backend feature is not complete while its fixture is skipped or delegated to
the runtime interpreter.

1. **Baseline and harness:** freeze the three denominators, classify every
   fixture by required constructs and runtime hooks, and add differential
   harness support without changing current interpreter expectations.
2. **Checked-program handoff:** publish and validate the closed semantic
   artifact, canonical identities, concrete calls, layouts, and trusted
   intrinsic identities required downstream.
3. **Complete HIR lowering:** all 182 run-pass fixtures lower to validated HIR
   with no source-form or name-based backend fallback.
4. **Backend-ready MIR:** those same 182 fixtures lower to validated CFGs with
   explicit evaluation, failure, ownership, and cleanup behavior.
5. **Object and link slice:** Cranelift emits deterministic host objects for a
   scalar/control-flow seed, links them with the versioned runtime, and rejects
   unsupported targets and missing toolchain inputs explicitly.
6. **Runtime surface expansion:** add typed ABI operations and layouts by
   feature family while continuously re-running every already-supported
   fixture. Strings, aggregates, collections, generics, results and optionals,
   reflection and JSON, capabilities and resources, and concurrency/runtime
   services remain on the matrix until their current behavior is covered.
7. **Successful execution parity:** all 30 `main` fixtures compile, link, and
   execute natively with interpreter-equivalent stdout, stderr, result, visible
   capability effects, and cleanup behavior.
8. **Runtime-contract parity:** all 25 runtime-contract fixtures compile and
   link. The 17 wrapping-success cases return their accepted values; the 8
   failure cases fail through the native runtime with the accepted message
   contract and release every live owned value and resource exactly once.
9. **Full parity release gate:** all three denominators are complete with no
   skips other than explicitly unimplemented features; the full Cargo suite and
   clean-host native suite pass; artifacts run without the source tree; and the
   stable design, architecture, and progress documents are updated to describe
   the proven implementation.

The parity report must publish counts as `passed / denominator` for each of the
three obligations. A single percentage would hide the difference between code
that merely lowers, code that executes successfully, and code that preserves
failure semantics.

## Current Coverage Matrix

`tests/native_parity.json` is the machine-checked fixture inventory. The table
below records implementation coverage; a row is complete only when its native
object, linked execution, and differential behavior gates all pass. Typed
lowering alone never changes an execution row to complete.

| Surface | Validated HIR/MIR | Cranelift object | Linked native behavior |
| --- | --- | --- | --- |
| Fixed-width integers, floats, booleans, and `nothing` | covered | scalar expressions, direct calls, branches, and loops covered | pending executable harness |
| Strings and bytes | covered | all current string intrinsics, direct Unicode-scalar string iteration, separately owned bytes storage, and encoding leaves | string search/replace, Unicode case changes, scalar `for` loops, byte and encoding fixture functions, nested cleanup, moves/views/clones covered |
| Structs, enums, bitfields, machines, and refinements | covered | concrete user structs and enums with typed payloads; bitfield construction and fields; machine construction, transitions, state tests and state fields; transparent secret/refinement representation, `coarsen`, and base-value refinement predicates; struct-field and secret-backed refinement validation pending | struct moves/views/clones, nested owners, explicit equality and failure cleanup; enum construction, payload transfer, clone, match and cleanup, plus scalar payload-enum equality; bitfield field ownership, clone, byte roundtrip, and reflected-builder width validation; machine state narrowing, transitions and owned payload cleanup; native/interpreter predicate success and false-result diagnostics for integer and owned-string refinements; direct bitfield width validation, aggregate payload-enum equality and broader refinement boundaries pending |
| Lists, maps, and sets | covered | scalar/string/bytes/sum/nested lists, primitive and indexed-row list sorting, sortedness checks and sets, and primitive-backed refinement keys and set elements | compiled list access, insert/remove, reverse/repeat, scalar iteration and sort; stable `list.sort_by` and `list.group_by` with named callbacks; indexed-row sorting for interpreter-supported key types; set insert/remove/membership/clone and iteration; map literals, insert/remove/lookup/from_lists/clone and key-value iteration; borrowed iteration through nested struct-field collection paths; refinement string and integer keys and set elements; one contextual generic empty-list path covered; other projected views, collection shape conversions, and callback helpers pending |
| Numeric aggregates | covered | `math.average` and `math.median` for `list[int64]`, `list[uint64]`, and `list[float64]` | native/interpreter differential fixtures cover all three element types and overflow-safe floating-point extremes; empty-list error contract covered |
| CSV | covered | checked parse, parse-with-header, and stringify leaves over owned lists and maps | strict quoting, CRLF, header values/errors, and nested allocation cleanup match interpreter |
| Crypto | covered | private SHA-256, SHA-512, MD5, and HMAC-SHA-256 byte kernels | native differential fixture covers public text digests, binary HMAC, long keys, secret comparison, and explicit declassification |
| Secret values | covered | transparent scalar and owned representations; redaction and string/bytes comparison | native differential fixture covers equal, unequal, length-mismatched and Unicode strings, bytes, redaction, and aggregate ownership |
| Results, optionals, and `handle` control flow | explicit CFG for statement-root and direct-call-argument handlers | genuine tags, owned payloads and selected extraction | nested sums, defaults, early returns, loop exits, terminal bypass, and ordered direct-call arguments covered; other nested-expression and refinement handlers pending |
| Function values, closures, and indirect calls | covered; capture-free inline bodies extract to ordinary checked functions while captured closures retain an explicit unsupported form | named and capture-free inline function addresses plus indirect calls for supported signatures; captured closures and view-parameter function values pending | named and capture-free inline callbacks passed, returned, and invoked through indirect calls; a linked fixture covers zero-argument and nested capture-free functions; captured closures pending |
| Compiler intrinsics and reflection | covered with checked operands, source-aware reflection metadata, and closed `IntrinsicId` identities | `type.name`, `type.kind`, `type.has_secret`, `type.kind_tag`, `type.primitive_tag`, recursively constructed `type.info`, checked `type.arg`, struct/bitfield, enum, and machine metadata lists and layouts, active enum variant and machine state metadata, reflected field values, and checked reflected-type dispatch covered; other aggregate reflection pending | direct and generic scalar reflection, nested `TypeInfo`, indexed type arguments, struct/bitfield, enum, and machine metadata, active enum and machine state selection, reflected field values, and alias-aware `comptime type` dispatch match the interpreter on positive cases; alias probes remain empty as required; mismatch diagnostics and other aggregate reflection pending |
| Capabilities and runtime resources | nominal checked types covered | explicit Stdout, Clock, Random, and Environment entry grants; others pending | Stdout output, Clock/Random sampling, and immutable Environment launch snapshots covered; exact-consumption checks and other providers/resources pending |
| Actors and structured concurrency | covered | pending | pending |
| JSON and trusted stdlib hooks | covered | checked `JsonTree` calls use trusted raw stdlib functions; supported structs, machines, collections, and top-level refinements specialize the checked source serializer, including public omission of direct secret fields; supported top-level enums use dedicated checked source hooks; primitive parse calls use private source decoders; concrete structs, bare and state-qualified machines, supported secret wrappers, top-level refinements over supported bases, lists, sets of supported hashable primitives, string-keyed maps, optionals, and results specialize the checked source parser; other JSON shapes remain pending | native/interpreter fixtures cover raw trees, primitive and structured serialization, enum unit and payload values, machine state envelopes and public secret omission, primitive and structured parsing, top-level refinement parsing and serialization, secret-bearing records and machines, exact validation, renamed fields, aliases, nested collections, result branches, errors, and owned cleanup; other concrete JSON types pending |
| Trace, breakpoint, assert, and failure reporting | covered | `int64` trace and zero- or one-binding `int64` breakpoints covered; other trace/breakpoint shapes and assert pending | `int64` trace and breakpoint debug lines match interpreter stderr, including false conditions and an out-of-scope local; other instrumentation pending |

The current fixture gates are therefore:

| Obligation | Passing | Denominator | Evidence |
| --- | ---: | ---: | --- |
| Typed backend lowering | 182 | 182 | `run_pass_backend_lowering_gaps_are_explicit_and_monotonic` |
| Native object generation | 141 | 182 | exhaustive Windows MSVC 207-row checkpoint; original 29 staged deterministic manifest gates retained |
| Successful/expected `main` execution | 23 | 30 | Windows MSVC production linking and exact interpreter stdout/debug-output comparison, including scripted Clock, Random, and Environment inputs |
| Runtime contracts | 25 | 25 | exhaustive runtime-contract probe, including scripted Clock and Random failures; matched behavior and checked native-value cleanup |

These counts track fixture gates, not a weighted percentage of Jett syntax or
runtime semantics: fixtures differ in size, overlap, and coverage. The object
gate is currently 144/182 (79.1%), a useful progress measure rather than a
claim that 77.5% of the language has native support.
Machine `TypeConstruction` now validates checked state and payload metadata,
builds the selected tagged record, and checks state-qualified targets at
finish. This adds `type_construction_machine.jett` to the object gate; a linked
fixture checks construction and handled owner, member, and missing-field errors.
Enum `TypeConstruction` now validates the checked `TypeVariant` and payload
field metadata, builds the selected tag and fields, and returns handled
owner, member, and missing-field errors. This adds native objects for
`type_construction_enum.jett` and `json_tree_reflection_construction.jett`.
Bitfield `TypeConstruction` now uses the native builder with checked field
metadata and width validation at finish, adding
`type_construction_bitfield.jett` to the object gate. A linked fixture compares
successful reconstruction and handled width errors with the interpreter.
Typed payload enum equality compares integer, boolean, floating-point, and
string fields for the selected variant, adding native objects for
`namespace_dotted_qualified_types.jett` and `namespace_qualified_types.jett`.
Aggregate payload comparison remains unsupported.
Active `type.variant_value` selection matches native/interpreter output for
payload and empty variants. Reflected `type.field_value` and
`type.variant_field_value` now read checked struct, bitfield, and enum payload
fields in dedicated native/interpreter fixtures, including owned and secret
payloads. Metadata mismatches fail safely, but their exact diagnostics and
alias-equivalence behavior still need interpreter parity.
Owned struct-field reads deep-clone the selected value while projected views
remain borrowed. This adds `generic_reflection_branch_specialization.jett` and
`generic_reflection_match_specialization.jett` to the object gate; other affected
reflection fixtures advance to later unsupported intrinsics.
Reflected field reads add object gates for `bitfield_uint64_reflection.jett`,
`json_reflection_flat_serializer.jett`, `type_reflection.jett`, and
`json_tree_reflection_variant_metadata.jett`. The latter now emits an object.
Native machine state selection and reflected state-field reads match dedicated
native/interpreter cases for general and state-qualified values. They move
`json_parse_machine_envelope.jett` to the unsupported `json.parse` gate and
`type_info_reflection.jett` to reflected type dispatch. Checked dispatch now
matches the runtime `TypeInfo` to one specialized body through canonical type
identity, including source aliases nested inside generic types. That adds
`namespace_comptime_reflection_aliases.jett` and `type_info_reflection.jett` to
the object gate. Checked `type.arg` selection then adds
`comptime_type_bind.jett` to the object gate. The native/interpreter fixture
covers nested generic arguments and alias bases; out-of-range native failures
still need exact interpreter diagnostic parity.
Checked machine layouts, state lists, and transition lists also lower to
ordinary native values. The native/interpreter fixture covers state-qualified
metadata and the empty total probe for non-machine and alias types.

The counts come from the exhaustive 207-row `native_parity` probe, which
attempts every fixture regardless of staged `object_emit` labels and returns
failure until all denominators pass. The original manifest pins 29 nonempty,
code-bearing object gates; the exhaustive Windows MSVC probe also attempts every
unmarked row and now proves 117. Four string/comptime fixtures began emitting
objects after the remaining string intrinsics were implemented; the payload
enum and unit-equality slice adds `enum_advanced.jett`, and bitfield value
support adds `namespace_exports_syntax.jett`; byte decoding adds three bitfield
roundtrip objects, two of which also execute native `main`; `trace_basic.jett`
adds one object and native `main`; primitive list sort adds
`list_sort_uint64.jett`; primitive sets add `set_empty_helpers.jett`; maps add
`map_from_lists_duplicate_keys.jett`, `map_merge_helpers.jett`,
`map_operations.jett`, and `map_set_source_surface.jett`.
Scoped breakpoint bindings add `breakpoint_basic.jett` to both native object
and linked `main` gates; multi-binding and non-`int64` breakpoint values remain
explicitly unsupported.
Compiler-owned predicate functions and CFG lowering for base-value refinement
boundaries add `integer_nonzero_proofs.jett` to the native object gate. The
native/interpreter fixture covers ordered ancestor predicates, false-result
messages, owned-string cleanup, and successful transfers. The remaining
refinement fixtures advanced to JSON intrinsics or primitive-backed sets;
predicate runtime failures, secret-backed inputs, already-refined inputs, and
refinement validation inside struct construction remain pending.
Primitive-backed refinement collection keys and elements, plus normalized
boolean lookup keys, add `primitive_collection_hash_types.jett` to native
object emission. A native/interpreter fixture covers string-refinement sets
and maps, integer-refinement sets, and boolean set/map lookup and removal.
Checked raw-tree JSON bridging adds `json_parse_error_parity.jett`,
`json_parse_success_parity.jett`, and `json_tree_value.jett` to object
emission. Primitive serialization uses the same trusted raw serializer in
native/interpreter execution fixtures. Private source decoders now handle
checked scalar parses for `string`, `bool`, `int64`, `uint64`, `float64`,
`bytes`, and `nothing`, including both public parse entrypoints. A dedicated
native/interpreter fixture checks values and shape/range errors. This moves
`json_parse_exact_primitive_edges.jett` to its later narrow-numeric parse
blocker, but does not yet add an object gate. Narrow numeric and other structured
JSON parsing remain blockers.
Lists, string-keyed maps, optionals, and results whose leaves are `string`, `bool`,
`int64`, `uint64`, `float64`, `bytes`, or `nothing` now instantiate the checked
stdlib decoder for both public parse spellings. A linked differential fixture
covers nested lists, numeric lists, optional null/present values, map bytes,
indexed list errors, and map value errors. Another linked fixture covers result
success/failure branches, nested results, exact parsing, and malformed envelopes.
Named aggregates and narrow numeric leaves retain their existing lowering until
their concrete source decoders can be checked and emitted. The exhaustive object count remains
118/182 because the affected run-pass fixtures still contain other unsupported
JSON shapes.
Sets of `string`, `bool`, `int64`, or `uint64` now use the checked decoder,
including exact parsing, duplicate elimination, membership, and indexed
element errors in a linked differential fixture. The interpreter handoff drops
ambiguous checked expression types from generic source spans instead of using
the last specialization's type for earlier calls. A portable run-pass check
mixes signed and unsigned set parsing to guard this behavior. The exhaustive
object count stays at 118/182; `json_parse_collection_edges.jett` now reaches
its named-struct result payload blocker.
Native `TypeConstruction` now carries an owned struct builder with checked
field layout metadata. `construct_put` transfers the builder and field value,
returns handled metadata and duplicate-field errors, and `construct_finish`
returns a constructed struct or a handled missing-field or owner error. A linked
native/interpreter fixture covers ordinary and generic structs, alias-typed
fields, successful construction, error messages, builder cloning, and cleanup.
The object gate adds `json_reflection_flat_decoder.jett` and
`reflection_type_id_duplicate_construction.jett`, reaching 120/182.
Refinement-validating construction remains a later native slice; the verifier
retains that explicit unsupported boundary.
The checked source JSON decoder now specializes concrete structs and aliases
before its general reflection fallback. The recursive native parse gate accepts
structs whose fields are already supported, including nested structs and
collections, while retaining the refinement and recursive-type boundaries.
Linked native/interpreter cases cover renamed fields, nested and list values,
alias targets and alias-typed fields, handled shape errors, and exact unknown
fields. Seven more run-pass fixtures emit native objects, reaching 127/182;
enum, bitfield, machine, secret, refinement, and narrow-numeric parsing still
need their own checked native paths.
Checked integer/float conversion adds `conversions.jett`.
Native empty `list[never]` length/emptiness support adds `list_operations.jett`.
Borrowed enum matching, explicit view-to-owner cloning at direct calls, and
borrowed list/map iteration over move-only elements add nine raw JSON tree
objects. `json_tree_parse_runtime.jett` also links and matches interpreter
behavior. Generic JSON reflection intrinsics remain guarded; this does not
establish typed JSON parity.
Shared interpreter/native encoding kernels add `encoding.jett` object emission;
a dedicated native main checks base64, hex, URL, and form encoding and their
error precedence against the interpreter.
Shared strict CSV parsing and header validation add `csv_operations.jett` and
`csv_escaping_helpers.jett` objects. A dedicated native main checks parsed
values, canonical quoting, malformed input, and header errors; allocation
fault tests verify nested list/map rollback.
Transparent secret/refinement representation and constant-time native secret
comparison add `secret_compare_boundary.jett` object emission; the two secret
integer wrapping fixtures now pass their runtime contracts. The remaining
runtime-contract gaps at that point required Clock and Random capability providers.
Explicit `coarsen` now carries the checked base representation through native
codegen, including owned string refinements and multi-step refinement ancestry.
Constructing and validating new refinement values at a `handle error` boundary
remains pending, so this slice does not change the fixture gate counts.
Shared private crypto kernels add `crypto.jett` and `use_imports.jett` object
emission. Their native leaves keep public wrappers in `.jett`, with a dedicated
main checking digest bytes, long-key HMAC, and explicit secret declassification.
Nested result/optional handlers in direct-call arguments add
`uint64_checked_expression_runtime_main.jett` object emission and native main
execution. Differential mains cover both handler branches and named-argument
evaluation order; arbitrary expression nesting and refinement handlers remain
open.
Enforcing immutable-local rebinding in the frontend and correcting affected
fixtures adds native objects for `string_iteration.jett`, `set_operations.jett`,
and `uint64_checked_expression_runtime_types.jett`. The last also passes a
supplemental native/interpreter dispatch execution check. Fixture membership
and denominators are unchanged. Unit and payload enums also pass dedicated
native/interpreter differential fixtures, including recursive owned payloads.
Native `math.average` and `math.median` over integer and floating-point lists
add `math_extra.jett` object emission. They share the interpreter's floating-point
reduction kernels and pass differential execution for ordinary and extreme values;
the empty-list runtime error is checked directly. The fixture has no `main`, so
the tracked native-main count is unchanged.
Direct string `for` iteration now lowers through the MIR sequence pass and
uses Unicode scalar count/index leaves, matching the interpreter rather than
the grapheme-cluster `string.chars` API. This adds `loops.jett` object emission;
dedicated Windows and Linux native/interpreter fixtures exercise a joined emoji,
empty input, and early `break`.
Production Clock authority and wall-clock sampling add native objects for
`clock_production.jett` and `clock_scripted.jett`. `clock_production.jett`
also passes native `main` execution, and a dedicated executable checks a
mixed Clock/Stdout entry. Clock conversion shares the interpreter's pre-epoch
flooring and range checks. The launcher now accepts an explicit test-only
sample script. The parity probe supplies the same samples to the interpreter
and native runtime for `clock_scripted.jett`, including pre-epoch timestamps,
and an empty script for the exhausted-provider runtime failure. Detecting
unconsumed samples at successful native process exit remains pending.
Random now uses one context-bound provider shared with the interpreter. Native
entry grants initialize OS entropy once, while the parity probe can inject
normalized bounded, unit-float, and boolean samples. The two Random runtime
failure fixtures check invalid and exhausted scripts with exact messages and
clean native teardown. `stdlib/random.jett` retains the public collection
algorithms; native `list.__swap` supports the scripted shuffle path.
Environment now freezes the same launch snapshot type in the shared runtime.
Native entry grants a context-bound token, and the `Environment.__args` and
`Environment.__get` leaves return independently owned lists and the exact
result/optional shape. The injected snapshot fixture covers duplicate names,
empty arguments, missing values, invalid names, and invalid Unicode values;
`time_and_os.jett` covers a production entry. Raw platform capture details
specified in the Environment design note still need a separate backend audit.
Unit-enum equality borrows its operands while comparing tags, so comparisons
of an enum field do not move that field out of its aggregate. The bitfield
roundtrip fixture now emits an object and matches interpreter execution; the
native bitfield fixture compares the decoded enum field twice before using its
parent again. General owning projection of move-only fields remains guarded.
Native arithmetic now accepts a nonzero refinement as the right operand of
integer division or modulo when its base matches the left operand. This is
covered by an object-emission test and the now-emitting
`integer_nonzero_proofs.jett` fixture.
Fixed-width bitfield values, collection payload ownership and byte roundtrips
also pass a dedicated native/interpreter fixture, including network order,
native order, enum discriminants, trailing payload bytes, decode errors and
allocation cleanup. Dynamic field-width validation remains outside this slice.
An unconstrained empty `list[never]` has a native empty-list layout. Expected
result types now refine an argument-inferred `never` before specializing a
generic call, so `list_shape_helpers.jett` emits an object; a dedicated native
main executes the resulting nested-list ownership path. Other collection
shape conversions still need explicit ownership metadata conversion.
Machine values use owned tagged records. Checked constructors, transitions,
state tests and state-qualified field projection now emit native objects for
eight machine run-pass fixtures, including namespace branches and negative
narrowing. A dedicated native/interpreter fixture exercises transitions,
borrowed field access and three-state branch narrowing. A runtime guard checks
the tag before treating a bare machine local as a narrowed state. JSON machine
fixtures remain blocked by generic JSON operations.
The `uuid.new` intrinsic now shares UUID v4 formatting and OS entropy handling
between interpreter and native runtime. This adds the `string_chars.jett`
object gate; a dedicated native executable checks its version, variant,
separators, and length without comparing random identifiers across runs.
Named function values and indirect calls unlock `list_higher_order.jett` after
`list.sort_by` moves its callback evaluation into `.jett`. Indexed row sorting,
sortedness checks, and source-level `list.group_by` then unlock
`list_source_surface.jett`. Dedicated native/interpreter programs check stable
sorting, callback results, large unsigned values, floating-point and boolean
keys, missing indices, and grouping.
Capture-free inline functions now extract to ordinary checked functions and
use the same native function-address path. This adds object gates for
`generic_function_value_wrappers.jett`, `inline_functions.jett`,
`list_map_extra.jett`, and `map_advanced.jett`. A linked differential fixture
covers inline list callbacks, returned callbacks, zero-argument functions,
and nested capture-free functions. Closures that read enclosing locals remain
guarded until the native function value can carry an environment.
Checked `list.insert_at` and `list.remove_at` leaves transfer list and element
ownership on success, including owned bytes elements. An indirect callback
returning a string now receives a planned owning temporary. Together these add
`list_extras.jett` to the object gate; linked differential fixtures cover valid
insert/remove positions and string-returning `list.map`. Invalid-index failures
currently report a static native message without the index, so exact diagnostic
parity remains pending.

Checked public JSON calls for supported structs, lists, string-keyed maps,
optionals, and results now instantiate the reflected `.jett` serializer after
the compiler-owned policy gate. The serializer selects type-kind and primitive
branches while checking each concrete instantiation. Borrowed containers are
cloned before consuming iteration.
Nested generic checks preserve the outer parameter types; HIR derives result
handler error bindings from the checked result type. A linked differential
fixture covers both public spellings, renamed and escaped fields, all supported
scalar categories, nested collections, optional/result branches, and cleanup.
Float32 fields, enums, bitfields, secrets, raw-tree fields, and other JSON
shapes still need this source path or another checked native lowering.
Bare and state-qualified machines whose state fields recursively use the
supported parser leaves now instantiate the reflected `.jett` decoder. A
linked native/interpreter fixture covers populated and empty states, exact
validation, a missing field, and a state-qualified mismatch. Empty reflected
field loops lower without a body. Supported bare and state-qualified machines
also instantiate the reflected source serializer; a linked fixture covers
renamed payload fields, nested machine collections, and public omission of
secret payload fields. Widening a state-qualified machine local to a bare
machine consumes that local; reuse requires an explicit clone. These changes
add `json_namespace_duplicate_machine_envelope.jett` and
`json_serialize_machine_envelope.jett` to the object gate. The exhaustive
Windows gate was 135/182 emitted objects. The checked decoder now accepts
supported `secret[T]` values and secret-bearing struct and machine fields. A
linked fixture checks direct secret parsing, closed-shape parsing, public
omission of record and machine secrets, and handled exact-parse errors. Native
MIR verification permits only the checked promotion from an inner local to its
`secret` wrapper. This adds `json_parse_machine_envelope.jett` and
`json_stdlib_bridge_delegation.jett` to the object gate. That exhaustive
Windows gate reached 137/182 emitted objects. A dedicated trusted `.jett` raw
parser now returns `secret[json.JsonTree]` for both public parse spellings;
`parse_exact` retains the raw tree's open-shape policy. The linked fixture
checks successful and malformed input, and
`json_parse_exact_secret_edges.jett` joins the object gate. The current
exhaustive Windows gate reached 138/182 emitted objects. Supported top-level
enums now use dedicated checked `.jett` parse and serialize hooks, keeping the
raw `json.JsonTree` wire path separate. The enum payload decoder keeps its
reflected construction inside the checked variant loop. A linked fixture
covers unit and payload variants, nested exact validation, unknown variants,
and public serialization. This adds `json_enum_shapes.jett`,
`json_result_shape_diagnostics.jett`, and
`reflection_type_id_duplicate_enum_payloads.jett` to the object gate. The
Windows gate reached 141/182 emitted objects. Checked source parsing and
serialization now accept refinements over supported bases. The reflected
decoder selects the refinement branch before its generic fallback, and native
HIR supplies the checked string type for refinement error bindings. A linked
interpreter/native fixture covers valid and rejected scalar values, exact
record parsing, and serialization. `json_refinement_exact_serialize_edges.jett`
adds one object; the Windows gate reached 142/182 emitted objects. Checked
source serialization now handles `bytes` through a trusted hex-string hook and
records containing raw `json.JsonTree` fields through an owned clone. A linked
interpreter/native fixture covers both public serialization spellings and raw
tree payloads. `json_serialize_public.jett` joins the object gate; the Windows
gate reached 143/182 emitted objects. Supported bitfields now serialize through
a checked source hook that dispatches unit-enum fields separately from ordinary
fields. A linked fixture covers enum-backed bits, wide integers, and list
payloads. `json_serialize.jett` joins the object gate; the current exhaustive
Windows gate is 144/182 emitted objects, 23/30 main outcomes, 25/25 runtime
contracts, and 182/182 typed lowerings. Nested enum parsing, bitfield JSON
parsing, refinement-validating struct fields, and other JSON shapes remain.

Verification-only empty objects do not count. Native
execution tests additionally assert computed output, not only process success.

Terminal failures count only after behavior and cleanup match. ABI context
destruction now rejects unreleased string owners; launcher exit 71 requires
successful destruction, while leaks override it with exit 72. Dedicated launcher
tests exercise both paths and nested native-call tests exercise temporary/local
cleanup. This is evidence for the current scalar/string runtime, not a claim
that unsupported resource families already have finalizer/effect instrumentation.

## Linux GNU executable harness

The production linker also accepts exactly `x86_64-unknown-linux-gnu` when it
is the compiler host. `NativeLauncherBundle::linux_gnu_v1` specifies the Rust
static launcher archive, ABI v1, dynamic GNU CRT, and ordered system libraries.
`cc` (or the literal executable path in `JETT_NATIVE_CC`) links the Cranelift
object without a shell. Cross-target bundles are rejected before linking.
Windows MSVC retains its static CRT, SDK discovery, library order, and atomic
publication contract. A Windows MSVC host test now builds its matching launcher
archive, links native executables, and compares scalar entry, stdout, owned
bytes, and generic struct fixtures with interpreter output. This is a host-local
execution gate; the full parity denominators still require the exhaustive audit.

`cargo test -p jett_driver --test native_execution` builds the target-matched
launcher archive and executes the scalar-entry fixture in an empty directory
with an empty environment and a deadline. The executable needs no compiler
source at runtime. This is one executable seed, not full native parity or a
clean-distribution packaging claim. All remaining fixture obligations remain.

Explicit `comptime` primitive results are imported into typed HIR before MIR
lowering, retaining their checked type and span. Ordinary pure calls remain
runtime calls. The backend rejects any unresolved `Comptime` marker instead of
emitting its source computation. Composite constants still need native layout
lowering and remain an explicit native parity gap. A native regression executes
baked `math.factorial(5)` after removing its source file.


## Native runtime-value slice

`active/native_value_abi.md` defines immutable context-associated string handles,
borrowed call inputs, owned results, terminal failure transport, and the separate
Stdout token. `jett_mir::copy_values::CopyValuePlan` computes definite initialization
and liveness over CFG backedges. Cranelift consumes those facts to release dead
locals, overwritten values, full-expression temporaries, and all frame owners on
return or terminal failure. Runtime allocation maps remove each string at its last
release; context destruction is a leak check, not a program-long value arena.

Ordinary Jett calls, branches, loops, argument evaluation, and string interpolation
are emitted code. Typed runtime leaves implement string storage, formatting,
grapheme slicing/counting, selected Unicode operations, stdout, and numeric kernels.
Native `string.index_of` and `string.count` search only at grapheme boundaries;
the former returns an owned optional value and the latter counts non-overlapping
matches. Both share the native split scanner. The same interpreter differential
fixture is wired into the Linux GNU and Windows MSVC execution suites.
They consume no interpreter Value, AST, HIR, or source operation names. Exact checked
IntrinsicId and concrete numeric type arguments select the native leaf signature.

MoveValuePlan now models linear bytes, selected sum payloads, list and user struct owners, with
call-bounded loans, active iteration loans and initialized owning slots. Full
ownership remains open for capabilities beyond Stdout, resources, other aggregate kinds,
closures and tasks. Nested-expression handlers and move-only iteration projections
remain pending. Unsupported types/forms remain rejected
rather than being made nominally supported by the copyable-string plan. CLI
packaging, other capability providers, and clean Windows MSVC execution also remain
release gates.

Phase 4 additionally executes both existing generic-struct and explicit-equality
fixture bodies with supplemental mains. Namespace interface mains now match exact
stdout natively. Struct fields retain their checked layouts and implicit-view
semantics; Equatable comparisons carry exact checker-selected method identities
into compiled calls. One emitted object is tested with four real process inputs.
The full workspace checkpoint passed 1763 tests; complete remains false. Enum
payloads, refinement-validating construction and projected-owner escapes remain
continuation work. See `native_value_abi.md` for the precise supported boundary.
