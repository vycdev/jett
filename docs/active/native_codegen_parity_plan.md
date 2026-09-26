# Native Code Generation Parity Plan

Status: accepted implementation strategy; checked-program, HIR, and MIR
prerequisite work is in progress.

This plan defines when Jett may claim native-code parity with the current
interpreter-backed language. It fixes the initial backend, the prerequisite IR
contracts, and measurable acceptance gates. Native execution must implement
accepted Jett semantics; it may not become a second source-language policy
layer or silently delegate runtime behavior to the interpreter.

## Scope and Baseline

The current fixture inventory establishes five separate denominators:

| Obligation | Denominator | Acceptance condition |
| --- | ---: | --- |
| Native lowering | 182 | Every `tests/run_pass/*.jett` fixture reaches validated HIR and MIR and is accepted by native object generation. |
| `main` execution | 30 | Every run-pass fixture that declares `main` links and runs on the supported host with its interpreter-equivalent expected outcome and observable behavior. One scripted graphics fixture intentionally returns a runtime error. |
| `verify` execution | 155 | Every run-pass fixture with top-level `verify` blocks links a native suite that executes each body in declaration order and exits successfully. |
| `property` execution | 3 | Every run-pass fixture with top-level `property` blocks links a native suite that executes each body for 100 deterministic trials chosen by the existing generator. |
| Runtime contracts | 25 | Every `tests/runtime_fail/*.jett` fixture links and matches its interpreter contract: 17 wrapping-success cases and 8 runtime-failure cases, including failure class, message contract, and cleanup behavior where applicable. |

These numbers are denominators, not a sample or a percentage target. Fixtures
added before parity is declared extend the applicable denominator. A fixture
may leave it only when the corresponding language or runtime feature is
explicitly marked unimplemented, with the reason recorded in the same change.

Run-pass files without `main`, including verification and property fixtures,
count toward the 182-fixture lowering obligation. They do not enter the
30-fixture `main` execution denominator. The native verification suite now
links and executes every checked top-level `verify` body in each of 155
fixtures as a separate execution gate. A native property suite uses the
interpreter's established deterministic `given` pools as test inputs, then
executes the checked property bodies as native code. Native failure-case
shrinking and diagnostics remain follow-up work.

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

The parity report must publish counts as `passed / denominator` for each
obligation. A single percentage would hide the difference between code
that merely lowers, code that executes successfully, and code that preserves
failure semantics.

## Current Coverage Matrix

`tests/native_parity.json` is the machine-checked fixture inventory. The table
below records implementation coverage; a row is complete only when its native
object, linked execution, and differential behavior gates all pass. Typed
lowering alone never changes an execution row to complete.

