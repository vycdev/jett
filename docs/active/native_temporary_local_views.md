# Lexical backing for temporary local views

Status: selected language contract, 2026-10-15. A bounded ordinary function-body
implementation has MSVC acceptance on 2026-10-16. All
1,842 core-library tests, 603 frontend/reference fixtures and nine CLI capture
tests pass. The native selection passes 23 tests and 46 paired Source-deleted
executions. Physical cleanup measurements and broader-context acceptance remain
pending. The full selected contract below is unchanged; this implementation
slice does not narrow its remaining obligations.

## Selected rule and tradeoff

An admitted immutable source local initialized by a view of an ordinary owned
temporary retains the whole exact producer result in compiler-owned storage.
Evaluate the producer once at its original initializer position and retain that
owner until the declaration's original lexical scope exits. Immutable forwarding
creates nonowning aliases to the same backing; it does not duplicate the owner
or extend the originating scope. Even an unused alias or one read only early in
the scope keeps that backing until scope exit.

Scope-based freeing in [Rule Set 10](../design.md) and Rule 24's lexical escape
bound support this choice. Previously, the local-view note left both temporary
extension and last-use expiry unresolved. Lexical retention keeps cleanup local
and explicit without introducing last-use loan analysis. It may retain unused
owned storage longer than a last-use scheme. That is the selected tradeoff;
keeping only a projected field or cloning it would change the ownership model.

The whole owner retires exactly once on normal scope completion or an exit by
Return, Respond, Default, Break, Continue or terminal failure. Acquire any owned
exit result, including an explicit clone, before retiring backing it may read.
End dependent aliases before dropping the owner. Loop iterations create separate
backing activations; backedges retire that iteration's owners, and other exits
retire only crossed scopes. Unselected branches create no owner. Existing cleanup
ordering and primary-error/cleanup precedence remain authoritative.

No last-use expiry, implicit clone, runtime owner pin or fake Source DefId is
introduced. Copyable endpoints retain their ordinary copy behavior. Existing
escape and dependent-loan rules, typed field paths, pending checks and failure
order remain unchanged. Views gain no permission to return, capture, enter an
owning aggregate, cross threads or outlive their originating scope.

This local rule does not grant actor-state lifetime or Resource custody. Mutable
alias transitions, allocating conversions without an exact owned-producer proof,
borrowed-sum alternate backing and other current implementation limits remain
separate. It adds no Source ban to compensate for missing native support.

## Checked initializer and generated owner proof

`CheckedViewSource::Other` records an unstable expression origin; it is not an
ownership or lifetime certificate. Lowering must construct one private checked
association joining:

- The authentic program/body and concrete ordinary, generic or reflected
  context, original alias declaration, immutable binding type and lexical scope.
- The original written View and exact owned producer occurrence, whole-owner
  type, checked target/construction facts, conversions and argument order.
- Every finite projection step's checked owner, field identity and endpoint
  type, preserving secret/refinement/interface identity and established machine
  state qualification without inferring authority from physical layout.
- One dominating generated owning initializer and the nonowning alias chain.
  The original whole result moves there once; it is not copied or re-evaluated.
- The source scope and each dynamic activation, current HIR/MIR headers and
  remaps, and every normal/abrupt cleanup edge. Validate these before pruning,
  including unused aliases and unreachable metadata.

A matching name, span, type or LocalId cannot substitute for this proof.
Generated storage has private compiler identity, not an invented resolved Source
declaration. Synthetic call/Handle extraction scopes must neither shorten nor
extend its lifetime. Alias reads retain the whole owner's dependency, but
last-use liveness cannot release it early. Only the hidden owner has an owning
cleanup slot. Failed initialization must distinguish vacant, acquired and retired
storage and must not publish an uninitialized alias.

Reference execution must retain the checked scope-owned obligation; incidental
Value cloning is not lifetime evidence. Native emission uses ordinary owning
storage for the whole producer and nonowning alias storage. Pending ancestors
still fail at the original projection before later effects; no implicit join or
metadata erasure is introduced. Exact checked proofs remain required for every
admitted executable body context.

