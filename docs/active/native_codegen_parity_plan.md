# Native Code Generation Parity Plan

Status: accepted implementation strategy; the native pipeline and fixture gates
are implemented, with semantic and release acceptance work still open. The
[acceptance audit](native_acceptance_audit.md) records current obligations;
historical progress estimates below are not measured completion percentages.

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
executes the checked property bodies as native code. Native terminal failures identify the property and trial number. The native
driver also replays and shrinks failed cases using the shared bounded search;
it returns the original suite output and shrunk inputs separately.

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
Cranelift. The Cranelift crates are pinned exactly to `0.132.3`. Native CI tests
the compiler and packages with Rust `1.97.1`:

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

The CLI now validates an explicit `build --target` before reading source and
accepts only the current supported native host (Linux GNU or Windows MSVC on
x86-64). The CLI emits native artifacts using validated installed runtime
manifests; `build --check` retains frontend-only validation. Clean supported-host
package verification remains a release gate.

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

1. **Baseline and harness:** freeze the fixture denominators, classify every
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
9. **Full parity release gate:** all fixture denominators are complete with no
   skips other than explicitly unimplemented features; the full Cargo suite and
   clean-host native suite pass; artifacts run without the source tree; and the
   stable design, architecture, and progress documents are updated to describe
   the proven implementation.

The parity report must publish counts as `passed / denominator` for each
obligation. A single percentage would hide the difference between code
that merely lowers, code that executes successfully, and code that preserves
failure semantics.

The portable `native_conformance` integration suite runs on both supported
x86-64 hosts (Linux GNU and Windows MSVC). It includes manifest-driven execution
of all 30 entry-point fixtures and 25 runtime-contract fixtures, with exact
stdout/stderr and terminal status comparison against the interpreter. Provider
scripts come from the inventory, using the same decoders as the standalone
report tool. Successful runs and entry-failure status 71 both require successful
runtime-context cleanup; cleanup failure cannot count as a matching entry error.
The suite also includes the exhaustive verify and property execution gates,
plus failure probes that prevent a failed
source assertion in a later body from publishing a native executable.
The property diagnostic regression also requires the exact shrunk counterexample,
diagnostic code, source span, and preservation of existing output artifacts for
program, verify-suite, and property-suite builds. This is the required frontend
validation path. Native driver replay and shrinking are verified separately
with a backend-injected failure after successful frontend validation.
The `native_parity` example also discovers top-level verify/property bodies
from parsed source and reports their fixture and body denominators. Its exit
status and `fixture_gates_complete` field certify those fixture gates only;
`complete` remains false while the separately listed release gates are open.
Its object gate uses native test lowering, which roots checked verify/property
bodies as well as program functions; ordinary program-only lowering can produce
an empty object for a valid test-only fixture and is not the inventory gate.

The [acceptance audit](native_acceptance_audit.md) maps these requirements to
their executable gates and separates them from unresolved semantic contracts.

## Current Coverage Matrix

The 2026-09-29 semantic audit found an additional native gap: interface-typed
runtime values and dynamic interface dispatch. Concrete interface method calls
already worked; typed erased boxes and generated dispatch now also cover direct
parameters, returns, stored values, mixed literals, and nested whole-container
conversions. Safe function signature adapters and runtime/baked identity now
cover primitive widths, collections, functions, machine states, and the tested
nominal refinements. The remaining interface audit and cross-platform release
gates are tracked in [native interface values](native_interface_values.md).
Passing the existing fixture denominators alone does not close these gates.

Explicit comptime values now retain each checked generic instantiation and
lexical reflected type-binding chain, including nested callback bodies. The
context fix and its differential regression coverage are recorded in
[native comptime contexts](native_comptime_context.md).

Refinement predicates now evaluate in their declaration context, isolated from
caller aliases, locals, and generic/scoped type bindings. Their input has the
same fully coarsened base type in both execution paths. The regression also
pins original comptime source identity through parentheses; see
[refinement declaration contexts](native_refinement_context.md).

`temporary_projected_views` extends the borrowed-field audit to owners returned
as temporary values. It covers nested list, set, map, Unicode string, and record
fields; direct and indirect calls whose later argument runs a fallback handler;
early returns from that handler; and loop `break`/`continue` cleanup. Exact output
pins single receiver evaluation and ordering, and the native launcher must exit
successfully without cleanup diagnostics.
`temporary_projected_view_failure` requires the original runtime error and
successful cleanup when a later indirect-call argument fails after borrowing
the nested temporary field; both the temporary owner and the failing argument's
owned collection must be released.

`tests/native_parity.json` is the machine-checked fixture inventory. The table
below records implementation coverage; a row is complete only when its native
object, linked execution, and differential behavior gates all pass. Typed
lowering alone never changes an execution row to complete.

For planning, the broad native-language coverage estimate is **about 85%**.
Track it in five-percentage-point steps as the remaining surface gaps close.
This is a judgment based on the matrix, while the fixture counts below are
measured pass/denominator gates.
Later dated notes retain earlier 80% checkpoints as history; 85% is the
current estimate.

