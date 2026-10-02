# Native local view aliases

Local aliases of views are an admitted source form with a bounded native
implementation. This note records its handoff and remaining limits; it adds no
ownership policy or claim of complete native coverage.

## Settled behavior and current gap

[Rule Set 24](../design.md#rule-set-24-read-only-views-solving-the-memory-borrowing-problem)
makes views read-only borrows: they can be read, passed as views, and explicitly
cloned into ownership. They cannot be consumed or escape their scope. For example:

```jett
function duplicate(view source: list[int64]) returns list[int64]:
    list[int64] borrowed = view source
    list[int64] forwarded = borrowed
    return clone forwarded
```

Previously the checker tracked these declarations by resolved identity without
transporting their borrow mode into HIR. MIR analyzed `Let` as an owning
expression and gave ordinary aggregate locals owning drop slots. Allowing the
initializer alone would transfer or drop a borrowed handle incorrectly.
Implicitly copyable results, explicit clones, and ordinary owned field copies
retain their existing behavior.

## Immutable local alias domain

The implementation admits direct aliases and forwarded aliases rooted in an
immutable owner or immutable view parameter, used for borrowed reads, view
calls, and explicit clones.
Preserve the checked type, including secret and refinement identity. Do not infer
view-type semantics from named aliases or turn arbitrary call results into views.

Carrier-preserving `coarsen` and `declassify` bindings retain their checked backing
chain through MIR lowering. Only a destination with a valid borrowed origin and
a validated transparent initializer takes this path. Ordinary owning expressions
keep their existing snapshots; an explicit `clone`, handler, or allocating
conversion cannot be reinterpreted as an alias initializer.

1. Export typed binding facts through ordinary, generic, and reflected comptime
   body handoffs. Facts must belong to the concrete instantiation: the same
   generic source span can describe a scalar copy or an aggregate borrow.
2. Represent borrowed local bindings and their source owners explicitly in HIR
   and MIR. Forwarded aliases retain the original owner dependency. Preserve
   these facts through handler extraction, generated functions, and ID remapping.
3. Carry persistent loan dependencies across statements and CFG edges. Extend
   owner liveness for alias reads, validate owner availability, and preserve
   borrowing in matches, sum handling, iteration, and debug observations. A view
   binding must never become an owning move merely because its name is local.
4. Give borrowed handles non-owning native storage. Only actual owners enter
   drop planning; normal exits, branches, loops, and terminal failures must clean
   them exactly once after dependent reads. Existing borrowed-handle operations
   may suffice, but runtime requirements must be verified rather than assumed.

Validate alias chains, lexical shadowing, generic specializations, nested control
flow, pending values, clone independence, debug reads, and failure cleanup with
linked reference/native comparisons and ownership-plan regressions.

MIR retains a may-created alias set across CFG joins. Once a reachable path has
created an alias, consuming or rebinding its root produces an explicit native
implementation error, including after the last alias read. This conservative
boundary avoids choosing lexical or last-use loan expiry. Alias reads keep
their immediate origins and ultimate owner live; aliases never receive owning
drop slots. Function descriptor aliases follow the same rule even though native
function storage otherwise permits copied descriptors.

Handler staging follows exact formal view modes for direct and indirect calls.
It snapshots a borrowed argument before a later handler and observes a callback
after its arguments. Qualified string and descriptor snapshots acquire their
own retained or cloned handles. Ordinary copy-owned alias expressions also
acquire ownership before entering a list, sum, or record, while borrowed
arguments keep raw handles. Plain string view expressions preserve existing
copy semantics in owning contexts.

## Stable immutable ordinary-struct field paths

The [typed stable-field contract](native_stable_projected_local_views.md) records
this bounded extension before compiler integration. Rule Set 24 already permits
readonly field views, forwarded aliases, and explicit clones. A field of an
immutable owner is a stable selected payload even though the payload type differs
from the whole owner's type; incidental reference cloning is not its ownership
contract. This extension transports and validates the existing borrow rather
than selecting a new lifetime rule.

An immutable local or immutable view parameter may back a finite field path.
The existing immediate binding dependency remains the source of owner liveness;
the initializer's typed `Field` nodes prove the path. Each step checks the exact
ordinary-struct owner, canonical field index, declared intermediate/endpoint
type, and terminating source-local identity. Forwarding from a projected alias
keeps that immediate alias dependency and its original owner chain. The checked
endpoint retains its type, including secret/refinement qualifications and
existing interface erasure. Bytes and lists are concrete witnesses, not an
endpoint allowlist; other endpoint operations retain their existing admission.

Only borrowed field access initializes this non-owning storage. Ordinary field
copies and explicit clones remain independently owned. No implicit clone, new
runtime symbol, hidden temporary owner, or duplicate path metadata is needed.
The whole owner stays initialized/live for all dependent aliases, including
across CFG edges, while aliases receive no owning cleanup slot. Existing field
access preserves pending checks on every ancestor and endpoint. Transparent
`coarsen`/`declassify` bindings retain the existing carrier/erasure restrictions.

Removing the old whole-owner/endpoint equality check requires typed validation
of every alias initializer before optimization or compaction, including unused
locals and unreachable blocks. Source facts must belong to their concrete
ordinary, generic, reflected, or generated body; copied origins from another
function cannot become borrowed captures. Invalid fields, types, unavailable
roots, or broken local chains remain handoff failures.

The initial source package has six successful candidates/controls, one terminal
argument-failure cleanup case, and two frontend-valid boundaries. Its reference
records are `target/native-stable-projected-view-preflight/results.json`; the
five projected candidates failed native HIR before this repair, independently
of the two already supported ownership controls.

An additional frontend/reference-checked fixture covers a borrowed struct field
followed by its own bytes field, scalar versus move-only `Carrier[T]` bodies,
and explicit `declassify`/`coarsen` operations on secret/refinement field aliases.
Its exact three-line source oracle passes in
`target/native-stable-projected-view-driver-draft/10_typed_endpoints.preflight.json`.
After integration and nominal-proof review, all 12 local-alias driver tests pass
(366 filtered, 11.78 seconds) in
`target/native-stable-projected-view-final-linked.log`. This includes eight
linked cases with matching debug/release runtime archives and source removal,
the mutable/temporary publication-sentinel group, and the three existing broad
alias tests. Successful clones retain independent ownership; later argument
failure preserves exact output, status 71, and cleanup.

All 896 compiler-phase library tests pass in
`target/native-stable-projected-view-final-phases.log`: 41 codegen, 416 comptime,
131 HIR, 86 MIR, and 222 typechecker tests. The nominal guard additionally rejects
invented or discarded refinement proofs while retaining explicit existing
ancestors, exact secret declassification, and outer secret promotion. An initial
negative-test lookup used the exported definition map for an unused type; the
corrected test reads its actual interned type and passes in the final batch.

The final driver/frontend batch passes all 83 driver library tests (79.46
seconds) and all 598 frontend fixtures (22.80 seconds) in
`target/native-stable-projected-view-final-driver-frontend.log`. The diagnostic
capture regression previously used a now-supported stable field alias as its
HIR failure trigger; it now uses the already excluded mutable-root case while
preserving exact comptime/verify observations and diagnostic transport.
The Linux-only projection-loan regression now tests real owner consumption
after alias creation instead of rejecting the alias declaration itself. Its
Windows target discovers zero tests, so Linux CI still must execute that gate;
the same source has an actual Windows native-object refusal in
`target/native-stable-projected-view-owner-conflict.json`.

The ordinary-struct checkpoint registered 378 native tests; its subsequent
frozen a0bf12df workspace passed all local gates. Focused results and the
later workspace acceptance remain separate evidence. Before the exact-state
follow-up below, a machine-field alias was frontend/reference-valid but
failed the struct-only proof, as characterized in
`target/native-next-after-projected-view/root_machine_field_alias.preflight.json`.

## Exact state-qualified machine field follow-up

Status: integrated; focused compiler and linked gates pass. New-source full
workspace and supported-host acceptance remain pending. Rule Set 24 and the
existing exact-state payload rule already permit the read-only source form.

The initializer's existing Field proof now accepts a declaration-backed
MachineState owner. It resolves the exact machine/state and payload fields,
then checks the zero-based FieldId, exact endpoint, base-owner type and actual
terminating local identity/type. Struct-to-machine and machine-to-struct paths
compose through the same finite tree. Every initializer, including unused ones,
still requires proof before compaction. Nominal refinement, outer-secret
promotion, exact Coarsen ancestors and direct-Secret Declassify remain unchanged.

MIR reuses persistent whole-owner loans, liveness, non-owning storage and cleanup.
Native emission keeps the existing machine tag-slot offset outside canonical
FieldId metadata and checks pending owners at each field access. Borrowed linear
reads retain their handle; owning copies and explicit clones acquire independent
ownership. No ABI, hidden owner, implicit clone/join or lifetime policy changes.

The initial nine-case characterization in
`target/native-machine-projected-view-preflight/results.json` recorded 18
successful frontend checks, eight reference successes and one terminal failure.
Six alias cases and two owner-change probes had 16 native refusals at the old
struct-only proof; the owning-copy control was not native-built in that batch.

After integration, three focused new HIR tests pass. The current library gate
passes 261 tests: 41 codegen, 134 HIR and 86 MIR in
`target/native-machine-projected-view-phases.log`. It includes exact malformed
unused state/index/endpoint/root, bare/foreign owner and orphan controls, plus
concrete generic contexts. This is not a new complete workspace/library claim.
All 83 driver library tests pass (66.89 seconds) in
`target/native-machine-projected-view-driver-frontend.log`. The corrected
`fixture_suite` invocation passes all 598 frontend fixtures (22.15 seconds)
in `target/native-machine-projected-view-frontend.log`; the earlier command
used the nonexistent `fixtures` target and executed no frontend tests.

The expanded local-alias gate passes 25 tests (366 filtered, 17.72 seconds) in
`target/native-machine-projected-view-pending-linked.log`. It includes the
existing ordinary-struct controls, seven exact-state cases and the consume/
transition publication-sentinel group, plus five pending-machine cases.
Those pending cases preserve whole-owner and intermediate-owner field errors,
depth-two endpoint/owner aliases, explicit clone/two-join copies and unchanged
originals. Ordered runtime Trace events, application stdout, stderr and status
0/71 match in both runtime profiles after source removal; release omits traces
while retaining terminal errors. Earlier pending frontend/reference preflight
is recorded separately in `target/native-machine-projected-view-pending-draft/results.json`.

The typed-endpoint case passes separately (one test, 391 filtered, 2.80 seconds)
in `target/native-machine-projected-view-typed-linked.log`, using matching
archives and source removal with exact application stdout and empty debug
events. `Envelope[int64]`, `Envelope[bytes]` and `Envelope[list[int64]]`
instances retain scalar-copy versus aggregate-borrow facts. Exact machine fields
retain `NarrowValues`/`Numbers` and `secret[list[int64]]` identities through
explicit borrowed Coarsen/Declassify;
grown clones leave reread originals unchanged. Its source preflight is
`target/native-machine-projected-view-typed-endpoints-draft/preflight.json`.
An initial handler around an already-proven Packet constructor reported E0308;
removing that invalid handler repaired only the fixture, without compiler changes.

These are 26 distinct executed local-alias tests across the 25-test batch and
the separate typed-endpoint pass, not one executed 26-test batch. The supplemental
native corpus registers 392 tests. All 15 machine Jett source-format checks and
Rust formatting checks pass. Broader endpoint/owner combinations retain their
independent obligations; the examples do not impose an endpoint allowlist.

Bare flow-narrowed machine origins, mutable backing chains, temporary roots,
projected writes, allocating conversions and owner consume/rebind/transition
after reachable alias creation remain outside this bounded native support.
The actual owner-change group reaches the existing typed ownership refusal
before archive lookup and preserves the output sentinel in both profiles.
No source loan-expiry or temporary-lifetime contract is selected.

The accepted a0bf12df frozen workspace and all four supported-host jobs are
centralized in
[the acceptance audit](native_acceptance_audit.md#exact-qualified-machine-field-local-view-focused-acceptance).
The `774b13e7` complete frozen workspace now passes; its supported-host workflow
remains in progress. The audit records exact final logs and counts. This local
acceptance does not validate later inferred JSON changes or establish full-language parity. The about-85%
planning estimate and fixed 207-fixture denominator remain unchanged.


## Bitfield field follow-up

The exact declared Bitfield owner now participates in the existing finite Field
initializer proof. Canonical zero-based identity and declared field type are
checked alongside the unchanged source, owner and endpoint proof. Struct-to-
bitfield paths, whole-bitfield aliases, view parameters and forwarding retain
the same immutable backing chain. Pending field checks, numeric copies, owned
field copies, explicit clones and non-owning alias cleanup remain unchanged.

Both focused HIR tests pass, including unused forged owner/index/endpoint/root
and orphan metadata. All seven linked cases pass together (3.89 seconds, 425
filtered), executing matching debug/release profiles after source removal.
These include exact no-LF output, owner rereads, empty payloads, whole pending
owner failure and five ordered payload-copy traces. The current corpus registers
432 tests. The crossed compiler gate passes 951 library tests, followed by all
83 driver library tests, 598 frontend fixtures and seven source-format checks.
New full-workspace and supported-host acceptance remain pending.

The accepted 08f9f7e7 parser predecessor and the bounded bitfield evidence are
recorded in [the acceptance audit](native_acceptance_audit.md#bitfield-field-local-views-focused-acceptance)
and [the bitfield contract](native_bitfield_projected_local_views.md). This
repair selects no mutable or temporary view, owner-change, capture-escape,
projected-write or loan-expiry policy and makes no claim for later repairs.


## Direct secret aggregate owner follow-up

The same typed initializer proof now retains one direct Secret layer on an
exact declared Struct, Bitfield or MachineState receiver. Nothing and an
already-secret nominal field keep their declared type; every other endpoint
gains exactly one Secret layer. Field identity/type, checked root, immutable
loans, non-owning slots and pending access remain unchanged. Explicit borrowed
declassification/coarsening and owned clones preserve their established roles.
No hidden-secret debugger or general lifetime policy is selected.

The three focused HIR tests pass (136 filtered), followed by all 954 crossed
compiler library tests. All seven linked cases pass in 2.82 seconds: six pin
full successful outcome parity, while the pending receiver separately pins its
raw reference message and existing native redaction with matching partial stdout
and status 71. This follows the already recorded open observation boundary;
it does not claim seven diagnostic-parity cases. All 83 driver library tests,
598 frontend fixtures and seven source-format checks pass. New
439-test workspace/platform gates remain pending.
Exact captures, selected proof and current evidence are in
[the secret-owner contract](native_secret_owner_projected_local_views.md).
The independent projected interface/handler repair remains unapplied.


## Boundaries requiring a contract

The existing [projected assignment note](../open_design/projected_field_assignment.md)
states that declaration facts are not complete borrow-provenance analysis and
that mutation with outstanding views needs a selected contract. Keep these
related questions explicit before extending the first domain:

- Rebinding or consuming an owner while an alias exists, including whether loan
  expiry for source validity is lexical or based on last use.
- Rebinding a mutable alias itself; current declaration facts remain read-only
  and do not define a general lifetime transition.
- Views of temporaries, projected temporaries, or allocating conversions. If
  lifetime extension is selected, they need a hidden owner evaluated once and
  kept alive for dependent aliases, not an implicit clone of every view.

Some such forms are already admitted by the frontend. Acceptance and incidental
interpreter cloning do not settle their ownership contract. Keep these forms
visible as unimplemented native cases; do not silently redefine them, implement
projected mutation, or count the first domain as complete view support.
