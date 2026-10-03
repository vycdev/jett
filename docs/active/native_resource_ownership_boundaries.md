# Static opaque Resource ownership boundaries

Status: the selected checker guards pass 36 focused groups, including 21 new
source groups, and the broader compiler/object/driver/frontend/lowering gates.
All 34 frozen source witnesses are rechecked in debug and release with stable
inputs. These are static prerequisites for live Resources. No runtime carrier,
provider installation, source-scope cleanup or native Resource operation is
introduced. Exact clean `b7180bd9` subsequently passes the full workspace; its
supported-host run remains in progress. The accepted local and platform status are in the
[acceptance audit](native_acceptance_audit.md#resource-static-ownership-boundaries).
The broad estimate remains about 85%; 207 inventory fixtures, 182 object
obligations, 464 supplemental cases and the whole-language 100% goal are unchanged.

## Selected source contract

The [opaque resource contract](../completed/opaque_runtime_resource_contract.md)
requires one owner and one cleanup obligation. Resource views cannot acquire
ownership by copying or be hidden in an owned payload. This does not ban owning
aggregates containing Resource data, safe nonowning field inspection or type-only
metadata. Production hook catalogs remain empty. The pre-Rust boundaries and
subsequent narrow diagnostic amendment are captured in:

- `target/native-resource-copy-guards-draft/contract.md` and `getter-owner-note.md`.
- `target/native-resource-view-payload-guards-draft/contract.md`.
- Its additive `map-machine-contract.md` and `diagnostic-dedup-contract.md`.
- `target/native-resource-source-guards-root-diagnostic-amendment.md`.

## Implemented static boundaries

| Operation | Checked boundary and preserved behavior |
| --- | --- |
| Plain field read used as an owning value | E0364 if the selected returned endpoint recursively stores Resource data. Explicit field views and unrelated copyable fields remain legal, including in Resource-bearing owners. |
| Clone and cloning collection/reflection getter | Existing recursive no-clone rule also covers `list.__get_clone` and owned field-value getters. Cloning a parent/endpoint cannot acquire another Resource owner. Consuming `map.get` is not converted into a clone or blanket banned. |
| Owning Some/Ok/Fail, list, map value or struct payload | E0401 for an explicit or otherwise known Resource-bearing view, in inferred and contextual checking. Actual owned transfers, None, empty containers, non-Resource views and phantom generic arguments without stored Resource data retain current admission. |
| Machine constructor or transition target payload | The same typed child acquisition guard applies. Real owned target data, empty states and ordinary type/arity checking remain intact. Enum written/aliased views retain their existing E0375/E0401 checks. |
| Machine transition source | Only a valid exact selected state whose data contains Resource invokes the existing owned-source gate: E0375 for a written view and E0401 for a known viewed alias. Actual owned sources and Resource-free selected states retain current behavior. |
| Exact Resource-root value reflection | E0300 for variant/state value reflection and all three owned field-value getters. Type-only Resource metadata and aggregate variant/state metadata remain available. |
| Exact Resource-root print/println | E0300 with ordinary-print wording. Release mode also retains E0362. Nested printing and hidden-Secret, known-absent or omitted rendering remain unchanged and unresolved. |

The recursive data predicate follows checked concrete fields and payloads,
including the exact selected MachineState. Unused nominal generic arguments do
not create stored Resource data. This is not a generic contains-Resource ban.

Owning versus borrowed field projection is explicit checker context. Only the
existing place path and transparent wrappers carry borrowed projection context;
a surrounding view does not authorize borrowed data inside a constructor.
Calls and owning payloads perform their normal child checks. Known-view facts
come from explicit View and existing viewed bindings/aliases, with the existing
parenthesis/coarsen/declassify handling; no general call-result borrow provenance
or view-expiry rule is introduced.

The payload diagnostic is emitted once per diagnosed source span, after typed
Resource-data and known-view checks. The guard records only that occurrence's
exact identifier legacy ownership counterpart. Existing ownership traversal
continues and retains unrelated E0400 and unhandled-value diagnostics. This
prevents duplicate E0401 without suppressing other errors or source checking.

## Executed source and compiler evidence

The first focused run passes 33 of 36 groups. Its three failures concern duplicate
inferred-map payload diagnostics and test sources that also require ordinary
unhandled-value diagnostics. The diagnostic-only/source-test correction above
preserves those policies. The initial result is retained in
`target/native-resource-source-guards-focused-initial.json`; it is not acceptance.
The earlier in-progress composition hash guard also refused to compose before a
final manifest existed; `target/native-resource-source-guards-composition-guard-failure.md`
records that no tracked mutation occurred.

The final focused run passes **36 groups**, including **21 new source groups**,
with all three source hashes unchanged in
`target/native-resource-source-guards-focused-after-diagnostic.json` and its log.
Both profiles cover copied fields/getters, inferred/contextual payloads,
forwarded and generic viewed aliases, bare-view diagnostic deduplication,
independent errors, exact-state transitions, root observation and preserved
owned/empty/non-Resource controls.

The frozen 20-source probe and additional 14-source payload probe all parse and
resolve under synthetic `SourceOrigin::Stdlib`, reserved FileId 10000 and an empty
resource-hook catalog. Their debug/release checks produce these actual results:

| Frozen source family | Observed outcome in both checker profiles |
| --- | --- |
| Original 04 through 06 and 08: field/reflected/getter copies | E0364. |
| Original 09 through 11: borrowed optional/list/struct payloads | E0401. |
| Additional 01 through 03: contextual/inferred/aliased map values | E0401. |
| Additional 06 through 08: machine constructor/transition target payloads | E0401. |
| Additional 09: viewed Resource-bearing transition source | E0375. |
| Original 07: root variant/state value reflection | E0300 for each operation. |
| Original 02: direct/root ordinary printing | E0300 per call; release additionally retains E0362. |
| Original 01 and 14 through 16/20; additional 10 through 14 | Resource tags, true owned/view transfers, type-only/absence and checker-stage comptime/property controls pass. No value evaluation or sample generation is claimed. |
| Original 12/13/17 through 19; additional 04/05 | Existing recursive-clone, view-return, interpolation/declassify, construction, JSON and enum-view refusals remain. |
| Original 03: nested printing | Still admitted in debug; release rejects printing with E0362. This open boundary is not hidden by the root guard. |

Exact reports are `target/native-resource-source-guards-original-witnesses-report.json`
and `target/native-resource-payload-audit-616b51b9-v2/report-after-source-guards.json`.
Their proof wrappers retain stable fixture/compiler inputs. All 34 source checks
are characterization only: no interpreter evaluation, comptime execution,
property generation, Resource provider or native Resource execution occurs.
The initial payload audit's malformed inferred-map source is retained separately;
the corrected frozen v2 source is the one checked above.

The broader gate passes **1058 library units** (63 codegen, 417 comptime, 139 HIR,
99 MIR, 54 resolver, 254 typechecker and 32 types), **63 codegen object integration
tests** (1.11 seconds), **83 driver tests** (69.79 seconds), **598 frontend fixtures**
(22.13 seconds) and **eight backend-lowering tests**. All three source hashes
remain unchanged. `target/native-resource-source-guards-phases.json` exits zero
at `2026-10-03T12:28:58.9045108Z` and names each exact gate log. This affected-phase
acceptance is distinct from a new complete frozen workspace and supported-host
run, both still pending.

## Required work before live providers

These guards do not implement a Value/interpreter Resource carrier, runtime hook
installation, HIR/MIR cleanup ownership or native ABI/drop support. Deterministic
fake-provider execution must prove actual moves, scopes, temporaries, failed
arguments, aggregate arms, error paths and exactly-once finalization before real
provider operations can be exposed. Consuming collection extraction must transfer
an owner and clean remaining elements rather than duplicate an internal key.

Other acquisition/escape paths and general call-produced borrow provenance still
need audit. Nested ordinary Resource printing requires an observation boundary
for occupied data, hidden Secret, known absence and omitted values; this slice
does not select that policy. Trace/breakpoint type-and-ownership-availability
summaries are distinct from ordinary printing and expose no registry key, slot,
generation or provider state. Opaque layouts and source/native live Resource
values remain unsupported. Actor/message transfer, pending work, cancellation,
late completion and real provider authority remain independent lifecycle work.
