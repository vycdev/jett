# Canonical value conversion proof identity

Status: implemented with local compiler and native view-regression acceptance
on 2026-10-16. Nine focused controls, 1,856 core-library tests, 603 fixtures and
nine CLI capture tests pass. The current-image native regression passes 24 tests
and 48 paired Source-deleted executions. Native execution of the new compound
required-callable and suite witnesses remains unproven. GitHub CI stays disabled.

## Construction and replay

Canonical value coercion can resolve and bind freshly materialized reflected
callable plans. Independent calls to the coercer create distinct private Arc
identities even when every Source expression and checked type is the same.
Therefore an independently recomputed table cannot authenticate the one actual
canonical construction used by a temporary, actor or mutable-sum proof.

Within value-conversion completion, the private receipt surrounds its single
actual coercer execution.
It retains boxed complete before/after Programs and borrows the live TypeInterner.
Replay requires the same arena address, complete ordered function table with
float-bit equality, Resource manifest, equality methods and complete private
archive identity. It returns the recorded canonical output, preserving all new
occurrence tokens and adapter bodies. Callers supply no output table or callback.

Each peer still validates its original proof and compares every actual operation,
header and callable identity. Existing independent actor and mutable-sum metadata
checks remain in force. Graph, baked reflected values and temporary views are
joined only after seeing the same unchanged-before archive. Final callable
sealing retains its separate exact old/new correspondence.

Completion mutates a local Program candidate and publishes it only after all
peer joins, HIR validation and backend-type validation succeed. The ephemeral
receipt is never stored in HIR or an archive, so its snapshots cannot refer back
to it. No runtime layout, ABI, provider, custody or Source semantics change.

Archive equality compares all private optional Arc identities even if the
original Source data is absent. The current base has data, execution, reflected
and Graph identities. The separate native-suite packet must add its native_suites
and required_cache fields to this same complete comparison when merged; neither
identity may disappear behind an empty-original fast path.

## Local compiler verification and remaining gates

All nine private controls pass. They cover a genuine closed
required reflected callable twice beside an ordinary temporary owner, repeated
completion, exact recorded bound tokens, independent coercer output refusal,
foreign arenas/plans/archives, edited headers and bodies, dropped/duplicated/
reordered input and output occurrences/functions, hidden Arc identities, signed
zero and NaN payload bits, failed publication and receipt lifetime without a cycle.
The synthetic malformed-archive and NaN controls test private comparisons only;
they grant no Source acceptance or native execution credit.

The genuine compound positive passes in Debug and Release checking modes. Its
first authored Source used `assert` in main, which reports E0340 because asserts
are confined to Verify/Property blocks. The fixture repair uses typed call and
`coarsen` locals for both required callable occurrences. Every Rust proof
assertion remains intact; no producer, occurrence, archive, repeated completion
or sealing check was removed. The failed first attempt has zero accepted credit.
The corrected positive is included in the complete nine-control gate.

The existing genuine Graph/baked/temporary composition control also passes one
test in both checking modes. The full successor core gate passes **1,856 tests**:
codegen 135, Comptime 683, HIR 280, MIR 379 and typechecker 379. The focused
controls are included in HIR's total. The full frontend/reference fixture gate
passes **603 tests**, and the unchanged Windows CLI capture suite passes **nine
tests** under its existing stack limit. Logs under `target/native/`:

- `canonical-conversion-compound-positive-v1.log` (failed), SHA-256
  `f4891373dff5eacdf2c9a1e175f8eb67f6eff0ca6f33324bcd812334142f1c99`.
- `canonical-conversion-compound-positive-v2.log`, SHA-256
  `fa986958329d859de65978f79ec9ea14a815a77c66fa0533f890b1023f01abca`.
- `canonical-conversion-focused-v1.log`, SHA-256
  `38c3a8d6d82b51e86113dfb633525d6242c1531d6200c67f31f71a04e5ddaf5c`.
- `canonical-conversion-graph-baked-temporary-v1.log`, SHA-256
  `fb2129d0a82a5e1c62b7dcd2c13859a9add435fad6e18e83bca3604d79d08f06`.
- `canonical-conversion-core-v1.log`, SHA-256
  `b712b29d7f69f505131282b590218c5242e3af5ede42494b01717b45b14691a0`.
- `canonical-conversion-fixtures-v1.log`, SHA-256
  `89dd139d83b8821d845158c2a74fd644270155f87cee02da2c54c1c182c5f94f`.
- `canonical-conversion-cli-v1.log`, SHA-256
  `45cafe79af2294857dbb443e83c7b4db81e728c3cfa0e7ae4ae821b8d26ef279`.

## Current-image native view regressions

The fresh MSVC gate passes compile-only acceptance, with zero Source execution
credit. Its subsequent existing-view regression selection passes **24 tests and
48 paired Source-deleted native executions**: 24 Source Debug/Runtime Debug
and 24 Source Release/Runtime Release. No publication refusals, legacy
Debug-only runs or full profile cross-product are counted. All 16 selection
logs have their pinned hashes and exact zero-failure test counts. The written
Source is removed before the binaries execute, with exact reference/native
stdout, stderr, status and debug-observation checks. All 1,060 compiler/Source,
68 reused Runtime, 65 fixture and 182 original run-pass input guards remain
unchanged. Receipts under `target/native/`:

- `canonical-conversion-native-gate-v1/gate-acceptance.json`, SHA-256
  `41ca89ea180268d5df296355dc45fded45c98bcd0676cfa1d86fc896cd94f637`.
- `canonical-conversion-native-views-v1/execution-summary.json`, SHA-256
  `fda6e2118a9a00409a45ea53429f490b1010242523877d9feb305f70e2097394`.

Both pin the compiled and executed gate image to SHA-256
`b37a2366042a3e0326825156eb151e72c3026cc3d593516400b78f5cb1fc6695`.
The selected bytes/List temporary, stable, scoped-call, mutable-owner, reflected
and actor persistent-view controls provide current-compiler regression
acceptance. They do not execute the new compound required-callable or Source17
Verify/Property suite witnesses. Those native obligations remain unproven.
Historical byte/List matrices are not added to this distinct 48-execution total.

The separate native-suite transition and its Source17 compound control are still
unapplied. They require fresh required-callable composition and exact Verify/
Property occurrence order after these packets merge. Linked Source-deleted
Debug/Release runs, provider/custody counters, cleanup and reentry remain actual
execution obligations. Equal sampled Property values must keep distinct ordinal
occurrences; Source17's configured trial count is not a universal suite count.
No provider/custody result may be inferred from its lack of Network parameters.

A required callable in an actor handler, or a constructor carrying actor-state
views, still needs genuine checked coverage of the exact bound-to-sealed token
transition. The ordinary compound positive does not establish it. The historical
actor witness is unexecuted and must first use a valid Source observation in
place of its main-body assert; a predicted snapshot mismatch is not a measured
failure. Nested callback-return proof identities also retain their own coverage
obligation. Preserve complete snapshots instead of weakening them to admit a
current body. Full temporary lifetimes and the 100% native-codegen goal remain
incomplete.
