# Checked private opaque resource hook identities

Status: the identity prerequisite is implemented. Its original focused receipt
passes all 14 tests, and that earlier compiler/object/driver/frontend/lowering
checkpoint passes. The subsequent clean `616b51b9` workspace and all four
supported-host jobs also pass. These are historical identity/static-guard
receipts, not broader acceptance of the later reference lifecycle.
The private reference follow-up now passes 83 Resource-focused tests; its latest
scope is recorded below. The production catalog stays empty. Whole-language native
coverage remains about 85%, with the fixed 207-fixture inventory and 100%
objective unchanged. The [acceptance audit](native_acceptance_audit.md)
keeps local, reference and platform evidence separate.

## Selected boundary before Rust

The [opaque resource contract](../completed/opaque_runtime_resource_contract.md)
requires an owned resource to come from an associated private trusted operation.
Authority follows loader origin, resolved declaration identity and complete
checked ownership modes. A callable name or matching runtime layout cannot
select that operation. The pre-Rust identity contract is frozen in
`target/native-opaque-resource-hook-draft/contract.md` (SHA `8dbd1458`); the
separate API selection is in `api-note.md` (SHA `f9daa540`). The earlier
[type-only metadata correction](native_resource_kind_tags.md) remains a
separate prerequisite.

This implementation establishes that identity/type boundary without choosing
production kernel names or public provider signatures. It uses compiler Rust
catalog input, not new Jett syntax, source configuration, environment variables
or command-line options. Ordinary production resolution supplies an empty
catalog. The specimen declarations used below belong only to the compiler test
harness and are not installed in the stdlib.

## Exact declaration and type facts

A catalog entry identifies the actual parsed resource name span and a closed
signature recipe. Resolution requires the existing successful Resource `DefId`,
its namespace, both a reserved stdlib FileId and independent loader
`SourceOrigin::Stdlib` evidence. It creates an associated private Function
`DefId` immediately after that resource through ordinary namespaced insertion.
Duplicate declarations, private access and top-to-bottom order still use normal
resolution rules. A private-looking name, a forged numeric FileId, an alias or
a same-named ordinary type cannot substitute for the resource declaration.

The compiler-created declaration is a real definition-table entry, not an AST
function with a fake body. Every actual source function body remains checked.
Synthetic diagnostic attribution may share the resource name span, but the
checker excludes the synthetic Function IDs from its source-span fallback map;
the resource's own declaration binding remains intact.

The checker derives each recipe inside its own type interner and binds the
resulting complete `Type::Function` to the exact Function `DefId`. Checked facts
retain that definition, the associated resource definition and type, the closed
hook kind, and the function type. Validation checks every parameter, complete
view/owned mode vector and result type, including unused entries. It does not
use the trusted function-signature tuple that omits view modes, a display name,
externally supplied TypeIds or the name-based intrinsic classifier.

The three closed recipes exercise Construct, BorrowOperation and Close facts:

| Test-only recipe | Parameters in order | Exact return |
| --- | --- | --- |
| NetworkFactory | `view Network`, owned `int64` script label | `result[R, string]` |
| NetworkBorrow | `view Network`, `view R` | `result[int64, string]` |
| Finalize | owned `R` | `nothing` |

Here `R` is the exact associated nominal Resource type. Transparent aliases
retain the ordinary canonical type identity. These are test signatures, not
new socket APIs or universal resource signatures. The logical script label is
not a resource key, provider descriptor or source-visible carrier.

## Explicit compiler entry and validation

`resolve_with_resource_kernels` and `check_with_resource_kernels` are fallible
compiler Rust entry points. Invalid catalog/proof metadata produces a typed
error without publishing partial checked authority. Independent
`validate_resource_kernels` and `validate_resource_hooks` entry points check all
joins, origin evidence and complete shapes before another phase uses the facts.
The checked validator also refuses a resolver result containing Error
diagnostics; a later phase cannot salvage an order/privacy/duplicate failure.

Legacy `check` and `check_with_options` retain default empty-catalog behavior.
They reject a nonempty privileged catalog with a bounded internal compiler
diagnostic, code 0, and no checked hook facts. Source argument, ownership,
visibility, order and body diagnostics retain their ordinary paths and codes.
No new source-policy diagnostic or body-validity exemption is introduced.

Resource remains distinct from Capability. Semantic purity and capability
eligibility do not change: the two capability-bearing specimen hooks use the
existing rules, while the resource-only finalizer is syntactically pure. A
future runtime provider installation must be explicit; purity, debug emission
and ambient stdlib caller flags cannot grant provider authority.

## Executed focused evidence

The candidate is applied on the clean `62de6505` metadata parent in an
isolated worktree. The frozen source candidate is `c38adcec`; it remains a
historical artifact. The first checker compile attempt exposes a missing
`mut` on the `finish_check` helper's checker parameter. After that repair, the
source-body test exposes an incorrect expected E0300: ordinary checking emits
E0311 for the invalid variable initializer. Only that assertion is corrected.
Both initial failures remain separate evidence, rather than successful gates.