| Surface | Validated HIR/MIR | Cranelift object | Linked native behavior |
| --- | --- | --- | --- |
| Fixed-width integers, floats, booleans, and `nothing` | covered | scalar expressions, direct calls, branches, and loops covered | linked native/interpreter scalar, conversion, numeric aggregate, and debug-value fixtures cover representative widths and floating-point edges; a width-by-operation arithmetic edge matrix now covers all integer and float widths, while exhaustive operand combinations remain pending |
| Strings and bytes | covered | all current string intrinsics, direct Unicode-scalar string iteration, separately owned bytes storage, and encoding leaves | string search/replace, Unicode case changes, scalar `for` loops, byte and encoding fixture functions, nested cleanup, moves/views/clones covered |
| Structs, enums, bitfields, machines, and refinements | covered | concrete user structs and enums with typed payloads; bitfield construction and fields, including result-wrapped width validation; machine construction, transitions, state tests and state fields; transparent secret/refinement representation, `coarsen`, checked refinement predicates for base, intermediate-refined, and secret-backed inputs, and result-wrapped direct and reflected struct construction with validated fields | struct moves/views/clones, nested owners, explicit equality and failure cleanup; enum construction, payload transfer, clone, match and cleanup, plus scalar and supported aggregate payload-enum equality; bitfield field ownership, clone, byte roundtrip, and direct/reflected-builder width validation; machine state narrowing, transitions and owned payload cleanup; native/interpreter predicate success and false-result diagnostics for integer, owned-string, and secret-backed refinements, including base values inserted into reflected struct, enum, and machine builders; enum payloads containing user structs and broader refinement boundaries pending |
| Lists, maps, and sets | covered | scalar/string/bytes/sum/nested lists, primitive and indexed-row list sorting, sortedness checks and sets, and primitive-backed refinement keys and set elements | compiled list access, insert/remove, reverse/repeat, scalar iteration and sort; `list.filter`, `map`, `flat_map`, `find`, `all`, `any`, `count`, `reduce`, `sort_by`, and `group_by` with named, inline, and captured callbacks; `map.filter`, `map_values`, and `for_each` callbacks; indexed-row sorting for interpreter-supported key types; set insert/remove/membership/clone and iteration; map literals, insert/remove/lookup/from_lists/clone and key-value iteration; representative `zip`, `chunk`, `enumerate`, `flatten`, set/list conversions, and map entries; borrowed iteration through nested struct-field collection paths; refinement string and integer keys and set elements; one contextual generic empty-list path covered; other projected views and unexercised generic collection shapes pending |
| Numeric aggregates | covered | `math.average` and `math.median` for `list[int64]`, `list[uint64]`, and `list[float64]` | native/interpreter differential fixtures cover all three element types and overflow-safe floating-point extremes; empty-list error contract covered |
| CSV | covered | checked parse, parse-with-header, and stringify leaves over owned lists and maps | strict quoting, CRLF, header values/errors, and nested allocation cleanup match interpreter |
| Crypto | covered | private SHA-256, SHA-512, MD5, and HMAC-SHA-256 byte kernels | native differential fixture covers public text digests, binary HMAC, long keys, secret comparison, and explicit declassification |
| Secret values | covered | transparent scalar and owned representations; redaction and string/bytes comparison | native differential fixture covers equal, unequal, length-mismatched and Unicode strings, bytes, redaction, and aggregate ownership |
| Results, optionals, and `handle` control flow | explicit CFG for statement-root, direct-call, indirect-call, supported intrinsic-argument, unary, cloneable binary, short-circuit boolean, collection, value-constructor, machine-transition, task `run`/`join`, actor-spawn, actor-message, field-receiver, state-test, clone, coarsen, declassify, condition, match-scrutinee, for-iterable, assertion-condition, and breakpoint-condition handlers | genuine tags, owned payloads and selected extraction | nested sums, defaults, early returns, loop exits, terminal bypass, ordered call arguments and constructor fields, scalar, string, and enum evaluation order, short-circuit fallback skipping, handled field/state projections in `if`, `while`, and `match`, one-time for-iterable evaluation, handled debug conditions, ordered transition source/payload defaults, handled task values including pending `nothing`, and actor source/argument order covered; other nested-expression and refinement handlers pending |
| Function values, closures, and indirect calls | covered; inline bodies extract to checked functions with explicit capture parameters and ownership modes | owned descriptors carry code addresses, copied capture environments, and source-derived debug labels; indirect calls pass the environment after the runtime context and borrow view parameters; parenthesized, returned, projected, inline, and pipeline callee expressions retain checked call order | named, capture-free, and captured callbacks passed, returned, copied through aggregates, and invoked through indirect calls, including view and capability parameters; callee handlers and terminal failures clean already evaluated owned arguments; explicit `comptime` materializes named, capture-free, and captured inline function values from closed pure expressions, including capability-accepting callbacks whose bodies execute only with runtime authority |
| Compiler intrinsics and reflection | covered with checked operands, source-aware reflection metadata, and closed `IntrinsicId` identities | `type.name`, `type.kind`, `type.has_secret`, `type.kind_tag`, `type.primitive_tag`, recursively constructed `type.info`, checked `type.arg`, struct/bitfield, enum, and machine metadata lists and layouts, active enum variant and machine state metadata, reflected field values, and checked reflected-type dispatch covered; other aggregate reflection pending | direct and generic scalar reflection, nested `TypeInfo`, indexed type arguments, struct/bitfield, enum, and machine metadata, active enum and machine state selection, reflected field values, and alias-aware `comptime type` dispatch match the interpreter on positive cases; alias probes remain empty as required; mismatch diagnostics and other aggregate reflection pending |
| Explicit `comptime` values | checked closed pure expressions and contextual expected types covered | evaluated scalar and supported composite values materialize as typed HIR; captured closure values gain typed caller-local bindings; original source bodies are not emitted | linked native/interpreter fixtures cover nested collections, sums, structs, bitfields, enums, machines, bytes, contextual `ok`/`fail`/`none`, named functions, capture-free inline functions, captured closures, and pending `nothing` values, including captured and list-contained values; runtime authority remains unsupported |
| Capabilities and runtime resources | nominal checked types covered | explicit Stdout, Clock, Random, and Environment entry grants; others pending | Stdout output, Clock/Random sampling, and immutable Environment launch snapshots covered; exact-consumption checks and other providers/resources pending |
| Actors and structured concurrency | covered | sequential `run`/`join`/`cancel` values and result propagation; pending depth for `nothing` persists through calls, captures, and aggregates; actor constructors, registration, handler dispatch, and state writeback covered; general pending-task representation remains pending | native/interpreter differential probes cover successful and failed string tasks, nested pending `nothing`, bare-unit join failure, current sequential cancellation behavior, pending-unit debug output and equality, actor allocation and owned-state cleanup, message ordering, state mutation, and responses; asynchronous scheduling and cancellation checkpoints remain pending |
| JSON and trusted stdlib hooks | covered | checked `JsonTree` calls use trusted raw stdlib functions; supported structs, machines, collections including sets of primitive-backed elements, and top-level refinements specialize the checked source serializer, including public omission of direct secret fields; supported top-level enums use dedicated checked source hooks; primitive parse calls use private source decoders, including bounded `int8`/`int16`/`int32`, `uint16`/`uint32`, and `float32` construction; concrete structs, bare and state-qualified machines, supported secret wrappers, top-level refinements over supported bases, lists, sets of primitive-backed hashable types, string-keyed maps, optionals, and results specialize the checked source parser; other JSON shapes remain pending | native/interpreter fixtures cover raw trees, primitive and structured serialization, enum unit and payload values, machine state envelopes and public secret omission, primitive and structured parsing, top-level refinement parsing and serialization, secret-bearing records and machines, exact validation, renamed fields, aliases, nested collections including lists/maps of enums and enum payload structs, result branches, errors, and owned cleanup; other concrete JSON types pending |
| Trace, breakpoint, assert, and failure reporting | covered | primitive, function, actor, `TypeConstruction`, and recursive list, set, map, optional, result, struct, bitfield, enum, and machine values trace; zero- and multi-binding breakpoints over those types; default and interpolated custom-message `assert` in test bodies; other special debug values pending | native debug lines match interpreter stderr for scalar and aggregate fixtures, including empty and recursive values, bitfield numeric/enum/payload fields, false conditions, Unicode strings, bytes, `nothing`, and an out-of-scope local; named, inline, captured, and comptime callbacks inside aggregates, with subsequent calls and failure cleanup; actor ordinals and partially filled struct, bitfield, enum, and machine builders match; custom assertion codegen, context-owned error text, launcher copy-out, and passing verify/property suites have separate tests; secret-bearing debug output and cross-function breakpoint binding scope pending |

