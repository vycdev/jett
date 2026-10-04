# Typed caller ownership: selected implementation contract

Status: caller repairs are committed at
`a63f891016fac7dc50fffdde33be8b0ce45c655f`. The complete Windows native suite
passes 480/480 with stable HEAD/tracked sources. Current full-workspace,
182-object inventory and supported-host acceptance remain separate obligations.
The integration began over `a9a6cd63db33fb29fb10f4b32607b3f82ab431bc`. An earlier formatted repair checkpoint
passed all 304 checker, 188 HIR, 181 MIR, 67 backend and 63 object-emission tests,
plus the MIR doctest. These counts describe that compiler-core gate.

A stable Windows native snapshot completed 472 of 479 supplemental test groups
successfully, with seven failures. Five failures subsequently passed focused
native replays: the nested breakpoint condition, primitive and refined interface
identity, contextual secret constructors in both profiles, and the 64-fixture
scalar/owned-collection group. The collection fixture corrections add explicit
views to set/map observers and preserve their output oracles. All 64 sources also
pass source checking. Ten breakpoint-context and three intrinsic owning-staging
regression groups pass. These native replays preceded the formatting-only pass;
the compiler-core gate above follows it. The later complete 480-test receipt
below supersedes the historical 472/479 result.

The bounded builder-generation and handled-producer repairs are now implemented.
The actual focused gate passes all 66 MIR caller acquisition groups and all seven
generation groups; three native emission groups and the original builder native
regression also pass. A new converted handled-view linked fixture passes in both
profiles after source deletion, observing exact payload lengths one and two.
All seven failures from the historical 479-group run now pass individual replays.
The fresh complete Windows native suite now passes all 480 tests at exact a63,
with zero failures and stable HEAD/tracked sources, in
`target/native480-after-owner-generation/receipt.json` (root terminal `06b7f8`),
ending `2026-10-04T14:20:12.987780+00:00`. Full workspace, current fixed-inventory
and supported-host acceptance remain pending. The historical focused results
remain distinct from that new complete-suite count.

Earlier integration receipts passed 598 frontend fixtures, 15 native caller groups
with source deletion and exact debug/release streams, and both property replays
that failed the earlier driver run. Their scope remains historical. The unchanged
exact-a9 baseline passed 464 native cases and all 182 object obligations; workflow
37130142896 passed all four Windows/Ubuntu build and installed jobs. Those baseline
receipts do not validate this candidate.

