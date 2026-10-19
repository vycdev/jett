# Native codegen campaign handoff

Jett parses and resolves Source, checks types and ownership, evaluates explicit
comptime expressions, lowers checked values through HIR and MIR, and emits
native objects with Cranelift. The native linker combines those objects with a
target-matched runtime launcher bundle; execution uses the checked native ABI.

Native parity means all five fixture gates in `tests/native_parity.json` pass
at one commit on Windows MSVC (`x86_64-pc-windows-msvc`): lowering, host object
emission, inventory execution, verify suites, and property suites.

The verified landing SHA is `dd0d415238e29743780e6372a126dbda40f9845d`.
Its five gates passed (1, 1, 2, 1 and 1 tests respectively); the workspace passed
4,260 tests, failed 0 and ignored 26 in 2h 48m 56.51s. That run selected test
optimization level 1 through the environment. `Cargo.toml` now selects it with
`[profile.test]`, retaining the default debug assertions and overflow checks.

The final closeout SHA and plain-command verification result are recorded in
[draft PR #281](https://github.com/vycdev/jett/pull/281). Use `git rev-parse HEAD`
to identify this checkout's revision. GitHub CI remains disabled; the PR remains
a draft for review.

Rerun the five gates and workspace with no build environment overrides:

```text
cargo test --locked -p jett_driver --test backend_lowering run_pass_backend_lowering_gaps_are_explicit_and_monotonic
cargo test --locked -p jett_driver --test native_parity_manifest native_parity_object_emit_obligations_emit_host_objects
cargo test --locked -p jett_driver --test native_conformance inventory_execution
cargo test --locked -p jett_driver --test native_conformance native_verify_suites_execute_for_all_run_pass_fixtures
cargo test --locked -p jett_driver --test native_conformance native_property_suites_execute_for_all_run_pass_fixtures
cargo test --locked --workspace --no-fail-fast
```

[Remaining native gaps and language-rule review](../../active/native_remaining_gaps.md)
are follow-up work. The notes in this folder preserve campaign history.
