# Checked private opaque resource hook identities

Status: the isolated identity implementation passes all 14 focused tests.
The broader compiler, object, driver, frontend and lowering gates also pass.
The subsequent clean `616b51b9` full workspace and all four supported-host
build/installed jobs also pass. The [acceptance audit](native_acceptance_audit.md#resource-static-ownership-boundaries)
keeps local and platform evidence separate. The production catalog is empty;
no Resource value or provider operation executes in this change. Whole-language native coverage remains about 85%, with the fixed
207-fixture inventory and 100% objective unchanged.

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

The checked map is not a provider installation. The reference interpreter still
needs a real owned Resource carrier, a deterministic fake provider and an
explicit runtime installation retaining the exact checked source/declaration
snapshot. Re-reading a same-named function and attaching earlier DefIds by
name/span is not a valid bridge.

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
