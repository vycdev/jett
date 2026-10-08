# Checked reflected-field execution and native ordinal transport

The checked reference interpreter now enters the exact original body selected
by a direct `for field in type.fields[T]()` loop and its
`comptime type Element = field.type_info` binding. This repairs existing checked
Source execution; it adds no language syntax, provider or runtime capability.
The checked Resource lowering path now retains a direct nominal-struct field
ordinal bridge through HIR and MIR. Four focused Source cases pass 16 linked
native executions and 24 entry attempts; the unchanged 191-case corpus also
passes all 382 Source-deleted Debug/Release executions. Original Source06 also
passes 18 exceptional same-grant executables / 36 entry attempts. Derived
Sources7/8/9 separately pass 42 native executables / 62 entries. Dedicated
Sources10/11 pass 48 native executables / 72 entries, recorded separately below.

## Retained selection proof

A private prepared loop retains its exact checked program, original For/body,
parent attempt, resolver binder definition, concrete owner and ordered field
snapshot. Each item carries that proof and its ordinal. Before entering the
selected scope, execution validates the actual item against the retained full
field metadata, rejoins the original binding and lexical path, and selects the
checker-owned `ReflectedIteration(ordinal)` body with its exact TypeId and
source-visible reflection. Names, equal canonical types and lookalike metadata
cannot identify an executable body. Preparation does not change the cursor.

The checked `type.fields` intrinsic reads retained owner metadata. An ambient
reflection table cannot supply the proof or redirect the selected body. Equal
`int64` fields retain distinct ordinals; an alias field additionally retains
its source-visible name and kind. Struct, bitfield and concrete generic owners
use the same boundary. Existing shape probes for primitives, enums and opaque
resources return empty field metadata without enabling a provider.

Named and recursive calls isolate their active item selectors and restore the
caller's selectors and aliases. Scope entry retires on ordinary completion,
Return, Break, Continue, Handle Default and unwinding. Entry cleanup restores
its original full context even when a provider or finalizer panics; the existing
cleanup-precedence rule remains intact. Closed pure required evaluation uses
the same checked selection without receiving a runtime provider or grant.

## Genuine acceptance

The reference checkpoint passed all 541 `jett_comptime` tests. The candidate's 3,664 frozen inputs and
recorded parent HEAD remain unchanged at that gate, as recorded in
`target/native-resource-reflected-field-reference-evidence/after-v5.json`
(receipt SHA-256
`5c3104bee1dd58acb19c861f17dd61c39aacbd879ad59856ea80dd3b507b7966`).
This includes 19 added tests for original-node/parent/program proof boundaries,
corrupt ordinals and metadata, distinct equal-type fields, aliases, genuine
same-body recursion, required evaluation, selected control signals, and
heterogeneous/bitfield/concrete-generic metadata.

The strict Resource runner executes unchanged Source06 plus three explicit
control-flow adaptations in both checking profiles. Its 74 entry attempts
cover nine original scripts, same-provider/grant reentry after every script,
and ten derived script controls. Exact provider/finalizer panic text and
cleanup-wins outcomes, ordered events, zero owners/registry, empty debug events
and full entry-context restoration are checked before interpreter teardown.
The original Source06 bytes remain exact; derived Break, Continue and primitive
Result Default bodies are named separately. Reference observations do not
independently count every native loan or expose remaining script length.

Reproduce the crate gate with:

```text
cargo test --offline --locked -p jett_comptime
```

## Native compiler ordinal bridge

The new native producer is the direct `for field in type.fields[T]()` loop
over a nominal struct at the root of an exact non-generic, noncapturing function
in an authenticated `CheckedResourceProgram`, with a direct
`comptime type Element = field.type_info` binding in that loop. It
retains every checker-selected `ReflectedIteration(ordinal)` body, including
fields with equal canonical types and equal source-visible type reflection.
The ordinary `lower_program` entry retains its existing equal-type/reflection
coalescing. Checking specializes type, name, kind and primitive facts, while
`field.index` and `field.name` branches inspect the current runtime field binder.
Existing native `type_construction_enum` and `comptime_reflected_callbacks`
fixtures already distinguish same-type fields. A separate ordinary positive
control passes 30 Source-deleted native executions without a production change.
This covers five Source forms across nominal/generic fields, aliases, bitfields,
variant/state fields, type arguments, nested fields and getter pipelines. It
establishes parity for those controls; no ordinary coalescing defect is established.

