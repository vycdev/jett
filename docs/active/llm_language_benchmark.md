# LLM Language Benchmark

Status: active

## Goal

Measure how reliably coding agents can solve the same bounded programming
tasks in Jett, Python, TypeScript, Go, and Rust. The benchmark is a feedback
loop for Jett's language and tooling design, not yet evidence for broad claims
that one language is better than another.

## Stages

1. Freeze the supported Jett surface as benchmark subset `v0.1`.
2. Validate original tasks and hidden graders against known-good solutions.
3. Run a small pilot with GPT-5.6 Luna at low, medium, and high reasoning.
4. Inspect failures for ambiguous tasks, unequal documentation, grader bugs,
   and missing Jett tooling.
5. Freeze corrected tasks and run the preregistered study with at least ten
   independent repetitions per cell.

The pilot is not used for headline comparisons. A task changed after seeing
pilot results receives a new version.

## Experiment shape

- Languages: Jett, Python, TypeScript, Go, Rust.
- Tracks: zero-shot, controlled onboarding, and parity-matched programming
  skills.
- Model: a pinned GPT-5.6 Luna snapshot when available; otherwise record the
  rolling alias, backend, client version, order, and exact run time.
- Reasoning: low, medium, high.
- Context: every initial attempt is an independent Responses API request.
- Repair: an optional paired `compile_repair` mode gives each failed one-shot
  exactly one second prompt with its prior answer and safe compiler feedback.
  Candidate diagnostics are preserved when private grader code can be excluded;
  otherwise only a normalized category is provided.
- Budgets: identical output-token and repair limits within a task cell.
- Repetitions: three for harness validation, at least ten for a frozen study.

The controlled-onboarding track supplies a compact, versioned reference for
each language. Reference sizes are recorded so documentation exposure can be
reported rather than hidden.

## Metrics

Correctness is primary: compile rate, test pass rate, pass@1, and pass@k.
Efficiency measurements are separate: input/output/reasoning tokens, estimated
API cost, latency, tool invocations, repair count, source lines/bytes, a simple
branch-count complexity proxy, and grader runtime. Runtime is never folded into
LLM correctness.

Every result records the task version, language adapter version, prompt hash,
model identifier, requested reasoning level, track, repetition, toolchain
versions, git revision, and raw response identifier. Cost is derived from a
versioned price configuration rather than embedded in historical results.

## Safety and authorization

Generated programs are untrusted. The harness requires an explicit unsafe flag
for local execution; publication runs belong in an isolated disposable VM or
container with no credentials, no network, resource limits, and a bounded
working directory. The local flag is suitable only for repository-owned
baseline fixtures.

API requests are dry-run artifacts by default. Executing paid calls requires a
separate explicit flag and `OPENAI_API_KEY`; creating the benchmark does not
authorize spending credits.

Codex subscription calibration is a separate backend. It requires explicit
subscription-usage confirmation, verifies ChatGPT login, strips API billing
credentials, and records results separately from API experiments.

## Exit criteria for the pilot

- All known-good solutions pass every hidden grader on the recorded toolchains.
- Re-running aggregation produces identical summaries.
- No prompt leaks hidden cases or expected outputs.
- Failures distinguish extraction, compile, test, timeout, and harness errors.
- Jett failures can be traced to model behavior, documentation, compiler
  diagnostics, or an explicit unsupported feature.
- The task set is revised or frozen before the larger run.

Implementation and operator instructions live in `benchmarks/README.md`.

## First calibration

The 2026-08-30 Codex-subscription calibration completed 30 independent Luna
medium-reasoning cells with no API key, repairs, or observed tool calls. It
passed 27/30: every established-language cell and every onboarded Jett cell
passed; all three zero-shot Jett cells guessed invalid surface syntax and
failed compilation. The versioned artifacts are in
`benchmarks/results/2026-08-30_codex_luna_medium_calibration/`.

This validates the harness and the usefulness of Jett onboarding, but the task
set has a ceiling effect and cannot isolate type-driven guidance from syntax
instruction. The next benchmark step is to add harder tasks that exercise type
modeling and repair quality before increasing repetitions or reasoning levels.

