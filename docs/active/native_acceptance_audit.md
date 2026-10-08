# Native parity acceptance audit

Full native parity is not established. This audit maps the requirements in
[the accepted plan](native_codegen_parity_plan.md) to executable evidence and
records the obligations that fixture success does not resolve. A release claim
must use passing results for the actual release revision, not an earlier green
checkpoint or a worktree modified during a long test run.

## Inventory gates

`tests/native_parity.json` contains 207 fixtures: 182 run-pass files and 25
runtime-contract files. The inventory validator compares it with the filesystem,
rejects duplicate paths and malformed rows, and checks entry-point obligations.
The verify/property gates discover test bodies from parsed source independently
of each row's lowering marker.

| Requirement | Denominator | Executable evidence |
| --- | ---: | --- |
| Checked HIR and MIR for every run-pass fixture | 182 | `backend_lowering::run_pass_backend_lowering_gaps_are_explicit_and_monotonic` lowers every discovered file and rejects any failure. |
| Nonempty native objects, including test-only roots | 182 | `native_parity_manifest::native_parity_object_emit_obligations_emit_host_objects` uses native test lowering and emits every inventory obligation. |
| Entry-point execution, exact output, and cleanup | 30 | `native_conformance::inventory_execution::native_inventory_main_execution_matches_interpreter` executes all manifest entries: 29 successful outcomes and one expected Graphics callback failure. |
| Runtime contract execution, exact output, and cleanup | 25 | `native_conformance::inventory_execution::native_inventory_runtime_contracts_match_interpreter` executes 17 wrapping-success cases and eight expected failures. |
| All checked verify bodies | 155 fixtures | `native_conformance::native_verify_suites_execute_for_all_run_pass_fixtures` discovers and executes every fixture with verify blocks. |
| Deterministic property trials | 3 fixtures | `native_conformance::native_property_suites_execute_for_all_run_pass_fixtures` executes the generated cases in native suites; the HIR suite builder requires 100 trials per property. |

The two inventory execution gates share provider-input decoding with the
standalone `native_parity` report tool. Clock, Random, Environment, and Graphics
inputs come from the manifest. Failure comparison requires exit 71 and exact
stderr; a cleanup failure overriding entry failure with exit 72 cannot pass.
The native launcher has a separate regression for that precedence.

The larger `tests/native/` corpus supplements these denominators. It is not a
replacement inventory or proof that every combination of language features has
been audited. Likewise, a fixture's reference in Rust is not proof of execution.

## Structural and distribution gates

| Requirement | Evidence that must pass at the release revision |
| --- | --- |
| Canonical identities and closed compiler handoff | Driver origin/entry tests, HIR backend type validation, generic and reflected-body regressions, and codegen tests for symbols based on structural types rather than session-local IDs. |
| Explicit control flow, evaluation order, ownership, and failure cleanup | MIR validation and ownership suites; linked handler, projected-view, temporary-owner, callback, actor, refinement, and terminal-failure regressions; native allocation-failure rollback tests. |
| Versioned C ABI and exact resource retirement | Runtime ABI version/layout tests, mismatch rejection, cleanup probes and panic containment; native launcher tests for successful and failing entry cleanup. |
| Deterministic objects, target selection, linking, and atomic publication | Codegen deterministic-object and unsupported-target tests; native driver linker/runtime validation and output-preservation tests. |
| Required comptime evaluation stays at compile time | Explicit-comptime and namespace-constant evaluation/materialization tests, per-instantiation context regressions, primitive value-shape rejection, and rejection of unbaked markers by codegen. No runtime initializer or interpreter fallback is an acceptance substitute. |
| Supported-host distribution | All four jobs in `.github/workflows/native.yml`: Linux GNU and Windows MSVC workspace/build/package jobs, followed by clean installed jobs with no checkout. Both runtime profiles must work after relocation and source removal. |
| Complete current workspace | `cargo test --locked --workspace --no-fail-fast` or equivalent test and doc-test invocations against a coherent revision. Rebuilding dependency artifacts during a run can invalidate later rustdoc inputs; such failures require a fresh doc-test run. |
| Stable documentation matches the implementation | Review `docs/design.md`, `docs/architecture.md`, the parity plan, and the notes below against the final implementation and verified gate results. |

The tested CI toolchain is Rust 1.97.1. Cross-compilation, future providers, and
asynchronous scheduling must not be inferred from host packaging or the current
sequential interpreter/native task model. Their implementation status remains
separate from parity for existing accepted behavior.

## Remaining semantic work

The following semantic and implementation gaps prevent a full parity claim:

- [Actor equality](../open_design/actor_equality.md): actor identity versus rejecting comparisons.
- [Actor handles escaping comptime](../open_design/comptime_actor_values.md): reject escaping handles versus materializing a defined actor graph.
- [Erased interface equality](../open_design/interface_value_equality.md): explicit comparison versus a defined dynamic equality contract.
- [Concrete-owner arguments in erased calls](../open_design/interface_same_owner_arguments.md): runtime validation versus rejecting unsafe erased calls statically.
- [Secrets hidden in debug print values](../open_design/debug_print_hidden_secrets.md): extend redaction to erased/builder payloads versus reject potentially secret values.
- [Projected field assignment](../open_design/projected_field_assignment.md):
  implement safe updates to owned mutable locals versus reject projected writes
  and use construction/rebinding. The checker now rejects immutable, temporary,
  and known view roots before either execution backend; owned mutable roots
  remain admitted without an implemented update contract.
- [List summation types](../open_design/list_sum_type_contract.md): numeric
  primitives and transparent aliases work in both paths, but the unconstrained
  source signature still admits refinements, secret wrappers, and nonnumeric
  elements. Native admission rejects them, and some reference empty sums have
  the wrong value shape. The domain and invariant contract remain unresolved.
- [Reflected field request compatibility](../open_design/reflected_field_request_compatibility.md):
  current checks admit some refinement/base requests with matching carriers and
  secrecy. Preserving the actual owner on an admitted interface read does not
  select the broader requested-type contract or authorize arbitrary nominal casts.

The [inferred uninhabited JSON parser repair](native_json_uninhabited_parse_slots.md)
is accepted at predecessor `08f9f7e7`: its 19 Source tests, frozen full workspace
and all four supported-host jobs passed. Each later revision still requires its
own coherent release gates.

The [interface audit](native_interface_values.md) also retains the remaining
facade, refinement-composition, and comptime combinations that need scrutiny.
Passing its growing regression suite does not remove those audit obligations.

## Selected boundaries and unimplemented extensions

Namespace constants have a selected current domain in `docs/design.md`:
implicitly copyable primitive values and transparent aliases are baked once by
the shared required evaluator; aggregates, nominal refinements, resources,
actors, function values and erased payloads report E9001. This is a frontend
boundary for both execution paths, not missing positive native support.
`native_global_constants_match_interpreter_without_source_in_both_profiles`
executes the admitted values and suites after Source removal, while driver
constant tests pin aggregate and hidden-pending rejection before publication.
Revalidate those gates at the final revision. The
[broader constant ownership note](../open_design/global_constant_execution.md)
records a possible extension and does not authorize runtime globals.

Current task/actor execution is sequential: eager `run`, one-level `join`,
no-op `cancel`, and synchronous actor dispatch. Existing run/join/cancel,
actor/message ownership and scalar actor-comptime Source gates remain parity
obligations. `docs/architecture.md` explicitly marks asynchronous scheduling
and capability cancellation checkpoints unimplemented; the reference path also
rejects Resource task/message transport and has no corresponding production
provider APIs. Introducing those absent APIs or a scheduler is separate feature
work, not a prerequisite for parity with the existing interpreter. Implemented
portions of partial features, including occupied Resource custody, still require
the complete original-Source evidence specified by the accepted plan.

The current private Resource checkpoint is 138 shared scenarios /276 native runs
in debug and release on MSVC, with Source absent at launch and zero obligations
before teardown. The nested-view packet passes all 276 independent reference runs
and all 276 native executions, including branch joins, scoped bodies, loops and
Break/Continue. The approximate feature estimate advances from 85% to 86%; it is
not measured code coverage or a changed inventory denominator. See the
[nested lifetime contract](native_resource_nested_sums.md). Archive provenance,
GNU private Source execution, and the final four supported-host workflow jobs
remain separate gates.

The [ordinary borrowed-sum implementation](native_ordinary_borrowed_sum_codegen.md)
now passes 15 reference Sources and 30 Source-deleted native debug/release
executions, including later handled-argument failures and mutable copied
primitive/String destinations. The fresh runtime also re-passes all 138 Resource
scenarios /276 native executions. Full affected MIR/codegen/object/CFG, runtime
and driver-library checks pass; coherent full workspace and current platform
acceptance remain required. The current rough feature estimate is about 87%,
with the fixed inventory denominators unchanged and the full 100% objective open.

[Direct collection and sum equality](../completed/direct_collection_equality.md)
is settled: both operators now report E0376 before comptime evaluation or native
lowering, including wrapped and concretely instantiated types. Frontend fixture
coverage and native debug/release publication checks pin this contract.

[User structs nested inside enum payloads](../completed/enum_payload_struct_equality.md)
now invoke exact explicit `Equatable.equals` implementations. Recursive frontend
validation, interpreter/comptime dispatch, and typed native cursor calls are
implemented. Linked debug/release coverage includes generic owners, containers,
recursive values, pending depth, method failures, and source removal. Compiler
phase suites and all 589 frontend fixtures pass. This closes this equality gate;
full inventory and platform CI are still independent acceptance obligations.

Normal native builds run frontend verify/property checks before emission; a
source-level failed property cannot be silently
bypassed to claim native failure coverage.
`native_builds_preserve_shrunk_property_diagnostics_and_existing_artifacts`
pins that public pre-emission contract: a property fails on its fourth generated
trial, shrinks both scalar and list inputs, and retains the exact E9000 message
and property-name span in ordinary program, verify-suite, and property-suite
builds. All three preserve an existing output artifact. The comparison also
checks the structured `test_file` result, including the trial count.
This verifies frontend diagnostic handoff; it does not establish native shrinking.
`native::property_tests::native_property_failure_identifies_the_trial_and_cleans_owned_values`
then changes a checked assertion only in test-owned MIR to exercise a failure
that the frontend did not encounter. Both debug and optimized linked suites
report the second property's name, trial 4, and original assertion text, with
successful owned-value cleanup. A failure injected after a successful suite
checks that the compiled calls cleared their context. Runtime ABI tests cover
context clearing, repeated/undersized failure copies, and caught predicate
errors. This supplies
case-specific diagnostics without skipping frontend validation or disclosing
argument values.

The native driver now retains checked input and function identities for replay
and shares the interpreter's bounded shrink search. The property inventory gate
and standalone parity report both use this driver. The regression
`native_property_runner_shrinks_native_failures_from_the_checked_session` injects
a checked-HIR assertion failure after frontend validation, then verifies native
prefix search, isolated replay, and scalar/list shrinking in debug and optimized
modes. It matches the interpreter's counterexample after deleting the source;
original suite output stays separate from replay output. Additional regressions
reject timeouts and an initial MIR fault that checked-HIR replay cannot reproduce.
Candidate generation/order and backend-error propagation are covered by the
shared shrink-search tests. The integer-domain regression covers both signed
and unsigned narrow pool boundaries and checks metadata through nested shrink
candidates. `native_property_shrinking_preserves_narrow_boundaries_and_nested_inputs`
compares debug and optimized native shrinking of an `int8` minimum with the
interpreter, including generic record, list, and map inputs. It prevents an
out-of-range positive candidate from replacing the signed minimum.
Refinement identities now survive candidate generation too. The interpreter
checks retained tags recursively, and native replay emits the existing checked
predicate handlers before calling the property. The regression
`native_property_shrinking_checks_refinement_chains_and_nested_predicates`
compares debug and optimized counterexamples for a namespaced refinement chain,
a generic record with a refined field, and a refined nonempty list whose elements
are refined. It also rejects a candidate whose predicate raises a runtime error,
without replacing the original property failure or failing cleanup. The direct
`Positive` regression keeps its counterexample at one rather than zero, and unit
tests cover nested container and sum shapes. Further failing-input combinations
remain part of the semantic audit; successful inventory counts alone do not
prove every shrink candidate can be replayed.

`native_property_shrinking_reports_the_executed_float32_input` pins binary32
counterexample fidelity: a generated `3.14` input is stored and reported as its
actual rounded value, `3.140000104904175`. Debug and optimized native replay agree
with the interpreter after source removal. Pool and shrink tests also check
binary32 identity, negative zero, finite limits, and subnormal rounding; binary64
values keep their original precision.

`native_property_shrinking_preserves_generated_generic_interface_owners` checks
canonical instantiated identities in generated records, including a transparent
type argument alias and distinct implementations reached through interface lists.
Its failure shrinks from three records to two records with zero-valued fields;
losing the owner during field shrinking would instead introduce a dispatch error
and wrongly accept a singleton. Native debug and optimized replay must match the
interpreter's exact counterexample after source removal.

The rejection-path audit found and addressed floating `modulo`, which was
accepted by the checker and interpreter but rejected by native binary validation.
`native_float_remainder_matches_interpreter_in_both_profiles` compares both
widths, signs, signed zero, NaN/infinity/zero-divisor cases, generic functions,
joined tasks, and explicit comptime results after source removal. Its pending
operand companion requires matching diagnostics and exit 71 with owned locals
on both operand sides and both profiles. The runtime leaf test also covers
subnormal values and verifies that NaN results do not set runtime failure.

The aggregate debug-print gate runs public values from five debug fixture
families through `print`/`println` and compares complete native/interpreter
output after source removal. It covers empty never-typed containers, absent
sums, explicit views, pending values, concrete interface owners, and borrowed
arguments spanning handlers. A terminal argument failure must produce no partial
call output and exit 71 after cleanup. Runtime layout tests reject a fabricated
inhabitant of an uninhabited debug node. These tests do not resolve the separate
hidden-secret print policy above.
The deep-value gate compares trace, print, breakpoint, and erased-interface
observation of 160-level recursive values, including pending nodes and secret
leaves, after deleting source. Runtime tests format 4,096-level values and shared
children, reject alias and payload cycles (including dynamic interface layouts),
and preserve output-buffer contents on failed formatting. This removes the
formatter's former 128-level cutoff. A separate deep-enum equality gate compares
debug and release binaries after source removal, including 160-level equal and
unequal payloads, different chain lengths, pending wrappers, NaN, and signed zero.
Runtime equality tests cover 4,096 levels, shared children, malformed cycles,
and short-circuit ordering before invalid later fields. These gates retain the
existing equality type restrictions and do not resolve the open policies above.
The deep-reflection dispatch gate checks 70 nested list arguments both directly
and through a named alias, comparing exact reflected names/kinds in debug and
release binaries after source removal. Runtime identity tests cover 4,096 alias
levels with alias preservation and erasure, shared metadata, cycles, and
malformed alias arguments. These replace the former 64-level identity cutoff.
The deep-interface conversion gate compiles 130 nested list types in debug and
release, removes source, and checks the converted empty container against the
interpreter. Runtime tests additionally convert populated 130-level lists,
inspect the boxed leaf's identity and payload, and inject allocation failures
at outer, middle, and innermost construction boundaries. Every failed conversion
preserves its source and retires partial owners. Truncated and extra descriptor
bytes are rejected before conversion.

The standalone report intentionally keeps `complete` false while these release
obligations are unresolved, even when every fixture count is full. The historical
85% planning estimate is not a measured code-coverage or completion result.

The refined-small-value interface gate executes `nothing`, inherited unit
refinements, booleans, and bytes in runtime and comptime lists, with nested
pending unit joins. It also checks borrowed coarsening/declassification through
direct calls, callbacks, projected fields, and later-argument handlers. Both
debug and release artifacts run after source removal. The companion failure
gate covers direct/indirect calls and both transparent wrappers, requiring the
interpreter's exact terminal error, no partial output, and exit 71 rather than
cleanup failure. MIR and codegen suites validate the shared ownership handoff.

The facade-context audit found a missing ownership-planner case: indirect calls
returning move-only values did not reserve their result temporary. A list built
from several callbacks returning interfaces failed native emission after
exhausting the planned slots. MIR now counts each owned indirect result, while
retaining separate slots for callee descriptors and argument evaluation.
`native_indirect_owned_results_match_interpreter_in_both_profiles` checks records,
enums, bitfields, machines, lists, sets, maps, bytes, optional/result values, and
construction builders. A third callback failure must retire earlier results and
the partially built collection, match the interpreter's error and output, and
exit 71. Both profiles execute after source removal.

`native_interface_math_facade_contexts_match_interpreter_in_both_profiles` checks
all int64/float64 forms of the overloaded abs/min/max facades, secret taint from
either argument, generic wrappers, captured public-value callbacks, wholly baked
results, and comptime-created callbacks invoked at runtime. It verifies concrete
owner dispatch through interface collections and exact output in debug/release
after source removal. These probes strengthen the facade/comptime audit without
settling the remaining equality or erased-call policy decisions.

The refined-sum audit fixes direct optional/result refinement boundaries to
validate their whole declared base before considering extraction. Its linked
matrix preserves erased payload owners across inheritance, fields, callbacks,
pending joins, and runtime/comptime values. Failure/absence cases and integer
result errors pin the independent string refinement-error contract.
The pending-candidate regression also exposed unguarded native sum extraction;
native tag reads now reject outer pending depth before payload transfer.
Debug/release terminal-error tests cover both sum families and branches, nested
pending depth, and an owned record payload after source removal. Runtime tests
verify that failed checks preserve owners and ready sums with pending payloads
remain extractable. Hidden-secret diagnostic observation remains subject to the
unresolved observation policy; native formatting retains its existing redaction.

