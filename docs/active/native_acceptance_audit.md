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

The [interface audit](native_interface_values.md) also retains the remaining
facade, refinement-composition, and comptime combinations that need scrutiny.
Passing its growing regression suite does not remove those audit obligations.

Native property failure shrinking and case-specific diagnostics remain open in
the accepted plan. Normal native builds already run frontend verify/property
checks before emission; a source-level failed property cannot be silently
bypassed to claim native failure coverage. Completion must either implement the
remaining native suite behavior under that policy or establish, from the actual
public execution contract, which diagnostics are supplied by the required
frontend stage. Passing properties alone do not resolve this requirement.

The standalone report intentionally keeps `complete` false while these release
obligations are unresolved, even when every fixture count is full. The historical
85% planning estimate is not a measured code-coverage or completion result.