| Surface | Validated HIR/MIR | Cranelift object | Linked native behavior |
| --- | --- | --- | --- |
| Fixed-width integers, floats, booleans, and `nothing` | covered | scalar expressions, direct calls, branches, and loops covered | linked native/interpreter scalar, conversion, numeric aggregate, and debug-value fixtures cover representative widths and floating-point edges; a width-by-operation arithmetic edge matrix now covers all integer and float widths, while exhaustive operand combinations remain pending |
| Strings and bytes | covered | all current string intrinsics, direct and projected Unicode-scalar string iteration, separately owned bytes storage, encoding leaves, and checked user-defined `Displayable.display` calls in interpolation | string search/replace, Unicode case changes, scalar `for` loops through nested borrowed fields, byte and encoding fixture functions, nested cleanup, moves/views/clones, and explicit interface display with a same-named inherent method covered |
| Structs, enums, bitfields, machines, and refinements | covered | concrete user structs and enums with typed payloads; bitfield construction and fields, including result-wrapped width validation; machine construction, transitions, state tests and state fields; transparent secret/refinement representation, `coarsen`, checked refinement predicates for base, intermediate-refined, and secret-backed inputs, and result-wrapped direct and reflected struct construction with validated fields | struct moves/views/clones, nested owners, explicit equality and failure cleanup; enum construction, payload transfer, clone, match and cleanup, plus scalar and supported aggregate payload-enum equality; bitfield field ownership, clone, byte roundtrip, and direct/reflected-builder width validation; machine state narrowing, transitions and owned payload cleanup; native/interpreter predicate success and false-result diagnostics for integer, owned-string, and secret-backed refinements, including base values inserted into reflected struct, enum, and machine builders; enum payloads containing user structs dispatch through exact explicit Equatable.equals; broader refinement boundaries remain pending |
| Lists, maps, and sets | covered | scalar/string/bytes/sum/nested lists, primitive and indexed-row list sorting, sortedness checks and sets, and primitive-backed refinement keys and set elements | compiled list access, insert/remove, reverse/repeat, scalar iteration and sort; `list.filter`, `map`, `flat_map`, `find`, `all`, `any`, `count`, `reduce`, `sort_by`, and `group_by` with named, inline, and captured callbacks; `map.filter`, `map_values`, and `for_each` callbacks; indexed-row sorting for interpreter-supported key types; set insert/remove/membership/clone and iteration; map literals, insert/remove/lookup/from_lists/clone and key-value iteration; representative `zip`, `chunk`, `enumerate`, `flatten`, set/list conversions, and map entries; borrowed iteration through nested struct-field and machine-state field collection paths, including owned string map values and Unicode string fields in a machine; refinement string and integer keys and set elements; contextual generic empty lists for struct, enum, and nested-list elements plus borrowed generic aggregate access covered; other projected views and unexercised generic collection shapes pending |
| Numeric aggregates | covered | `math.average` and `math.median` for all ten fixed-width integer and floating-point element types | native/interpreter differential fixtures cover every primitive width and overflow-safe floating-point extremes; empty-list and pending-operand error contracts covered |
| CSV | covered | checked parse, parse-with-header, and stringify leaves over owned lists and maps | strict quoting, CRLF, header values/errors, and nested allocation cleanup match interpreter |
| Crypto | covered | private SHA-256, SHA-512, MD5, and HMAC-SHA-256 byte kernels | native differential fixture covers public text digests, binary HMAC, long keys, secret comparison, and explicit declassification |
| Secret values | covered | transparent scalar and owned representations; redaction and string/bytes comparison | native differential fixture covers equal, unequal, length-mismatched and Unicode strings, bytes, redaction, and aggregate ownership |
| Results, optionals, and `handle` control flow | explicit CFG for statement-root, direct-call, indirect-call, supported intrinsic-argument, unary, cloneable binary, short-circuit boolean, collection, value-constructor, string-interpolation, machine-transition, task `run`/`join`, actor-spawn, actor-message, field-receiver, state-test, clone, coarsen, declassify, condition, match-scrutinee, for-iterable, assertion-condition, and breakpoint-condition handlers | genuine tags, owned payloads and selected extraction | nested sums, defaults, early returns, loop exits, terminal bypass, ordered call arguments and constructor fields, scalar, string, and enum evaluation order, short-circuit fallback skipping, handled field/state projections in `if`, `while`, and `match`, one-time for-iterable evaluation, handled debug conditions, ordered transition source/payload defaults, handled task values including pending `nothing`, actor source/argument order, and interpolated success/failure paths with ordered segment evaluation covered; other nested-expression and refinement handlers pending |
| Function values, closures, and indirect calls | covered; inline bodies extract to checked functions with explicit capture parameters and ownership modes | owned descriptors carry code addresses, copied capture environments, and source-derived debug labels; indirect calls pass the environment after the runtime context and borrow view parameters; parenthesized, returned, projected, inline, and pipeline callee expressions retain checked call order | named, capture-free, and captured callbacks passed, returned, copied through aggregates, and invoked through indirect calls, including view and capability parameters; callee handlers and terminal failures clean already evaluated owned arguments; explicit `comptime` materializes named, capture-free, and captured inline function values from closed pure expressions, including capability-accepting callbacks whose bodies execute only with runtime authority |
| Compiler intrinsics and reflection | covered with checked operands, source-aware reflection metadata, and closed `IntrinsicId` identities | `type.name`, `type.kind`, `type.has_secret`, `type.kind_tag`, `type.primitive_tag`, recursively constructed `type.info`, checked `type.arg`, struct/bitfield, enum, and machine metadata lists and layouts, active enum variant and machine state metadata, reflected field values, and checked reflected-type dispatch covered; other aggregate reflection pending | direct and generic scalar reflection, nested `TypeInfo`, indexed type arguments, struct/bitfield, enum, and machine metadata, active enum and machine state selection, reflected field values, and alias-aware `comptime type` dispatch match the interpreter on positive cases; alias probes remain empty as required; mismatch diagnostics and other aggregate reflection pending |
| Explicit `comptime` values | checked closed pure expressions and contextual expected types covered | evaluated scalar and supported composite values materialize as typed HIR; captured closure values gain typed caller-local bindings; original source bodies are not emitted | linked native/interpreter fixtures cover nested collections, sums, structs, bitfields, enums, machines, bytes, contextual `ok`/`fail`/`none`, named functions, capture-free inline functions, captured closures, and pending `nothing` values, including captured and list-contained values; runtime authority remains unsupported |
| Capabilities and runtime resources | nominal checked types and static Resource copy/payload guards covered | entry grants for all eleven capability types; capability task handles retain authority and pending depth through `run`/`join`; provider effects remain limited to implemented families | Stdout output, Clock/Random sampling, and immutable Environment launch snapshots covered; native/interpreter fixtures compare plain and pending Stdout and Environment debug values, nested capability joins, Stdout cancellation, and a joined Stdout write; successful native runs reject unconsumed scripted Random, Clock, and Graphics inputs; other providers/resources pending |
| Actors and structured concurrency | covered | sequential `run`/`join`/`cancel` values and result propagation; pending depth for `nothing` persists through calls, captures, and aggregates, while owned strings, bytes, lists, sets, maps, optionals, results, structs, enums, bitfields, machines, function descriptors, and `TypeConstruction` builders retain pending depth through runtime handles; actor identities and capability authority retain depth through separate scalar handles; actor constructors, registration, handler dispatch, and state writeback covered; all fixed-width integer, floating-point, and boolean task carriers retain pending depth | native/interpreter differential probes cover successful and failed string tasks, nested pending `nothing` and owned values including optional and result branches, captured functions, builders, actor identities, and capabilities, bare-unit and plain Stdout join failure, current sequential cancellation behavior, pending-value debug output, string and enum direct equality errors, pending function-call, builder-use, and actor-message errors, and aggregate enum-payload equality, actor allocation and owned-state cleanup, message ordering, state mutation, and responses; asynchronous scheduling and cancellation checkpoints remain pending |
| JSON and trusted stdlib hooks | covered | checked `JsonTree` calls use trusted raw stdlib functions; supported structs, machines, collections including sets of primitive-backed elements, and top-level refinements specialize the checked source serializer, including public omission of direct secret fields; supported top-level enums use dedicated checked source hooks; primitive parse calls use private source decoders, including bounded `int8`/`int16`/`int32`, `uint16`/`uint32`, and `float32` construction; concrete structs, bare and state-qualified machines, supported secret wrappers, top-level refinements over supported bases, lists, sets of primitive-backed hashable types, string-keyed maps, optionals, and results specialize the checked source parser; other JSON shapes remain pending | native/interpreter fixtures cover raw trees, primitive and structured serialization, enum unit and payload values, machine state envelopes and public secret omission, primitive and structured parsing, top-level refinement parsing and serialization, secret-bearing records and machines, exact validation, renamed fields, aliases, nested collections including lists/maps of enums and enum payload structs, result branches, errors, and owned cleanup; other concrete JSON types pending |
| Trace, breakpoint, assert, and failure reporting | covered | primitive, capability, function, actor, `TypeConstruction`, and recursive list, set, map, optional, result, struct, bitfield, enum, and machine values trace; zero- and multi-binding breakpoints over those types; default and interpolated custom-message `assert` in test bodies; other special debug values pending | native debug lines match interpreter stderr for scalar and aggregate fixtures, including empty and recursive values, bitfield numeric/enum/payload fields, false conditions, Unicode strings, bytes, `nothing`, plain/pending capabilities, and an out-of-scope local; named, inline, captured, and comptime callbacks inside aggregates, with subsequent calls and failure cleanup; actor ordinals and partially filled struct, bitfield, enum, and machine builders match; custom assertion codegen, context-owned error text, launcher copy-out, and passing verify/property suites have separate tests; lexical breakpoint frames, nested-scope exit, and typed closure captures covered; recursive secret redaction covered, including inferred bindings, generic errors, and reflection failures |

Task ownership planning follows the representation beneath secret and
refinement wrappers. Both `run` and `join` reserve the owned intermediates emitted
for those payloads; erased-interface joins reserve their extracted owner too.
Contextually secret byte tasks previously exceeded the temporary-slot bound
before the expected pending-operand diagnostic could execute.

Numeric and boolean task values now retain pending depth through native locals,
direct and indirect function calls, closure captures, and result payloads.
Linked differential coverage spans `int64`, `uint64`, `float32`, `float64`, and
`bool`, including nested joins and cloned results. Scalar pending depth now
survives struct and enum fields plus primitive list construction, cloning,
insertion, removal, iteration, access, and sorting. Native unary and binary
operators plus boolean branch and breakpoint conditions now reject pending
primitive values with interpreter diagnostics; short-circuit operators retain
the interpreter's right-operand evaluation order. Pending scalar values in
unexamined intrinsic operands remain open.
Numeric list aggregates now reject pending containers and elements before
operating on scalar payload bits, with differential cases for `list.sum`,
`math.average`, and `math.median`.
Public list intrinsics reject pending list containers, and index-taking list
intrinsics reject pending integer indexes with interpreter-matching errors.
Differential cases cover length, append, get, insert, remove, sort, indexed
sort, sortedness, and both swap indexes.
`range`, `bytes.get`, `bytes.slice`, `string.slice`, and `string.repeat` now
reject pending integer operands with interpreter diagnostics. The byte and
string operations also reject a pending receiver. Other intrinsic consumers
of pending values remain open.
Scalar math intrinsics now reject pending numeric arguments before invoking
native kernels, with differential cases spanning every argument-bearing scalar
math intrinsic and each argument position used by the multi-argument forms.
Numeric conversions and `string.from_*` now reject pending primitive operands
with interpreter type errors. Numeric string parsers and byte/text conversion
intrinsics reject pending string or bytes handles before parsing or formatting.
Primitive set elements now preserve pending depth through membership, updates,
cloning, debug output, and iteration. Pending set containers fail at public set
intrinsics. Native `for` rejects pending list, string, map, and set containers
before iteration, matching the interpreter's shared iterable error. Pending
primitive map keys and values now retain pending depth through literals,
updates, lookup, removal, cloning, `from_lists`, and iteration. Pending map
containers fail at public map intrinsics, and pending input lists fail at
`map.from_lists`. Other intrinsic consumers of pending primitives remain open.
Reflected construction retains pending scalar field depth through builder puts,
cloning, trace output, and finished struct, enum, and machine values. Pending
numeric bitfield fields produce the interpreter's validation error.

Capability task handles preserve pending nesting independently of the original
authority token. Joining a task restores the authority needed by the capability
operation; joining plain Stdout follows the interpreter's current cancellation
result. Stdout, Clock, and Random debug values use the interpreter's current
`nothing` representation, while Environment and Graphics retain their named
capability representation. This closes a handle-shaped task gap while the
remaining task semantics above stay open.