Actor handler capability and state snapshots are explicit leading HIR/MIR
parameters, and `respond` participates in native ownership analysis. Checked
actor state initializers lower into constructor functions that source spawns
reference directly. Typed runtime leaves register actor-owned state records,
replace owned fields, and clean them at context destruction. Native actor
allocation, state writeback, and message dispatch pass linked differential
fixtures; true asynchronous scheduling remains a separate task.

The current fixture gates are:

| Obligation | Passing | Denominator | Evidence |
| --- | ---: | ---: | --- |
| Typed backend lowering | 182 | 182 | `run_pass_backend_lowering_gaps_are_explicit_and_monotonic` |
| Native object generation | 182 | 182 | `native_parity_object_emit_obligations_emit_host_objects`; every run-pass fixture emits reachable native code |
| Successful/expected `main` execution | 30 | 30 | Windows MSVC production linking and exact interpreter stdout/debug-output comparison, including scripted Clock, Random, Environment, and Graphics inputs |
| Native `verify` execution | 155 | 155 | `native_verify_suites_execute_for_all_run_pass_fixtures`; 507 top-level bodies execute across the 155 fixtures |
| Native `property` execution | 3 | 3 | `native_property_suites_execute_for_all_run_pass_fixtures`; 18 top-level bodies each execute 100 generated trials |
| Runtime contracts | 25 | 25 | exhaustive runtime-contract probe, including scripted Clock and Random failures; matched behavior and checked native-value cleanup |

These counts track fixture gates, not a weighted percentage of Jett syntax or
runtime semantics: fixtures differ in size, overlap, and coverage. The object
gate is currently 182/182 (100%), a useful progress measure rather than a
claim that the same fraction of the language has native support. Native
property-body execution is measured separately from the object and verify gates.

`tests/native/arithmetic_matrix.jett` adds linked differential checks across
all eight fixed-width integer types and both floating-point widths. It checks
integer wraparound, minimum divided and reduced by negative one, unsigned
division/remainder, float arithmetic, signed zero, infinities, and NaN
comparisons through runtime function calls. The 81-test Windows native suite
includes this case; the matrix is an edge sample, not exhaustive over all
operand values.
`tests/native/json_nested_enum.jett` adds linked differential coverage for
lists and string-keyed maps of enums, including a list of enum values whose
payload contains a user struct. Exact parsing and serialization agree with
the interpreter. The 82-test Windows native suite now includes both added
fixtures; fixture gate denominators remain unchanged.

As of 2026-09-26, the working estimate for overall native language coverage is
**about 80%**. This is a deliberately coarse progress marker, reviewed in
five-percentage-point steps against the coverage matrix above, not a computed
ratio or a release gate. The fixture counts remain the reproducible measures.
View-parameter function values, pending `nothing` tasks, and general
function-expression calls gained native execution, closing known gaps without
yet supporting the next five-point step of the estimate. It stays below full
parity while known semantic gaps remain in
ownership and nested handlers, capability/resource providers, asynchronous
task behavior, JSON shapes, reflection, and special-value diagnostics.

Pending `nothing` values now retain their nesting as an unowned native scalar.
Each `run` adds a level; `join` unwraps one level, while joining plain `nothing`
returns `fail("task was cancelled")`. Calls, branches, closures, lists, maps,
optionals, results, structs, enums, machines, and explicit `comptime` preserve
that value. Native output and terminal comparison errors match the interpreter.
The current sequential `cancel` leaves its operand unchanged and returns plain
`nothing`; this parity slice does not implement the designed concurrent
cancellation checkpoints. Dedicated native fixtures cover these behaviors
outside the fixed 182-fixture object denominator.
Validation for this slice passed the full 53-test Windows native execution
suite and the unchanged 182/182 object gate, plus focused runtime, backend, and
driver unit tests.

