# Checked reflected-field execution

The checked reference interpreter now enters the exact original body selected
by a direct `for field in type.fields[T]()` loop and its
`comptime type Element = field.type_info` binding. This repairs existing checked
Source execution; it adds no language syntax, provider or runtime capability.
Native reflected Resource lowering remains a separate pending obligation.

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

All 541 `jett_comptime` tests pass. The candidate's 3,664 frozen inputs and
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

## Remaining native and reflection work

Reference success does not establish native emission, linking or Source-deleted
execution. Native HIR currently coalesces equal field type/reflection bodies;
MIR scope association and final dispatch must retain an authenticated
original-loop/ordinal/body mapping. Removing the duplicate-identity refusal
alone would choose the wrong equal-type field body.

The direct-field proof does not complete reflected variant/state/TypeInfo.args
loops, every generic function composition, broader aggregate custody or other
pending Resource lifetimes. These remain implementation obligations for
existing Source semantics. No compiler-policy ban follows from this bounded
repair, and the full native-codegen goal remains active.
