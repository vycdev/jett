# GPT-5.6 Luna Jett skill smoke calibration v0.5.1

This targeted Codex-subscription diagnostic ran the ten benchmark tasks once
with Jett, the `skill_assisted` track, and medium reasoning. It is not a
language ranking or a statistically stable estimate.

## Outcome

| Task | Result | Tokens (input / output / reasoning) | Code bytes |
| --- | ---: | ---: | ---: |
| account_state_evolution | pass | 14,664 / 370 / 143 | 1,131 |
| bounded_weighted_sum | pass | 14,638 / 555 / 424 | 437 |
| first_duplicate | pass | 14,645 / 323 / 243 | 304 |
| inventory_batch | pass | 14,745 / 679 / 391 | 1,269 |
| merge_sorted_intervals | compile error | 14,357 / 726 / 505 | 1,038 |
| order_lifecycle | pass | 14,469 / 872 / 415 | 2,502 |
| recursive_expression | pass | 14,423 / 724 / 345 | 2,076 |
| score_lines | compile error | 14,445 / 2,127 / 1,702 | 2,029 |
| signed_gcd | pass | 14,402 / 350 / 241 | 303 |
| triangle_kind | pass | 14,617 / 288 / 131 | 463 |
| Total | 8/10 | 145,405 / 7,014 / 4,540 | 11,552 |

Reasoning tokens are included within output tokens. Of the input total, 108,032
tokens were cached.

## Finding

The previous v0.5.0 Jett skill treatment passed 1/10. This v0.5.1 smoke passed
8/10, and none of its submissions used the former invalid
`mutable name: Type` local syntax. Because the model alias is rolling and each
cell has one observation, the seven-point change is diagnostic rather than a
causal estimate, but disappearance of the targeted failure pattern supports
the skill correction.

Both remaining failures used multiline argument lists. Jett treats a newline
as the end of an ordinary call expression, so each produced parser cascades.
This common formatting instinct was not yet stated in the skill.

## Execution record

- Backend: ChatGPT-authenticated Codex subscription; no API key or API-billed
  request was used.
- Model: rolling `gpt-5.6-luna`; medium reasoning; `codex-cli 0.145.0`.
- Treatment: benchmark `0.5.1-pilot`, Jett `skill_assisted`, skill SHA-256
  `5d0e76ccb4b1b784257e76fed5b72b48b5339b25304f39d48f92cec9173e9598`.
- Isolation: ten fresh empty sessions, ignored user/project instructions,
  read-only sandbox, and zero observed tool calls.
- Grading: no-network `jett-bench:0.5.1` image with 2 GiB memory, 2 CPUs, 256
  PIDs, all capabilities dropped, and no new privileges.
- Usage: 145,405 input tokens, 7,014 output tokens, 4,540 reasoning tokens,
  11,552 code bytes, and about 155 seconds of model latency.

The rolling-alias limitation and medium reasoning support were checked against
the [official Luna model page](https://developers.openai.com/api/docs/models/gpt-5.6-luna).
