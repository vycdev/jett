# Native JSON for inferred uninhabited slots

This is a selected implementation gap, not an unresolved JSON type-domain
decision. Public JSON policy already admits the uninhabited type inferred for
an unconstrained empty collection or absent sum slot. The reference serializer
executes its checked trusted stdlib body. The characterized native selector
omitted Type::Never and left otherwise admitted targets on unsupported generic
JSON intrinsics. The applied serializer repair passes its compiler, linked,
ownership and public-policy gates below. The accepted `774b13e7` complete
workspace predates this repair. The `05494ed4` serializer revision now passes its
complete frozen workspace and all four supported-host jobs, centralized in
[the acceptance audit](native_acceptance_audit.md#current-frozen-revision-and-inferred-json-follow-up).

## Bounded contract

Preserve the inferred Never slot and select the existing source serializer for
an otherwise supported carrier. This includes empty lists, absent optionals,
one-sided results, empty string-keyed maps, and such slots inside nested records
or collections. Every other slot retains its ordinary recursive eligibility and
public policy. Runtime emptiness does not permit unsupported functions, actors,
interfaces, capabilities, resources, TypeConstruction, non-string map keys, or
secret data outside the existing public projection rule.

Do not choose a default element type, introduce an inhabited Never value or
source spelling, add a fake runtime representation, or substitute a generic
native JSON implementation for the checked stdlib body. Parser source selection
is a separate follow-up: empty and nonempty inputs, exact validation and failure
order require their own characterization. No parse selector change is selected
by this serializer repair.

Generic source bodies remain checked before HIR omits impossible list/map loop
bodies. Existing MIR sequence preparation retains carrier evaluation and the
length/pending check before bypassing an impossible element read. Existing sum
preparation retains the sum observation and pending check before bypassing an
impossible extraction. Bare Never specializations are not implicit project
callable roots, but retained calls or function references still require ordinary
verification. Inhabited carriers are verified normally. These preparations run
before native verification and independently of optimizer settings; ordinary
pure-call folding cannot make an ill-typed source body valid.

## Actual pre-change characterization

The original three-source family is recorded in
`target/native-next-after-qualified-machine/results.json`: six frontend profile
checks and three format checks pass; all three reference runs succeed with
application stdout `[]\n` and empty stderr. The inferred serialize and
serialize_public calls have four native AOT refusals across both profiles,
each reporting the unsupported generic serializer before archive lookup. The
explicit list[int64] control instead reaches the deliberately missing runtime
bundle in both profiles. Those missing-bundle controls are not linked passes.

The seven additional slot witnesses are recorded in
`target/native-json-inferred-slots-witness/results.json`: all 14 frontend profile
checks and seven format checks pass. Six reference runs succeed with empty
stderr/debug events and these exact application bytes:

| Carrier | Application stdout |
| --- | --- |
| Absent optional | `null\n` |
| Result with absent error type | `{"ok":7}\n` |
| Result with absent success type | `{"fail":"bad"}\n` |
| String-keyed empty map | `{}\n` |
| Generic record containing an empty list | `{"items":[]}\n` |
| Nested empty list | `[[]]\n` |

The seventh reference run evaluates a depth-two pending empty list. It preserves
application stdout `before\n`, emits no debug event, and fails with
`runtime error: for loop requires a list, string, map, or set value`; it never
writes the later JSON line. All 14 additional AOT attempts still fail at the
unsupported generic serialize or serialize_public intrinsic. No linked native
success or pending-error parity is proved by these pre-change refusals.

### Pure source and public-policy controls

Six additional fixtures are characterized in
`target/native-json-inferred-slots-extra-witness/results.json`. All six format
checks pass. The two positive fixtures pass four frontend profile checks and
reference execution with empty stderr/debug events: the pure direct/pipeline
facades produce `[]:[]:[]:[]\n`, and public projection of a record's secret field
produces `{"items":[]}\n`. The pure fixture also passes its reference test run:
one verify block and one property with 100 trials, two of two passed, no debug
observations. Explicit comptime evaluation remains closed and pure.

The four negative fixtures fail all eight frontend profile checks with one
error apiece. Two unused-variable warnings in the ill-typed-body fixture are
separate from its sole E0311 error:

| Control | Sole error |
| --- | --- |
| Ordinary serialization of an empty function-valued list | E0347 |
| Public serialization of an empty function-valued list | E0347 |
| Public serialization of an empty secret-wrapped element list | E0603 |
| Fabricating int64 as inferred Never inside an empty loop | E0311 |

All 12 AOT attempts preserve an existing output sentinel. The positive four
still stop at unsupported generic serializers before archive lookup; the
negative eight stop at their typed frontend policy/source error. These are
pre-change controls, not linked native passes or new public exemptions.

The separate owner-reread witness in
`target/native-json-inferred-slots-owner-witness/results.json` passes its format
check, both frontend profile checks, and reference execution with exact
`[]:[]:{"items":[]}:0:0:0\n`, empty stderr and no debug events. It borrows a
projected list from Envelope[T], calls both serializers, serializes an owned
whole-record clone, and rereads the alias, owner field and original list. Both
native profiles still refuse the generic serializer before archive lookup and
preserve their existing output sentinels. Ownership parity for this repaired
native path is established by the linked owner-reread gate below.

## Focused acceptance and remaining release gates

The production change adds `Type::Never` to serializer eligibility only;
parser and public policy remain unchanged. Its checker regression pins inferred Never rather
than a default primitive, direct/pipeline source targets, a concrete list[int64]
control, closed intrinsic identity, unsupported inhabited function targets,
and rejection of an ill-typed body inside an empty loop.

The focused checker regression passes in
`target/native-json-inferred-slots-focused-checker.log`. All 14 new driver
tests pass together in 8.09 seconds (392 filtered) in
`target/native-json-inferred-slots-linked.log`: the ten original carrier controls,
combined pure main/verify/property suites, public projection, grouped typed
negative publication controls, and owner rereads. The 13 runtime sources build
with matching debug/release archives and execute after source deletion. Exact
application bytes and empty debug/frontend observations are pinned. The pending
case exits 71 with exactly `before\n` and the recorded terminal message plus LF;
an exit-72 cleanup override cannot pass. The pure verify and 100-trial property
binaries also execute after source deletion in both profiles with empty output.
The four-source negative group retains its sole error code and existing output
sentinel before missing-archive lookup in both profiles.

The crossed library gate passes 948 tests: codegen 41, comptime 416, HIR 134,
MIR 86, resolve 48 and typecheck 223, recorded in
`target/native-json-inferred-slots-phases.log`. The initial invocation used the
nonexistent package name `jett_codegen` and executed no tests; the actual
`jett_codegen_cranelift` package was then tested successfully. That failed
invocation remains in `target/native-json-inferred-slots-phases-package-error.log`.
Driver library checks pass 83 tests in 66.79 seconds in
`target/native-json-inferred-slots-driver-frontend.log`; all 598 frontend
fixtures pass in 21.73 seconds in `target/native-json-inferred-slots-frontend.log`.
All 17 byte-identical characterized Jett sources pass formatting, along with
Rust formatting and whitespace checks, in `target/native-json-inferred-slots-format.log`.

The accepted `05494ed4` revision registers and executes all 406 supplemental
native tests, along with the complete workspace and all four supported-host
jobs. The audit centralizes its exact results; these accepted serializer gates
do not validate later source changes. The separately characterized parser gap
and its pending repair are recorded in
[the parser contract](native_json_uninhabited_parse_slots.md).

The independent current machine checkpoint and predecessor workspace/platform
evidence are in
[the acceptance audit](native_acceptance_audit.md#current-frozen-revision-and-inferred-json-follow-up).
The whole-language objective remains 100%, the broad planning estimate remains
about 85%, and the fixed inventory remains 207 fixtures. This slice does not
settle the open actor/interface/debug/lifetime/global-constant/list.sum or
reflected-request policies, or implement future reference providers.
