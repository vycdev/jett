# Checked reflected-field execution and native ordinal transport

The checked reference interpreter now enters the exact original body selected
by a direct `for field in type.fields[T]()` loop and its
`comptime type Element = field.type_info` binding. This repairs existing checked
Source execution; it adds no language syntax, provider or runtime capability.
The checked Resource lowering path now retains a direct nominal-struct field
ordinal bridge through HIR and MIR. Four focused Source cases pass 16 linked
native executions and 24 entry attempts; the unchanged 191-case corpus also
passes all 382 Source-deleted Debug/Release executions.

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
has the exact outcome, ordered events, exhausted script and zero ordinary and
Resource obligations before teardown; the Session is destroyed exactly once.
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

## Remaining native and reflection work

The producer breadth is deliberately limited to the exact direct nominal-struct
case in the checked Resource program. Generic and alias owners, bitfield field
families, reflected variants, machines, `TypeInfo.args`, nested reflected
producers and broader function compositions remain compiler implementation and
Source-evidence obligations in the checked Resource path. The ordinary controls
above establish their own bounded parity and leave ordinary `lower_program`
unchanged. Exceptional reflected native exits/reentry and broad aggregate
Resource custody remain separate acceptance obligations. This boundary adds no
compiler-policy ban on existing Source semantics.

The full native-codegen goal remains active at the rough 87% planning estimate.
The fixed 207-fixture / 182-run-pass inventory denominators do not change.
