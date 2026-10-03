# Native descriptors with an uninhabited invocation input

Status: the repaired `90d82701` revision passes its complete clean workspace
and all four supported-host jobs. Later call-view staging at `55027914` passes
its complete clean workspace; its supported-host jobs remain separate and live. The
separate latent nested/secret-result follow-up passes the focused compiler and
linked gates below; complete 464-case/platform acceptance remains pending.
The native parity objective remains whole-language coverage.

## Existing source behavior

An unconstrained empty list supplies the internal uninhabited element type.
An enclosing generic can still create an inline callback whose parameter has
that type, return it, store it, explicitly clone it, mark it pending or drop it.
Creating a callback does not execute its body. Source `list.map` evaluates its
input and mapper normally and returns a real empty output list without ever
producing an element or invoking the callback. Source bodies receive normal
checking before empty-loop lowering.

The callback descriptor is an inhabited owned value. Its invocation input has
no value and no scalar carrier. An exact checked non-capture parameter equal to
the internal Never TypeId grants descriptor-only emission authority. Captures,
generic markers, source names, display labels and a return-only Never signature
do not grant it. The result may also be Never when an invocation is already
impossible, including one direct secret qualification; no result carrier or
default payload is created.

## Representation and validation contract

Preserve original FunctionIds, signatures, modes and capture facts. Original
typed metadata is validated before pruning; ordinary native control-flow and
ownership verification follows normal sequence/handler preparation. Valid
high-level loop bodies must not be rejected by running a prepared-native
ownership checker before that lowering. Malformed unused types, locals,
references, signatures and captures must not be laundered by pruning.

Original type shape and prepared native value eligibility are separate phases.
An exact Never function result remains valid original metadata even without a
Never input. Existing checked optional/result arms can contain such a closure,
including an impossible capture, whose creation and invocation disappear only
after arm preparation proves them absent. Recursive container and nominal
metadata keeps this distinction; malformed view schemas, foreign child TypeIds
and recovery results still fail before pruning. A surviving return-only Never
function still fails native classification, and an actual descriptor cannot
store a Never capture. Latent owned-capture metadata inside an already
uncallable body is checked separately and creates no environment. This does
not broaden descriptor-only authority.

Inline extraction shares a dense local table with its enclosing function. The
actual prepared factory retains the extracted child's unused Never formal in
that table, even though the parent has no statement or parameter using it.
Native preparation compacts only unused parent slots, across every original
block and all typed references, after original local and debug type validation.
It retains every real parameter, binding, capture, debug observation and loan
dependency; it does not remove parent blocks or grant a Never runtime slot.
Every surviving callable Never parameter/local/use still fails. The captured
factory MIR and failed first integration gates are retained under
`target/native-uninhabited-callback-local-diagnostic.log`,
`target/native-uninhabited-callback-codegen-rerun.log` and
`target/native-uninhabited-callback-linked-initial.log`.

A descriptor edge retains storage identity without making the original body or
its arbitrary callees callable. Retained direct or indirect invocations with a
bare Never input remain invalid. Capture evaluation, initialization, exact
types, copyability, lexical order, owned environments, pending depth, labels,
cloning and cleanup keep their existing contracts.

The existing descriptor layout requires a nonzero code address even for
observation. A descriptor-only identity uses an internal trap entry with only
runtime context and environment parameters and no return. It accepts no Never
scalar, emits no original body effect and returns no invented value. No valid
checked call reaches it. This adds neither a runtime ABI leaf nor a source
exception, implicit authority clone or interpreter fallback.

## Actual pre-repair characterization

Root characterized nine exact LF sources using the existing built CLI before
compiler integration. All nine current source-format checks and eighteen
frontend profile checks pass. Eight reference successes match exact expected
stdout and empty stderr; the independent terminal-cleanup source fails after
`before:0:1:5\n` with `runtime error: list.__remove_at: index -1 out of bounds`.
All agent captures have zero debug events. Sixteen native probes refuse bare
Never representation; the two integer-control probes reach object generation
and then the deliberately unavailable runtime bundle. All eighteen output
sentinels survive. These are characterization results, not linked acceptance.

