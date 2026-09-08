# Benchmark protocol v0.2

This protocol retains the isolation, backend separation, generation, repair,
safety, and analysis rules in v0.1. It adds the first direct type-driven task.

## Matrix

The pilot now has 4 tasks x 5 languages x 2 tracks x 3 reasoning levels x 3
repetitions = 360 rows. The Codex-subscription calibration slice is 40 fresh
medium-reasoning rows with one repetition and no repairs. Running that slice
still requires explicit subscription-usage confirmation.

## Typed-domain task

`order_lifecycle` supplies equivalent closed state, event, error, and outcome
types for Jett, Python, TypeScript, Go, and Rust. A solution must implement the
same 20 state/event combinations. The primary observations are:

- whether the required static model compiles;
- whether all legal and illegal transitions are correct;
- whether accepted outcomes preserve the next-state type;
- whether the source uses a prohibited type-system escape hatch;
- source size, complexity, diagnostics, tokens, and repair behavior.

Each adapter uses its normal static checker before hidden runtime tests: Jett
build, Pyright strict mode, TypeScript strict mode with implicit-return checks,
Go compilation, and Rust compilation. Tool versions are pinned in the runner
image and recorded in results.

## Type-policy preflight

The task declares small, versioned forbidden-pattern lists per language. They
reject common ways to erase the required types or evade exhaustiveness, such as
`Any`, TypeScript `any` and suppressed diagnostics, empty Go interfaces, Rust
unsafe escapes, and catch-all/default branches. A rejection is recorded as
`policy_error` before generated code executes.

These policies are intentionally narrow and are part of the task version. They
do not prove that two type systems have identical strength. In particular, Go
does not provide compiler-checked exhaustive switches over named integer
constants. The benchmark therefore reports static-check success, policy
success, and behavioral correctness separately rather than claiming uniform
compile-time guarantees.

## Interpretation

The typed task is a calibration instrument, not yet a headline benchmark. A
ceiling across established languages, ambiguity in the required declarations,
or language-specific grader friction triggers a new task version before more
repetitions are collected. Onboarding and zero-shot remain separate treatments;
the onboarding comparison cannot isolate one paragraph of guidance from the
rest of each language reference.
