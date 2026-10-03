# Native descriptors with an uninhabited invocation input

Status: implementation integrated; focused compiler, linked, driver and frontend
gates pass. Complete workspace and supported-host acceptance remains pending.
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
impossible; no result carrier or default payload is created.

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
store a Never capture. This does not broaden descriptor-only authority.

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
448 cases. Fresh complete-workspace/object and supported-host acceptance for
the repaired revision is still required; the about-85% estimate and fixed 207
inventory remain unchanged.