The immutable `ResourceSourceArchive` captures the exact original checked
program, declaration, For and binding objects, resolver identities, concrete
owner TypeId and source-aware owner reflection, ordered complete field table,
checked body facts, HIR field headers and original lexical paths. Only the
successful checked constructor can create this archive membership. Public HIR
changes, source spans, field names, an equal TypeId or lookalike metadata cannot
replace the original selection proof.

MIR derives an ordinal guard from that archive: the original immutable field
binder's `TypeField.index` must equal the selected ordinal. Each guard leads to
a one-arm `ReflectedTypeDispatch` which checks the source-aware TypeInfo identity
before entering that ordinal's exact body. Two `int64` fields can therefore
select different field-index- or field-name-dependent bodies. Each native
TypeInfo dispatch retains the existing unique-identity rule. Metadata stays
ordinary storage and grants no executable authority by itself.

Each arm carries its original lexical path. Nested handler lookup remains
inside that arm's archived subtree, preventing equal source spans in distinct
specializations from redirecting a Scope. Existing borrowed-projection and
lexical-retirement seals retain distinct local/site membership, child-before-
parent loan endings and Return cleanup rules. Private MIR records seal each
guard, dispatch, body target, refusing target and current field header against
both the original archive and the constructor graph. Canonical sequence edits,
block deletion and local remaps transport only current sites and headers;
original Source paths stay fixed. Reordered equal-type ordinals, redirected
guards or body targets, changed metadata, stale paths and edited original HIR
fail validation before provider or runtime effects.

An ordinary helper inside the same checked Resource program may retain its own
exact field-loop proof. Its MIR witness supplies only that proof: runtime
classification stays ordinary, with no Resource execution plan, hidden Scope
parameter, provider permission or custody authority. Existing ordinary sequence
preparation still handles its map, string, view and uninhabited loops through
the private finite `SequenceEdit` validator. It preserves function identities
and original headers, forbids added custody, and refreshes only the current
graph and independent body seal. Resource execution functions retain their
separate reconstructed consuming-list transition.

No Source rule, runtime leaf, ABI or wire-v2 record changes in this bridge.

## Affected compiler phase gate

The integrated candidate passes 1,202 checks: 542 comptime, 203 HIR, 286 MIR,
107 Cranelift codegen, 63 object and one CFG check. The ten result groups report
zero failed or ignored tests. Focused controls cover equal-type ordinal/body
selection, field-index/name-dependent bodies, altered original HIR and metadata,
reordered guards, redirected dispatch/body targets, stale Scope paths, proof-only
helpers, and their retained ordinary sequence preparation. The canonical view
control is `for observed in view values:`; the uninhabited control uses the
inferred empty `for impossible in list():` form.

`target/native-resource-reflected-field-native-bridge-evidence/after-v4.json`
records actual session 4116, terminal `1335da`, exit zero and all 3,679 frozen
inputs unchanged at parent HEAD `f9fa84a6f05b2886a83e09fad4ae3b6dcb22c74a`.
The receipt explicitly records `native_execution_accepted: false`. These
phase checks do not prove linked native execution, matched runtime archives,
Source absence at launch, script exhaustion or zero native obligations.

## Native execution acceptance

The four focused cases are the byte-preserved original Source06, distinct
field-index bodies, distinct field-name bodies and a proof-only ordinary helper.
Both checking profiles pass eight single-entry executables and eight clean
two-entry executables: **16 Source-deleted native executions / 24 entry attempts**.
The clean two-entry cases retain the same Session, provider and grant. Each entry
has the exact outcome, ordered events and zero ordinary/Resource obligations
before teardown. The combined script is exhausted before one Session destruction.
The focused receipt is
`target/native-resource-reflected-field-native-bridge-evidence/native-acceptance-v5.json`
(SHA-256 `0b05489e3b97201ba0aee4642c2f157e81d5f9684e818f347eb403f775cda10a`), actual session 29429,
terminal `9cfc8d`, exit zero.

