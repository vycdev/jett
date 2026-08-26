# Initial C FFI and Generated Binding Contract

Status: proposed decision for [#53](https://github.com/vycdev/jett/issues/53).
Parser, generator, and native backend implementation remain pending.

## Context

Jett describes `jett bind` as an ahead-of-time C header translator, but the
existing examples are sketches rather than a compiler contract. In particular,
`# foreign: symbol_name` is only a comment convention: the lexer discards
comments, `FunctionDef` has no foreign metadata, every parsed function requires
a body, and the current build path stops after checking. The repository also
has no `jett_bind` crate, C parser dependency, native code generator, or linker.

The old sketches additionally ask the generator to infer ownership and errors
from names such as `Create`, `Free`, or a negative return. Those guesses are the
same kind of plausible but unsafe behavior that generated bindings are meant to
avoid. A header describes representation and calling convention; it usually
does not describe who owns a pointer, what encoding a character pointer uses,
or which return values mean failure.

This contract chooses a first-class source declaration for foreign symbols, a
small target-resolved C subset, and an explicit policy input for semantics that
a header cannot prove. It also separates parsing and deterministic generation
from future native execution. It does not implement C ingestion, native code
generation, C++, callbacks, arbitrary layouts, or dynamic library loading.

## Decisions

The initial boundary follows these rules:

1. Foreign metadata is syntax, never a semantic comment.
2. Every binding file names one target triple and one `C` ABI.
3. Header facts come from a target-configured C frontend; ownership, text,
   fallibility, and effects come from explicit binding policy.
4. Every foreign call visibly borrows a `Foreign` capability.
5. Private ABI declarations and public Jett wrappers are separate declarations.
6. Unsupported declarations fail deterministically instead of being guessed or
   silently skipped.
7. Generated source is reproducible and checked in like other vendored source.
8. Parser/checker support may land before any foreign call can execute.

These are source and compiler contracts. A particular libclang version, LLVM
version, linker, or runtime representation is not part of the language API.

## Canonical Source Form

The reserved top-level form is a `foreign` block:

```jett
namespace counter

# The digest is a format-valid placeholder in this design example.
foreign source "counter.h" digest "sha256:cab2fc80c5b97bc735d2e561e68bc8682638ea957b23ad9f29807f4d45ac80f6" target "x86_64-unknown-linux-gnu" abi "C":
    export opaque pointer Counter to "counter"
    function create_raw(view ffi: Foreign, initial: int32) returns optional[Counter] links "counter_create"
    function read_raw(view ffi: Foreign, view counter: Counter) returns int32 links "counter_read"
    function destroy_raw(view ffi: Foreign, counter: Counter) returns nothing links "counter_destroy"

export function create(view ffi: Foreign, initial: int32) returns result[Counter, string]:
    Counter handle = create_raw(view ffi, initial) handle:
        return fail("counter.create: C returned null")
    return ok(handle)

export function read(view ffi: Foreign, view counter: Counter) returns int32:
    return read_raw(view ffi, view counter)

export function destroy(view ffi: Foreign, counter: Counter) returns nothing:
    destroy_raw(view ffi, counter)
    return nothing
```

The wrapper body in this example is abbreviated only where the current
optional-matching surface would distract from the FFI declaration. Generated
files contain complete ordinary Jett bodies.

The canonical grammar shape is:

```text
foreign_block := "foreign" "source" STRING "digest" STRING "target" STRING "abi" STRING ":" NEWLINE INDENT foreign_item+ DEDENT
foreign_item  := ["export"] (opaque_item | function_item)
opaque_item   := "opaque" "pointer" IDENT "to" STRING
function_item := "function" function_signature "links" STRING
```

`source`, `digest`, `target`, `abi`, the C spelling after `to`, and the linker
symbol after `links` are semantic string literals preserved in the AST. The
digest is lowercase `sha256:<hex>` over the canonical generator version,
logical header contents and transitive selected includes, normalized frontend
arguments, target, and policy bytes. Comments may add human context, but changing
a comment cannot change a binding.

A foreign function has a signature and no Jett body. It is private unless
explicitly marked `export`, using the same namespace visibility rule as every
other declaration. `jett bind` does not export raw declarations: it emits
private `_raw` declarations followed by the public wrappers selected by policy.

Foreign blocks are generated-source syntax, not a general handwritten escape
hatch. For an output such as `deps/counter.jett`, `jett bind` also writes and the
project loader requires `deps/counter.jett.bind.toml` with this version-1 shape:

```toml
version = 1
output = "counter.jett"
source = "../vendor/counter.h"
policy = "../bindings/counter.toml"
target = "x86_64-unknown-linux-gnu"
generator = "jett_bind 0.1.0"
digest = "sha256:<64 lowercase hex digits>"
frontend_args = []
```

Paths are relative to the sidecar and cannot escape the project or dependency
root. The loader accepts a foreign block only when the adjacent sidecar names
that file and its source, target, generator, and digest exactly match the
semantic block. `jett build` invokes the same in-memory regeneration as
`jett bind --check`; the selected header, transitive selected includes, policy,
and configured frontend must therefore be available for a native FFI build. The
output and sidecar are replaced together only after both validate. A missing
sidecar, unknown key, unavailable input, absent or malformed digest, edited
output, or stale input is an error. This prevents accidental ABI drift; it is
not a cryptographic attestation that a C library is honest. The selected header
and reviewed policy remain trusted native-build inputs, and the native linker
cannot prove that a library's implementation matches its header. Jett therefore
still guards every call with `Foreign`.

Only one foreign block is allowed in a generated file. It must follow the
namespace and any types needed by its signatures, and its wrappers must follow
the block. Ordinary strict top-to-bottom visibility applies. Foreign functions
cannot appear in `mutual:`, interfaces, implementations, structs, actors, or
local scopes, and they cannot be generic.

The initial ABI spelling is exactly `"C"`. The target chooses the platform C
calling convention. Aliases such as `"cdecl"`, target-specific conventions
such as `stdcall`, and an omitted ABI are rejected rather than normalized.
Additional ABI spellings require a later design record and dedicated fixtures.

## `Foreign` Capability and Effects

`Foreign` is a compiler-provided capability representing permission to cross an
unverified native boundary. It grants no specific filesystem, network, process,
or environment authority; it records that Jett cannot prove which effects the
callee performs.

Every foreign function declaration must have this exact first source parameter:

```text
view ffi: Foreign
```

The parameter participates in ordinary Jett capability propagation but is not
passed as a C argument. The checker rejects a missing, owned, renamed, or
non-leading `Foreign` parameter on a foreign declaration. Requiring one
canonical spelling keeps signatures and generated diffs predictable.

Consequences follow the existing capability rules:

- a program that uses a foreign binding declares an owned `Foreign` parameter
  on `main`, and the entry wrapper injects an unforgeable zero-sized token;
- ordinary functions borrow it and propagate it to callers;
- actors receive it through explicit `clone` handoff because they may outlive
  the caller; structured `run` tasks may borrow `view Foreign` like other
  capabilities, with the owner remaining borrowed until `join`; it is not
  serializable or constructible by user code, but may occupy capability
  parameters/fields under the existing rules;
- capability-free functions cannot call foreign declarations or wrappers;
- `verify`, property generation, comptime evaluation, and constant folding
  cannot execute a foreign call;
- foreign calls are never pure, memoized, reordered, or eliminated;
- interpreter execution reports that the native FFI backend is unavailable
  before attempting dispatch.

A policy file may document narrower real-world effects for human review, but it
does not remove the `Foreign` parameter in the initial contract. C headers do
not prove capability behavior, and silently treating a declaration as pure is
not allowed.

## Opaque Handles and Ownership

`opaque pointer Name to "CName"` introduces a nominal, non-constructible,
non-inspectable linear handle whose ABI carrier is exactly one target-width,
non-null C data pointer. `optional[Name]` uses the same pointer carrier with null
as `none`. The C pointee spelling is semantic metadata preserved through checked
AST, HIR, and MIR for diagnostics and ABI lowering; it is not a Jett namespace
lookup. Function pointers use a different ABI category and are unsupported.

The source parameter mode carries the ownership contract:

- `handle: Handle` transfers ownership to C and consumes the Jett value;
- `view handle: Handle` borrows the handle for the duration of the call;
- `optional[Handle]` represents a nullable returned owned pointer;
- `optional` is not permitted for a borrowed return because Jett cannot express
  a view whose lifetime escapes the call.

An owned handle returned from C must have an explicitly named release function.
That declaration registers the handle's compiler-managed cleanup operation; it
does not rely on Jett's current general linear checker diagnosing an unused
owner at scope exit. A consumed handle must be paired by policy with a C
parameter that takes ownership. Handles with no valid release operation are
unsupported in the initial subset. The generator never infers ownership from
`create`, `new`, `destroy`, or `free` name fragments.

MIR drop elaboration must release every live owned foreign handle exactly once
on fallthrough, explicit return, error propagation, and task cancellation.
Cleanup runs in reverse acquisition order. An explicit release or ownership
transfer marks the handle moved and suppresses the implicit cleanup; a borrowed
handle never owns cleanup. `optional[Handle]` cleanup calls release only for
`some`. This is a destructor rule specific to policy-backed foreign handles,
not a silent change to ordinary Jett linear values.

Compiler-inserted release is infallible, non-suspending, and non-cancellable.
When cancellation is observed at a capability checkpoint, cleanup for all live
foreign handles runs before `join` surfaces `CancelledError`. A foreign call
already dispatched is atomic from Jett's task-control perspective: its declared
ownership transfer either completed or did not begin, and cleanup follows that
result. Process aborts and host failures outside Jett's control cannot promise
cleanup. Native callable bindings remain unavailable until this drop
elaboration is implemented and verified.

A release function must be declared `infallible`, return C `void`, and consume
exactly one handle of the matching type. A release API that can report failure
is unsupported initially: consuming the owner before a failed release would
remove any safe retry path, while retaining it would not match the C call's
possible transfer. This restriction is validated before wrappers are emitted.

The first version does not expose arbitrary raw pointers, pointer arithmetic,
interior pointers, borrowed returns, aliasing owners, or user-visible handle
fields. Mutable access to hidden C storage is conservatively represented as a
foreign effect through a borrowed opaque handle; Jett code cannot inspect that
storage while the call is active. Thread-safety and callback lifetime promises
are out of scope and therefore callbacks are unsupported.

## Initial C Declaration and Type Matrix

The C frontend is configured for the binding file's exact target before it
resolves typedefs, enum widths, record completeness, and calling conventions.
The generated Jett spelling records the resolved representation, not a host
machine guess.

| C input after target layout | Initial Jett representation | Condition |
|---|---|---|
| `void` return | `nothing` | Return position only |
| `_Bool` | `bool` | Parameter or return |
| exact-width signed integers | `int8`, `int16`, `int32`, `int64` | Width proven by frontend |
| exact-width unsigned integers | `uint8`, `uint16`, `uint32`, `uint64` | Width proven by frontend |
| `short`, `int`, `long`, `long long` and unsigned forms | Matching fixed-width Jett integer | Width resolved for target |
| `float`, `double` | `float32`, `float64` | IEC 60559 layout reported by frontend |
| integer constant or enum constant | Typed Jett constant | Frontend fully evaluates value and fixed width |
| typedef | Resolved underlying supported type | Alias chain has no cycle or unsupported type |
| pointer to incomplete or policy-opaque record | `opaque pointer` handle | Ownership and release policy supplied |

For a scalar parameter or return, its fixed-width Jett type is also its complete
C ABI carrier. A typedef's original spelling is retained in source provenance
and diagnostics, while its target-resolved carrier is stored on the foreign
signature. C enum constants may become ordinary Jett constants, but enum-typed
parameters and returns are unsupported until foreign declarations can preserve
an explicit enum carrier independently from a semantic Jett enum.

Plain `char`, character pointers, and string/bytes marshalling are unsupported
in the initial subset. They require a later contract for pointer-plus-length
versus NUL termination, encoding, embedded NUL, allocation, temporary-buffer
lifetime, C mutation, and conversion failure.

The following are unsupported in the initial slice:

- variadic functions and `va_list`;
- non-`C` calling conventions;
- C++ declarations, overloads, methods, references, and exceptions;
- unions, bit-fields, flexible array members, vectors, complex numbers,
  `long double`, and by-value record or array parameters/returns;
- arbitrary pointers, multiple pointer indirection, pointer arithmetic, and
  borrowed pointer returns;
- all character pointers, strings, byte buffers, and enum-typed parameters or
  returns;
- function pointers, callbacks, and asynchronous retention of Jett values;
- writable output parameters and in/out buffers;
- global variables, thread-local variables, inline functions, and inline
  assembly;
- unevaluated or type-dependent macros;
- ownership or error behavior not fixed by policy.

Unsupported declarations outside the explicitly selected public symbol set do
not matter. An unsupported declaration reachable from a selected symbol is an
error; it is never emitted as a placeholder.

## Fallibility and Error Wrapping

The C declaration preserves the real ABI return and parameter types. A public
wrapper may expose `result[T, string]`, `optional[T]`, or an ordinary `T`, but
only when binding policy gives one of these explicit rules:

- `infallible`;
- `null_is_error` plus a stable public error message;
- `zero_is_success` or `nonzero_is_error` plus a stable public error message.

No rule is selected from symbol names, return types, `errno` conventions, or
comments in the header. If a selected function has no fallibility policy, the
generator fails even when its return type appears conventional. Raw C status
values may be exposed only when policy deliberately declares them part of the
public wrapper contract.

Generated error strings are deterministic and do not include pointer values,
absolute paths, or unchecked C memory. Backend link/load errors are compiler or
runtime diagnostics, not values invented for a wrapper's declared error type.

## Binding Policy Input

Facts absent from C must be supplied in a reviewed UTF-8 TOML policy file passed
explicitly to `jett bind`. Version 1 uses exact symbol keys and no wildcard or
name-pattern defaults. A minimal policy for the canonical example is:

```toml
version = 1
namespace = "counter"
target = "x86_64-unknown-linux-gnu"
abi = "C"
symbols = ["counter_create", "counter_read", "counter_destroy"]
constants = []

[handles.Counter]
c_type = "counter"
release = "counter_destroy"

[functions.counter_create]
public_name = "create"
return = "owned:Counter"
fallibility = "null_is_error"
error = "counter.create: C returned null"

[functions.counter_read]
public_name = "read"
fallibility = "infallible"

[functions.counter_read.parameters.counter]
mode = "borrow:Counter"

[functions.counter_destroy]
public_name = "destroy"
fallibility = "infallible"

[functions.counter_destroy.parameters.counter]
mode = "take:Counter"
```

The version-1 schema requires:

- output namespace and selected C symbols;
- target triple and `"C"` ABI, matching the command line;
- public Jett names (selected entries produce exported wrappers; raw declarations
  remain private);
- opaque handle ownership, release function, and transfer rules;
- fallibility and stable error mapping;
- a `constants` array of exact C constant spellings to expose (empty when none);
- expected header identity or digest where reproducible builds require it.

Each selected function has exactly one table. Pointer parameters and returns
have an explicit `borrow:Handle`, `take:Handle`, or `owned:Handle` mode;
non-pointer parameters need no policy because the frontend supplies their exact
carrier. Each function has exactly one fallibility rule. Specific function keys
override nothing because version 1 has no global defaults. The generator rejects
unknown keys or enum values, duplicate public names, policy for an unselected
symbol, missing selected symbols, unknown handles, release cycles, and incomplete
ownership or fallibility policy. It also rejects a release function unless it
returns C `void`, has `fallibility = "infallible"`, and takes the matching handle
exactly once. There is no heuristic mode. A future interactive tool may propose
policy, but proposed values must be materialized and reviewed before generation.

## Target, Linking, and Layout

`jett bind` requires an explicit canonical LLVM target triple. It never defaults
to the machine running the generator. The first implementation supports the
project's documented native targets when the configured C frontend and sysroot
are available:

- `x86_64-unknown-linux-gnu`;
- `x86_64-pc-windows-msvc`;
- `aarch64-apple-darwin`.

`wasm32-wasi` has no initial C FFI support. Other triples produce a deterministic
unsupported-target diagnostic rather than falling back to host layout.

Generation inputs include the target triple, sysroot, include roots, preprocessor
definitions, selected header, and policy. Include roots in checked-in provenance
are project-relative logical names, not absolute host paths. The generator asks
the C frontend for integer widths and enum layout. It does not reproduce C
layout with handwritten arithmetic.

The binding file records symbols, not library search paths. Project configuration
will eventually map a logical native dependency to target-specific static or
import libraries. Native lowering must verify that the binding target equals the
build target and report a mismatch before linking. Dynamic loading by path is a
separate future design.

## Namespaces, Order, and Reproducibility

A generated file owns exactly one ordinary project/dependency namespace. The
namespace must satisfy the existing uniqueness rule and cannot use compiler-only
stdlib fragments. Raw names are private and end in `_raw`; only intended wrapper
functions, handle types, enums, and constants are `export`ed.

Declaration order is deterministic and dependency-correct:

1. typed constants needed by later declarations;
2. opaque handle types;
3. private foreign declarations;
4. private error helpers;
5. public wrappers.

Within a dependency-independent group, the generator preserves selected header
source order, then uses C fully qualified spelling as a stable tie-breaker. It
does not alphabetize declarations across dependencies or rely on forward
references. Cycles that cannot be represented without an unsupported pointer or
layout are diagnosed rather than wrapped in `mutual:`.

For identical normalized inputs and generator version, output bytes are
identical. The generator version is folded into the semantic digest; logical
source and digest are foreign-block fields. Comments may repeat them for readers.
Output never contains timestamps, absolute paths, random identifiers,
locale-dependent text, or host-specific line endings. The canonical output is
UTF-8 with LF endings and one final newline. `--check` regenerates in memory,
verifies the digest, and fails when the checked-in file differs.

## Diagnostics and Failure Atomicity

Binding diagnostics use a dedicated stable code range and a common record:

```text
code, logical header path, source range when available, C spelling, reason
```

Examples include unsupported target, unsupported calling convention, missing
ownership policy, character pointer, variadic function,
unsupported reachable type, duplicate generated name, and unresolved symbol
policy. Diagnostics are sorted by normalized logical path, source offset, code,
and C spelling. Raw C frontend messages may be attached as notes after the
stable Jett message, but they do not determine ordering or the primary code.

By default any error prevents output. The command writes to a temporary sibling
and atomically replaces the requested file only after parsing, policy
validation, generation, formatting, and self-checking all succeed. A failed run
must not leave a partial or newly truncated binding file. There is no default
"generated with skipped declarations" mode.

## Dependency-Ordered Implementation

### Slice 1: parser, formatter, resolver, and checker

Tracked by [#173](https://github.com/vycdev/jett/issues/173).

- reserve and parse `foreign`, `source`, `digest`, `target`, `abi`, `opaque`,
  `pointer`, `to`, and `links`;
- add foreign block/type/function metadata to the AST and source ranges;
- format the canonical form and expose declarations to LSP/ASP queries;
- enforce namespace, declaration-order, visibility, non-generic, ABI, target,
  and exact `view ffi: Foreign` rules;
- reject foreign execution in the interpreter, comptime, and verify paths with
  a focused backend-unavailable diagnostic;
- add parser/run-pass/compile-fail/formatter fixtures without claiming linking.

This slice can be implemented before HIR or MIR. `jett build` may validate a
binding file but must state that it produced no native artifact, matching the
current validation-only build behavior.

### Slice 2: deterministic `jett_bind` generation and CLI

- add a `jett_bind` crate with an explicitly pinned C frontend integration;
- parse and validate policy without heuristic defaults;
- add `jett bind HEADER --policy POLICY --target TRIPLE --output FILE` and
  `--check`;
- resolve only selected symbols and the types they transitively require;
- emit canonical source, format it, parse/check it again, and replace atomically;
- add hermetic C-header fixtures and byte-for-byte snapshots for each supported
  target, including unsupported-case and stale-output tests.

This slice proves deterministic generation and checked source ownership. It does
not claim that `jett run` or `jett build` can call C.

### Slice 3: checked-program, HIR, and MIR preservation

This depends on the checked-program/HIR boundary in
[#20](https://github.com/vycdev/jett/issues/20) and the MIR/ownership boundary in
[#22](https://github.com/vycdev/jett/issues/22). Foreign identity, target, ABI,
symbol, ownership mode, and no-body status must survive lowering as typed data.
MIR verifies consumes/borrows and treats each call as an opaque side-effecting
operation. This slice also elaborates the registered release function on every
fallthrough, return, propagated-error, and cancellation edge, in reverse
acquisition order, and proves that explicit release or transfer suppresses the
cleanup exactly once. No optimizer may infer purity from the C signature or
remove/reorder foreign cleanup.

### Slice 4: native lowering, libraries, and linking

- lower supported scalar and opaque-handle signatures for the exact target;
- omit the source-only `Foreign` capability argument from the C call ABI;
- resolve project-configured native libraries and selected symbols;
- report target, missing library, and missing symbol errors deterministically;
- add tiny C fixture libraries that exercise success, explicit failure, handle
  create/borrow/destroy, ownership rejection, and target mismatch.

Interpreter emulation is not a substitute for this slice. Strings/bytes, dynamic
loading, callbacks, by-value records, writable buffers, and additional ABIs
require follow-up design and tests.

## Verification Requirements

The implementation must eventually cover:

- round-trip parsing and formatting of the canonical foreign block;
- rejection of semantic comments as foreign metadata;
- rejection of a missing or misplaced `Foreign` capability;
- private-by-default raw names and ordinary namespace collision behavior;
- strict top-to-bottom resolution around handles, raw calls, and wrappers;
- target-resolved scalar, constant, and pointer widths without host leakage;
- every unsupported matrix row with stable diagnostics;
- absent ownership/fallibility policy with no heuristic fallback;
- exactly-once owned-handle cleanup on fallthrough, early return, propagated
  error, and cancellation, including reverse acquisition order;
- no duplicate cleanup after explicit release or ownership transfer, no cleanup
  for borrowed handles or `none`, and cleanup for `some(handle)`;
- structured tasks borrowing `view Foreign` until `join`, versus actor clone
  handoff for potentially longer-lived authority;
- identical output from identical normalized inputs;
- `--check`, failure atomicity, and LF output;
- interpreter/comptime/verify rejection before dispatch;
- later native ABI and ownership fixtures on every supported target.

Documentation-only work verifies Markdown structure, links, formatting, and the
ordinary workspace checks. It must not claim parser, generator, or backend
support until those slices land.

## Deferred Questions

The following remain deliberate follow-up work:

- additional calling conventions and targets;
- by-value C record layout and unions;
- writable buffers, output parameters, and borrowed returns;
- callbacks, thread affinity, and asynchronous retention;
- shared/reference-counted foreign ownership;
- dynamic library discovery and loading;
- C++ interop;
- a narrower capability model layered over `Foreign`;
- whether a future policy format becomes stable public project syntax.

None of these should be inferred by the initial generator. A later extension
must add one canonical source spelling, checked metadata, deterministic
failures, and target fixtures before broadening the supported matrix.
