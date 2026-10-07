# Native borrowed Resource sums

The full native-codegen parity goal remains active. This note records the verified local
implementation family after required value materialization at 17d6c841.
It implements the existing view rules; it adds no ownership or lifetime syntax.

## Source and evidence

The shared conformance corpus contains 73 scenarios. Sources 45 through 56 add
written and bare View actuals for None, Some, Fail and Ok; two named-argument
aborts after an earlier borrowed sum; and two handled borrow-domain failures.
All 146 reference executions passed in both profiles before production changes.
The original native baseline first refuses Source45 at HIR lowering, File0
186..236: a handled view is not yet recognized as a stable local origin.
The expanded corpus now passes all 146 Source-deleted native executions against
freshly measured Debug/Release MSVC runtime archives. Exact events, consumed
scripts, failure channels and zero ordinary/Resource obligations before teardown
remain mandatory. This supersedes the earlier 61-scenario / 122-run checkpoint.

## Existing source contract

An immutable typed declaration whose initializer explicitly views the handled
success payload may borrow from an explicitly viewed optional/result binding.
Its exact Optional or Result kind, success payload type and original binding
identity remain checked. The failure branch in this family exits the function.
An alternate Default, including one in a conditional before a final Return,
needs its own backing/lifetime proof and cannot inherit the original sum origin.
Defaults belonging to nested handlers are distinct from the outer continuation.
Temporary producers cannot acquire stable alias identity by annotation alone.

Written View actuals keep the caller's sum and sole occupied Resource owner.
A bare owned actual into a View formal transfers the entire sum into owning
argument storage; the callee only borrows that holder. Caller source argument
order governs evaluation even when named formal order differs. A later argument
failure skips the callee, ends all prefix leases, and drops each acquired holder
once. None and Fail carry no occupied Resource token or finalizer.

## Proof across compiler phases

Checked binding metadata retains the exact concrete body's resolved backing
DefId. HIR verifies the complete typed handled-view initializer against that
immutable local chain, without changing general assignment-root recognition.
The private checked Source archive supplies authority; public metadata or a
matching span/type never grants Resource execution permission.

MIR retains the original function, declaration, handled expression and target,
sum and endpoint types, failure block, source/alias headers, and concrete scope.
Canonical lowering introduces nonowning sum and payload headers authenticated
against that original occurrence. A fresh SumTag and its exact Branch select
Some/Ok before projection. Borrowed extraction never performs an owning SumTake
or manufactures an owner slot. Altered tag sources, arms, headers, current bodies,
foreign archives and truly disconnected extractions must fail independent proof,
including before canonical pruning or remapping.

## Native execution

Conditional sum views need a distinct runtime representation from direct
Resource loans. Occupied payload views are bounded child loans; absence creates
no occupied loan. Fail's ordinary string is observed/copied without consuming the
caller shell. All handles remain tied to the exact attempt, frame, registered
nominal shape and payload path. The original shell cannot move, be replaced, be
taken or be destroyed while any shell/payload view is live, including absent
shells that have no Resource token to enforce that restriction.

Children retire before their parent shell view; source call retirement precedes
argument-holder cleanup. Registration rows must authenticate conditional shape,
loan source, selected path, operation and frame rather than infer permission
from a handle tag. Any runtime schema or implementation change requires fresh
Debug/Release single-runtime archives, symbol/source measurements and a pinned
receipt before the complete Source corpus may count as native acceptance.

## Verification and remaining work

The complete reference suite passes 521 tests, including all 146 current Source
executions; all 93 driver-library tests pass. A separate test correction verifies
Source33's exact scalar and hook proofs and fixes the previous commit's sole
Ubuntu/Windows CI failure. Rust and all twelve new Source formatting checks pass.
All 2,188 compiler/source/config inputs revalidate; the final MIR formatting change
is an exact error-return line wrap, followed by another complete 254-test pass.

All 305 checker, 203 HIR, 254 MIR and 215 runtime library tests pass. Three
focused MIR groups verify actual Source profiles, altered headers/guards/bodies,
foreign or missing proofs, disconnected extraction and exact loan ordering. Six
runtime groups cover conditional shell custody, failure companions, child loans,
argument-prefix aborts, forged registration and C output preflight. All 102
codegen library and 63 object tests pass, including unchanged prior forgery
controls. Fresh custody validation precedes local-view queries that consume its
proof, preserving forged-graph rejection as invalid MIR.

Both freshly measured archives contain 37 Resource leaves, eight private test
exports and one main, with static CRT and the actual ordered native library list.
Their independent receipt is pinned in the native harness. Neither this private
MSVC gate nor earlier platform CI proves current full workspace/GNU acceptance.
Broader aggregates, qualified values, callbacks, reflection, production providers,
asynchronous execution and complete platform acceptance remain in the full plan.

The first constructor inventory is limited to root-body Resource aliases.
Nested/scoped Resource alias declarations, ordinary non-Resource borrowed sums,
and alternate Default payload lifetimes remain separate unverified native paths.
Checker/HIR acceptance alone does not establish native execution for these paths.
