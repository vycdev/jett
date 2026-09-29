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
| Required comptime evaluation stays at compile time | Explicit-comptime evaluation/materialization tests, per-instantiation context regressions, and rejection of unbaked comptime expressions by codegen. No runtime interpreter fallback is an acceptance substitute. |
| Supported-host distribution | All four jobs in `.github/workflows/native.yml`: Linux GNU and Windows MSVC workspace/build/package jobs, followed by clean installed jobs with no checkout. Both runtime profiles must work after relocation and source removal. |
| Complete current workspace | `cargo test --locked --workspace --no-fail-fast` or equivalent test and doc-test invocations against a coherent revision. Rebuilding dependency artifacts during a run can invalidate later rustdoc inputs; such failures require a fresh doc-test run. |
| Stable documentation matches the implementation | Review `docs/design.md`, `docs/architecture.md`, the parity plan, and the notes below against the final implementation and verified gate results. |

The tested CI toolchain is Rust 1.97.1. Cross-compilation, future providers, and
asynchronous scheduling must not be inferred from host packaging or the current
sequential interpreter/native task model. Their implementation status remains
separate from parity for existing accepted behavior.

## Remaining semantic work

The following decisions remain unresolved and prevent a full parity claim:

- [Enum payloads containing user structs](../open_design/enum_payload_struct_equality.md): exact explicit equality versus rejecting those comparisons.
- [Actor equality](../open_design/actor_equality.md): actor identity versus rejecting comparisons.
- [Actor handles escaping comptime](../open_design/comptime_actor_values.md): reject escaping handles versus materializing a defined actor graph.
- [Erased interface equality](../open_design/interface_value_equality.md): explicit comparison versus a defined dynamic equality contract.
- [Concrete-owner arguments in erased calls](../open_design/interface_same_owner_arguments.md): runtime validation versus rejecting unsafe erased calls statically.
- [Secrets hidden in debug print values](../open_design/debug_print_hidden_secrets.md): extend redaction to erased/builder payloads versus reject potentially secret values.
- [Direct collection and sum equality](../open_design/direct_collection_equality.md): reject unsupported comparisons at compile time versus preserve their interpreter runtime errors.

The [interface audit](native_interface_values.md) also retains the remaining
facade, refinement-composition, and comptime combinations that need scrutiny.
Passing its growing regression suite does not remove those audit obligations.

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
