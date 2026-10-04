# Checked private opaque resource hook identities

Status: the identity prerequisite is implemented. Its original focused receipt
passes all 14 tests, and that earlier compiler/object/driver/frontend/lowering
checkpoint passes. The subsequent clean `616b51b9` workspace and all four
supported-host jobs also pass. These are historical identity/static-guard
receipts, not broader acceptance of the later reference lifecycle.
The private reference follow-up now passes 36 Resource-focused tests; its scope
is recorded below. The production catalog stays empty. Whole-language native
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
HIR hook identity/operation lowering, Resource-linear MIR transfer/drop proofs
and native carriers/providers are still missing. Nested ordinary printing,
hidden Secret, known absence and omitted rendering policy is unchanged.
