# Original actor and retained temporary-owner finalization

This is a bounded compiler transition for the existing checker-valid Packet/list
composition with an initial actor reflected getter. Source policy is unchanged.
The original direct and required-comptime Sources remain byte-identical. The full
combined Packet Source remains the existing TEMPORARY_SOURCE control, SHA-256
`a1f3fa32e14828daebb472f316608683b303187eba63b987edbab56d381a2f0f`.

The actual callable sealer remains the only token producer. Its complete input,
output, exact checked Arc, borrowed arena, raw source epoch and Getter/Adapter
occurrence receipt are unchanged. A separate private producer accounts for the
Original.canonical_producer OnceLocks; table equality compares marker Arc identity
and cannot by itself authenticate those cells' open-to-closed state.

The consuming Lowerer registers each exact proof at mint before body and argument
clones, then closes registration before canonical passes. This non-Clone, non-
Default registry is bound to the original module/check addresses and arena. It is
not reconstructed from a caller Program or checked Arc. A by-value fresh wrapper
owns both registry and unpublished Program. The clone-based ordinary wrappers get
no capability. No public API returns pending markers or the registry. Failure
drops the private construction; cloning alone supplies no rollback for shared
OnceLocks. Ordinary/no-actor/no-delta behavior keeps the existing late setter.

On an actual actor snapshot delta with markers, preparation consumes that fresh
registry and authenticates the sole sealer's real input/output. It joins every
minted P to exact checked Source function/root/statement facts, unique owner/alias
initialization, complete locals and rewritten projection. Each cell must be open.
Canonical candidates are cloned only from the retained BEFORE owner and normalized
by removing only its own exact outer marker. All canonical operands, Source call
certificates, order, spans, types, nested plan identities and foreign argument
proofs remain intact. The normalized OUTPUT owner must equal that candidate,
including callable Arc identity and float bits. A changed canonical producer
refuses until its separate current-canonical transition is represented.

Proof closure visits expression-owned markers and the hidden
SourceArgumentWitness.temporary_origin in all four ownership-bearing expression
variants. Each predecessor must be the exact registered earlier declaration in
the same function/body and checked session; legitimate repeated references are
deduplicated. Self, forward, absent, foreign and conflicting declaration rows
refuse. Initial retained payloads cannot import sealed/materialized callable or
Graph epochs. This establishes a finite earlier-proof dependency graph. It adds
no Source or Generated call, runtime effect, Resource custody or entry authority.

Preparation writes nothing. The actual actor finalizer derives an opaque complete
checkpoint from its retained actor stage and the real initial sum producer. That
checkpoint verifies the full current table, not only temporary rows. Exact
source/checked/arena, dependency, OPEN-state and affected attachment preflight
checks all pass before the first cell write. Commit consumes preparation and
fills each OnceLock once with the retained BEFORE value. Only afterward does
the existing actor epoch capture its complete pre-attachment after table and
attach the edge. Only an opaque completion from this producer can skip the
original late setter; already-filled cells alone are never an admission.

The raw source Arc may temporarily retain open cells inside this unpublished
construction. Final P payloads cannot point back to it: they come from the old
input, equal the actual token output and own only a finite earlier-P graph, with
no sealed current epoch. The final actor edge owns its raw predecessor and a
complete table that does not contain the edge. Current/historical full actor
comparison gates are unchanged. No combined Arc cycle has been reproduced;
actual Weak expiry controls verify this new bounded graph rather than treating
process exit as proof.

Seven new controls cover real BEFORE/OPEN observation and final filled states,
idempotent completion, foreign context/arena/inventory/table refusal before
writes, exact source Arc and complete sibling/function drift, hidden earlier
proofs with repeated argument uses and invalid edges, rejection of real sealed
output payloads as BEFORE replacements, actual archive/Program clone lifetime,
and a separate actor/interface-coerced temporary producer. The unchanged combined
control also requires full HIR/backend/archive/epoch validation and real E/S Weak
expiry. It changes its expected result only after real lowering succeeds; no
Source or selected-value oracle is rewritten. Test-only observers borrow actual
producer receipts and checkpoints; they mint no positive proof or capability.

## Verification boundary

The implementation is applied and formatted. All 35 focused HIR controls pass,
including seven new initial-finalization controls and the existing actor,
required-bake, sealer and temporary-view regressions. The broad local checkpoint
passes **2,553 distinct tests**: 1,900 compiler-library tests (Codegen 135,
Comptime 691, HIR 311, MIR 384 and Typecheck 379), 603 frontend/reference fixtures
and 50 CLI tests. The 35 focused controls are included in HIR 311, not added to
that total. Exact broad logs under target/native/ are:

