# Opaque resource type-only reflection tags

Status: the affected compiler/frontend gates and subsequent clean `616b51b9`
full workspace pass. That head's supported-host run remains in progress; local
acceptance and predecessor platform evidence are centralized in the
[acceptance audit](native_acceptance_audit.md#resource-static-ownership-boundaries).
The broad coverage estimate remains about 85%.

## Selected contract and correction

The [opaque resource contract](../completed/opaque_runtime_resource_contract.md)
already selects a nominal type name plus `TypeKind.resource_type`, no primitive
tag and empty field, variant and machine-state metadata. Type aliases retain
their existing `alias_type` tag. The pre-Rust correction was captured in
`target/native-resource-kind-tag-contract.md`.

The checker already selected `resource_type` for a resource type, but the builtin
enum schema omitted that variant, so source comparisons failed with E0319.
The shared reflection mapper returned `unknown_type` for the same resource.
The correction appends the missing variant after all existing enum members,
preserving their indices, and maps the shared `resource` metadata kind to it.

These two table entries do not admit resource values, source constructors,
cloning, reflected construction, serialization, runtime providers or native
resource carriers. Live ownership and finalization remain separate work.

## Executed regressions

The initial focused gate reproduces all three failures in
`target/native-resource-kind-before.log`: source/native E0319 and the reference
`unknown_type` mismatch. The final source-derived regressions check:

- Stdlib resource comparisons through both `type.kind_tag` and `type.info`,
  with ordinary alias-tag behavior preserved.
- Checked reference metadata returning the resource tag, absent primitive tag
  and empty field, variant and machine-state lists.
- A real project caller reaching three synthetic stdlib functions through
  normal checking, HIR/MIR lowering and nonempty native object emission, with
  all three function symbols retained and no resource value created.

The native fixture's initial private-only form emitted no retained functions;
its attempted explicit-comptime form was also inappropriate for the raw-HIR
test helper. Those failures remain in `target/native-resource-kind-after.log`
and `target/native-resource-kind-object-after.log`. The final reachable
project/stdlib fixture passes in `target/native-resource-kind-object-final.log`.
No production reachability or comptime workaround was added.

The broader wrapper passes **800 compiler tests**: 63 codegen library units,
63 codegen object integration tests, 417 comptime units, 225 typecheck units
and 32 types units. All **598 frontend fixtures** also pass (22.31 seconds).
Rust formatting passes. All four tested Rust source hashes remain unchanged;
the wrapper exits zero at `2026-10-03T10:44:38.0454370Z` in
`target/native-resource-kind-phases.json`, with exact logs
`target/native-resource-kind-libraries.log` and
`target/native-resource-kind-frontend.log`.

## Remaining native resource work

Type-only reflection and [checked hook identities](native_resource_hook_identity.md)
are implemented prerequisites. The selected static copy/acquisition guards are
recorded in the [ownership note](native_resource_ownership_boundaries.md). Real
owned carriers, provider authority, other ownership paths, ordered cleanup,
explicit close, error propagation, actor transfer and task cancellation still
need checked implementations and reference/native execution evidence. This
correction changes neither the 207-fixture inventory, its 182 object obligations
nor the 464-case supplemental native corpus.
