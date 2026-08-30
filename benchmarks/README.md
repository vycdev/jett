# Jett LLM benchmark

This directory contains the versioned pilot described in
`docs/active/llm_language_benchmark.md`. It is usable offline: validation,
planning, baseline grading, request generation, and result aggregation do not
need an API key.

The configured model is GPT-5.6 Luna, which supports low, medium, and high
reasoning. The benchmark supports the Responses API and Codex subscription as
separate backends. Model behavior and pricing can change, so API publication
runs should pin a dated snapshot when one exists and update the price file from the
[official model page](https://developers.openai.com/api/docs/models/gpt-5.6-luna).
Request fields follow the official
[Responses API reference](https://developers.openai.com/api/reference/resources/responses/methods/create).

## Commands

```text
cargo build -q -p jett_cli
python tools/jett_bench.py validate
python tools/jett_bench.py plan --output target/jett-bench/plan.jsonl
python tools/jett_bench.py baselines --allow-unsafe-local
python tools/jett_bench.py requests --output target/jett-bench/requests.jsonl
python tools/jett_bench.py codex-calibration --confirm-subscription-usage
python tools/jett_bench.py codex-calibration --language jett \
  --track skill_assisted --limit 10 --confirm-subscription-usage
python tools/jett_bench.py codex-repair graded-initial.jsonl \
  --output target/jett-bench/codex-repair/raw.jsonl \
  --confirm-subscription-usage
python tools/jett_bench.py aggregate results.jsonl --output summary.json
python -m unittest tools.tests.test_jett_bench
```

`requests` emits auditable request bodies and run metadata; it does not contact
OpenAI. `api-run` exists for an authorized pilot and requires both
`--confirm-spend` and `OPENAI_API_KEY`. Start with a small `--limit`, pin a model
snapshot in the experiment file, and run generated code only inside an isolated
environment. `--allow-unsafe-local` is intentionally alarming: local process
timeouts do not provide a security boundary.

`codex-calibration` uses the ChatGPT login held by the Codex CLI, removes API
credential variables from the child environment, and refuses to run unless
`codex login status` reports `Logged in using ChatGPT`. The current v0.5.1
calibration is a 150-cell medium-reasoning slice run in fresh ephemeral sessions
from empty temporary directories. The Luna name is currently a rolling alias
rather than a dated snapshot, so every row records the alias, UTC completion time, Codex version,
backend, deterministic sequence, and event-log hash. Do not pool these rows with
Responses API rows: the Codex agent wrapper is part of this treatment.
Optional `--language` and `--track` filters select a balanced diagnostic slice
before `--limit` is applied.

Generation does not execute submitted code. Grade the resulting JSONL inside
the no-network container:

```text
python tools/jett_bench.py grade-results /results/raw.jsonl \
  --output /results/graded.jsonl --allow-unsafe-local
```

The optional `codex-repair` treatment is paired with those graded initial
rows. A passing one-shot stops. A failing row from any context track receives
exactly one second prompt containing the original public task, its first source
file, and compiler feedback. Candidate-source diagnostics are included when
they can be separated from private grader code; otherwise the prompt receives
only a normalized failure category. Grade the repair JSONL in the same
no-network container, then aggregate the initial and repair files together.
The summary preserves one-shot scores and separately reports repair success and
pass-after-repair rates.

The isolated runner recipe and required no-network/resource controls are in
`sandbox/README.md`.

## Layout

- `config/pilot.json`: experiment matrix and mutable price assumption;
- `tasks/*/task.json`: public semantics, signatures, and language adapters;
- `tasks/*/baseline.*`: repository-owned known-good solutions;
- `tasks/*/hidden.*`: graders, excluded from prompts;
- `references/*.md`: controlled-onboarding sheets;
- `schemas/*.json`: machine-readable result and task contracts.

The current skill-assisted extension is specified by `protocol_v0.5.2.md` and
`jett_subset_v0.5.2.md`. It inherits v0.5.1 and corrects the Jett expression
and integer-rendering gaps identified by the targeted smoke run. Its Python
adapter adds pinned Pyright strict checking; all five typed-task adapters perform a
static-check phase before hidden runtime tests.
Task-specific forbidden patterns are a narrow preflight against type erasure
and catch-all branches, and a rejection is recorded as `policy_error`.

The v0.3 task set adds recursive closed data and a maintenance task that asks
the model to evolve supplied source. Starter-source hashes are recorded with
planned and generated rows so the maintenance input is auditable.

The task set contains ten tasks. The four v0.4 additions cover optional values
and sets, struct/list transformation, typed map updates, and canonical string
parsing with structured failures.

The v0.5 matrix adds a third `skill_assisted` context track, for 1,350 planned
rows and a 150-row balanced medium calibration. Five repo-scoped skills live in
`.agents/skills/`. The harness materializes their instruction files directly
for deterministic evaluation, records their hashes and byte counts, and checks
that they contain no task identifiers or complete benchmark source fixtures.

Never paste hidden files into prompts or model repair diagnostics. Results and
raw responses should be written below `target/jett-bench/`, which is already
ignored by the repository's `target/` rule.

The onboarding track gives every language the same type-driven development
instruction plus a language-specific section. `reference_bytes` includes both,
so documentation exposure remains visible in results.
The skill-assisted track applies the same common instruction, then adds that
language's complete skill bundle. `skill_sha256`, `skill_bytes`, and
`reference_bytes` make the larger Jett context and every future skill revision
explicit.