Actor handler capability and state snapshots are explicit leading HIR/MIR
parameters, and `respond` participates in native ownership analysis. Checked
actor state initializers lower into constructor functions that source spawns
reference directly. Typed runtime leaves register actor-owned state records,
replace owned fields, and clean them at context destruction. Native actor
allocation, state writeback, and message dispatch pass linked differential
fixtures; true asynchronous scheduling remains a separate task.
Actor state now preserves pending primitive depth across constructor
initialization, handler entry, state updates, responses, and joins. Linked
interpreter/native fixtures cover `int64`, `bool`, and `float64` state, including
trace output and repeated handler calls. This closes the identified actor
state propagation gap while broader task scheduling remains open.

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
`tests/native/projected_machine_sequences.jett` compares list, set, and map
iteration through a state-qualified machine's fields with the interpreter,
including repeated calls on a borrowed machine parameter. The 83-test Windows
native suite includes this projected-view case.
`tests/native/generic_empty_collections.jett` adds linked differential
coverage for generic empty lists specialized to structs, enums, and nested
lists, and for borrowed generic `list.first` calls returning owned aggregate
elements. The 84-test Windows native suite includes this case.
`tests/native/displayable_interpolation.jett` checks that interpolation of a
user-defined value calls its explicit `Displayable.display` implementation in
both the interpreter and native executable. A same-named inherent method
returns different text, so the fixture also checks exact interface dispatch.
The 85-test Windows native suite includes this case.
`tests/native/handled_interpolation.jett` covers a `handle` inside an
interpolated expression. MIR now extracts those handlers and saves interpolated
values in source order before a later fallback can run. The linked fixture
checks successful and fallback paths plus observable segment evaluation order;
the 86-test Windows native suite includes it.
`tests/native/json_nested_bitfield.jett` adds a linked differential case for
bitfield decoding and serialization inside a struct. It covers an enum-backed
field, byte payload, and nested field-path errors for invalid enum and byte
values. The 87-test Windows native suite includes this case.
`tests/native/projected_string_iteration.jett` covers repeated Unicode-scalar
iteration through a string field of a borrowed struct. The linked
`tests/native/generic_map_aggregates.jett` specializes generic empty maps and
lookups for struct, enum, and nested-list values. The 89-test Windows native
suite includes both collection cases.
The 90-test Windows native suite also checks that successful linked programs
reject unconsumed scripted Random, Clock, and Graphics inputs with the same
message and retained output as the interpreter.
`tests/native/nested_borrowed_collections.jett` iterates a map of owned structs,
list, set, and Unicode string through a nested borrowed struct field twice.
Projected strings now use the same MIR root loan and field path as projected
collections, and the native validator admits string loans. The 91-test linked
suite compares output and successful cleanup with the interpreter.

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
`docs/completed/secret_debug_output.md`. The 62-test Windows native suite,
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
signed zero, and NaN. Payloads containing user structs now invoke exact checked
`Equatable.equals` methods through resumable cursors and compiled calls. The
frontend rejects missing methods and retains opaque method boundaries. See
`docs/completed/enum_payload_struct_equality.md` for implementation and linked
debug/release coverage.
Actor handles expose a separate equality mismatch: direct actor `==` is
accepted by the checker but fails in the interpreter, while recursive enum
equality compares actor IDs and native aggregate equality rejects the actor
payload. `docs/open_design/actor_equality.md` records the policy choices;
native actor-payload equality remains conservative until one rule is selected.
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
and an empty script for the exhausted-provider runtime failure. Successful
native entries now reject unconsumed scripted Random, Clock, and Graphics
inputs in the interpreter's priority order, while earlier runtime failures
remain primary. A linked native/interpreter test checks the exact terminal
message, exit status, and retained stdout for each provider.
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
terminal failures now identify the property and one-based trial number. A
backend-injected assertion failure tests the fourth trial of the second
property through linked debug and optimized objects, including cleanup of owned
inputs. The native suite driver now locates and replays failed cases, then
uses the existing shrink search with native execution for every candidate.
The scalar/list regression matches the interpreter counterexample in debug and
optimized modes after source removal. Broader failing-input combinations still
need the same semantic audit as the rest of native codegen.

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

The pending-value parity pass now checks string text operations and
`bytes.length`/`bytes.concat` before their runtime kernels read a wrapped
receiver or argument. `string.join` also diagnoses a pending string inside an
otherwise plain list. Interpreter/native differential fixtures cover unary,
multi-argument, list, and bytes cases. This closes those identified behavior
gaps; the coarse overall language-coverage estimate remains 80% until the
remaining semantic matrix is audited.

Encoding, CSV, and crypto trusted hooks now validate pending inputs against
the interpreter's operation-specific errors. CSV stringify also formats a
pending string field and rejects a pending row. Secret string and byte task
values now use the underlying runtime representation for `run` and `join`,
including contextual promotion of a `run` byte value to `secret[bytes]`.
Differential fixtures cover those paths; the coarse coverage estimate stays
at 80% while broader semantic gaps remain.

Capability intrinsic parity now distinguishes the interpreter's current
pending-value rules: Clock, Random, and Environment operations reject a
pending authority; Environment lookup rejects a pending key; `Stdout.write`
accepts a pending authority and renders pending text. Linked differential
fixtures cover each case. The coarse overall estimate remains 80% while
unresolved language and runtime gaps remain.

Secret comparison and bitfield byte conversion now reject pending operands
with interpreter-matching diagnostics. Bitfield payload encoding detects a
pending `list[uint8]` container or element before writing bytes, while
`secret.redact` retains its unconditional redaction behavior. Linked
differential fixtures cover these cases. The coarse estimate remains 80%.

As of 2026-09-27, the working estimate is **about 85%**. The pending-value
passes above closed identified gaps in strings, bytes, encoding, CSV, crypto,
capabilities, secrets, bitfields, and actor state. This is a five-point planning
step, not a fixture-derived percentage. Projected views and generic collection
shapes, nested handlers, aggregate reflection, runtime providers and
scheduling, concrete JSON shapes, and special-value diagnostics remain
incomplete.

Graphics callback state now carries primitive pending depth from the initial
value through each update result and render call. Scripted interpreter/native
fixtures cover nested pending `int64`, `bool`, and `float64` state; the coverage
estimate remains 85% while the broader gaps above remain.

Reflected `type.field_value`, `type.variant_field_value`, and
`type.machine_field_value` now restore primitive pending depth from the selected
field after metadata validation. Linked differential fixtures cover nested
integer, boolean, and floating-point struct fields plus enum and machine
payloads. Other aggregate reflection and mismatch diagnostics remain open.

Linked reflected-read coverage now also includes pending owned strings, lists,
maps, sets, optionals, and results in struct fields, enum payloads, and machine
state fields. The fixtures check each copied value's debug shape and
successful `join` after reflection; other aggregate shapes and mismatch
diagnostics remain open.

MIR handler extraction now keeps a noncopyable view of an immutable local at
the call site while hoisting the later handled argument into control flow.
This permits a `view Stdout` before a handled optional in direct calls,
indirect calls, and the `Stdout.write` intrinsic without copying capability
authority. A linked fixture compares all three forms with the interpreter.

Native `type.arg` now checks pending, negative, and out-of-range indexes with
interpreter-matching errors before selecting checked `TypeInfo` metadata.
Differential fixtures include nested pending depth and alias-typed owners;
other reflected-metadata mismatch diagnostics remain open.

Pending `int8`, `int16`, `int32`, `uint8`, `uint16`, and `uint32` now have
linked differential coverage through nested tasks, joins, traces, and aggregate
fields. Explicitly handled refinement conversions also preserve predicate
evaluation errors and reject pending boolean predicate results with the
interpreter's message. Direct struct construction and reflected struct, enum,
and machine builder finish paths now share that error handling. Differential
fixtures cover pending predicate results in all four shapes and a predicate
evaluation failure during reflected struct finish. Other refinement boundaries
still need coverage. The current broad planning estimate remains about 85%;
these fixtures do not establish a new five-point step.

Reflected struct, enum, and machine field reads now report the interpreter's
specific owner-mismatch error when the metadata index selects a checked
candidate field but the `TypeField` belongs to another type, variant, or state.
Linked differential fixtures cover all three owner forms; invalid-index and
field-name diagnostics still need parity work.

Reflected field reads now validate a checked field candidate before checking
whether its type can satisfy the requested result type. This preserves owner
error precedence and gives interpreter-matching requested-type errors for
struct, enum, and machine fields. A foreign owner with an incompatible
requested type is covered by the same differential gate.

Native reflected reads now check the `TypeField` owner before searching for a
candidate field. Differential fixtures cover foreign indexes beyond the active
struct, enum variant, or machine state's fields, plus empty active variants and
states. The interpreter's owner error is retained even when no candidate
exists; forged metadata and other invalid-index diagnostics remain open.

`tests/native/json_nested_machine_parse.jett` adds linked differential
coverage for exact JSON parsing of bare machines in lists and string-keyed
maps, state-qualified machines in records and lists, and nested missing-field,
unknown-state, extra-field, and state-mismatch errors. All match the
interpreter; other concrete JSON shapes remain open, so the broad estimate
stays at about 85%.

Native value validation and ownership analysis now treat a contextually
inferred `never` arm as uninhabited rather than an unsupported runtime type.
Linked fixtures cover pending `some`, `none`, `ok`, and `fail` values carried
through reflected struct, enum, and machine fields, plus pending empty lists
and maps embedded in a struct. The interpreter and native paths agree on
debug output, joins, and cleanup. Other semantic gaps keep the broad estimate
at about 85%.

Pending reflection metadata now follows the interpreter at native use sites.
`TypeInfo` and `TypeField` survive `run`, `join`, trace, and subsequent valid
reflection. Unjoined `TypeField` values fail with the interpreter's diagnostic
for struct, enum, and machine field reads and `type.construct_put`;
unjoined `TypeVariant` and `TypeMachineState` values return the corresponding
builder-start error. Linked differential fixtures exercise all of these paths.

Native reflected variant/state selection and struct, bitfield, enum, and machine
field reads now reject pending source values with the interpreter's full value
diagnostic when their checked debug layout is available. Secret-bearing pending
owners still use a conservative native error pending the secret-debug policy.
Field-read validation preserves metadata-before-value precedence;
for enum and machine payloads, a pending source value precedes a foreign field
owner error. Differential fixtures include state-qualified machines, foreign
fields, and both pending metadata and a pending source value.

Linked JSON differential probes now cover nested secret wrappers in lists,
maps, results, and optionals, including an exact-parse error inside a secret
list. A recursive enum also roundtrips through serialize and parse. These
cases match the interpreter and expand evidence for concrete JSON shapes;
they do not close the remaining JSON row or change the broad 85% estimate.

Native string interpolation now renders pending scalar operands with their
nested `pending(...)` wrappers, matching the interpreter for representative
signed, unsigned, boolean, and floating-point values. Primitive JSON
serialization preserves that text for numeric and boolean inputs; a checked
source helper also handles nonliteral `nothing` without dropping argument
evaluation. Pending JSON parse and string-serialization inputs retain the
interpreter's errors. Linked differential fixtures cover the successful and
failing paths. Other intrinsic operands and JSON shapes remain open, so the
broad estimate remains about 85%.

