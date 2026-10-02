# Native JSON parsing for inferred uninhabited slots

This is a selected implementation gap under existing public JSON policy.
Unconstrained empty collections and absent sum slots retain their inferred
Never type. Actual reference execution already decodes admitted empty/absent
carriers and returns handled errors when raw input attempts to occupy a Never
slot. The characterized native parser selector omitted that slot and retained
an unsupported generic intrinsic. The applied source-selector repair passes
the focused compiler and 19-source linked gates below; coherent final
workspace/platform acceptance remains pending.

The accepted serializer revision and predecessor workspace/platform evidence
are centralized in
[the acceptance audit](native_acceptance_audit.md#current-frozen-revision-and-inferred-json-follow-up).
They do not validate this parser repair.

## Bounded existing contract

Preserve the inferred slot and use the checked trusted source decoder for an
otherwise supported target. An actual empty array or absent optional needs no
Never payload; a one-sided result can decode its inhabited arm. Occupied Never
requests return the existing unsupported-reflected-type error with the actual
index, field or result-arm prefix. Raw syntax, shape checks and parse_exact
validation retain their ordinary order. Handling this error does not make a
Never value constructible.

There is no default element type, source Never spelling, cast, retag, fake
runtime representation, native generic JSON fallback or public-domain change.
Every inhabited slot keeps its ordinary recursive eligibility. Unsupported
functions, actors, interfaces, capabilities, resources and builders remain
rejected; Set element and string Map key checks remain unchanged. Runtime
emptiness never exempts an unsupported inhabited target.

## Checked source and reachability proof

Never reflection has unknown_type, no primitive tag and canonical name
`<never>`. The selected decoder's closed TypeInfo/TypeKind argument facts skip
the primitive, alias, refinement, secret and named-aggregate branches. The
existing canonical JsonTree name guard also skips its incompatible success
body. The final unknown-kind branch returns only
`fail("unsupported reflected JSON type: <never>")`; it starts no construction
builder and manufactures no successful payload.

These are checked specialization facts, not optimizer folding. Ordinary pure
helpers and unchecked runtime conditions do not make an ill-typed source body
valid. The checker source-target unit uses trusted fail(raw) facades only to
pin direct/pipeline target metadata, concrete inferred slots and closed parse
identities; it does not prove the real decoder executes.

The real decoder has JsonTree input and inhabited result[Never, string] output.
It remains a normal reachable callee. Native validation already supports the
inhabited string error arm; the existing uninhabited sum pass can remove only
impossible success extraction after retaining the decoder call and sum
observation. A nonempty raw array loops over inhabited int64 indexes and still
calls the Never decoder. Optional and result decoding skip only an unoccupied
arm. Syntax/shape/tag checks, pending errors, prefix construction, input
evaluation and owned cleanup must remain observable independently of optimizer
settings. This requires no new runtime carrier or ABI.

## Actual pre-change characterization

Eight direct-call sources are recorded in
`target/native-json-inferred-parse-slots-witness/results.json`; eight pipeline
counterparts are recorded in
`target/native-json-inferred-parse-pipeline-witness/results.json`. Each family
passes eight format checks, all 16 frontend profile checks, and eight reference
runs with exact application stdout and empty stderr/debug observations. An
unused generic inference anchor warning is separate from checking success.
Each family has 16 AOT generic parse/parse_exact refusals before runtime archive
lookup; all 16 existing output sentinels remain intact. None is a linked native
pass.

The four shapes are empty list, absent optional, result with no inferred error
type and result with no inferred success type. Both APIs/forms retain the
literal occupied-slot errors:

| Slot | Handled decoder error |
| --- | --- |
| First list element | `0: unsupported reflected JSON type: <never>` |
| Optional payload | `unsupported reflected JSON type: <never>` |
| Missing result error type | `fail: unsupported reflected JSON type: <never>` |
| Missing result success type | `ok: unsupported reflected JSON type: <never>` |

Valid empty/absent inputs and supported result arms succeed. Malformed inputs
retain unterminated-array or exactly-one-result-key errors, as pinned in the
individual source captures. The lenient/exact outputs agree for these selected
inputs; this is not a blanket equivalence claim for the two APIs.

Three further string-keyed map, generic record and nested-list sources are
characterized in `target/native-json-inferred-parse-recursive-witness/results.json`.
All three format checks, six frontend profile checks, three reference/agent
runs pass with exact handled errors and empty stderr/debug observations. Their
six AOT generic parser refusals occur before unavailable-archive lookup and
preserve all six sentinels. Actual occupied-slot prefixes are respectively
`x:`, `items: 0:` and `0: 0:` before the same unsupported-reflected-type error;
valid empty map/record fields or nested empty arrays succeed. Malformed inputs
retain their recorded unterminated-object/array errors.

The runtime source gate has 19 actually characterized sources.
That source count is separate from registered Rust groups and executed native
tests; no linked pass follows from these refusals.

## Focused acceptance and remaining gates

The initial eight direct groups passed together in 9.14 seconds (406 filtered)
in `target/native-json-inferred-parse-direct-linked.log`. After integrating the
eight pipeline and three recursive groups, all 19 pass together in 14.08 seconds
(406 filtered) in `target/native-json-inferred-parse-linked.log`. The helper
builds both profiles with matching launchers, deletes source only
after both builds, and then requires process status zero, every exact literal
line and empty stderr/runtime/frontend/artifact observations. Errors are handled
within the program; success status must not conceal a skipped Never decoder.

The focused checked-target unit passes once (223 filtered) in
`target/native-json-inferred-parse-focused-checker-applied.log`. The crossed
library gate passes 949 tests (41 codegen, 416 comptime, 134 HIR, 86 MIR,
48 resolve and 224 typecheck) in `target/native-json-inferred-parse-phases.log`.
The driver gate passes all 83 library tests in 72.25 seconds and all 598 frontend
fixtures pass in 23.41 seconds, recorded in
`target/native-json-inferred-parse-driver.log` and
`target/native-json-inferred-parse-frontend.log`. These linked gates exercise
the real trusted stdlib decoder, separately from the metadata unit's stub.

All 19 new Jett format checks pass in the structured
`target/native-json-inferred-parse-format.log`; Rust format and diff checks also
pass. The corpus now registers 425 supplemental tests; this is separate from
the executed 19-test focused gate. The coherent 425-test frozen workspace,
final object obligations and all four supported-host jobs remain pending.
Do not substitute the accepted 05494ed4 serializer revision or registered
counts for current-head workspace/platform evidence.

The initial focused-unit log selected zero tests and is not accepted evidence;
the applied-unit log above records the actual pass. The source application
guard also detected a serializer-unit formatting-only baseline difference;
reviewed current hashes and patch context checks resolved it before application.
Neither event changes the selected parser behavior or validates an extra test.

The bitfield payload alias candidate is an independent later repair. This slice
does not settle open actor/interface/debug/lifetime/global-constant/list.sum or
reflected-request policies, implement future providers, or change the fixed
207 inventory. The objective remains whole-language 100% native parity, tracked
with the unchanged about-85% coarse estimate.
