# Ordinary entry in a Resource object

The ordinary-entry bridge has bounded Source17 object/native acceptance and a
current compiler proof checkpoint. It changes entry metadata and wrapper emission,
preserving Jett source semantics and ordinary function calling conventions.
The earlier v20 whole-workspace gate and all eleven v21 MSVC-v3 native
regression gates pass; exact-revision CI remains pending.

## Historical failure and selected repair

Root's v7 gate compiled the candidate and passed all fourteen MIR controls and
all forty-four runtime controls. The first thirteen original Source17 wrappers
emit actual objects before wrapper14 fails. The reported error is `callee has
no fresh execution-family signature`. Root's v7 terminal is `6b7069`, session
15179; its unchanged-3742-input receipt is
`d57fca527e65a7008a8fce54a1c0293cdc984a90bddd01b2c1485b2a4bedead6`.

`emit_for_triple` selects the Resource emitter when the checked manifest is
nonempty. `EmittedResourceLayout::from_program` validates the selected current
FunctionId and its original Source header, populates execution-family rows, and
then calls `rows.function_signature(entry)` before checking
`plan.function(entry)`. Thus the reported error is the selected entry lookup.
`source_row` first checks `plan.function(target)` and would report a different
error for a missing Source callee.

For wrapper14, `original_source(entry)` succeeded. This entry has an initially
captured proof-only witness. Its required values and original/current graph are
authenticated, but it does not need runtime Resource custody.
`has_execution_records` and `validate_resource_ownership` correctly keep it out
of `plan.functions()`. `absent_required_controls` likewise remains ordinary.

## Decision

Keep the existing execution-family classification. Introduce a separate private
entry Scope record for the exact driver-selected ordinary entry. The native
program wrapper owns this Scope and its completion. The Jett main and ordinary
helpers keep their ordinary context/environment ABI.

Appending the ordinary main to `plan.functions()` is unsuitable: signature and
translation currently use presence in that collection to add a third Scope
parameter and select `ResourceEmission`. Falling back to the ordinary object
emitter is also unsuitable: it omits the installed private archive entry,
authenticated root tuple, and retirement contract.

## Authenticated MIR API

A readonly `ResourceEntryScopePlan` with private fields and no public
constructor. It exposes the exact selected `FunctionId`, complete identity and
header, one root `ResourceFrame`, and one `ResourceOperation` whose role is
`Complete`. This frame and completion describe wrapper execution. They do not
claim that a Source branch or Source Return executed a Resource operation.

The entry-aware API is:

```rust
pub fn validate_resource_ownership_for_entry<'p>(
    program: &'p Program,
    types: &'p TypeInterner,
    entry: FunctionId,
) -> Result<ResourceOwnershipPlan<'p>, Vec<ValidationError>>;

impl ResourceOwnershipPlan<'_> {
    pub fn entry_scope(&self) -> Option<&ResourceEntryScopePlan>;
}
```

The existing validator and `function`/`functions` getters retain their meaning.
The entry-aware validator first runs complete current call and witness
validation. If the selected entry is already an execution-family function, it
uses the existing family plan without adding a bridge. Otherwise it mints only
the separate entry record under all of these conditions:

- The selected ID denotes the exact current function in the dense table.
- The selected function has its own initially captured Source witness. Another
  function's archive is not sufficient authority to reconstruct its MIR body.
- Its original/materialized archive, manifest, interner, identity, complete
  current graph and local/parameter headers pass the existing private checks.
- It is not required-only and has no runtime execution-family classification.
- The exact original/current native root header is closed, capture-free,
  Network-only (including zero parameters), and returns Nothing, matching the
  currently supported Resource entry header.

The record borrows the immutable validated Program through the plan lifetime.
It adds no owner slot, loan, carrier slot, callable proof, Source invocation,
descriptor authority, provisional return or execution-family member. The
original required-value proof remains responsible for authenticating the
materialized values and body through canonical preparation.

At initial authenticated checked lowering, capture a proof-only witness for
every capture-free, Nothing-returning, Network-only function (including zero
parameters) in an authentic Resource Source archive. This uses identity and
header, not a `main` name. Require `source.manifest().is_some()` and preserve the
complete original/current archive join. An independent full-body graph seal is
mandatory for this proof-only class, including genuinely pure bodies with no
required-value row. Missing-witness validation derives eligibility from the
exact immutable execution-archive function ID and header. A public header edit
cannot remove the original function from this mandatory-witness class.

This metadata condition does not alter `runtime_custody_needed`, which remains
the execution-family classifier. Raw HIR/default archive and ordinary
non-Resource paths remain unchanged. Public mutable MIR is never sealed
retrospectively. The entry-aware constructor selects only the exact passed ID
from these authenticated candidates, and still rejects required-only selection.

