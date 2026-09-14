# 100-problem four-cell benchmark

Model: gpt-5.6-luna; reasoning: medium; backend: Codex subscription.

| Language | Context | Budget | Pass | Input tokens | Cached input | Output tokens | Reasoning tokens | Latency (s) | Code chars | Code bytes |
| --- | --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| jett | skill_assisted | one_shot | 71/100 | 1,534,606 | 1,132,544 | 83,045 | 56,638 | 1,933.1 | 111,093 | 111,093 |
| jett | skill_assisted | compile_repair | 87/100 | 1,995,017 | 1,469,184 | 109,518 | 70,592 | 2,533.3 | 165,362 | 165,362 |

Repair budgets include initial and repair usage; passing initial responses receive no second prompt.
Code counts include every generated candidate, including failures. Reasoning tokens are included in output tokens.
Cached input is a subset of input tokens. Latency sums model-call durations, not parallel campaign wall time.

Each task/context has one observation. The model alias can change; these are calibration results.
The task set is used for skill development; repeated evaluation is not an untouched held-out estimate.
No API-billed requests were used; subscription allowance was consumed.