Function parameter ownership now remains part of checked function type
identity through native emission. Differential coverage includes borrowed and
consumed inputs, mixed borrowed/owned arguments, captured closures, explicit
comptime function values, generic forwarding, pipelines, and views evaluated
before handlers. Generic calls and pipelines reject a view passed to an owned
parameter at the frontend; affected JSON map observers explicitly clone their
owned-input API arguments instead of relying on an implicit native clone.
General function-expression calls now lower and execute through parenthesized
callbacks, immediately invoked returned functions, function-valued fields, and
pipeline targets. They retain the existing indirect-call order: evaluate
arguments in source order, then evaluate the callee expression, then invoke the
selected function value.
Argument handlers may therefore change which callback is selected, and an early
return from an argument must skip callee evaluation. Constructor, declared
function, and compiler-intrinsic identities retain their checked dispatch.
Function parameter ownership, purity, and capability rules apply to expression
callees exactly as to existing function values; no new call syntax is introduced.
The frontend must check ownership in that same argument-before-callee order,
including parenthesized owners, and must enforce capability purity for all
callee shapes. Named arguments require checked declaration parameter names.
Parentheses around a declared function preserve those names; an anonymous
function type supplies no names, so labels on such calls must be rejected
instead of silently interpreted as positional arguments. This conservative
boundary avoids adding parameter names to function type identity or deriving
them from whichever implementation happens to be stored at runtime.
Inline function bodies follow the same signature-based purity rule as declared
functions. Checking a closure body uses its own function context; constructing
a closure does not execute that body or inherit the caller's comptime authority.
Pure expression calls retain secret input taint on their return values.
Enum constructor payloads carry checked parameter order and a separate source
evaluation permutation through HIR, MIR, and native emission. Reordered named
payloads, handlers, early returns, pipelines, and baked enum values preserve
both their field layout and lexical effects. A function-valued field whose name
matches a variant of its result enum still invokes the callback.
Mixed named and positional arguments fill the next unbound parameter in both
execution paths. Interpreter calls isolate the callee's lexical locals and
namespace aliases, including aliases retained by returned closures.
Validation passed all 56 Windows native execution tests and the unchanged
182/182 object gate. After the final argument-binding and lexical-scope fixes,
all 296 interpreter tests, 578 frontend fixtures, 10 Graphics frontend tests,
and the three dedicated native expression-call/enum tests passed again.
The new native fixtures extend focused coverage without changing the fixed
run-pass object denominator.
Function-valued trace and breakpoint bindings now render the interpreter's
source-derived labels. Named callbacks retain their canonical source name;
inline callbacks retain their source parameter names independently of capture
count, generic specialization, and native symbol identity. The owned label
travels with the descriptor through clones and nested lists, maps, optionals,
results, records, recursive enums, and machines. Debug reads never invoke the
callback or expose captured values. Native fixtures cover repeated tracing,
subsequent invocation, conditional breakpoints, terminal-failure cleanup, and
callback traces in `verify` bodies and 100 generated `property` trials.
Generic closure factories also now receive concrete native instantiations:
ordinary generic signature annotations no longer trigger reflection-only
handling, and HIR parameters come from each checked concrete function signature.
Integer and string specializations retain their own parameter/capture types.
The function-debug slice passed the full 58-test Windows native suite and
182/182 object gate, plus 578 frontend fixtures, 153 typechecker tests, 59 HIR
tests, 26 MIR tests, 54 backend tests, 90 runtime tests, and 61 driver unit tests.
Generic factories still reject move-only captures with E0402. Overall coverage
remains at the coarse 80% estimate while the matrix's remaining gaps are open.

Concrete source method values now pass through checker-selected body identities
into HIR and native function descriptors. This covers struct methods and
interface implementations stored, returned, cloned, projected from aggregates,
invoked through generic factories, baked with explicit `comptime`, and traced.
Receivers remain explicit first parameters with their checked `view` modes.
Where an inherent method and interface implementation share a name, concrete
method values use the inherent body, while interface-qualified calls use the
implementation. Interpreter method calls now retain their declaration namespace
so lexical helper references agree with native execution. The differential
`tests/native/method_function_values.jett` fixture exercises these paths in
`main`, `verify`, and 100 property trials. The broad native percentage remains
the coarse 80% estimate pending further coverage-matrix closure.
Explicit `comptime` extraction now retains the visible block-local `use`
aliases for each closed expression. An inner alias may shadow an outer one;
evaluation isolates each expression so the outer binding is restored afterward.
`tests/native/comptime_namespace_aliases.jett` compares baked named and returned
callbacks through both scopes with the interpreter. This removes the previous
local-alias frontend gap without admitting runtime locals to comptime.
Actor handles now participate in native trace and breakpoint layouts, including
handles nested in lists. Registration preserves the interpreter's `actor#N`
spawn ordinal independently of native allocation IDs; debug validates each
handle and never reads actor state. `tests/native/debug_actor_values.jett`
compares both debug lines and subsequent message sends with the interpreter.
Other special debug values and cross-function breakpoint scope remain open;
overall language coverage stays at the coarse 80% estimate.
`tests/native/debug_type_construction.jett` compares empty and partially filled
builders for struct, bitfield, enum, and machine owners, including a struct
whose fields are supplied out of declaration order. Native builder metadata
stores checked field debug layouts and successful insertion order; tracing
does not consume the builder. The secret-field output policy is recorded in
`docs/open_design/secret_debug_output.md`. The 62-test Windows native suite,
182-fixture typed-lowering gate, and backend/runtime unit suites pass after
this change.
The linked `tests/native/collection_callbacks.jett` fixture now covers
higher-order list and map helpers, including an inline predicate, a captured
sort key, and an observable `map.for_each` callback. The linked
`tests/native/collection_shapes.jett` fixture covers generic pair/indexed
records, chunk/zip/flatten/enumerate, set/list conversion, and map entries.
Both compare exact output with the interpreter. These checks replace the broad
"callback helpers and collection shape conversions pending" label with the
remaining unexercised generic shapes and projected views; the overall coarse
80% estimate is unchanged.
Reflected enum and machine builders now accept already validated exact
refinement field values. The native JSON source selector uses this for enum
payloads and machine states, including a machine with a refined field beside
an ordinary field. `tests/native/json_refined_aggregates.jett` checks
primitive-backed and struct-backed refinements, valid parsing, predicate
failures, and direct reflected construction against the
interpreter. Trusted JSON decoders bind each inserted `Field` from checked
reflection metadata. Ordinary struct builders now accept base values for
refined fields and invoke checked predicates at finish, including nested
refinements and mixed refined and ordinary fields of the same base type.
Enum builders now also accept base payload values and validate the selected
variant's checked predicates at finish. Machine builders now accept base
payload values for bare and state-qualified targets, checking only the active
state's predicates at finish. The overall coarse estimate stays 80%.
At this checkpoint, the 68-test Windows native suite, 182/182 native object
gate, 182/182 typed-lowering gate, and focused HIR/MIR/Cranelift suites pass.