The final focused run passes six resolver tests, seven checker tests and the
synthetic-span collision test. These parsed-source and malformed-metadata tests
cover exact IDs/modes, aliases, default absence of authority, stdlib origin
spoofing, private access, declaration order, source/catalog duplicates, ordinary
source body checking, corrupted unused joins and foreign TypeIds. All ten tested
Rust source hashes are stable across the successful run. Exact results and
per-gate logs are in:

- `target/native-resource-hook-focused-after-diagnostic.json`.
- `target/native-resource-hook-resolver-focused-after-diagnostic.log`.
- `target/native-resource-hook-checker-focused-after-diagnostic.log`.
- `target/native-resource-hook-span-focused-after-diagnostic.log`.

The broader wrapper passes all **1037 compiler library tests**: 63 codegen,
417 comptime, 139 HIR, 99 MIR, 54 resolver, 233 typechecker and 32 types units.
All **63 codegen object integration tests** pass (1.23 seconds), all **83 driver
tests** pass (77.95 seconds), all **598 frontend fixtures** pass (23.63 seconds),
and all **eight backend-lowering tests** pass (170.93 seconds), including the
182-file lowering inventory. The wrapper exits zero with all ten Rust source
hashes unchanged at `2026-10-03T11:18:33.3848153Z` in
`target/native-resource-hook-phases.json` and its five referenced logs. Rust
formatting and diff checks pass. Clean `616b51b9` subsequently passes its complete
workspace; supported-host acceptance for that head remains pending. The exact
predecessor record is in the acceptance audit above. This slice adds no
supplemental linked Resource case, object obligation or completed language row.

## Required lifecycle and backend followup

The [static ownership guards](native_resource_ownership_boundaries.md) now close
selected field/getter copies, known-view payload acquisition and root-value
observation gaps. They do not establish source execution or settle the remaining
nested printing and borrow provenance boundaries.