## Codegen projection and wrapper

`resource_execution::emit` validates the original graph with the entry-aware API
before preparation. `EmittedResourceLayout::from_program` repeats that fresh
entry-aware proof after canonical preparation. It serializes a bridge signature,
root Scope frame and Complete row separately from execution-family rows. The
root has only the Root parent. All ordinals are produced from the private typed
record, not from source names or a public map override.

`plan.function(main)` and `plan.function(absent_required_controls)` remain None.
Both declarations and body translations therefore keep the ordinary two hidden
parameters and ordinary ownership/caller proofs. Entry metadata presence in the
layout's Source-family signature, frame and operation maps exclude bridge rows.
`source_row` retains its mandatory family-plan check. A legitimate ordinary call
to main elsewhere continues to use the ordinary ABI.

The bridge-aware wrapper performs this exact sequence:

1. Obtain the installed entry Scope through the existing EntryScope leaf.
2. Validate the compiled function/signature/Scope tuple with ScopeValidate;
   ordinary main has no Resource prologue to perform this validation for it.
3. Obtain Network arguments through the existing EntryNetwork checks.
4. Call ordinary main with context, zero environment and ordinary parameters;
   do not append the Scope argument or select ResourceEmission for its body.
5. Read the existing NativeLeaf::Status after the body. Do not take, reset or
   replace the original body status.
6. Attempt the exact bridge Complete row through ScopeComplete, passing that
   original status. ScopeComplete is the existing cleanup leaf. Its selected
   error can differ from a nonzero original body status, so do not return that
   selected error prematurely before observing the retired root outcome.
7. Read EntryOutcome with the compiled tuple and return its exact original body
   status. If retirement or tuple validation failed, propagate the authentic
   refusal; do not invent a success or a replacement body status.

Existing Resource-family entries continue through their current three-parameter
body and completion path. No C ABI, layout wire version, runtime leaf, report
format or provider behavior changes are needed.

Reachability must explicitly include the selected bridge entry as a native root
without making it an execution-family member. Required-only exclusion and all
ordinary transitive dependency/type checks remain intact. Keep all fourteen
wrappers, twelve original exports, refinements and shared Source fixtures.

## Required controls

- Actual original wrapper14 emits and runs in both profiles, with the correct
  required primitive values, body status and zero-before-destroy obligations.
- All first thirteen original wrappers and the complete export/refinement
  inventories continue to pass; do not substitute simplified Sources.
- Typed plan and emitted signature controls prove main and the ordinary helper
  remain ordinary, while only the selected wrapper has the bridge Scope and
  Complete leaf calls.
- A genuine checked pure entry with no Resource operation, comptime expression
  or required-value row receives its own initial proof-only witness and bridge.
  An equivalent raw/default-archive HIR path receives no Resource entry proof.
- A genuine ordinary internal call to the selected main remains valid with its
  ordinary ABI; bridge metadata must not give it Source-family authority.
- Missing, foreign or copied/resealed entry witness/body/header substitutions
  fail the current private proof checks, before canonical preparation can erase
  the mutation.
- A required-only selected ID, wrong current ID, changed original Network/Nothing
  header, capture or stale canonical graph refuses bridge construction.
- An ordinary body failure preserves its original status and message/channel,
  still attempts exact root retirement, and completes with no live obligations.
- Missing installation, wrong installed tuple and an invalid completion ordinal
  refuse without manufacturing Source activation or status.

Root owns production integration, remaining execution gates and final receipts.
The measured bounded checkpoints and current proof repair are recorded below.

The initial independent body seal uses existing certified canonical transports.
Consuming ordinary list iteration is covered by a genuine positive control.
Borrowed-view iteration and other unsupported composed transports remain
pending unless their own exact constructor transition is implemented and proved;
this bridge adds no general reseal of a changed body.

## Bounded object/native acceptance and current compiler checkpoint

All fourteen whole-Source17 wrappers emitted **28/28** genuine host objects in
both profiles at v14, with zero refusals. The full original 3,600-byte Source
prefix, twelve exports, unused refined contract, required controls, verify and
property100 remain intact and in scope. This main-wrapper gate does not establish
every original purpose or live aggregate value.

target/native-resource-carrier-compile-evidence/after-v14.json (SHA-256
43c3a3f1fc2fa6d1e4c6cbd7cc7e0bad4f0d560dce0685e6a8fcb2b411aee2dc)
records actual session 94028, terminal f7b2aa, exit zero.

