# Occupied Resource struct primitive companions

Status: implemented with bounded local MSVC acceptance. Full-language and
supported-host acceptance remain open.

The selected v9 extension connects the existing implicitly copyable primitive
surface to occupied Resource struct construction: all eight integer types,
float32/float64, immutable string, bool and nothing. Transparent aliases retain
their canonical checked TypeId and source reflection metadata. No new language
spelling or admission rule is selected.

The original checked declaration, exact field relation, written argument order
and private constructor operation proof remain authoritative. Physical
validation checks integer width and range, a distinct `Value::Uint64` carrier,
and float32 as the exact f64 widening of binary32, preserving signed zero,
infinities and NaN behavior. It never rounds an untrusted supplied carrier into
acceptance. The ordinary checked expression path applies the declared
primitive's existing normalization rules first; that rounding boundary remains
separate from physical validation.

Every `run` layer remains transported data. Integer, floating-point and bool
carrier children store depth separately from payload bits. Nothing preserves
its existing scalar-word depth encoding: codegen sends zero payload plus depth
into the carrier and reconstructs the depth word on observation. String handles
preserve their existing depth through clone, observation and retirement.
Construction does not imply a join or readiness. Single-word observation refuses
pending carrier scalar data; the generated path uses the depth-preserving pair.
The two additive authenticated leaves,
`jett_rt_v1_resource_carrier_child_scalar_task` and
`jett_rt_v1_resource_carrier_observe_scalar_task`, retain all existing signatures
and ABI1/wire3. The composed catalog becomes 58 production exports: 38 ordinary,
15 carrier and five startup. The eight private hooks remain unchanged. Depth
grants no Resource ownership, provider authority or session proof.

The extension preserves the current Callback, Owner, Actor graph and generated
startup contracts, including exact original/current archive transitions and
canonical collection iteration. New scalar Run/Join traversal remains tied to
its original checked constructor endpoint and qualifier; it does not bypass
Graph transport or general Resource custody validation. Driver artifact reuse
is bound to the exact source, support bytes, profile and runtime archive. It
caches completed artifacts, never live contexts, authority or session witnesses.

Original Sources33..35 and their support remain unchanged. New Sources36..44
cover primitive and alias fields, width boundaries, binary32 normalization,
distinct float64/uint64, pending transport and inspection, prior ordinary
failure, and the primitive pending matrix. A uniquely named trusted support
namespace is appended only for the new collectors. The authored 23-case table
includes clean completion, failed factories, cleanup panic and expected exit71.
It supplies normal and same-provider-grant reentry controls in both profiles.
The accepted execution counts and their evidence scopes are recorded below.

## Verification and remaining acceptance

The original v9 all-target Cargo check, Rust formatting checks and 82
measurement-helper controls passed. The initial eleven focused compiler groups
passed 46 tests, including both real reference-oracle controls over all 23 cases
and both checked source modes, plus Driver object emission in both Jett
optimization settings. Object emission alone supplies no linked execution.

After the ordinary source-completion repair, actual Cargo runs pass all four
affected compiler libraries: 1,322 tests (Comptime 620, HIR 221, MIR 347 and
Codegen 134). The Runtime library passes 366 tests, including exact ordinary
status, independent Resource refusal and cleanup controls. These unit runs
precede the private report-copy repair and two-file native oracle correction.
Only those three exact guarded files differ afterward. The unit groups do not
constitute a full workspace test run.

Resource source completion now retains the ordinary diagnostic when the
returned nonzero body status exactly matches that recorded ordinary failure. A
wrong nonzero status, independent Resource failure or real cleanup panic retains
its existing refusal or cleanup channel. The private test observation API copies
the selected channel directly, preserving an empty Resource message for an
ordinary-only failure. Production failure-copy behavior is unchanged.

Six stale native expected Resource-message fields are corrected across two test
files: five ordinary Bounds cases and absent Source13. Their original Source,
reference message, body/cleanup status, selected channel, process exit, events,
custody and reentry assertions remain unchanged. Historical duplicated refusal
bytes are explicitly superseded; no root/nested special case is introduced.

Fresh normal and private MSVC Debug/Release archive measurements pass the
58-export catalog, COFF, static CRT, exact C signatures, source/fingerprint and
ordered native-library guards. The corrected native gate is freshly compiled
against its exact private receipt pin and dep-info. All sixteen corrected
predecessor Resource gates pass 754 Source-deleted executions and 900 entries.
Both new scalar gates pass 92 Source-deleted executions and 138 entries: 46 normal
executions complete 46 entries, and 46 same-grant reentry executions complete 92
entries. Reentry counts come from successful guarded Rust loops and their
per-child decoded report assertions; they are not individual per-child log
counters. Existing scripts, streams, reports, statuses, custody and ordinary-empty
checks remain required. The current Resource and Startup receipts bind the
complete corrected source.

Earlier Graph acceptance passed 32 Source-deleted executions, and selected normal
Callback, Owner, Actor and interface regressions passed 42 tests before the isolated
oracle correction. Their source differs only in those two exact expected-message
tables. The unchanged selected tests and production code retain that bounded
evidence; it is not a fresh execution of the corrected test binary. The fresh
guarded Startup rerun passes its complete 80-execution matrix. Earlier failed
native attempts supply no acceptance. These results do not constitute a fresh
full workspace or complete language conformance run.

Full workspace/inventory, exact-revision supported-host CI, GNU and clean
installed-distribution acceptance remain pending. The known deterministic CI
failures remain a separate repair obligation; this slice claims no exact-revision
CI acceptance. Archive measurement and object emission alone supply no runtime
result. Earlier donor and predecessor results retain their original scope and
revision.

Generic and recursive Resource aggregates, refinements, secret qualifications,
collections, projection/extraction/replacement, reflection, captures and
platform breadth remain full native-codegen obligations. This extension adds
no Source exclusion. The bounded results above do not establish full-language
coverage. The rough full-goal estimate remains 87% until broader acceptance
supports a change.