Cloneable local results and optionals now stay available after a native
`handle` extraction. MIR snapshots a local sum before reading its tag and
taking a payload. Linked differential cases reuse empty and present optionals,
results, owned list payloads, and a view parameter. They also cover direct and
indirect calls that stage a struct-field collection view before a later
handler rebinds the parent, plus nested projected map/list iteration. Other
handler and collection shapes remain open; the coarse estimate remains 85%.

Native `run` now snapshots cloneable local aggregate inputs before creating
pending task values. The interpreter leaves the original local readable;
native code previously moved it and rejected a later trace. Differential
fixtures cover optional, result, list, map, and struct locals, comparing both
original and pending debug output. Repeated handles on projected optional and
result struct fields also match the interpreter. Other noncloneable task inputs
and unexamined control-flow shapes remain open at the coarse 85% estimate.

`TypeConstruction` builders are now included among cloneable MIR snapshots.
The interpreter leaves a local builder readable after `run`, while native
lowering previously moved it. Linked differential coverage checks the original
and pending builder, repeated handles on an optional builder, and a borrowed
builder call staged before a later handler rebinds its caller local. Other
noncloneable runtime values and task semantics remain open at about 85%.

Native `coarsen` and `declassify` now snapshot cloneable owned locals before
transferring the transparent representation. Differential fixtures reuse a
refined list after `coarsen` and declassify a secret list twice, matching the
interpreter's local-read behavior. Noncloneable conversion inputs and the
matrix gaps above remain open; the broad estimate stays about 85%.

Linked native/interpreter coverage now exercises an enum payload containing a
user struct with an owned list field: construction, borrowed match, clone,
trace, list storage, and cleanup agree. Equality for user-struct payloads now
invokes exact explicit methods, including through containers, generic owners,
refinements, and machines. Linked debug/release and interpreter/comptime coverage
is recorded in `docs/completed/enum_payload_struct_equality.md`. This closes the
nested-struct equality gate without claiming completion of the full native audit.

Native refinement validation now snapshots a cloneable owned local before
testing its predicate, leaving the source readable after success and inside
the handled failure branch. Linked differential fixtures cover a list
refinement with both a nonempty and an empty source, plus a nested optional
handler that supplies a refinement candidate. Other unexamined refinement
sources and expression shapes remain open at about 85%.

Resolver facts now retain a bare source-function call when its name also
spells a standard module prefix. Linked native/interpreter coverage calls
same-namespace `validate` and `math` functions alongside `math.abs`, checking
that native HIR selects the source functions for bare calls and the standard
module for the dotted call. The broad estimate remains about 85%.

Linked differential failures now cover pending lower and upper arguments to
public `random.int64`. Its source wrapper compares the bounds before reaching
the private bounded-random intrinsic; native and interpreter both reject that
comparison with the same pending-value diagnostic. Pending capability inputs
were already covered separately. The broad estimate remains about 85%.

Repeated `join` of copyable local string and `int64` task values now has linked
native/interpreter coverage, including a later trace of the still-pending
local. The checker rejects repeating `join` on an owned list task after its
first consumption, so the test stays within the language's ownership rule.
This narrows task-value coverage without changing the broad estimate.

Native entry grants now also cover opaque Stderr, Stdin, Filesystem, Network,
Process, and Log values. These capability families have no interpreter I/O
provider, but their accepted value behavior remains part of parity. The
portable conformance suite compares moves, explicit views, indirect calls,
nested task wrapping/joins, trace output, and plain-capability join failure for
all six families. Runtime tests require distinct authority across kinds and
contexts. Implementing their provider effects remains separate work.


### Recursive secret observations and declared debug types

Trace and one-line breakpoint output now use the decided `[redacted]` marker
through secret aliases/refinements and nested collections, records, recursive
enums, machines, and all reflected builder kinds. Pending secret payload depth
is hidden. Typed reflection failures use the same redaction. Native layouts do
not inspect hidden payload handles, and redacted layouts cannot be used for
equality. Public text equal to the marker retains ordinary pending formatting.
Declared debug labels survive canonical alias interning, concrete generic
specialization, reflected body expansions, and closure capture lowering.
Interpreter loop, match, and handler bindings now carry type metadata; generic
handler/pipeline failures use active invocation types. Binding initialization
precedes visibility in a breakpoint. Linked differential coverage exercises
these observations plus seven pending-owner/builder failure paths.

Consumed bindings are omitted from breakpoint snapshots through a shared
compiler-owned exclusion map. Differential coverage includes local transfers,
call arguments, loop elements, handled results, branch joins, mutable
reinitialization, copyable aliases, and actor handlers. Actor snapshots now
exclude caller locals and restore the caller frame after handler completion or
failure. This closes the consumed-binding observation decision; it does not
establish the remaining full-release gates.


### Native release policy

`build_host_executable_with_options` and its program-object counterpart now
apply the same release checker policy as frontend builds. HIR discards complete
trace/breakpoint observations before MIR lowering, including breakpoint condition
effects, while retaining validation of their checked source. Cranelift uses its
speed optimization setting for release objects. The linked release differential
fixture proves debug-condition output is absent, application capability output
remains, debug builds match the interpreter, and forbidden debug printing cannot
publish a release artifact. Runtime bundle selection and CLI packaging are
implemented below; clean supported-host verification remains a release gate.

### Packaged CLI artifact layout

The native CLI uses an explicit versioned launcher manifest, either supplied
with `--runtime-bundle` (or `JETT_NATIVE_RUNTIME_BUNDLE`) or installed at
`lib/jett/runtime/<host>/<debug|release>/launcher.json` beside the package's
`bin/jett`. Each manifest records the compiler package version, native ABI,
target, profile, CRT mode, archive filename, and ordered native link libraries.
The loader validates those inputs against this compiler before linking.

A package also carries `lib/jett/stdlib`; discovery prefers an explicit
`JETT_STDLIB_DIR`, then the installed sibling directory, then the development
source tree. Native `build` publishes `target/<host>/<profile>/<stem>` by default
(with `.exe` on Windows), with `-o` for an explicit output. `build --check`
retains frontend validation without requiring a launcher or entry point.
Packaging builds both runtime profiles and keeps compilation outside the CLI;
the installed compiler never starts Cargo to obtain its runtime.


Local Linux package smoke checks pass from an unrelated working directory:
installed-stdlib discovery, linked debug/release output after source removal,
agent artifact reporting, default output paths, target/profile rejection, and
preservation of an existing artifact after failed builds. The packaging tool
includes dependency notices and a standalone smoke script for clean consumers.
These local results do not substitute for the separate clean Windows/Linux jobs.


### Native property shrinking implementation

The driver retains the deterministic cases and their exact checked function
identities in the same lowering session. After a full native suite fails with
entry status 71 and successful cleanup, it can identify the earliest failing
case by compiling and running shorter prefixes, then confirm that case alone.
This avoids parsing human-readable trace or failure output as a control protocol.
Each replay and shrink candidate runs in a new process and runtime context.
The existing ordered, 50-step shrink search is shared with the interpreter;
only the failure predicate differs. Compiler, linker, process, timeout, and
cleanup failures abort shrinking rather than becoming candidate failures.
Replay recompiles only compiler-owned input constructions against the retained
checked HIR, with no source rechecking or runtime interpreter fallback. This
initial implementation favors an exact existing contract over caching candidate
objects. Normal source validation remains mandatory. A failure that cannot be
reproduced in isolation must be reported, never converted to a passing suite.


### Shrink candidate integer ranges

A generated input and every accepted shrink candidate must remain representable
by the declared `given` type. The audit found that an `int8` minimum input could
shrink to 128: the interpreter treated the resulting range error as another
property failure, while native replay correctly rejected the invalid literal.
Narrow integer pools now retain concrete `Value::Typed` identity
through containers and record fields. The shared shrinker preserves that
identity and filters candidates against its primitive range. Candidate ordering
and the 50-step bound stay unchanged. Plain `int64` and `uint64` carriers already
encode their ranges. The refinement handling below extends this invariant to
user predicates.


### Refinement-preserving shrink candidates

Property generation must retain the canonical refinement identity after filtering
its initial pool. Shrinking may simplify that payload, but an invalid predicate
result is not a property failure. The interpreter validates retained tags
recursively before binding a candidate. Native candidate replay constructs
checked refinement handlers that call the already-lowered predicate functions;
a rejected candidate returns successfully without entering the property body.
Original generated trials and prefix replay keep their existing trusted input
path, so validation does not add observable predicate calls to normal suites.
Predicate chains use declaration identities and the existing handler ordering,
including refinements inside aggregates. No property body or native predicate
falls back to the interpreter.

### Binary32 property input fidelity

The shared property pool previously stored `3.14` as a binary64 value even for
`float32` givens. Execution rounded it at a typed boundary, but counterexample
formatting reported the original carrier instead of the executed input.
Generated binary32 values now retain their primitive identity and rounded
payload. Shrinking preserves both, including when halving a subnormal requires
another binary32 rounding. The binary64 pool and candidate order remain intact.
The native regression compares the fourth-trial failure and exact rounded
counterexample in debug and optimized modes after removing the source.

### Generated generic record identity

Property-generated generic records must carry the same instantiated owner as
source-constructed records. The former pool retained only the base display name:
a `list[Boxed[int8]]` passed as `list[Named]` could not dispatch `Named.name`, even
though a source-constructed list passed. Generation now retains canonical generic
arguments, and field shrinking preserves that identity. The regression covers
two implementations, a transparent argument alias, list erasure, and shrinking
that keeps two simplified records as the smallest failure. Debug and optimized
native replay must match the interpreter after the source is removed.

### Floating remainder

The rejection-path audit found that the checker and interpreter accept floating
`modulo`, but native binary validation admitted only integer operands. Native
validation and emission now cover both float widths through a typed binary64
remainder leaf, with explicit binary32 widening and result rounding. This uses
the established interpreter operation, including signed zero, NaN, infinity,
and zero-divisor behavior. Pending operands still fail before arithmetic and
follow ordinary owned-value cleanup. Linked tests compare debug and optimized
execution, generic calls, explicit comptime results, and pending tasks.