The separate v15 gate passes **28 Source-deleted single-entry executables /
28 entries**, including Source13's exact ordinary bounds error, and **26 clean
same-grant reentry executables / 52 entries**: **54 native executables /
80 entries**. Exact completion/message/event reports, all eight zero Resource
custody counts and empty ordinary storage before one teardown, event boundaries
and script exhaustion pass. Source13's repeated ordinary-error call on the same
grant remains pending; thirteen clean Sources in both profiles do not replace
that lifetime obligation.

target/native-resource-carrier-compile-evidence/after-v15.json (SHA-256
b20c4d9427b11676e7207e5691171c64a861666e0f728b8aa24dba631123154b)
records actual session 93295, terminal b1a472, exit zero. Both v14/v15 retain
all **3,752 frozen inputs / nine runtime files unchanged** at parent
91e3706b770124cbddca952e5fe6b558abc170be. The measured v15 runtime receipt pin is
74a811660ee1772e0374e04579dd51234e8ff38191b942335bedaab5b4e4b6cd.

The current post-refinement-repair compiler gate is separate from v15 native
acceptance. At v19, **17 focused entry controls (13 MIR + four codegen)** pass,
including four new raw/refinement archive controls. Full package gates pass
**314 MIR checks (313 library + one doctest)** and **181 Cranelift checks
(118 library + 63 object)**, with zero failures or ignored tests.

target/native-resource-carrier-compile-evidence/after-v19.json (SHA-256
b549b995be46093f29dbf45314b47f1eea3b09c9bfc20cebf068e477bfaa8f8b)
records actual session 65346, terminal 26ed19, exit zero and all **3,757 frozen
inputs / nine runtime files unchanged** at the same parent. Earlier v3 runtime
gates passed **275 checks at v6** and **44 focused carrier checks at v7**. These
are historical revision-specific results, not current whole-runtime or workspace
success.

## Current workspace and wider-language observations

The earlier v20 whole-workspace gate passes **3,582 checks, zero failures and
13 ignored tests**, across **66 real selected groups / 92 total result groups**.
Actual session **75431**, terminal **1f1c8b**, exits zero for
`cargo test --offline -q --workspace --no-fail-fast`.
target/native-resource-carrier-compile-evidence/after-v20.json (SHA-256
fb59285ab91f6d15ce57ab59ecf7cc5c00187cdf4c323de04b5bbafe4a78e7d2)
records all **3,757 source inputs / nine runtime files unchanged** at parent
91e3706b770124cbddca952e5fe6b558abc170be. The inputs-v20 manifest SHA-256 is
58dd6f590a1bf2fd63f04d0ec0abdd15aa71452e9e00b1ff36b1feb29ab3612a;
the workspace log SHA-256 is
f610439bbbcb334d88de90c8b8ebf4178f1015be564cc7caf63bba2b5d6ed8da.
This is measured workspace success, not fresh ignored-native acceptance.

All **11 fresh-v3 native gates now pass**, each selected as a real one-test gate.
Actual session **41872**, terminal **3c7b60**, exits zero with **576 Source-deleted
native executions / 672 entry calls**, across Debug and Release. This includes
the unchanged **191-case / 382-execution corpus drawn from 113 distinct Sources**,
carrier controls, reflected-body controls and owned-Return regression. The v20
workspace's ignored-test count is historical; v21 independently executes the
eleven native gates rather than treating ignored tests as acceptance.

target/native-resource-carrier-compile-evidence/after-v21.json (SHA-256
38f67805c93e66e273d29de84c1cfc3351d70c0235bf656ea661244474582fa2)
records all **3,757 source inputs / nine runtime files, HEAD and inventory
unchanged** at parent 91e3706b770124cbddca952e5fe6b558abc170be. The frozen native
binary SHA-256 remains
110ba3b7bf23e63677ac7afd13a83fc2fa98aeccebc553e8df4f294fb5bc872f.
Matched MSVC v3 archive provenance passes before and after execution with receipt
pin 74a811660ee1772e0374e04579dd51234e8ff38191b942335bedaab5b4e4b6cd.
The successful runner report SHA-256 is
699f2364769ccdd489c7322832f2410690129d8f687006239e3dc6f0b6e82ae6.
This gate establishes no GNU runtime-archive acceptance.

Owned-Return cleanup failure retains caller/entry-root **body255 / cleanup255**;
the observations establish retirement and root completion, not a separate nested
callee completion. The accepted historical Return semantics and exact Source
prefixes remain intact. Earlier prospective fresh-v3, full-191/382 and reflected-
Return gate statements in retained historical sections are superseded by v21;
broader body/Default, generic/reflected lifetimes and occupied custody remain open.