The wrapped-enum regression closes accepted secret constructor and equality
paths through unit and narrow variants, collection and exact struct payloads,
generic calls, projected fields named like variants, inherited refinements,
secret-backed refinements, nested pending joins, and explicit comptime values.
Debug and release artifacts execute after source removal and match the reference
output. Adjacent mixed public/secret arithmetic and logic preserve result taint;
malformed handoff tests reject lost secrecy and missing exact payload methods.
Debug layout tests retain root redaction and reject nested secret equality graphs.
These checks do not settle secret terminal-diagnostic observation policy.

The namespace-constant handoff materializes literal values and earlier constant
references in declaration order, including unused declarations. All required
calculations, including operators, interpolation, and calls, require explicit
`comptime`; ordinary calculation initializers report E0378. E0377 still forbids
mutable globals. A shared declaration-keyed table supplies the reference,
verify/property, and native drivers. HIR `Constant` reads are baked before MIR
rather than emitted as startup work. Primitive shape checks reject hidden pending
values with E9001.
Debug constant traces retain declared type spelling through a scoped observation
temporary; that temporary does not enter later breakpoint snapshots and release
lowering removes the trace.

The public-driver regressions in `global_constants.rs` cover literal-only
programs, namespace isolation across files, generic and inline reads,
verify/property contexts, required explicit calculations, initializer evaluation,
unused aggregate and pending rejection, and preservation of diagnostics and
existing native outputs.
`native_global_constants_match_interpreter_without_source_in_both_profiles`
provides the linked primitive-constant obligation, including test bodies and
source removal. These regressions must pass against the final revision alongside
the inventory and platform gates; they do not resolve move-only constant ownership
or establish full native completion.

Local checks for this slice pass: compiler-phase suites, all 593 frontend
fixtures, all 182 run-pass backend-lowering cases, ten public constant-driver
regressions, and linked constant programs and verify/property suites after
source removal. Both program profiles are covered. The final revision still
requires the independent workspace, inventory, and platform distribution gates.

The primitive-list sum audit found that native admission accepted only `int64`,
while the reference path also lost narrow-width wrapping and returned an integer
zero for empty floating lists. Both paths now use the checked numeric primitive
width, preserving per-addition binary32 rounding and IEEE infinity/NaN behavior.
Runtime and interpreter regressions cover widths, empty values, and pending
element failures. `native_primitive_list_sums_match_interpreter_in_both_profiles`
adds linked program and verify/property execution after source removal. These
supplemental cases do not change the inventory denominator or establish full
native acceptance.

The pending-aggregate audit found unchecked public field/tag reads and projected
sequence ancestors, plus lost scalar payload depth during enum matching. Native
consumers now reject outer pending owners before observation or transition
payload execution, while raw internal projections remain usable. Scalar enum
bindings retain their pending depth through match and one-level joins.
`native_pending_aggregate_access_fails_before_observation_in_both_profiles`
pins ten exact public reference failures, debug task traces, and successful
failure cleanup. `native_ready_and_joined_aggregate_access_preserves_owners_in_both_profiles`
covers repeated borrowed enum-list matches, ready/projected owners, and fully
joined records, machines, enums, and scalar payloads. Its `int8`, `float32`,
`bool`, and `nothing` enum payloads pin depth-two, depth-one, and ready traces
through successive joins. Both profiles run after source removal. Runtime
checks also preserve nested pending children and typed
redaction. Hidden-secret diagnostic observation remains open; missing debug
layouts use conservative redaction rather than proving public output parity.

The projected-assignment prerequisite handoff rejects immutable, temporary, and
known view roots before execution in functions, inline bodies, actor handlers,
comptime requests, and verify/property bodies. It also rejects rebinding an
owned mutable local from a move-only view, including parenthesized targets,
without changing declaration facts across branches. Owned mutable field updates
still need the open execution contract above. The known borrowed return and
owned argument checks below extend this prerequisite; named view-type aliases
and general call-produced borrow provenance remain separate ownership work.

`projected_assignment.rs` pins frontend diagnostics, rejection before evaluation,
and preservation of existing native outputs for both profiles and test suites.
`native_projected_reads_and_reconstructed_owners_match_interpreter_in_both_profiles`
executes reads, owned copies, and construction/rebinding after source removal,
including verify and property suites. Local validation passes all 179
typechecker tests, all 594 frontend fixtures, all 182 run-pass lowering cases,
four public driver regressions, and five linked projected-value regressions.
These checks establish the prerequisite handoff, not support for field updates
or full native parity.

The native list-ordering audit found three additional accepted-domain gaps:
primitive-backed refinement sorts failed admission, narrow numeric sortedness
and indexed sorting selected no comparison carrier, and pending strings/rows
could be ordered by their hidden payload. Sort and comparison carrier selection
now follows refinement bases at every numeric width while retaining the exact
checked list type. Summation stays primitive-only. Pending strings, scalar keys,
and rows compare as equal before payload access; ready floating sortedness uses
IEEE `<=`, preserving empty/singleton and pending-pair behavior for NaN.

`native_list_sort_refinements_and_ordering_match_interpreter_in_both_profiles`
executes both ordering fixtures in debug and release and runs their native
verify/property suites after source removal. It includes local explicit-comptime
refined numeric/string lists, nominal method dispatch, numeric limits and
rounding, duplicates, empty/singleton cases, NaN/infinity, indexed payload
markers, and pending depth through joins. The separate outer-pending regression
matches the public failure and exit 71 in both profiles. All 66 codegen tests,
122 runtime unit tests and its integration suites pass, as do the existing
pending-list and primitive-sum linked regressions. This closes these ordering
mismatches without changing the inventory denominator or resolving the open
summation domain.

The native math-aggregate audit found that `math.average` and `math.median`
accepted all ten primitive numeric list types in the checker and reference path,
but native admission handled only the three wider carriers. Both native APIs
now admit the complete checked primitive domain and transparent aliases, retain
their exact `float64` result, and validate carrier bits before decoding. Signed
narrow values extend with their sign and binary32 values widen after rounding;
the existing binary64 aggregate algorithms and input ownership remain unchanged.
Refinements, secret wrappers, and nonnumeric elements remain rejected.

`native_math_aggregate_primitives_match_interpreter_in_both_profiles` compares
all widths, aliases, ordinary generic calls, explicit-comptime results, extrema,
large unsigned values, binary32 precision, and IEEE behavior. Program binaries
in debug/release and native verify/property suites run after source removal.
Its failure companion pins 20 typed-empty cases and six outer/first/later pending
cases against exact interpreter output, diagnostics, and exit 71 in both profiles.
All 68 codegen tests, 126 runtime unit tests and its integration suites pass,
alongside the existing linked pending-math and extreme-value regressions.
These supplemental regressions do not change the inventory denominator or
settle the separate summation and other open policy gates.

The uninhabited-iteration audit found accepted loops over inferred empty lists
and maps failing native admission on their `never` bindings. HIR now omits
lowering these checked bodies while retaining iterable expressions and reserved
generic identities. Native sequence preparation preserves one-time evaluation,
pending and projected-ancestor checks, and loan cleanup, then bypasses extraction.
Only changed functions have unreachable blocks and unused locals compacted;
retained spans, debug types, parameters, captures, and function identities remain
intact. Dead concrete generic specializations with bare `never` signatures are
excluded only from implicit project roots. Retained references still reach them
and cannot acquire a fabricated runtime carrier.

`native_empty_collection_iteration_matches_interpreter_in_both_profiles` pins
ordinary generic calls, dead named/inline callbacks and debug events, borrowed
and consuming loops, once-only argument effects, nested collections, contextual
concrete empty values, joined inputs, and explicit-comptime results. Debug/release
programs and native verify/property suites execute after source removal. Four
runtime/comptime pending-list/map cases retain exact output and errors with exit
71; a public-driver companion rejects invalid arithmetic, interpolation, and
unresolved calls in dead bodies before lowering. Malformed MIR controls preserve
invalid IDs and inline metadata for validation rather than hiding them in pruning.
All 69 HIR tests, 32 MIR unit tests and its integration test, 71 codegen tests,
and eight backend-lowering regressions covering all 182 run-pass cases pass.
These checks retain the inventory denominator and do not establish full native
acceptance or settle the remaining open policies.

The uninhabited-handler audit found optional/result handlers attempting to
extract impossible payloads, and generated callbacks retaining unused parent
locals whose types have no native carrier. Native preparation selects the sole
inhabited arm only for an exact checked `SumTag` branch. It retains source
evaluation, snapshots, and pending validation, then compacts changed graphs.
Explicitly generated inline functions also compact unused parent locals and
become reachable through retained calls and descriptors. Original structural
validation protects pruned graphs; global debug labels remain validated even
for omitted functions.

The proof depends on directional frontend compatibility: concrete values cannot
populate `never` through defaults, declarations, calls, constructors, returns,
or required comptime evaluation. Mixed collections and repeated generic
arguments fill compatible absent slots in either order without promoting
numeric widths, erasing nominal types, or weakening callable variance and view
modes. Contextual handlers preserve producer signatures and convert inhabited
nested payloads explicitly; empty owned containers receive their target
ownership flags. Cloneable local sums snapshot their underlying source before
conversion, preserving owned and borrowed reuse.

Generic reflection now retains declared nested alias witnesses through
identifiers, explicit views, parentheses, and concrete named/inline callback
signatures. Root transparent aliases still peel, and repeated concrete witnesses
follow declaration parameter order, including named arguments. The reference
evaluator fills nested `never` slots using the same variance and source-witness
rules. Field, returned-call, and clone-expression witness recovery remains a
separate metadata audit; this handoff does not claim those expression forms.

`native_uninhabited_sum_arms_match_interpreter_in_both_profiles` covers absent
handlers, empty reverse, dead and surviving callbacks, once-only effects,
contextual interfaces and function values, owned/view source reuse, nested
pending joins, mixed collection inference, alias reflection, and required
comptime values. Debug/release programs and native verify/property suites run
after source removal. Its pending companion covers eight runtime/comptime
sum/reverse failures with exact output and exit 71 in both profiles. Public
driver controls reject invalid dead arms and manufactured `never` values before
execution or native publication, retaining existing output files. These
supplemental cases do not change the 207-obligation inventory denominator.

Local validation passes all 195 checker tests, 325 reference/comptime tests,
74 HIR tests, 46 MIR unit tests and its integration test, and 73 codegen tests.
All 594 frontend fixtures and eight backend-lowering regressions covering all
182 run-pass files pass. All 182 inventory object obligations emit nonempty
native objects, with deterministic repeat-emission controls passing. The four
new native regressions and eleven adjacent native tests pass, including both
profiles and native verify/property execution after source removal. Formatting
and whitespace checks pass. Complete workspace, inventory execution, and
platform distribution results for the final revision remain independent gates;
this handoff does not establish full native parity.

The reflected-field owner audit found that an admitted read of an
interface-backed refinement as its base interface reused the base box and lost
the refinement's method implementation. Native record, enum, and machine reads
now select conversion metadata from the actual declared field type and run the
conversion after the existing metadata, requested-type, and pending checks.
Conversion borrows and clones the source, preserves pending depth, and produces
one owned result.
Ordinary and reflected secret-qualified destinations choose the underlying
nominal owner after removing only matching outer secret qualifiers. Unqualified
interface destinations still select explicit secret-owned implementations;
additional source qualifiers and nested secret payload redaction remain intact.

All 78 codegen tests pass, including five unit controls pinning owner selection,
unchanged admission, and nested redaction. Both new native conformance tests pass.
`native_reflected_interface_field_owners_match_interpreter_in_both_profiles`
covers direct/reflected dispatch, exact and ancestor reads, runtime/comptime
values, source reuse, pending joins, qualified secrets, and native verify/property
suites after source removal.
`native_reflected_interface_field_failures_match_interpreter_in_both_profiles`
checks requested-type, active-member, and pending-metadata failures in both
profiles. All 19 adjacent interface conformance tests, three deep-conversion and
reflection tests, and eight pending-reflection tests pass, including existing
explicit secret-owner implementations.
These supplemental cases leave the inventory denominator unchanged and do not
settle the open reflected admission contract or establish full native parity.

The qualified interface callback audit found that `Reader.read` checked as a
function value but became a non-function in the reference evaluator and lacked
a native dispatcher target. Checked method values now distinguish concrete
source bodies from interface slots. Callback-only slots are collected through
ordinary, generic, and nested reflected bodies and resolve to the existing
dispatcher with exact parameter types and view modes. Declared slots remain
available without an implementation, and creating a callback executes no body
or authority. Runtime receivers still select the existing implementation.
Transparent aliases resolve in declaration scope, including when a caller's
namespace import or generic parameter has the same name as the target.

An implementation of another interface for an interface type can share the
declared method's qualified display name. The declaration's slot takes
precedence during reference lookup, invocation, and explicit compile-time
callback materialization; failure to select an implementation cannot fall back
to the colliding source body. The source implementation retains its distinct
native identity.

All 201 checker, 332 reference/comptime, 77 HIR, 47 MIR, and 594 frontend fixture
tests pass. The two
new native conformance tests pass in debug and release after source removal,
including compiled verify/property suites, aliases, multiple owners and slots,
explicit receiver reuse, callback adapters, generic and compile-time values,
capability parameters, argument-before-callee effects, canonical debug names,
name collisions, and pending-receiver failures. All 13 adjacent linked function
value tests and 19 adjacent interface tests pass. Complete inventory, workspace,
and host distribution gates
remain independent obligations for the final revision; the supplemental
callback fixture does not change the inventory denominator or settle the
concrete-owner argument policy for erased calls.

The borrowed-boundary audit found known move-only views that could be returned
or passed to owned parameters through local aliases and unchecked body
contexts. The checker now reports E0401 for these boundaries across ordinary,
generic, indirect, and pipeline calls, preserving view facts through
parentheses, coarsening, and declassification. Explicit written view arguments
retain E0375. Direct return-view annotations also report E0401, including unused
generic headers and callable results; the check does not expand named aliases
or prewalk unused generic bodies.

`view_owned_returns.jett`, `view_owned_arguments.jett`,
`view_explicit_owned_modes.jett`, and `view_return_annotations.jett` pin these
diagnostics.
`view_escapes::borrowed_values_cannot_escape_before_program_or_suite_execution`
checks rejection before runtime, test suites, and explicit comptime evaluation.
`view_escapes::borrowed_value_escapes_preserve_existing_native_publications`
checks existing program outputs in both profiles and compiled test suites.
`native_borrowed_return_clone_controls_match_interpreter_in_both_profiles` uses
`borrowed_return_clone_controls.jett` for source removal, exact output, compiled
verify/property suites, cloned returns and owned arguments, scalar/string
copies, owned field copies, cloned coarsening/declassification, and closed
comptime.

Focused validation passes all 213 checker, 332 reference/comptime, 77 HIR,
47 MIR, 78 codegen, and 598 frontend fixture tests. All eight backend-lowering
tests pass, including the 182-fixture run-pass obligation. Both new public driver
regressions and the linked clone-control regression pass, including debug and
release after source removal and compiled verify/property suites.

These supplemental regressions do not change the inventory denominator or
establish full view support. At that checkpoint native local view-alias lowering
remained unsupported; the frontend checks added no lifetime or projection policy.

The direct contextual secret-constructor gap is resolved for source
`list(...)`, `map(...)`, `some(...)`, `none`, `ok(...)`, and `fail(...)` with
checked outer `Secret` qualification. HIR builds the exact inner container,
then restores the original checked TypeId and span with the existing
`InterfaceCoerce`. Payload interface, function, and inferred `never`
conversions remain unchanged; nominal refinements and arbitrary producers are
untouched. This correction adds no policy and keeps strict native constructor
shape checks.
Declared outer-secret local types are also retained, including repeated
wrappers, while less-qualified initializers keep their checked producer types
and use the existing initialization coercion. This closes the local/read
metadata mismatch without extending contextual inference or changing the
one-layer `declassify` rule.

Focused constructor-normalization HIR validation has passed.
`native_contextual_secret_constructor_failures_preserve_order_and_cleanup`
passes five payload-failure cases in debug and release after source removal,
preserving lexical effects, the terminal failure, and cleanup.
`native_contextual_secret_constructors_match_interpreter_in_both_profiles` is
the success regression for `contextual_secret_constructors.jett`. It passes
in debug and release after source removal with all nine stdout lines matching
the interpreter, root-secret debug redaction, and release trace omission. The
compiled verify suite and 100 property trials also pass. All 82 HIR, 47 MIR,
332 reference/comptime, and 78 codegen tests pass with the declared-local
correction. All eight backend-lowering tests also pass, including the 182-file
run-pass obligation. Complete workspace, inventory, and host distribution gates
remain independent obligations for the final revision.

Direct and forwarded immutable local view aliases now have a bounded native
handoff. Checked binding modes retain immediate resolved origins separately in
ordinary, generic, and reflected bodies. HIR validates immutable backing chains
and allocation-free carrier-preserving initializers. MIR remaps and retains
origins, expands every alias read into backing-owner liveness, and excludes
aliases from cleanup slots. Owner consumption or rebinding after reachable
alias creation remains a conservative native implementation error; this does
not settle source loan expiry or temporary lifetime rules. The remaining domain
is recorded in [the local alias note](native_local_view_aliases.md).

Formal view modes survive direct and indirect handler staging. Copy-owned
string and callback observations acquire independent snapshots where needed;
ordinary clones and implicitly copied results acquire owning temporaries before
entering containers. Borrowed arguments retain their existing backing handle.
Plain string view expressions keep their existing copy behavior. Borrowed
enum matches snapshot the scrutinee before payload extraction.

