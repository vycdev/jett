# Jett LLM benchmark

This directory contains the versioned pilot described in
`docs/active/llm_language_benchmark.md`. It is usable offline: validation,
planning, baseline grading, request generation, and result aggregation do not
need an API key.

The configured model is GPT-5.6 Luna, which supports low, medium, and high
reasoning through the Responses API. Model behavior and pricing can change, so
publication runs must pin a snapshot and update the price file from the
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
python tools/jett_bench.py aggregate results.jsonl --output summary.json
python -m unittest tools.tests.test_jett_bench
```

`requests` emits auditable request bodies and run metadata; it does not contact
OpenAI. `api-run` exists for an authorized pilot and requires both
`--confirm-spend` and `OPENAI_API_KEY`. Start with a small `--limit`, pin a model
snapshot in the experiment file, and run generated code only inside an isolated
environment. `--allow-unsafe-local` is intentionally alarming: local process
timeouts do not provide a security boundary.

The isolated runner recipe and required no-network/resource controls are in
`sandbox/README.md`.

## Layout

- `config/pilot.json`: experiment matrix and mutable price assumption;
- `tasks/*/task.json`: public semantics, signatures, and language adapters;
- `tasks/*/baseline.*`: repository-owned known-good solutions;
- `tasks/*/hidden.*`: graders, excluded from prompts;
- `references/*.md`: controlled-onboarding sheets;
- `schemas/*.json`: machine-readable result and task contracts.

Never paste hidden files into prompts or model repair diagnostics. Results and
raw responses should be written below `target/jett-bench/`, which is already
ignored by the repository's `target/` rule.