At v21, Source13's repeated ordinary error remained pending; the v24
same-grant gate below closes that bounded obligation. Occupied resources, wider
whole-language parity and the successor's exact-revision CI remain pending.
The rough 87% estimate and fixed 207/182 inventory do not advance.

The separate 15-case wider-language probe is an **observation baseline, not
acceptance**. Actual session **77834**, terminal **ee797f**, leaves compiler
inputs unchanged and records eight emitted objects, six object refusals, one
earlier backend failure and three reference failures. Frontend/driver build
success does not establish linked parity. Its authoritative receipt is
target/native-language-parity-probe-observations-v1/root-after-evidence.json
(SHA-256 434cba150528d36e018d89b9dbe3acb05adf0309ac6a5aeb47a8ea1b906b4c3d).

## Raw refinement proof boundary

Pure refinement predicates lowered through raw/default HIR without an
authenticated checked Resource archive stay ordinary through canonical
preparation. Declaration kind or spelling alone supplies no Resource witness or
entry Scope. Raw bodies with actual Resource custody retain the independent
mandatory capture/refusal path.

Authenticated predicate proof requirements derive the original declaration kind
from the immutable archived function. Removing a witness and changing the current
predicate kind cannot escape that requirement. Constructor seals reject missing
or foreign proof, modified predicate body and changed header. The repair preserves
ordinary raw refinement behavior while retaining authenticated original authority.

## Remaining acceptance

The earlier whole-workspace gate passes at v20, and all eleven matched-
MSVC-v3 native gates pass at v21, including full **191-case / 382-execution**,
carrier, reflected-body and owned-Return regression. These remain independent
historical receipts; current ordinary-error regression and workspace acceptance
are recorded at v24 below. Earlier accepted GNU Source-prefix and Return
documentation and wire-v2 receipts retain their independent historical scope.
The successor's exact-revision CI remains pending. No GNU runtime-archive
acceptance is inferred from these local MSVC results.

Occupied aggregate transport/replacement, broader Resource-valued Default
and generic/nested reflected lifetime compositions remain full-goal obligations. Whole Source/export/refinement and verify/property100
purposes remain required. No refusal counts as successful Source execution, no
Source policy exclusion is introduced, and the full native parity goal stays
active. The rough 87% estimate and fixed 207/182 inventory do not advance.

## Accepted ordinary-error successor checkpoint (v24)

The changed ordinary runtime passes its measured **307-check** suite with fresh
matched MSVC `measurements-02` archives. The accepted twelve-gate native scope
covers **604 Source-deleted executables / 728 entries**: the new all-fourteen
same-grant gate contributes **28/56** from session **62286**, including both
Source13 Bounds errors and original body1/cleanup0/Body completions; session
**76385**, terminal **d30da4**, passes the eleven predecessor gates with
**576/672**. The latter includes full **191-case/382-execution** regression from
**113 distinct Sources**, carrier, reflected body, owned Return and original
Source06. No separate nested callee completion is inferred from Return root
observations.

The fresh whole workspace passes **3,614 checks, zero failures and 14 ignored
tests**, across **66 nonzero / 92 total groups**, at actual session **75810**,
terminal **0f5a7b**. Independent verification at **58261 / 75fabb** authenticates
all **3,763 source inputs / nine fresh and nine historical runtime artifacts**,
HEAD/inventory, both native binaries and fresh02 provenance/log hashes unchanged.
The authoritative receipt is
`target/native-resource-carrier-compile-evidence/after-v24-native12-workspace.json`
(SHA-256
`4dabdff125cefac5891d6b026cb0b67ec14448db67915755116f014698ea6aa2`).
The v20/v21 receipts above retain their accepted historical scope; current
ordinary-error regression is established independently by v24. Earlier
Source13-pending statements describe their original checkpoint and are now
superseded for this bounded same-grant obligation. See the
[ordinary terminal-error note](native_resource_ordinary_error_reentry.md) for
the exact archive, binary and per-attempt messages/retirement evidence.

Local MSVC results provide no GNU runtime-archive or full platform acceptance.
The separate normal-main linked baseline is complete: sixteen observations,
twelve exit0 and four exit71, from eight exact opaque objects reused with
Debug/Release runtime artifact profiles; all fifteen raw-probe Sources remain
preserved. This is baseline collection without a desired Source oracle, new
feature acceptance or ordinary public-Main context-retirement observations.
Broader ordinary parity and fixes remain open. Occupied custody/replacement, broader Default and
generic/nested reflected lifetimes, wider language parity and the successor's
exact-revision CI remain open. The rough **87%** estimate, fixed **207/182**
inventories and full 100% goal do not change.