## Separate operation-bounded call views

[Internal call-view staging](native_scoped_call_view_staging.md) keeps its own
temporary through one complete consuming operation. Its BeginCallView/EndCallView
rules are unchanged. Ending an inner call loan cannot retire a persistent local
alias's backing. The new local rule cannot lengthen an existing call stage, and
a call-stage End cannot be used as this source local's scope-exit cleanup proof.
Shared root/projection validation is useful only with distinct lifetime proofs.

## Bounded ordinary function-body implementation

The first native implementation recognizes an immutable alias declaration in
the original body of an ordinary nongeneric named function. Its producer is an
exact checked direct call to an ordinary nongeneric named function returning a
finite ordinary struct. The existing typed finite struct-field path selects a
bytes or list endpoint. HIR and MIR cover both endpoint kinds; the checked
reference transport currently supports bytes endpoints only. Reference list
transport remains owed. The native whole-owner proof currently admits finite
primitive/string/bytes/list/struct data; other wrapper, sum, collection,
recursive and runtime-authority owner shapes remain outside this accepted
slice. Nested declaration scopes, loops, generic/reflected bodies and types,
actor handlers, Resource owners and other producer families remain
implementation gaps, not new checker prohibitions.

HIR preserves the original `CheckedViewSource::Other` fact. It gives the whole
owner private generated storage without a Source DefId and associates the exact
original declaration, producer occurrence, typed projection and function-body
scope with an opaque receipt on the Source call. Canonical producer capture is
single use, after the original canonical passes. The immutable checked archive
supplies the expected inventory before current markers are inspected. Exact
conversion and peer-callable replay preserves that inventory; erasing a current
marker or current function cannot remove its obligation while the archive is
retained. Reusing this archive grants no Resource custody or execution family.
Stripping the entire archive together with every private marker remains the
existing public IR boundary; the controls make no stronger claim.

MIR retains a separate immutable HIR association and seals complete function
headers and the current graph. It validates one complete acquisition before
alias publication, either a direct call or an authenticated staged call followed
by the existing call-loan Ends and an exact owning move. Definite initialization
precedes a read-independent lifetime pin. That pin keeps the whole owner live
through the original function body's cleanup edges, including after the last
alias read and for an unused alias. The alias has no owning slot. Existing
result emission acquires the complete owned result before ordinary terminal
cleanup; no emitter, Runtime helper or ABI change is needed for this slice.

Temporary admission requires an actual immutable original `VarDecl` with a
written `view` initializer, its canonical variable definition/name/span and
checked `Other` origin. Viewed enum patterns and loop binders may share that
origin fact, but keep their existing borrowed execution paths and gain no
temporary owner authority. Complete nested AST lookup still detects genuine
written temporary declarations before raw or unsupported-region execution;
those checked-entry and nested-region refusals remain in place.

Reference execution retains the whole Value in checked activation-owned
vacant/live/retired storage. Payload-free private tickets and nonowning argument
frames access that owner instead of cloning a view into public Value storage.
Original root functions declared through `mutual:` retain the declaration's
DefId. Their later immutable definition is joined through its original resolver
name-span edge to that same DefId, without equating the body span to the earlier
declaration span. Registration still requires the unique original definition,
complete typed AST, canonical name, namespace and checked signature; temporary
receipts also retain the exact physical root, call and activation checks. Method
dispatch remains outside this ordinary-root activation family.

Explicit clones acquire an independent owned result. An exact successful-output
packet carries the acquired return across scope cleanup, bound to its original
function and activation. The private tests cover these storage and protocol
edges; they do not constitute a complete connected lifecycle trace or native
allocation/destruction observation.

