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
| Strings and bytes | covered | all current string intrinsics, separately owned bytes storage, and encoding leaves | string search/replace, Unicode case changes, byte and encoding fixture functions, nested cleanup, moves/views/clones covered |
| Structs, enums, bitfields, machines, and refinements | covered | concrete user structs and enums with typed payloads; bitfield construction and fields; transparent secret/refinement representation; machines and validating refinements pending | struct moves/views/clones, nested owners, explicit equality and failure cleanup; enum construction, payload transfer, clone, match and cleanup, plus unit-enum equality; bitfield field ownership, clone and byte roundtrip covered; width validation, payload-enum equality and other aggregate kinds pending |
| Lists, maps, and sets | covered | scalar/string/bytes/sum/nested lists, primitive list sorting and sets, and primitive-key maps with owned values | compiled list access, reverse/repeat, scalar iteration and sort; set insert/remove/membership/clone and iteration; map literals, insert/remove/lookup/from_lists/clone and key-value iteration; projected views, contextual empty-list conversion, refinements, and callback helpers pending |
| CSV | covered | checked parse, parse-with-header, and stringify leaves over owned lists and maps | strict quoting, CRLF, header values/errors, and nested allocation cleanup match interpreter |
| Crypto | covered | private SHA-256, SHA-512, MD5, and HMAC-SHA-256 byte kernels | native differential fixture covers public text digests, binary HMAC, long keys, secret comparison, and explicit declassification |
| Secret values | covered | transparent scalar and owned representations; redaction and string/bytes comparison | native differential fixture covers equal, unequal, length-mismatched and Unicode strings, bytes, redaction, and aggregate ownership |
| Results, optionals, and `handle` control flow | explicit statement-root handler CFG | genuine tags, owned payloads and selected extraction | nested sums, defaults, early returns, loop exits and terminal bypass covered; nested-expression handlers pending |
| Function values, closures, and indirect calls | covered, but closure bodies still require explicit MIR function extraction | pending | pending |
| Compiler intrinsics and reflection | covered with checked operands and closed `IntrinsicId` identities | pending | pending |
| Capabilities and runtime resources | nominal checked types covered | explicit Stdout entry and write; others pending | Stdout output covered; other providers/resources pending |
| Actors and structured concurrency | covered | pending | pending |
| JSON and trusted stdlib hooks | covered | pending | pending |
| Trace, breakpoint, assert, and failure reporting | covered | `int64` trace statements covered; other trace types and breakpoint/assert pending | `int64` trace debug lines match interpreter stderr; other instrumentation pending |

The current fixture gates are therefore:

| Obligation | Passing | Denominator | Evidence |
| --- | ---: | ---: | --- |
| Typed backend lowering | 182 | 182 | `run_pass_backend_lowering_gaps_are_explicit_and_monotonic` |
| Native object generation | 73 | 182 | exhaustive Windows MSVC 207-row checkpoint; original 29 staged deterministic manifest gates retained |
| Successful/expected `main` execution | 14 | 30 | Windows MSVC production linking and exact interpreter stdout/debug-output comparison in `native_execution_windows` |
| Runtime contracts | 22 | 25 | exhaustive runtime-contract probe; matched behavior and checked native-value cleanup |

These counts come from the exhaustive 207-row `native_parity` probe, which
attempts every fixture regardless of staged `object_emit` labels and returns
failure until all denominators pass. The original manifest pins 29 nonempty,
code-bearing object gates; the exhaustive Windows MSVC probe also attempts every
unmarked row and now proves 73. Four string/comptime fixtures began emitting
objects after the remaining string intrinsics were implemented; the payload
enum and unit-equality slice adds `enum_advanced.jett`, and bitfield value
support adds `namespace_exports_syntax.jett`; byte decoding adds three bitfield
roundtrip objects, two of which also execute native `main`; `trace_basic.jett`
adds one object and native `main`; primitive list sort adds
`list_sort_uint64.jett`; primitive sets add `set_empty_helpers.jett`; maps add
`map_from_lists_duplicate_keys.jett`, `map_merge_helpers.jett`,
`map_operations.jett`, and `map_set_source_surface.jett`.
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
runtime-contract gaps require Clock and Random capability providers.
Shared private crypto kernels add `crypto.jett` and `use_imports.jett` object
emission. Their native leaves keep public wrappers in `.jett`, with a dedicated
main checking digest bytes, long-key HMAC, and explicit secret declassification.
Fixture membership and
denominators are unchanged. Unit and payload enums also pass dedicated
native/interpreter differential fixtures, including recursive owned payloads.
Fixed-width bitfield values, collection payload ownership and byte roundtrips
also pass a dedicated native/interpreter fixture, including network order,
native order, enum discriminants, trailing payload bytes, decode errors and
allocation cleanup. Dynamic field-width validation remains outside this slice.
An unconstrained empty `list[never]` has a native empty-list layout, but
converting it to a contextually typed list still needs ownership metadata
conversion. `list_shape_helpers.jett` remains guarded at that mismatch.
`set_operations.jett` now passes its set operations and reaches an existing
immutable-local reassignment mismatch between accepted source and native MIR
verification; that policy discrepancy remains unresolved.
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