This implements the existing Rule24 contract rather than a new source ownership
rule. No Resource carrier, provider, ticket runtime or finalizer acceptance follows
from the caller/native receipt. A separate private reference Resource slice now
passes 36 focused tests, recorded in
[the resource-hook note](native_resource_hook_identity.md#reference-lifecycle-execution);
production providers and native Resource support remain disabled.
The full 100% native-codegen objective and rough 85% feature estimate remain open.

## Existing authority and actual proof

Rule10 (`docs/design.md:2091-2095`) and Rule24 (`:5794-5848`, especially the
four-row table at5821) determine caller ownership independently of callee access.
Written `view` keeps a caller owner; bare owned data is relinquished even when
the formal is View. Explicit view to Owned is E0375. A known move-only borrowed
binding cannot supply ownership; retain conservative E0401 for its bare input,
including a formal View destination. Exact implicitly copyable values copy.
Field reads borrow the parent and acquire the endpoint by the already selected
copy operation; Resource-bearing field copies remain prohibited. A view of a
producer needs a temporary backing owner, rather than an existing binding to
retain. Root flattening, general owner-changing overlap, alias expiry and projected
write policies remain unselected. The bounded builder-generation follow-up below
implements existing admitted source behavior.

I independently read the immutable root-executed exact94 report with SHA
`a9dbc168c298b84d75a88c15442849ede9cac182bc572af0deff713651041e04`.
All10 ordinary sources have matching debug/release check outcomes (20checks),
no timeout/schema failure, and the five admitted default-profile reference runs
have exit0, empty stderr and debug0. Cases02/03/07/08 are admitted retention
holes; cases04/05/06/09/10 already give sole E0400. Case01's explicit views are
admitted. Case07 proves a source-first Owned destination is retained because
legacy analysis ignores the actual formal-to-source permutation. These are
historical exact94 observations, not current-a9 ordinary CLI re-execution.

Root also actually built and executed the fixed checking-only a9v2 probe;
root-execution-proof-attempt2.json reports build0/execution0 and stable guards.
The separate actual results SHA is
`a18000c88557644514b6b64b30c495d4796de26192a18a50c9198ed34d966e89`.
All12 synthetic checks (six sources x debug/release) agree between profiles:
Resource01/03/04/06 are accepted;02/05 are rejected by the Check phase with
sole E0400 `` `owner` was consumed and cannot be used again ``. Accepted03's
actual formal-to-source permutation is [1,0]. Accepted snapshots retain exact
private Construct [View Network,Owned int64], BorrowOperation [View Network,
View Resource], and Close (the Finalize recipe) [Owned Resource] shapes. Formal modes and
permutations are present, but a caller retention handoff table is explicitly
unavailable. Thus the qualified/mixed/pipeline holes affect actual checked
Resource signatures, while the indirect/private-bare controls already reject.
Resource06 preserves explicit borrows then an owned Close boundary.

The probe has no provider installation, interpreter/native invocation or
application stdout. Its historical hard-coded compiler_provenance string says
build proof pending; the separate root execution proof supplies actual build
and linkage provenance, without modifying that saved result. No runtime/native
cleanup result is inferred from reference value copies or synthetic preparation.

## Schema

Put the closed checker types in a new `jett_typecheck/src/caller_ownership.rs`.
The following describes concrete fields/variants, not applied Rust:

- `CheckedCallerSyntax::{Bare, WrittenView}` records original spelling. Written
  view is recovered only through Paren and already checked carrier-preserving
  Coarsen/Declassify wrappers; it is never inferred from the formal or inserted
  HIR/MIR View. The existing E0375 source-boundary check remains authoritative.
- `CheckedCalleeAccess::{Owned, View}` records the exact destination access.
- `CheckedCallerEffect::{Copy, RetainBorrow, TransferOwned, RelinquishOwned,
  ObserveData}` records the separately validated caller disposition. ObserveData
  is available only in the selected lexical test/debug context below.
- `CheckedBindingFact { definition: DefId, declaration_span: Span, ty: TypeId,
  mode: CheckedBindingMode, mutable: bool }` anchors a binding to the same
  resolution session and concrete body. Parameter facts include their actual
  concrete types and View modes. Forwarded alias modes retain the existing
  immediate `CheckedViewSource`; they are not replaced by the final owner.
- `CheckedCallerOrigin` is `Binding(CheckedBindingFact)`,
  `BorrowedProjection { source: CheckedViewSource }`,
  `OwnedFieldCopy { parent: CheckedViewSource }`, or `OwnedExpression`.
  BorrowedProjection applies only to a checked explicit view of a field path.
  A plain field is OwnedFieldCopy. Clone and owned calls/constructors are
  OwnedExpression, including explicit views of their produced result. The
  original typed field expression proves every owner/index/endpoint; no second
  vector of mutable field indices is added. Source Other means no stable
  original binding and cannot be upgraded to a retained caller owner.
- `CheckedArgumentOwnership { source_span: Span, source_index: usize,
  parameter_index: usize, actual_type: TypeId, parameter_type: TypeId,
  syntax: CheckedCallerSyntax, origin: CheckedCallerOrigin,
  callee_access: CheckedCalleeAccess, effect: CheckedCallerEffect }`.
- `CheckedInvocationTarget` is an exact resolved source DefId, the existing
  `CheckedGenericCall`, existing `CheckedMethodCall` or `CheckedInterfaceCall`,
  an indirect Function TypeId, or closed IntrinsicId. No name/namespace/raw
  spelling is an authority.
- `CheckedInvocationShape` is `Function { signature_type: TypeId }` for the
  exact substituted source/indirect Function type, or `Intrinsic { intrinsic,
  result_type: TypeId, operands: Vec<CheckedIntrinsicOperandRole> }`. The closed
  operand roles validate source arity/variadic shape and allowed source access,
  rather than fabricating a source Function DefId or function arity.
  Print/Println have one checked PrintArgument role per variadic source actual
  and identity source order; the current checker has no ordinary signature or
  call-order fact for them. Their permitted written views remain physically
  borrowed and bare linear arguments remain consuming. Metadata appended by
  HIR has a distinct compiler-added role, never a missing source actual.
- `CheckedCallOwnership { target, shape: CheckedInvocationShape,
  context: CheckedOwnershipContext, arguments: Vec<CheckedArgumentOwnership> }`
  stores arguments in lexical SOURCE order. `CheckedOwnershipContext` is
  Ordinary, Verify, Property, or BreakpointExpression. Invocation syntax and
  observing context are distinct facts; Trace is its existing statement.

The effect is checked from the complete tuple rather than accepted as a free
Retain flag. Ordinary noncopyable Binding/Owned + Bare yields TransferOwned to
Owned, RelinquishOwned to View. WrittenView + View yields RetainBorrow, whether
its backing is an owned binding, known borrow, projected borrow or an owned
producer temporary. Known borrowed Binding + Bare is E0401 before an effect is
accepted. A new owned endpoint/producer follows the same callee access but has
no original binding to invalidate. Copyable type proof yields Copy and does not
invent authority for a Resource/capability. WrittenView + Owned remains E0375.
A temporary's RetainBorrow means protect its one acquired backing through this
operation, not retain a non-existent caller binding after it.

The schema contains no public ticket ID, registry key, Resource runtime value,
Finalize permission, raw DefId foreign-session token or synthetic carrier.

## Checker ownership contexts and integration

Add `call_ownership: HashMap<Span, CheckedCallOwnership>` and
`binding_facts: HashMap<Span, CheckedBindingFact>` to CheckResult,
CheckedGenericFunctionInstantiation, CheckedBodyFacts, TypeChecker and
ActiveGenericInstantiation. Binding maps use declaration-name Span to fit
existing capture/clear operations, while each value carries the exact DefId.
A scoped body's ownership walker inherits the exact incoming lexical binding
state/context; local facts are extracted by declaration span. Captured incoming
bindings use their parent concrete context, not a top-level definition_types
entry overwritten/restored by a different generic instantiation.

Ownership context is lexical, not inherited from whoever triggered checking.
Use a private current-context stack/save-restore field for recording: entering
an ordinary named/generic/inline function body selects Ordinary, even when its
instantiation was requested from a verify/property/breakpoint expression.
Entering the actual verify/property body or breakpoint expression selects its
observation context; nested reflected scopes inherit that exact lexical owner.
Do not derive new caller effects solely from ambient in_verify_block/
in_property_block flags. Existing generic checking already resets those flags
(checker.rs:7227-7228); follow that boundary and restore it for the new ownership
context too. check_function_impl itself has no equivalent reset. The caller site may
ObserveData while the checked callee body still consumes its Owned formals.
Add a source test instantiating the same generic body from a test block and an
ordinary function, proving that only the lexical test call has ObserveData.

Recording routes to the last active generic frame or root, matching
record_call_argument_order/record_binding_mode. Wire initialization, completion,
clone/swap, checked_body_facts and clear_checked_body_facts. Nested
CheckedComptimeTypeBinding owns its own complete facts. Inline parameters,
locals, handler/match/loop bindings and incoming captures need exact facts
where they enter the existing checked environment. CheckResult.definition_types
is useful for ordinary declarations, but cannot substitute for per-instance
local facts after generic type_env restoration.

After argument binding/type checking, record each accepted source invocation
once, using the same target/signature and formal-to-source permutation. This
covers ordinary direct, explicit/inferred generic, source method/interface,
indirect and pipeline call routes. Error recovery still checks malformed/excess
arguments normally; it must not manufacture a successful ownership fact.
Pipeline source0 is an explicit virtual source occurrence: first step uses the
original input origin; later steps use the previous evaluated producer result.
`into view` supplies WrittenView for source0. Extras retain their own syntax.

Replace source-name first-View retention in ownership.rs with a borrowed
`OwnershipFacts` context over the same Module/ResolveResult/checker maps.
Use DefId-keyed state and exact declaration facts for typed invocation effects;
names are diagnostic labels only. Walk actual arguments in lexical source
order, looking up destination mode by each checked parameter_index. This
consumes a bare owner before any later actual can reread it, even when named
arguments were checked in formal order. Preserve the existing args-before-
indirect-callee evaluation order. Transparent wrappers recurse to the proven
binding; a field copy observes the parent without moving the whole parent.

Ordinary bodies use root facts. Generic templates are checked at each concrete
instantiation (checker.rs:5748-5793,5843-5848); analyze each selected concrete
body with its own parameter types/facts, not the legacy AST `bytes` stand-in.
Nested scoped/reflected bodies push their own frame and consume the existing
CheckedStaticSelection/CheckedComptimeTypeSelection. Do not re-evaluate type
reflection or visit deliberately unchecked branches. Existing flow join/loop
conservatism is retained; this work grants no new borrow lifetime. Any legacy
untyped helper retained for AST unit tests must be separate from production
checked analysis and must not authorize source-name retention.

Do not flatten generic maps by Span. finish_check's current flattened
with_debug_types copyability conjunction is observation compatibility only; it
is not caller authority. Concrete breakpoint/consumption observations should
stay with their owning frame, with any compatibility summary conservative and
never used to decide caller ownership. Keep existing exact span/code/message
suppression when a typed E0401 replaces its legacy counterpart, preserving
unrelated ownership diagnostics.

## Math, JSON and debug are separate audits

1. `stdlib/math.jett:28-31` declares Owned list parameters for average/median.
   Rule24 therefore consumes a bare owning list. Their private compiler
   intrinsic may physically read the operand; that does not retain the caller.
   ownership.rs:1030-1041's literal math names lack independent selected
   retention authority. The design's1648-1649 example reuses scores without
   written view/clone although both formals are Owned; record/repair that stale
   example during aligned documentation rather than adding an exception.
   Likewise design5770/5807 forwards a borrowed data binding bare into the
   SOURCE list.length wrapper. IntrinsicId::ListLength is list.__length;
   from_callable_name does not classify list.length as that intrinsic, and the
   real wrapper itself forwards with explicit view (stdlib/list.jett:11-12).
   Correct those stale examples to written view. They are not independent
   authority for implicitly forwarding into ordinary source wrappers.
2. `stdlib/json/90_public_api.jett:16-64` declares View for raw queries and
   serializers, Owned copyable string for parsers. design3990 and6033/7057
   require explicit caller view for retained data and pipelines. Remove
   is_json_implicit_view_facade as ownership authority for source calls. Keep
   the public JSON policy/type/source handoff identities intact. When HIR
   substitutes a trusted source facade/helper, preserve the ORIGINAL caller
   disposition; the new implementation callee cannot retroactively retain it.
3. design6455-6475 explicitly allows ordinary-data observation in verify,
   property and breakpoint expressions; represent that lexical context.
   Authority-bearing capability/Resource input cannot gain an owner or clone
   from it. Keep existing Resource observation/printing guards. Trace retains
   its selected non-destructive statement semantics. Print/Println remain
   closed variadic compiler operations: bare linear actuals do not gain a new
   retention exception; explicit view physically borrows. Release checking,
   debug event ordering and unresolved hidden-Secret rendering stay unchanged.
   ObserveData preserves source availability only with an independently
   validated ordinary-data acquisition: copyable inputs copy; a supported
   cloneable data input to Owned gets one checked snapshot owner, and to View
   is borrowed. It never changes an Owned formal into a borrowed physical
   parameter. A function descriptor also needs its existing capture-ownership
   validation; its Function signature alone does not prove authority-free
   captures. Where supported snapshot acquisition is not proved, retain the
   current lowering refusal rather than expanding cloning authority. This
   slice changes neither absent/type-only controls nor the unsettled nested
   hidden observation domain. Existing can_snapshot_view is a physical
   supported-data predicate, not standalone source permission.

For compiler-owned operations, enumerate callee access by closed IntrinsicId
and exact source-operand role, with evidence from the existing selected syntax
and checker. MIR intrinsic_borrows is a PHYSICAL access map, not sufficient
proof of caller retention. No blanket IntrinsicId exemption is allowed. Pure
scalar/metadata operands remain copyable; constructor payload transfer and
already selected explicit borrowed operands keep their operation-specific
rules. A source Resource hook uses its exact checked declaration/signature and
ordinary Rule24, not this intrinsic route. Unproven operation-specific source
syntax is a separate prerequisite to implementing that entry, not a fallback
Retain fact.

## HIR, MIR and supported ordinary native lowering

Attach a required typed call-ownership packet to HIR Call/IndirectCall/Intrinsic
(or one shared invocation operand structure used by those variants). Lower
source facts into formal argument order using the existing checked
source_indices and retain evaluation_order as the inverse lexical permutation.
Convert origin DefId to current-function LocalId through the existing local
join. Keep signature/target and original occurrence separate from the physical
operand. The packet retains actual source input type and expected parameter
type separately: an already checked interface/coarsen/secret conversion may
change the lowered operand type, so validate the existing typed conversion tree
instead of demanding an incorrect raw actual-equals-parameter equality.
Validate count, unique complete source/formal permutation, TypeIds, exact
mode/signature, valid immediate local source and typed field path before
any pruning, including dead/unused source calls.

Compiler-generated calls use explicit generated-operation metadata with fully
validated argument acquisition/access; they do not use missing/None to retain
arbitrary source inputs. Helper dispatch, equality/refinement adapters and JSON
handoffs keep the original source packet when representing that invocation.
Extra reflected metadata operands are identified as generated copyable metadata
in a derived LOWERED operand shape, without changing the original source
argument count/permutation. Any generated linear operand still requires its
own checked acquisition. This
is not a caller-name or all-generated-call escape hatch.

MIR preserves the packet through handlers, inline extraction, scoped views,
compaction and every LocalId remap. An inserted View for formal callee access
never changes WrittenView or RelinquishOwned into RetainBorrow. Existing
explicit source aliases remain persistent; compiler call-view scopes retain
their existing bounded lifetime. Original source roots are provenance, not
implicit liveness loans created from bare inputs.

For supported ordinary linear data, lower RelinquishOwned-to-View by moving
the evaluated owned input once into an internal owning temporary at that
source-order position, then lending that temporary to the call. The original
binding is unavailable to later actuals/continuations. The temporary remains
owned across later-argument failure/early return and the complete call, then
normal owned cleanup releases it. No clone acquires authority. Physical View
access stays separate from ownership acquisition. Existing explicit
view-of-producer handling uses its one backing acquisition and no original
caller retention. At the initial HIR join, validate WrittenView against the original AST actual
and keep that source witness distinct from subsequently inserted physical
View. Later MIR validation checks the preserved packet/staging relation rather
than rediscovering caller syntax from the current operand wrapper. Validate
forged caller packets before MoveValuePlan and native eligibility, not after
successful optimizer pruning.

This supported-data path is a caller ownership/ordinary cleanup proof only.
Type::Resource still has no native scalar/layout/runtime carrier or drop ledger;
this slice must not enable it in is_linear/scalar_kind/emission. Future live
Resource requires typed hook dispatch/effects, sealed dynamic identity, exact
owning tickets, partial-argument reverse cleanup and Resource finalization on
every exit. The new packet will be a prerequisite, not that implementation.

## Bounded implementation files and proof

Likely files: new typecheck/caller_ownership.rs; checker.rs exports/context wiring;
ownership.rs typed flow; HIR lib.rs plus call validation/helper consumers;
MIR lib.rs, handlers.rs, move_values.rs, sequences/prune.rs and generated/inline
remap consumers; ordinary native call emit/verification where acquisition must
survive physical borrowing. Resource runtime/providers are excluded. Update
aligned design/architecture and active Resource notes after root selects scope.

Required tests are source-driven and run both frontend profiles: all20 old
characterizations are motivations, not expected legacy compatibility. Only01
keeps caller owners;02/03/07/08 now reject the invalid reread, and04/05/06/09/10
retain E0400. Add valid last-use versions, transparent wrappers, named mixed
permutation, direct/alias/indirect/pipeline forms, first versus later pipeline
producer, known-view aliases/parameters (exact E0401), copyable scalars, field
copy versus projected view, owned clone/result and explicit producer temporary.

Pin exact caller packets for generic copyable versus linear instantiations and
nested reflected/scoped bodies sharing Span. Source order must catch duplicate
owner use across permuted actuals. Independently pin lexical observation context
and math/JSON source signatures without bare-retention whitelist. Use the six
exact synthetic hook sources with the checking-only catalog harness: all12
pre-change outcomes are now actual root evidence. After repair, Resource01/03/04
must reject their invalid rereads,02/05 remain E0400, and06 remains admitted.
Add valid last-use counterparts proving RelinquishOwned without requiring
Finalize execution. No provider executes.

Original HIR/MIR malformed metadata tests must reject missing/count/permutation,
wrong signature/mode/type, forged source origin, alias claimed Owned and inserted
physical View claimed WrittenView, including unused calls before pruning. Native
supported-data controls cover both profiles, source deletion, exact stdout/debug
streams, partial-argument failure and balanced cleanup. Registration or a passing
metadata test is not live Resource or whole-language coverage acceptance.

At selection, no implementation or tests have run for this change. The full100% objective,
about85% broad estimate, fixed207 inventory/182 object obligations and464
supplemental native cases remain unchanged.

## Root selection receipt

Selected source contract SHA256: 963a045fbe05e40aa59c1ba6a6729a92fa6ccf83b9883b9dd4fcb8f09622ba17.
Manifest SHA256: 2b7bc6319cb79e0bb6d97d904bebf0820bb764d71b57be59b79234cd094b5e2a.
All 31 input guards and four artifact guards were checked before this write.
The selected behavior follows existing Rules 10 and 24. This note precedes
tracked Rust changes; the complete checker/HIR/MIR/ordinary cleanup scope above
remains required. The overall objective remains whole-language native parity.

## Exact producer and capture handoff

Root selected the additive caller-origin handoff before its Rust implementation at 2026-10-03T17:28:13.4850053Z (contract SHA256 dda43905e81f5d74ce939a9e1c3ad26098e1f06c42079a84b4f9f2fd21035665). Capture facts retain exact declaration DefIds in their concrete frame and are included by in-body source call occurrences even when their declarations are outside the body span. Resolved named functions, compiler-baked namespace constants and static namespace/type members are produced expressions; they cannot supply nonexistent caller-local roots. Local function-valued variables retain Binding provenance. No signatures alone prove closure capture cloning, no root/generic maps are flattened and no Resource authority is added. The selected handoff is implemented in this candidate; the header records current verification and remaining acceptance gates.


## Captured builder generations and handled conversion inputs

The generation path is restricted to an original checked written view of an
owned mutable plain `TypeConstruction` binding. Its complete original call,
Source identifier fact, full declaration header and assignments remain sealed;
only current loan/root/RHS/site identities undergo verified canonical remaps.
The replacement RHS is evaluated before the captured old generation enters
private escrow. The current replacement and captured backing have separate,
once-only cleanup on completion and abandoned call paths. No builder clone,
public carrier, source ownership rule or ABI permission is introduced. Broader
simultaneous cohorts and other owner kinds remain unaccepted breadth.

The handled-conversion path preserves original OwnedExpression/WrittenView/
RetainBorrow authority, its private Handle witness, exact owning storage,
Begin/End lifetime and produced-result CFG. Fresh caller acquisitions associate
only a fully validated interface boxing occurrence with its actual concrete View
input. Move analysis borrows that input; the boxing result is independently owned
and the runtime may copy its payload. Other same-type/span occurrences grant no
permission, and this does not authorize a general View-to-owner escape or a
zero-runtime-copy claim.

Root's actual focused receipts pass 66 MIR acquisition groups (2722a1), seven
generation groups (5f71c0), three native emission groups (76179f), the original
builder native regression (f9b4a4), and the new converted linked fixture (161f8b).
The last fixture observes payload lengths one and two, matches both profiles and
runs after source deletion.
The generation gate's earlier six-of-seven result was an invalid total-local-count
test assertion; its separate amendment checks the exact unused seed removal,
dense current IDs and immutable archives. Full acceptance remains pending.
Resource lifecycle/providers/native cleanup, concurrency breadth and the full
100% objective remain whole-goal work, with the broad estimate still about 85%.
