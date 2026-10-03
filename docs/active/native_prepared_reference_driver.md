# Retained checked-program reference execution

Status: implemented isolated follow-up over
`94bb6b2e021181ec283489c9a02f29a09497fbc7`. The contract was selected before
tracked Rust changes. All 46 focused Resource groups and four new driver groups
pass with unchanged source hashes. The broader 544 driver/HIR/resolver/typechecker
tests, 598 frontend fixtures and eight backend-lowering regressions also pass.
New full-workspace and supported-host acceptance for this follow-up remain
pending; exact94's failed full run is not
accepted by a later single-case rerun.

The public BuildResult/build_file API remains unchanged. A private
PreparedReference retains its ordinary BuildResult observations plus an
`Option<Arc<CheckedResourceProgram>>` and the primary FileId0 main's exact saved
span/namespace. Build-file preparation parses/discovers once, preserves actual
loader origins, and uses CheckedResourceProgram::prepare internally with an
empty production catalog. The full checked session remains sealed. Comptime and
verify run once from that saved merged Module and original scoped/generic facts.

run_file_inner delegates to private preparation then run_prepared_reference.
Runtime first emits retained frontend observations and refuses build.has_errors.
A successful build lacking a retained snapshot is an internal handoff failure,
never permission to reparse or initialize a provider. The runtime entry is the
saved FileId0 main span in the exact checked Module, not the first support-file
main. Interpreter.register_module resets namespace when item FileId changes
(interpreter.rs1330-1339); registering the merged module once preserves support,
project and entry order. There is no second filesystem read, parse or module
rediscovery after preparation. The unchanged macOS graphics-thread selection
still reads/parses before preparation; the entire public run path is not claimed
to parse once. Existing default argument, explicit environment,
Clock/Random/Graphics installation, output/debug capture and terminal sample
checks retain their order and behavior. No Resource provider, descriptor,
ResourceValue, ownership ledger, native ABI or execution effect is introduced.

Selected through-failure diagnostics adjustment:

- Parse/Resolve/CheckDiagnostics retain observations through the failed phase in
  original parse -> resolve -> check order, without duplicate phase vectors.
- ResolveMetadata and CheckMetadata become { source, diagnostics } variants.
  Their typed error is preserved by Display and std::error::Error::source.
- `diagnostics()` retains `Option<&[Diagnostic]>` compatibility and returns Some
  even when a metadata failure observed zero diagnostics.
- Clone an earlier diagnostic prefix only on failure where needed. Successful
  snapshot records remain owned in their original phase structures; do not run
  resolution/checking again to recover warnings.
- Driver keeps discovery/parse observations in the exact prepared ParseResult.
  Metadata rejection appends one operation-appropriate internal error while
  retaining prior warnings. Successful preparation concatenates each retained
  phase once before required evaluation. Error-bearing builds may retain the
  checked snapshot internally for attribution, but runtime must gate the build
  error before execution or provider initialization.

Tests must use a real source unused-binding warning before checker E0311,
parser-warning injection into a real ParseResult with attribution stated,
warning retention before typed catalog errors and source-derived resolver
failures. Pin order/span/code/message and warning-only success. Driver controls
prepare source once then mutate/delete entry/support files, preserve exact
primary entry and per-file namespace reset with a support main, preserve actual
comptime/verify/runtime observations once, refuse a failed frontend before any
provider setup, and reject a corrupted successful handoff missing its snapshot.
No test needs a live Resource or native archive. The focused gates below are
executed; new full-workspace/platform acceptance remains pending.

Driver build_source and backend lowering are outside this bounded preparation
repair. Their existing paths are not claimed unified. Required-evaluation
runtime-only Resource invocation eligibility remains a separate prerequisite.
The whole-language 100% objective, about-85% estimate, fixed 207 inventory/182
object obligations and 464 supplemental cases remain unchanged.


## Verification and remaining boundaries

The original target source draft is `e304c625` with manifest `51e6aaaf`; its
contract is `8d0e6fc5`. Root applied and formatted the two files in isolation.
`target/native-prepared-reference-driver-focused.json` passes 46 Resource and
four driver groups, with exit zero and both formatted source hashes unchanged,
ending `2026-10-03T14:12:41.4871736Z`. Three new Resource groups pin ordered
warning prefixes and typed metadata causes. Four driver groups preserve the
saved source/primary entry, ordered observations and pre-provider failure gates.
The parser-warning control injects a Diagnostic into a real ParseResult and
does not claim that the parser currently emits warnings.

The broader driver/HIR/resolver/typechecker wrapper passes all 544 tests
(87/139/54/264) with both source hashes unchanged in
`target/native-prepared-reference-driver-phases.json`, ending
`2026-10-03T14:15:36.7702211Z` with exit zero. All eight backend-lowering regressions
(198.73 seconds), including the fixed inventory lowering check, and 598 frontend
fixtures (24.41 seconds) pass with those hashes unchanged in
`target/native-prepared-reference-driver-frontend.json`, ending
`2026-10-03T14:20:34.8384468Z` with exit zero. New full-workspace/platform gates
remain pending. The exact94 full workspace failed one
native NamedTempFile.reopen operation with PermissionDenied; all 182 object obligations and
463 other native cases pass. The affected case's elevated single-case rerun
passes, but neither run is full94 acceptance. The accepted b718 local and all-four supported-host receipts, plus the
historical `616b51b9` platform result, are centralized in
[the acceptance audit](native_acceptance_audit.md#retained-reference-preparation).
This follow-up adds no native inventory case and changes no coverage metric.

Runtime-only required-evaluation effects, program-bound Resource callable
identity, caller ownership retention, Resource carriers/providers, ordered
cleanup and native lifecycle remain prerequisites. The sealed snapshot and
private reference bridge alone grant none of that authority. build_source and
backend lowering keep their existing independent paths. A formal View mode is
not proof of the caller's retained owner; no new generic borrowing policy is
selected here.
