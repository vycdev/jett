# Checked actor required-value baking

This is a bounded HIR transition after actor-owned required cache proofs exist.
It preserves the language rule that explicit `comptime` is closed and pure.
The private checked ordinary producer authenticates its complete input/archive,
constructs each typed value once, matches every result against its private cache
proof, preserves all original locals, and authenticates every added binding.
Only then does it mint a temporary receipt retaining the actual before/after
Programs, exact successful cache, checked session and borrowed type arena.

The actor transition authenticates the whole produced output against that
receipt. It preserves original ActorHandlerEntry receipts/roles and constructor
view inventories, and renews complete snapshots from the receipt-retained output.
Constructor view receipts, facts, locals, backing roots and initializer projections
stay original. Handler role view projections are not reconstructed. The renewed
table is installed in Graph/reflected/temporary peers only at that fixed private
boundary; no public edited table or Generated role can mint replacement authority.

Actor required owner matching joins the exact checked actor DefId/ActorId,
handler or field ordinal, original declaration/body spans, namespace/origin,
response or constructor type, and immutable original HIR operand/header. Equal
values, labels and spans do not substitute for original checked provenance.

Complete snapshot bodies compare exact floating-point bits, including NaN
payloads and signed zero, using the existing HIR block comparison.

## Actual callable sealing and the current execution table

The sole actual callable sealer now retains a private ephemeral receipt of its
complete input/output Programs, checked Source session, borrowed type arena and
source epoch. It replays only the exact recorded getter/adapter token slots,
identified by function, preorder expression and field/adapter position. Bound to
sealed pairs retain their exact admission, return plan, occurrence, materialized
source, checked reference Arc and Graph-source lineage. Initial adapter insertion
must have its unique actual paired getter. An expected table or caller-owned
edit vector cannot create the receipt.

The existing baked view authenticates its complete current input and exact
checked/cache/property/original identities before replaying that retained real
output. It preserves the existing mutable-sum replay and validates full actor
snapshots before and after. It then retains the actual sealed table required by
MIR; MIR's complete archive equality remains unchanged. This renewal does not
admit an actor-body-changing delta. A callable plan moved into a generated
adapter can change the full execution table while leaving the handler snapshot
unchanged.

The four focused receipt controls cover materialized Some-to-Some renewal,
idempotence, foreign checked/cache/arena/archive identities, previous/current
token or occurrence splices, complete unrelated output edits and transactional
refusal. Broader initial-getter, actor-changing and Graph/source-lineage contexts
retain their separate obligations.

The successor focused gates pass **11 HIR tests**, each exercising Debug and
Release checking modes: seven actor required-bake controls and four actual
sealing-receipt controls. The whole unchanged witness control now completes
actual value conversions/sealing, validates HIR/backend output and enforces the
strict full execution-table equality required by MIR. The actor controls also
cover equal-valued occurrences, constructor persistent views, exact
receipt/cache/arena replay, complete input/output edits, transactional late
refusal and NaN/signed-zero bits. Current logs under `target/native/` are:

- `actor-required-sealed-focused-v1.log`, SHA-256
  `c7a954b62fb632a263d81bd31f2a2e440017e12c56cdf3ea825f0455527cf7e5`.
- `actor-required-sealing-receipt-focused-v2.log`, SHA-256
  `5b85981dfdbb8f8cfc1266d7505ec648875c7eb73ecc8e4cb732b552c3032d0c`.

An earlier receipt-test filename filter selected zero tests and supplies no
acceptance evidence. The broader successor gates and separately measured native
output checkpoint below pass within their stated scope.

The initial gate passed six controls and failed the arena-negative test because
`TypeInterner` is not Clone: cloning its borrowed reference retained the same
arena. The test now uses an independently checked same-Source arena. This was a
test repair, with no production change for that failure. The failed log is
`target/native/actor-required-bake-focused-v1.log`, SHA-256
`5c9505545c7704cb3cf14118ff7bd9dd8f76b0e8a27e17c0dc8bc74c2ca1b553`,
and receives no accepted-gate credit.

The fresh successor gates pass **2,539 distinct Rust tests**: 1,886 compiler-library
tests (Codegen 135, Comptime 691, HIR 297, MIR 384 and Typecheck 379), 603
frontend/reference fixtures and all 50 CLI tests. The 11 focused controls are
included in HIR 297. Current logs under `target/native/` are:

- `actor-required-sealed-core-v1.log`, SHA-256
  `7c335abafed1644f9207db8c01bf577316afbb198c87dda58c78e137aebaedfc`.
- `actor-required-sealed-fixtures-v2.log`, SHA-256
  `91a63d3ecf3b365d02d71a61e8b19009002b6b5f497fb494b67da514ea9c1dc2`.
- `actor-required-sealed-cli-v1.log`, SHA-256
  `45bde9c8f4c0132b4689115af273529664cd77111a00a8a143f8d4f5a431d957`.

The first fixture invocation named a nonexistent test target and ran no tests;
it supplies no acceptance or compiler-failure evidence. All 1,081 current
compiler/Source input hashes match
`native-codegen-recovery-2026-10-09/actor-required-sealed-current-source-inputs-v1.json`,
SHA-256 `bd9dd1206287a75f93ff746bc833cf003a111bd579d8842f1bbab53edeed197c`.

