# Native bitfield projected local views

## Selected contract

Rule Set 24 admits readonly views of immutable local data. Existing bitfield
field access exposes its declared semantic type, including the final
`list[uint8]` payload on a byte boundary. The bounded native repair extends only
the typed initializer proof to a declaration-backed Bitfield Field step.

Resolve the exact bitfield and canonical zero-based FieldId, then compare the
borrowed `BitfieldFieldDef.ty` exactly. Preserve the checked base/owner type,
intermediate and endpoint types, terminating LocalId/type and immutable immediate
backing chain. Existing nominal/secrecy qualification, transparent conversions,
cycle checks and successful-initializer ledger remain required, including for
unused/orphan metadata before optimization. No parallel field table is allocated
or copied. Ordinary structs, bitfields and exact-state machine steps can compose
through the existing finite Field tree.

MIR retains its persistent whole-owner loans, liveness and non-owning slots.
Existing native emission uses zero-based bitfield indexes and checks pending
receivers at each Field access. Borrowed linear reads retain their handle;
numeric fields retain implicit copying, ordinary owned field reads retain copies
and explicit clones acquire independent ownership. The existing payload schema,
runtime carrier and ABI do not change. No implicit clone/join, hidden owner,
mutable/temporary view, alias escape, projected write, owner-change permission or
loan-expiry policy is selected. This note does not claim a later owner domain or
MIR repair.

## Actual pre-change characterization

The first two sources in `target/native-next-after-inferred-json/results.json`
pass two format checks, four frontend profile checks and two ordinary default-
profile reference runs with empty debug observations. Exact application outputs
are `2:3:2:4007ff` and `3:2:4007ff`, with no LF. Both borrowed-alias AOT profiles
refuse the typed HIR projection before archive lookup; both owning-copy controls
reach unavailable-archive lookup. All four existing output sentinels survive.

Three sources in `target/native-bitfield-projected-view-extra-witness/results.json`
pass three format checks, six frontend profile checks and three default-profile
reference/agent runs with exact two-line output and empty stderr/debug captures.
They cover a view parameter, mixed Envelope/Packet paths and an empty payload,
with forwarding, independently grown copies and original owner rereads. Their
six prior AOT attempts refuse the borrowed proof and preserve six sentinels.

The two sources in `target/native-bitfield-projected-view-pending-witness/results.json`
pass two format checks and four frontend profile checks. One default-profile
reference/agent run fails at whole pending Packet access after `before:whole\n`,
before its after-binding marker. The other succeeds with
`before:payload\nitems:3:2\n` and five exact ordered Trace events: two cloned
payloads go from depth two to one while the original stays at depth two. The
four prior AOT attempts refuse the borrowed proof and preserve four sentinels.
These prior refusals and reference runs are distinct from new native execution.

## Focused acceptance and remaining gates

Both new HIR tests pass (134 filtered, 0.00 seconds) in
`target/native-bitfield-local-view-hir.log`. They pin ordinary-struct/bitfield
composition, view parameters, forwarding and owned controls, plus unused forged
owner/index/endpoint/root and orphan metadata with otherwise valid structure.

All seven linked cases pass together (425 filtered, 3.89 seconds) in
`target/native-bitfield-local-view-linked.log`. Both matching-profile builds
complete before source deletion. Executions require exact application stdout,
status, stderr, typed runtime events and empty frontend/artifact observations.
The whole pending owner reports status 71 and its exact field-access error;
status 72 cannot pass as cleanup success. Payload-copy success preserves five
ordered debug Trace records and empty release stderr, with original depth-two
payload retained. Silent successes require status zero and empty observations.
The crossed library gate passes 951 tests (41 codegen, 416 comptime, 136 HIR,
86 MIR, 48 resolve and 224 typecheck) in
`target/native-bitfield-local-view-phases.log`. All 83 driver library tests pass
(86.96 seconds) in `target/native-bitfield-local-view-driver.log`; all 598
frontend fixtures pass (21.88 seconds) in
`target/native-bitfield-local-view-frontend.log`. The rebuilt CLI passes all seven
fixture-format checks in `target/native-bitfield-local-view-format.json`, and
Rust formatting and diff checks pass.

The corpus registers 432 supplemental tests. New-head complete workspace,
all 182 object obligations and supported-host acceptance remain
pending. The accepted clean 08f9f7e7 parser predecessor is recorded separately
in [the acceptance audit](native_acceptance_audit.md#current-frozen-revision-and-inferred-json-follow-up).
It cannot substitute for the new source gates. Whole-language native parity
remains the 100% objective, tracked with the unchanged about-85% estimate and
fixed 207 inventory.
