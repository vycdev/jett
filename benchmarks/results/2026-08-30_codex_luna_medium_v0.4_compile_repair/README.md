# GPT-5.6 Luna paired compile-and-repair calibration v0.4

This is the adaptive second stage of the 100-row v0.4 calibration. The 80
initial passes stopped after prompt one. Each of the 20 compile failures
received one new prompt containing the public task, its first source, and the
candidate compiler diagnostic. The original one-shot scores remain unchanged.

## Outcome

| Language | Initial | Repairs passed / attempted | Final after repair |
| --- | ---: | ---: | ---: |
| Jett | 7/20 | 3/13 | 10/20 |
| Python | 16/20 | 4/4 | 20/20 |
| TypeScript | 18/20 | 2/2 | 20/20 |
| Go | 20/20 | 0/0 | 20/20 |
| Rust | 19/20 | 1/1 | 20/20 |
| Total | 80/100 | 10/20 | 90/100 |

By context track, zero-shot rose from 36/50 to 41/50 and onboarding rose from
44/50 to 49/50. Jett zero-shot improved only from 0/10 to 1/10; Jett onboarding
improved from 7/10 to 9/10.

## Repair usage and code size

| Language | Attempts | Passed | Tokens (input / output / reasoning) | Code bytes (total / mean) |
| --- | ---: | ---: | ---: | ---: |
| Jett | 13 | 3 | 180,710 / 8,152 / 5,034 | 14,937 / 1,149.0 |
| Python | 4 | 4 | 53,274 / 1,440 / 434 | 4,383 / 1,095.8 |
| TypeScript | 2 | 2 | 27,017 / 960 / 195 | 3,255 / 1,627.5 |
| Rust | 1 | 1 | 13,202 / 145 / 0 | 631 / 631.0 |
| Total | 20 | 10 | 274,203 / 10,697 / 5,663 | 23,206 / 1,160.3 |

Reasoning tokens are included within output tokens. Of the repair input total,
219,136 tokens were cached. The 20 revisions contain 653 non-empty source
lines. No API key or API-billed request was used; the run consumed Codex
subscription allowance.

## Findings

All seven established-language failures were repaired. This included the three
Python rows that had been behaviorally correct but rejected by strict Pyright,
both TypeScript exhaustiveness failures, the Rust inference failure, and the
remaining Python duplicate-set annotation failure.

Jett repaired onboarded `recursive_expression`, onboarded
`merge_sorted_intervals`, and zero-shot `account_state_evolution`. Its ten
remaining failures were still compile-time failures. The revised programs
continued to guess unsupported indexing or declaration syntax, mishandled
optional handling or namespace qualification, or reached Jett's function
complexity limit. One compiler diagnostic is therefore enough for established
languages, but usually not enough to bootstrap Jett without its reference
sheet.

## Execution record

- Backend: Codex subscription with the rolling `gpt-5.6-luna` alias; medium
  reasoning; `codex-cli 0.145.0`; 20 fresh ephemeral sessions.
- Feedback: all 20 prompts received candidate-line compiler diagnostics; no
  private grader source or expected values were included.
- Isolation: empty temporary directories, ignored user configuration and
  project rules, read-only sandbox, and zero observed tool calls.
- Grading: no-network `jett-bench:0.4` container with 2 GiB memory, 2 CPUs, 256
  PIDs, all Linux capabilities removed, and no new privileges.
- Usage: 274,203 input tokens, 10,697 output tokens, 5,663 reasoning tokens,
  and about 254 seconds of model latency.

`results.jsonl` contains the 20 graded repair rows. `summary.json` aggregates
those rows together with the immutable parent calibration and reports paired
pass-after-repair rollups.