| Case | Reference application stdout |
| --- | --- |
| Direct inline descriptor through an empty map | `direct:0:-1\n` |
| Returned captured descriptor through an empty map | `captured:0:-1\n` |
| Integer callback bodies actually invoked | `direct:2:7:captured:3:3\n` |
| Descriptor-list cloning and getter ownership | `owners:0:1:1\n` |
| Depth-two pending descriptor through an empty map, then two joins | `pending:0:0\n` |
| Live descriptor/list/bytes cleanup at an independent terminal error | `before:0:1:5\n` |
| Both callback input and result uninhabited | `identity:0\n` |
| A normal loop body with absent and really invoked callbacks | `loop:0:-1:1:3\n` |
| Equality inside absent and really invoked callback bodies | `equality:0:false:1:true\n` |

Captures and source hashes are under
`target/native-next-after-owner-views`,
`target/native-uninhabited-callback-controls-witness`,
`target/native-uninhabited-callback-never-result-witness` and
`target/native-uninhabited-callback-loop-body-witness` and
`target/native-uninhabited-callback-binary-witness`.
The loop source's initial wrong mutable-binding spelling failed with E1000;
that source and its captures are retained under `initial-invalid-mutable`.
It is not evidence of a native gap. The canonical corrected source supplies
the current accepted characterization above.

The equality witness passes ordinary source checking and preserves its Binary
operation in HIR. Descriptor metadata checking must accept exact Never operands
for equality and inequality with a bool result without requiring a scalar
carrier. This permission applies only to an already proven uncallable body;
phantom literals, mismatched operands/results and callable Never inputs retain
their rejection. It does not select arithmetic or ordering behavior for Never.

The initial linked baseline compiles the first eight driver cases and reports
one passing integer control and seven failures at the existing Never boundary
(3.24 seconds, 439 predecessor cases filtered out). The baseline is preserved
in `target/native-uninhabited-callback-linked-baseline.log`. It proves the
regressions reproduce the gap, not that the repair passes.

## Required acceptance

Meaningful compiler units must preserve typed descriptor storage, actual integer
callable behavior and the original-MIR negative contracts. Invalid IDs, wrong
signatures/view modes, Never captures, stale capture counts/types, false callable
edges and return-only shortcuts must still fail. Source body validity must
remain independent of empty-loop emission.

Linked cases require both matching runtime profiles, both builds before source
deletion, exact stdout/stderr/status, empty unintended observations and normal
owned-value cleanup. The independent failure must exit 71, never cleanup status
72. Existing full-stdlib, compiler/driver, frontend, workspace/object and
supported-host gates remain separate obligations for the actual repair head.

The accepted predecessor source `cb617ecf` passes a complete clean 439-test
workspace and all 182 object obligations. The documentation-only `194ce065`
revision passes all four supported-host build/package and relocated installed
jobs. Those results do not accept the current callback repair.
The fixed inventory remains 207 and the broad estimate remains about 85%.

## Focused acceptance

All 966 crossed compiler library tests pass in
`target/native-uninhabited-callback-phases.log`: 52 codegen, 416 comptime,
139 HIR, 87 MIR, 48 resolve and 224 typecheck. The new units check descriptor
entry/signature/storage and original loop/equality/capture metadata, enclosing
foreign local/debug types and surviving Never reads/initializers, plus named
control-flow, ABI order and disconnected-block dependencies during compaction.

All nine linked cases pass together (3.65 seconds, 439 filtered) in
`target/native-uninhabited-callback-linked.log`. Both matching runtime profiles
build before source deletion; exact application bytes, status, observations and
cleanup pass. The independent owned-list failure retains status 71 and its
original error; cleanup status 72 cannot pass. Nine source-format checks pass in
`target/native-uninhabited-callback-source-format.json`; Rust formatting passes.
The supplemental corpus now registers 448 cases. Registration is separate from
the accepted predecessor's complete 439-case execution.

All 83 driver library tests pass (69.44 seconds) in
`target/native-uninhabited-callback-driver.log`, followed by all 598 frontend
fixtures (24.60 seconds) in `target/native-uninhabited-callback-frontend.log`.

