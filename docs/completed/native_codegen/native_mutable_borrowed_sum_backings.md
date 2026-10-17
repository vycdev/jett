# Mutable owned borrowed-sum backing

## Status and selected boundary

The bounded mutable-owned ordinary borrowed-sum extension has focused MSVC
acceptance recorded below. Existing immutable-backing, Resource, actor and
local-view receipts retain their original scope. No native coverage estimate
or historical fixture denominator advances here.

A canonical declaration views a Handle whose target is an explicit View of one
stable original sum binding. A mutable backing must be a genuinely checked Owned
local or owned parameter. An immutable alias cannot authorize owner changes:
persistent move-only payload aliases continue blocking whole-owner consumption,
rebinding, field replacement and state changes, including after the last read.
Generated source/output slots remain immutable and nonowning.

Implicitly copyable primitive/String destinations keep their checked Owned mode
and no persistent alias origin. A String acquires its ordinary retained copy.
Only the two constructor-authenticated intermediates retire at its final Let;
later source replacement or owning unwrap must leave the copied value valid.
The Result failure companion continues acquiring its one owned String copy.

## Original proof and canonical transitions

The opaque HIR receipt is produced at every genuine canonical local declaration,
including copied destinations that bypass local alias-origin metadata. It joins
the exact active root DefId/declaration/type/mutability/Owned fact, alias fact,
original initializer, real function identity and concrete required-scope/type
binding path. View parameters and borrowed match/iteration binders are excluded
by their original role evidence. Missing source facts cannot fall back to flat
or enclosing maps. Absence of view_source, matching names or public headers is
not ownership. The per-lowering token is not an independent check-session brand.

Initial sealing follows canonical inline extraction, conversion and genuine
reflected callable sealing. It pins the unique actual Let and complete physical
function. Later conversion authenticates the protected before tree and exact
canonical replay. Reflected source normalization excludes only the independent
sum inventory to avoid circular snapshots. The genuine private callable pass
supplies exact old/new callable tokens; sum renewal replays only those typed
changes and compares the complete resulting body before updating its snapshot.
Callable identity stays exact. No public current-tree reseal is introduced.

MIR admission consumes that receipt and retains it in the original declaration
and projection row. Current validation rejoins the immutable original HIR record
before the independent CFG/header/tag/arm/field checks. Existing exact sequence,
block and LocalId remaps remain mandatory. Public missing, foreign or mutated
receipts cannot gain authority from a refreshed current-tree seal.

## Preserved semantics and explicit debt

Outer pending sums fail at the existing typed tag observation before extraction
or failure continuation. Ready payload pending metadata, Return-only absence/fail
continuations, nested scalar Defaults and error/cleanup order remain unchanged.
Alternate outer Default payload lifetimes, temporary-backed Handle roots, mutable
View parameters, borrowed binder roots, erased/capability/actor/Resource carriers
and general alias expiry are not admitted by this extension. Scoped copied-Handle
contexts still need a genuine selected source fact; no capture fallback is added.
Baked/materialized callable composition remains unverified implementation debt;
the ordinary same-function callable control does not establish it.

The selected [temporary local-view lifetime contract](native_temporary_local_views.md)
is separate and remains pending implementation. This stable binding receipt needs
a genuine Source DefId and cannot authorize its hidden temporary owner. No runtime,
ABI, helper, measured archive or Resource/actor custody permission changes here.

## Authored Source and proof controls

The five additive native cases contribute ten profile-matched Source-deleted
executions to the selected matrix below:

- Source03 and Source15 differ from their preserved originals only in two backing
  declarations each. Scoped/forwarded move-only aliases and mutable copied final
  bindings retain their exact original output.
- Source13 and Source14 differ only in their one backing declaration. The original
  RuntimeFailure oracle checks named-argument order and terminal-failure behavior
  after the borrow in both profiles.
- Source16 keeps primitive/String copies across old-shell replacement and then
  observes both the retained old copy and new owning payload.

HIR controls cover selected Owned facts for both destination classes, changed or
foreign receipts, missing origins, duplicate declarations, denied resealing and
canonical conversion. A genuine same-function reflected callable composition
lowers and completes conversions twice while retaining exact callable identity.
MIR controls cover fresh proof admission, original rows/current headers, immutable
nonowning slots, copied-intermediate retirement, persistent owner-change refusal
and actual nonidentity LocalId compaction through a captured callback.

## Bounded MSVC acceptance