| Log | SHA-256 |
| --- | --- |
| actor-temporary-owner-core-regression-v1.log | 0745179c292cc76b55510000b8530bd17073909042a09f57d51d8af913f5c11b |
| actor-temporary-owner-fixture-regression-v1.log | 66e11f2cff69c9394ca1aa9e36167af3662d654a2148267ba6038ddf9f4d4e49 |
| actor-temporary-owner-cli-regression-v1.log | a077a1eb56e2638311665196eb3e980f9ce548fe61a6927fa2ec5924dd030b14 |

The fresh compile-only gate is accepted with zero Source execution credit.
The separate native selector passes one Rust test and accepts **one unchanged
Source, two objects and two native processes**. The 970-byte Packet witness pairs
Source/Runtime Debug and Release. Both actual outputs are exactly observed:5
plus LF, with empty stderr and zero exit. These measurements grant the bounded
output/cleanup acceptance below, without adding historical executions or
inferring physical lifetime.

Prior actor checkpoint 8827c57321e6e0d74cee6fd30dd21a7f11940d8c retains its
18 focused/2,546 broad tests and two-Source/four-object/four-process native scope.
The Resource observation results committed at
e9162a5d5aabc6cc5c1386d7c3366d4abc80baf5 also retain their original counts and
receipts. Neither historical checkpoint is promoted to current actor or Source17
regression acceptance by the tests above. Native physical actor/callback/queue
ownership, same-context reentry and full native acceptance remain unmeasured.

## Fresh compiler gate

The accepted gate binds 1,093 compiler/Source inputs. Its image is 40,657,920 bytes
with SHA-256 c308ff075c6dc904684867a3854e6f9d82961c1e417a01a05dd899dbe138237d.
The inner and outer terminals complete successfully: exit zero, reaped quiet
owned processes, disposed root handle, released leases and no errors. Receipt
paths below are relative to target/native/:

| Receipt | SHA-256 |
| --- | --- |
| private-observation-v2-gate-compile-v4/gate-acceptance.json | 290a8b403a76aec6b10d7e1e0b5e27cce268a78cc6600a717761c82914498160 |
| private-observation-v2-gate-compile-v4/terminal.json | 4612c72c6116b744f7836726098dbe22c45cc176078665716b6d92668f88af97 |
| private-observation-v2-compile-supervisor-v5/terminal.json | 32b46311bdfd5959ef93f7f05c20677f91030fc6c0172ca6ad562663c7220508 |

This is actual compilation, with zero Source or host ABI execution credit. Its
separate actual native execution receipt is recorded below.

## Paired ordinary native acceptance

The current selector passes one Rust test in 199.77 seconds, separately from the
2,553 regression tests. Its one unchanged Source emits two objects and executes
two processes, one matched Debug pair and one matched Release pair. These are
not two Sources or a full Source/Runtime profile cross-product.

Both rows require exact stdout bytes observed:5 followed by LF, empty program
stderr and exit zero. The written Source is removed and the host object dropped
before launch; the isolated directory contains only the executable. Native
children are reaped, executables and temporary directories removed. The outer
process is quiet/reaped, its root handle closed and its lease released. The
authoritative acceptance leaf is complete with zero failures.

Receipt paths below are relative to
target/native/native-bounded-actor-temporary-owner-execution-v1/:

| Receipt | SHA-256 |
| --- | --- |
| execution-acceptance.json | 076ac84a3717a56fafa9a5450edc96003a2ec3035e50c4fd10ef3eada9fe78b4 |
| test-process.json | 9782953688918c25d1d42adc161145ba03d9a7fd21934e0b79227d2f44446fb6 |
| native-evidence/actor-temporary-owner.json | 2b19eea41a33e6a94d89050443cad25062e1802e3a76159b7243c982525bbe34 |
| native-evidence-catalogue.json | 56dee1eb862cf6c4d541bb5b7d86eace86623271af16a0581200dfc7473c4e07 |

The schema-v1 observation file is 5,863 bytes and retains two actual rows. The
prerelease execution-terminal.json remains intentionally complete=false and
lease_released=false; its digest is
c5ef3ad06831febba147b872821bd77586151189d04171c973d86868b2fa435e.
The later acceptance leaf proves successful release and is authoritative.
Every observation retains physical_lifetime_measured=false,
reentry_measured=false and full_native_acceptance=false. Process/file cleanup
and compiler Weak expiry do not establish native actor/callback/queue lifetime.

## Remaining obligations

A producer whose operands gain callable tokens needs a separate immutable current-
canonical proof derived from actual receipt slots, including None-to-Some. That
would require a validated HIR consumer API used by MIR, whose current temporary
path directly checks matches_owner_initializer. Do not refill old cells, replace
P identities, strip markers, recapture current tables, or broaden historical
ancestor/Graph/Resource admission. Such compositions remain part of the 100%
goal, alongside later actor epochs and unrepresented proof transport. No language
design change is made, so docs/design.md remains unchanged.
