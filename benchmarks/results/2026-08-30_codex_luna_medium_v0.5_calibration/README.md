# GPT-5.6 Luna programming-skill calibration v0.5

This is an exploratory Codex-subscription calibration, not a publishable
language ranking. It used one independent medium-reasoning response per
task/language/track cell: 10 tasks x 5 languages x 3 tracks = 150 responses.

## Harness result

| Language | Zero-shot | Tokens (input / output / reasoning) | Code bytes (total / mean) | Onboarding | Tokens (input / output / reasoning) | Code bytes (total / mean) | Skill-assisted | Tokens (input / output / reasoning) | Code bytes (total / mean) |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| Jett | 0/10 | 128,092 / 10,392 / 7,986 | 11,002 / 1,100.2 | 8/10 | 135,027 / 6,752 / 4,185 | 12,316 / 1,231.6 | 1/10 | 139,700 / 8,140 / 5,747 | 11,739 / 1,173.9 |
| Python | 6/10 | 127,577 / 3,552 / 1,146 | 10,901 / 1,090.1 | 8/10 | 131,827 / 3,953 / 1,433 | 10,857 / 1,085.7 | 8/10 | 134,279 / 4,030 / 1,856 | 9,615 / 961.5 |
| TypeScript | 9/10 | 127,768 / 3,861 / 1,186 | 10,346 / 1,034.6 | 10/10 | 130,886 / 4,190 / 1,456 | 10,346 / 1,034.6 | 9/10 | 135,142 / 4,710 / 1,959 | 10,757 / 1,075.7 |
| Go | 9/10 | 127,354 / 3,665 / 1,322 | 8,471 / 847.1 | 9/10 | 131,569 / 4,109 / 1,654 | 8,901 / 890.1 | 10/10 | 134,474 / 4,219 / 1,768 | 8,910 / 891.0 |
| Rust | 9/10 | 127,663 / 3,197 / 892 | 10,059 / 1,005.9 | 10/10 | 131,254 / 4,599 / 1,884 | 13,098 / 1,309.8 | 10/10 | 135,407 / 4,544 / 1,939 | 11,410 / 1,141.0 |
| Total | 33/50 | 638,454 / 24,667 / 12,532 | 50,779 / 1,015.6 | 45/50 | 660,563 / 23,603 / 10,612 | 55,518 / 1,110.4 | 38/50 | 679,002 / 25,643 / 13,269 | 52,431 / 1,048.6 |

Reasoning tokens are the reported reasoning portion of output tokens, not an
additional amount. Of the 1,978,019 input tokens, 1,688,064 were cached. Code
bytes are counted from extracted source, including whitespace and newlines.

Automated grading passed 116/150. Rust passed 29/30; Go and TypeScript passed
28/30 each; Python passed 22/30; Jett passed 9/30. Onboarding scored 45/50,
skill-assisted 38/50, and zero-shot 33/50.

## Task difficulty

| Task | Zero-shot | Onboarding | Skill-assisted | Total |
| --- | ---: | ---: | ---: | ---: |
| account_state_evolution | 4/5 | 5/5 | 5/5 | 14/15 |
| bounded_weighted_sum | 4/5 | 5/5 | 4/5 | 13/15 |
| first_duplicate | 3/5 | 5/5 | 4/5 | 12/15 |
| inventory_batch | 3/5 | 4/5 | 3/5 | 10/15 |
| merge_sorted_intervals | 3/5 | 5/5 | 4/5 | 12/15 |
| order_lifecycle | 3/5 | 5/5 | 4/5 | 12/15 |
| recursive_expression | 2/5 | 2/5 | 2/5 | 6/15 |
| score_lines | 3/5 | 4/5 | 4/5 | 11/15 |
| signed_gcd | 4/5 | 5/5 | 4/5 | 13/15 |
| triangle_kind | 4/5 | 5/5 | 4/5 | 13/15 |

`recursive_expression` remained the hardest task. Both TypeScript failures
mis-narrowed the recursive union. Python strict checking caused several
failures, and its zero-shot `score_lines` submission reached hidden tests but
returned the wrong failure line.

## Skill-treatment finding

The parity-matched skills worked normally for established languages:
skill-assisted and onboarding both scored 37/40 when Jett is excluded.
Jett diverged sharply: onboarding scored 8/10, while its programming skill
scored 1/10.

Seven of the nine failed Jett skill responses used the same invalid local
declaration form, `mutable name: Type = value`. The skill reference says to
put `mutable` before the type, but contains no concrete mutable-local example;
the shorter onboarding sheet explicitly demonstrates accepted
`mutable Type name = value` declarations. The other two skill failures exceeded
the compiler's complexity limit. This calibration therefore identifies a
Jett-skill documentation defect; it is not evidence that fuller skills are
generally harmful.

## Execution record

- Backend: Codex subscription, logged in through ChatGPT; no API key was used.
- Model: rolling `gpt-5.6-luna` alias; no dated immutable snapshot was listed
  by the official model page at run time.
- Client: `codex-cli 0.145.0`; medium reasoning; one response per cell.
- Isolation: 150 fresh ephemeral sessions in empty directories; user
  configuration and project rules ignored; read-only sandbox; zero observed
  tool calls.
- Grading: no-network disposable `jett-bench:0.5` container with 2 GiB memory,
  2 CPUs, 256 PIDs, all Linux capabilities removed, and no new privileges.
- Usage: 1,978,019 input tokens, 73,913 output tokens, 36,413 reasoning tokens,
  about 1,805 seconds of model latency, and 158,728 extracted code bytes.
- Cost: no API charge was incurred; the run consumed Codex subscription usage.

The rolling-alias limitation and supported medium reasoning level were checked
against the [official Luna model page](https://developers.openai.com/api/docs/models/gpt-5.6-luna).

`results.jsonl` preserves every prompt and skill hash, response ID, raw answer,
extracted source, token count, event-log hash, grading command, and diagnostic.
`summary.json` contains reproducible per-cell and aggregate rollups. Raw Codex
event logs remain local because their relevant metadata and hashes are captured
in each result row.

## Interpretation limits

- There is one observation per cell, so uncertainty and pass@k estimates are
  not meaningful.
- The rolling model alias can change. Exact timestamps, order, client version,
  response IDs, and hashes improve traceability but do not create snapshot-level
  reproducibility.
- Codex includes an agent wrapper and large system context. Do not pool these
  rows with direct Responses API results.
- The Jett skill should be corrected and versioned before another calibration;
  this immutable run remains the evidence for that change.