### Public aggregate debug printing

`print` and `println` previously accepted aggregate arguments in the frontend
but failed native signature validation. Native printing now uses the existing
typed debug formatter for public containers, records, enums, machines, bitfields,
function values, actors, capabilities, builders, and erased values. The shared
ownership rule preserves explicit views while consuming ordinary arguments.
Empty literal containers and absent sums have an explicit uninhabited layout
child. Differential tests cover five debug fixture families, argument evaluation
order, borrowed values across handlers, and cleanup without partial output after
a later argument fails. Release rejection remains unchanged. Hidden-secret
printing is a separate unresolved policy, recorded in
`../open_design/debug_print_hidden_secrets.md`.

### Direct collection comparison audit

Direct equality of bytes, lists, maps, sets, optionals, and results previously
reached runtime errors in the interpreter and internal build errors in native
code. The user selected compile-time rejection on 2026-09-30. Both operators
now report E0376 before evaluation or lowering, including aliases, refinements,
secrets, generic instantiations, and explicit comptime expressions. Native
debug/release builds preserve existing artifacts on rejection. The settled
contract is recorded in
[direct collection equality](../completed/direct_collection_equality.md).

### Deep recursive debug values

A finite 160-level enum printed in the interpreter but failed native formatting
with `invalid native trace label`. Typed native formatting now uses an explicit
work stack and active-path cycle detection instead of its 128-level recursion
limit. The linked regression covers pending recursive values, erased owners,
repeated children, trace, print, and secret-bearing breakpoint snapshots.
Runtime tests cover 4,096 levels and malformed cyclic schemas/payloads without
recursive traversal or partial output. Existing aggregate equality remains a
separate audit obligation.

### Deep supported enum equality

Comparing a 160-level recursive enum with itself returned `true` in the
interpreter and an invalid-struct runtime error natively. The existing supported
aggregate equality path now uses an explicit stack with active-path cycle
checks. Child comparisons retain their order and stop at the first unequal
payload, including before malformed later fields. Linked debug/release tests
cover deep values, unequal leaves and lengths, pending nodes, NaN, and signed
zero after source removal. Runtime tests cover 4,096 levels and malformed cycles.
This does not add equality for user structs, actors, interfaces, or direct
collection operands; their recorded policy decisions remain open.

### Deep reflected type identities

A scoped reflected binding for a field with 70 nested list arguments succeeded
in the interpreter and failed native dispatch with invalid `TypeInfo`. Runtime
identity construction now uses an explicit stack instead of a 64-level recursion
limit, retaining the existing identity encoding and alias preservation/erasure
modes. Linked debug/release tests cover both direct and aliased deep fields;
runtime tests cover 4,096 alias levels, shared subtrees, cycles, and malformed
arguments. This changes no language binding or specialization rule.

### Deep interface conversion descriptors

An empty list with 130 nested concrete element types converted successfully to
the corresponding interface types in the interpreter but failed native descriptor
parsing. Parsing now uses an explicit parent stack instead of a 128-level cutoff,
with the same encoding and validation. Linked debug/release tests cover the
accepted empty-container source; runtime tests cover populated deep conversions,
the boxed leaf identity, allocation-failure rollback, truncation, and extra bytes.

### Borrowed transparent values and small refinement owners

The saved transparent-borrow fix now has linked debug/release gates. Refined
`nothing`, `bool`, and `bytes` values retain their interface owners at runtime
and comptime, including inherited unit refinements and nested pending joins.
Coarsening and declassification preserve views through direct and indirect
calls, projected fields, repeated reads, and later-argument handlers. Both
profiles execute after deleting source. Companion failures retain the exact
interpreter error and exit 71 after cleanup when a later argument fails. This
closes the unfinished fixture verification from the saved commit; the open
semantic decisions and supported-host release gates remain in the acceptance
audit.

### Native suite build modes

Verify/property suite lowering, object emission, and executable APIs now accept
explicit `BuildOptions`; their existing wrappers retain default debug behavior.
Release options apply source policy, remove trace/breakpoint execution, and
enable optimized codegen. Three focused regressions pass in both runtime
profiles after source removal, including 100 property trials, default-object
compatibility, optimized-object comparison, manifest profile rejection, and
E0362 publication rollback. Supplemental native conformance now has 308 tests.
The fixed inventory denominator and broad 85% estimate are unchanged.

Before this API change, the frozen `b733296a` workspace (compiled source
`cca40a2e`) passed the complete locked workspace run and doc-tests, including
all 305 then-current native conformance tests and every inventory gate. Later
source changes and supported-host distribution retain their independent gates.

### Deep type syntax recognition

Generic calls and typed local declarations no longer stop recognizing valid
types after fixed 20/40/60-token lookahead budgets. Balanced delimiters are
scanned within the actual logical line, and callable return prefixes are
iterative. Five focused regressions cover complete direct/pipeline requested
types, callable and local type trees, non-consuming bounded recognition, and
malformed-input recovery. All 92 parser tests and 598 frontend fixtures pass.
This preserves the existing grammar and does not claim completion of recursive
reflected-field execution or change the broad 85% estimate.

### Recursive builtin reflected producers

Already admitted list, set, map, optional, and result requests now carry recursive
typed producer plans from HIR through MIR. Structural preflight over every
occupied child completes before new predicates; child checks precede enclosing
root checks, and map keys precede values. Declared exact/ancestor proofs and
shared root prefixes skip established checks. Changed wrappers still require
readiness when their children reuse proof, without joining pending values.
Private projections preserve the candidate and original source through success,
failure, and cleanup.

Generic nominal argument facts come from canonical checker TypeIds, including
unused parameters and aliases. Ordinary nominal payload fields are excluded
from requested-refinement detection, and backend validation visits unused args.
All ten new linked groups pass across 96 cases, every getter/direct/pipeline
form, both runtime profiles, source deletion, terminal errors, and compiled
verify/property suites with 100 trials. Library, object, runtime, and frontend
checks pass; supplemental conformance now contains 318 tests. The fixed inventory
and broad 85% estimate are unchanged. Broader reflected admission, open semantic
rules, and final workspace/distribution acceptance retain their separate gates.

### Selected debug-event isolation

Print/println now join trace and breakpoint in a typed, ordered compiler debug
capture. Application stdout remains capability-backed; native debug transport
uses exact stderr bytes, including partial print fragments and terminal-error
adjacency. Driver and CLI retain actual comptime, frontend verification, and
runtime observations separately, escape agent rows, and preserve preceding
events on failure. Comptime baked values contain no events, and property
shrinking does not duplicate original observations.

Recovery tracks attempted comptime span/context pairs without caching failed
values. Native replay failures preserve the completed original suite's streams
separately from private replay output; initial process errors retain their
existing typed contract.

Release E0362 and current value-formatting/secrecy restrictions remain unchanged.
The independent hidden-secret print choice is still open. Its dedicated phase,
transport, and protocol gates now pass: 416 comptime, 83 driver library, 144
runtime, 50 CLI, and five new linked native transport tests. This closes the
selected channel implementation obligation. The conformance suite now contains
323 tests. The frozen `b1cf8943` complete workspace and doc-tests passed,
including all 182 object obligations and 598 frontend fixtures; the log is
`target/native-workspace-b1cf8943.log`. Its supported-host workflow
37002829148 passed all four Linux/Windows build/package and installed-package
jobs, including clean relocation, source removal, and both profiles.
The prior `3bc23e6a` revision
passed all four supported-host jobs in workflow run 36995400983. Later source
changes retain their own workspace and supported-host acceptance gates.
The fixed inventory denominator and broad 85% estimate are unchanged.

### Qualified machine JSON and parser pipelines

The checker now selects native JSON source bodies from only the chosen payload
of a state-qualified machine. A bare machine still requires every state's
payload to be JSON-compatible, and the invalid selected integer-map payload
retains public E0343 refusal. All-state metadata and qualified construction
checks remain unchanged. This repairs a concrete eligibility defect where an unrelated
integer-keyed map state blocked valid ready-state requests.

Nested `state.fields` loops over the complete `type.machine_states[T]()`
metadata now check candidates for all declared fields and their source aliases
when the request uses a canonical machine or state-qualified type. Field-level
alias identities are preserved; top-level aggregate aliases remain
non-matching probes with empty metadata.
Direct active-state field loops remain selected. JSON eligibility and complete
reflected iteration therefore use separate, existing payload boundaries.

The completed candidate set exposed the private map decoder's incompatible
string-map return for a non-string-keyed T. The selected source repair rejects
unsupported key types in a closed, alias-aware branch before checking that
construction body. Public non-string map requests remain E0343 errors; valid
string-keyed maps are unchanged. Malformed other-state payloads can now report
the private unsupported-key error before a final qualified-builder mismatch.
Required-field and exact-validation precedence, full metadata, and the builder
lifecycle remain unchanged. The bounded tradeoff is captured in the machine
JSON contract before guard integration. Fresh reference characterization
records the unsupported-key error with the `cached.entries` prefix, while
exact numeric payloads retain the earlier expected-object validation error.
Expanded literal controls pin these messages and missing-field precedence
within the earlier passing 22-test linked gate. The later inactive-payload
decoder group also passes separately. The final-source JSON family gate
passes all 39 tests, including the 23 new groups; complete workspace/platform
acceptance remains pending.

The private fallback raw decoder now uses the visible canonical condition
`type.name[T]() == "json.JsonTree"` to select its typed copying body.
Existing checked static selection keeps that body out of inactive callable,
interface, and actor specializations. An ordinary pure metadata helper does
not authorize branch skipping through folding; source validity stays
independent of optimization. Public JSON admission and wrapper paths remain
unchanged.