Nested supported enum and bitfield fields now select checked JSON source hooks
inside records and collections. Generic `type.name[T]()` equality with a
literal gives raw `json.JsonTree` its distinct checked branch, while reflected
names expand imported namespace aliases. A linked native/interpreter fixture
covers nested parsing, serialization, and namespace-qualified names. The
exhaustive gate gained `json_reflection_bridge_parity.jett`,
`json_shape_matrix.jett`, `json_tree_reflection_parse_wrapper.jett`, and
`namespace_use_alias.jett` without losing an earlier object pass;
`json_runtime_reflection_metadata.jett` also gained its `main` behavior match.
Checked generic `type.has_secret[T]()` branches now omit whole secret-bearing
record fields before native serialization, including fields holding collections
of secrets. The trusted reflected decoder inserts already validated refinement
values; native builder insertion checks exact field metadata and type. Native
ownership planning reserves separate temporaries for validated records and
their result wrappers. Linked native/interpreter fixtures cover the nested
serializer and decoder, including error paths. Struct builder insertion of
base values into refinement fields now runs checked predicates at finish;
the same holds for enum and machine payloads, with validation restricted to
the selected variant or state.
The exhaustive audit gained `json_reflection_nested_decoder.jett` and
`json_reflection_nested_serializer.jett`, without losing an earlier pass.
Captured inline functions now extract to checked functions with explicit
capture parameters. An owned descriptor carries both the code address and a
copied environment; indirect calls pass the environment through a hidden
argument. Linked native/interpreter fixtures cover nested closures, loop
captures, higher-order callbacks, and captured strings and narrow/floating
numbers. The 207-row audit gained `captured_local_function_name.jett`,
`closures.jett`, and `closures_advanced.jett`, and `main` execution gained the
first of those, without losing an earlier pass. Before this step, function
values had moved to owned descriptors, with a linked fixture covering copies
through struct fields, lists, and maps; that audit held at 159/182.
Checked source JSON parsing now accepts sets of narrow integers and
primitive-backed refinements as well as the earlier `string`, `bool`, `int64`,
and `uint64` cases. A linked native/interpreter fixture covers duplicate
elimination, boundary values, indexed range and refinement errors, exact
parsing, and a set inside a record. The 207-row object audit remains 159/182
with no lost passes; this fixture is outside that fixed gate.
Checked source JSON serialization now handles sets of primitive-backed
elements, including refinements, directly, through pipelines, and as record
fields. The serializer clones a viewed set before iterating it. A linked
native/interpreter fixture covers integer bounds, a string refinement, and
all three call shapes. The 207-row object audit remains 159/182 with no lost
passes; this additional differential fixture sits outside that fixed gate.
Checked source JSON decoding now accepts `float32` directly and within
supported aggregates by reading a JSON number as `float64` and explicitly
rounding to binary32. Checked source serialization accepts `float32` as well.
The pure `float32.from_float64` conversion works at runtime and in explicit
`comptime` expressions. A linked native/interpreter fixture covers those
paths and shape errors. The 207-row audit gained
`json_parse_exact_primitive_edges.jett` and `json_sized_primitives.jett`
without losing an earlier object pass. The denominator remains 182 because
the added differential fixture is outside the exhaustive run-pass gate.
Checked stdlib JSON decoders now construct `int8`, `int16`, `int32`, `uint16`,
and `uint32` from range-validated `int64` values, including inside supported
aggregates. A linked native/interpreter fixture covers bounds, malformed
shapes, and a record with a `list[uint16]`; its output also matches the
pre-change interpreter. At that narrow-integer checkpoint, the exhaustive
audit kept the same 157 object passes, and both affected fixtures reached
their later `float32` parse blocker.
Direct struct constructors lower checked base-value field predicates into MIR
branches before creating their required `result[Struct, string]`. Native
execution matches the interpreter for successful string and integer fields,
ordered nested predicates, handled failures, and a constructor nested in a
call. Inputs already validated as an intermediate refinement skip their
ancestor predicates; secret-backed integer and owned-string inputs pass their
underlying values to the checked predicate. The same rule applies at explicit
refinement `handle` boundaries. A linked native/interpreter fixture covers
their successful and rejected paths. Exact refined fields retain the success
path. At that checkpoint the 207-row audit remained 157/182 with no regressions; the earlier exact-field change gained
`type_construction_builder.jett`.
The ownership checker now treats `ok`, `fail`, and `some` payloads as moves.
The secret-bearing JSON policy fixture explicitly clones a payload used in
both result arms and now emits a native object; the 207-row audit gained that
one object gate without losing any previous pass.
Self-recursive structs with a finite base and supported fields now specialize
the source JSON serializer, parser, and exact validator. The eligibility walk
accepts a revisited struct without skipping checks of its other fields, and
the existing checked specialization cache keeps the generated call graph
finite. A linked native/interpreter test round-trips a nested `Node` through
`json.parse_exact`; `recursive_owned_values.jett` joins the object gate.
Generic record helpers instantiated for aliases and other non-record kinds now
emit an empty native builder that preserves handled construction failures.
The checker also keeps the reflected source kind in scoped `comptime type`
bindings, so alias JSON validation reaches the aliased base and its renamed
fields. Linked native/interpreter fixtures cover direct alias-builder failures
and nested alias JSON parsing. `json_parse_exact.jett` and
`reflection_type_id_duplicate_aliases.jett` join the object gate.
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
Aggregate payload comparison now uses a recursive type descriptor for lists,
sets, maps, optionals, results, bytes, bitfields, machines, and nested enums.
`tests/native/enum_aggregate_equality.jett` compares matching and differing
values with the interpreter, including collection order, recursive payloads,
signed zero, and NaN. Payloads containing user structs remain unsupported:
their equality requires an exact `Equatable.equals` implementation, and the
current enum comparison path does not dispatch that method. The intended
language rule for this nested case is recorded in
`docs/open_design/enum_payload_struct_equality.md`.
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
and linked `main` gates. Later primitive debug-value formatting adds
multi-binding breakpoints. A recursive type graph now formats aggregate
bindings for trace and breakpoint; bitfields and special values remain open.
Compiler-owned predicate functions and CFG lowering for base-value refinement
boundaries add `integer_nonzero_proofs.jett` to the native object gate. The
native/interpreter fixture covers ordered ancestor predicates, false-result
messages, owned-string cleanup, and successful transfers. The remaining
refinement fixtures advanced to JSON intrinsics or primitive-backed sets.
Predicate runtime failures remain pending; the later slices above add
secret-backed and already-refined inputs plus direct struct construction.
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
`json_parse_exact_primitive_edges.jett` to its then-later narrow-numeric parse
blocker, but did not add an object gate. The checked integer decoders described
above leave `float32` and other JSON shapes pending.
Lists, string-keyed maps, optionals, and results whose leaves are `string`, `bool`,
`int64`, `uint64`, `float64`, `bytes`, or `nothing` now instantiate the checked
stdlib decoder for both public parse spellings. A linked differential fixture
covers nested lists, numeric lists, optional null/present values, map bytes,
indexed list errors, and map value errors. Another linked fixture covers result
success/failure branches, nested results, exact parsing, and malformed envelopes.
At that checkpoint, named aggregates and narrow numeric leaves retained their
existing lowering. Later source decoders cover supported aggregates and narrow
integers; `float32` and other JSON shapes still block the affected fixtures.
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
collections, while retaining the refinement and recursive-type boundaries at
that checkpoint.
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
Handlers inside unary and plain-scalar binary expressions now extract into MIR
before native ownership validation. Boolean `and` and `or` use explicit MIR
branches so a right-side handler runs only when needed. The linked
`tests/native/nested_handle_binary.jett` fixture checks both handler outcomes,
left-value capture before a fallback mutation, unary negation, a binary value
inside a call argument, and short-circuit skipping. Handlers inside list and
map literals, struct, bitfield, enum, and machine constructors, and
optional/result wrappers now extract into MIR. Constructor fields are
evaluated into owned locals in source order before aggregate construction.
`tests/native/nested_handle_constructors.jett` compares native stdout and
debug lines with the interpreter for present and missing values, including a
handled map key, named-field order, checked bitfield width, and owned string
payloads. Indirect function-value calls with owned arguments now extract
argument handlers into MIR in lexical order before reading the callee.
`tests/native/nested_handle_indirect_call.jett` checks both handler branches
and a fallback that rebinds the mutable callback;
native emission also evaluates ordinary indirect-call arguments before the
callee to match the interpreter. Direct view arguments of cloneable values now
snapshot into MIR locals before a later handler can mutate their source. MIR
lowering uses the checked type interner and rejects resource-bearing aggregates
at this boundary. Supported intrinsic arguments also extract handlers in
lexical order; `tests/native/nested_handle_indirect_view.jett` and
`tests/native/nested_handle_intrinsic.jett` compare both paths with the
interpreter. `tests/native/nested_handle_indirect_aggregate_view.jett` checks
that a list view captures the old value before a fallback replaces its source.
`tests/native/nested_handle_borrowed_stdlib_call.jett` checks the same rule for
a source stdlib call with a borrowed list parameter and handled index.
`tests/native/nested_handle_enum_binary.jett` checks that enum equality clones
its left operand before a right-side handler mutates the source, including a
constructed enum and copyable string equality. Borrowed aggregate compiler
`tests/native/nested_handle_refined_binary.jett` compares primitive-backed
integer and string refinements through the same path. Borrowed aggregate compiler
intrinsics, authority-bearing direct views, other refined binary operand
combinations, other nested forms, and source syntax for a handler inside
interpolation remain open.
Field receivers and state tests now extract nested result/optional handlers
before native ownership validation. `tests/native/nested_field_handle.jett`
covers owned and nested fields, defaults, and early-return failures;
`tests/native/nested_state_handle.jett` covers handled state checks in `if`
and `while`, including repeated condition evaluation. Handled `match`
scrutinees and `clone`, `coarsen`, and `declassify` operands have separate
native/interpreter fixtures. `tests/native/nested_for_handle.jett` covers a
handled list iterable with success, default, and early-return paths and
observes one source call per loop. `tests/native/nested_debug_condition_handle.jett`
covers handled assertion and breakpoint conditions, including a default that
reaches a passing assertion. `tests/native/nested_transition_handle.jett`
compares a handled machine transition source and payload with the interpreter
on success and default paths, including their observable evaluation order and
an earlier payload captured before a later handler mutates its source local.
Terminal `return` handlers nested in transition operands are accepted by the
checker but currently fail in the interpreter with `return` cannot escape
expression evaluation, so that parity case remains open.
`tests/native/nested_task_handle.jett`
compares handled `run` and `join` operands with the interpreter for string and
pending `nothing` values, including success and default paths.
`tests/native/nested_actor_spawn_handle.jett`
and `tests/native/nested_actor_message_handle.jett` cover handled actor
constructor, target, `send`, and `ask` expressions; their output checks
lexical argument order and an earlier argument preserved across a later
handler mutation. The 80-test Windows native suite exercises these paths;
the coarse language-coverage estimate remains 80% while other gaps remain
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
and nested capture-free functions. At that checkpoint, closures reading
enclosing locals still awaited native environments.
Checked `list.insert_at` and `list.remove_at` leaves transfer list and element
ownership on success, including owned bytes elements. An indirect callback
returning a string now receives a planned owning temporary. Together these add
`list_extras.jett` to the object gate; linked differential fixtures cover valid
insert/remove positions and string-returning `list.map`. Invalid-index failures
now preserve the signed index in the context-owned terminal message. Dedicated
native/interpreter fixtures compare the exact error text for insertion past the
end and removal at a negative index; runtime leaf tests cover both index signs
for both operations while checking that the list remains unchanged.

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
payloads. `json_serialize.jett` joins the object gate; the Windows gate reached
144/182 emitted objects. Dedicated checked bitfield parse and exact-parse
hooks now decode ordinary fields and unit-enum annotations through reflected
construction. A bounded source decoder converts checked JSON integers to
`uint8` for bitfield payload lists and supported other JSON shapes. Linked
native/interpreter cases cover valid payloads, missing fields, enum shape
errors, range errors, and exact unknown-field errors.
`json_bitfield_shapes.jett`, `namespace_duplicate_leaf_types.jett`, and
`reflection_type_id_duplicate_named_owners.jett` join the object gate. The
current exhaustive Windows gate is 147/182 emitted objects, 23/30 main
outcomes, 25/25 runtime contracts, and 182/182 typed lowerings. Enum payloads
containing raw JSON, other narrow numeric shapes, refinement-validating struct
fields, and other JSON shapes remain.
Checked source JSON parsing now accepts raw `json.JsonTree` values nested in
supported containers and enum payloads. The raw decoder clones a borrowed tree
when returning an owned value. A mixed-payload enum can compare with a known
unit variant by tag without interpreting its aggregate payload. A linked
native/interpreter fixture covers raw enum payload parsing, exact validation,
serialization, and unit comparisons. `json_enum_bitfield_exact_edges.jett` and
`json_parse_collection_edges.jett` join the object gate. The current exhaustive
Windows checkpoint is 149/182 objects, 23/30 main outcomes, 25/25 runtime
contracts, and 182/182 typed lowerings.
Pipeline calls to `json.serialize` and `json.serialize_public` now select the
same checked source serializer as direct calls for supported structured types.
A linked native/interpreter fixture covers both forms, and `pipeline_into.jett`
joins the object gate. That exhaustive Windows checkpoint was 150/182
objects, 23/30 main outcomes, 25/25 runtime contracts, and 182/182 typed
lowerings.
Supported records with refinement fields now use the checked source JSON
decoder. It validates each field as its exact refinement type before insertion
into the reflected native builder. Native verification rejects builder inputs
that could pass an unvalidated base value into a direct refinement field;
accepting those values requires predicate execution at builder finish. A linked
fixture covers valid and rejected JSON, exact unknown-field errors, and reflected
reconstruction. `json_parse.jett` and `json_parse_refinement_valid.jett` join
the object gate. That exhaustive Windows checkpoint was 152/182 objects,
23/30 main outcomes, 25/25 runtime contracts, and 182/182 typed lowerings.

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

