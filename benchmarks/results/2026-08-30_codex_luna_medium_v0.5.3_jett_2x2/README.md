# GPT-5.6 Luna Jett 2x2 calibration v0.5.3

This controlled Codex-subscription diagnostic crossed two context treatments
with two prompt budgets over the same ten tasks. The repair cells are
conditional: only a failed one-shot receives one compiler-feedback prompt.

## Four-cell outcome

| Context | Prompt budget | Final pass | Tokens (input / output / reasoning) | Generated code bytes |
| --- | --- | ---: | ---: | ---: |
| No skill | one-shot | 0/10 | 127,706 / 9,609 / 7,176 | 11,448 |
| Jett skill v0.5.3 | one-shot | 9/10 | 146,790 / 5,884 / 3,351 | 11,993 |
| No skill | compile + repair | 1/10 | 263,571 / 16,797 / 12,039 | 22,791 |
| Jett skill v0.5.3 | compile + repair | 9/10 | 162,246 / 7,321 / 4,275 | 14,666 |

Two-prompt usage is cumulative across the initial and repair stages. All ten
no-skill rows required repair; only one skill-assisted row did. Reasoning tokens
are included within output tokens.

## Interpretation

In this sample, the skill changed one-shot performance from 0/10 to 9/10. A
compiler diagnostic without Jett onboarding repaired only
`account_state_evolution`, producing 1/10 final. The skill-assisted failure was
`recursive_expression`: its first response exceeded complexity 10, and its
repair extracted helpers but used reserved word `result` as a parameter name.

This is one observation per task under a rolling model alias, so it is strong
diagnostic evidence for this task set, not a stable population estimate or a
language ranking.

## Execution record

- Backend: ChatGPT-authenticated Codex subscription; no API key or API-billed
  request was used.
- Model: rolling `gpt-5.6-luna`; medium reasoning; `codex-cli 0.145.0`.
- Treatment: benchmark `0.5.3-pilot`; zero-shot reference bytes 0; Jett skill
  bytes 8,519, reference bytes 8,795, SHA-256
  `75aba9af03bb132ae01b9ed5c854d8fd7d2954de243bce2ac7628f0ad36cc74c`.
- Isolation: 31 fresh empty sessions, ignored user/project instructions,
  read-only sandbox, and zero observed tool calls.
- Grading: no-network `jett-bench:0.5.3` image with 2 GiB memory, 2 CPUs, 256
  PIDs, all capabilities dropped, and no new privileges.
