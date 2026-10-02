# Native directly secret-qualified aggregate local views

## Selected typed proof

Design's secret field rule and Rule Set 24 already admit readonly projected
aliases from directly secret-qualified aggregates. The bounded HIR repair
preserves the actual checked receiver and accepts precisely an exact declared
Struct, Bitfield or MachineState owner, optionally carrying one direct
`Secret(owner_type)` layer. State and canonical FieldId lookup remain exact.

For a secret receiver, a declared `nothing` or already-secret field retains
its exact TypeId, including nominal refinements whose declared base chain reaches
Secret. All other field expressions must be exactly one
`Secret(declared_field_type)`. Finite checked metadata traversal detects the
exception without substituting the field's nominal identity. Final local secret
promotion is an independent existing annotation rule. Multiple owner layers,
implicit declassification, sibling/base refinement substitution and arbitrary
carrier equality do not supply the Field proof.

Keep exact source LocalId/type, base owner, intermediate/endpoint types,
immutable preceding origin chain, cycle checks and validated-initializer ledger.
Unused or orphan aliases cannot bypass proof before compaction. Explicit
borrowed Coarsen/Declassify preserve their exact selected conversion and backing
owner; explicit clones and ordinary owned fields remain independent owners.
The same generic field can require a secret borrowed binding but produce an
implicitly copyable scalar after explicit declassification.

MIR persistent loans, whole-owner liveness, non-owning slots and cleanup are
unchanged. The native field access retains the secret base and existing pending
checks. No allocation, hidden owner, implicit clone/join, ABI change, mutable or
temporary view, capture escape, projected mutation, owner-change permission or
loan-expiry policy is added. The independent interface handler staging draft
is unapplied; this proof does not resolve its scoped internal loan problem.
No hidden-secret debugger or terminal observation policy is selected here.

## Actual pre-change source evidence

`target/native-secret-owner-projected-view-witness/results.json` (SHA256
`904a9430ebbd75af1a5d52eaa478c194a12556c49535f9ece88cf3cc26f27aeb`)
records all seven sources. Seven formatter checks and 14 frontend profile
checks pass. Ordinary default-profile reference/agent captures record six
successful sources and one pending-receiver failure, all with no debug events.

Direct and nested/forwarded struct aliases, the declassified owned control,
secret bitfield payload, owned exact-state machine and machine view parameter
preserve literal application lines and original owner rereads. Every visible
data observation uses existing explicit declassification. Their prior AOT
attempts preserve all 14 output sentinels: 12 alias attempts fail first at HIR
owner validation, while the two owning-copy controls reach unavailable archive
lookup. These are predecessor boundaries, not repaired native execution.

The pending nested struct prints only `before:secret\n`, then the reference
reports its existing pending Packet field-access error. It reaches neither the
after-binding marker nor a later observation. The original reference error
contains the underlying aggregate. Its normal CLI `error: ` reporting prefix
is separate from `RunFailure.message`. Native retains its existing checked
`[redacted]` receiver rendering. This exact difference is already recorded in
[the open observation note](../open_design/debug_print_hidden_secrets.md);
qualified-field admission preserves native redaction and does not choose
the reference observation policy.

## Current acceptance and remaining gates

The formatted/rebased HIR proof adds three tests: nested/forwarded exact owner
and declared qualification exceptions; concrete generic scalar/linear binding
facts and explicit clones; fifteen malformed qualification/owner/index/root
and unused/orphan proof controls. All three pass (136 filtered, 0.00 seconds)
in `target/native-secret-owner-local-view-hir.log`. The first warning-interrupted
invocation produced no test result and is not an executed gate.

All seven linked tests pass together (2.82 seconds, 432 filtered) in
`target/native-secret-owner-local-view-linked.log`. Both matching-profile
executables build before source removal and require exact stdout, status,
stderr and empty runtime/frontend/artifact observations. Six successful cases
pin full outcome parity. The pending case requires the existing raw reference
message separately from native `runtime error: field access is not supported on [redacted]`,
with matching partial stdout and native status 71. Cleanup status 72 cannot
substitute for that status. This records the open diagnostic boundary, not
seven diagnostic-parity cases or a new secrecy rule.

The initial attempt's six passes and one failed shared-error comparison
(4.56 seconds) are retained in
`target/native-secret-owner-local-view-linked-initial-mismatch.log`. Correcting
the test to the already documented backend-specific error contract leaves
source policy, native redaction and interpreter behavior unchanged.
All 954 crossed library tests pass (41 codegen, 416 comptime, 139 HIR, 86 MIR,
48 resolve, 224 typecheck) in `target/native-secret-owner-local-view-phases.log`.
All 83 driver library tests pass (66.62 seconds) in
`target/native-secret-owner-local-view-driver.log`; all 598 frontend fixtures
pass (22.51 seconds) in `target/native-secret-owner-local-view-frontend.log`.
All seven source-format checks, Rust formatting and diff checks pass in
`target/native-secret-owner-local-view-format.json`.

The supplemental corpus registers 439 tests. The exact clean `cb617ecf`
workspace passes all 439 native tests (905.04 seconds), all 182 object
obligations (516.91 seconds), 83 driver library tests (91.44 seconds), all 598
frontend fixtures (19.08 seconds), remaining workspace targets and doc-tests.
The unchanged-head, clean-start/end wrapper finishes at
`2026-10-02T20:55:57.0941072Z` with exit zero and 92 successful result groups,
recorded in `target/native-workspace-cb617ecf.log` and its JSON summary.
Supported-host acceptance for this revision remains pending. The prior accepted
08f9f7e7 platform and bitfield focused gates are centralized in
[the acceptance audit](native_acceptance_audit.md#current-frozen-revision-and-inferred-json-follow-up).
The predecessor platform result does not validate this secret revision.
Whole-language native parity
remains the 100% objective, tracked with about-85% and the fixed 207 inventory.