The first fresh native gate compiled, but its actual unchanged-witness test
failed during Debug object emission: MIR reported `Resource lowering differs
from its original checked HIR archive`. No object or native process was produced
and no observations file exists; this attempt receives **zero native execution
credit**. The existing reference-output assertion completed before that failure.
The gate receipt is
`target/native/actor-required-bake-native-gate-v1/gate-acceptance.json`, SHA-256
`7308274ed4aa96745fe58ab664b178f030a83b4d5a1ef3dadc96572d14cb5228`.
The execution summary is
`target/native/actor-required-bake-native-execution-v1/execution-summary.json`,
SHA-256 `fda2927016246aaf492d252d77710e8ee0213e0741c6117119ad19dcdc967a61`;
its failed tests.log is SHA-256
`f7f73426842e8a8910eb4228a0090408b23df994cff8b1b7874c0715b0cc489a`.
That failed attempt's 1,079 Source and 68 Runtime before/after guards passed
unchanged. It prompted the actual-sealer repair above. The later accepted
output checkpoint below supplies separate successor evidence.

The existing whole-witness HIR control was then strengthened to complete actual
value conversion/sealing, validate HIR/backend output and require the strict full
execution-table equality used by MIR. It first failed because the retained
current execution table kept stale sealing tokens. Its RED log is
`target/native/actor-required-execution-archive-red-v1.log`, SHA-256
`9282eef9c67c067e6823d0bc58dc55839ebc96895c953e28a339792f3cbc50df`.
The earlier seven-control/2,520-test receipts and 1,079-input map predate that
strengthening and cannot establish acceptance of the later production repair.
The current focused control and fresh broader successor gates are green after
actual-sealer receipt renewal. The native successor attestation below is separate.

The next Rust native test passed with two actual paired output observations, but
its outer v2 attestation remained incomplete because PowerShell formatting
wrapped the harness summary. Its summary keeps accepted observations at zero;
the two raw successful outputs are bounded facts, not promoted acceptance.
The fresh v3 raw-process runner attestation below supplies acceptance for its own
two executions; the earlier raw outcomes are not added to that count.

The HIR unit controls alone supply no native execution credit. The accepted frontend checkpoint
`2e0ebb0e9e27d11eee0e68611fc726803c79a65a` validates retained lazy closure
environments before creating function values. Required expressions cannot retain
unavailable actor-state or handler-parameter captures, including nested bodies or
same-name namespace constants. This snapshot transition preserves that gate.
That frontend checkpoint's CLI checks passed for the unchanged whole witness
and closed-capture control in both checking modes; all four free-capture controls
refused with E9001 in both checking modes. Its frontend-only evidence is
`native-codegen-recovery-2026-10-09/actor-required-frontend-cli-v3/check-result.json`
(SHA-256 `e6c1c28dbb746559d8f39c265a09a3a1995c3e9d5fbc9069214c6efe022170c1`).

## Paired ordinary native output checkpoint

The original whole witness remains byte-identical with Source SHA-256
`ce01b25e20a4955067e87fbc7389f1957b21fd10de6e76575e337f2f98111665`.
A fresh accepted v3 native selector passes **one Rust test**, separately from the
2,539 compiler/fixture/CLI tests above. It emits **two objects and runs two
processes**, pairing Source Debug with the existing normal Runtime Debug bundle
and Source Release with normal Runtime Release. Each process returns exactly
`observed:5\n`, empty program stderr and exit zero through the public object/link
path. This is one genuine complete Source, not a reduced actor or altered callback.

The isolated entry Source is removed before launch, and the emitted host value
and reference result are dropped. The launch directory contains only its
executable. Both children are reaped; executables and temporary directories are
removed afterward. No per-case raw binary/object copies are retained. All 1,081
Source and 68 reused Runtime before/after guards pass, with no Runtime rebuild.
The outer Rust test is reaped without timeout and uses raw stream capture; all
four exact log checks pass.

Receipts under `target/native/` are:

- `actor-required-sealed-native-gate-v2/gate-acceptance.json`, SHA-256
  `5fa7b7be9040cfeb47ec916998fc85ce1b8be5443238830a9d0d7a0ccdfb8170`.
- `actor-required-sealed-native-execution-v3/execution-summary.json`, SHA-256
  `2777801e8a42d77750755aa8cf1d57fd557507b6b61fb9c39438a41545e0d627`.
- `actor-required-sealed-native-execution-v3/observations.json`, SHA-256
  `3adeff2c8a254704150c0951107fb7be4c4fb61e9b8377008202cce570011207`.

The executed fresh compiler image is SHA-256
`db795907492bf9ca9a219f5d794bec247270fdf496d76e7c51a29e78dc934114`.
These are accepted ordinary output observations. File deletion and host-value
drops do not measure physical Runtime ownership, actor queues, callback
finalizers or same-context reentry. Both rows retain
`physical_lifetime_measured=false`, `reentry_measured=false` and
`full_native_acceptance=false`; the full native-codegen goal remains open.

Remaining independent obligations include actual bound-to-sealed callable token
replay when sealing changes an actor body, authenticated source lineage for
Seal.sources, actor-owned Graph producer transitions and Resource-hook-bearing
actor required execution. A callback plan that moves into a generated adapter
need not change its handler snapshot; no unconditional sealing failure follows.
Physical lifetime, actor queues, callback ownership/finalizers and same-context
reentry remain separate from ordinary native output. Do not erase actor snapshots,
change Source, disable Resource transport or infer broader native coverage from
this bounded HIR transition.