Explicit `comptime` values are materialized into typed HIR before MIR lowering,
retaining the checked type and span. The native property-case value materializer
also reconstructs supported composite values: lists, maps, sets, sums, structs,
bitfields, enums, machines, and bytes. The checker propagates contextual expected
types through `comptime`, so a closed `ok` or `none` can adopt the declared sum
type. Ordinary pure calls remain runtime calls. Missing or unsupported baked
values fail before codegen rather than executing their source computation. Native
regressions execute baked `math.factorial(5)` after removing its source file and
compare `tests/native/comptime_composites.jett` with the interpreter. Other
Runtime authority remains outside this materialization boundary. Checked named
functions resolve by exact namespace, name, and signature; inline functions
resolve by their source body span and signature. Evaluated closure captures are
materialized into typed caller-local bindings and copied through the ordinary
native closure environment path. `tests/native/comptime_function_values.jett`
and `tests/native/comptime_captured_functions.jett` compare direct, returned,
and list-contained function values, including captured strings, with the interpreter.


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

The September 2026 actor slice gives spawn expressions a checked constructor
identity and message expressions a checked handler identity in HIR and MIR.
Native constructors allocate context-owned actor state; handlers receive the
actor as their hidden environment, evaluate ordered arguments, write mutable
state back, and return or discard responses with owned-value cleanup. Native
execution tests compare actor state mutation, string ownership, named arguments,
numeric contexts, and duplicate namespace names with the interpreter. The
earlier exhaustive Windows audit reported 174/182 emitted objects (95.6%), 27/30
linked `main` outcomes, 25/25 runtime contracts, and 182/182 typed lowerings.
That percentage tracked fixture object coverage, not overall language coverage.
At that checkpoint, the eight remaining object failures were four Graphics
fixtures and four verify-only fixtures with no reachable code symbols; the
three remaining linked-main failures were scripted Graphics fixtures.