`local_view_aliases.jett` and its two linked tests pass in debug and release
after source removal. The success test pins fourteen stdout lines and four
debug traces, including secret redaction and release trace omission. It covers
list, bytes, string, struct, interface, enum, and function aliases; generic and
reflected bodies; sibling scopes, loops, pending values, clone independence,
handled later arguments, and container copies that outlive their helper frame.
The compiled verify suite and 100 property trials pass. The terminal-failure
test preserves lexical output and the exact error with exit 71 in both profiles,
proving successful checked cleanup rather than a cleanup-error override.

All 217 checker, 89 HIR, 59 MIR, 332 reference/comptime, and 78 codegen tests
pass, as do all 598 frontend fixtures and eight backend-lowering tests, including
the 182-file run-pass obligation. Thirteen adjacent function-value tests, nine
handler tests, and three pending-string tests also pass. These supplemental
fixtures do not change the inventory denominator or establish full workspace,
inventory execution, or supported-host distribution parity for this revision.

Strict native graphics decoding now preserves the reference distinction between
terminal shape errors and handled domain failures. The reproduced `width: run 0`
case previously read zero as a ready dimension and returned a handled failure;
it now reports the exact terminal Config width error before rendering or provider
use. The decoder checks pending record owners, field metadata, strings, list
containers, and record elements before payload access, in reference field order.
Complete decoding precedes domain bounds, and window dimensions are read only
after successful Config validation. Existing ABI leaves and callback state
metadata remain unchanged.

Four linked regressions in `native_conformance/pending_graphics.rs` pass in debug
and release after source removal. Their 53-case matrix covers Config, Scene,
Color, Rect, and Text roots and fields; Scene list containers/elements; nested
pending depth, partial joins, closed comptime values, competing errors, and ready
domain failures. Initial and later render failures preserve lexical stdout and
the exact terminal error with exit 71, including unused scripted input. The fully
joined session control retains pending State across callbacks, pins three debug
traces and release omission, and passes compiled verify and 100 property trials.

Seven focused runtime regressions additionally check all 38 pending metadata
sites at depths one and two, malformed payload guards, source preservation,
one-layer joins, provider ordering, and unfinished-session cleanup. All 145
runtime tests, 78 codegen tests, nine linked graphics tests, and fourteen graphics
frontend/reference integration tests pass. These supplemental cases leave the
inventory denominator unchanged; full workspace, inventory execution, and
supported-host distribution gates remain independent final-revision obligations.

Forwarded pending Graphics authority now fails at kernel entry before Config
decoding or domain validation. The source-admitted helper probe previously
returned a handled width-zero failure and continued execution; it now preserves
the reference terminal `graphics.__run expects a Graphics capability` error.
The emitter reuses the existing capability pending-depth leaf after lexical
argument evaluation. It neither consumes nor implicitly joins authority and
adds no ABI operation or callback policy.

Three linked regressions in `native_conformance/graphics_authority.rs` pass in
both profiles after source removal. The minimal probe and eleven companion
cases pin nested depth, partial joins, capability-before-Config error precedence,
ready/final-joined authority, and explicit-view reuse. Terminal outcomes require
exact stdout/stderr and exit 71. A separate successful control opens and closes
two scripted sessions with the same fully joined authority, proving that its
context-bound identity remains usable at the provider boundary. All twelve
linked graphics tests and 78 codegen tests pass with the guard. Supplemental
coverage leaves the inventory denominator unchanged and does not replace the
workspace, inventory, or supported-host distribution gates for this revision.

The locked Windows workspace run at `22fcbf13` passed all inventory gates:
182 HIR/MIR fixtures, 182 native objects, 30 entry cases, 25 runtime contracts,
155 verify fixtures, and three property fixtures with 100 trials each. Its
native conformance target reported 259 passes and three failures; all other
workspace targets and doc-tests passed. The three failures exposed borrowed
`coarsen` initializers that MIR had replaced with owned snapshots, invalidating
their checked backing origins.

MIR now preserves only local alias initializers accepted by the shared typed
origin validator. Borrowed `coarsen`, `declassify`, and forwarded aliases retain
their backing handles; ordinary owned expressions still acquire snapshots.
Malformed clone, handler, and different-source initializers remain rejected.
The earlier projected-local admission error is pinned directly, separately from
source escape, conflicting borrow, and moved-owner errors. A portable debug and
release regression verifies rejection before launcher lookup and preserves an
existing executable.

All 62 MIR tests, 78 codegen tests, 21 linked interface tests, both public
view-escape tests, and four local alias regressions pass with this repair.
The linked nonstring qualification regression runs in both profiles after
source removal, checks independent returned clones and unchanged backing
owners, and requires the exact later terminal failure with successful cleanup.
The Linux-only projection-loan test
expectation is updated but requires Linux validation. This repair does not
change the inventory denominator or extend native view origins; full workspace
and supported-host distribution results must still be established for the
final revision.

The implicit display-result gap is resolved at its interpolation boundary.
The reproduced method returning `run "shown"` previously printed
`value:pending(shown)` natively while the reference interpreter stopped after
`before` with `Displayable.display returned pending(shown) instead of string`.
HIR now carries `DisplayResult` around only the implicitly selected call.
MIR preserves it inside staged segment initializers, and codegen checks its
already owned string through a borrowed runtime leaf before later segments.
Exact-string validation, reachability, ownership planning, pruning, and native
constant baking retain the marker without adding an owning slot.

Three linked regressions in `native_conformance/display_results.rs` cover the
minimal failure, six terminal cases, and successful controls in both profiles
after source removal. Terminal cases pin nested depth, Unicode and empty text,
partial joins, primitive and erased-interface owners, method-failure precedence,
once-only debug output, suppressed later handlers, exact stdout/stderr, and
exit 71 with cleanup. Success controls pin ordinary pending-string formatting
from explicit calls, fully joined implicit and generic display, namespaced
implementations, receiver reuse, closed comptime, compiled verify, and 100
property trials.

All 91 HIR, 64 MIR, 80 codegen, 151 runtime, and 332 reference/comptime tests
pass. Five focused runtime tests and the independent C ABI regression preserve
string handles and reference counts, pending layers, exact dynamic failures,
first-error status, and terminal cleanup. Supplemental cases leave the
inventory denominator unchanged; complete workspace and supported-host
distribution validation remain final-revision obligations.

