# Native internal call-view stages

This is a bounded native implementation of Rule 24's existing explicit,
read-only call borrow. It does not change persistent source local-alias lifetimes.
The selected contract was captured before Rust in the internal stage contract
and the field/direct-origin amendments:

- `target/native-projected-interface-view-staging-owner-change-witness/scoped_stage_contract.md`
- `target/native-scoped-call-view-origin-witness/contract.md`
- `target/native-scoped-call-view-value-origin-witness/contract.md`

The source authority remains `docs/design.md` Rule 24, particularly the explicit
call-site `view`, call-bounded lifetime and subsequent owner-rebinding examples
(lines 5755–5775, 5893–5915 and 5939–5942). The zero-copy contract protects
backing storage while a view exists (5990–5995). Architecture's existing handler
snapshots remain bounded by recursive cloneability (1199–1202); explicit interface
cloning does not authorize a new implicit erased-authority snapshot.

The implementation stages the original checked view eagerly before a later
handled operand. Typed MIR `BeginCallView` and `EndCallView` identify the internal
loan; synthesized local names grant no authority. Exact local identity and
endpoint types, checked field owners/indices and transparent qualifiers remain
validated before pruning, including unused and unreachable metadata. Direct
views keep their pending metadata and add no readiness guard. Projected fields
retain every eager receiver guard, so a pending owner fails before a later
handler can emit its marker.

Safe terminal mutable owners remain protected through the consuming operation
and may be rebound afterward. Original owned temporary producers evaluate once
into normal owning storage; the internal alias owns no destructor slot and
does not clone or rebox an interface. Ordinary owner cleanup remains separate
from End. Normal completion materializes the operation's owned/copy result
before End. Aborted handler returns end abandoned stages before evaluating an
owned return operand. Break/continue end only stages belonging to operations
abandoned by the targeted loop exit. Source-declared aliases retain their
existing persistent origin and owner-loan checks.

The source argument's original explicit view and checked borrowed position are
both required. Formal-mode wrappers cannot hide a bare argument's ownership
transfer. Direct/indirect calls, existing borrowed intrinsic positions and
complete reflected read/validation operations use consuming-operation bounds.
Owning aggregate/collection/actor/transition paths retain their existing staging
eligibility; this change does not store borrowed views in owned data. No runtime
ABI, reference count, owner pin, resource clone or escaping-view permission is
introduced.

An erased owner consumed or rebound by a later operand while the first view is
still needed remains a separate source/checker reconciliation. Actual frontend
and interpreter admission of those overlap sources is recorded in
`target/native-call-view-owner-overlap-audit/contract.md` and the owner-change
witness package. Ending a loan cannot retain backing storage destroyed during
the operation. Existing borrowed-place safety is preserved; this implementation
selects neither a new source rejection rule nor arbitrary implicit authority
snapshots. Whole-language parity remains unfinished at that boundary.

Root applied the guarded 90d82701 candidate only in the isolated managed staging
worktree. Root repaired one Rust mutable-walk diagnostic with a typed recursive
root replacement, repaired the loop fixture's required default path, and aligned
the persistent-alias test with direct internal staging while retaining its
owner-loan rejection. The resulting focused gates actually passed **99 MIR
library tests and 57 codegen tests**, including the object-emission units. This
is not the independent full object inventory or a full workspace acceptance.
The primary 90d82701 full-workspace audit remains a separate checkpoint.

All nine linked regressions pass from unchanged, root-characterized LF
sources. They pin eager field ordering, pending first-error, the declared Named
alias control, post-call owner consumption, early-return ownership, mutable field
and direct owners, and owned field/direct temporary evaluation once. Each test
checks the literal reference outcome and empty frontend/runtime events, builds
both profiles with matching runtime archives, deletes source before executing
bounded native binaries, and compares exact stdout, stderr and terminal status.
The pending failure expects native status 71 and the canonical runtime message;
normal CLI's reporting prefix is not part of that runtime message. The executed
gate passes nine tests in 44.95 seconds (448 filtered) with all 21 compiler/test
source hashes unchanged, recorded in `target/native-scoped-call-view-linked.log`
and `.json`.

All 983 crossed compiler library tests pass (57 codegen, 416 comptime, 139 HIR,
99 MIR, 48 resolve, 224 typecheck) in `target/native-scoped-call-view-phases.log`.
All 83 driver library tests pass in 70.22 seconds in
`target/native-scoped-call-view-driver.log`. The actual frontend/lowering gate
passes eight backend tests (154.20 seconds), 598 frontend fixtures (23.94 seconds)
and two view-escape tests (8.73 seconds) in
`target/native-scoped-call-view-frontend-and-lowering.log`. The separate
`native-scoped-call-view-frontend.log` records an initial wrong test-target name;
that invocation is not a fixture acceptance result. Rust formatting, nine source
format checks and diff checks pass.

The preceding exact clean `90d82701` revision passes its complete 448-test
workspace, all 182 object obligations and 92 result groups. Its supported-host
[workflow 37108099164](https://github.com/vycdev/jett/actions/runs/37108099164)
passes all four Windows/Linux build/package and relocated installed jobs, last
completing `2026-10-03T09:03:23Z`. Those results accept the preceding callback
repair, not this later staging implementation. The exact clean `55027914`
staging commit now passes its complete 461-case workspace, all 182 object
obligations, remaining targets and doc-tests with 92 successful groups and an
unchanged head. Its terminal record is centralized in
[the descriptor note](native_uninhabited_callback_descriptors.md#accepted-predecessor-and-separate-follow-up).
Supported-host workflow 37112967414 also passes all four Windows/Linux build/
package and relocated installed jobs, last completing `2026-10-03T10:23:08Z`.
No full surface completion, denominator change, or new coarse coverage percentage
is claimed by this note.

## Expanded consuming-operation acceptance

Four additional root-characterized sources extend the same internal loan to an
indirect interface-field call, a source call borrowing Stdout, a complete
reflected field getter plus Positive producer validation, and an owned temporary
whose later handler fails independently in list.remove_at. The last case keeps
ordinary bytes/list slots live, evaluates its original producer once, suppresses
callee/after markers, and retains the exact list.__remove_at diagnostic and
native status 71 instead of cleanup status 72. The capability case uses the
source formal's checked View mode; StdoutWrite's intrinsic mask is unchanged.

All four selected LF sources pass formatting and frontend checking in both
profiles. Actual default-profile reference/agent captures have empty debug
events; the success outputs and independent failure are literal test oracles.
The original reflected-source formatting failure and exact original capture
remain in `target/native-scoped-call-view-consumer-witness`; root formatted a
separate selected copy and recaptured all four sources in
`target/native-scoped-call-view-consumer-formatted-witness`. No execution oracle
was inferred from a formatting change.

The final combined gate passes **all 13 linked tests in 25.95 seconds** (448
filtered), with all 25 compiler/test source hashes unchanged, in
`target/native-scoped-call-view-linked13.log` and `.json`. Both matching runtime
profiles build before source deletion, then bounded launches compare exact
status/stdout/stderr and empty unintended observations. All 13 tracked source
format checks pass in `target/native-scoped-call-view-source-format13.json`.
Rust formatting and diff checks pass. The supplemental corpus now registers
461 cases. Complete local workspace and 182-object acceptance at `55027914`
and all four supported-host jobs are recorded above. The fixed inventory is
still 207 and the broad planning estimate remains about 85%.