All five affected compiler library targets pass **1,807 tests**: Typecheck 379,
Comptime 663, HIR 261, MIR 369 and Codegen 135. The log is
`target/native/mutable-sum-core-libraries-v1.log`, SHA-256
`6ab23ff17459a6000209349df28c39fcb77dcd903288974836eeb6a815f0a895`.
The focused HIR and corrected MIR groups each pass five tests; these are reruns
within the library total. Full Rust formatting passes. These five targets are
an affected-library gate, not the full workspace, Driver or fixed inventory.

The selected native matrix passes **10 tests** and **46 Source-deleted
executions**, covering 23 distinct programs:

| Selection | Tests | Executions | Runtime Debug | Runtime Release |
| --- | ---: | ---: | ---: | ---: |
| Five additive mutable-sum Sources | 5 | 10 | 5 | 5 |
| Unchanged ordinary borrowed-sum packet | 1 | 24 | 12 | 12 |
| Unchanged supplemental borrowed-sum packet | 1 | 6 | 3 | 3 |
| Exact actor direct persistent-view control | 1 | 2 | 1 | 1 |
| Exact actor projected persistent-view control | 1 | 2 | 1 | 1 |
| Exact reflected mutable-owned view control | 1 | 2 | 1 | 1 |
| Total | 10 | 46 | 23 | 23 |

Every execution pairs Source Debug/default with normal Runtime Debug or Source
Release/optimized with normal Runtime Release. This is not the full profile
cross-product. The authenticated harness checks Source absence, exact stdout,
stderr and exit status, including the original terminal-failure oracles. The
selected matrix has **zero publication refusals**; runtime failures are executed
failure oracles and supply no publication-refusal count. The five additive
Sources contribute ten executions, rather than 46 new programs.

Fresh compiler gate:
`target/native/mutable-sum-gate-compile-v1/gate-acceptance.json`, SHA-256
`4e2cd94e7ccc2cb23ae22b14a65e66a547d4136e148cf7f2708294cfb147c819`.
It pins the Debug test executable `native_conformance-3b1b799f1cd09233.exe`,
SHA-256 `165d50c4c149bb12d1a59e4549401d2960cda242855636ece26fd49d94197624`.
Execution summary:
`target/native/mutable-sum-native-execution-v2/execution-summary.json`, SHA-256
`df43d3141cbef95c2c68402cb1fff94e78f5aa753c50d9ab523ad241d45ea7b4`.
These paths use the primary checkout's shared `target/native`. The interrupted
v1 attempt is not a completed matrix and supplies no additional acceptance.

Before/after guards preserve **1,046 compiler/source inputs**, **68 reused
runtime artifacts** and all **50 fixture hashes**. Guarded fixtures are not the
executed-program denominator. The frozen runner is
`native-codegen-recovery-2026-10-09/mutable-sum-execution-v1/run-execution.ps1`,
SHA-256 `e681256ae305bc6af5d56f4593e0623243876cfbf3b4b3ed71deacd9bced3f75`.
Its exact count/launcher inputs and all six passing selections are bound by the
summary. Runtime/helper/ABI and measured bundles remain unchanged. The reused
normal receipt SHA-256 is
`2b2b9cd529c451c216072dc2b563253149f409bd3ef9ea867a1a86da1c637069`;
the guarded private receipt SHA-256 is
`35db08fbe46f339dad36731e24998497fdd77b459b0a283ed236cd124638db60`.
Guarding that private bundle grants no private-launcher execution credit.

These normal-runtime controls establish output/status/order parity. They supply
no new private cleanup-counter observations, complete physical cleanup counts
or full Resource-corpus acceptance. Earlier 753/31/64 and 1,797/32/66 local-view
receipts remain specific to their own revisions and selections.

Temporary-backed Handle roots, mutable View-parameter roots, borrowed binders,
absent or mismatched selected facts, alternate outer Default payload lifetimes,
scoped copied-Handle contexts lacking genuine selected source facts and
baked/materialized callable composition retain their separate proof or
implementation limits. The lexical whole-owner temporary-backing contract
selected at `f865e3f1` is unchanged and still awaits implementation and cleanup
gates, including unchanged Source09. This stable binding receipt cannot
authorize a hidden temporary owner. No last-use expiry, implicit clone, invented
Source identity, wider escape or operation-bounded call-stage lifetime is added.
Complete Resource transport, wider actor/task compositions, asynchronous
scheduling, full coherent workspace/inventory execution and exact-revision local
supported-host/installed acceptance remain open. The fixed 207-fixture/182-object
denominators and full native-codegen goal are unchanged.
