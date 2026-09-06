# GPT-5.6 Luna paired compile-and-repair calibration v0.5

This is the adaptive second stage of the 150-row v0.5 calibration. The 116
initial passes stopped after prompt one. Each of the 34 failures received one
new prompt containing the public task, its first source, and safe candidate
feedback. The original one-shot scores remain unchanged.

## Outcome

| Language | Initial | Repairs passed / attempted | Final after repair |
| --- | ---: | ---: | ---: |
| Jett | 9/30 | 5/21 | 14/30 |
| Python | 22/30 | 7/8 | 29/30 |
| TypeScript | 28/30 | 0/2 | 28/30 |
| Go | 28/30 | 2/2 | 30/30 |
| Rust | 29/30 | 1/1 | 30/30 |
| Total | 116/150 | 15/34 | 131/150 |

By context track, zero-shot rose from 33/50 to 39/50, onboarding from 45/50
to 49/50, and skill-assisted from 38/50 to 43/50. Jett ended at 1/10
zero-shot, 9/10 onboarding, and 4/10 skill-assisted.

## Repair usage and code size

| Language | Attempts | Passed | Tokens (input / output / reasoning) | Code bytes (total / mean) |
| --- | ---: | ---: | ---: | ---: |
| Jett | 21 | 5 | 302,359 / 12,273 / 6,651 | 26,751 / 1,273.9 |
| Python | 8 | 7 | 108,015 / 4,120 / 1,677 | 11,323 / 1,415.4 |
| TypeScript | 2 | 0 | 27,763 / 1,106 / 221 | 4,162 / 2,081.0 |
| Go | 2 | 2 | 27,146 / 1,647 / 720 | 3,632 / 1,816.0 |
| Rust | 1 | 1 | 13,200 / 164 / 17 | 631 / 631.0 |
| Total | 34 | 15 | 478,483 / 19,310 / 9,286 | 46,499 / 1,367.6 |

Reasoning tokens are included within output tokens. Of the repair input total,
381,440 tokens were cached. No API key or API-billed request was used; the run
consumed Codex subscription allowance.

## Findings

The established languages repaired 10 of 13 failures. Go and Rust reached
30/30, Python reached 29/30, and both TypeScript `recursive_expression`
failures remained rejected by strict narrowing checks.

Jett repaired 5 of 21 failures. Its skill-assisted track improved from 1/10 to
4/10, but six repaired submissions still failed compilation. Five retained or
introduced invalid declaration syntax, while the remaining `score_lines`
submission mishandled optional unwrapping. One diagnostic is therefore not
enough to compensate for the missing concrete syntax examples in the v0.5
Jett skill.

## Execution record

- Backend: Codex subscription with the rolling `gpt-5.6-luna` alias; medium
  reasoning; `codex-cli 0.145.0`; 34 fresh ephemeral sessions.
- Feedback: 31 compiler diagnostics, one normalized compile category, one
  public-policy diagnostic, and one private-test summary; no hidden source or
  expected values were included.
- Isolation: empty temporary directories, ignored user configuration and
  project rules, read-only sandbox, and zero observed tool calls.
- Grading: no-network `jett-bench:0.5` container with 2 GiB memory, 2 CPUs, 256
  PIDs, all Linux capabilities removed, and no new privileges.
- Usage: 478,483 input tokens, 19,310 output tokens, 9,286 reasoning tokens,
  and about 464 seconds of model latency.

`results.jsonl` contains the 34 graded repair rows. `summary.json` aggregates
those rows with the immutable parent calibration and reports paired
pass-after-repair rollups.
