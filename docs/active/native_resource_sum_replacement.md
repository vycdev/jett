# Native direct Resource sum replacement

The current implementation extends whole-binding assignment to distinct owned
optional[R] and result[R,E] shells with an exact checked Resource/failure shape.
The original 204-byte witness remains
23f997a76378f0e805805f7cc3a57b168e00863121bd3e54ab2b93f28366d4ec.
Design and architecture already record the selected contract; this note tracks
its bounded implementation and verification.

The RHS is evaluated once and remains independently owned through preflight and
old-value cleanup. Successful cleanup precedes destination publication. RHS
failure preserves old when it has not consumed it; old cleanup failure retires
the staged RHS once and keeps established error priority. Live None/Fail shells
are owned values with real loans, and Fail retains its ordinary error companion.
Both-absent replacement can run in an otherwise eligible pure purpose; occupied
Resource operations retain their existing capability/purpose restrictions.

Two related compiler repairs preserve this behavior. Typed failure construction
uses existing payload_bits packing and extraction uses typed unpack_payload;
primitive bits no longer become ordinary owning handles, while String/compound
ownership remains intact. Nested owned declarations retain exact original
Source scope/path/header/initializer and sealed current-site records. Exact
lexical exits drop live owners once, clear moved/dead slots and retain pending
Return ownership until transfer. General resealing grants no new owner facts.

These changes reuse the existing Replace operation and wire contract. Production
ABI 1/layout 3, 58 production and 8 private Resource exports, private report
protocol 2 and receipt version 1 remain unchanged. No Source restriction, new
capability policy or hidden clone is introduced. Primitive-failure controls keep
their actual zero MIR owning-temporary bound.

## Local checks and archive measurements

The frozen local receipt records **3,038 distinct checks**: 409 Runtime tests,
1,976 compiler/phase checks, 603 frontend/reference fixtures and 50 CLI tests.
The receipt is resource-sum-replacement-root-verification-v1/local-verification.json,
SHA-256 325751dfe2bf1335c12114667a7a21017bdacb6a449d82abcc5e72991510405f.
It retains exact inputs and logs; focused subsets are not added again.

Six fresh MSVC archives are measured: normal Runtime+Launcher Debug/Release and
private Runtime Debug/Release. The paired run completes with reaped children,
released lease, unchanged 80 Runtime inputs and genuine empty before/after
liveness arrays. Its maximum observed cache is 8,060,836,909 bytes within its
8 GiB limit. These measurements grant zero Source executions. Receipt paths
below are relative to target/native/:

| Receipt | SHA-256 |
| --- | --- |
| resource-sum-replacement-normal-msvc-v1/receipt.json | be980ae18006737304ae69af4d254f78cad15c51fe8e6226c6a2b9ca9cf4b6b4 |
| resource-sum-replacement-private-msvc-v1/receipt.json | 454be0bf810ecc500e362fd99de37e02a909ca3abfc4d89c92afb06e8d1cfc50 |
| resource-sum-replacement-measurement-paired-v1-terminal.json | b725c561901d53bc2fd27b5ae6f5cf07265051be498efc6e4b7a7b1a283395a9 |

## Native acceptance boundary

The fresh compile-only gate is **accepted** with 1,111 compiler/Source inputs.
The 40,875,520-byte compiler image is
03a6ffebbcd1b5a56d986d61bddbaec3ebf164c7af9c480d068df19e2ba4924d.
Gate acceptance is resource-sum-replacement-gate-compile-v3/gate-acceptance.json,
SHA-256 f8e0623b59056bf3a6da1b5f94c4bcd54948a83df67863bbfd8c516cfb719be3.
Inner and outer terminals complete with unchanged guards, quiet/reaped children
and released leases. Compilation supplies zero Source/host execution credit.

Ordinary-object preflight is accepted: its named Rust control passes all six
original Source endpoints in Debug and Release. It supplies no linked Source
execution credit. The separate single-entry matrix accepts **38 child rows and
38 attempts**, using **12 fresh programs across six distinct Sources** in paired
Debug/Release. All exact Rust oracles pass; actual child exits are 22 zero and
16 expected cleanup-failure status 71, with empty stdout/stderr. Every observed
attempt records retired logical Resources and empty aggregate ordinary storage;
private reports have zero incomplete flags. The five actual acquisition/dispatch counters include nonzero
measurements and are retained at their entry boundaries.

All written Source/support files are absent before launch and each fresh
execution working directory is closed and absent afterward. Single-entry
compilation directories, objects and executables are retained for matrix reuse.
This is not prelaunch object deletion or a checked-program Weak-expiry test.
The adapter itself completes quietly with reaped processes, a closed root handle,
released lease and no guard failures.

The retained-Session matrix also accepts **38 child rows and 76 attempts**,
with **38 fresh programs across the same six Sources** in Debug/Release. Each
child processes its selected first attempt then the ready attempt in the same
retained Session. Distinct attempt IDs and exact first-end/second-begin counters
retain the boundary observations. All attempts record retired logical Resources
and empty aggregate ordinary storage with zero incomplete flags. The 38 child
exits again match 22 zero and 16 expected status 71; child streams are empty.
Written Source/support is absent before launch, and each complete compilation/
execution temporary directory is closed and absent afterward. Neither matrix
measures checked-program Weak expiry or per-kind physical storage.

A pre-Cargo SDK-environment refusal and a later shared-test-harness import failure
remain preserved and unaccepted. The subsequent repair changes only the Rust
import path; original Jett Sources and oracles remain unchanged.

Accepted execution leaves below are relative to target/native/. Their separate
process receipts and complete raw-file catalogues are frozen with the checkpoint.

| Phase | Acceptance SHA-256 |
| --- | --- |
| native-bounded-resource-sum-object-preflight-execution-v1 | c5689a99986adfe4746d6f9b347784b1fb46ed123bf330444915f9709cca3744 |
| native-bounded-resource-sum-single-execution-v1 | eb7aa76b517cd0b2075b5086c4b85f9e439121c72bf8f7dc9533bf97d2e54bf5 |
| native-bounded-resource-sum-reentry-execution-v1 | 1c99637b0fe029179a18a81fff393da14f1f34394bf5393def1f29c04422943c |

The two execution matrices total **76 native children, 114 attempts and 50 fresh
programs**, covering a union of **six distinct Sources**. Object preflight is
separate and is not added to these program or execution counts. The frozen
combined rollup is resource-sum-replacement-root-native-run-v1/accepted-observations.json,
SHA-256 25cdbc3dbbc2bf62b2ab20c9868dfb5efedc1de02707aee1938e0f1f7f3c1660.

The unchanged pure_replacement_suites.jett fixture passes required evaluation
and native Verify/Property object emission. Its actual suite execution remains
a separate open obligation.

Earlier actor 268ec7a9 and Runtime e916 observations retain their own compiler,
archive and execution scope. They are not promoted to current Resource sum or
Source17 acceptance. Physical storage/lifetime, per-kind private retainers,
normal-launcher custody/reentry, wider carriers, same-local sum reseating,
aggregate/projected/actor-state replacement and complete workspace/inventory/
installed-host acceptance remain open. Full acceptance remains false; existing
rough tracking and fixed denominators are unchanged. GitHub CI stays disabled.