The locked Windows workspace run at `7cdf24b1` passed every workspace target
and doc-test. Native conformance reported 267 passes with no failures. All
inventory gates passed: 182 HIR/MIR fixtures, 182 native objects, 30 entry
cases, 25 runtime contracts, 155 verify fixtures, and three property fixtures
with 100 trials each. Source remained frozen throughout the run. Linux-only
tests and both supported hosts' clean package jobs still require CI results for
the final revision; a local Windows pass does not substitute for those gates.
The [supported-host run for `7cdf24b1`](https://github.com/vycdev/jett/actions/runs/36872780921)
has since completed successfully: both Linux and Windows workspace/build/package
jobs and both clean installed-package jobs passed. That result proves the prior
checkpoint's distribution gates, not those of a later compiler revision.

Additional source probes at that checkpoint found two gaps outside the fixed inventory. A direct
struct `Equatable.equals` returning `run true` is accepted by the checker but
the reference interpreter reports `Equatable.equals must return bool`. Native
`==` instead prints `pending(true)` and exits successfully; native `!=` reports
`'not' requires a boolean operand` with exit 71. Secret-qualified operands show
the same equality-result gap when the secret is initialized from a ready local.
The enum payload callback already checks result depth; the direct operator
call needed its own preserved result boundary before inequality negation.

The direct equality-result gap is now resolved. HIR wraps only implicitly
selected struct equality calls in `EquatableResult`; MIR preserves that boundary
through operand and later-handler staging. Codegen checks pending depth with the
existing `RejectPendingScalars` leaf before using the boolean or negating it.
The child's exact checked boolean type, including leading secret qualification,
is retained. Explicit method calls remain ordinary task-producing calls. No
runtime ABI operation or owned temporary is added, and malformed boolean,
qualification, and nominal-refinement metadata remains rejected.

All 93 HIR, 66 MIR, 82 codegen, and 332 reference/comptime tests pass. Three
linked regressions in `native_conformance/equality_results.rs` cover both
operators in a minimal reproduction, 16 terminal matrix cases, and successful
controls. Debug and release binaries run after source removal. Failures pin
true and false payloads at depth one and two, partial joins, exact method-error
precedence, left-to-right effects, once-only traces, suppressed later handlers
and short-circuit effects, secret-qualified operands, exact stderr, and exit 71
with successful cleanup. Successful controls cover ready and fully joined
implicit equality, ordinary explicit pending calls, unchanged owners,
namespaced generic and reflected comparisons, closed comptime, compiled verify,
and 100 property trials. Six adjacent linked enum regressions also pass.
Supplemental cases do not change the inventory denominator; the prior locked
workspace result remains specific to `7cdf24b1`, and the final revision still
requires workspace and supported-host distribution validation.

The separate direct `secret[Item]` constructor admission gap is also resolved.
Its checked construction previously retained the outer secret type without an
inner constructor/qualification split, and native verification correctly
rejected the inconsistent metadata. HIR now normalizes only source calls with
their checked struct-construction fact and an exact, nonvalidating nominal
target. Leading secret qualifiers are restored through the existing coercion
after construction; field conversions, source order, pending depth, and owner
identity remain intact. Validating result constructors, nominal refinements,
ordinary producers, and reflected builder calls are preserved unchanged.

All 96 HIR, 66 MIR, and 83 codegen tests pass. Three focused HIR tests cover
direct/local/generic/specialized and parenthesized constructors, nested
qualification and `run`, interface/function/refinement payloads, handlers,
source construction provenance, unchanged producers and validating results,
and strict normalization controls. The object regression independently rejects
raw secret construction, a qualified inner constructor, a different nominal
target with the same fields, and invalid refinement-validation metadata.

Two linked regressions in `native_conformance/secret_struct_constructors.rs`
pass in both profiles after source removal. Successful controls pin eight
handwritten output lines and root-secret trace redaction, narrow fields,
generic and nested constructors, function and inline returns, full joins,
closed comptime, refinement-backed interface dispatch, callback adaptation,
independent owned field copies, unchanged public producers, compiled verify,
and 100 property trials. Three terminal field cases pin reversed named-field
evaluation order, the exact earlier error, suppressed later effects, and exit
71 cleanup. Both existing linked contextual collection/sum constructor
regressions also pass. Neither repair changes the inventory denominator or authorizes a
broader equality or constructor policy. Final-revision workspace and
supported-host distribution validation remain required.

Direct field reads from secret-qualified aggregates now pass native admission.
A minimal ready `Item` promoted to `secret[Item]` previously printed `7` in the
reference backend, while native verification rejected `hidden.value` with
`field owner mismatch`. The verifier now mirrors the existing checker rule:
one direct secret wrapper around the exact struct, bitfield, or machine-state
owner; unchanged `nothing` and already-secret field types, including nominal
secret-backed refinements; otherwise exactly one wrapper around the declared
field TypeId. Nominal owners, indexes, and result qualification remain strict.
Emission, pending checks, machine-state indexing, and field ownership paths
are unchanged; no ABI operation is added.

All 84 codegen tests pass. The new object regression rejects eight malformed
field contracts, including erased secrecy or nominal identity, added or doubled
secrecy, qualified `nothing`, different owners, invalid indexes, and nested
secret receivers. Three linked regressions in `native_conformance/secret_fields.rs`
pass in both profiles after source removal. Successful controls pin eight
handwritten output lines, narrow numbers and Unicode strings, lists, refined
interface owners and captured callbacks, already-secret fields, `nothing`,
secret-backed nominal fields, bitfield and state-qualified machine reads,
generic/reflected controls, owned field copies surviving their helper scope,
independent list mutation, nested child depth and joins, closed comptime,
compiled verify, and 100 property trials. Pending-child failures preserve exact
depth, first-error precedence, and exit 71 cleanup.

Pending secret receiver errors intentionally retain native typed redaction.
Depth-one, depth-two, and partially joined receiver regressions pin the raw
reference error separately from native `[redacted]`, with exact output and
cleanup in both profiles. Those tests do not claim diagnostic parity or resolve
the [hidden-secret observation policy](../open_design/debug_print_hidden_secrets.md).

A separate accepted-source probe still exposes direct contextual secret
bitfield construction: `secret[Header] hidden = Header(value: 7)` runs and
prints `7` in the reference backend, but HIR reports `checked bitfield
construction has an invalid type`. Public construction followed by secret
qualification works, as the linked field suite demonstrates. This is an
implementation gap outside the fixed inventory; width-validating constructors
and machine construction need their own evidence before extending that repair.
Final-revision workspace and supported-host distribution gates remain open.

The locked Windows workspace run at `65c6ba68` passed every workspace target
and doc-test with source frozen throughout. All 275 native conformance tests
passed, including the eight new linked equality, secret-struct-construction,
and secret-field regressions. The independent inventory gates also passed:
182 HIR/MIR fixtures, 182 native objects, 30 entry cases, 25 runtime contracts,
155 verify fixtures, and three property fixtures with 100 trials each.
This checkpoint does not certify a later revision or Linux-only execution;
the [supported-host run for this revision](https://github.com/vycdev/jett/actions/runs/36884693314)
has since passed both Linux and Windows workspace/build/package jobs and both
clean installed-package jobs. This certifies that checkpoint's distribution,
not a later compiler revision.

The contextual secret bitfield and exact-state machine constructor gap is now
resolved. HIR peels only leading secret wrappers while selecting an already
resolved declaration, retaining the exact nominal bitfield or machine/state
identity. Plain bitfields retain their value output; validating bitfields retain
`result[Bitfield, string]` and their width-validation flag. The existing coercion
restores the original checked secret type after construction. Field conversions,
lexical evaluation order, pending depth, result tags, and ownership remain intact;
native verification and the runtime ABI are unchanged.

All 99 HIR, 66 MIR, 85 codegen, and 598 frontend fixture tests pass. Three new
linked regressions cover literal and dynamic bitfields, successful and failed
width validation, exact machine states, nested qualification, parentheses,
generic bodies, namespace and destination aliases, returns, inline callbacks,
full joins, closed comptime, unchanged public producers, owned payload copies,
machine interface/callback payloads, compiled verify, and 100 property trials.
Seven handwritten output lines pin the successful entry behavior. Debug and
release binaries execute after source removal. Reversed named arguments and
handled machine payloads preserve source order; terminal field failures suppress
later effects and result handlers, retain the exact first error, and exit 71 with
successful cleanup. The object test rejects 19 malformed nominal, result,
validation-flag, state-index, payload, and qualification contracts. Three adjacent
bitfield regressions, machine-transition and baked-machine regressions, and both
contextual collection/sum constructor regressions also pass.

The contextual secret validating struct constructor gap is resolved. HIR moves
leading secret qualification outside the exact `result[Struct, string]`
constructor while preserving its checked construction fact, nominal success
type, validation flag, predicate chains and lexical field order. MIR's generated
sums now retain their ordinary result type inside the qualification coercion.
Empty predicate chains keep the constructor's result shape; ordinary producers
and nonmatching nominal or result types are unchanged.

The HIR and strict object regressions pass, including malformed outer/inner
qualification, different nominal success types, wrong error or success types,
validation flags and erased result shape. Three linked regressions in
`native_conformance/secret_validating_struct_constructors.rs` pass in both
profiles after source removal. Eight handwritten successful output lines cover
generic/nested/parenthesized constructors, returned and inline values, public
producers, closed comptime, full joins and independent string/list copies.
Compiled verify and 100 property trials pass. Predicate failures retain captured
result data and source order; field evaluation failures remain terminal, precede
later predicates/handlers and exit 71 with successful cleanup.

Qualified result joins now preserve their checked outer boundary in both
backends. Exact results retain their type; secret-qualified and nominally
refined results receive an additional result around the joined payload. The
interpreter also reconstructs Join types from the active checked facts before
declared fallback, so refinement handlers extract that outer result correctly.
The native move planner admits only explicit-view borrowing for Join's owned
copy and preserves ordinary view-consumption and escape rejections.

All 339 interpreter, 69 MIR, 100 HIR, 86 codegen and 598 frontend tests pass.
Three linked regressions in `native_conformance/qualified_result_tasks.rs` pass in both
profiles after source removal. Handwritten Unicode output and typed debug
observations pin exact/qualified/refined/aliased results, generic and reflected
bodies, nested secrecy, closed comptime, one-layer joins, independent list
copies, explicit-view owner reuse, pending scalar children and nonstring inner
errors. Compiled verify and 100 property trials pass. The inner failure handler
remains separate from the outer Join handler; its later terminal failure retains
the first error, effect order and exit 71 cleanup. No runtime leaf signature,
verifier type rule or fixture denominator changes.

The direct local and struct-field predicate reuse repair follows the selected
design: interpreter validation captures the original checked source type before
evaluation or primitive normalization. Exact refinements reuse their invariant,
and ancestor promotion runs only the remaining base-first predicates. Handled
optional/result success payloads retain their checked proof; whole nominal sum
refinements remain distinct. Coarsening supplies its selected output proof, and
a sibling sharing the carrier proves nothing. Forced validation of changed
property candidates and metadata-free calls remains on its existing path.

The same audit exposed generic constructor type selection: reference execution
returned a plain struct for `Box[Positive]`, while a caller's unrelated
`T = Positive` made `Box[bool]` validate `true` as Positive. Constructor arguments
now resolve in caller context and map to the declaration's own parameters.
Field resolution, validation flags, normalization, and retained identities use
the declaration context; caller imports cannot redirect canonical field names.
The selected concrete fields determine the existing checked result contract,
including an empty predicate chain. Caller context is restored after success,
captured predicate failure, or terminal construction failure.

All 350 interpreter tests pass, including eleven new proof, normalization,
context, and generic-constructor controls. Four linked regressions in
`native_conformance/refinement_reuse.rs` pass in debug and release after source
removal. Handwritten stdout and trace expectations pin exact/local/field reuse,
ancestor suffix order, explicit coarsening, owned string/list copies, one-layer
pending joins, captured predicate errors, and rejection of pending inputs to new
predicates. Concrete generic result shapes, closed comptime, compiled verify,
and 100 property trials also pass. These additions retain the fixed inventory
denominator and require final-revision workspace and distribution validation.
The existing native shrink regression for refinement chains and nested
predicates also passes in both profiles, preserving validation of changed
candidates and caught predicate errors.

Checked named function parameters and exact returns now retain their original
expression proofs. Arguments and proofs are captured before evaluation and
permuted together for named calls and source pipelines. Actual return-expression
facts travel with return signals through nested handlers; destination annotations
and runtime labels never supply that proof. Raw call entry points still force
parameter and return validation, even with a checked body map installed. Proof
reuse stays disabled throughout their nested invocation tree, including calls,
locals, constructors, callbacks, and forced validators. Four additional unit
controls reject forged refined struct children and list elements and check mode
restoration after success and failure. Checked program and zero-argument legacy
verify entries use an explicit bridge without synthesizing argument proofs.
Reflected builder finish retains its documented validation of provided values.

Seven new interpreter tests and four linked regressions in
`native_conformance/refinement_calls.rs` pass. Debug and release programs run
after source removal with handwritten output and trace expectations for scalar,
string/list, method, NamedFunction, generic/reflected, pipeline, named-order,
handler/default, and pending-depth cases. Pure closed comptime, compiled verify,
100 property trials, and forced builder completion also pass. Inline descriptors
now retain their declared return annotation in captured lexical context.
Seven additional interpreter tests cover invalid fresh inline results before
local, argument and return consumers; exact and ancestor proofs; owned Unicode
strings and lists; pending depth; generic/reflected and namespace alias scope
restoration; primitive normalization; and forced raw callbacks. Host-generated
callbacks retain their separate forced-call path; these tests do not establish
proof reuse for that scope.

The proof audit found that the private list sum fallback can produce raw zero
under a checked Positive result annotation. Source execution now forces
validation of the fresh result against its actual intrinsic type argument before
that annotation becomes reusable proof. Actual arguments resolve before nested
evaluation. Four unit tests cover invalid/valid results, pending and mixed-type
errors, second-generic selection, primitive aliases, ordinary and pipeline
consumers, raw/no-metadata behavior, and partial checked maps. A driver control
pins empty Positive summation's reference constraint failure and native codegen
refusal in both profiles, preserving an existing publication before launcher
lookup. This repairs proof safety without selecting the unresolved refined-sum
domain or widening native admission.

Ten reflected producer unit tests cover root refinements and admitted ready
builtin wrappers across record, enum, and machine fields. Shape preflight finishes
before predicates; only occupied arms and collection members are checked, in
their existing order. Declared schemas preserve exact invariants and skip proven
ancestor prefixes. Exact roots retain nominal identity and pending depth without
rechecking. Changed nested pending, secret, callable, and nominal generic requests
fail conservatively before publishing proof; diagnostics do not render values.
Raw and metadata-free field readers preserve their existing consuming boundaries.
Five frontend-admitted source probes now reject zero-as-Positive before optional,
list, declassify, callable, or nominal-field consumers. This reference safeguard
does not select broader reflected admission or certify native producer predicates.

Fourteen additional boundary and canonical-type unit tests preserve written view
validation, resolved primitive widths, generic parameter identities, captured
closure metadata, reflected bindings, inferred call/debug types, and caller
imports on success and failure. Written aliases resolve once; installed and checked
types keep their canonical owners. Fresh CLI runtime and closed comptime probes
pass under colliding imports, including enum and machine payload loops and a
closure fixture containing Unicode strings and owned lists. All 396 interpreter
tests and all 598 frontend fixtures pass. The four linked call regressions and the
refined-sum refusal control also pass against this combined source revision.
Final workspace and supported-host distribution validation remain required.

Reference reflected reads now also reuse a stronger declared refinement as proof
of its requested root ancestor. The lookup follows only the actual declaration's
root chain, preserves normalized result identity and pending depths, and never
uses a runtime label or descends through an outer qualifier or child schema.
Three regressions cover all three getters, both metadata modes, transparent
aliases, owned Unicode values, source preservation, pending depths 0/1/2, and
raw, disabled, unrelated-fact, and forged-label controls. All 399 interpreter
tests pass. A fresh CLI success probe matches the independent scalar, owned,
pending, operand-order, and predicate-trace oracle, including five reciprocal
ancestor reads with no extra predicate. Native execution remains a separate
producer-validation obligation.

Native returns now establish newly requested root refinements before exposing
their checked result type. Returning `-1` under Positive, or a Positive value of
7 under High constrained above 10, rejects with the reference error. The original
checked expression supplies source proof before coercion: exact returns skip
predicates, and ancestor returns check only their missing suffix. Concrete
generic/method signatures select destinations; nested inline targets restore the
enclosing function, and reflected body facts keep the selected return target.
MIR stages the candidate once, preserves pending depths, and forwards the first
predicate failure through a private exact-string `RuntimeFailureMessage` boundary
and the existing borrowed runtime failure leaf. Owned candidates and error strings
retain their cleanup slots. No source spelling, ABI operation, or broader outer
secret/container return admission is added.

Eleven focused HIR, MIR, and object-emission tests and all 214 HIR/MIR/codegen
unit tests pass. Three linked return regressions and two inline regressions pass
in debug and release after source removal, with handwritten scalar/ancestor,
owned Unicode/string/list, generic/method/reflected, alias capture, pending,
predicate-error, operand-precedence, and exit-71 cleanup expectations. Closed
comptime, compiled verify suites, and 100-trial property suites also pass.
These five supplemental tests bring native conformance to 298 tests without
changing the fixed acceptance inventory. Full workspace and supported-host
distribution checks still need to certify the final revision. Return proof reuse
cannot replace validation at a reflected producer.

Native reflected root-refinement producers now validate their selected cloned
field after the original getter completes its selector and payload checks. Exact
and stronger declared sources reuse established proof; ancestor promotions
evaluate only their missing suffix, and sibling or base sources check the whole
requested chain. HIR retains complete per-slot plans and exact canonical
predicate identities. MIR consumes those plans before exposing its internal
raw-read marker, preserving owned cleanup, pending depths, source identity, and
the first original error. Requested admission and outer secret/callable/nominal
conversion rules are unchanged.

Fourteen focused HIR, MIR, and object tests, all 224 HIR/MIR/codegen library tests,
and all 55 object-emission tests pass. Three linked regressions pass through both
direct and piped getter forms in debug and release after source removal,
including all three getters, dynamic selectors, thirteen terminal failure cases,
owned Unicode/string/list values, unchanged sources, pending depths 1/2,
operand staging, closed comptime, verify, and 100-trial property execution.
The pipeline helper shares the direct getter's metadata expansion, preserving
source/selector order, complete slot plans, bitfields, and narrowed machine owners.
Native conformance now has 301 supplemental tests; the fixed inventory denominator
is unchanged. At that checkpoint, new native nested-container invariants were
implementation gaps; the recursive builtin checkpoint below closes that bounded
producer work. The other admitted one-operand reflection pipeline forms
are covered by the later checkpoint below.
Final workspace and supported-host distribution gates still require verification.

The locked Windows workspace run at `52bd4dcb` passed every target and doc-test
with tracked source frozen throughout. All 284 native conformance tests passed,
including the six new qualified-result and secret validating-constructor tests.
The independent gates passed for 182 HIR/MIR fixtures, 182 native objects,
30 entry cases, 25 runtime contracts, 155 verify fixtures, and three property
fixtures with 100 trials each. The
[supported-host run for that revision](https://github.com/vycdev/jett/actions/runs/36898786909)
passed both workspace/build/package jobs and both clean installed jobs.
This checkpoint does not certify the later refinement
reuse repair or its final workspace and distribution obligations.

The locked Windows workspace run at `a57d69a6` also passed every target and
doc-test with source frozen throughout. All 288 native conformance tests passed,
including the four direct refinement reuse regressions. Its independent inventory
gates passed for 182 HIR/MIR fixtures, 182 native objects, 30 entry cases,
25 runtime contracts, 155 verify fixtures, and three property fixtures with
100 trials each. The
[supported-host run for this revision](https://github.com/vycdev/jett/actions/runs/36905939310)
passed both workspace/build/package jobs and both clean installed jobs. This
certifies that checkpoint, not the later
call-proof or native-return changes.

The frozen workspace run at `8ad7b1e4` and the supported-host runs at
[`768fdb15`](https://github.com/vycdev/jett/actions/runs/36923339203) and
[`8ad7b1e4`](https://github.com/vycdev/jett/actions/runs/36970926217) failed with a
reference runtime stack overflow during native conformance. Sequential isolation
identified nested machine JSON parsing. The source call path is finite: a
temporary diagnostic stack increase completed the case, and unoptimized Windows
assembly showed the general expression evaluator reserving 79,328 bytes per
expression. Ordinary and generic calls now dispatch before that large match,
retaining central checked-result normalization and signal propagation. The
production stack budget remains 8 MiB. All six adjacent nested JSON conformance
tests and all 396 interpreter tests pass with the repair. Complete workspace and
supported-host distribution validation still need to certify the repaired
revision; the failed runs do not establish those gates.

The locked Windows workspace run at `f8e70722` passed every target and doc-test
with tracked source frozen throughout. All 399 interpreter tests, 598 frontend
fixtures, and 301 native conformance tests passed. The independent inventory
gates passed for 182 HIR/MIR fixtures, 182 native objects, 30 entry cases,
25 runtime contracts, 155 verify fixtures, and three property fixtures with
100 trials each. This checkpoint includes the call-frame stack repair, declared
ancestor proof reuse, native return validation, and direct/piped reflected root
producer validation. It does not close the remaining nested-container or other
reflection pipeline gaps, or the unresolved language policies.

The [supported-host run for the stack repair at `57e0b003`](https://github.com/vycdev/jett/actions/runs/36974175496)
passed both workspace/build/package jobs and both clean installed jobs. The
[run for `f8e70722`](https://github.com/vycdev/jett/actions/runs/36975813263)
passed both workspace/build/package jobs and the Linux clean installed job.
Its Windows clean installed job failed when the native linker exceeded its
60-second deadline on the first relocated debug build, without linker output.
This does not establish the fourth distribution gate. The
[run for `cca40a2e`](https://github.com/vycdev/jett/actions/runs/36980991837)
is live in both platform workspace checks; its eventual installed-package result
must determine whether the linker timeout recurs. The earlier repair's green
distribution evidence does not certify these later changes.

The admitted one-operand observers `type.variant_value`,
`type.machine_state_value`, and `type.arg` now append their existing checked
metadata through a shared direct/pipeline HIR helper. Pipeline result signatures
select metadata types; the step span describes the input. Source operands run
first and once. Variant ordinals remain distinct from discriminants, and narrowed
machines retain the complete base layout. Admission, runtime ABI, pending/index
checks, and ownership semantics are unchanged.

Five focused HIR/object tests, all 227 HIR/MIR/codegen library tests, and all
57 object-emission tests pass. Three linked regressions pass through direct and
pipeline forms in debug and release after source removal, covering exact metadata,
independent field ownership, operand/consumer order, nine terminal errors, closed
comptime, compiled verify, and 100-trial property execution. The fixtures use
ordinary return factories to keep a separate whole-machine reassignment defect
outside these observer tests. Native conformance now has 304 supplemental tests;
the fixed inventory denominator is unchanged. Full workspace and supported-host
distribution validation remain obligations for this later revision.

Reference reflected-type comparison now removes completed aliases/refinements
from its active expansion path. Repeated map/result arguments therefore normalize
independently, while actual recursive re-entry remains bounded. Four regressions
cover all three selectors, map key/value and occupied-arm order, exact pending
depths 0/1/2, changed-pending refusal, unchanged source, selector precedence,
declaration namespace, cycles, and protected conversion refusals. All 403
interpreter tests and all 598 frontend fixtures pass. Fresh CLI map and result
controls pass through all three getters with the expected predicate traces and
unchanged sources. This reference repair does not widen the native raw getter's
currently narrower nested-type compatibility gate.

Native whole-machine local storage now retains the checked bare machine
annotation instead of the initializer's precise state. Owned mutable locals can
therefore be rebound to another valid state of the same nominal machine, as the
existing annotation-erasure rule selects. Producer types, owner identities, and
source spans remain precise. Assignment verification retains the storage/owner
check and additionally enforces an exact checked state target when a visible
guard narrows a full-machine slot; foreign owners and other states still fail.

Two HIR and two object regressions pass, along with all 229 HIR/MIR/codegen
library tests and all 59 object-emission tests. The linked regression passes in
debug and release after source removal, including owned replacement, move and
clone independence, self-clone, 32 repeated cleanup cycles, same-state guarded
replacement, pending depths 2/1, closed comptime, compiled verify and 100-trial
property execution. Fresh reference CLI output and traces match the independent
oracle. This closes the separate whole-machine reassignment defect found while
isolating the observer pipeline fixtures. Native conformance now has 305
supplemental tests; the fixed inventory denominator is unchanged. Full workspace
and supported-host distribution gates remain required for this later revision.

The admitted alias-as-constructor spelling also remains an unresolved source
contract; see [type alias constructor calls](../open_design/type_alias_constructor_calls.md).
Supplemental cases do not change the inventory denominator. Full workspace and
supported-host distribution validation remain obligations for the final revision.

The frozen `b733296a` workspace, whose compiled source is unchanged from
`cca40a2e`, passed `cargo test --locked --workspace --no-fail-fast`, including
all doc-tests. This verifies 403 interpreter tests, 598 frontend fixtures,
305 supplemental native conformance tests, all 182 native object obligations,
and the complete verify/property inventory. The log is
`target/native-workspace-b733296a.log`; later changes need their own verification.

Native verify/property suite APIs now forward explicit build options through
lowering, emission, and executable building. Existing default wrappers retain
debug behavior. Three focused regressions pass: default and explicit-debug
objects match; debug/release suites execute with matching runtime manifests
after source removal, with 100 property trials and release traces omitted;
release debug printing reports E0362 before publication and preserves existing
output; and emitted release objects match optimized codegen while differing
from unoptimized controls. Both runtime manifest profiles reject a mismatched
selection. Native conformance now contains 308 supplemental tests. This repairs
suite mode selection; it does not close the remaining language-policy gaps.

Deep reflected requests exposed parser lookahead ceilings before either backend
could check the requested type. Generic calls and typed locals now recognize
balanced type syntax across the complete logical line, with iterative callable
return prefixes and no fixed 20/40/60-token cutoff. All 92 parser tests and all
598 frontend fixtures pass. The five new parser regressions cover direct and
pipeline requests, nested callable arguments and locals, source spans,
non-consuming recognition, missing EOF sentinels, and malformed-input recovery.
This repairs recognition of existing syntax; recursive reflected-field native
execution remains a separate obligation.

### Recursive builtin reflected producers

Native reflected field reads now establish newly requested refinements inside
the five already admitted builtin wrappers: list, set, map, optional, and result.
Typed HIR trees retain actual and requested types, canonical predicate identities,
and proven prefixes. MIR completes all occupied structural checks before any new
predicate, then checks values in collection order with each map key before its
value. Both passes preserve the independent candidate and original source.
Exact schemas, named ancestors, and shared root prefixes reuse declared proof;
changed wrappers retain readiness even when their child predicates are skipped.

Canonical generic argument TypeIds distinguish nominal fields from arguments.
An ordinary Element containing a Positive field does not request new invariants
through list[Element]. Used or unused Positive generic arguments do retain
wrapper readiness, including transparent aliases. Closed backend validation
visits unused argument edges and rejects malformed or forged plans.

All ten new linked groups pass over 96 source cases. They cover every getter,
direct and pipeline calls, debug/release runtime profiles, source deletion,
ready/empty/inactive wrappers, recursive combinations, Unicode ownership,
operand order, exact and ancestor pending exceptions, first-failure order,
selector precedence, terminal-handler bypass, and compiled verify/property
suites with 100 trials. Fresh reference admission and literal output/trace
oracles agree. The logs are `target/native-recursive-linked-all.log` and
`target/native-recursive-linked-nominal.log`.

All 494 types/typechecker/HIR/MIR/codegen library tests, 141 runtime tests,
62 object-emission tests, and 598 frontend fixtures pass. Native conformance
contains 318 supplemental tests; the fixed inventory denominator remains 207.
This closes the bounded recursive builtin producer gap, without selecting
broader requested-type admission, changed nominal/callable/interface casts,
or protected secret observation. Final frozen workspace and supported-host
distribution gates remain required for this revision.

### Selected debug-event channel isolation

The decided print/println diagnostic channel is implemented across reference
and native execution. One typed interpreter buffer preserves Trace, Breakpoint,
Print, and Println events with exact text; no newline is inserted between events.
Native DebugPrint emits the same staged bytes to stderr, apart from application
Stdout. Native differential gates compare only runtime events, excluding actual
frontend/comptime observations and private replay.

Build, lowering, native artifact, test, and run outcomes retain preceding events
on success and failure. Compiler captures keep phase and test/file association;
baked values do not replay events and property shrinking remains private. CLI
agent rows encode known phase and kind and escape exact text instead of
classifying prefixes or leaking raw worker output. Human tools emit captured
debug bytes to diagnostic stderr before later errors.

Failed comptime recovery records actual span/context attempts separately from
baked values, so repeated checked contexts and namespace initializer markers do
not duplicate failures or events. Failed values remain unavailable. Native
property replay errors retain the original suite streams when that attempt
returned captured process output and suppress private replay runtime streams in
the public error chain. Initial execution errors and timeouts retain their
existing typed error contract rather than inventing an original-suite record;
actual linker diagnostics remain available.

Focused verification passes: 416 comptime tests, 83 driver library tests,
144 runtime tests, all 50 CLI unit/command tests, and all five new linked native
transport tests. The latter cover exact event boundaries and kind controls,
frontend/runtime separation, failed argument ownership cleanup, partial-print
terminal error adjacency, source deletion, compiled verify/property suites,
release E0362 plus argument checking, and preservation of an existing output.
The final focused logs are `target/native-debug-isolation-comptime-final.log`,
`target/native-debug-isolation-driver-final.log`,
`target/native-debug-isolation-cli-final.log`, and
`target/native-debug-isolation-native-transport-final.log`; the unchanged runtime
gate is in `target/native-debug-isolation-library-driver-check.log`.

The frozen `b1cf8943` workspace passed
`cargo test --locked --workspace --no-fail-fast`, including all doc-tests,
323 supplemental native conformance tests, all 182 native object obligations,
598 frontend fixtures, and the complete fixed inventory gates. The log is
`target/native-workspace-b1cf8943.log`. This verifies that frozen revision;
the later JSON changes require their own workspace acceptance.

The prior `3bc23e6a` revision passed its frozen workspace and doc-tests in
`target/native-workspace-3bc23e6a.log` and all four supported-host jobs in
[workflow run 36995400983](https://github.com/vycdev/jett/actions/runs/36995400983).
The `b1cf8943` supported-host
[workflow run 37002829148](https://github.com/vycdev/jett/actions/runs/37002829148)
passed all four Linux/Windows build/package and installed-package jobs.
The installed-package jobs cover clean relocation, execution after source
removal, and both debug/release profiles. This accepts the frozen debug
isolation revision on its supported hosts; later JSON source changes still
require their own workspace and supported-host acceptance.
The broad estimate remains about 85%; the fixed inventory denominator remains
207. Hidden-secret policy and other semantic obligations remain independent.

### Qualified machine JSON and parser pipeline dispatch

Native JSON source eligibility now matches the existing public admission
boundary: a state-qualified machine visits only its selected payload, while a
bare machine still visits every declared state. The unrelated state in the
regression schema contains an integer-keyed map; qualified ready-state JSON is
admitted, but bare and cached-state requests retain E0343. Public policy,
all-state reflection metadata, secret projection, and qualified builder-finish
checks are unchanged.

Nested `state.fields` loops originating in `type.machine_states[T]()` receive
checked field-type and source-alias candidates from every declared state,
including canonical `Machine at state` requests. Source aliases in these
candidates belong to field TypeInfo; top-level aggregate aliases remain
non-matching probes with empty metadata. Runtime state metadata remains complete,
so an unselected field must not reach an unmatched reflected dispatch arm.
Direct active-state field loops retain their selected-payload candidates.
This compiler handoff repair is distinct from selected-state JSON eligibility.

The private fallback decoder now uses the direct canonical condition
`type.name[T]() == "json.JsonTree"` for raw-tree copying, matching the
named decoder's existing checked static selection. An ordinary pure metadata
helper call had left the raw return body typechecked under unrelated T,
exposing E0311 for inactive callable, interface, and actor fields. Pure-call
folding does not control source validity. This narrow source repair keeps
the raw body out of those specializations without changing public JSON
eligibility, reflection metadata, or the existing typed wrapper paths.

Completing those candidates exposed a private map decoder branch that returned
`map[string, Val]` for a non-string-keyed T. An alias-aware closed key guard now
limits typed string-map construction to exact string keys. Public non-string
map targets still fail E0343. A malformed qualified envelope naming another
state's unsupported map payload instead returns an unsupported-key failure
when its field decoder is reached, before the eventual qualified builder
mismatch. Required-field lookup and exact validation retain their order.
This bounded diagnostic-precedence tradeoff is recorded in the
[machine JSON contract](../open_design/state_machine_json_contract.md);
it introduces no key cast, dummy value, early state gate, or global builder
change. Fresh reference characterization pins the actual first wrapped
error for empty and nonempty cached-state maps as
`cached.entries: JSON object maps require string keys, got int64`. Lenient
numeric entries reach the same key refusal; exact numeric entries retain
the earlier `entries: expected object, got number` validation error. Missing
entries still report `missing required field 'entries' for app.Session at ready.cached`.
The expanded direct/pipeline controls compare these exact error prefixes,
continued successful parsing, retained owners, and complete reference/native
streams. All 22 groups passed (323 filtered) in 175.92 seconds in
`target/native-json-qualified-driver-final.log`.

Parser pipelines now record the declared `json.parse` or `json.parse_exact`
source facade from their checked `result[T, string]` payload using the existing
direct-call handoff mechanism and recursive eligibility predicate. Argument
checking, input ownership, pipeline order, step-local handlers, and intrinsic
identity are unchanged. An independent int64 pipeline witness exposes this
dispatch defect without requiring machine qualification.

Fresh pre-change CLI witnesses checked and ran successfully through the
reference path while native lowering refused the generic JSON intrinsic. Bare
machine controls reported E0343. These characterization results identify the
defects; they are not validation of the new implementation or regression suite.

The qualified-JSON repair suite contains 23 independent tests and 32 Jett fixtures.
It is intended to cover all four JSON operations in direct and pipeline forms,
qualified aliases in lists, wrong-state handling, lenient and exact extra fields,
borrowed-value preservation, primitive parser input clone/reread, debug and
release profiles, source deletion, output publication controls, and pure
selected-state serialization through explicit comptime, verify, and 100-trial
property suites. Additional controls exercise exact string-key aliases and
independent all-state reflection with field aliases, canonical ready/empty
qualifiers, and direct selected-field loops. Runtime fixtures use
capability-backed Stdout, with debug and frontend observations checked
separately. Missing entries in the wrong-state payload are now covered;
unknown or missing state tags, missing payloads, other missing required
fields, secrecy, and broader wrapper combinations remain outside this
suite's claim. One new group over three inactive callable/interface/actor
payloads checks all four direct JSON APIs for a valid selected ready state,
both runtime profiles, retained source owners, and source deletion. Those
cases do not admit the unsupported payloads themselves as JSON targets or
claim malformed-input behavior for them.

Recorded phase checks pass: 40 codegen and 416 comptime library tests in
`target/native-json-qualified-compiler-phases.log`, plus 125 HIR, 79 MIR,
and 221 typechecker tests in `target/native-json-qualified-remaining-phases.log`.
The driver library batch reported 82 passes and one temporary capture-file
creation PermissionDenied failure; the exact failed test passed its isolated
retry (one pass, 82 filtered) in
`target/native-json-qualified-driver-permission-retry.log`. The original
batch is not described as an uninterrupted passing run.

At this qualified-JSON checkpoint, the compiled supplemental corpus contains
346 tests, confirmed
by the final JSON family gate's 39 passes and 307 filtered in
`target/native-json-qualified-json-family-final.log` (438.34 seconds).
That gate includes all 23 new groups on the final source and 16 existing JSON
regressions; it is not 346 executed passes. The earlier 22-group checkpoint
and additional group's pass remain recorded separately in
`target/native-json-qualified-driver-final.log` and
`target/native-json-qualified-unselected-payloads-final.log`.
All 598 frontend fixtures pass on the repaired source
in `target/native-json-qualified-frontend-fixtures.log` (20.66 seconds).
The frozen `23ac34c4` complete workspace passes in
`target/native-workspace-23ac34c4.log`, including all 346 native conformance
tests (838.03 seconds), all 182 native object obligations, all 598 frontend
fixtures (22.88 seconds), and doc-tests. Its full driver library batch also
passes all 83 tests, separately from the earlier temporary-file retry.
The head and worktree remained unchanged throughout that run. Supported-host
acceptance for this revision remains pending in
[workflow run 37013565734](https://github.com/vycdev/jett/actions/runs/37013565734):
the Windows build/package job has passed; Linux and clean installed-package
acceptance are not yet complete. Earlier phase results do not validate later
source additions.
This slice supplements the unchanged 207-fixture inventory; the
broad estimate remains about 85%.

### Qualified machine JSON inside optional, result, and map wrappers

An additional 24 linked tests cover `optional[Session at ready]`, both arms of
`result[Session at ready, Session at ready]`, and `map[string, Session at ready]`.
Each wrapper crosses all four public JSON APIs with direct and pipeline calls.
The machine's unrelated cached state contains an unsupported integer-keyed map.
The controls exercise occupied and absent optionals, both result arms, populated
and empty maps, and retained original owners. Three additional bare-machine
controls keep E0343 for an empty optional, an inactive result arm, and an empty
map; runtime emptiness does not exempt a target from public type policy.

All 27 new Jett fixtures passed frontend/reference preflight in
`target/native-json-qualified-wrapper-preflight/results.json`: the 24 valid
cases produced exact literal stdout and empty stderr, and the three invalid
cases reported E0343. All 24 linked tests then passed (346 filtered) in
`target/native-json-qualified-wrapper-linked.log` (190.12 seconds), using both
matching runtime profiles after source removal. The extended policy group
passes (one test, 369 filtered) in `target/native-json-qualified-wrapper-policy.log`
(7.93 seconds), preserving existing output artifacts in both profiles.
All 27 source-format checks and Rust formatting checks pass.

This test-only extension registers 370 supplemental native tests. The frozen
346-test workspace result and these focused passes remain separate evidence;
a complete 370-test workspace and supported-host run have not yet been accepted.
Wrong-state wrapper errors, deeper wrapper combinations, aliases, secrets, and
pending values retain their independent audit obligations. The broad estimate
remains about 85%, and the fixed inventory remains 207 fixtures.

### Stable ordinary-struct field local-view checkpoint

The typed borrowed initializer now proves ordinary-struct field paths rather
than requiring the selected endpoint to equal its whole owner's type. Exact
source identity, field types, nominal refinement qualification, initializer
coverage, and original-function validation before native compaction remain
required. Alias storage is non-owning; dependent reads retain the real owner.
Copied inline-function tables discard origins belonging to another body.

`target/native-stable-projected-view-final-phases.log` passes all 896 library
tests across codegen/comptime/HIR/MIR/typechecking. The final native alias gate
passes all 12 tests (366 filtered, 11.78 seconds) in
`target/native-stable-projected-view-final-linked.log`: eight linked cases,
mutable/temporary publication boundaries, and three existing broad alias tests.
Both runtime profiles execute after source deletion with exact output/status,
independent clone ownership, and later-failure cleanup. The initial unused-type
negative-test lookup was corrected to the actual type interner before the final
passing phase batch; it did not require a compiler behavior change.

The final driver/frontend batch passes all 83 driver library tests and all 598
frontend fixtures in `target/native-stable-projected-view-final-driver-frontend.log`.
The stale capture test now uses the existing mutable-root HIR boundary. The
Linux-only projection-loan gate now checks owner consumption after creating a
valid field alias; its zero-test Windows target is not an executed pass. Linux
CI remains required, and the matching Windows CLI source independently refuses
the owner change before archive lookup.

This ordinary-struct checkpoint registered 378 native tests. Its later frozen
a0bf12df workspace passed all local gates, while supported-host acceptance
retains its separate revision-specific obligation. A qualified machine-field
alias was source-valid but native-refused before the exact-state follow-up
below. Neither this checkpoint nor the fixed inventory establishes full-
language parity; the planning estimate stays about 85% and denominator 207.

### Exact qualified-machine field local-view focused acceptance

The exact declared MachineState owner lookup is integrated with unchanged
source/nominal proof, persistent loans, non-owning slots and runtime ABI.
Malformed unused state/index/endpoint/root metadata and concrete generic
contexts pass the compiler gate: 261 tests (41 codegen, 134 HIR, 86 MIR) in
`target/native-machine-projected-view-phases.log`. All 83 driver library tests
pass in `target/native-machine-projected-view-driver-frontend.log`; all 598
frontend fixtures pass in `target/native-machine-projected-view-frontend.log`
under the corrected `fixture_suite` target.

The 25-test local-alias batch passes (366 filtered, 17.72 seconds) in
`target/native-machine-projected-view-pending-linked.log`; the typed-endpoint
case passes separately (one test, 391 filtered, 2.80 seconds) in
`target/native-machine-projected-view-typed-linked.log`. These are 26 distinct
executed tests, not one combined batch. Linked cases use matching runtime
profiles and execute after source removal, preserving original owners, clone independence, exact
pending/intermediate errors, depth-two traces and explicit joins, terminal
status/cleanup. Separate boundary cases prove before-publication owner-change
refusal without looking up the archive. Generic endpoint
modes and nominal/secret field identities are pinned by the final case. All 15
machine Jett source-format checks and Rust formatting checks pass. Initial
source-only preflight and the corrected E0308 fixture handler are recorded in
[the scoped note](native_local_view_aliases.md#exact-state-qualified-machine-field-follow-up).

Bare flow-narrowed machine origins, mutable backing chains, temporary roots,
projected writes, allocating conversions and owner consume/rebind/transition
after reachable alias creation remain outside this bounded native support.
The actual owner-change group reaches the existing typed ownership refusal
before archive lookup and preserves the output sentinel in both profiles.
No source loan-expiry or temporary-lifetime contract is selected.

The previous frozen a0bf12df workspace passed with unchanged head/worktree
and exit zero at 16:01:32 UTC in `target/native-workspace-a0bf12df.log`: all 378
native tests (785.23 seconds), all 182 object obligations (491.87 seconds), 598
frontend fixtures (20.05 seconds), 83 driver library tests (75.05 seconds), and
the remaining workspace/doc-tests. That prior result does not validate later
machine source/test additions.

The earlier a0f0c982 workflow
[37019114837](https://github.com/vycdev/jett/actions/runs/37019114837) passed all
four jobs. The a0bf12df workflow
[37027835546](https://github.com/vycdev/jett/actions/runs/37027835546) also passes
all four Linux GNU/Windows MSVC build/package and clean installed-package jobs.
These accepted predecessor revisions do not establish new-source platform
acceptance.

### Current frozen revision and inferred JSON follow-up

The exact-state machine repair is committed as `774b13e7`. Its complete frozen
workspace passes with unchanged source and exit zero at 17:12:39 UTC in
`target/native-workspace-774b13e7.log`, including all 392 native tests (858.92
seconds), all 182 object obligations (the four-test object manifest gate passes
in 505.90 seconds), the remaining workspace targets and doc-tests. Its
[workflow 37035818971](https://github.com/vycdev/jett/actions/runs/37035818971)
now passes all four supported-host jobs. Broader owner/endpoint
combinations and independent language surfaces remain audit obligations.

The applied inferred JSON serializer repair passes 14 new linked/publication
tests (8.09 seconds), 948 crossed compiler library tests, 83 driver library
tests, all 598 frontend fixtures, and 17 Jett format checks. Both profiles pin
source-deleted execution, pending failure/cleanup, pure/comptime/verify/property,
public policy and owner rereads. Its actual captures, exact logs and boundaries
are centralized in [the scoped note](native_json_uninhabited_slots.md).
The serializer revision `05494ed4` passes its complete frozen workspace with
unchanged source, clean start/end and exit zero at 18:03:56 UTC in
`target/native-workspace-05494ed4.log` and its JSON summary: all 406 native tests
(873.42 seconds), all 182 object obligations (four-test manifest gate, 508.07
seconds), 83 driver library tests (65.72 seconds), all 598 frontend fixtures
(20.69 seconds), remaining workspace targets and doc-tests. Its
[workflow 37041625704](https://github.com/vycdev/jett/actions/runs/37041625704)
passes all four Linux GNU/Windows MSVC build/package and clean installed-package
jobs. These accepted serializer results do not validate subsequent parser or
bitfield changes.

The applied parser source-selector repair passes all 19 direct, pipeline and
recursive linked tests together (14.08 seconds, 406 filtered), pinning actual
valid, occupied-slot and malformed-input lines in both profiles after source
removal. The metadata unit passes once; the crossed library gate passes 949
tests, followed by 83 driver library tests, all 598 frontend fixtures and all
19 new source format checks.
The parser revision `08f9f7e7` passes its complete frozen workspace with
unchanged head, clean start/end and exit zero at
`2026-10-02T19:49:44.2598850Z`, recorded in
`target/native-workspace-08f9f7e7.log` and its JSON summary: all 425 native
tests (878.50 seconds), all 182 object obligations (four-test manifest gate,
506.38 seconds), 83 driver library tests (70.43 seconds), all 598 frontend
fixtures (21.95 seconds), remaining workspace targets and doc-tests. Its
[workflow 37053539878](https://github.com/vycdev/jett/actions/runs/37053539878)
passes all four Linux GNU/Windows MSVC build/package and relocated installed-
package jobs (last completion `2026-10-02T20:19:29Z`). This accepted predecessor
does not validate the later bitfield source changes. Exact parser behavior
and focused logs are in [the parser contract](native_json_uninhabited_parse_slots.md).
The objective remains whole-language native parity; the about-85% planning
estimate and fixed 207-fixture denominator do not measure completion of
these focused slices.


### Bitfield field local views: focused acceptance

The declaration-backed Bitfield Field step now preserves exact semantic field
type, canonical zero-based identity and the existing immutable root/path proof.
No MIR, runtime ABI, payload schema or lifetime policy changes are required.
Both focused HIR tests pass (134 filtered) in
`target/native-bitfield-local-view-hir.log`. All seven new linked cases pass
together in 3.89 seconds (425 filtered) in
`target/native-bitfield-local-view-linked.log`, using matching profiles and
source removal with exact application bytes, status, stderr and typed traces.
Pending whole-owner access fails before the after-binding marker with status
71; cleanup status 72 cannot pass. Explicit payload-copy joins preserve the
original depth-two payload, and release omits trace events.

The crossed compiler gate passes 951 library tests (41 codegen, 416 comptime,
136 HIR, 86 MIR, 48 resolve and 224 typecheck) in
`target/native-bitfield-local-view-phases.log`. All 83 driver library tests pass
(86.96 seconds) in `target/native-bitfield-local-view-driver.log`, and all 598
frontend fixtures pass (21.88 seconds) in
`target/native-bitfield-local-view-frontend.log`. All seven new source-format
checks pass in `target/native-bitfield-local-view-format.json`; Rust formatting
and diff checks pass.

The bitfield checkpoint registers 432 supplemental tests. The later clean
`cb617ecf` workspace below includes this repair and passes all 182 object
obligations. Supported-host jobs for both owner-view revisions now pass, as
recorded below. Initial source characterization,
exact qualification/ownership limits and required gates are in
[the bitfield contract](native_bitfield_projected_local_views.md). The accepted
08f9f7e7 result above is predecessor evidence, not new-head acceptance. The
about-85% coarse estimate and fixed 207 inventory remain unchanged.


### Direct secret aggregate local views: focused acceptance and open observation

The selected exact one-Secret owner/endpoint proof is integrated. It retains
declared nothing/already-secret nominal exceptions, root/index/type anchoring,
immutable loans, non-owning storage and existing pending guards. Three focused
HIR tests pass (136 filtered, 0.00 seconds) in
`target/native-secret-owner-local-view-hir.log`. The warning-interrupted first
invocation produced no test result and is not acceptance evidence.

All seven linked cases pass together (2.82 seconds, 432 filtered) in
`target/native-secret-owner-local-view-linked.log`, with matching profiles and
source-deleted execution. Six pin successful outcome parity. The pending case
separately pins the raw reference message and native `[redacted]` error, matching
`before:secret\n`, status 71 and cleanup. This preserves the already recorded
[open observation boundary](../open_design/debug_print_hidden_secrets.md);
it neither weakens native redaction nor claims diagnostic parity.
The initial six-pass/one-failure attempt (4.56 seconds) is retained in
`target/native-secret-owner-local-view-linked-initial-mismatch.log`. Its mistaken
shared error oracle was corrected to the existing separate-backend contract;
no source policy or interpreter behavior was changed.

All 954 crossed compiler library tests pass (41 codegen, 416 comptime, 139 HIR,
86 MIR, 48 resolve, 224 typecheck) in
`target/native-secret-owner-local-view-phases.log`.
All 83 driver library tests pass (66.62 seconds) in
`target/native-secret-owner-local-view-driver.log`; all 598 frontend fixtures
pass (22.51 seconds) in `target/native-secret-owner-local-view-frontend.log`.
All seven source-format checks, Rust formatting and diff checks pass in
`target/native-secret-owner-local-view-format.json`.

The corpus registers 439 supplemental tests. The exact `cb617ecf` workspace
passes with clean start/end and unchanged head at
`2026-10-02T20:55:57.0941072Z`, recorded in
`target/native-workspace-cb617ecf.log` and its JSON summary. All 439 native
conformance tests pass (905.04 seconds), all 182 object obligations pass in the
four-test manifest gate (516.91 seconds), and all 83 driver library tests
(91.44 seconds), 598 frontend fixtures (19.08 seconds), remaining workspace
targets and doc-tests pass. The wrapper reports exit zero and 92 successful
result groups. This accepts both owner-view repairs at that exact source
revision; it does not settle the preserved observation-policy boundary.
The documentation-only `194ce065` revision has identical compiler/runtime/test
sources and passes all four jobs in
[workflow 37064035604](https://github.com/vycdev/jett/actions/runs/37064035604):
Windows MSVC static-CRT and Linux GNU build/package jobs, followed by both
relocated installed-package jobs (last completion `2026-10-02T22:10:29Z`).
The standalone `cb617ecf` run 37060490263 was superseded and cancelled before
starting any job; that cancelled run is not acceptance evidence. The bitfield
`5d919052` revision separately passes all four jobs in
[workflow 37058389987](https://github.com/vycdev/jett/actions/runs/37058389987)
(last completion `2026-10-02T21:16:06Z`). Exact selected rules and actual seven-source
characterization are in
[the secret-owner contract](native_secret_owner_projected_local_views.md).
The about-85% estimate, fixed 207 inventory and whole-language 100% objective
remain unchanged; the independent MIR staging repair is not applied.


### Uninhabited-input callback descriptors: focused acceptance

An inferred empty-list callback remains an inhabited function descriptor even
when a non-capture input cannot exist. Native emission now retains its exact
captures, label, cloning, pending state and cleanup with a nonzero trap entry;
it creates no Never argument/result carrier and emits no original body effect.
Original descriptor metadata and all original local/debug types are checked
before preparation. Named enclosing functions compact only unused frame slots
across all original blocks and typed dependencies; surviving callable Never
parameters, locals and invocations remain invalid.

All 966 compiler library tests pass (52 codegen, 416 comptime, 139 HIR, 87 MIR,
48 resolve and 224 typecheck) in `target/native-uninhabited-callback-phases.log`.
All nine linked regressions pass together (3.65 seconds, 439 filtered) in
`target/native-uninhabited-callback-linked.log`, using matching profiles and
source deletion. Direct/returned/captured descriptors, owned cloning, pending
joins, Never-result identity, loop/equality bodies, invoked integer controls and
terminal failure cleanup retain exact outcomes. Nine Jett source-format checks
and Rust formatting pass. The corpus registers 448 supplemental cases.
All 83 driver library tests pass (69.44 seconds) in
`target/native-uninhabited-callback-driver.log`; all 598 frontend fixtures pass
(24.60 seconds) in `target/native-uninhabited-callback-frontend.log`.

The first isolated unit attempts failed before MIR because their loader does
not load the stdlib facade. Declared source helpers corrected those test inputs;
the driver cases retain full stdlib evaluation. The actual factory diagnostic
then exposed unused child formals in the enclosing frame table. Initial logs,
pre-change captures, exact bounds and remaining gates are in
[the descriptor contract](native_uninhabited_callback_descriptors.md).
The subsequently repaired `90d82701` full workspace/object and supported-host
results are recorded below. The earlier 439-case predecessor alone could not
supply that proof. The whole-language objective remains 100%, the fixed
inventory remains 207, and the broad planning estimate stays about 85%.

## Original function metadata in absent sum arms

The clean `d14b830b` full native suite reports 446 passes and two existing
sum-arm failures (941.02 seconds). Original preflight rejects a closure's exact
Never result before checked impossible-arm preparation can remove the closure.
The remaining object/workspace targets were stopped after that complete native
suite failed; `target/native-workspace-d14b830b.json` records exit -1, unchanged
clean head and the stop reason. No complete-workspace or object acceptance is
claimed for this attempt.

OriginalMetadata type validation now permits that exact function-result
metadata recursively while preserving view-schema and inhabited child checks.
NativeValue classification remains strict, and a return-only Never signature
still grants no descriptor authority or runtime capture/value representation.
The three new codegen units pin all three absent optional/result arms, omitted
closure symbols, nested metadata validity, malformed child/result/view schemas
in both local and debug types, and surviving return-only rejection.

All 969 compiler library tests pass in
`target/native-original-function-metadata-phases.log`. The two previously failing
linked tests pass (48.51 seconds) in
`target/native-original-function-metadata-linked-sum-arms.log`, retaining both
profiles, pending diagnostics, source deletion and native verify/property
execution. All nine callback cases pass again (3.82 seconds) in
`target/native-original-function-metadata-linked-callbacks.log`. Exact repaired
head `90d82701` subsequently passes its clean complete workspace and all 182
object obligations. Its unchanged-head log and JSON are
`target/native-workspace-90d82701.log` and `.json`: 448 native cases pass in
944.48 seconds; the four-test object manifest gate passes in 553.77 seconds;
all 83 driver library tests, 598 frontend fixtures, remaining targets and
doc-tests pass. Exit zero and 92 successful groups complete at
`2026-10-03T08:31:51.9056689Z`. Exact-head supported-host workflow 37108099164
passes all four Windows/Linux build/package and relocated installed jobs, last
completing `2026-10-03T09:03:23Z`. The corpus is 448, the fixed inventory 207, and the
planning estimate about 85%.

## Internal call-view stages across handlers

Original explicit borrowed arguments use typed MIR Begin/End scopes when the
existing cloneable snapshots cannot preserve the borrow. Exact checked local
and field origins, pending receiver order, normal owned temporary storage and
post-operation owner use are covered; source alias lifetime rules and ownership
changes during later operands remain independent. The focused execution record
and complete acceptance obligations are in
[the call-view contract](native_scoped_call_view_staging.md). This is additional
native implementation coverage, not whole-language completion or a denominator
change.

## Latent nested and secret-result callback metadata

The isolated follow-up preserves exact Never-input descriptor authority while
checking latent nested function bodies and direct Secret[Never] result metadata.
Validated target/signature/mode/capture edges precede cycle-safe body traversal;
no nested environment, native callable root, runtime carrier or ABI is created.
Live return-only callbacks, created Never captures, phantom values, malformed
metadata and opaque Resource layouts remain rejected. Exact selected proof and
centralized execution evidence are in
[the descriptor note](native_uninhabited_callback_descriptors.md#latent-nested-closures-and-direct-secret-results).

All 63 codegen tests and 27 linked regressions pass with unchanged source hashes.
The broader compiler run passes 1052 tests (989 library units plus 63 integration
tests); its wrapper's incorrect expected-count guard is not a compiler failure.
The 83 driver, 598 frontend and eight backend-lowering gates also pass with
unchanged sources. Exact clean `72237c4a` subsequently passes its complete
464-case workspace, all 182 object obligations and 92 successful result groups;
its supported-host workflow remains in progress. Independently, `55027914`
passes all four supported-host jobs as well as its complete local workspace.
Exact-head acceptance records are centralized in the descriptor note above.
The fixed inventory remains 207, object obligations 182, broad estimate about
85%, and whole-language goal 100%.

## Resource type-only kind tags

The checked builtin enum schema and shared reflection mapper now agree on the
accepted opaque `resource_type` tag. Three regressions cover source comparisons,
reference type metadata with empty shapes, and a native object reached through
an ordinary project caller. The affected 800 compiler tests and 598 frontend
fixtures pass with unchanged source hashes. This adds no live Resource carrier,
provider, source constructor or cleanup behavior. Clean `616b51b9` subsequently
passes its full workspace; supported-host acceptance for that head remains
separate. Exact evidence and remaining
lifecycle prerequisites are in [the active note](native_resource_kind_tags.md).

## Checked private resource-hook identities

The identity-only prerequisite creates associated private Function DefIds through
ordinary resource declaration resolution and checks complete function types and
ownership modes against the exact nominal Resource definition. Stdlib FileId
and independent loader origin, ordinary privacy/order/duplicates and unused
metadata validation are required. Production catalogs stay empty; no source
constructor, provider operation, runtime Resource value or native carrier is
created. See [the selected contract and scope](native_resource_hook_identity.md).

The isolated final focused run passes six resolver, seven checker and one
span-collision test with ten unchanged Rust source hashes. Exact results/logs
are retained in `target/native-resource-hook-focused-after-diagnostic.json` and
its referenced per-gate logs. The earlier missing-`mut` compile failure and
source-body diagnostic expectation correction are not successful gates. The
broader 1037 compiler library tests, 63 codegen object integration tests, 83
driver tests, 598 frontend fixtures and eight backend-lowering tests also pass
with unchanged sources in `target/native-resource-hook-phases.json`. The
subsequent clean `616b51b9` full workspace passes, as recorded below; its
supported-host run remains in progress. No reference or native Resource
lifecycle was executed. This is checked identity proof, not a
resource-provider parity row closure or inventory/percentage increase.


## Resource static ownership boundaries

The accepted local predecessor is exact clean `616b51b9`: its frozen full
workspace exits zero with all 92 result groups, 464 supplemental native cases
(940.26 seconds), all 182 object obligations (four manifest groups, 534.07
seconds), remaining workspace targets and doc tests passing. Start/end HEAD and
cleanliness agree in `target/native-workspace-616b51b9.json`; the run ends at
`2026-10-03T12:11:54.8924860Z`. The older `62de6505` supported-host workflow
[37118139645](https://github.com/vycdev/jett/actions/runs/37118139645) succeeds in
all four Windows/Linux build and installed relocation jobs. The exact
`616b51b9` workflow 37119573097 is still in progress; predecessor platform
acceptance does not transfer to this head or the new guards.

The new static guards refuse recursive Resource-copy acquisitions (E0364),
known Resource views stored in owning payloads (E0401), and exact Resource-root
printing/value reflection (E0300). Exact Resource-bearing machine transition
sources reuse existing owned-argument checks. Safe views, unrelated copyable
fields, actual owned payload transfers, empty carriers and type-only metadata
remain admitted. Nested printing, hidden-Secret observation and other ownership
paths remain separate work; no runtime/provider/native Resource value is enabled.

All 36 focused groups pass, including 21 new source groups, in
`target/native-resource-source-guards-focused-after-diagnostic.json`. The original
20 sources and additional 14 payload sources all parse/resolve and are checked
in both profiles with stable inputs. Their reports show the intended refusals
and retained controls; these probes do not evaluate comptime/property bodies,
install providers or execute native Resource code. See the
[complete static scope and source evidence](native_resource_ownership_boundaries.md).

The broader wrapper passes 1058 library units (254 typechecker units), 63
codegen object integration tests (1.11 seconds), 83 driver tests (69.79 seconds),
598 frontend fixtures (22.13 seconds) and eight backend-lowering tests. All three
source hashes remain unchanged; `target/native-resource-source-guards-phases.json`
ends with exit zero at `2026-10-03T12:28:58.9045108Z`. New complete-workspace and
supported-host acceptance remain pending. This prerequisite adds no linked
Resource case or object obligation: the about-85% estimate, fixed 207 inventory,
182 obligations, 464 supplemental cases and whole-language 100% objective stay
unchanged.


## Immutable checked Resource program handoff

Exact clean `b7180bd9` completes its frozen full workspace with exit zero and
all 92 successful result groups. It passes 464 supplemental native cases
(958.70 seconds), all 182 object obligations (four-test manifest gate, 650.63
seconds), remaining workspace targets and doc tests. Start/end HEAD and
cleanliness match in `target/native-workspace-b7180bd9.json`; the actual session
1952 is terminal with exit zero and ends `2026-10-03T13:18:31.4769949Z`.
Its supported-host workflow
[37123835396](https://github.com/vycdev/jett/actions/runs/37123835396) now succeeds
in all four Windows/Linux build and installed relocation jobs. The last Windows
installed job ends `2026-10-03T14:14:19Z`; the exact-head receipt is retained in
`target/native-platform-b7180bd9.json`. Earlier in-progress observations are
superseded by that result, without transferring acceptance to exact94.

Exact predecessor `616b51b9` now also has supported-host acceptance: workflow
[37119573097](https://github.com/vycdev/jett/actions/runs/37119573097) succeeds in
both Windows/Linux build and installed relocated-package jobs. The final
Windows installed job completes `2026-10-03T13:14:20Z`. This supersedes that
run's earlier in-progress observations; it does not validate the new handoff.

`CheckedResourceProgram::prepare` owns its ParseResult and derives resolution
and checking internally with explicit loader origins/private catalog input.
Only immutable source/resolver/checker/interner observations are exposed. Error
diagnostics from each source phase prevent sealing, and complete unused hook
identity/signature checks still apply. Public hook validation adds independent
checker-error refusal while ordinary checking keeps inspectable source repair
diagnostics. No private operation executes or Resource carrier is installed.

The root-owned focused gate passes all **43 Resource groups**, including seven
new source-driven snapshot groups and the strengthened invalid-body phase-gate
assertion. The broader **454 library tests** pass: 139 HIR, 54 resolver and 261
typechecker. Five compiler hashes remain unchanged in
`target/native-resource-checked-program-focused.json` and
`target/native-resource-checked-program-phases.json`, both exit zero. The
[scope and evidence note](native_resource_checked_program.md) preserves the
pre-Rust contract and separates this handoff from driver retention, reached-hook
runtime eligibility, call ownership, cleanup and native Resource admission.

Full-workspace and supported-host validation for this new change remain pending.
The fixed 207 inventory, 182 object obligations and 464 supplemental native
cases are unchanged; the about-85% estimate is still coarse planning and the
whole-language native-codegen objective remains 100%.


## Retained reference preparation

The isolated reference follow-up over exact `94bb6b2e` retains a private
`Arc<CheckedResourceProgram>` and primary FileId0 main span/namespace alongside
unchanged public BuildResult observations. Required evaluation uses that same
merged AST and checked facts. Runtime gates compilation and missing-snapshot
failures before providers, registers the saved module once and performs no
post-preparation read/parse/rediscovery. The macOS preliminary graphics-thread
read remains scoped outside this guarantee. Through-failure diagnostics preserve
earlier phase warnings and typed metadata causes. This enables no live Resource,
provider, callable descriptor authority or native ABI. See
[the selected contract and scope](native_prepared_reference_driver.md).

All **46 focused Resource groups** and **four new driver groups** pass in
`target/native-prepared-reference-driver-focused.json`, ending
`2026-10-03T14:12:41.4871736Z` with exit zero and both formatted source hashes
unchanged. These include three new warning/cause groups and four driver controls
for deleted or mutated sources, primary selection despite a support main,
once-only frontend/runtime observations, and failure before provider setup.
All **544 driver/HIR/resolver/typechecker tests** also pass: 87 driver, 139 HIR,
54 resolver and 264 typechecker. `target/native-prepared-reference-driver-phases.json`
ends with exit zero at `2026-10-03T14:15:36.7702211Z`; both source hashes remain
unchanged. All **eight backend-lowering regressions** (198.73 seconds), including
the fixed inventory lowering check, and **598 frontend fixtures** (24.41 seconds)
pass in `target/native-prepared-reference-driver-frontend.json`, ending
`2026-10-03T14:20:34.8384468Z` with exit zero and the same two source hashes.
New full-workspace and supported-host acceptance remain pending.

Exact94's frozen full workspace is **not accepted**: it ends with Cargo exit101,
91 successful result groups and stable clean HEAD in
`target/native-workspace-94bb6b2e.json` at `2026-10-03T14:10:25.5278499Z`.
Its native suite passes 463 cases and fails one NamedTempFile.reopen operation
with PermissionDenied. All four object-manifest tests pass (562.05 seconds),
but that does not make the workspace successful. The affected exact94 native
case passes its separate rerun (one passed, 463 filtered, 7.98 seconds) under
elevated execution; this is a changed permission context, not acceptance of the
failed whole-workspace run. The original failure remains evidence.

The last accepted local predecessor remains `b7180bd9`, with its 464 native
cases, 182 object obligations and 92 result groups recorded above. The same b718 head also has all-four supported-host acceptance, recorded above;
616 remains historical. These receipts do not transfer to exact94 or this
follow-up. The exact94 workflow 37126837754 is still in progress. No linked native case or
object obligation is added: fixed 207 inventory/182 obligations, 464 supplemental cases, about-85% planning
and the whole-language 100% goal remain unchanged.

## Typed caller ownership integration

The isolated caller-ownership change over `a9a6cd63` implements Rule24 caller
disposition and Rule25 ordinary-data observation. Sealed source-call facts rejoin
exact declarations, argument permutations and raw/physical types through HIR,
MIR and native validation. Generated calls, temporary staging, viewed loops and
handled defaults carry separate checked proofs. The
[implementation contract](native_call_ownership_facts.md) records the scope.

The earlier formatted caller-core checkpoint passed 304 checker, 188 HIR,
196 MIR, 70 backend and 63 object-emission tests, plus the MIR doctest. Focused
handled-producer and builder-generation checks passed 66 MIR acquisition groups,
seven generation groups, three native emission groups and both original native
regressions. All seven failures of the historical 472/479 run subsequently
passed their individual replays.

The complete Windows native suite now passes **480/480** at exact committed
`a63f891016fac7dc50fffdde33be8b0ce45c655f`. Root terminal `06b7f8` and
`target/native480-after-owner-generation/receipt.json` record zero failures and
stable HEAD/tracked sources, ending `2026-10-04T14:20:12.987780+00:00`
(2320.39 seconds). Resource candidate changes are absent from that run.
This is complete native-corpus acceptance for that caller head, not whole-language
completion or new current full-workspace/platform acceptance. Fixed lowering
remains 182 fixtures; a separate earlier stable recovery scan passed all 182.
Current 182-object, workspace and supported-host receipts remain required.

The accepted unchanged a9 baseline passed 92 workspace result groups, 464 native
cases and all 182 object obligations. Workflow 37130142896 passed all four
Windows/Linux build and installed jobs. Those historical receipts do not transfer
to the Resource candidate. The fixed inventory remains 207 with 182 object
obligations and the rough feature estimate stays 85%. Async/actor lifetimes,
broader caller-generation/capture/reflection cases and Resource backend/provider
execution remain in the full 100% objective.

## Resource reference lifecycle execution

The private reference Resource implementation over a63 passes all **36 focused
tests** in `target/native-resource-after-checked-call-relay-focused.log`
(root terminal `923450`; zero failures, 418 filtered, 0.07 seconds).
The suite includes exact ordinary/mutual declaration identity, retained source
registration, real both-profile Source construct/borrow/move/close and reverse
cleanup, occupied returns, indirect close, later lexical actual failure,
ordinary closure return7, exact terminal list-kernel failure and mixed cleanup
failure precedence. Provider events and zero live custody/registry entries are
asserted before teardown. Closed descriptor/absence and required-worker purpose
controls do not invoke a runtime provider. The
[detailed reference scope](native_resource_hook_identity.md#reference-lifecycle-execution)
keeps runtime eligibility, custody and catalog authority separate.

These are private reference tests with a compiler-test-only scripted provider,
not native Resource execution. Production catalogs/providers remain disabled;
no shipped Resource API or native hook/carrier/drop path is admitted. Whole
scoped/reflected/indirect/worker and aggregate ownership breadth, real providers,
async/actor cancellation and late-completion paths remain pending. Broad library,
whole-workspace and supported-host acceptance of the Resource candidate are
also pending. The 36 tests add no native inventory row and do not change the
480-case caller receipt, 207/182 fixed denominators or about-85% estimate.

## Checked Resource reference transport follow-up

The later scoped/required/generic, pipeline and mutable-assignment transport
passes **83/83** focused reference groups (`61a351`) in
`target/native-resource-concrete-intrinsic-pipeline-syntax-fixed-focused.log`.
Exact original region/body/step/statement proofs survive nested entry and restore
metadata/cursors. Concrete intrinsic metadata preserves generic substitutions
and alias names; copied source packets and foreign contexts remain refused.
The complete formatted gate passes **1,186/1,186** (501 Comptime, 87 driver, 598 frontend; `4ba85c`) in `target/native-resource-transport-formatted-property-fixed-broad.log`. Pipeline actuals retain lexical order, and assignment replacement retains the
previous owner until RHS succeeds and cleanup/publication complete.

The prior 36/43 receipts remain historical. The foundation `7157f564`
[supported-host run](https://github.com/vycdev/jett/actions/runs/37217759157) passed
all four jobs and excludes this follow-up and custody core `92421063`. These reference
controls do not execute native Resource values. The 480-case native caller receipt,
fixed 207/182 inventory and approximate 85% estimate keep their separate scopes;
full providers, aggregates, capture/reflection/refinement breadth and
async/actor/cancellation native execution remain whole-goal requirements.

## Checked Resource HIR/MIR handoff

The original checked Resource manifest and exact Source hook calls now survive
HIR-to-MIR lowering, including unused hook validation and lexical argument
order. The five focused Source groups pass (`852dfc`). The formatted complete
phase gate passes **522/522**: 191 HIR, 198 MIR, 70 codegen and 63 object-emission
tests (`961b04`, `target/native-resource-manifest-metadata-fixed-phase.log`).
Earlier foreign nested-type rejection is pinned at manifest validation, with
the lower native type gate still tested independently.

This is checked metadata preservation. Native Resource custody operations,
ownership/drop CFG, registration/ABI and linked Source execution remain pending.
These phase receipts do not extend the separate 480-case native caller receipt
or foundation-only platform acceptance. The fixed 207/182 inventory, approximate
85% feature estimate and whole-language 100% objective remain unchanged.

## Native Resource registration protocol

The bounded private runtime metadata issuer now validates every layout row and
issues context-bound nominal kind/slot metadata without creating a provider,
payload, grant, owner or loan. Exact guarded application and independent
byte/API review precede the initial **9/9** registration replay (`f753aa`). A
separate containing-function signature consistency correction adds an unused
Return-frame refusal control. The formatted full runtime passes **167/167**,
including ten registration and thirteen core custody controls (`56e7ac`,
`target/native-resource-registration-formatted-library.log`).

These are runtime Rust protocol tests. The new metadata issuer has no connected
native Source ABI or provider, and the compiler still refuses Resource execution
without the dedicated ownership/drop plan. They do not enlarge the separate
480-case native caller acceptance, fixed 207/182 inventory or approximate 85%
estimate. Matching native Resource linked execution in both profiles, then full
providers, aggregates and concurrency breadth, remain required.

## Source-authenticated ownership handoff

The original checked HIR/type archive and constructor-emitted MIR witness now
support a separate `ResourceOwnershipPlan`. It records exact scopes, operation
and provisional-return frames, owner holders, incoming views, bounded loans,
transfers, occupied sum extraction and cleanup obligations. Original/current
Source call associations preserve bare owned Resource-to-view custody without
creating ordinary owning operand claims. Copied queries, changed call sites,
operands, Source certificates, evaluation order and nominal type meanings are
refused. Unnamed Resource-bearing intermediate expressions retain their witness.
Canonical block/local compaction retains exact current associations.

All **12 focused ownership groups** pass (`155203`,
`target/native-resource-ownership-associated-focused.log`). The Source-derived
missing nominal-table regression passes (`fe7b0a`), returning the precise error
without a panic. The original nominal fixture retains both field-corruption
oracles after copying the complete original nominal tables and selecting its
actual Source Holder type. The malformed-type phase controls exposed a validation
ordering bug; valid type/invocation metadata is now required before custody
queries, preserving the exact manifest diagnostic and independent scalar refusals.

The complete formatted affected gate passes **535/535** (192 HIR, 210 MIR,
70 codegen, 63 object-emission), plus 32 type-library tests and one MIR doctest
(`3c6a8a`, `target/native-resource-ownership-valid-metadata-phase.log`). Earlier
failed runs remain diagnostic evidence. This is initial compiler ownership
analysis: connected native frames, provider/ABI operations, operational cleanup,
handled actual normalization, complete aggregates/reflection and linked binaries
executed after source deletion remain required. The separate 480-case caller
receipt, fixed 207/182 inventory and approximate 85% estimate are unchanged.

The broader driver/frontend gate also passes **693/693**: 87 driver library,
8 backend-lowering tests (including all 182 run-pass lowering obligations) and
598 frontend fixtures (`450ea4`,
`target/native-resource-ownership-driver-frontend.log`). The previous registration
commit's Windows CI job failed its CLI native-setup capture test with a main-thread
stack overflow; that separate platform regression is under investigation and is
not represented as current ownership or whole-workspace acceptance.

## Native context adapter and custody operations

The private runtime adapter connects the existing authenticated stationary
context lease, bounded layout issuer, registry and custody core. Exact current
attempt, frame, owner, loan, prepared hook and Resource sum records retain the
non-cloneable core tokens. Ordinary and Resource lookup IDs share one checked
monotonic allocator. Installation publishes a scripted provider and paired
ordinary Network grant transactionally after preflight; that provider is test
only. Production state starts absent with disabled providers and no new exported
Resource leaf.

Real core operations now cover root entry, Operation frames, direct/descriptor
hooks, acquisition, moves, bounded borrowing, close/drop, occupied sums, staged
replacement and protected reverse cleanup. Completion preserves the observed
body status verbatim and separately selects cleanup, host panic and current
ordinary/Resource body failure. Ordinary FailureTake is neither invoked nor
reset by the adapter. Refused transfers retain the old owner; a cleanup fault
cannot publish success. Borrow domain Fail stays ordinary Result data.

Initial compilation found three private API/field-name integration errors. The
first runnable focused check passed 13/14 and exposed string-only cleanup being
used for an ordinary borrow Result. The bridge now uses the existing recursive
native destructor, and an additional real-provider BorrowFail control checks
sum/string retirement, duplicate-drop refusal, clean completion and exact events.
All **15/15 focused groups** pass (`fd8577`,
`target/native-resource-state-adapter-companion-fixed-focused.log`). After
formatting, the full runtime package passes **182 unit + 13 integration tests**
(`96d995`, `target/native-resource-state-adapter-formatted-runtime.log`). The
earlier compile/failure logs remain diagnostic evidence.

These are runtime host-protocol controls, not linked Source execution. Exact
Source child Scope/Return activation, resident incoming views, connected C leaves
and emission, operational compiler cleanup, the matched archive and linked
source-deleted binaries in both profiles remain required. The adapter-head Windows
CLI setup-capture stack overflow is recorded separately; the later helper
repair below supersedes that pending local regression. Broader
aggregate/reflection/refinement, provider and concurrency coverage remains part
of the full goal. Native caller 480/480, the fixed 207/182 inventory and the
approximate **85%** feature estimate retain their separate scopes.

## Windows lowering stack repair

The unchanged CLI native-setup capture test reproduced a main-thread stack
overflow at Resource ownership and adapter heads. The adapter-head Windows CI
also completed the native conformance suite with **480/480 passing** before the
workflow failed its separate CLI capture target (`b32d80`, workflow
`37244085637`). This is current adapter-head supported native execution evidence;
it still includes no native Resource lifecycle case.

The repair extracts the existing heavy HIR statement/equality/inline-function
branches and MIR expression/per-actual branches into private helpers. Original
Source facts, Resource call capture, preflight, lexical order, shared call-view
scope, rollback, spans, diagnostics and context restoration remain at the same
operations. Source fixtures, assertions, snapshots, launcher order and stack
sizes are unchanged. Exact guarded application reconstructs both files and all
39 hunks (`3a8820`); the independent reconstruction record agrees.

The original regression now passes **1/1** (`285533`), and the formatted full
CLI capture target passes **9/9** (`508636`). The formatted phase gate passes
**535/535** HIR/MIR/codegen/object checks plus 32 type tests and the MIR doctest
(`f8fe67`). The broader gate passes **693/693**: 87 driver library, 8 lowering
checks including all 182 run-pass fixtures, and 598 frontend fixtures (`647ede`,
`target/native-windows-stack-decomposition-driver-fixtures.log`). An initial
command used the nonexistent test target frontend; no driver test ran in that
attempt. The corrected fixture_suite command supplies the actual receipt.

Actual Debug assembly measurement (`779571`,
`target/native-windows-stack-decomposition-frame-measurements.json`) records
MIR lower_value decreasing from 169,448 to 15,272 bytes and the ordered-call
loop from 73,736 to 4,504 bytes. Its separate per-actual helper uses 70,104 bytes.
HIR lower_expression decreases from 117,624 to 106,200 bytes and lower_statement
from 108,552 to 100,216 bytes. These are individual function-frame measurements,
not a whole-program stack limit or proof of every recursion shape.

The local platform regression is repaired. Whole-head supported-host CI and
connected Resource Source ABI/emission/cleanup and linked lifecycle execution
remain required. The fixed 207/182 inventory and approximate 85% coverage
estimate remain unchanged; runtime/metadata checks do not supply the missing
Resource native execution.

## Resource execution family and layout prerequisite

Resource-free original direct Source callers now belong to the authenticated
Resource execution family. Fresh whole-program ownership plans project exact
formal facts, sum shell roles, Scope/Return distinctions and the actual
driver-selected entry into deterministic immutable v2 layout metadata. The
readonly object accessor emits no Source body or provider execution.

Eight focused groups pass in both compiler profiles. The formatted compiler
gate passes **543/543** plus the MIR doctest (`fc3f02`); the final Return
projection passes all **74 codegen and 63 object checks** (`45ddfe`). The driver
gate passes **693/693** (`b9de51`). Corrections preserve the existing owned
runtime-capability entry rule and private constructor authority. See the
[implementation receipt](native_resource_hook_identity.md#resource-execution-family-and-emitted-layout).

Native Resource Source execution remains unaccepted until dedicated body/ABI
emission and cleanup run in linked binaries with Source inputs removed. The
native **480/480** result remains the separate adapter-head Windows test receipt;
it contains no admitted Resource lifecycle case. The approximate **85%** feature
coverage estimate remains unchanged. Remaining aggregate, captured/reflected,
refinement, provider and concurrency cases stay within the full goal.

## Native Source runtime transition gate

At the Source runtime successor, all nine focused Source controls pass, as do
three v2 registration, two all-or-nothing transfer and fifteen retained adapter
controls. The repaired formatted runtime package passes 196 unit and thirteen
integration tests (`fd04d3`). One historical malformed-header assertion was
updated because v2 is now supported; unsupported versions remain refused.

This supplies host/runtime and C-boundary evidence only. It does not add a
Source-native lifecycle fixture or close the Resource execution row. Generated
bodies, ordinary companion cleanup, matched archive and Source-deleted linked
runs remain pending; whole-language parity remains the goal and the broad
estimate stays about 85%. See the [runtime handoff](native_resource_hook_identity.md#native-source-runtime-and-custody-leaves).

The final Source runtime successor additionally passes three causal ordinary
companion ownership regressions and five exact root-outcome controls. The final
formatted runtime gate is **204 unit plus thirteen integration tests**
(`716d31`). The malformed companion controls were observed failing before the
fix (`bf61fa`) and passing after it (`5af1b8`). Root-outcome evidence (`4f200a`)
preserves original body zero independently of cleanup failure and rejects
active/stale/foreign or wrong-purpose observations before effects. These remain
host/C protocol checks; generated Source-native lifecycle admission is pending.

## Resource ordinary companion compiler gate

The fresh-plan companion analysis passes three both-profile Source groups and
**546 HIR/MIR/codegen/object checks plus the MIR doctest** (`f48b08`). It preserves
ordinary initialization/liveness/alias/move/type/Source call checks and excludes
Resource endpoints from ordinary owned storage. Current expression identity
selects already validated roles; public ordinary Resource refusals remain.
Readonly acquisitions borrow the same fresh immutable program. This compiler
gate does not establish emitted Resource bodies or linked lifecycle execution;
the fixed inventory and approximate 85% feature estimate are unchanged.

## Direct Resource body and matched archive checkpoint

The direct Resource emitter's five original-Source object/ABI groups pass in
both compiler profiles. The formatted codegen/object/HIR/MIR gate passes
551 checks plus the MIR doctest; a detected ordinary CFG regression is repaired
by allocating Resource failure blocks and cleanup slots only in the selected
family. Ordinary runtime checking passes 217 tests. Four exact report encoder
controls and six actual driver decoder controls also pass.

Both private Windows MSVC runtime profiles are compiled and inspected with
the actual static-CRT flags, native-library vector and discovered SDK/linker.
Each archive contains exactly one main, eight private test exports and 32
Resource leaves. The production archive excludes private main/test exports.
The HOST C ABI controls pass with each measured archive, including malformed
input, foreign/stale context, repeated entry and output-capacity refusals. These
handwritten controls add no Source coverage. Their actual primary report shows
zero obligations before destroy and exact Constructed/Finalized events.

The real reference gate and native gate each pass five original Source scenarios
under both profiles: connected ownership/borrow/return/close, factory failure,
later-actual ordinary failure, implicit drop and finalizer panic. Native binaries
run after both Project and synthetic Stdlib directories are deleted. The strict
report checks original body/cleanup outcomes, messages, events, exhausted script,
ordinary storage and every Resource obligation before exactly one destroy.
These are ten native executions in one conformance group, separate from the
earlier 480-case corpus. The first local Windows compiler-test slice is accepted;
production provider and supported-host distribution acceptance remain pending.

The actual broader driver gate passes 693 tests, including all 182 lowering
fixtures. Indirect/aborted/Replace and broader Resource semantics remain within
the full goal. The fixed 207/182 inventory and rough 85% feature estimate retain
their separate scopes. The private measured library order begins bcrypt then
advapi32; the production canonical vector reverses those first two. The reviewed
private prerequisite retains actual measured ordering and changes no production
launcher validation or installed manifest.

### Plain Resource replacement pre-effect validation

At the runtime successor to cc06aa1f, `native-resource-replacement-preflight-focused.log` passes four replacement groups and `native-resource-replacement-preflight-runtime.log` passes 220 runtime checks. Exact loan, consumed-handle and private generation-exhaustion controls preserve both owners before any old finalizer, then verify valid once-only cleanup. No wire or C ABI change is made. This evidence does not establish Source-native mutable replacement, aggregate replacement, or current supported-host acceptance. The earlier 486 native regression result and ten Source runs remain evidence for cc06aa1f. Rebuilt matched archives and Source replacements are required next; overall tracking remains about 85%.

### Source plain Resource replacement acceptance

The formatted Source replacement successor to `2b9442b1` passes 558 compiler/object/HIR/MIR checks plus one MIR doctest, and all 606 frontend/backend-lowering fixture checks. `native-resource-replacement-final-reference.log` and `native-resource-replacement-final-native.log` each pass eleven shared original Source scenarios in both profiles. The native launch deletes Project and synthetic Stdlib Source, observes exact events/body/cleanup/messages/scripts, retires every Resource/ordinary obligation before one destroy, and preserves body 255/cleanup 255 for executed old-finalizer failure. Exact self-reseat emits no finalizer/transfer, while failed RHS evaluation leaves old available to its handler. The fixed 207 inventory/182 obligations and broad 85% estimate are unchanged.

The new measured archive receipt `54a38248` preserves exact CRT/library/symbol provenance; both HOST C ABI controls pass. The broad driver-library attempt is 86 passed/1 failed due to CaptureLinkerOutput PermissionDenied during pre-spawn capture setup; the exact isolated property replay passes. The error alone does not identify capture creation versus second-handle opening. This is not a clean full driver-library result. The previous 486 native regression result remains specific to `cc06aa1f` until the complete suite is rerun on the new commit. Conditional/aggregate/projected replacement, other Resource execution and supported-host production distribution remain unproven.


### Exact native Resource replacement regression

The clean pushed `cda86ac7` Source replacement commit passes the complete default
native gate: 486 tests, zero failures and one ignored Resource group, in 1355.37
seconds. All 2099 recorded Rust/Jett/Cargo inputs remain exact after termination.
The ignored group was separately executed in all 22 native and 22 real-reference
scenario/profile runs. The earlier 86/87 driver-library result remains a recorded
capture setup failure; its isolated pass alone did not make that broad gate clean.

### Retained native output capture

The driver retains append-only capture files, gives children owned duplicate
handles, and reads bounded snapshots with explicit offsets. It no longer opens
the second file object through ReOpenFile. Private error context identifies
creation, duplication or snapshot while retaining the original I/O source.
The original PermissionDenied log proves a pre-spawn setup failure, but does
not distinguish creation from reopening or establish an external cause.

After correcting the authored Windows test commands' quoting without changing
their stream/status/deadline oracles, all 12 capture checks and three command
checks pass. The unchanged property replay regression passes twice in isolation,
then the complete driver library passes all 92 tests (152.85 seconds). Existing
Source deletion, both optimize modes, 104 trials, trial-4 sentinel, counterexample
and original-stream requirements remain intact. Child polling/kill/reap and
deadline behavior are unchanged. This verification repair does not increase the
about-85% feature estimate. Full native and supported-host acceptance of the next
combined Source implementation remain separate gates.

## Checked Source call staging and active-frame sums (2026-10-05)

Direct Resource calls with handled actuals now lower to constructor-owned typed
Begin/Stage/Invoke/End regions. The sealed witness keeps the complete original
Source tuple and current graph; fresh CFG analysis proves lexical argument order,
arrived-prefix custody, exact endpoints and retirement before a handler Return
operand. Complete prepared Source and borrowed-formal metadata exists even when
Invoke is skipped. Public graph edits cannot recreate that authority.

A genuine first-factory failure initially returned native protocol refusal after
ConstructionFailed, while the reference returned cleanly. The repair separates
Scope storage from active execution: a private exact-site projection supplies the
current frame for sum observations, transfers and extraction. Runtime top-frame,
slot ancestry and original Source checks remain strict. Both first-factory
success and failure now execute with their original outcomes.

Eighteen shared Source cases pass in both reference and native profiles (36
executions each). Native launch follows Source deletion; exact body, cleanup,
channel, messages, events, script consumption and zero obligations are observed
before teardown. Seven additions cover minimal and original-helper abandonment,
successful invocation, retained written views, first-factory failure and success,
and cleanup panic. The formatted compiler/object gate passes 573 checks plus the
MIR doctest; 606 frontend/lowering fixture checks, all four inventory gates
(including 182 host objects), and the complete 92-test driver library pass.
The final formatted Source replay also passes all 36 reference and 36 native
executions. The full current native regression and supported-host jobs remain
separate release gates.

Whole-language coverage remains about 85%. Indirect/refinement/aggregate calls,
static region selection, broader exits and projections, production providers and
shared scheduling remain part of the full objective. These local Source results
do not establish complete native parity.

### Return-prefix and completed failure propagation (2026-10-06)

The shared original Source gate now passes 22 scenarios in both reference and
native profiles (44 executions each). Later handled argument failure retires
both temporary view owners in reverse lexical acquisition order. A handler
Return retires its earlier temporary owner before its original helper borrows
an independent retained owner. A default continues to one invocation; actual
cleanup panic prevents the Return helper and still cleans the retained owner.
All native programs launch after Source deletion and require exact channels,
messages, events, script consumption and zero obligations before one destroy.

Two causal mismatches were repaired without changing those oracles. The reference
Return operand originally ran before pending-operation retirement; ordinary
callables then exposed an inherited caller-floor regression. Each callable now
has its own boundary. Native cleanup already retired both owners, but SourceStatus
converted the callee's recorded status 255 into generic refusal 1. A private
terminal completion record preserves original body and actual cleanup separately,
while exact caller/Source identity and retired Scope/Return guards remain strict.

All 508 reference-library, 92 driver-library and 222 runtime checks pass. Six
Return-prefix controls cover floor/Scope/receipt/refusal/panic behavior; eleven
native Source controls include exact body 71, clean body 0 with real cleanup
panic, wrong activation/tuple and absent completion authority. Fresh measured
single-runtime archives in both profiles retain one main, eight private exports,
32 Resource leaves, static CRT, ABI 1 and layout wire 2. The source-deleted gate
passes against those archives; production providers remain disabled.

The parent a30ca97e source tree has all four supported-host jobs green, with
full workspace and 486 default native tests on each host. Its local full run's
26 deadline failures subsequently passed unchanged deadlines in 25 plus one
replays; that does not replace a clean complete local run. Full successor
workspace/platform release acceptance remains distinct from the local gates.
The full 100% objective is active, with the coarse estimate unchanged at 85%.


### Required Resource values and exact ordinary helper bodies (2026-10-06)

The latest local shared Source checkpoint is 61 scenarios and 122 executions per
backend in debug/release. It supersedes the earlier 22/32/46-scenario checkpoints
above. Required evaluation runs once through a provider-free checked cache before
runtime registration; the native path retains original Source provenance through
materialization, production value conversion, MIR validation, object emission,
linking and Source-deleted launch. Exact events, script consumption, failure
channels and zero obligations before teardown remain mandatory.

All nine required-value MIR controls and 251 MIR library tests pass. The genuine
scalar-helper baseline accepted a changed required value and canonical pruning;
current body-only proof capture rejects both without changing that helper's
ordinary calling convention. Typed sum headers and normalized operation nodes
also participate in disconnected-custody detection. Exact guarded absence still
creates no Resource payload owner or finalizer at runtime.

These results are local MSVC private-archive acceptance. GNU private execution,
full current workspace/default-native/platform gates and the remaining semantic
families remain separate requirements. Whole-language tracking remains about 85%;
the full 100% objective is active.

The complete affected codegen suite passes 100 library and 63 object tests;
all 93 driver-library tests also pass. Final Rust/Jett formatting and whitespace
checks pass, and all 2,168 frozen source/config inputs revalidate unchanged.

### Verified borrowed optional/result Resource payloads (2026-10-07)

Sources 45..56 add twelve scenarios covering written/bare None/Some/Fail/Ok,
named-argument prefix abort, and handled borrow-domain failure. The unchanged
baseline passed all 73 shared scenarios / 146 reference executions but refused
Source45 at HIR stable-origin admission. The complete corpus now passes 146
Source-deleted native executions against freshly measured Debug/Release MSVC
private runtime archives, with exact events and zero obligations before teardown.

All 305 checker, 203 HIR, 254 MIR and 215 runtime library tests pass. Three focused
MIR groups and six runtime groups cover original-body custody, guarded projections,
conditional shell lifetimes, failure companions, argument aborts and forgery.
The full codegen suite passes 102 library and 63 object tests, including unchanged
prior invalid-MIR controls. Archives contain 37 Resource leaves, eight private
exports and one main; independent source/symbol/CRT receipts remain mandatory.

Admission is limited to root-body Resource aliases. Nested/scoped aliases,
ordinary non-Resource borrowed sums and alternate Default payload lifetimes remain
pending. The rough full-language estimate stays about 85%, and the full 100%
native-codegen goal, GNU Resource gate and broader language/platform requirements
remain open. See [the integration contract](native_resource_borrowed_sums.md).

All 521 reference-evaluator and 93 driver-library tests pass. The existing
Source33 required-value assertion now verifies its exact two original proofs,
including scalar 761 and the unique checked hook; other fixtures retain exact
one-value counts. This separately fixes the sole failure observed on both hosts
at 17d6c841. Rust and all twelve new Jett Source format checks pass, and the
2,188 frozen source/config inputs revalidate. The final MIR-only formatting change
is exactly one error-return line wrap and passes all 254 MIR tests again.


### Verified nested Resource lexical lifetimes (2026-10-07)

Sources57..95 extend the original shared corpus to 138 scenarios. The Source-only
baseline passed all 276 reference executions but refused native Source57 at exact
constructor-scope admission. Private declaration/exit records and ordered native
loan retirement now pass all 276 Source-deleted MSVC executions in both profiles,
including owned backing reuse, sibling identities, borrow failures, Break and
Continue. All 2,234 frozen inputs revalidate; runtime bytes and the measured
37-leaf ABI1/wire2 archives remain unchanged.

All 261 MIR, 103 codegen library, 63 object, 521 reference-evaluator and 93 driver
library tests pass. Six public object baselines and eighteen forged-exit refusals
bind codegen admission to exact private records. The rough feature estimate now
tracks about 86%, with unchanged fixed 207/182 inventory denominators. The whole
100% objective stays active. Ordinary borrowed list sums have a separate genuine
12-Source reference baseline and 24 native unsupported-handler refusals; their
implementation is not included here. Updated coherent-revision workspace/platform
and private GNU execution remain independent gates.


### Accepted ordinary borrowed-sum checkpoint (2026-10-08)

The ordinary borrowed-sum implementation is accepted at clean committed HEAD
`2c9f9aa8f2cd481c1d99ce6340ddf63688b2657a`. Its coherent locked workspace passes
**3,446 tests across 92 result groups**, with **0 failed and 1 ignored**,
including **491 default native-conformance tests** and the **182-object inventory
gate**. All **3,637 tracked inputs remain unchanged** after execution, and the
worktree remains clean. The receipt is
`target/native-ordinary-2c9-workspace-evidence/after-evidence.json`, SHA-256
`07fb0df1d059420c2705111a00da6d56b820e5e168eb37fc757c3fdadbe2dd1a`; its workspace
log SHA-256 is
`42657fc3b9ca894ee3a3a2d9b4997c1af044e15520f5b2031e6aeb01bf7b2e4f`.

The exact-revision [CI run 37673804359](https://github.com/vycdev/jett/actions/runs/37673804359)
passes all four jobs: Windows MSVC and Linux GNU build/conformance, plus both
relocated installed-package jobs. This closes the coherent-workspace and
supported-host workflow requirements for this accepted ordinary-sum revision.
It does not establish the later expanded Resource pipeline test packet or a
complete language/distribution gate.

The [ordinary borrowed-sum family](native_ordinary_borrowed_sum_codegen.md)
advances the broad planning estimate to **about 87%**. That judgment remains
separate from the fixed **207-fixture / 182-run-pass inventory**; neither
denominator changes. Other ordinary-carrier, Resource, reflection and semantic
obligations remain open.

### Source-only Resource pipeline checkpoint (2026-10-08)

The [pipeline packet](native_resource_pipelines.md) has measured **23 cases / 46
Debug-and-Release attempts** through each distinct local gate: checked Source
reference execution, Cranelift object emission, and linked native execution with
Project and synthetic Stdlib Source files absent at launch. The reference focus
is included in the accepted **161-case / 322-profile-execution reference
corpus**, with **522 reference tests passing**. The reference observation tuple
proves exact events and zero outstanding owners/registry entries; it does not
measure script consumption. Native reports additionally require zero remaining
scripted inputs and zero obligations before teardown.

`target/native-resource-pipeline-source-evidence/acceptance-v2.json` records all
**3,648 frozen repository inputs unchanged**; both matched local MSVC archives
and six nested receipts separately revalidate. Provider-panic retirement and cleanup-error precedence now
pass through genuine Source pipeline fixture 20: body/cleanup/channel and exit
are respectively **1/0/2, 73** and **1/255/3, 71**, with exact runtime-operation
and cleanup-error messages. These are tests of existing behavior, with no
language or ABI change.

The preserved full **161-case / 322-execution linked native corpus passes**
(1256.05 seconds), with all frozen inputs and measured archives unchanged.
Its separate receipt is `full-acceptance-v2.json` in the same evidence directory.
Object emission is not execution, and these matched local MSVC runs do not prove
GNU or other platform acceptance of the expanded packet. Broader Resource
`For`/`Match`, reflected bodies and ordinary families remain separate work. This
checkpoint leaves the **207/182** inventory and **about 87%** estimate unchanged.

### Expanded Resource For/Match transport (2026-10-08)

The [control-flow successor](native_resource_control_flow.md) preserves custody
through consuming Resource-free list ForEach and ordinary Resource-free enum
Switch, without a language or ABI change. Constructor-owned reconstruction
authenticates the three temporary headers, source/cursor/length initialization,
extraction/increment prefix and original CFG; independent original/current
borrowed, lexical, call and descriptor proofs remain mandatory. Strict joins and
final raw ForEach refusal are unchanged. Historical v2 acceptance remains
22 cases / 44 Source-deleted MSVC executions, three focused MIR groups and
450 affected phase checks.

V3's seven accepted Source files 01..05/07/08 cover consecutive and nested loops
as well as Fallthrough/Break/Continue and both unit-enum arms. All 30 cases /
60 focused Source-deleted MSVC executions pass in Debug/Release (204.35 seconds),
with exact outcomes/events/channels/messages, script exhaustion and zero ordinary/
Resource custody before teardown. Reference zero-owner/registry observations do
not measure that full native tuple. The receipt
`target/native-resource-control-flow-source-evidence/focused-acceptance-v3.json`,
SHA-256 `7f41c389daa268942afe8a645085daac18d746f553f3be32e93d14f798fc30ea`, records
all 3,658 frozen inputs and both measured archives plus six nested receipts
unchanged at HEAD `50ad73a0693c502d1fa05720091b80a4ab623104`. Four focused MIR
groups pass; 89 Resource reference groups include all 191 cases / 382 profile
executions, and final object preflight separately emits 60 objects.

The clean base `50ad` workspace independently passes 3,448 tests with zero
failures across 92 result groups. That is separate from the modified successor's
coherent-revision gate. Full 191/382 Source-deleted MSVC native execution now
passes in 1,288.83 seconds. `full-acceptance-v3.json` records actual exit zero,
all 3,658 frozen inputs, both measured archives, six nested receipts and the
executable unchanged; its SHA-256 is
`08bcf865bfed8a9b49f262a6c611336260cd0446ba8517139c71ab10627fb806`.
Successor workspace and exact-revision CI acceptance remain pending. Source06's
separate GNU reference repair passes authority, recursion and pure control-flow
tests plus 74 strict Resource entry attempts, including same-grant reentry after
failures; `after-v3.json` records all 3,662 inputs unchanged. Its native
HIR/MIR/codegen ordinal bridge remains pending, and no GNU native acceptance
follows from those reference results.
Erased carriers, borrowed/projected/Never-element or other iterators and broader
For/Match/reflected combinations remain compiler proof obligations under existing
Source semantics. The full goal stays active at the rough 87% planning estimate;
fixed 207/182 inventory denominators remain unchanged.

### Checked reflected-field reference repair (2026-10-08)

[Direct field selection](native_resource_reflected_fields.md) now passes all
541 comptime tests, including 19 added authority/recursion/control/metadata and
Resource tests. Strict original Source06/derived controls pass 74 entry attempts
in both checking profiles, including same-provider/grant reentry after every
selected failure. `after-v5.json` records all 3,664 inputs unchanged at the
complete crate gate. This proves checked reference behavior; native reflected
Resource emission, Source-deleted execution and the full goal remain pending.