## Typed-domain calibration v0.2

The first harder task is now `order_lifecycle`. It requires closed types for
five states, four events, two errors, and accepted/rejected outcomes, then
grades the complete 20-case transition product. Jett uses payload enums and
exhaustive matching; the other languages use their closest conventional static
model.

Every adapter now performs its language's static check before runtime grading.
Python uses pinned Pyright strict mode, TypeScript enables strict and
implicit-return checks, and task-versioned preflight rules reject common type
erasure and catch-all branches. Go's compiler cannot prove exhaustive switches
over named constants, so the protocol reports that limitation instead of
claiming identical guarantees.

The v0.2 matrix contains 360 planned study rows and a 40-row medium Luna
subscription calibration slice. That slice has not been executed: it consumes
subscription allowance and still requires explicit confirmation at run time.
Known-good baselines and the isolated image must pass before calibration data
is collected.

The 40-cell medium Luna calibration is now recorded in
`benchmarks/results/2026-08-30_codex_luna_medium_v0.2_calibration/`. Automated
grading passed 36/40; all four failures were zero-shot Jett compile failures,
while every onboarded cell passed. Manual review reduced public-contract
compliance to 35/40 because the zero-shot Go lifecycle submission used a
forbidden panic fallback that the v1.0.0 grader missed. That task version is
calibration-only. Task v1.0.1 and benchmark config v0.2.1 now reject
panic/throw shortcuts across Python, TypeScript, Go, and Rust before a larger
study.

## Recursive and maintenance expansion v0.3

The suite now adds `recursive_expression`, which evaluates recursive closed
data with typed division-failure propagation, and `account_state_evolution`,
which evolves supplied working source by adding a payload-bearing state. The
harness now supports versioned starter files and records their hashes.

The matrix is 540 study rows. Its balanced medium Luna calibration slice is 60
rows with no repairs. It should only be run after all 30 reference baselines
pass locally and in the no-network image; this v0.3 slice has not yet been run.

## Ten-task collection and parsing expansion v0.4

Four tasks extend coverage beyond recursion and state modeling:
`first_duplicate`, `merge_sorted_intervals`, `inventory_batch`, and
`score_lines`. They exercise sets, optionals, structs, list transformations,
maps, canonical integer parsing, and typed validation errors.

The suite now contains ten tasks, producing 900 study rows and a balanced
100-row medium Luna calibration slice. All 50 reference baselines must pass
locally and in the no-network image before that slice is run.

The 100-row medium Luna slice is recorded in
`benchmarks/results/2026-08-30_codex_luna_medium_v0.4_calibration/`. Automated
grading passed 80/100: Go 20, Rust 19, TypeScript 18, Python 16, and Jett 7.
Onboarding improved Jett from 0/10 to 7/10. Three Python failures passed hidden
behavior tests but were rejected by Pyright's strict unnecessary-`isinstance`
diagnostic; the official score remains static-check based.

The next treatment is now implemented as a paired compile-and-repair pass over
failed zero-shot and onboarding rows. It does not replace the 100 one-shot
scores: passing rows stop after prompt one, while failures receive one repair
prompt and are reported through repair success and pass-after-repair rates.

That repair pass is recorded in
`benchmarks/results/2026-08-30_codex_luna_medium_v0.4_compile_repair/`. It
repaired 10/20 failures and raised the paired result from 80/100 to 90/100.
Python, TypeScript, and Rust repaired every failed row; Jett repaired 3/13,
ending at 10/20. The remaining Jett failures were all compile-time failures,
showing that a single diagnostic usually does not replace onboarding for the
current language surface.

## Programming-skill expansion v0.5

The next context treatment gives all five languages a proper programming skill,
not only Jett. Each repo-scoped skill contains the same type-driven workflow,
compiler-repair loop, evaluation boundary, and language reference shape. Jett's
reference is larger because its syntax and ownership rules are not present in
model pretraining; every row records the exact skill hash and byte count.