The unchanged predecessor corpus separately passes **191 cases / 382
Source-deleted native executions** in Debug/Release in 1,273.64 seconds. All
strict reports pass. The full receipt is
`target/native-resource-reflected-field-native-bridge-evidence/full-native-acceptance-v5.json`
(SHA-256 `31d6b59bebc1e11c18b9dfb6102b5f5486f28dfd69d1439fce15498aa4050db9`), actual session 49641,
terminal `1d2132`, exit zero. Both execution gates keep all **3,686 frozen inputs**
and **nine runtime files** unchanged, including the matched measured MSVC
Debug/Release archives and their build/CRT/native-static-libs receipts. The frozen
test executable SHA-256 is
`76fe2a90106ee8ede00eb86758cefbaa4cefb42d6b818e90a06337b029aba39d`.
The phase receipt above remains a distinct earlier compiler gate.

## Ordinary reflection positive control

The independent ordinary control passes three Source families in five forms,
each with main, verify and property suite executables in both checking profiles:
**30 Source-deleted native executions**. Reference stdout and empty debug-event
oracles, explicit-comptime stdout and native suite outcomes all agree. Its receipt
`target/native-ordinary-reflected-ordinal-control-evidence/after-v1.json`
(SHA-256 `4a282285082fc411b4d8714a890b13c7146d81970533522469eb5848d4a0177e`) records actual session 21527,
terminal `6befe6`, exit zero and all 3,703 frozen inputs unchanged. It explicitly
records `production_changed: false` and no established ordinary equal-type
coalescing defect. These ordinary controls do not establish Resource transport
for those broader reflection families.

## Original reflected failure single entries

The byte-identical 929-byte Source06 now passes all eight original exceptional
scripts in both checking profiles: 16 reference observations, 16 emitted objects
and **16 Source-deleted native executions**. The cases cover handled construction
and borrow failures, provider panic, explicit close cleanup failure and both
provider/domain failure with cleanup failure. Exact completion channels, messages,
ordered events, script exhaustion, zero ordinary/Resource obligations before
teardown and one Session destruction agree with the strict native oracles.

`target/native-reflected-exceptional-native-evidence/after-v1.json`
(SHA-256 `38662e59e4dfebfbbde4236ee0a8d6b2cc647b6a9554ea8afb9f2b1119bf3085`)
records actual session 2180, terminal `aa0907`, all three commands exiting zero,
and all 3,720 frozen inputs plus nine measured runtime files unchanged at
`2be038c77f084395e8638beaaf2617fe86b49730`. Only additive test leaves are involved;
compiler/runtime code, four READY controls and the 191-case catalog are unchanged.
These single entries do not establish exceptional same-grant reentry.

## Original reflected exceptional same-grant reentry

All nine original Source06 scripts now execute followed by exact READY on the
same Session, provider and paired Network grant in both checking profiles:
**18 Source-deleted native executables / 36 entry attempts**. The shared checked
reference matrix separately passes 36 real calls. This includes all three
cleanup-failure cases as well as the original handled Return and provider-panic
paths. The exact 929-byte Source06 and original scripts remain unchanged.

The private launcher first completes the actual attempt and copies its original
body status, cleanup status, selected channel, exact ordinary/Resource message
bytes, event boundary and live counts. Every entry has zero owners, loans, frames,
registry entries, owner/loan/frame handles and provisional returns, plus empty
ordinary value storage, before teardown. After these checks, completed Resource
cleanup failure permits the declared next entry when current ordinary status is
zero. Existing `entry_begin` independently validates the installed entry, attempt
budget, ordinary first-error/cleanup state and absence of active or residual custody.
It creates fresh attempt/root identities on the retained Session/provider/grant.
No failure latch is cleared and no reset or failure-taking API is invoked.

Each original completion and diagnostic remains immutable in the report. Events
retain one continuous sequence with exact per-entry boundaries; the combined
script is exhausted and the Session is destroyed once. The first nonzero process
exit remains 71 or 73 after clean READY. Handled-domain Return with failed cleanup
retains original body 0, cleanup 255, Cleanup channel and exit 71. These are exact
observed exceptional outcomes; the second entry does not replace the first record.