Initial isolated unit sources incorrectly called the unloaded `list.length`
facade. They were corrected to a declared pure consuming helper, while the
driver cases keep full-stdlib calls. The initial codegen and MIR logs are
preserved; they are not acceptance results. The subsequent actual factory-MIR
diagnostic exposed the transferred parent slots and selected the compaction
repair before the passing gates above. Ordinary callable body checks remain
independent of descriptor-only metadata; unrelated unsupported latent operations
are not claimed complete by these nine cases.

## Full-run regression and original function metadata repair

The exact clean `d14b830b` full native suite finishes with 446 passes and two
failures (941.02 seconds) in `target/native-workspace-d14b830b.log`. Both existing
sum-arm regressions fail with `type <never> is unsupported in function value
result (Never)`: original preflight demands a runtime representation for a
`function() returns Never` closure before sum preparation removes its impossible
arm. This is distinct from named local compaction. After the complete native
suite reported these failures, root stopped the remaining object/workspace
targets; the wrapper records terminal exit -1, unchanged clean head and the
reason in `target/native-workspace-d14b830b.json`. That run grants no complete
workspace or 182-object acceptance.

The repair adds a recursive typed OriginalMetadata/NativeValue distinction.
Only exact Never function results gain the original-metadata exception;
native scalar classification, descriptor proof and capture eligibility remain
strict. Three new codegen units cover absent optional success, result success
and result error closures, omitted closure symbols/environments, valid nested
metadata, malformed nested local/debug types and live return-only rejection.

All 969 compiler library tests pass in
`target/native-original-function-metadata-phases.log`: 55 codegen, 416 comptime,
139 HIR, 87 MIR, 48 resolve and 224 typecheck. Both previously failing linked
tests pass (48.51 seconds, 446 filtered) in
`target/native-original-function-metadata-linked-sum-arms.log`; all nine callback
tests pass again (3.82 seconds, 439 filtered) in
`target/native-original-function-metadata-linked-callbacks.log`. The existing
linked tests retain both runtime profiles, source deletion, pending-error order,
comptime observations and native verify/property execution. The corpus remains
448 cases. The exact clean `90d82701` complete workspace subsequently passes
all 448 native cases (944.48 seconds), all 182 object obligations (553.77 seconds),
remaining targets and doc-tests. The wrapper reports unchanged clean start/end,
exit zero and 92 successful groups at `2026-10-03T08:31:51.9056689Z` in
`target/native-workspace-90d82701.log` and `.json`. Workflow 37108099164 remains
the independent supported-host gate for that revision and passes all four build/
package and relocated installed jobs, last completing `2026-10-03T09:03:23Z`.
The about-85% estimate,
fixed 207 inventory and whole-language objective remain unchanged.

## Latent nested closures and direct secret results

The selected pre-Rust contract is
`target/native-uninhabited-latent-callback-draft/contract.md` (SHA `422abde8`).
The exact non-capture Never input remains the sole descriptor-only authority.
The retained outer descriptor may contain a nested zero-argument closure that
captures this absent input; creating or calling that inner closure is only
latent source behavior. A direct Secret[Never] result of the same uncallable
callback similarly needs no result carrier. Concrete instantiations retain
ordinary closure environments, invocation and explicit declassification.

The verifier follows validated FunctionRef, ClosureRef and function-adapter
edges from the already proven metadata root. Exact target identity, signature,
view/capture modes and capture-local types are checked before a visited
FunctionId set prevents cycles and repeated body validation. Every followed
original body is checked without granting native reachability or a frame.
Composite bodies use OriginalMetadata classification. Only exact Never and
one direct Secret[Never] gain the absent-value metadata treatment; no arbitrary
qualification/refinement is peeled. Actual created captures, live return-only
callbacks, callable Never edges, phantom literals, malformed nested metadata
and unsupported opaque Resource layouts retain their existing rejection.
The source policy, native trap ABI, runtime and hidden-secret rules are unchanged.

### Characterization and focused execution

