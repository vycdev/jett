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
- Tracks: zero-shot and controlled onboarding.
- Model: a pinned GPT-5.6 Luna snapshot when an API run is authorized.
- Reasoning: low, medium, high.
- Context: every initial attempt is an independent Responses API request.
- Repair: optional bounded repairs receive the prior answer and normalized
  diagnostics in a new request; they do not reuse hidden expected values.
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

## Exit criteria for the pilot

- All known-good solutions pass every hidden grader on the recorded toolchains.
- Re-running aggregation produces identical summaries.
- No prompt leaks hidden cases or expected outputs.
- Failures distinguish extraction, compile, test, timeout, and harness errors.
- Jett failures can be traced to model behavior, documentation, compiler
  diagnostics, or an explicit unsupported feature.
- The task set is revised or frozen before the larger run.

Implementation and operator instructions live in `benchmarks/README.md`.
