# Private Resource Runtime observations

Status: the implementation across 16 Rust paths passes 407 focused local checks
and six fresh MSVC archives have been measured. The fresh compiler gate is
accepted. The bounded native ingress and whole-Source17 gates now pass. Archive
measurement and compile-only acceptance grant zero new Source executions. Historical reports retain their original protocol and limits.

| Contract | Value |
| --- | --- |
| Production Runtime ABI | 1 |
| Current Resource layout wire | 3 |
| Production Resource/private C exports | 58 / 8, unchanged signatures |
| NativeResourceCounts / ArchiveInfo | 72 / 32 bytes |
| Receipt / normal launcher manifest version | 1 / 1 |
| Private ArchiveInfo.test_protocol | 2 |
| Private Source report | JTRRP002, numeric version 2 |
| Private host-control envelope | JTRHC002 |

The double-private recorder measures five separate cumulative pipeline stages:
all HookPrepare ingress, acquire-HookPrepare ingress as a subset, FactoryPrepare
ingress, FactoryCommit ingress and actual provider.construct dispatch. A live
authenticated context is required for attribution. Refused outputs/handles or
early purpose/descriptor/provider checks do not erase already observed ingress.
Unknown/dead context is unobservable rather than an attributed zero. These
stages are not summed into a fictitious acquisition count.

Successful Property mutations retain exact optional context bytes. For Resource-
ABI calls entered with an installed Property context, validated prologues and
first real completion publication retain exact call/scope and checked target
identities, original body versus selected failure status and actual installed
int64 parameter bits. Genuine Begin/End use the original attempt owner; counters/
history stay cumulative across reentry and observation. Nested calls entered
with Property context remain raw history. Calls entered without that context,
ordinary ABI calls and non-int64 parameter values are explicitly unobserved.

The matched private launcher copies an owned Rust snapshot through the existing
authenticated state contract. It adds no C export or pointer-layout field and
does not retain Jett values, consume provider script, clear failure channels or
grant Source/custody/provider authority. Strict report decoding rejects old
versions, unknown flags, malformed kinds/lengths and broken attempt/call ordering.
Known sticky incompleteness is preserved in reports and excludes Source17
acceptance. None and Some(empty) remain distinct. Bounds/allocation failures
never fabricate known-zero telemetry or change Source error selection.

Generic decoding retains legal context overwrites and redundant clears; it does
not select a new Source rule. Source17's separate oracle compares its actual
runtime schedule and installed argument bits to the retained checked plan;
static labels remain expected-only inputs. Each independent
acquisition stage must be measured rather than inferred from provider events.
Archive measurement alone establishes no native history; the accepted runs
below establish only their reported observations.

Individual ordinary storage-kind counts, private artifact-retainer counts,
normal-launcher custody/reentry and native physical lifetime remain unresolved.
This instrumentation does not by itself grant full native acceptance.

Ingress controls must keep original-Source entries, host ABI controls and pure
recorder/writer controls separate. Mode 11 has no Session or Source-entry credit.
Mode 12 deliberately expects a BYTE_LIMIT observation fault while preserving real
diagnostic/status/cleanup; that control cannot certify Source17 telemetry
completeness. Source17 requires every sticky incomplete flag to be clear.

## Local checks and measured archives

The 407 focused checks comprise 389 normal-cfg Runtime library tests, 12 strict
report decoder controls, one Source17 wrong-value/order/incomplete-call oracle
control and five host decoder controls. These local tests do not execute Jett
Source through the matched private launcher. Their logs under target/native/ are:

| Log | SHA-256 |
| --- | --- |
| private-observation-v2-runtime-unit-v1.log | bd2b734d74ce3e05d7ca4532bca33d2ca1afe02e0f1caaad27d44245011c5d87 |
| private-observation-v2-report-controls-v1.log | 5ad4ac199ef13dec8fa76d6df08815f8ad98e43042b1aa34d9e995a23f3fbf80 |
| private-observation-v2-source17-oracle-v1.log | a5cca540afe1c11a8c3c7e75a6287ee0232f94e368101d7a792ffd9e56feeecd |
| private-observation-v2-host-decoder-v1.log | 5882ae7b4cc7f69c64df48b8d1c69976b1a1b39e96252da8594c87d69d56871b |

The paired measurement completes with both child processes exiting zero, reaped
owned children and a released lease. All 77 recorded Runtime input entries,
config/helper maps, actual tool maps and 68 historical artifact guards remain
unchanged. Four normal rows cover Runtime+Launcher
Debug/Release; two private rows cover Runtime Debug/Release. Exact receipt paths
are relative to target/native/:

| Receipt | SHA-256 |
| --- | --- |
| private-observation-v2-normal-msvc-v1/receipt.json | 4e41eca939e70f4c1ea162639b3f714510726a118a200d38bb89bddc6d575e8c |
| private-observation-v2-private-msvc-v1/receipt.json | d19cc38b1d38a6f84783ad0fd1dc219cea722d88ce5e1db8ddba2b025d64dbe0 |
| private-observation-v2-measurement-paired-v1-terminal.json | 411f53b3f27b98962ed900c336bd1a7c89f88c7836c7ccebdccd3b53ee6aa52f |

The actual Source17 normal-receipt literal is refreshed from that measured
receipt. Its harness hash is
201788c70e0ecb6dc3e475970606e7e3a747e28af00181541b131888a65da26c;
the whole original Source remains
c8e74848356a5b3d09ab6659318b1e1292718d34443f7f37222bafd6f3eed0ff.
The 16-path formatted map d8da07ef518ed74c09fb9f27b5dc4947ceb9101f450508f653f22fdc60dc0e50
precedes that one literal replacement; the actual replacement is recorded
separately, rather than rewriting the earlier map.

