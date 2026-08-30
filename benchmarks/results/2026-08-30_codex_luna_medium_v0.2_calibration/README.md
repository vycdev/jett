# GPT-5.6 Luna typed-domain calibration v0.2

This is an exploratory Codex-subscription calibration, not a publishable
language ranking. It used one independent medium-reasoning response per
task/language/track cell: 4 tasks x 5 languages x 2 tracks = 40 responses.

## Harness result

| Language | Zero-shot | Zero-shot tokens (input / output / reasoning) | Onboarding | Onboarding tokens (input / output / reasoning) |
| --- | ---: | ---: | ---: | ---: |
| Jett | 0/4 | 50,400 / 2,268 / 1,607 | 4/4 | 52,479 / 1,564 / 688 |
| Python | 4/4 | 50,643 / 1,033 / 220 | 4/4 | 51,622 / 1,103 / 217 |
| TypeScript | 4/4 | 50,962 / 1,222 / 196 | 4/4 | 51,570 / 1,342 / 304 |
| Go | 4/4 | 50,343 / 860 / 184 | 4/4 | 51,955 / 1,344 / 560 |
| Rust | 4/4 | 50,916 / 1,064 / 221 | 4/4 | 51,767 / 1,268 / 378 |
| Total | 16/20 | 253,264 / 6,447 / 2,428 | 20/20 | 259,393 / 6,621 / 2,147 |

Reasoning tokens are the reported reasoning portion of output tokens, not an
additional amount to add to output. Of the input totals, 206,848 zero-shot and
200,704 onboarding tokens were cached.

The automated graders reported 36/40 passes. All four compile failures were
zero-shot Jett submissions. Luna again guessed braces, `var`, `%`, or missing
colons for the simple tasks. For `order_lifecycle`, it copied the requested
Jett declarations accurately but guessed arrow-style match arms and unqualified
variant construction. Every onboarded Jett submission compiled and passed.

The new typed lifecycle task received 9/10 automated passes. Its onboarded Jett
solution used small exhaustive helpers that respected Jett's complexity limit.
Python, TypeScript, Go, and Rust all passed both tracks, so the task still has a
ceiling effect for established languages at this sample size.

## Manual compliance audit

One automated pass is a false positive. The zero-shot Go lifecycle submission
ends with `panic("unreachable")`, violating the public no-panic constraint.
Hidden tests cover all declared state/event pairs but did not exercise invalid
underlying Go integer values, and the source-policy preflight did not reject
`panic`. Therefore:

- automated correctness is 36/40;
- audited public-contract compliance is 35/40;
- automated `order_lifecycle` correctness is 9/10, but audited compliance is
  8/10.

This is a grader finding, not evidence against Go or the model. Task version
1.0.0 must not be used for a larger study. Its replacement must reject
panic/throw shortcuts in every adapter before new calibration data is pooled.

## Execution record

- Backend: Codex subscription, logged in through ChatGPT; no API key was used.
- Model: rolling `gpt-5.6-luna` alias; the official model page listed no dated
  immutable snapshot at run time.
- Client: `codex-cli 0.145.0`.
- Reasoning: medium; one response per cell; no repairs.
- Isolation: 40 ephemeral sessions in empty directories; user configuration
  and project rules ignored; read-only sandbox; zero observed tool calls.
- Grading: no-network disposable `jett-bench:0.2` container with 2 GiB memory,
  2 CPUs, 256 PIDs, all Linux capabilities removed, and no new privileges.
- Usage: 512,657 input tokens (407,552 cached), 13,068 output tokens, and 4,575
  reasoning tokens. Total model latency was about 359 seconds.
- Cost: no API charge was incurred; the run consumed Codex subscription usage.
- Codex CLI did not expose the API runner's fixed output-token cap; observed
  outputs remained bounded, but this is a protocol difference.

The rolling-alias limitation and supported medium reasoning level were checked
against the [official Luna model page](https://developers.openai.com/api/docs/models/gpt-5.6-luna).

`results.jsonl` preserves every prompt hash, response ID, raw answer, extracted
source, token count, event-log hash, grading command, and diagnostic.
`summary.json` contains reproducible per-cell and aggregate rollups. Raw Codex
event logs remain local because their relevant metadata and hashes are already
captured in each result row.

## Interpretation limits

- There is only one observation per cell, so uncertainty and pass@k estimates
  are not meaningful.
- The rolling model alias can change. UTC times, deterministic order, client
  version, response IDs, and event-log hashes improve traceability but do not
  provide snapshot-level reproducibility.
- Codex includes an agent wrapper and large system context. Do not pool these
  rows with direct Responses API results.
- The established-language cells remain at or near a ceiling. The next task
  should require a deeper typed invariant and first close the lifecycle grader
  loophole.