Three exact LF sources have actual default-profile reference/agent status zero,
empty stderr/debug observations and literal stdout:

| Case | Application stdout |
| --- | --- |
| Nested closure in an absent callback plus invoked integer control | `nested:0:1:7\n` |
| Cloned/stored secret-result descriptor plus explicit integer declassification | `secret:1:7:1\n` |
| Concrete nested/secret-only control | `control:1:7:9\n` |

Before the repair, all three source-format and six frontend profile checks
passed. The first two sources refused Never function-result representation in
both native profiles; the concrete control reached the deliberately unavailable
runtime bundle. All six output sentinels survived. Fresh characterization
with the isolated repaired CLI repeats all three format/default reference/agent
and six frontend checks, preserves exact source hashes/output, and now reaches
object generation and the unavailable bundle for all six native probes. These
probes are object-admission evidence, not linked execution. Exact captures and
CLI provenance are in `target/native-uninhabited-nested-secret-callback-witness`
and `target/native-uninhabited-latent-callback-after`.

All **63 codegen library tests pass** (0.30 seconds) in
`target/native-latent-callback-codegen.log`. Six new source-derived units pin
latent symbol absence with retained outer/concrete symbols, direct secret result
descriptor classification, original-body phantom result rejection, exact owned
capture edges, malformed nested local/debug types and live return-only rejection.
The combined linked wrapper passes all **27 tests**: three new cases (6.95
seconds), nine existing descriptor cases (18.01 seconds), two sum-arm cases
(87.04 seconds) and thirteen scoped call-view cases (25.69 seconds). Both matching
runtime profiles build before source deletion, then compare exact status/output
and empty unintended observations. The wrapper exits zero with all seven formatted
compiler/test source hashes unchanged at `2026-10-03T09:53:15.5452645Z` in
`target/native-latent-callback-linked.json` and its four logs. The three new Jett
source-format checks pass. The isolated corpus registers 464 supplemental tests;
registration is not complete-workspace acceptance.

The broader compiler run passes **989 library units**: 63 codegen, 416 comptime,
139 HIR, 99 MIR, 48 resolve and 224 typecheck. It also passes 62 comptime and one
MIR integration test, for **1052 actual tests** with Cargo exit zero. Its wrapper
exits one solely because the expected count incorrectly excluded the 63
integration tests; all seven source hashes remain unchanged. Preserve that
wrapper/log as a count-guard failure, not a compiler failure or full-workspace
acceptance. Its actual passing tests do not require repetition.

The corrected remaining-gates wrapper passes all 83 driver tests (89.77
seconds), 598 frontend fixtures (24.61 seconds) and eight backend-lowering
tests (168.01 seconds). It exits zero with the same seven source hashes unchanged
at `2026-10-03T10:01:25.0163025Z` in
`target/native-latent-callback-remaining-phases.json` and its three logs. Rust
formatting checks pass. The earlier compiler count-guard record remains
`target/native-latent-callback-phases.json` with the actual passing Cargo log
`target/native-latent-callback-libraries.log`; it is not silently replaced.

**Pending:** complete 464-case workspace, independent 182-object and
supported-host acceptance for the actual follow-up revision remain required.
These focused gates do not accept that future revision's complete corpus.

### Accepted predecessor and separate follow-up

The accepted `90d82701` complete 448-case workspace, 182 object obligations,
92 result groups and all four supported-host jobs above remain historical
exact-head evidence. Main `55027914` now passes its complete clean workspace:
461 native cases (915.93 seconds), the four-test manifest covering all 182 object
obligations (534.62 seconds), 83 driver tests (78.52 seconds), 598 frontend
fixtures (22.20 seconds), remaining targets and doc-tests. The wrapper exits
zero with 92 successful groups, clean start/end and unchanged exact head at
`2026-10-03T09:56:47.1797084Z` in `target/native-workspace-55027914.log` and
`.json`. Supported-host workflow 37112967414 is still running. This accepts
that local staging revision, not its platform or the separate isolated
latent-callback follow-up. The whole-language objective stays 100%, broad tracking
stays about 85%, and the fixed 207 inventory/182 object obligations are unchanged.