`json.parse` and `json.parse_exact` pipeline steps also record a checked source
facade from their result payload through the existing direct-call handoff
mechanism. Fresh primitive pipeline witnesses establish that the missing
handoff was an independent defect. Ordinary JSON policy, argument evaluation,
ownership, and step-local handling are unchanged.

The qualified-JSON repair suite has 23 tests over 32 Jett fixtures, covering
direct/pipeline operations, qualified list aliases, handled state/extra-field
errors, input preservation, both profiles, source removal, publication policy,
pure comptime/verify/property execution, exact string-key aliases, and an
independent all-state reflection control with canonical ready/empty qualifiers.
All 22 earlier groups passed (323 filtered) in
`target/native-json-qualified-driver-final.log` before the final fallback
guard. The additional group over three inactive callable/interface/actor
fixtures passes all four direct JSON APIs for the selected ready payload in
both runtime profiles after source removal (one pass, 345 filtered) in
`target/native-json-qualified-unselected-payloads-final.log`. The compiled
native corpus at that checkpoint contains 346 tests. These separate focused
runs do not establish 346 executed passes. The final-source JSON family gate passes all
39 tests (307 filtered), including all 23 new groups and 16 existing JSON
regressions, in `target/native-json-qualified-json-family-final.log`
(438.34 seconds).

Recorded phase checks pass: 40 codegen, 416 comptime, 125 HIR, 79 MIR, and
221 typechecker tests. The driver batch's 82 passes plus one temporary capture
PermissionDenied failure are followed by a passing isolated retry of that
exact test; this is distinct from a clean full-batch result. All 598 frontend
fixtures pass on the repaired source in `target/native-json-qualified-frontend-fixtures.log`
(20.66 seconds). The frozen `23ac34c4` complete workspace subsequently passes
in `target/native-workspace-23ac34c4.log`, including all 346 native tests,
182 object obligations, 598 frontend fixtures, all 83 driver library tests,
and doc-tests, with unchanged head and worktree. Workflow 37013565734 has
passed its Windows build/package job; Linux and clean installed-package
acceptance remain pending. Later test additions retain their own acceptance
gates. The JSON row retains its other concrete-shape obligations;
the broad estimate remains about
85%, and the fixed inventory remains 207 fixtures with 182 object obligations.

### Qualified machine JSON wrapper coverage

Twenty-four additional native tests cross optional, result, and string-keyed map
wrappers with all four public JSON APIs and direct/pipeline forms. They select
the valid ready payload while another state contains a non-string-keyed map.
Occupied/absent optionals, both repeated-type result arguments and arms,
populated/empty maps, and retained owners are covered. Three additional empty
or inactive bare-machine targets retain compile-time E0343 and preserve output
publication in both profiles.

All 27 fixture preflights pass in
`target/native-json-qualified-wrapper-preflight/results.json`. All 24 new linked
tests pass (346 filtered, 190.12 seconds) in
`target/native-json-qualified-wrapper-linked.log`, comparing exact reference
output in both matching runtime profiles after source deletion. The extended
policy group passes (one test, 369 filtered, 7.93 seconds) in
`target/native-json-qualified-wrapper-policy.log`. Source and Rust formatting
checks pass. No production code or public admission rule changes in this slice.

The supplemental corpus now registers 370 tests. This is not a complete frozen
370-test workspace or supported-host acceptance result; the accepted local
workspace checkpoint above contains 346. Wrong-state wrapper diagnostics,
deeper combinations, aliases, secrets, pending values, and independent language
contracts remain open audit work. The broad 85% estimate and 207-fixture
inventory denominator remain unchanged.

### Stable ordinary-struct field local views

Immutable local/view-parameter field aliases now preserve the checked immediate
binding origin. Their existing typed initializer proves each ordinary-struct
owner, field, endpoint, and source-local identity before native compaction.
Owner liveness and non-owning cleanup storage remain unchanged. Conversion
proofs preserve nominal refinements, allowing only existing explicit ancestors,
exact secret declassification, and harmless outer secret promotion.

At the ordinary-struct checkpoint, all 896 compiler-phase library tests and
12 local-alias driver tests passed on its integrated source. The latter include
eight new linked cases with
matching debug/release archives, source removal, independent clones, and later
argument-failure cleanup; the corpus registers 378 tests. Exact logs and scope
are in [the local-view note](native_local_view_aliases.md). These focused gates
do not replace complete frozen workspace and supported-host acceptance.

A qualified-machine field alias passed frontend/reference execution but failed
the native origin proof before the exact-state follow-up below. Mutable/temporary
roots and unresolved lifetime decisions also remain separate obligations. The
broad estimate stays about 85%; the fixed inventory remains 207 fixtures.

### Exact qualified-machine field local views: focused acceptance

Exact declared MachineState field steps now preserve checked immediate origin,
state/source identity, endpoint and nominal qualification through the existing
initializer tree. Persistent loans, non-owning slots, pending checks and tag
offsets remain the existing MIR/emitter contract. Source policy and ABI do not
change; bare flow-narrowed/mutable/temporary origins and owner changes remain
outside bounded support.