The Graphics parity rows carry the same deterministic key and close events
used by interpreter graphics tests. The native launcher configures those
events before program entry, and the audit obtains its interpreter oracle from
the scripted provider. The runtime exposes typed Graphics session leaves with
scripted open/present/key/close lifecycle, domain failures, and context cleanup.
The compiler emits the private `graphics.__run` callback loop and tracks source
state ownership across updates. The three named-callback Graphics mains now
link and match interpreter output or terminal callback failure.

The September 2026 exhaustive Windows object audit first reached 177/182
emitted objects (97.3%), 30/30 linked `main` outcomes, 25/25 runtime
contracts, and 182/182 typed lowerings. Inline Graphics callbacks with `view`
parameters then raised object coverage to 178/182 (97.8%). Those percentages
measure the fixture set, not overall language coverage.

Ordinary object emission now includes checked `verify` and `property` bodies as
native functions, so the four test-only fixtures emit actual body symbols
rather than empty objects. A property `given` is a typed function parameter;
generation and shrinking stay with the test runner. Native `assert` has a
terminal failure path, and test-body reads clone ordinary owned values to
preserve Jett's relaxed test-block ownership policy. The full 182-fixture
test-body audit now emits 182/182 objects (100%). The regular object gate
checks all 182 rows, with deterministic byte checks on representative objects.
The native verify runner now synthesizes a source-file suite entry that calls
only exact top-level `verify` bodies in declaration order. It runs the 155
verify-bearing fixtures through the production launcher and linker. Extracted
inline callbacks remain reachable through their parent bodies, rather than
becoming independent zero-argument tests. The native property runner embeds
the existing generator's typed cases into a separate suite entry. All 18
property bodies across three run-pass fixtures execute 100 native trials each;
failure-case shrinking and case-specific diagnostics are still pending.

Direct bitfield constructors now route checked dynamic widths through the
existing reflected construction validator, preserving the interpreter's
result and error text. Primitive trace and breakpoint statements now assemble
one debug line from live native bindings. The differential
`tests/native/debug_primitives.jett` fixture covers every fixed-width integer
category, both float widths, booleans, UTF-8 strings, bytes, `nothing`, and
sorted multi-binding breakpoints. `tests/native/debug_aggregates.jett` covers
nested and empty collections, optional and result branches, structs, recursive
enums, machine states, and direct/decoded bitfields against interpreter output.
Custom assertion messages now evaluate on the failed branch and cross the
runtime boundary through a copy-out failure API, preserving the static v1
result contract. `tests/native/assert_messages.jett` checks that a passing
assertion skips an otherwise failing message expression and compiles in native
verify and property suites; a failing message is exercised by backend, runtime,
and launcher tests. Special debug values and cross-function breakpoint binding
scope remain open.
