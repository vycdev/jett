# Settled Design Spec Fixtures

This directory holds language-level fixtures derived from settled, canonical
rules in `docs/design.md`.

These fixtures are intentionally not wired into the default `cargo test` suite
yet. They are a spec backlog:

- Promote a fixture into `tests/compile_pass`, `tests/compile_fail`, or
  `tests/run_pass` when we start implementing that slice.
- Add promoted fixtures to `crates/jett_driver/tests/fixture_suite.rs`.
- Only add settled semantics here. Do not add tests for open questions,
  ambiguous wording, or aspirational design notes.

Current pending batch:

- None
