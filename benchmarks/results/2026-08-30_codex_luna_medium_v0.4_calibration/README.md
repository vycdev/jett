# GPT-5.6 Luna ten-task calibration v0.4

This is an exploratory Codex-subscription calibration, not a publishable
language ranking. It used one independent medium-reasoning response per
task/language/track cell: 10 tasks x 5 languages x 2 tracks = 100 responses.

## Harness result

| Language | Zero-shot | Zero-shot tokens (input / output / reasoning) | Zero-shot code chars (total / mean) | Onboarding | Onboarding tokens (input / output / reasoning) | Onboarding code chars (total / mean) |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| Jett | 0/10 | 126,688 / 8,887 / 6,882 | 9,472 / 947.2 | 7/10 | 135,516 / 6,899 / 4,312 | 12,145 / 1,214.5 |
| Python | 8/10 | 126,963 / 4,005 / 1,576 | 10,837 / 1,083.7 | 8/10 | 131,617 / 6,304 / 3,682 | 10,867 / 1,086.7 |
| TypeScript | 9/10 | 127,253 / 3,968 / 1,303 | 10,200 / 1,020.0 | 9/10 | 131,090 / 4,427 / 1,579 | 11,064 / 1,106.4 |
| Go | 10/10 | 127,484 / 4,165 / 1,684 | 8,902 / 890.2 | 10/10 | 131,163 / 4,506 / 1,995 | 9,171 / 917.1 |
| Rust | 9/10 | 126,547 / 4,260 / 1,712 | 11,207 / 1,120.7 | 10/10 | 131,286 / 4,140 / 1,588 | 11,706 / 1,170.6 |
| Total | 36/50 | 634,935 / 25,285 / 13,157 | 50,618 / 1,012.4 | 44/50 | 660,672 / 26,276 / 13,156 | 54,953 / 1,099.1 |

Reasoning tokens are the reported reasoning portion of output tokens, not an
additional amount to add to output. Of the input totals, 552,448 zero-shot and
518,912 onboarding tokens were cached. Code characters are counted from the
extracted source, including whitespace and newlines.

Automated grading passed 80/100. Go was the only language with 20/20. Rust
passed 19/20, TypeScript 18/20, Python 16/20, and Jett 7/20. Onboarding raised
the overall pass rate from 36/50 to 44/50. For Jett specifically it raised the
result from 0/10 to 7/10; excluding Jett, onboarding changed 36/40 to 37/40.

## Task difficulty

| Task | Zero-shot | Onboarding | Total |
| --- | ---: | ---: | ---: |
| account_state_evolution | 4/5 | 5/5 | 9/10 |
| bounded_weighted_sum | 4/5 | 5/5 | 9/10 |
| first_duplicate | 3/5 | 5/5 | 8/10 |
| inventory_batch | 3/5 | 4/5 | 7/10 |
| merge_sorted_intervals | 3/5 | 4/5 | 7/10 |
| order_lifecycle | 4/5 | 5/5 | 9/10 |
| recursive_expression | 3/5 | 2/5 | 5/10 |
| score_lines | 4/5 | 4/5 | 8/10 |
| signed_gcd | 4/5 | 5/5 | 9/10 |
| triangle_kind | 4/5 | 5/5 | 9/10 |

The four v0.4 additions reduced the previous ceiling. `recursive_expression`
was hardest: both TypeScript submissions produced semantically exhaustive
nested switches whose fallthrough was not accepted by the compiler, both Jett
submissions failed syntax checking, and the onboarded Python submission hit a
strict-checker diagnostic.

## Audit findings

No automated pass violated the public no-panic, no-throw, no-catch-all, or
type-escape policies during source review. All 20 automated failures were
compile/static-check failures; there were no hidden-test failures.

Three Python submissions were behaviorally correct but Pyright strict mode
rejected an explicit final `isinstance` as unnecessary: both `inventory_batch`
tracks and onboarded `recursive_expression`. Their hidden tests pass when run
without that diagnostic. The official static-check score remains 80/100, while
a behavior-only reading would be 83/100. This is checker friction to address
before a larger study, not evidence of an algorithmic failure.

Jett's three onboarding failures expose reference-sheet gaps: two used bracket
indexing instead of `list.get`, and one used reserved word `result` as a
parameter. All ten zero-shot Jett submissions failed compilation, mostly by
guessing braces, `let`, bracket indexing, unqualified variants, or missing
colons. This reinforces that Jett currently requires onboarding for agents.

## Execution record

- Backend: Codex subscription, logged in through ChatGPT; no API key was used.
- Model: rolling `gpt-5.6-luna` alias; the official model page listed no dated
  immutable snapshot at run time.
- Client: `codex-cli 0.145.0`.
- Reasoning: medium; one response per cell; no repairs.
- Isolation: 100 fresh ephemeral sessions in empty directories; user
  configuration and project rules ignored; read-only sandbox; zero observed
  tool calls.
- Grading: no-network disposable `jett-bench:0.4` container with 2 GiB memory,
  2 CPUs, 256 PIDs, all Linux capabilities removed, and no new privileges.
- Usage: 1,295,607 input tokens (1,071,360 cached), 51,561 output tokens, and
  26,313 reasoning tokens. Total model latency was about 1,223 seconds.
- Output: 105,571 extracted code characters across 100 submissions.
- Cost: no API charge was incurred; the run consumed Codex subscription usage.

The rolling-alias limitation and supported medium reasoning level were checked
against the [official Luna model page](https://developers.openai.com/api/docs/models/gpt-5.6-luna).

`results.jsonl` preserves every prompt hash, response ID, raw answer, extracted
source, token count, event-log hash, grading command, and diagnostic.
`summary.json` contains reproducible per-cell and aggregate rollups. Raw Codex
event logs remain local because their relevant metadata and hashes are already
captured in each result row.

## Interpretation limits

- There is one observation per cell, so uncertainty and pass@k estimates are
  not meaningful.
- The rolling model alias can change. Timestamps, deterministic order, client
  version, response IDs, and event-log hashes improve traceability but do not
  provide snapshot-level reproducibility.
- Codex includes an agent wrapper and large system context. Do not pool these
  rows with direct Responses API results.
- Static-check success intentionally counts toward the score; the separate
  behavior-only note does not replace the official result.
