# Exact concrete arguments for erased interface dispatch

Erased interface calls validate additional concrete argument requirements against
the receiver-selected implementation before conversion or method body entry.
Different nominal types remain different even when their fields have identical
layouts. The selected runtime contract was recorded before implementation on
2026-10-08; this does not define equality operators on erased interfaces.

The original wrong-owner Source05 now reports
`interface dispatch concrete argument owner mismatch` before its Left body.
Original matching-owner Source12 retains `left:one:two`. The fifteen original
parity Sources remain unchanged; the two required fixtures restored with this
slice retain their exact recovered bytes.

## Implementation

Reference direct and named-callback calls preflight already evaluated arguments
before installing callee scopes, selecting body metadata or normalizing values.
The selected declaration namespace and type substitutions preserve nominal
refinements, full generic arguments and primitive widths.

Native dispatch borrows already evaluated formal locals and checks every required
finite conversion descriptor before conversion. The additive `InterfaceValidate`
leaf checks retained exact TypeIds and occupied container/sum arms without cloning,
changing native value storage or transferring custody. Descriptor parsing can
allocate Rust `Vec`/`Box` storage. Existing unboxing and malformed-handle checks
remain mandatory; validation failure follows the existing cleanup path.

Pending receivers preserve canonical undefined-dispatch failure. Direct pending
identity can be checked without joining or reading fields; unavailable containers
provide no child-owner facts. Earlier argument failures keep their message and
output order. Matching view and owning argument modes remain unchanged.

## Current integration verification

On 2026-10-12 the restored Actor/Resource/Owner integration passes all 2,331
workspace library tests, including eleven focused reference and six native runtime
preflight controls. Broader native interface regressions pass. Rust formatting and
whitespace checks pass.

Normal MSVC static-CRT Debug and Release launcher archives each pass ten native
Owner tests. The original nine tests execute thirteen Source-deleted cases per
runtime profile with unoptimized Jett objects. An additional production `Stdout`
test executes matching and mismatched owners with both Jett optimization settings:
four more executions per profile, seventeen per profile and thirty-four overall.
Its release-policy builds preserve successful production output or the exact
owner-mismatch error and exit without requiring a debug-output exception.

All original Source output, body-entry witness, evaluation-order, pending failure
precedence, expected statuses and cleanup assertions remain operation-specific.
The tests compile before removing the Source and launch fresh native processes.
Normal launcher builds use current runtime sources, including the additive leaf.

## Remaining scope

The checker substitutes the selected owner when an entire interface parameter is
the interface type. It does not recursively substitute Self inside
`list[Pairable]`; that declared erased-child contract remains unchanged. Nested
source coverage uses a concrete collection owner such as `list[optional[Left]]`.
Helper-level recursive checks do not establish unsupported Source signatures.

Refinement and pending-value preflight controls are reference/runtime tests;
the current native Source suite does not establish their full linked coverage.
The optimized production control covers the original matching/mismatched pair,
not every nested/generic witness. GNU, installed-distribution and complete
coherent-revision native parity remain open. This checkpoint does not establish
normal Main Resource cleanup counters, a public Resource provider or full-goal
completion. The rough 87% estimate and fixed inventory remain unchanged.

Historical isolated 3,640-test and twenty-six-execution records describe earlier
candidate work. Their target receipts were discarded during disk cleanup; those
records are not acceptance evidence for this integration.