One genuine checked Source control passes a temporary bytes actual to the
factory before a later handled argument. MIR requires the first call loan
before the sum observation, invokes the factory once, ends that call loan, then
moves the whole Packet into its persistent lexical owner. Its returned explicit
clone retains that owner in the exit plan. A separate genuine child-local
compaction test covers a nonidentity local remap. Disconnected-block fixtures
are synthetic private replay controls: the public Resource graph guard and
pruner refuse their artificial preimage. Only isolated exact temporary replay
restores the original graph and unchanged independent receipts. These tests
grant no genuine Source-generated unreachable-block or public-pruning credit.

The initial **27 focused tests** pass: reference 7, HIR 10 and MIR 10. A further
ordinary-entry regression passes after repairing interface-method dispatch in
the reference adapter. Only genuine original root declarations receive an
ordinary activation; unrelated methods keep their established dispatch, while
edited claimed roots and ineligible bodies requiring a temporary frame refuse.
Two additional mutual-declaration controls pass with Debug and Release checked
Source. They cover original producer, caller and borrowed callee definitions,
one whole-owner acquisition/retirement, and refusals for edited body/span/header/
namespace/name facts before acquisition. The full current core-library gate
passes **1,842 tests**: typechecker 379, Comptime 678, HIR 271, MIR 379 and
codegen 135, including the two written-declaration classification controls
below. All compiler focused controls are included in that total. Exact logs
under `target/native/` are `temporary-local-view-focused-v4.log`, SHA-256
`cd3f32de06fed2008516fe2e3297f1e9a8310b5cfe01accba68caee4390b7251`,
`temporary-ordinary-entry-focused-v1.log`, SHA-256
`92da6acdfeaa20c57522508a9d12194d30fff5707da15b416338d70cd58cada3`,
`temporary-mutual-entry-focused-v1.log`, SHA-256
`02ba9c2b11b282641211dcc89aed18d2a34db2d882ca3c8d26492e245ea8200f`,
and `temporary-local-view-core-libraries-v4.log`, SHA-256
`e1fa74d244fc54b25e3044a349fe321d25c94198694c17e05cae4de872ab13b9`.
The earlier full fixture attempt failed with 546 passes and 57 failures because
the adapter confused mutual declaration and definition spans. Its log is
`temporary-local-view-fixtures-v1.log`, SHA-256
`8fe22530ee635e56f2ef5b82ade5766dc2444d9b7307d4fe03e9ddf4b4b9e5bd`.
The successor fixture attempt also fails, with 549 passes and 54 failures:
`temporary-local-view-fixtures-v2.log`, SHA-256
`34eb9d0f7e4d6a4bc4d0ec0f15f66494b99f792aaf3ab1abffbebc9bf037b0bb`.
The written-declaration classification repair corrects the false admission of
existing viewed pattern and iteration binders. All 12 reference call controls
pass, including two new classification controls and the mutual controls:
`temporary-written-view-focused-v1.log`, SHA-256
`47be7144f0e2a90ea9879053d4cadf00613d300e7ec9957ee8ca7f21cbd9a33b`.
The real JSON namespace smoke test also passes:
`temporary-written-view-namespace-smoke-v1.log`, SHA-256
`105f1d0a3bd723d623e3c57f568b18bdac2b79e1a29140fd76af403fbfde1ef5`.
The full successor fixture gate passes **603 tests** in
`temporary-local-view-fixtures-v3.log`, SHA-256
`da41cfcf837bc381f0cfecd23f11e210f9c6f48c5f9f9f9926c20b9ee59c5d65`;
the namespace smoke is included in that fixture selection. All **nine CLI
capture tests** pass in `temporary-local-view-cli-capture-v1.log`, SHA-256
`317b28bbd05dbfff3f68519d86c2cb52bd7cc80066a67a342cf2f48efca00745`.
These successful successor gates do not convert earlier failed invocations
into acceptance. Other failed or intermediate runs are not accepted gates.
GitHub CI remains disabled at the user's request; these are local results.

## Bounded native execution and remaining cleanup gates