The repaired verifier authenticates the actual linker invocation by physical
file identity and digest while preserving strict recorded command spelling. Its
exact pinned measurement join uses the genuine preflight's observed empty
liveness list; the terminal's duplicate empty-object serialization is preserved
as a known serialization defect and supplies no liveness observation. The guard
controls pass four real argument baselines and 12 refusal controls. These checks
do not replace native execution.

The native ingress and Source17 v2 history/counter gates now have accepted
execution receipts joining the compiled gate, exact archive/source/tool inputs,
raw reports and authoritative execution terminals. Actual categories and
observations follow below. Per-kind physical observations and full_acceptance
remain open. GitHub CI remains disabled.

## Accepted compiler gate

The fresh MSVC native-conformance gate compiles successfully and its inner and
outer terminals are accepted. The retained gate image is 40,563,712 bytes with
SHA-256 7a626568c675d99fe8464843efe69735232d9e0b8fb0d786853193801fde6205.
Exact receipt paths are relative to target/native/:

| Receipt | SHA-256 |
| --- | --- |
| private-observation-v2-gate-compile-v3/gate-acceptance.json | 3d05e4499c87c4d606ca515ca1ab2c7c9ece96331749bb564b62dfe017f78cc3 |
| private-observation-v2-gate-compile-v3/terminal.json | 55d5c2889f7fa5fae6e19a668c3cee6f07e71c9a7bdfd35bfc75de219691b3e6 |
| private-observation-v2-compile-supervisor-v4/terminal.json | 2276b0b86a8469365642ce61179a323573a013e1b255cedcaab8c474a75866a5 |

The compiler process exits zero and is reaped, its root handle is disposed,
owned processes are quiet and both leases are released. The accepted guards bind
1,089 compiler/Source inputs, 69 fresh Runtime artifacts, 68 historical artifacts,
33 measurement artifacts, 27 helper entries and 182 original run-pass inputs.
The outer watcher observes a maximum 7.5453 GiB within the 8 GiB limit. These
receipts explicitly record zero Source and zero host ABI executions.

Two earlier attempts remain preserved and unaccepted. The first watcher refuses
a recycled descendant PID after Cargo's successful compile and terminates the
producer; the second preserves its guards and releases its leases but refuses
a cached test image. The accepted third gate requires a fresh compiler
invocation after updating only the test target timestamp; its Source bytes
remain 1eb3316afe08153c48f4da8e9e33cc233ac30896d6b3695947f5e45d0c4b7e75.
The original Source17 fixture remains unchanged. Five pure PowerShell and eight
pure Python identity controls also pass; they are separate support checks, not
additional Jett Source executions or part of the 407 local-test total.

## Bounded native observation acceptance

The ingress gate passes 32 actual processes: eight original-Source processes with
10 actual entries read from report headers, 22 host ABI controls and two pure
recorder/writer controls. Its four linked artifact rows are build evidence, not
executions. There are two distinct original Source hashes. Source processes,
host ABI controls and pure writer controls retain separate credit categories.

Whole unchanged Source17 passes eight processes/four objects across Debug and
Release with matched normal and private launchers. The four private processes
enter twice each on the same context, giving eight private entries. For each
profile, the private Property process records two actual 100-trial entries and
6,404 cumulative history records; the private Verify process records four
history records. All private incomplete flags are zero. Each of the five
independent acquisition counters is measured zero, rather than inferred from
empty provider events. Successful Property context/call history and actual
installed int64 bits pass the retained Rust oracle. Exact bits stay in original
hashed raw reports and JSON bytes; no JavaScript-number rewrite supplies proof.

All eight Source17 statuses are zero. Written Source and objects are deleted
before launch; checked-program Weak and input-Arc roots are released. Each child
is reaped and its executable/PDB removed. Normal-launcher custody, reentry,
acquisition and history telemetry remains null and unmeasured. Per-kind ordinary
storage and private artifact-retainer counts beyond the stated root releases
remain open, as do physical lifetime and full_acceptance.

Exact accepted receipt paths are relative to target/native/:

| Receipt | SHA-256 |
| --- | --- |
| native-bounded-ingress-execution-v3/execution-acceptance.json | 56c746359c5fcd7ab73399ed98f64de5fe80d048d94f2d4a33b0706d09c773fb |
| native-bounded-ingress-execution-v3/native-evidence-catalogue.json | 4347e7b4a69a43b43466954538f07dad69a99c83aee5a0d127eabb97d231ae39 |
| native-bounded-source17-execution-v3/execution-acceptance.json | a59a3d4cadf23a05d30b7ecceb034f501dd40d02f404570f33309d787192087b |
| native-bounded-source17-execution-v3/native-evidence/summary.json | 271caf5fbd93f9fd4733185188ad270b344f2dcebba0f126ed93df984ffdb5e6 |

The Root rollups under
private-observation-v2-root-native-runs-v3/ingress/accepted-observations.json
(c1d7a8a8d103c23b7d239bbf833d98d37b22bd04ce043c9baf86c02579291dee)
and source17/accepted-observations.json
(510b1fced511bd5817f3df2e9cfd2af04edf3c223520ab528453524f04feaf93)
retain the authenticated categories and original report hashes.

The accepted v4 runner configures a 15-second helper-startup allowance, 5-second
CIM operation timeout, 30-second single cleanup deadline and 3,600-second active
deadline. Two earlier prelaunch refusals remain preserved: missing physical-tool
PATH and insufficient five-second helper startup. Neither receives Source
execution credit. GitHub CI remains disabled.