Fresh independently measured MSVC Debug/Release archives were built in actual
session 23542, terminal `e09148`, exit zero. Their receipt is
`target/native-reflected-exceptional-reentry-archives/measurements-01/receipt.json`
with compiled pin
`96721717326f7d6f96dd2bb98dc1d6e919f7119235e33e103e5ac89bd6f29d0a`.
The private launcher continuation changed; runtime ABI 1, Source wire 2 and
script/report protocol 1 remain unchanged. The archive build alone is separate
from the subsequent actual Source execution.

`target/native-reflected-exceptional-reentry-evidence/after-v3.json`
(SHA-256 `f3dc34c6a13079c6e0be04716d37af9a69580a7b99b859a4ebd5301376e3aa8b`)
records actual session 30992, terminal `22c716`, both Rust harnesses exiting zero,
and all **3,722 frozen inputs / nine runtime files unchanged** in the tested
candidate based on parent `b5c55ac7ed9ec559fbae6616de74fff507bff042`. The exact
manifest filters each ran one real test. Earlier session 52970 selected zero
tests; preserved `after-v2.json`
explicitly records zero executions and nonacceptance. The earlier single-entry,
four READY and 191/382 receipts remain separate evidence with their original
archive pins. This gate establishes the original exceptional same-grant matrix;
fresh-archive predecessor, coherent-workspace and exact-revision CI gates remain
separate obligations.

## Derived reflected control native acceptance

The unchanged derived Sources7/8/9 now pass their historical ten-script matrix
in both checking profiles. The executed signals are **three handled Returns,
two Break paths, two Continue paths and three scalar Handle Default paths**.
The first-borrow failure in Source7 and first/later-borrow failures in Source8
return from their handlers before Break or Continue; they are counted as Return.
A separate eleventh Source9 READY baseline was added and verified, without
changing Source bytes or the original ten scripts.

Break preserves the total17 guard and retires the selected scoped-type body
before the consuming close. Continue preserves both field visits and total34,
and skips the statement after its scoped-type body. Scalar Default substitutes
17 for the failed primitive borrow, resumes the selected body and preserves both
field visits and total34. These exact controls exercise selected body retirement
with the outer optional owner retained until its existing close or handled Return.
They do not
establish arbitrary Resource-valued Default or other body/Default lifetime
compositions.

The five actual gates pass **22 single-entry reference calls, 40 same-grant
reference calls and 22 emitted objects**, followed by **22 Source-deleted
single-entry executables / 22 entries** and **20 same-grant reentry executables /
40 entries**: **42 native executables / 62 entries** and **62 reference calls**.
Every historical first script is followed by its own unchanged Source's READY
on the same Session/provider/paired Network grant; the separately verified
Source9 READY supplies its two-borrow tail. Exact completion tuples, diagnostic
bytes, continuous events and per-entry boundaries, zero ordinary/all eight
Resource obligations before teardown, combined-script exhaustion and one clean
Session destruction all pass. The three explicit-close cleanup cases retain
body 255 / cleanup 255 / Cleanup channel / exit 71 and exact
`native Resource cleanup failed`; their original completion and first nonzero
process exit remain unchanged after clean READY. Existing `entry_begin` checks
remain authoritative; no reset, failure-latch clearing or provider/grant
reinstallation is introduced.

`target/native-reflected-derived-control-native-evidence/after-v1.json`
(SHA-256 `f9cfe12b7a2013fd63ac0e73672a1c4fbee617e01a366e5747a1ab17a1e6da24`)
records actual session **18249**, terminal **e1bcf6**, all five commands exiting
zero with **one real test each**, and all **3,725 frozen inputs / nine runtime
files unchanged** in the tested candidate based on parent `f4c63a0f`. It uses
the unchanged measured MSVC Debug/Release receipt pin
`96721717326f7d6f96dd2bb98dc1d6e919f7119235e33e103e5ac89bd6f29d0a`.
The earlier original Source06 nine-script, four READY, ordinary-control and
191/382 receipts remain separate evidence. No GNU carrier/fresh-v3 native
execution acceptance follows from this local MSVC gate.

## Dedicated reflected body and owned Resource Return acceptance

