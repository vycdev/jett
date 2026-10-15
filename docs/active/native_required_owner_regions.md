# Original required regions inside inline bodies and refinement predicates

Status: local acceptance recorded below; exact-revision supported-host and
installed acceptance remain open.

Published Graph checkpoint `9113f395039afe41752be98f6fba55588af01ae6`
failed the same two required-evaluation checks on Windows MSVC and GNU. The
contextual comptime reference oracle reported three E9001 errors; the refinement
declaration oracle reported one. Those were checked materialization failures,
not timeouts or native Runtime mismatches. The local repair is verified only by
the receipts below; it does not establish new GNU execution or successor CI.

The repair adds private required-region paths into exact retained inline bodies
and an exact checked refinement-predicate origin. It keeps the enclosing
declaration, concrete generic instance and ordered scoped selections. These
paths are restricted to explicit required evaluation and grant no runtime
Resource or capability permission. The existing closed and pure checks remain
authoritative, and the ordinary executable walker still stops at callable
boundaries.

Required-value proofs reconstruct the same original occurrence and context.
Private HIR extraction records join an original inline node to its canonical
extracted function and enclosing owner. Refinement proofs join the exact checked
declaration and original predicate HIR. Resolver names use the canonical
namespace and leaf; HIR declaration identity retains the original alias leaf.
Both must match their respective retained metadata. DefId, kind, declaration
span, namespace, origin, constraint, original header and operand checks remain
required. Full original/current archive checks, the value matcher and
once-per-original-attempt evaluation remain in place.

The original contextual and refinement fixtures and expected interpreter
streams remain unchanged. Focused controls cover physical clones, foreign
checked programs, wrong enclosing owners, generic and scoped selections,
callable-boundary ancestry, changed extracted headers and operands, wrong
refinement declarations and alias names, runtime-purpose refusals, and metadata
restoration before reentry.

Focused acceptance: 20 tests across 17 actual invocations, with no failures or ignored tests. Receipt:
`target/native/required-owner-focused-v2/focused-results.json`, SHA-256 `14494e22ef31e941ea97e87fb2a06dee503e9daf929279bd633150aae0d5f96e`.
Broad affected-library acceptance: 1,334 complete library tests (Cranelift 134, Comptime 624, HIR 229 and MIR 347), plus 25 selected Driver tests (library 14, backend lowering 7, generic function values 2 and named arguments 2), all with no failures or ignored tests. Receipt:
`target/native/required-owner-broad-v3/broad-results.json`, SHA-256 `4241e0b95302f388f2d748f2c8cbb6599262541d66b1af72723ed479c82e5797`.
The broad run's final guard authenticates 2,099 current Source/fixture inputs
and 68 unchanged Runtime artifacts. Its library run completed successfully in
v2; v3 resumed only the Driver batches after independently checking unchanged
inputs. The later test-only restoration extension passes one focused control:
`native-codegen-recovery-2026-10-09/scalar-v9/required-owner-restoration-acceptance.json`, SHA-256 `a6ba8bf827ff749e400c7460ef7cf5d39399c541b4f1c77236fc06dc19f691e0`.
It checks same-program failure, full metadata restoration and successful reentry.
This changes no production code. The earlier 20-test and native receipts precede
that test-only extension and retain their own snapshot; the final breadth
includes it. The two native fixtures and their expected streams remain exact.

Both unchanged original selectors, contextual comptime and refinement
declaration contexts, pass two Debug native main executions and two interpreter
main executions with Source present. Receipt:
`target/native/required-owner-native2-v2/after.json`, SHA-256 `a8e1014f3c7f79d10bff2b0077f353a9357d69f74b5486681d3f90f3ce9f225d`.
Their original stdout and full rendered debug-event stderr match the reference.
These gates request no Release source check, execute no Release runtime, and
invoke no Verify/Property suite. They supply no Source-deleted lifecycle credit.
The existing measured Runtime archives are reused unchanged; guarding the
Release bundle does not establish Release execution.

These results close the bounded local RequiredOwner regression. Full workspace
and fixed-inventory conformance, exact-head MSVC/GNU and clean installed
distribution remain separate obligations. The generated VerifySuite transition
and Verify/Property capture-owner repairs retain their own evidence scope.
The rough 87% estimate and fixed 207/182 inventory denominators remain unchanged;
this does not complete the native-codegen goal.