Current focused library gates pass 261 tests (41 codegen, 134 HIR, 86 MIR),
and the driver/frontend gate passes 83 library tests and 598 fixtures. The
local-alias gate passes 25 tests (366 filtered, 17.72 seconds), with one
separate typed-endpoint pass (391 filtered, 2.80 seconds): 26 unique executed
cases, not a combined batch. Linked cases execute in both profiles after
source removal; rejection controls preserve publication before archive lookup.
Exact pending owner/intermediate errors and endpoint clone/two-join traces,
owner-change publication refusal, generic copy/borrow facts and nominal/secret
field identities are pinned. Exact logs and initial source-only preflight
distinctions are recorded in
[the local-view note](native_local_view_aliases.md#exact-state-qualified-machine-field-follow-up).

The accepted a0bf12df workspace and all four supported-host jobs are recorded
in [the acceptance audit](native_acceptance_audit.md#exact-qualified-machine-field-local-view-focused-acceptance).
The `774b13e7` complete frozen workspace passes, including all 392 native
tests and 182 object obligations; all four supported-host jobs also pass.
Exact logs and acceptance boundaries are in the audit. This accepted
local revision predates the inferred JSON repair below.

At that machine checkpoint the corpus registered 392 tests. The estimate
remains about 85%; broader view/endpoint shapes, open semantics and independent language rows remain
audit work toward the unchanged whole-language objective.

### Inferred uninhabited JSON serializer slots: focused acceptance

Public JSON policy and actual reference execution admit inferred empty lists,
absent optionals, one-sided results, string-keyed empty maps, and nested
record/collection carriers. The characterized native selector omitted their
Never slot and retained an unsupported generic intrinsic. The applied repair
selects the existing checked source serializer without changing inference,
public type policy, or runtime representation. Parse eligibility remains its
own characterization and implementation obligation.

Actual pre-change captures, the pending-carrier failure boundary, and the
executed linked/compiler gates are in
[the scoped contract](native_json_uninhabited_slots.md). Source-body checking,
carrier evaluation and cleanup cannot be replaced by optimizer folding or an
empty-value exemption for unsupported inhabited targets. These evidence and
implementation states do not raise the broad estimate or change the fixed
inventory; the full 100% language objective remains intact.

The further actual pure/comptime/reference-suite and public-policy controls are
recorded in that note along with the separate projected-owner reread witness.
All 14 new driver tests pass together, covering the original carriers, pure
native suites, public projection, grouped negative publication controls and
owner rereads. The crossed compiler gate passes 948 library tests, followed
by 83 driver library tests, all 598 frontend fixtures and 17 Jett format checks.
The serializer revision `05494ed4` passes its complete 406-test frozen native
corpus, 182 object obligations, remaining workspace/doc-tests and all four
supported-host jobs. Exact logs and counts are centralized in
[the acceptance audit](native_acceptance_audit.md#current-frozen-revision-and-inferred-json-follow-up).

### Inferred uninhabited JSON parser slots: separate selected repair

The existing checked decoder returns an error when raw input tries to populate
an inferred Never slot; empty/absent carriers and supported one-sided result
arms still decode. The bounded source-selector repair preserves that slot and
retains the error-producing result[Never, string] specialization, ordinary raw
parse/exact validation and field/index/arm prefixes. It changes no public JSON
domain, inferred default, body-validity rule, runtime representation or ABI.

Actual pre-change source characterization covers eight direct and eight pipeline
sources across four inferred shapes and both parse APIs, plus three recursive
carrier sources with their actual field/key/index failure prefixes. All 19
linked cases now pass together in both profiles after source removal; the
crossed compiler gate passes 949 tests, followed by 83 driver library tests,
all 598 frontend fixtures and all 19 new source format checks. The corpus
registers 425 supplemental tests, separate from this executed focused gate.
The parser revision 08f9f7e7 subsequently passes its complete frozen
425-test workspace and all 182 object obligations. Its supported-host workflow
also passes all four jobs. Exact accepted-head evidence is
centralized in [the acceptance audit](native_acceptance_audit.md#current-frozen-revision-and-inferred-json-follow-up). The scoped
evidence and acceptance requirements are in
[the parser contract](native_json_uninhabited_parse_slots.md). This follow-up
does not increase the broad about-85% estimate, change the fixed 207 inventory,
or narrow the whole-language 100% objective.


### Bitfield field local views: bounded follow-up

The existing readonly field rule now has a declaration-backed Bitfield step in
the native initializer proof. Exact field identity/type, source/owner anchoring,
qualification, immutable loans and non-owning cleanup remain unchanged. Existing
numeric copies, owned field copies, explicit clones and pending checks retain
their behavior; no new ABI, payload shape or lifetime rule is introduced.

Both focused HIR tests and all seven new linked cases pass. The latter execute
both matching runtime profiles after source deletion and preserve exact output,
owner rereads, empty payloads, pending-owner failure and ordered copy-join traces.
The crossed compiler gate passes 951 library tests, followed by all 83 driver
library tests, 598 frontend fixtures and seven source-format checks. The corpus
registers 432 tests at its checkpoint. The later clean `cb617ecf` full workspace
includes the repair and passes 439 native tests and all 182 object obligations.
Supported-host acceptance passes at `5d919052` and the later documentation-only
`194ce065` revision. Required proof, pre-change captures and exact executed logs are
in [the bitfield contract](native_bitfield_projected_local_views.md) and
[the acceptance audit](native_acceptance_audit.md#bitfield-field-local-views-focused-acceptance).
The broad estimate stays about 85%, the fixed inventory stays 207, and the
whole-language objective remains 100%.


### Direct secret aggregate local views: bounded focused acceptance

The declared-owner proof now preserves the already selected one-direct-Secret
receiver rule and exact endpoint qualification. Nothing and already-secret
nominal fields remain exact; other fields gain one Secret layer. Existing root,
immutable-loan, pending and non-owning cleanup contracts are unchanged.

Three focused HIR tests, all 954 crossed compiler library tests and all seven
linked cases pass. Six cases pin successful outcome parity; the pending receiver
pins raw reference error and existing native redaction separately under the
[open observation policy](../open_design/debug_print_hidden_secrets.md), with
matching partial stdout/status and preserved cleanup. It does not establish
diagnostic parity or select new policy. All 83 driver library tests, 598 frontend
fixtures and seven source-format checks pass. The exact clean `cb617ecf`
workspace passes all 439 native tests, all 182 object obligations, remaining
workspace targets and doc-tests, with unchanged head and exit zero at
`2026-10-02T20:55:57.0941072Z`. The documentation-only `194ce065` revision
passes all four supported-host build/package and relocated installed jobs in
[workflow 37064035604](https://github.com/vycdev/jett/actions/runs/37064035604),
with identical compiler/runtime/test sources. Exact
proof, captures and required resolution are in
[the secret-owner contract](native_secret_owner_projected_local_views.md) and
[the acceptance audit](native_acceptance_audit.md#direct-secret-aggregate-local-views-focused-acceptance-and-open-observation).
This changes neither hidden-secret observation policy nor the independent
unapplied MIR handler staging. Whole-language 100% remains the objective,
with about-85% tracking and the fixed 207 inventory unchanged.


### Inferred uninhabited callback inputs: descriptor storage and invocation

Native codegen now distinguishes retaining a callback descriptor from invoking
its original body. An exact non-capture Never input permits normal owned
descriptor storage, captures, cloning, pending state and cleanup through a
nonzero internal trap entry, with no Never scalar or return payload. Original
typed metadata is checked before preparation; unused enclosing-frame slots from
inline extraction compact without deleting parent blocks or typed dependencies.
Surviving callable Never inputs and forged values remain rejected.

All 966 compiler library tests and all nine source-deleted linked cases pass.
The latter exercise both matching profiles with exact success/failure outcomes,
captured ownership, pending joins, real integer calls, ordinary loop bodies and
equality inside uncallable bodies. Nine source-format checks and Rust formatting
pass, as do all 83 driver library tests and 598 frontend fixtures.
The corpus registers 448 supplemental cases at this checkpoint; the repaired
`90d82701` complete-workspace/object and supported-host result below subsequently
accepts that source scope.
Pre-change characterization, initial failures and selected proof boundaries are
in [the descriptor contract](native_uninhabited_callback_descriptors.md) and
[the acceptance audit](native_acceptance_audit.md#uninhabited-input-callback-descriptors-focused-acceptance).
This does not complete all callback-body operations or the independent borrowed
handler staging. Whole-language 100% remains the objective, tracked at about 85%
with the fixed 207-fixture inventory.

### Original function metadata repair after the full callback run

The exact clean `d14b830b` native suite completes with 446 passes and two
sum-arm failures. Original preflight demands a native result carrier for a
closure that checked impossible optional/result arm preparation later removes.
The remaining workspace/object targets were stopped after this complete native
suite failed; its log and terminal metadata retain the unchanged clean head,
446/2 outcome and explicit stop reason. It is not full acceptance.

The recursive OriginalMetadata/NativeValue type-validation distinction preserves
the exact Never result as original metadata, without admitting a surviving
return-only callback or Never capture. All 969 compiler library tests, both
previously failing linked sum-arm tests and all nine callback tests pass. Three
new codegen units cover absent-arm closure removal and strict nested metadata
and live-value negatives. See
[the regression evidence](native_uninhabited_callback_descriptors.md#full-run-regression-and-original-function-metadata-repair).
The exact clean repaired revision `90d82701` subsequently passes the complete
workspace with an unchanged head: all 448 native tests (944.48 seconds), all
182 object obligations in the four-test manifest gate (553.77 seconds), all
83 driver library tests and 598 frontend fixtures, remaining targets and
doc-tests. The wrapper exits zero with 92 successful result groups at
`2026-10-03T08:31:51.9056689Z`; exact log/metadata are
`target/native-workspace-90d82701.log` and `.json`. Its supported-host workflow
[37108099164](https://github.com/vycdev/jett/actions/runs/37108099164) passes all
four Windows/Linux build/package and relocated installed jobs, last completing
`2026-10-03T09:03:23Z`. Corpus 448, inventory
207, about-85% tracking and the whole-language 100% objective remain unchanged.

### Internal call-view scopes across handlers

Original explicit view arguments now have eager typed Begin/End stages for
noncloneable exact local and field origins. A protected terminal mutable owner
may be rebound after the operation; an owned temporary evaluates once into
ordinary owning storage. Complete results precede End, and aborted owned
returns/loop exits abandon only their operation's internal loans. Persistent
source aliases and same-operation owner-change safety remain separate.
See [the implementation and focused gates](native_scoped_call_view_staging.md).
Whole-language coverage remains about 85%, with the fixed 207-fixture inventory
and 100% native-language objective unchanged.

### Latent nested bodies and direct secret callback results

Exact Never-input callbacks now preserve source-valid nested closure metadata
and one direct Secret[Never] result without demanding a runtime carrier. Typed
function/closure/adapter edges are checked before a separate cycle-safe original
body traversal; this does not add native reachability or relax live callable,
capture, nominal, Resource or hidden-secret policy guards. The source remains
checked before any body pruning. No runtime or ABI change is required.

The isolated follow-up passes 63 codegen tests, all 27 linked regressions and
1052 tests in the broader compiler run; the count-guard wrapper discrepancy is
recorded separately from passing tests. All 83 driver, 598 frontend and eight
backend-lowering tests also pass. Exact clean `72237c4a` subsequently passes its
complete 464-case workspace, all 182 object obligations and 92 successful result
groups; its supported-host workflow remains in progress. The preceding
`55027914` staging revision passes all four supported-host jobs as well as its
complete workspace. See
[the centralized proof and gates](native_uninhabited_callback_descriptors.md#latent-nested-closures-and-direct-secret-results).
This bounded repair changes neither the fixed 207 inventory/182 object
obligations nor the about-85% estimate and whole-language 100% objective.

### Opaque resource type-only metadata

The builtin `TypeKind` enum now admits the already selected `resource_type`
variant, and the shared reflection mapper returns the same tag. Existing enum
indices, alias tags and empty opaque shapes are preserved. Source checking,
reference metadata and native object emission are tested without a Resource
value. The affected 800 compiler tests and 598 frontend fixtures pass; the
subsequent clean `616b51b9` full workspace also passes. Supported-host acceptance
for that head remains separate. See
[the contract and executed evidence](native_resource_kind_tags.md). Live resource
hooks, carriers, provider authority and exactly-once cleanup remain open.

### Checked private resource-hook identity prerequisite

Compiler-supplied private resource declarations now retain exact Resource and
Function DefIds, independent stdlib origin evidence, normal privacy/order and
complete checked function ownership modes. Explicit fallible compiler entries
validate all facts; default production resolution has an empty catalog. No
public provider spelling, live carrier, resource hook dispatch or native drop
admission is introduced. The selected pre-Rust contract and implementation are
in [the active identity note](native_resource_hook_identity.md).

All 14 focused resolver/checker/span tests pass with stable source hashes in
`target/native-resource-hook-focused-after-diagnostic.json`. The initial helper
mutability compile error and corrected source-body E0311 assertion remain
separate failure evidence. The broader 1037 compiler library tests, 63 codegen
object integration tests, 83 driver tests, 598 frontend fixtures and eight
backend-lowering tests also pass with unchanged sources. Clean `616b51b9`
subsequently passes its full workspace; its supported-host workflow remains
in progress. Reference provider installation, ordered source cleanup,
HIR/MIR transfer/drop proofs and native lifecycle execution still remain. This
prerequisite changes neither the fixed 207 inventory/182 object obligations nor
the 464-case supplemental native corpus, about-85% estimate or full 100% goal.


### Static Resource ownership prerequisites

Resource-bearing owned field copies and cloning collection/reflection getters
now report E0364. Known borrowed Resource data cannot be acquired by owning
optional/result, list/map, struct or machine payloads (E0401); a Resource-bearing
exact-state transition source retains the existing owned-argument boundary.
Exact Resource-root printing and value reflection report E0300. Safe explicit
views, unrelated copyable fields, real owned transfers, empty carriers and
type-only metadata remain admitted. Nested printing policy is unchanged.

All 36 focused resource groups pass, including 21 new source groups. The 34
frozen synthetic-stdlib sources are rechecked in debug and release: selected
copy/acquisition/root-observation gaps are refused and owned/view/empty controls
are retained. The broader 1058 library units, 63 codegen object integration
checks, 83 driver tests, 598 frontend fixtures and eight backend-lowering tests
also pass with three unchanged source hashes. Details and the separate accepted
`616b51b9` predecessor record are in the
[ownership note](native_resource_ownership_boundaries.md) and
[acceptance audit](native_acceptance_audit.md#resource-static-ownership-boundaries).
This is checker-stage safety evidence. Resource carriers, fake/real providers,
source cleanup and native lifecycle execution remain required. The new full
workspace and supported-host gates are pending; 85%, 207 fixtures, 182 object
obligations and the 464-case supplemental corpus are unchanged.


### Immutable checked Resource program prerequisite

The exact clean static-guard head `b7180bd9` now passes its full workspace:
all 92 result groups, 464 supplemental native cases and 182 object obligations.
Its supported-host workflow 37123835396 also passes all four Windows/Linux build
and installed relocation jobs. The earlier `616b51b9` four-job result remains
historical. These are separate commit-specific receipts in the acceptance audit.

The next compiler handoff retains one owned parsed Module, ResolveResult,
CheckResult/interner and explicit loader origins, derived internally and exposed
only through shared references. Parser, resolver and checker errors prevent
sealing; public hook validation rejects checker errors independently while
ordinary checking retains source repair diagnostics. All 43 focused Resource
groups pass, including seven new snapshot groups; all 454 resolver/typechecker/
HIR library tests pass with five unchanged compiler source hashes. The
[checked-program note](native_resource_checked_program.md) records exact scope.
This supplies no provider, runtime Resource, cleanup or native ABI. Private
reference-driver retention is now implemented in the following bounded
follow-up. Program-bound invocation eligibility and exact caller ownership
handoff remain required; a formal View mode alone does not recover caller
retention. Full-workspace and supported-host gates remain commit-specific.
The about-85% estimate, fixed 207 inventory/182 object obligations, 464 native
cases and whole-language 100% objective remain unchanged.


### Retained reference preparation

A private driver result now retains the exact sealed checked Module and primary
FileId0 main span/namespace through reference execution. Public BuildResult is
unchanged. Runtime uses one merged registration and performs no source read,
parse or module rediscovery after preparation. The earlier macOS graphics
thread-selection read remains outside this bounded repair. Phase errors retain
all preceding warnings in order, and metadata errors retain their typed cause.
Required comptime and verification still run once and gate execution before
providers. No Resource value, provider, hook dispatch or native ABI is enabled.

All 46 focused Resource groups and four new driver groups pass with stable
source hashes. All 544 driver/HIR/resolver/typechecker tests also pass with those
hashes unchanged. All 598 frontend fixtures and eight backend-lowering
regressions also pass. New full-workspace/platform acceptance remains pending. The exact94 predecessor
workspace failed after a NamedTempFile.reopen PermissionDenied; its affected
native case passes an isolated rerun, not whole-workspace acceptance. See
[the reference contract and proof](native_prepared_reference_driver.md) and
[centralized acceptance context](native_acceptance_audit.md#retained-reference-preparation).
The about-85% estimate, fixed 207 inventory/182 obligations, 464 supplemental
native cases and whole-language 100% goal are unchanged.

### Typed caller ownership integration

The caller disposition selected by Rule24 now survives exact typed source,
HIR, MIR and native handoffs. Separate sealed proofs cover generated calls,
temporary staging, viewed loops and handled defaults; Rule25 data snapshots
preserve existing interface conversion. An earlier formatted integration
checkpoint passed 293 checker, 598 frontend, 168 HIR, 147 MIR, 67 backend and
63 object tests. At that checkpoint all 15 linked caller groups passed in both
profiles after source deletion, and the two failed driver property replays
passed independently.

The current formatted compiler-core gate passes 304 checker, 188 HIR, 196 MIR,
70 backend and 63 object-emission tests, plus the MIR doctest. These current
counts remain separate from full-workspace, complete-native and supported-host
acceptance. Per-pass signature projection reuse preserves all validation
boundaries and exact diagnostics.

The bounded handled-conversion follow-up now passes all 66 MIR acquisition
groups and a new linked payload-reading fixture in both profiles after source
deletion. Captured mutable plain `TypeConstruction` generations pass seven MIR
groups, three native emission groups and the original builder native regression.
All seven failures of the historical 479-group run now pass individual replays.
These paths preserve exact private Source/CFG authority and add no source clone
or ABI permission; runtime interface boxing may copy its payload. Broader
simultaneous cohorts and other owner kinds remain open.

The [contract](native_call_ownership_facts.md) and
[acceptance record](native_acceptance_audit.md#typed-caller-ownership-integration)
separate these focused results from the later complete Windows native receipt:
480/480 pass at exact committed `a63f891016fac7dc50fffdde33be8b0ce45c655f`,
with stable HEAD/tracked sources in
`target/native480-after-owner-generation/receipt.json`, ending
`2026-10-04T14:20:12.987780+00:00`. Resource changes are absent. Full-workspace,
current 182-object inventory and supported-host acceptance remain pending. The a9 predecessor is accepted locally
and on all four supported-host jobs. The fixed 207 inventory, about 85% tracking
and 100% whole-language goal remain unchanged. A separate private reference Resource lifecycle now passes 36 focused tests,
including actual Source cleanup and required-worker controls. Production
provider dispatch and native ownership/cleanup remain required.


### Private reference Resource lifecycle

The exact checked program now supports private reference hook descriptors,
runtime-purpose invocation gates, non-cloneable owning tickets and ordered
custody through Source calls, scopes, temporaries and occupied results.
Ordinary/mutual declarations rejoin their retained executable AST and exact
resolver metadata; registration clones cannot replace checked bodies. Caller
source order and formal permutation remain separate. Explicit close and reverse
cleanup retire owners before context teardown. Cleanup failure overrides entry
failure; clean cleanup preserves an ordinary entry error and source result.fail
remains data. The public production catalog and provider remain disabled.

Root terminal `923450` passes all 36 Resource-focused reference tests in
`target/native-resource-after-checked-call-relay-focused.log`, including real
both-profile Source lifecycles, failed later actuals, indirect close, terminal
failure and required-worker/purpose controls. This is separate from the committed
caller native480 receipt and adds no native inventory case. Broad library,
full-workspace and supported-host acceptance of this Resource candidate remain
pending. See [the exact scope](native_resource_hook_identity.md#reference-lifecycle-execution).
Whole scoped/reflected/indirect/worker and aggregate ownership breadth, real
providers, native hook/carrier/drop integration and task/actor cancellation remain
required. The about-85% estimate, fixed 207/182 inventory and 100% goal are unchanged.

### Source plain Resource replacement

Native assignment now executes exact checked plain Resource replacement, self-reseat and vacant-after-close transfer. Six new shared scenarios extend the original five; all 11 pass in both real-reference profiles and as generated native programs after Source deletion. Runtime preflight preserves invalid inputs before finalization, and failure tests distinguish an ordinary RHS error from old-finalizer cleanup failure. The formatted compiler gate passes 558 checks plus the MIR doctest and606 frontend/lowering checks pass. One temporary capture failure in the broad driver-library run passes isolated replay; clean full-library, complete native regression and supported-host results remain separate gates. Conditional/aggregate/projected replacement and broader Resource/provider/async/reflection work remain required. The coarse estimate remains about 85%, with fixed 207 fixtures and 182 object obligations unchanged.

### Handled Resource call progress (2026-10-05)

The local native Resource gate now includes eighteen shared original Source
scenarios in both profiles (36 native and 36 reference executions). Direct Result
handlers inside call arguments preserve lexical evaluation, prepared formals,
acquired-prefix cleanup, retained views, clean skipped invocations and observed
cleanup failure. Nested first-factory success/failure also distinguish Scope
storage from the current execution frame. Every native case runs after Source
deletion and observes zero obligations before destroy.

The formatted affected compiler/object gate passes 573 checks plus the MIR
doctest, with 606 frontend/lowering fixture checks, all four fixed inventory
gates (182 emitted host objects), and the complete 92-test driver library.
The final formatted Source replay passes all 36 native and 36 reference runs.
This advances the Resource row without closing its remaining indirect, aggregate, refinement and broader
control-flow obligations. The broad estimate remains about 85%; full current
native/workspace and supported-host release gates remain separate requirements.

### Callable cleanup boundaries (2026-10-06)

Three new original Source programs extend the shared lifecycle gate from 18 to
22 scenarios. Both reference and native profiles pass all 44 executions each;
native launch follows Source deletion and observes exact reports and zero
obligations before teardown. The additions cover reversed lexical/formal order
with two temporary view owners, a Return helper using an independent owner,
a continuing default, and cleanup panic suppressing the Return helper.

The reference evaluator now retires the pending call prefix before evaluating
the original Return operand. Ordinary closure and named-callable bodies have
their own operation floor and no inherited Resource return destination. Native
Source joins retain authenticated terminal failure outcomes, keeping original
body status separate from cleanup instead of replacing status 255 with 1.

The affected gates pass all 508 reference-library tests, all 92 driver-library
tests, and all 222 runtime tests (209 unit and 13 integration). Six private
Return-prefix controls and eleven focused native Source controls are included
in those totals. Fresh matched debug/release archives contain one main, eight
private exports and all 32 Resource leaves each, with the unchanged ABI/wire.

Parent a30ca97e's exact source tree passed the complete workspace on GNU and
MSVC, including 486 default native tests per host and both source-absent installed
package jobs. The local full run had 26 deadline failures; all 26 subsequently
passed at the same deadlines, in a 25-case replay and an isolated inventory
replay. This is not a clean full local run. The successor's complete workspace
and supported-host checks remain separate release gates. Overall tracking stays
about 85%; indirect/aggregate/refinement Resource calls, broader projections and
reflection, production providers and shared scheduling remain in the full goal.
