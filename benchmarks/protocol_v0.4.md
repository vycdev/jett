# Benchmark protocol v0.4

This protocol retains the v0.3 generation, isolation, static-check, policy,
and analysis rules. It adds four collection and parsing tasks. The onboarding
treatment uses language reference sheets at v0.5.

## Matrix

The pilot has 10 tasks x 5 languages x 2 tracks x 3 reasoning levels x 3
repetitions = 900 rows. The Codex-subscription calibration slice is 100 fresh
medium-reasoning rows with one repetition and no repairs.

## Paired compile-and-repair treatment

The one-shot calibration remains the primary result. An optional adaptive
second stage may be run over its failures:

1. Grade the first answer in the isolated no-network runner.
2. Stop immediately when it passes.
3. Otherwise send one new prompt with the same public task, the first answer,
   and bounded compiler feedback.
4. Grade the revised complete source once and record it as `compile_repair`.

Zero-shot and onboarding remain the context tracks; compile-and-repair is an
evaluation mode paired to either track. It is not an independent initial draw.
Raw diagnostics are allowed only when they refer to candidate source lines.
Diagnostics naming private files or appended private-grader lines are replaced
with a normalized category. Reports retain the original score and add repair
success, pass-after-repair, and repair token totals.

## Added tasks

- `first_duplicate` tests order-sensitive duplicate detection with a set and
  optional result.
- `merge_sorted_intervals` tests immutable input, structs, and list
  transformation over pre-sorted closed intervals.
- `inventory_batch` tests typed event variants, map accumulation, payload
  preservation, and first-failure reporting.
- `score_lines` tests exact string shape, canonical int64 parsing, duplicate
  detection, and typed error precedence.

## Interpretation

Report results by task and track before aggregation. In particular,
`score_lines` includes library knowledge and exact parsing semantics, while
`inventory_batch` includes collection ownership in Jett. Those dimensions are
intentional, but they should not be mistaken for pure algorithm scores.
