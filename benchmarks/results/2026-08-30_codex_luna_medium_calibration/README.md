# GPT-5.6 Luna Codex-subscription calibration

This is an exploratory harness calibration, not a publishable language ranking.
It used one independent medium-reasoning response per task/language/track cell:
3 tasks x 5 languages x 2 tracks = 30 responses.

## Result

| Language | Zero-shot | Onboarding |
| --- | ---: | ---: |
| Jett | 0/3 | 3/3 |
| Python | 3/3 | 3/3 |
| TypeScript | 3/3 | 3/3 |
| Go | 3/3 | 3/3 |
| Rust | 3/3 | 3/3 |
| Total | 12/15 | 15/15 |

Overall, 27/30 submissions passed. All three failures were Jett zero-shot
compile failures. Luna guessed braces, `var`, `%`, missing colons, or nested
namespace braces; the underlying algorithms were recognizable. All three
onboarded Jett submissions compiled and passed their hidden tests.

This shows that the compact Jett onboarding sheet is sufficient for these
simple tasks and that zero-shot model familiarity is currently low. It does
not isolate type-driven guidance: onboarding also supplies basic syntax. The
100% result for both tracks in the four established languages is a ceiling
effect, so harder tasks are needed before comparing type-driven behavior.

## Execution record

- Backend: Codex subscription, logged in through ChatGPT; no API key was used.
- Model: rolling `gpt-5.6-luna` alias; no dated snapshot was available.
- Client: `codex-cli 0.145.0`.
- Reasoning: medium; no repairs.
- Isolation: 30 ephemeral sessions in empty directories; user configuration
  and project rules ignored; read-only sandbox; zero observed tool calls.
- Grading: no-network disposable container with resource limits and all Linux
  capabilities removed.
- Usage: 381,342 input tokens (284,160 cached), 5,647 output tokens, and 2,538
  reasoning tokens. Total model latency was about 207 seconds.
- Cost: no API charge was incurred; the run consumed Codex subscription usage.
- Codex CLI did not expose the API runner's fixed output-token cap; observed
  outputs were small, but this is a protocol difference.

Model availability and subscription inclusion were checked against the
[official Codex model guide](https://learn.chatgpt.com/docs/models) and
[Codex pricing guide](https://learn.chatgpt.com/docs/pricing). The rolling
alias limitation was checked against the
[official Luna model page](https://developers.openai.com/api/docs/models/gpt-5.6-luna).

`results.jsonl` preserves each prompt hash, response ID, raw answer, extracted
source, token usage, event-log hash, grading commands, and diagnostic. The raw
Codex event logs remain local because their relevant metadata and hashes are
already captured. `summary.json` contains per-cell and aggregate rollups.

## Interpretation limits

- There is only one observation per cell, so pass@k and uncertainty estimates
  are not meaningful yet.
- The model alias can change over time; UTC timestamps, deterministic order,
  client version, and response IDs are recorded, but this is weaker than a
  pinned snapshot.
- Codex includes an agent wrapper and large system context. These results must
  not be pooled with direct Responses API results.
- These three tasks are intentionally small and are too easy for meaningful
  comparisons among Python, TypeScript, Go, and Rust.
