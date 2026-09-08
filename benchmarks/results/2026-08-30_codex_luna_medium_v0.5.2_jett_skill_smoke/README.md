# GPT-5.6 Luna Jett skill smoke calibration v0.5.2

This targeted Codex-subscription diagnostic ran the ten benchmark tasks once
with Jett, the `skill_assisted` track, and medium reasoning. It is not a
language ranking or a statistically stable estimate.

## Outcome

| Task | Result | Tokens (input / output / reasoning) | Code bytes |
| --- | ---: | ---: | ---: |
| account_state_evolution | pass | 14,960 / 315 / 96 | 1,102 |
| bounded_weighted_sum | pass | 14,514 / 363 / 226 | 477 |
| first_duplicate | pass | 14,424 / 207 / 127 | 304 |
| inventory_batch | pass | 14,530 / 739 / 446 | 1,269 |
| merge_sorted_intervals | pass | 14,748 / 1,172 / 976 | 879 |
| order_lifecycle | compile error | 14,559 / 604 / 135 | 2,868 |
| recursive_expression | compile error | 14,505 / 810 / 433 | 2,362 |
| score_lines | compile error | 14,628 / 1,841 / 1,474 | 1,630 |
| signed_gcd | pass | 14,700 / 248 / 137 | 405 |
| triangle_kind | compile error | 14,400 / 231 / 105 | 360 |
| Total | 6/10 | 145,968 / 6,530 / 4,155 | 11,656 |

Reasoning tokens are included within output tokens. Of the input total, 111,104
tokens were cached.

## Finding

Both v0.5.1 failures passed, so the multiline-call and integer-rendering
anchors reached the model. One new submission guessed unsupported
`map.contains`; the implemented membership operations are `map.has` and
`map.contains_key`. Three compact-looking submissions exceeded complexity 10
because boolean operators and nested exhaustive matches add decision points.

The 6/10 score is below the previous 8/10 smoke, but each cell still has one
observation under a rolling model alias. Treat the difference as sampling
noise and diagnostic evidence, not a regression estimate.

## Execution record

- Backend: ChatGPT-authenticated Codex subscription; no API key or API-billed
  request was used.
- Model: rolling `gpt-5.6-luna`; medium reasoning; `codex-cli 0.145.0`.
- Treatment: benchmark `0.5.2-pilot`, Jett `skill_assisted`, skill SHA-256
  `a3cbc6c516b223d8f1306db7c31773677ed747fa3460eac07cf435aeaf6e9ea5`.
- Isolation: ten fresh empty sessions, ignored user/project instructions,
  read-only sandbox, and zero observed tool calls.
- Grading: no-network `jett-bench:0.5.2` image with 2 GiB memory, 2 CPUs, 256
  PIDs, all capabilities dropped, and no new privileges.
- Usage: 145,968 input tokens, 6,530 output tokens, 4,155 reasoning tokens,
  11,656 code bytes, and about 149 seconds of model latency.
