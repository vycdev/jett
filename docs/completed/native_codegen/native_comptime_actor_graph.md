# Native materialization of comptime actor graph templates

Status: implemented in the current combined compiler. Focused local checks and
32 linked MSVC executions passed; broader completion gates remain pending.

Pure closed required expressions freeze their reachable actors, typed state and
admitted captures at successful completion. The baked cache holds an immutable
template. Every runtime evaluation of the expression imports a fresh graph;
call and loop activations do not share mutable template state. Once a result is
bound, ordinary reads and aliases retain the same live instances. A graph-wide
memo preserves sharing and keeps distinct seed nodes distinct.

Export/import covers actors inside admitted typed/refined/interface wrappers,
aggregate values, pending carriers, builders and callback captures. Checked
nominal identities and existing pending behavior remain intact. A type-aware
work queue and visited-node memo avoid treating nested actor IDs as independent
scalars. Import does not execute initializers, handlers, source predicates, the
original expression or compile-time debug events. Runtime capabilities and
Resource custody cannot be cloned into a template. Resource hook descriptors
retain their separate private checked association and gain no authority from a
graph packet.

Current raw native allocation remains confined to the exact checked actor
constructor. The snapshot path carries a privately derived reconstruction plan;
it cannot admit arbitrary raw ActorSpawn allocation. A two-phase unpublished
import reserves fresh identities, populates checked fields, validates completion
and then publishes the root. Any failure retires all partial owners and
registrations. Existing context-local liveness and global nonreused native actor
identities remain unchanged.

Existing Source admission remains unchanged. Actor closure capture is move-only
and rejected with E0402. The saved rejection control remains required; the graph
contract covers admitted captures. Unchanged primitive closure captures retain
their original typed context and capture witnesses. Reflected callable values
must retain their checked plan and authenticated source through reconstruction
and lowering. Neither copying an obsolete witness after changing its captures
nor clearing/resetting a witness can authorize a graph import. This integration
introduces no Actor-capture resealing policy.

The current compiler derives HIR reconstruction from the checked program and
retains independent original/current MIR seals, native JA1 carrier schemas and
private reserve/fill/commit/abort staging. Owner preflight, Callback plans and
capture witnesses, Resource constructor/incoming-frame checks and occupied
carrier cleanup remain separately authenticated. Graph and callable witnesses
are renewed together against the same original/current program transition;
exact leaf association selects the adapted callback. Required-value helper
relocation preserves reflected-callable authentication and materialization.

The protected constructor producer mapping and ordinary storage planning,
including graph programs with no Resource kinds, are recorded in
[Graph constructor transport](native_graph_constructor_transport.md).

The existing generated Resource startup is reused without a parallel installer
or Runtime-purpose issuer. Graph hook metadata needs exactly one additional
normal export, `jett_rt_v1_resource_graph_descriptor_box`; the composed Resource
inventory is 56 production symbols and eight separate private hooks. This proof
provides no Scope, Network grant or live Resource owner. The normal provider
remains Disabled. Descriptor graph startup supplies no production I/O credit.

Protected collection iteration keeps graph storage tied to its original
producer. Canonical sequence preparation records only the exact generated
iterator owner initialized once from that iterable, separately from original
Source headers. Current validation checks its initializer, header, sequence
element/value selection and consuming behavior. Altering a same-typed local or
iterator operation grants no storage proof.

Generated Resource completion releases Resource companions before retiring
actor state and unfinished graph imports belonging to the exact Running
ordinary attempt. Retirement precedes emptiness checks and terminal sealing.
First body diagnostics survive; cleanup failure retains priority and prevents
clean reentry. Context destruction observes the already drained actor table.

## Current local validation

The current `cargo test --offline --locked -q --lib --no-fail-fast -p jett_hir -p jett_mir -p jett_codegen_cranelift -p jett_driver --target x86_64-pc-windows-msvc -- --test-threads=1` run passed all 793 tests across 4 result groups, with zero failures and ignored tests. This current command covers HIR, MIR, codegen and Driver. The separately pinned predecessor run contributes 1673 passing tests across 18 other package result groups. Those results remain historical evidence; the combined count does not establish a fresh full workspace library run. Complete workspace integration/native conformance, exact-revision supported-host CI, GNU and installed distribution remain open.

The current interface compatibility selection passed 25 native tests with unchanged compiler and artifact guards: 21 original interface-prefix tests and four generic, deep-conversion and reflected-interface controls. This selection is a subset of normal native conformance. Earlier ordinary callback, Owner and Actor regression results retain their predecessor revision and are not counted as current compatibility acceptance.

The current sixteen original private Resource gates passed 754 Source-deleted native executions and 900 observed entries, using the fresh compiled receipt pin and unchanged compiler/source guards. The bounded startup matrix passed all 80 selected executions with Source absent, unchanged inputs and no retained case executables.

The preceding compiler passed MIR 344, graph metadata 13, focused constant 23 and
the exact latent-callback gate with two Source-deleted Debug/Release executions.
Its broad21/2119 and separate MIR 344 runs covered 2463 workspace library tests,
with no workspace integration credit. Runtime 361, HIR 218, Python 74, map object
emission and constructor controls are earlier local observations. The ordinary
required-replacement and wrapper-sequence repairs passed their focused controls
and the current linked acceptance recorded below. No speedup ratio
or newly verified builder domain is claimed.

