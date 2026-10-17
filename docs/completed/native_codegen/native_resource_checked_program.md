# Immutable checked resource program handoff

Status: implemented compiler handoff prerequisite. Its original isolated step
over `b7180bd9` passed 43 focused Resource groups and the resolver/typechecker/HIR
suites. The later private reference lifecycle now passes 36 focused tests, as
recorded below; broader acceptance of that Resource candidate remains pending.
The accepted authority is the opaque runtime resource contract in
`docs/completed/opaque_runtime_resource_contract.md`.

The handoff must own the parsed merged program and perform its own resolution
and checking. It must not accept an independently assembled Module,
ResolveResult and CheckResult as proof that those values describe one program.
The constructor consumes ParseResult, explicit loader SourceOrigins and the
existing private Rust resource catalog. Parser, resolver and checker Error
diagnostics each prevent publication of a successful snapshot. All hook
identities and complete signature modes are validated, including unused hooks.
Warnings remain observations and do not become errors.

The sealed snapshot keeps private Module, ResolveResult, CheckResult/interner
and SourceOrigins fields. Shared immutable access preserves generic and scoped
body facts, call permutations, binding modes, reflection and exact definitions
from that one check. There is no mutable or into-parts access and no constructor
from previously checked parts. Compiler input remains trusted Rust loader input;
this does not authenticate malicious Rust code or grant provider authority.

Ordinary checking must still return source diagnostics for invalid bodies.
Separate crate-private catalog identity/shape checking from the public phase
handoff validator. The latter additionally rejects checker Error diagnostics;
checking with a valid catalog can return an Error-bearing CheckResult for
repair, but that result cannot pass the successful-program handoff.

This step enables no live Resource, provider, cleanup ledger or native carrier.
The production catalog remains empty. The private reference driver now retains
the sealed checked program instead of reparsing after checking; its isolated
follow-up and commit-specific acceptance are recorded in
[the reference preparation note](native_prepared_reference_driver.md).
Raw DefId/TypeId integers are local to
the retained program, not proof that a value came from another session. Runtime
callable identity, ownership retention and execution effects must be tied to
that snapshot before dispatch. Resource-only finalizer eligibility and current
source-checker/lowering view-retention differences remain open prerequisites;
no capability signature, source purity rule or implicit borrowing rule is
changed by this handoff.

Source-driven gates must cover real accepted private calls, parser/resolver/
checker rejection, release policy, warnings and empty catalogs, unused hooks,
origin/catalog refusals, preservation of concrete generic and call-order facts,
and independent mutation of original input copies after preparation. These are
the selected pre-Rust test matrix; executed results are recorded below.
Whole-language native coverage remains
about 85%; the fixed inventory is 207, with 182 object obligations and 464
supplemental native conformance cases.


## Implemented boundary and executed evidence

`CheckedResourceProgram::prepare` stores one owned parse/resolve/check session,
with immutable accessors for the original module, full resolver/checker facts,
loader origins and parser diagnostics. The reference follow-up extends
`ResourceProgramError` to retain observations through the failed phase, including
preceding parse/resolve warnings. Structured metadata variants keep their typed
cause and observations; diagnostics() returns Some even for an empty prefix,
and Error::source preserves the cause. Warning-only parse
observations are retained; the warning test injects a compiler diagnostic into a
real ParseResult and does not claim that the parser emitted that warning.

The public hook validator refuses an Error-bearing CheckResult with
`SourceCheckingFailed`. The diagnostic checker uses a separate crate-private
identity validator, so normal E0311 source-body failures remain inspectable and
cannot be reclassified as valid execution inputs. Existing malformed/unused
hook identity, complete mode and foreign TypeId checks remain intact.

The frozen pre-format source draft is patch `1d247656` and manifest `b6306c34`
under `target/native-resource-checked-program-draft/`; root applies and formats
it in isolation. All **43 focused Resource groups**, including seven new
handoff groups and a strengthened existing invalid-body assertion, pass in
`target/native-resource-checked-program-focused.json`, ending
`2026-10-03T13:23:01.1489478Z`. The broader **454 library tests** (139 HIR, 54
resolver and 261 typechecker) pass in
`target/native-resource-checked-program-phases.json`, ending
`2026-10-03T13:24:25.9374022Z`. All five compiler source hashes remain unchanged
through both gates. These are compiler-source tests, not live Resource execution.

The accepted predecessor is exact clean `b7180bd9`. Its full workspace ends at
`2026-10-03T13:18:31.4769949Z` with all 92 result groups, 464 native conformance
cases (958.70 seconds) and all 182 object obligations (four-test manifest gate,
650.63 seconds). Its supported-host workflow
[37123835396](https://github.com/vycdev/jett/actions/runs/37123835396) now passes
all four Windows/Linux build and installed relocation jobs, finishing
`2026-10-03T14:14:19Z`. The earlier exact `616b51b9` workflow
[37119573097](https://github.com/vycdev/jett/actions/runs/37119573097) passes all
four Windows/Linux build and installed relocation jobs, finishing
`2026-10-03T13:14:20Z`. That result does not validate this new handoff.

The snapshot proves parsing, resolution, checking and hook shape only. It does
not bypass explicit comptime or verification failures, retain baked runtime
authority or authenticate a foreign raw DefId. The separate reference bridge
repairs post-preparation rereading without granting live Resource authority.
Typed caller ownership now carries the existing Rule24 effects separately from
callee access. The later private reference lifecycle adds program-bound hook
descriptors, reached-hook purpose checks and one-owner custody ledgers; its
36-test acceptance is recorded in
[the resource-hook note](native_resource_hook_identity.md#reference-lifecycle-execution).
The successful snapshot alone grants none of those execution rights. Broader
scoped/reflected/indirect/worker cases, production providers and native drop
elaboration remain required.


## Retained reference integration

The isolated follow-up over `94bb6b2e` keeps public BuildResult unchanged while
retaining the exact checked program and primary FileId0 entry privately. Its
focused gate passes all 46 Resource groups and four new driver groups with both
source hashes unchanged. The tests retain earlier warnings/metadata causes,
run the original primary/support program after source deletion or mutation,
keep comptime/verify/runtime observations once, and refuse failed or missing
handoffs before provider setup. All 544 driver/HIR/resolver/typechecker tests, 598 frontend
fixtures and eight backend-lowering regressions also pass with stable source
hashes. New full-workspace/platform acceptance for this follow-up remains
pending. The detailed receipt and predecessor failure
context are centralized in
[the reference note](native_prepared_reference_driver.md#verification-and-remaining-boundaries)
and [acceptance audit](native_acceptance_audit.md#retained-reference-preparation).
