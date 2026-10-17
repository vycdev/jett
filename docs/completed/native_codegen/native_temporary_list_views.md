# Temporary List views: bounded acceptance

The bounded List transport slice passes local MSVC acceptance on 2026-10-16:
17 focused reference controls, selected compiler-library/fixture/CLI regressions,
and 24 tests / 48 paired Source-deleted native executions. The new List selector
accounts for one test and two executions within that complete matrix. The earlier
focused List pair overlaps the full matrix; it does not increase the distinct
accepted total beyond 48. Physical cleanup and the full connected lifetime
contract remain open.

The selected [lexical temporary-view contract](native_temporary_local_views.md)
remains unchanged. An ordinary producer result stays owned until its original
function scope exits; aliases and named view parameters carry nonowning tickets.
This slice extends the checked reference adapter through List endpoints already
represented by HIR/MIR proof. Its bounds add no checker restriction or language
spelling and do not narrow the complete lifetime rule.

## Transport and authentication

The producer is an ordinary nongeneric named function returning a finite ordinary
struct. An exact checked finite field path selects a List endpoint. The complete
owner, including an unviewed sibling, remains in activation-owned vacant/live/
retired storage. Alias publication and forwarding use payload-free Nothing
placeholders and private exact live tickets. Explicit Source clone produces a
whole owned List copy.

Named observers may be ordinary or generic. A receipt identifies the exact
retained checked generic instance by immutable program and instance index,
including definition, concrete arguments and full specialization. Calls retain
original callee/actual storage, the full ownership packet, parameter permutation,
concrete signature, view modes and selected body facts. Registered Source is
authenticated before independent argument effects. Runtime type projection and
Source-aware type arguments rejoin the exact retained instance; missing facts
do not fall back to global generic body maps.

Both private frame boundaries select checked type projection by the exact
DefId-owned declaration span and rejoin it to that original instance. A later
mutual body-name span is a different key. Canonical mutual declarations retain
their original DefId through the immutable resolver edge and full registered
Source authentication; neither boundary guesses from a name or body span.

Temporary-owner admission preserves the accepted classifier: an exact immutable
original VarDecl with a written view initializer, canonical variable/name/span
and authentic checked Other-view fact. Existing viewed enum patterns and
iteration binders keep their ordinary borrowed paths and gain no temporary
owner authority. Nested true temporary declarations retain raw-entry and
unsupported-region refusals.

The observer adapter borrows the original List for length, sum in the already
specified numeric domain, and get of checked implicitly copyable primitive or
String elements. Get preserves negative/out-of-range optional results and may
copy one copyable element; it does not copy the backing List. Move-only get and
Handle composition remain separate work, without changing Source list.get.
Runtime helpers and ABI are unchanged by this slice.

## Compiler and fixture verification

All 17 focused reference controls pass: five new List controls and twelve
existing call controls. The List controls retain real shipped list.jett bodies
and cover numeric/String endpoints, explicit/inferred generic observers, nested
stdlib calls, same-template specialization and foreign-program identity,
changed-specialization refusal, copied get results and bounds. The genuine user
mutual observer checks distinct declaration/body spans and the exact DefId edge.

The focused log is target/native/temporary-list-transport-focused-v1.log,
SHA-256 cac01d28dac98fada049d521bd80b9139d842d10e9495ebb4861998d7dabd90c.
The terminal reports 17 passed / 0 failed. Root then removed only unnecessary
Rust or-pattern parentheses; retained before/after images are in
temporary-list-pattern-cleanup-v1. Successor selected regressions all pass:

| Check | Tests | Log under target/native | SHA-256 |
| --- | ---: | --- | --- |
| Compiler libraries | 1847 (135 + 683 + 271 + 379 + 379) | temporary-list-transport-core-v1.log | d86f8177789a9aed444317787a49a81854a0e5cd1ca3eb7101ea2d9dde8ebd7b |
| Fixtures | 603 | temporary-list-transport-fixtures-v1.log | d7d77d9c9b9641516890253923c3d822b8b2af2727a532e92f11cf40c1e6594a |
| CLI capture | 9 | temporary-list-transport-cli-v1.log | 56c1b715305b34e36cbb7bf0f537b8ae1e94a510668201b59935adce20db6033 |

Each terminal result has zero failures; Root reports complete sequence exit zero.
These selections do not establish full-workspace or whole-native-corpus coverage.

The first full matrix attempt fails with exit 101 because the authored Source14
uses unsupported match/some/none optional patterns. Its List selector fails
checked-reference parsing before List native building or execution. The failed
aggregate accepts zero executions, despite two earlier retained selectors
passing; their partial observations are never added to a successor receipt.
All Source/Runtime/fixture/original-run-pass guards pass in that failed attempt.

- Failed summary: target/native/temporary-list-view-native-execution-v1/execution-summary.json,
  SHA-256 55751b44050f635c90b0972c39f0f26938ee91171a5fc74448da7d82387be711.
- Failed List log: temporary-list.log in the same directory,
  SHA-256 907fdd85438cc9b59e77c39d1957e175c00bfc323720e4885b22556fb8ae5550.