Normal MSVC Runtime and Launcher archives were measured in Debug and Release
with 56 production Resource exports, static CRT and no private exports. The
receipt retains actual ordered normal native-library notes. The canonical
launcher-manifest vector differs in the order of its first two libraries; both
vectors are retained separately, and linked acceptance must establish the
selected manifest vector.

Fresh normal MSVC linked acceptance passed 32 Source-deleted executions: 12
public and four protected hook executions with each Debug/Release Runtime
archive. Public coverage comprises nine original ordinary fixtures, an adapted
callback and a production Stdout callback. Original and adapted debug cases used
Debug Source; the production callback and both hook Sources covered both Jett
optimization choices. Exact unique rows, independent reference/stream oracles,
successful terminal/reap/timeout records, absent Source and object hashes passed
with unchanged compiler and used-launcher guards. Each case executable was
removed after reaping. Actual compiler native-library notes and the canonical
manifest vector remain separate records; the linked executions establish the
selected manifest vector. Admission/map collectors provide no acceptance credit.

The older failures of original hook Sources07/08 occurred before graph
construction in a compiler lacking generated Resource startup. They remain
historical observations, not current Source rejection oracles. The original
aggregate success case and body/header/pointer/resealed-parent forgery controls
remain required.

Integration/native conformance, exact-revision supported-host CI, GNU and
installed distribution remain open. Wider original probes,
post-comptime state mutations, identity collisions, repeated activation,
sharing/distinct nodes, admitted nested carriers/captures, nominal dispatch,
failure cleanup and no replay remain obligations wherever current evidence has
not established them. Release Source print rejection remains separate from the
Debug/Release runtime archive profiles. The rough 87% estimate and fixed
207-fixture/182-object inventory are unchanged; this slice does not establish the
full 100% native-codegen goal.

This implementation covers the existing sequential evaluator and supplies no
asynchronous scheduling or production-provider credit. Existing run/join/cancel,
actor/message ownership and other implemented reference behavior remain native
parity obligations. Adding the absent asynchronous scheduler or production
provider APIs is separate future feature work, not a prerequisite for parity
with the existing interpreter.

## Original graph analysis batching

`OriginalGraphAnalysis` reconstructs all original owner rows once for an exact
immutable checked execution archive. Its private constructor validates archived
type meanings and dense function IDs, rebuilds factories and accounts for unknown
call/message edges with the existing bounded fixed point and exact row comparison.
Non-convergence produces no admission rows. Each selection checks exact archive
identity, archived types and the complete original owner; equal Source text or a
TypeId cannot substitute for those checks.

The Resource execution closure retains this private immutable table and shares
it with constructor capture and typed graph validation. Reconstructed closures
reuse the same table while independently recomputing their function and handler
sets. Closure equality also checks the table's exact retained archive identity.
Current MIR body/header, constructor, root shape, transport and pointer-membership
checks remain mandatory. The table grants no runtime Scope or callable authority
and is never stored in a global or cross-session cache.

A completed analysis also records whether it contains any original graph rows.
Only that private result for the exact archive, together with empty current
metadata, permits a negative storage answer without repeating graph-specific
validation. It never yields a storage proof or removes a custody function.
Original archive/type authentication and independent closure reconstruction remain
mandatory; positive or malformed current records retain full validation.

## Required observation capture

Ordinary retained checked programs prepare namespace constant initializers through
the same original-program and purpose checks as collected explicit expressions.
Both passes select the same private checked attempt key, so successful and failed
initializers are evaluated once and retain their actual observations once. Baked
runtime reads stay silent. This repair adds no runtime initializer, global storage,
Resource transport, provider or custody and changes no constant admission rule.

## Authenticated ordinary required replacements

Every retained checked ordinary archive now carries required replacements through
the private authenticated transition, even when no graph or reflected callable
plan selected the owner. The original archive is checked before replacement;
empty Resource kinds, the unique original owner/context proof, typed Source site,
unchanged original locals and exact generated bindings remain required. Canonical
graph/callable conversion renews the existing seals at that fixed boundary. Raw
baking cannot silently change a retained actor archive.

For an admitted `TypeConstruction` value, the matcher uses the sole existing
evaluated-builder producer and compares its complete canonical typed transcript,
including schema/owner, payload, field order, pending layers and failure operations.
This path requires zero added locals and bindings. It introduces no broader
builder-domain admission or runtime fallback. The two focused HIR controls passed primitive replacement, foreign/pre/post-edit
refusal and altered builder-transcript checks. The three original interface
native regression tests passed with Source present and unchanged inputs; these
results provide no Source-deleted execution credit.

## Ordinary wrapper sequence continuity

An ordinary actor/interface wrapper may retain authenticated Source entry-Scope
metadata while owning no Resource execution records. Its collection loop must
still use the existing finite ordinary sequence transition. The wrapper route
requires exact original entry-Scope capture and excludes graph metadata; the
ordinary reflected-field route keeps its existing qualification.

The transition independently refuses execution, constructor/seal, call, region,
borrowed-sum and lexical-custody records. It checks the exact before/current
witness, canonical whole-body edit, original function/local headers and absence
of new custody. It may retain the authenticated wrapper but cannot install a new
Scope, provider, owner or proof. Dedicated graph sequence transport and actual
Resource execution routes remain unchanged. The MIR control passed the list[Named] Actor loop, five iterator/header/body/Source
alterations and real Resource-owner/direct-call predecessor refusals. The three
original interface native regression tests passed with Source present. Fresh
complete graph, Resource and startup proofs remain separate requirements.
