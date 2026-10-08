# Typed Resource aggregate transport

Status: typed absence transport, current compiler proof checks, the v20 whole-
workspace gate and all eleven v21 MSVC-v3 native regression gates pass. This
work preserves the existing Source
language and full native parity goal, with no aggregate, refinement, machine or
generic Source exclusion.

The whole original absent aggregate Source contains twelve exported constructors,
record/generic/enum/machine/qualified declarations, an unused refined view contract,
required controls, verify and property100. Fourteen wrappers preserve its exact
prefix. The historical pre-carrier baseline passed MIR lowering and public MIR
validation but failed ObjectEmission in both profiles because the single-leaf
ownership/layout model could not represent those declarations. The bounded
object/native checkpoints below supersede that historical failure.

The selected replacement is a private typed carrier plan with a complete original
type graph and opaque runtime roots. Constructor-issued roots retain nominal owner,
concrete generic arguments, ordered fields/payloads, selected variant/state,
qualification and checked refinement proof boundary. Root generation and typed
paths identify each projection; equality of physical layout or type spelling is
not authority. Full original HIR/type/Source and current MIR/CFG witnesses are
validated before the plan is issued. Existing single-leaf custody remains separate.

Zero live leaves is the first connected implementation. Empty collections, each
`none` shell, result failure data and inactive enum/machine payloads preserve their
complete selected data. A type's potential Resource paths are not removed by
actual absence. Latent occupied view formals and refinement metadata remain in
their original interfaces. Unimplemented live tree operations refuse before
effects while the same root/path interface awaits individual leaf custody support.
Copied physical Resource or grant handles never receive that authority.

Wire v3 adds typed graph, carrier slot/constructor/projection and Source carrier
roles. Strict accepted v1/v2 decoding and all old tag meanings remain. Fixed-width
status/opaque-u64 C ABI v1 records remain selected; actual new leaf symbol/layout
checks are required. Ordinary ownership fields cannot contain Resource-table sum
or owner handles. Typed observations route exact carrier lengths, keys, tags,
fields and sequence extraction through dedicated leaves.

An explicit borrowed-sum adapter retains carrier parent/path/generation and lease
while presenting selected optional/result children to the existing sum-loan
protocol. Owned extraction transfers once. Original call lexical staging, abandoned
prefix retirement before Return operands, Default joins, body/cleanup separation,
provisional Return publication and one-shot retirement remain unchanged.

Bounded whole-Source17 object/native gates and current fresh-v3 regression pass
as recorded below, including the unchanged 191-case and reflected Return gates.
Broader lifetime/purpose acceptance remains required.

Legitimate occupied trees, multiple kinds and dynamic paths, aggregate replacement,
machine transitions, refinement construction, recursive nominals, other reflected
families and derived or aggregate exceptional native reentry remain full-goal
follow-ons. Original Source06 single entries and original-then-READY same-grant
reentry were accepted at f4c63a0f and now pass fresh matched-v3 regression at
v21. Earlier wire-2 and bounded v15 absence receipts remain separate historical
evidence; current post-repair regression is established by v21 below. Existing checker/reference semantics determine live
replacement and destruction order; record any unresolved tradeoff before
implementing new behavior in Rust.

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

The current v20 whole-workspace gate passes **3,582 checks, zero failures and
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

Source13's repeated ordinary error, occupied resources, wider whole-language
parity and exact-revision CI remain pending. The rough 87% estimate and fixed
207/182 inventory do not advance.

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

The current whole-workspace gate passes at v20, and all eleven fresh matched-
MSVC-v3 native gates pass at v21, including full **191-case / 382-execution**,
carrier, reflected-body and owned-Return regression. Exact-revision CI remains
pending. Earlier accepted GNU Source-prefix and Return documentation and wire-v2
receipts retain their independent historical scope; only the measured v21 gate
supplies current MSVC-v3 regression credit. No GNU runtime-archive acceptance is
inferred from these local MSVC results.

Source13 repeated-error lifetime, occupied aggregate transport/replacement,
broader Resource-valued Default and generic/nested reflected lifetime compositions
remain full-goal obligations. Whole Source/export/refinement and verify/property100
purposes remain required. No refusal counts as successful Source execution, no
Source policy exclusion is introduced, and the full native parity goal stays
active. The rough 87% estimate and fixed 207/182 inventory do not advance.