The skills contain no benchmark task names, source fixtures, hidden graders,
observed solutions, or task-specific repair advice. For deterministic
evaluation, the harness serializes the complete language skill into the prompt
instead of relying on automatic activation. This produces a 1,350-row study
matrix and a 150-row balanced medium calibration slice.

That slice is recorded in
`benchmarks/results/2026-08-30_codex_luna_medium_v0.5_calibration/`. Initial
grading passed 116/150: zero-shot 33/50, onboarding 45/50, and skill-assisted
38/50. Excluding Jett, onboarding and skill-assisted tied at 37/40. Jett scored
0/10 zero-shot, 8/10 onboarding, and 1/10 skill-assisted.

The Jett skill result exposes a specific documentation gap. Seven of its nine
failed responses used invalid `mutable name: Type` declarations. The reference
describes type-before-name order but, unlike the onboarding sheet, shows no
mutable-local example. The skill must gain compiler-accepted binding and
collection examples and receive a new version before another calibration.

Benchmark v0.5.1 makes that correction. The Jett skill now contrasts
parameter/field syntax with local syntax and includes compiler-checked generic
examples for mutable bindings, enums, optionals/results, collections, views,
cloning, consuming updates, and typed helper extraction. Tasks and language
semantics are unchanged; only future skill-assisted rows use the new version
and hash.

The paired repair stage is recorded in
`benchmarks/results/2026-08-30_codex_luna_medium_v0.5_compile_repair/`. It
repaired 15/34 failures and raised the final result to 131/150. Jett repaired
5/21 and ended at 14/30; the established languages repaired 10/13 and ended at
117/120. These runs use one observation per cell and the rolling Luna alias, so
they remain calibration evidence rather than a language ranking.

## Jett skill smoke calibration v0.5.1

A targeted ten-task Jett `skill_assisted` smoke run is recorded in
`benchmarks/results/2026-08-30_codex_luna_medium_v0.5.1_jett_skill_smoke/`.
Initial grading passed 8/10, compared with 1/10 for the prior skill treatment.
The former mutable-local syntax pattern disappeared. The two failures wrapped
ordinary function or constructor arguments across newlines, which Jett does
not accept.

The paired repair pass is recorded in
`benchmarks/results/2026-08-30_codex_luna_medium_v0.5.1_jett_skill_smoke_compile_repair/`.
Neither failure repaired successfully. One retained multiline call arguments;
the other corrected that form but guessed `int64.to_string` instead of the
implemented `string.from_int64` spelling.

Benchmark v0.5.2 adds those two general syntax anchors to the skill. No task,
grader, adapter, or language rule changed. A future smoke run must use the new
version and skill hash rather than appending to the v0.5.1 evidence.

## Jett skill smoke calibration v0.5.2

The targeted v0.5.2 Jett `skill_assisted` smoke passed 6/10 initial tasks.
Both tasks that failed under v0.5.1 now passed, confirming that multiline-call
and integer-rendering guidance reached the model. The four new failures were
compile errors: one guessed nonexistent `map.contains`, while three exceeded
the complexity limit through boolean chains or nested exhaustive matches.

All four submissions passed after one compiler-feedback repair prompt, making
the paired final result 10/10. Benchmark v0.5.3 therefore adds the exact
`map.has` membership spelling and explains that nested matches and every
`and`/`or` condition add complexity decision points. It remains general
language guidance and contains no task-specific solution material.

## Jett skill 2x2 calibration v0.5.3

The controlled v0.5.3 run measured four cells over the same ten tasks:
zero-shot one-shot, zero-shot with one compile-repair prompt, skill-assisted
one-shot, and skill-assisted with one compile-repair prompt. Zero-shot passed
0/10 initially and repaired 1/10. Skill-assisted passed 9/10 initially; its
single repair failed, leaving 9/10 final.

This isolates the skill as the dominant treatment in this sample. A compiler
diagnostic alone did little to recover programs written without Jett syntax
knowledge. The skill-assisted failure initially exceeded the complexity limit;
its repair split helpers successfully but used reserved built-in type word
`result` as a parameter. Benchmark v0.5.4 adds that general lexical constraint
without including task-specific source or advice.
