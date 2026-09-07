# GPT-5.6 Luna Jett skill smoke repair v0.5.1

The two initial smoke failures each received one second prompt containing the
public task, submitted source, and safe compiler diagnostic. Passing initial
rows stopped after prompt one.

| Task | Repair result | Tokens (input / output / reasoning) | Code bytes |
| --- | ---: | ---: | ---: |
| merge_sorted_intervals | compile error | 14,724 / 739 / 516 | 1,052 |
| score_lines | compile error | 16,415 / 543 / 128 | 1,844 |
| Total | 0/2 | 31,139 / 1,282 / 644 | 2,896 |

`merge_sorted_intervals` repeated the multiline call form despite the parser
diagnostic. `score_lines` placed calls on one line but then used the unsupported
`int64.to_string`; the implemented rendering function is
`string.from_int64`. These findings produced the general v0.5.2 skill anchors.

The run used two fresh ChatGPT-authenticated Codex subscription sessions, no
API key, the rolling Luna alias at medium reasoning, and the same no-network
`jett-bench:0.5.1` grading image as the parent smoke run. Total model latency
was about 29 seconds. `summary.json` preserves the initial 8/10 and final 8/10
paired outcomes separately.
