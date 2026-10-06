# Returned Resource hook descriptors

Native lowering now supports exact returned compiler-hook descriptors for the
Factory, Borrow and Close recipes. Immutable bindings, aliases, root Return
relays, storage without invocation and immediately invoked returned callees
retain the original checked hook. These are installation-owned metadata, with
no Resource owner, finalizer or ordinary code/environment Function allocation.
A matching public function type never selects a hook or grants provider authority.

MIR captures the original return and producer family from the retained
ResourceSourceArchive, then rechecks exact headers, original/current Source
calls, aliases and Return occurrences against the current CFG. An independent
body/entry/header snapshot protects callee effects and selected actual order.
Canonical block/local transforms authenticate that snapshot before consuming
remaps; missing retained targets fail without erasing evidence. Fresh CFG facts
invalidate changed locals and joins. Readonly descriptor getters select exact
current occurrences; copied expressions do not receive that authority.

A preceding ordinary branch with no nested Return can preserve one exact root
Return. Nested or conditional descriptor returns, mutable/dynamic producers,
captures, generic/scoped bodies and descriptor escapes remain pending. The
current rule does not infer an identity from the callable signature.

Native wire shape 10 records the exact returned hook. Wire tag 15 joins the
original indirect invocation to its hook, signature and separate physical
Acquire, Borrow or Close target at the exact operation frame. The emitter opens
the operation, evaluates and stages original actuals, evaluates the opaque
callee once last, and only then prepares the descriptor invocation. Existing
runtime installation, hook, signature, target and frame checks remain intact.
Raw returned metadata is adopted only after SourceStatus authenticates completed
Scope retirement. Exact opaque occurrences bypass ordinary Function clone and
ownership storage; ordinary named callbacks retain their separate lifetime.

Authenticated Resource function roots seed the existing typed reachability
traversal before dependencies are walked. An ordinary pure helper reached from
an otherwise unused descriptor accessor is declared and verified normally; it
retains the ordinary context/environment ABI and receives no hidden Scope or
carrier exemption.

The initial six-case baseline passed 76 reference executions across 38 scenarios
but failed native MIR with `indirect custody call lacks an exact descriptor
producer`. The expanded corpus has 46 scenarios, with all 92 reference and 92
Source-absent linked native executions passing across debug and release. Stored
and immediate callbacks cover all three recipes, aliases, relays, unused
metadata, factory failure, argument-before-callee effects, failed later actual,
failed callee and finalizer failure. Every execution checks zero obligations
before context teardown.

A review regression reproduced a copied/resealed branch followed by canonical
pruning being accepted before the repair. Nine focused MIR groups and all 242
MIR library tests now pass, including block/local pruning and missing-target
controls. Seven focused codegen groups, all 100 codegen library tests and 63
object tests and all 92 driver-library tests pass. Runtime sources and the
matched private archives remain unchanged.

This is local MSVC private-archive acceptance. Required/comptime materialization,
returned named callbacks, dynamic/captured/scoped/generic/handled/pipeline forms,
nested return transport, broader aggregates, production providers and
concurrency remain required for the full native-codegen goal. GNU private-archive
Source execution and final-revision workspace/distribution gates remain separate
obligations. The coarse whole-language estimate remains about 85%; this corpus
is execution evidence rather than measured whole-language coverage.
