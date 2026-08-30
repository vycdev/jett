# Benchmark protocol v0.1

## Unit of observation

One row is one task, language, track, reasoning level, and repetition. Initial
rows use fresh context and must not share response IDs or conversation state.
The planned pilot has 3 tasks x 5 languages x 2 tracks x 3 reasoning levels x
3 repetitions = 270 rows. The frozen study raises repetitions to at least 10.

## Tracks

`zero_shot` provides the common task statement, language name, required
signature, and output rules. `onboarding` adds the versioned compact reference
for that language. Neither track exposes tests from `hidden.*` files.

## Generation and grading

1. Generate a complete file with a fixed maximum output-token budget.
2. Extract one fenced block if present; otherwise use the full text.
3. Save the raw response and extracted source separately.
4. Compile and run hidden tests in an isolated environment.
5. If repairs are enabled, provide bounded normalized diagnostics and the prior
   source in a new request. Never include hidden assertions or expected values.
6. Record every attempt. A successful repair does not rewrite the initial row.

Tasks impose deterministic input bounds. A timeout, crash, malformed response,
compile failure, or failed hidden test is a non-pass with a specific phase.

## Analysis

- Report compile rate, pass@1, and pass@k with sample counts for every cell.
- Use the standard estimator `1 - C(n-c,k) / C(n,k)` only when `n >= k`.
- Report tokens, estimated cost, latency, repairs, source size/complexity, and
  runtime independently; do not combine them into an opaque score.
- Preserve raw JSONL and the configuration used to aggregate it.
- Compare tracks separately before making any pooled comparison.

Pilot-derived edits invalidate only the affected task version. Pilot data is
kept for diagnosis but excluded from the frozen study.