Sources10/11 are separately named additive controls; the original Sources7/8/9,
historical ten scripts and scalar-Default READY baseline remain byte-identical.
Twelve new shared cases execute genuine first- and second-field successful
Returns, handled first/later borrow failures and four cleanup combinations in
both checking profiles. Each Source retains three equal-int64 reflected fields,
its statements after the Return condition and its statements after the loop.
Finite provider scripts and terminal sentinels verify that Return skips those
statements and all later fields.

Source10 returns `nothing` from the selected scoped-type body while an outer
optional Resource owner remains live. Its exact borrowed projection ends and the
outer owner retires during Scope completion. Both cleanup cases retain original
body **0**, cleanup **255**, Cleanup channel, empty ordinary message, exact
`native Resource cleanup failed` and first process exit **71**. This differs from
the historical derived explicit-close cases' body255/cleanup255.

Source11's helper returns an actual distinct, unborrowed Resource owner892 while
aliasing outer owner891. Successful body retirement and outer cleanup precede
publication; the caller's exact Borrow892(29) and consuming close establish the
returned owner's identity and custody. Its ordered events place Finalized891
before caller Borrowed892. Both helper cleanup cases suppress publication and
caller borrowing, retire891 then the provisional892, and retain root body **255**,
cleanup **255**, Cleanup channel and exit **71**. The helper Source follows a
normal Return path before cleanup failure propagates. The directly observed
tuple belongs to the entry root; the report does not separately read a nested
callee completion. Clean success and
handled failures remain body0/cleanup0 with empty messages and exit0.

Actual session **64018**, terminal **0b2a26**, passes two one-test reference groups
and one final object group: **24 single-entry reference calls + 48 same-grant
reference calls = 72**, and **24 objects**. Actual session **47358**, terminal
**dcfb82**, passes both one-test linked groups: **24 Source-deleted single-entry
executables / 24 entries** plus **24 same-grant reentry executables / 48 entries**,
for **48 native executables / 72 entries**. Every first script is followed by its
same byte-identical Source's READY on the same Session/provider/paired Network
grant. Increasing nonzero attempt identities, immutable first completion/messages
and first nonzero exit, continuous events/entry boundaries, all eight zero native
Resource counts and empty ordinary storage before teardown, combined-script
exhaustion and one clean destruction all pass. No reset, latch clearing or grant
replacement is introduced.

`target/native-reflected-body-return-native-evidence/after-v1.json` records the
reference/object gate. The linked receipt is
`target/native-reflected-body-return-native-evidence/after-v2.json`
(SHA-256 `9250a5d37375cd66ba212e1cfcaa489ef0b55571dba0efa72dc59a6d306baa18`).
Both gates retain all **3,730 frozen inputs / nine measured v2 runtime files**
unchanged in the tested candidate based on parent
`91105876e5dbbbe9bbaa1d19bc2408ec9a274255`. The unchanged measured MSVC
Debug/Release receipt pin is
`96721717326f7d6f96dd2bb98dc1d6e919f7119235e33e103e5ac89bd6f29d0a`.
Earlier original/derived/191-case receipts retain their counts and archive pins.
This is bounded local MSVC Return acceptance; it adds no fresh-v3, GNU carrier,
coherent-workspace or exact-revision CI acceptance.

## Remaining native and reflection work

The producer breadth is deliberately limited to the exact direct nominal-struct
case in the checked Resource program. Generic and alias owners, bitfield field
families, reflected variants, machines, `TypeInfo.args`, nested reflected
producers and broader function compositions remain compiler implementation and
Source-evidence obligations in the checked Resource path. The ordinary controls
above establish their own bounded parity and leave ordinary `lower_program`
unchanged. Original Source06 exceptional same-grant reentry is accepted above.
The derived historical control matrix and dedicated body/owned Resource Return
controls are accepted above. Same-owner Return while its resident lease remains
live, broader body/Default lifetimes and broad aggregate Resource custody remain
separate obligations. Return operands still precede certified lexical retirement;
the accepted owned-return control uses a distinct unborrowed survivor. This
boundary adds no
compiler-policy ban on existing Source semantics.

The full native-codegen goal remains active at the rough 87% planning estimate.
The fixed 207-fixture / 182-run-pass inventory denominators do not change.