The checked map is not a provider installation. The private reference
interpreter now has the carrier, ordered custody ledger and internal scripted
provider described in [the reference record](#reference-lifecycle-execution).
Execution retains the same successful checked source/declaration snapshot;
re-reading a same-named function and attaching earlier DefIds by name/span
remains invalid. Production catalogs and provider installation stay disabled.

One owned value carries one cleanup obligation; a copied internal key or
ordinary Rust value clone cannot create another owner. The ordered acquisition
ledger must cover scopes, expression temporaries, partial arguments, owned
storage/return transfers and occupied aggregate arms. Cleanup must run in
reverse acquisition order on fallthrough, returns, handled/terminal failure and
loop exits, including fallible paths that bypass today's normal scope pop.
Explicit close uses the same finalizer and suppresses the later owner drop.
The existing runtime registry's context teardown is not proof of source-scope
cleanup.

HIR must retain the checked hook/resource identity. Typed MIR transfer/drop
elaboration must prove exactly-once cleanup and no borrowed drop before native
Resource carriers or provider operations become admissible. Native refusal of
unsupported Resource values stays conservative. Actor/message transfer,
pending work, cancellation, late completions, capability provenance and real
provider/socket adapters remain required parts of the full goal. Neither
implicit Resource cloning nor runtime pinning is selected here.


## Successful checked-program handoff

The later [immutable checked-program boundary](native_resource_checked_program.md)
owns its parse/resolve/check session and refuses parser, resolver and checker
errors before publication. Public hook validation now also refuses checker Error
diagnostics; diagnostic checking retains the separate identity-only helper so
ordinary source failures stay inspectable. This closes the successful-source
proof gap without installing a provider or changing the Close signature, source
purity rule or Resource value representation. Driver retention and reached-hook
runtime eligibility remain separate prerequisites.


## Reference lifecycle execution

The reference implementation over committed caller head `a63f8910` retains an
Arc of the successful CheckedResourceProgram. Physical Resource carriers and
hook descriptors keep that program identity and exact nominal type/hook facts.
The non-cloneable holder ticket is the owner; cloning an internal carrier or
ordinary Value cannot duplicate custody. Caller packets select source-order
argument effects before formal permutation, so View access alone cannot retain
an owner passed bare. Ordinary checked calls relay evaluated ownership envelopes
through entry and return. Generic body selection remains exact; broader scoped
and reflected cursor/capture cases are not claimed complete.

Executable registration rejoins the unique retained Item::Function and exact
DefInfo. Ordinary declarations have no name-span resolution entry; their actual
DefInfo span supplies the declaration join. Actual mutual bodies additionally
rejoin the authoritative resolution and the retained forward declaration.
Namespace/header checks stay exact, and the retained checked body executes even
if a host registration clone's body changes. A raw name or matching signature
cannot install a hook or replace executable Source authority.

The ledger tracks source scopes, operation temporaries, partial arguments,
owned returns/storage and occupied result payloads. Views carry checked live
backing without ownership. Explicit close retires the owner once. Reverse-order
cleanup on normal/early return, handled/terminal failure and failed later actuals
is observed before context teardown. The later-named-argument witness preserves
source order `[token, marker, net]` and formal permutation `[1, 0, 2]`: exactly
one Borrow501 proves the first actual ran, and a second would expose erroneous
callee entry after the marker failed.

Cleanup continues through older owners after a finalizer panic. Cleanup failure
overrides entry failure; successful cleanup preserves an ordinary entry error.
Ordinary source result.fail remains a protocol value. The terminal Source
fixture reaches the actual checked synthetic-Stdlib list kernel and pins
`list.__remove_at: index -1 out of bounds`. The ordinary closure fixture returns
Int64(7) to its Rust caller while the Resource/grant remain unreferenced by the
closure. Both controls assert cleanup/provider observations before teardown.

Descriptor creation does not execute a provider. Reached Construct, Borrow and
Close hooks require ReferenceRuntime purpose and exact checked target/signature,
caller facts, nominal type, context and authority provenance. Close remains
semantically pure at the source signature boundary; required evaluation cannot
invoke it. Closed descriptors and absent values remain usable without a provider
where already checked. Property evaluation restores its prior purpose on ordinary
success, failure/shrinking and unsupported sample-pool returns.

Actual root receipt `923450` passes all **36 Resource-focused reference tests**
(36 passed, zero failed, 418 filtered; 0.07 seconds) in
`target/native-resource-after-checked-call-relay-focused.log`. Seven declaration
identity/registration groups, real both-profile Source lifecycles, terminal and
mixed-failure cleanup, and required-worker/purpose controls are included.
Earlier API, parser and declaration-identity failures remain preserved in their
original logs; they are not passing receipts. Root receipt `14ee9c` also passes
all **454 comptime library tests and 87 driver library tests** (541 total) in
`target/native-resource-reference-libraries.log`. The complete frontend fixture
suite then passes **598/598** in
`target/native-resource-reference-fixture-suite.log`. These checks cover the same
unformatted Resource reference candidate, before later pipeline/assignment,
absence-shape and scoped-entry changes. Full-workspace and supported-host
acceptance of this Resource candidate remain pending.

The required reference work still includes exact scoped/reflected/captured and
worker body contexts, empty Resource-containing aggregate validation, pipeline
input and argument custody, mutable binding replacement, repeated checked-entry
restoration, and occupied aggregate transport/iteration. The existing 36-test
receipt does not cover these gaps. Native providers and MIR cleanup remain
required after reference lifecycle parity.

The typed absence follow-up repairs exact unoccupied list/map, nominal
struct/generic-struct, enum and machine layouts. It checks every occupied
ordinary field, element, key, variant and state against the checked schema;
absence never grants an owner or provider right. An enum variant constructor
is joined through its exact resolved owner, checked callable signature and
variant field tuple before any actual evaluation. Actuals are evaluated once
in lexical order and only then permuted into fields. Occupied Resource enum
and other aggregate custody transport remains required.

The expanded focused replay passes **43/43** (`e78a71`), including five new
aggregate/required-worker groups and two both-profile pipeline/assignment
frontend-prerequisite groups. The latter two are source-checking evidence,
not pipeline or assignment runtime acceptance. Current broader receipt
`fa9a70` passes **461 comptime library tests, 87 driver library tests and
598 frontend fixtures** (1146 total) in
`target/native-resource-after-absence-enum-broad.log`. Rust formatting and all
22 Jett fixture format checks pass. The original prerequisite E0378 failure
and enum dispatch failure are retained in their logs, superseded by these
actual passing results.

Ordinary refinement fields inside an otherwise Resource-containing absent
aggregate still need exact predicate validation; a nominal type tag alone is
not proof. Constrained aliases whose base contains Resource retain the existing
E0367 rejection. The stronger original-body/scoped/reusable-entry layer remains
an unaccepted intermediate until collected required-expression workers and
remaining contexts retain their positive controls. Pipeline/assignment source
validity does not implement custody transport, and production/native providers
and async/actor lifetimes remain required.

The separate committed caller suite passes **480/480 native tests** at exact
`a63f891016fac7dc50fffdde33be8b0ce45c655f`; see
`target/native480-after-owner-generation/receipt.json`, ending
`2026-10-04T14:20:12.987780+00:00` with stable HEAD/tracked sources. That corpus
contains no Resource candidate and grants no native Resource/provider acceptance.
Its exact supported-host workflow
[37206485724](https://github.com/vycdev/jett/actions/runs/37206485724) passes both
Windows/Linux locked workspace builds and both installed relocation jobs,
ending `2026-10-04T14:51:19Z`. The installed jobs have no source checkout;
`target/native-platform-a63f8910.json` retains the authoritative four-job receipt.
That result validates the caller commit and does not transfer to this Resource
reference candidate.
The 36 reference tests add no native inventory case. The fixed inventory stays
207 fixtures/182 object obligations and the broad feature estimate stays about
85%, with the whole-language 100% objective unchanged.

Production hook catalogs remain empty; no Resource declaration/provider is
installed in the stdlib. Scripted provider installation is compiler-test-only.
Whole scoped/reflected/indirect/worker breadth, other aggregate ownership paths,
real providers, task/actor cancellation and late completion remain required.
At that reference checkpoint, HIR hook identity/operation lowering, Resource-linear
MIR transfer/drop proofs and native carriers/providers were still missing. Nested ordinary printing,
hidden Secret, known absence and omitted rendering policy is unchanged.

## Native custody core protocol

The private native runtime core is implemented independently of reference AST
transport. Program/kind/owner-slot references keep one constructor-owned
registration identity. Non-cloneable owner and loan tokens rejoin the registry's
context, nominal kind, key generation and authority provenance. Transfers
advance holder generations without minting an owner from a copied carrier.
Chronological scope/operation/return frames retire loans before reverse owner
cleanup. A protected finalizer panic does not stop cleanup of later owners.

Resource completion retains cleanup-first precedence in a separate channel;
ordinary native first-error behavior is unchanged. The controls cover failed
publication cleanup, active-loan move refusal, stale/foreign/wrong-generation
tokens, repeated entries, counter exhaustion, cleanup continuation and every
entry/cleanup outcome pair. They assert owner, loan, frame and registry counts
before context teardown. Dropping a copied physical carrier or token is not
source-scope cleanup.

Root's actual focused receipt `7b0e8f` passes **13/13** protocol tests in
`target/native-resource-runtime-custody-core-first-focused.log`. After runtime
formatting, `9e9957` passes **157/157** complete runtime library tests in
`target/native-resource-runtime-custody-core-formatted-library.log`.
At the92421063 checkpoint, registration was test-only. Its later private
metadata implementation is recorded below. Native ABI, Resource CFG ownership/
drop plans, linked source-deleted lifecycle execution and real providers
remain required. This protocol core adds no native inventory
case; the broad feature estimate remains **85%** and the whole-language objective
remains 100%.

## Checked required, scoped, pipeline and assignment transport

The expanded reference transport preserves the immutable checked program and
original function, generic/scoped body, namespace initializer, explicit comptime,
verify or property region. Entry selection and metadata installation are
transactional; normal return, failure and re-entry restore the previous body
cursor and complete concrete type/reflection facts. A cloned AST or another
program/context cannot replace the original source occurrence.

Pipelines stage actuals in source order using original step identities before
formal permutation. Mutable assignment keeps its declaration slot after a move
or Close, evaluates RHS while the previous owner remains observable, then retires
and publishes owners exactly once. Reverse cleanup still completes after an
eligible finalizer panic. Generic intrinsic arguments use the exact selected
body's concrete type IDs and complete reflection records, preserving alias names;
no ambient metadata or unresolved source type name supplies that proof.

Root receipt `61a351` passes **83/83** focused tests in
`target/native-resource-concrete-intrinsic-pipeline-syntax-fixed-focused.log`.
This includes all 12 pipeline and seven assignment groups, the preserved generic
Source27 oracle and four new both-profile metadata/corruption groups. The two
test helper API corrections and canonical `into` fixture spelling preserve all
expected values and refusal oracles. The complete formatted reference/driver/frontend gate also passes all **1,186 tests** (501 Comptime, 87 driver, 598 frontend; `4ba85c`) in `target/native-resource-transport-formatted-property-fixed-broad.log`. Rust formatting and all eight new Jett
fixture format checks pass (`658c63`). Earlier failed gates remain in target logs.

The foundation `7157f564` supported-host run
[37217759157](https://github.com/vycdev/jett/actions/runs/37217759157) completed all
four Windows/Linux workspace, package and relocated installed-runtime jobs.
That receipt excludes these later transport changes and custody core `92421063`.
Ordinary refinement children, complete capture/reflection/worker contexts,
occupied aggregates and real providers remain required. This is reference
execution; it adds no native inventory case or native Resource admission.

## Checked HIR/MIR Resource manifest handoff

The checked lowering entry accepts the original immutable
`Arc<CheckedResourceProgram>`. Its private manifest retains every nominal
Resource declaration and trusted hook, including unused entries, and validates
exact declaration origin, type identity, closed recipe and complete signature.
Program-bound kind and hook references cannot be minted by raw lowering or a
matching name/signature. HIR descriptor and invocation nodes preserve the exact
original Source call facts, operands and lexical argument order; MIR retains the
same manifest through its visitors and remaps.

This is a metadata handoff. Resource actuals that require unsupported eager
staging fail transactionally. Ordinary acquisitions, copy/move/drop plans,
native constants and emission retain explicit pending Resource refusals.
Descriptor retention does not issue runtime storage, a provider or ownership.

The focused original Source gate passes all five groups (`852dfc`). After
formatting and the reviewed diagnostic successor, the complete phase gate passes
**522/522**: 191 HIR, 198 MIR, 70 codegen and 63 object-emission tests
(`961b04`, `target/native-resource-manifest-metadata-fixed-phase.log`). Foreign
nested type IDs are now rejected by recursive manifest validation before the
ordinary native type gate; the controls pin that exact diagnostic and separately
retain lower-level malformed-type refusal coverage. All other Source fixtures,
corruption inputs and expected values are unchanged.

Resource CFG ownership/drop plans, native registration/ABI, real providers and
linked source-deleted Resource execution remain required. These checks add no
native execution case and do not increase the approximate 85% coverage estimate.

## Bounded native registration and nominal issuance

The private runtime registration path now owns and validates the complete
`JTRSC001` layout blob. A 56-byte header, explicit little-endian tags, bounded
section sizes and dense ordinals reject malformed/truncated/trailing data.
Every unused hook, canonical signature/shape, occupied path, frame and operation
reference is checked before the sole production-capable issuer creates fresh
nonzero nominal kinds and the complete slot-kind projection. One context-bound
installation holder rejects repeated installation, foreign context, shutdown
and nonempty registry state. Metadata agreement cannot create Source authority,
a provider grant, a resident loan or ownership from carrier bits.

Scope, Operation and Return signatures all describe the containing Source
function. Signature agreement is checked across that function's frames even at
different instruction sites; hook and callee signatures remain separate. The
new refusal control also corrupts an unused Return frame. Physical acquisition
preflight checks slot bounds, generation/creation exhaustion and storage
capacity before a later provider runs. The core preparation and future opaque
token-table preflights remain separate mandatory checks.

The exact six-file/eight-hunk packet and eleven inputs passed Root guarded
application (`58a435`) after independent byte/API review. The initial nine
registration groups pass (`f753aa`). After the selected frame-signature
strengthening and formatting, the complete runtime passes **167/167**
(`56e7ac`, `target/native-resource-registration-formatted-library.log`), including
ten registration and the existing thirteen custody protocol controls.

The context/ABI adapter, typed grants, provider installation, compiler ownership
and cleanup CFG, occupied sums and linked source-deleted execution are still
required. These are runtime metadata/protocol tests; they add no native Source
execution case and keep the approximate coverage estimate at 85%. Full aggregate,
capture/reflection/refinement, real-provider and async/actor/cancellation breadth
remain part of the 100% objective.

## Source-authenticated ownership handoff

The original checked HIR/type archive and constructor-emitted MIR witness now
support a separate `ResourceOwnershipPlan`. It records exact scopes, operation
and provisional-return frames, owner holders, incoming views, bounded loans,
transfers, occupied sum extraction and cleanup obligations. Original/current
Source call associations preserve bare owned Resource-to-view custody without
creating ordinary owning operand claims. Copied queries, changed call sites,
operands, Source certificates, evaluation order and nominal type meanings are
refused. Unnamed Resource-bearing intermediate expressions retain their witness.
Canonical block/local compaction retains exact current associations.

All **12 focused ownership groups** pass (`155203`,
`target/native-resource-ownership-associated-focused.log`). The Source-derived
missing nominal-table regression passes (`fe7b0a`), returning the precise error
without a panic. The original nominal fixture retains both field-corruption
oracles after copying the complete original nominal tables and selecting its
actual Source Holder type. The malformed-type phase controls exposed a validation
ordering bug; valid type/invocation metadata is now required before custody
queries, preserving the exact manifest diagnostic and independent scalar refusals.

The complete formatted affected gate passes **535/535** (192 HIR, 210 MIR,
70 codegen, 63 object-emission), plus 32 type-library tests and one MIR doctest
(`3c6a8a`, `target/native-resource-ownership-valid-metadata-phase.log`). Earlier
failed runs remain diagnostic evidence. This is initial compiler ownership
analysis: connected native frames, provider/ABI operations, operational cleanup,
handled actual normalization, complete aggregates/reflection and linked binaries
executed after source deletion remain required. The separate 480-case caller
receipt, fixed 207/182 inventory and approximate 85% estimate are unchanged.

The broader driver/frontend gate also passes **693/693**: 87 driver library,
8 backend-lowering tests (including all 182 run-pass lowering obligations) and
598 frontend fixtures (`450ea4`,
`target/native-resource-ownership-driver-frontend.log`). The previous registration
commit's Windows CI job failed its CLI native-setup capture test with a main-thread
stack overflow; that separate platform regression is under investigation and is
not represented as current ownership or whole-workspace acceptance.

## Native context adapter and custody operations

The private runtime adapter connects the existing authenticated stationary
context lease, bounded layout issuer, registry and custody core. Exact current
attempt, frame, owner, loan, prepared hook and Resource sum records retain the
non-cloneable core tokens. Ordinary and Resource lookup IDs share one checked
monotonic allocator. Installation publishes a scripted provider and paired
ordinary Network grant transactionally after preflight; that provider is test
only. Production state starts absent with disabled providers and no new exported
Resource leaf.

Real core operations now cover root entry, Operation frames, direct/descriptor
hooks, acquisition, moves, bounded borrowing, close/drop, occupied sums, staged
replacement and protected reverse cleanup. Completion preserves the observed
body status verbatim and separately selects cleanup, host panic and current
ordinary/Resource body failure. Ordinary FailureTake is neither invoked nor
reset by the adapter. Refused transfers retain the old owner; a cleanup fault
cannot publish success. Borrow domain Fail stays ordinary Result data.

Initial compilation found three private API/field-name integration errors. The
first runnable focused check passed 13/14 and exposed string-only cleanup being
used for an ordinary borrow Result. The bridge now uses the existing recursive
native destructor, and an additional real-provider BorrowFail control checks
sum/string retirement, duplicate-drop refusal, clean completion and exact events.
All **15/15 focused groups** pass (`fd8577`,
`target/native-resource-state-adapter-companion-fixed-focused.log`). After
formatting, the full runtime package passes **182 unit + 13 integration tests**
(`96d995`, `target/native-resource-state-adapter-formatted-runtime.log`). The
earlier compile/failure logs remain diagnostic evidence.

These are runtime host-protocol controls, not linked Source execution. Exact
Source child Scope/Return activation, resident incoming views, connected C leaves
and emission, operational compiler cleanup, the matched archive and linked
source-deleted binaries in both profiles remain required. The adapter-head Windows
CLI setup-capture stack overflow is recorded separately; the later helper
repair below supersedes that pending local regression. Broader
aggregate/reflection/refinement, provider and concurrency coverage remains part
of the full goal. Native caller 480/480, the fixed 207/182 inventory and the
approximate **85%** feature estimate retain their separate scopes.

## Windows lowering stack repair

The unchanged CLI native-setup capture test reproduced a main-thread stack
overflow at Resource ownership and adapter heads. The adapter-head Windows CI
also completed the native conformance suite with **480/480 passing** before the
workflow failed its separate CLI capture target (`b32d80`, workflow
`37244085637`). This is current adapter-head supported native execution evidence;
it still includes no native Resource lifecycle case.

The repair extracts the existing heavy HIR statement/equality/inline-function
branches and MIR expression/per-actual branches into private helpers. Original
Source facts, Resource call capture, preflight, lexical order, shared call-view
scope, rollback, spans, diagnostics and context restoration remain at the same
operations. Source fixtures, assertions, snapshots, launcher order and stack
sizes are unchanged. Exact guarded application reconstructs both files and all
39 hunks (`3a8820`); the independent reconstruction record agrees.

The original regression now passes **1/1** (`285533`), and the formatted full
CLI capture target passes **9/9** (`508636`). The formatted phase gate passes
**535/535** HIR/MIR/codegen/object checks plus 32 type tests and the MIR doctest
(`f8fe67`). The broader gate passes **693/693**: 87 driver library, 8 lowering
checks including all 182 run-pass fixtures, and 598 frontend fixtures (`647ede`,
`target/native-windows-stack-decomposition-driver-fixtures.log`). An initial
command used the nonexistent test target frontend; no driver test ran in that
attempt. The corrected fixture_suite command supplies the actual receipt.

Actual Debug assembly measurement (`779571`,
`target/native-windows-stack-decomposition-frame-measurements.json`) records
MIR lower_value decreasing from 169,448 to 15,272 bytes and the ordered-call
loop from 73,736 to 4,504 bytes. Its separate per-actual helper uses 70,104 bytes.
HIR lower_expression decreases from 117,624 to 106,200 bytes and lower_statement
from 108,552 to 100,216 bytes. These are individual function-frame measurements,
not a whole-program stack limit or proof of every recursion shape.

The local platform regression is repaired. Whole-head supported-host CI and
connected Resource Source ABI/emission/cleanup and linked lifecycle execution
remain required. The fixed 207/182 inventory and approximate 85% coverage
estimate remain unchanged; runtime/metadata checks do not supply the missing
Resource native execution.

## Resource execution family and emitted layout

The compiler now captures the Resource execution family only after validating
the complete original checked HIR archive. Resource-free direct Source caller
predecessors receive sealed constructor witnesses and fresh Scope/invocation
plans. Edited current graphs cannot remove or recapture those witnesses.
Exact formal projections retain actual and parameter types, access, spelling,
effects and lexical evaluation order. Absent Optional and failed Result shells
receive exact holders; an ordinary failure companion is extracted once.

One fresh whole-program plan now projects deterministic bounded v2 layout bytes
and the exact driver-selected entry tuple. Every manifest hook remains checked,
including unused descriptors. The immutable object accessor emits metadata only.
Return transfer executes under the callee root Scope while retaining its private
Return destination; publication uses the same completed Scope and resolves the
active caller destination. These are distinct from the original recorded Return
storage and site facts.

The original producer packet reconstructs 11 paths and 33 hunks (`4319e3`).
Actual compilation exposed a missing FunctionId ordering implementation and a
private-module import; the fixes use stable dense IDs and supported readonly MIR
reexports. New fixtures were corrected to the existing owned-capability entry
rule after E0503. No language or checker rule was relaxed. Two narrow writer
successors preserve Return storage while projecting its active Scope (`25bf9c`,
`f36175`). Independent source/API review found no additional concrete blocker.

All eight focused groups pass in both compiler profiles (`1cba25`, `242535`).
The formatted compiler/object gate passes **543/543** plus the MIR doctest
(`fc3f02`). After the final Return projection, all **74 codegen and 63 object
checks** pass again (`45ddfe`). The driver gate passes **693/693**: 87 library,
8 lowering and 598 frontend fixture checks (`b9de51`). These are Source, compiler
and metadata checks. Dedicated body/ABI emission, ordinary companion cleanup,
matched runtime archive and Source-deleted linked lifecycle runs remain required.

## Native Source runtime and custody leaves

The runtime validates complete v2 Source invocation rows and implements the
32 exact Resource custody leaves. Formal permutation, lexical evaluation order,
owned parameter slots, resident parent loans, callee Scope/Return roles and the
original caller result destination are checked before execution. Version 1
remains historical metadata and cannot activate the new Source transitions.

Source entry preflights all owner generations, borrows, destination holders and
capacity before transferring any owner. Resident views keep one parent loan.
Return publication joins the completed callee Scope, current provisional Return
activation and exact caller destination. Scope/Return cleanup failure cancels
publication and outranks the separately retained observed body status. None and
Fail shells move whole; ordinary Fail companions use ordinary recursive drop.
C output alignment, overflow and known overlaps are refused before effects.
Production state and providers remain disabled.

The guarded runtime packet applied 16 paths and 61 hunks (`4788d3`). Its first
nine Source controls passed (`ea7d1f`). The formatted focused gate passed three
v2 registration, two batch-transfer and all fifteen previous adapter controls
(`29d4b6`). The first full suite exposed an obsolete test that expected version 2
to be an invalid header. The correction tests unsupported versions 0, the next
version and u32::MAX; no decoder refusal was weakened. The repaired formatted
runtime suite passes **196 unit and 13 integration tests** (`fd04d3`).

These checks execute real private runtime transitions and C-boundary controls,
not generated Jett Source bodies. Dedicated native body emission, ordinary
companion cleanup, a matched archive and Source-deleted linked lifecycle runs
remain required. Real providers, occupied aggregates, captured/reflected
contexts and concurrency retain their full-goal obligations. The approximate
85% estimate and fixed inventory are unchanged.

Independent byte review found eight unrelated Unicode test literals changed by
encoding conversion in the authored packet; Root restored the original HEAD
bytes exactly and reran the runtime suite (`e45fe3`). Independent semantic review
also found that ordinary nested sums could carry the wrong physical payload
ownership bit. Three causal controls failed before the repair (`bf61fa`), then
passed with exact immediate Optional/Result ownership validation (`5af1b8`),
preserving malformed carriers and unrelated live strings on refusal.

A selected pre-Rust root-outcome seam preserves the original body status after
exact root Scope retirement. Its readonly C lookup joins the same installed
entry tuple and current Runtime attempt, refusing active, stale, child, foreign
or worker-purpose roots without latching a body fault. Output refusal precedes
state effects. Body zero stays zero when cleanup panics; host completion retains
its independent cleanup-first channel. The successor applied five paths and
eleven hunks (`f54e7e`). One authored cleanup control incorrectly left its owner
in an Operation holder; the actual failure (`5763d2`) led to moving it through
real Source Return publication into the root holder. All five controls then
passed (`4f200a`). The final formatted runtime suite passes **204 unit and 13
integration tests** (`716d31`). Source-native execution remains pending.

The final reference Resource regression filter also passes **67/67** (`fd2bdb`).
Independent current source/API reviews confirm the malformed ordinary-sum fix
and exact readonly root-outcome boundary. Their finite scope does not establish
multi-owned Source actual or recursive activation coverage or native execution.

## Fresh-plan ordinary companion analysis

ResourceCompanionPlan borrows the fresh custody plan, exact immutable current
function and types. It retains ordinary storage/type/child, initialization,
liveness, alias, loan and move analysis and complete original Source acquisition
validation. Resource carriers do not enter ordinary owned storage. Readonly
ordinary storage and CallerAcquisitions retain the same current program lifetime;
public ordinary Copy/Move/Caller gates keep their Resource-pending refusals.
Exact expression occurrence roles are captured by the fresh CFG validator;
identical cloned expressions do not select operations.

The guarded prerequisite applied seven paths and 24 hunks (`7bca1d`). Actual
compilation exposed use of the public caller admission gate inside the private
companion path (`12afac`). The repair uses the same complete internal Source
validator only in the already authenticated companion context, retaining the
ordinary public gate. The constructor also stores its readonly Source acquisition
result for dedicated body emission. All three both-profile Source groups pass
(`bbbf1d`). The formatted gate passes **546 checks**: 74 codegen, 63 object,
192 HIR and 217 MIR tests, plus the MIR doctest (`f48b08`). Independent current
source/API review confirms the finite ordinary-analysis boundary. Dedicated
body emission and linked native Resource lifecycle execution remain pending;
whole-language coverage remains the about-85% estimate.

## Direct body and matched archive checkpoint

Direct original-Source body emission now compiles the selected Scope ABI,
lexical Source/Hook actual staging, resident-view transport, typed sums,
ordinary failure companions and provisional Return publication. Five focused
groups check both compiler profiles. The full formatted compiler gate passes
551 checks plus the MIR doctest after restricting Resource cleanup storage and
blocks to the selected family. Exact indirect descriptors, eagerly aborted
Resource acquisition CFGs and mutable Replace rows remain explicit successor
work; they are part of the full goal.

The actual double-cfg debug and release runtime archives compile on Windows
MSVC with coherent `+crt-static` dependency flags. Complete symbol inspection
finds exactly one main, eight private observation/host exports and all 32
production Resource leaves in each. A separate production archive has no
private main or test exports; the ordinary runtime suite passes 217 checks.
The measured vectors retain their original order and duplication, including
`/defaultlib:libcmt`. These are local Rust 1.93.1 checks; supported-host CI uses
its separately pinned toolchain and remains a distinct gate.

Four encoder controls run against exact current record declarations and the
real report module; six driver decoder controls pass. They preserve unknown
versus empty observations and original body/cleanup channels. Actual HOST C ABI
negative controls pass with both measured profiles; these handwritten controls
remain separate from Source conformance. The real reference and native gates
each pass five shared original Source scenarios under both profiles, covering
connected lifecycle, factory failure, partial actual failure, implicit drop and
finalizer panic. Native execution runs after deleting Project and synthetic
Stdlib Source directories and independently verifies outcomes, exact events,
script exhaustion and zero Resource/ordinary obligations before one destroy.
This accepts the first local compiler-test slice. It does not remove provider,
aggregate, reflection, refinement, task, actor, concurrency or supported-host
distribution obligations. The broader driver gate also passes 693 checks.

## Replacement refusal before finalization

The native plain-owner replacement protocol now validates both owners and reserves generation/acquisition capacity before old cleanup. New causal controls cover a loan on either owner, a consumed stale RHS handle and deterministic RHS generation exhaustion; each refusal preserves live owners and finalizer events, followed by a valid replacement or transfer. The unchanged success/finalizer-panic baseline also passes. The formatted focused gate passes four tests, and the full runtime gate passes 220 checks (207 unit and 13 integration). These are runtime/HOST controls. Source Replace producer/writer/emitter and aggregate replacement remain required; the 85% estimate is unchanged. Earlier matched archives and ten Source-native executions apply to cc06aa1f, before this runtime change. Fresh private archives and Source acceptance must be measured separately.

## Source mutable replacement checkpoint

The original Source replacement gate failed at the missing evaluated-RHS layout field before this compiler change. MIR now seals that field, the writer emits exact existing tag 12, and native assignment handles live replacement, exact self-reseat and vacant-after-close transfer separately. Seven new both-profile compiler groups cover endpoints, Source/header mutation refusals and object routes. Actual compilation required one missing test-only ExpressionKind import; canonical formatting changed no Source behavior. The final compiler/object/HIR/MIR gate passes 558 checks plus the MIR doctest, and 606 frontend/lowering fixture checks pass. All eleven shared original Source cases now pass both real reference and Source-deleted native execution under debug and release (22 executions each). Six added cases cover live replacement/use, self-reseat, close/rebind, handled RHS failure borrowing old, ordinary RHS failure and old-finalizer panic cleaning both owners.

The fresh runtime archives have measured receipt `54a38248`; both HOST C ABI profiles pass. The complete driver-library run passes 86/87, with one pre-spawn temporary capture PermissionDenied; that exact property replay passes in isolation. This is recorded separately from Source acceptance, and no clean complete driver-library or current full-native regression pass is inferred. Whole-language coverage remains about 85%; indirect/handled/aggregate/worker/provider/concurrency and supported-host obligations remain required.