`crates/jett_driver/tests/native_conformance/local_view_aliases/09_temporary_root_boundary.jett`
remains byte-exact SHA-256
`c0f288b659bdf84bf87fb7f46576eb7cadd3b440c000f726b5b1c099ccfd83af`.
It declares `bytes borrowed = view make_packet().data` and never reads the
alias. The selected backing is the whole returned Packet and its owned bytes
until main's lexical scope exits. Expected stdout/stderr and debug events are
empty. Its driver control now requires successful execution rather than the
previous native refusal. The new formatted Source13 includes producer-once
markers, a later forwarded read, an unviewed sibling and explicit-clone
independence. Source13 remains SHA-256
`48391e6a0fa7390f6d36cb190c8e4100d835218ca317232a64ec8ec228cd59aa`.
Both controls pass checked reference and paired Source Debug/Runtime Debug and
Source Release/Runtime Release native execution after deleting the written
Source file: **two tests, four native executions**. These are paired profiles,
not the full Source/Runtime cross-product. The exact expected stdout, stderr,
status and debug observations pass; Source09 retains empty output.

The complete refreshed regression selection passes **23 tests and 46 paired
Source-deleted executions**, 23 per Runtime profile, with zero publication
refusals and zero legacy Debug-only executions. It includes the two temporary
controls plus eight stable projected-view, eight scoped-call, two mutable-owner,
one reflected mutable-view and two actor persistent-view controls. Those actor,
reflected and stable regression controls do not establish temporary owners in
those contexts. All 1,056 compiler/Source inputs, 68 reused Runtime artifacts,
64 fixture inputs and 182 original run-pass inputs remain guarded and unchanged.

The fresh compile-only gate has zero Source executions; the subsequent matrix
provides the execution credit. Receipts under `target/native/` are:

- `temporary-local-view-gate-compile-v2/gate-acceptance.json`, SHA-256
  `fb39b49856ad89bbdfb5777008a1b9365b6ca4d95a04940a89e5285be4eb6891`.
- `temporary-local-view-native-execution-v2/execution-summary.json`, SHA-256
  `314b6e92a7cd0235bbf92292e49df9e92ebb86490de290982ae23c6865876c0a`.

Both receipts pin the actual compiled gate image to SHA-256
`a1f9a700a5219d91bbbf784a5768dc9ec87558a78a72ecd3ae7a9be7a8e28a14`.
The earlier matrix attempt failed and receives no accepted execution credit;
its partial observations are not added to the successful successor.
Empty-output success and producer markers alone do not prove physical lifetime,
sibling destruction timing or absence of hidden cloning.

Remaining acceptance work for the full contract includes:

- Preserve Source09 and Source13 as the accepted bytes baseline. Add equivalent
  joined checked-reference and paired Source-deleted native coverage for list
  endpoints and the remaining whole-owner shapes.
- Exercise nested scopes, skipped branches and loop fallthrough/Continue/Break,
  retaining backing after the last read until its original scope exits.
- Acquire an owned `return clone borrowed` or reply before backing retirement;
  preserve negative consumption/capture/escape controls and early Handle exits.
- Preserve pending-root and later-failure order, exact terminal diagnostics and
  cleanup of partial producers, whole owners, siblings and unrelated earlier
  values. Retire each owner once on success and failure without changing error
  precedence.
- Extend genuine checked reference and paired Source-deleted native Debug/Release
  execution to the remaining contexts. Inspect ownership/drop plans and measure
  existing private allocation/destruction counters: producer output cannot prove absence of
  hidden cloning. Require no leaked owners after teardown.
- Use authentic proof mutations and runtime fault injection for wrong context,
  producer/type/field/scope, stale remaps, missing/duplicate initialization and
  early/missing/double cleanup. Refusals grant no execution credit. Preserve the
  existing call-stage temporary controls and their independent End behavior.

The [local-view note](native_local_view_aliases.md) and
[stable projected-view note](native_stable_projected_local_views.md) retain their
bounded implementation status and historical receipts. The bounded
implementation, passing compiler controls and bounded MSVC execution do not
complete the selected temporary-backed contract or whole-language native parity.
