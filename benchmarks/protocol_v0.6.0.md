# Benchmark protocol v0.6.0

This development calibration expands the existing ten problems to 100:
30 additional numerical/sequence problems, 30 text/parsing problems, and
30 structured-data, state, graph, and maintenance problems. Every problem has
equivalent Jett, Python, TypeScript, Go, and Rust interfaces and graders.
The original three arithmetic tasks receive adapter version 1.0.1: strict
Pyright checking and a separate Go compile phase, with unchanged semantics.
The 90 additions include language-neutral `cases.json` fixtures with at least
ten distinct inputs each. Evaluator-only generators retain independent oracles
or manually specified expected answers, not just cross-language agreement.

## Fixed campaign and four cells

The campaign uses GPT-5.6 Luna, medium reasoning, one repetition, and only
Codex ChatGPT-subscription authentication. No paid API requests are authorized.
There are 1,000 initial prompts: 100 problems x five languages x two contexts.

| Context | One-shot cell | Compile-and-repair cell |
| --- | --- | --- |
| No skill | Initial response | Same response, plus one repair if it failed |
| Programming skill | Initial response with skill | Same response, plus one repair if it failed |

Passing initial responses stop. Each failing initial response receives exactly
one second prompt with its original public task, original context, submitted
source, and public compiler feedback (or a normalized category when private
grader locations cannot be excluded). Hidden tests and expected values never
enter evaluated prompts. These are paired cells, not four independent draws.

The larger configuration still supports onboarding sheets, three reasoning
levels, and multiple repetitions; its full 13,500-row plan is not this campaign.
The initial campaign retains the v0.5.4 Jett skill content and all four other
language skills. Common type-driven guidance is identical across skills;
skill hashes and byte counts disclose differing context sizes.

## Identity, validation, and execution

Before preparation, all 500 trusted baselines must pass. Freeze complete
prompts, task/skill/compiler/tooling inputs, configuration, and grading image
ID. Journal each completed generation and grade, resume only missing rows,
and stop dispatch on infrastructure errors. Preserve separate raw events for
every attempted backend call, including timeouts and rejected tool use.

Each evaluated prompt runs in a fresh, empty, ephemeral Codex session with
user configuration and rules ignored; observed tool use invalidates the call.
The model remains a rolling alias, not a pinned dated snapshot. The configured
API `max_output_tokens` setting is not enforced by Codex CLI; the subscription
treatment uses the same CLI defaults and 300-second call timeout in every
language. Record actual tokens and latency; do not claim identical token caps.

Execute every generated candidate in its own disposable no-network container
with bounded CPU, memory, process count, and time. Jett first builds the
candidate without hidden `verify` blocks, then appends and tests them. A
comptime hidden-assertion failure is a test failure, not a source compile
failure. Compiler errors against the required interface remain compile errors.
This corrects a grader classification issue; it changes no language rule and
does not retroactively rewrite earlier results.

## Reporting and skill follow-up

Publish four cells per language with pass counts, input/cached/output/reasoning
tokens, summed model-call latency, code characters, and UTF-8 bytes. Repair
totals include the initial call. Missing metrics are unavailable, not zero;
cached input is part of input and reasoning is part of output. Parallel elapsed
time is distinct from summed call latency. Code totals include failed candidates.

After the complete first campaign, inspect Jett failures and revise only general
implemented-language guidance. Do not add task algorithms, solutions, fixtures,
or failure-specific recipes to the skill. Freeze a new version and repeat all
100 Jett skill-assisted initials and their paired repairs. Preserve both skill
revisions and report regressions or no improvement as well as gains.

This is a development suite, including previously inspected tasks, not an
untouched holdout. One observation per task/context and a mutable model alias
do not support broad causal language rankings. Later independent tasks and
repetitions are still needed. Results remain incomplete until all required
rows, repair pairings, and the skill follow-up have been verified.