The fixture-only repair adds number_text(optional[int64]) and
name_text(optional[string]) with canonical bare handle, replacing four optional
text observations. Producer-once numeric/String scenarios, forwarding, mutual
explicit/inferred count, sum/is_empty, get at 0/-1/3, clone/append independence,
the test selector and fixed expected stdout are retained. Compiler files and
native test counts do not change.

The formatted repaired Source14 is SHA-256
ae81376d38a94f9890146138609ee5df641cf49bca9e15530464a50fea6d1c99.
Its frontend receipt in temporary-list-fixture-handle-repair-v1/frontend-receipt.json
is SHA-256 c8eb735fb7d852c9a3388ba556548f96b1a865df8d07aa9165a98cb4519a45f0:
format and Debug/Release build --agent --check all exit zero; each frontend
check reports zero errors and zero warnings. The failed before image is
4723d2929ee5d45f93980ca670e495c2a92e1f6a956c78dcce09baacbec00656.
The initial format-only result and compile-v1 gate never established Source14
validity. That historical gate remains bound to its earlier Source inventory.

## Accepted paired native matrix

The fresh matching MSVC compile-v2 gate passes compile-only acceptance with zero
Source-execution credit of its own. Its receipt is
target/native/temporary-list-view-gate-compile-v2/gate-acceptance.json, SHA-256
40ad0d06d7a5585659b3e39311d60363cbd39848b2384bd5fe9d4760dd4c2968.
The compiled gate image is SHA-256
76f8a7020fb82fd53d573324f5a38730df68261581add438025243798f3a0c82.

The focused repaired List selector passes one test/two paired Source-deleted
executions in target/native/temporary-list-view-native-focused-v2/execution-summary.json,
SHA-256 ae05fa796b52584804f5004fbd3315a0839a5963fc5a92760dec15b49a4e0411.
Its selected log is SHA-256
958d5430766ec664c44de2811514f94fcc14c15e28aaef5607f80429b12572b9.

The complete successor passes all 24 tests and 48 paired Source-deleted
executions, with 24 Source Debug/Runtime Debug and 24 Source Release/Runtime
Release executions. There are zero publication refusals and zero legacy
Debug-only runs; no full profile cross-product is claimed. The terminal receipt
is target/native/temporary-list-view-native-execution-v2/execution-summary.json,
SHA-256 a2b39a53402f88709493b2b022d97d6d8ce342c8b46b62252ce7007d7cd1ee84.
All sixteen selected log hashes and exact passing terminal test counts were
independently checked against that receipt.

| Accepted selection | Tests | Debug/Debug | Release/Release | Source-deleted executions |
| --- | ---: | ---: | ---: | ---: |
| Retained selections, freshly rerun | 23 | 23 | 23 | 46 |
| New List selector | 1 | 1 | 1 | 2 |
| Complete matrix | 24 | 24 | 24 | 48 |

The 48 already include the earlier focused List pair. The distinct accepted
packet total is 48, not 50. The committed byte checkpoint at 291ccbb0 retains
its own historical 23/46 receipt, and is not added to the refreshed List matrix.
The retained controls cover two bytes temporary tests, eight stable projected
views, eight scoped calls, two mutable owners, one reflected mutable view and
two actor persistent views. Their regressions grant no temporary-owner authority
in reflected or actor contexts.

The exact new selector is
local_view_aliases::native_temporary_local_list_views_match_checked_reference_without_source.
It calls run_case once. That helper obtains the genuine checked reference
outcome, compares it with fixed expected stdout, builds each matching normal
Runtime profile, deletes copied Source, then compares native status/stdout/stderr
and applicable Debug events. The repaired fixture observes one producer call per
scenario, numeric and String Lists, alias/generic reads, copyable get/bounds and
explicit clone independence. Its success oracle requires no stderr or Debug
events. These observations establish selected behavior, not physical lifetime.

All 1058 compiler/Source, 68 reused Runtime artifact, 65 fixture and 182 original
run-pass guards remain unchanged before/after execution. All six count/launcher
sources are retained; Source14 alone grows the fixture inventory from 64 to 65.
The ready-v2 runner changes only its Source14 pin from the first ready runner;
all prior 23 tests, selectors and expected stdout stay intact.

- Repaired Source14: ae81376d38a94f9890146138609ee5df641cf49bca9e15530464a50fea6d1c99.
- local_view_aliases.rs: 23687b0f4d91e310645059f4170c358090e82cb8ffd627075c835016745c5a1d.
- Unchanged Source09: c0f288b659bdf84bf87fb7f46576eb7cadd3b440c000f726b5b1c099ccfd83af.
- Unchanged Source13: 48391e6a0fa7390f6d36cb190c8e4100d835218ca317232a64ec8ec228cd59aa.

## Remaining contract

Output and logical audit events do not prove physical owner destruction timing
or absence of hidden allocation/cloning. Private measured cleanup counters,
whole-owner/sibling lifetime, earlier-owner cleanup order and full connected
failure traces remain required. Nested regions, loops, abrupt exits,
general/generic producers, reflected bodies, actor/Resource owners and move-only
get/Handle results retain their implementation and acceptance obligations.

This receipt does not establish complete Resource/Graph integration,
suite-factory, full native inventory/corpus, workspace/doc-tests,
GNU/installed-distribution or whole-language native acceptance. The fixed
207-fixture/182-object denominators and rough planning estimate are unchanged.
GitHub CI remains disabled at the user's request.
