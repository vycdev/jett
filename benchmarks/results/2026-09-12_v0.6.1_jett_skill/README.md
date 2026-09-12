# Full-100-task Jett skill follow-up v0.6.1

The revised general Jett skill passed **71/100 initially and 87/100 after one
failed-only repair**, compared with **33/100 and 66/100** for the original
v0.5.4 guide. All 100 tasks were rerun; no favorable subset was selected.
The follow-up retained 100 initial responses and 29 repairs, of which 16 passed.
There were no interrupted calls, replacement attempts, or unaccounted responses.

This is a separately versioned development experiment using
`gpt-5.6-luna`, medium reasoning, through ChatGPT-authenticated Codex CLI
`0.153.4`. No API-billed requests or usage resets were used. The
[original five-language four-cell campaign](../2026-09-12_v0.6.0_four_cell/README.md)
remains unchanged and is not pooled with this follow-up for performance scores.

## Outcomes and regressions

Every denominator is 100. A repair-budget result includes the original answer;
an initially passing answer receives no repair.

| Budget | Old guide | Revised guide | Newly passing tasks | Regressions | Net gain |
| --- | ---: | ---: | ---: | ---: | ---: |
| One-shot | 33/100 | 71/100 | 43 | 5 | +38 points |
| After repair | 66/100 | 87/100 | 26 | 5 | +21 points |

The one-shot comparison has 28 unchanged passes and 24 unchanged failures.
After repair, there are 61 unchanged passes and eight unchanged failures.
[`comparison.json`](comparison.json) preserves every paired task outcome,
run/response identity, all four category lists, and metric differences.

One-shot regressions: `num_knapsack_value`, `num_prefix_balances`,
`order_lifecycle`, `score_lines`, and `triangle_kind`.

After-repair regressions: `data_patient_triage`, `data_sensor_sessions`,
`data_tree_subtree_weight`, `num_knapsack_value`, and `recursive_expression`.
All failures and regressions are retained; none was selectively resampled.

## Usage and generated code

| Metric | Old one-shot | Revised one-shot | Old after repair | Revised after repair |
| --- | ---: | ---: | ---: | ---: |
| Saved responses | 100 | 100 | 167 | 129 |
| Input tokens | 1,476,343 | 1,534,606 | 2,493,187 | 1,995,017 |
| Cached input, included above | 1,187,840 | 1,132,544 | 1,987,840 | 1,469,184 |
| Output tokens | 96,501 | 83,045 | 145,918 | 109,518 |
| Reasoning tokens, included above | 71,461 | 56,638 | 99,554 | 70,592 |
| Summed call latency, seconds | 2,102.7 | 1,933.1 | 3,243.3 | 2,533.3 |
| Source characters | 103,174 | 111,093 | 190,698 | 165,362 |
| Source UTF-8 bytes | 103,174 | 111,093 | 190,698 | 165,362 |

The revised one-shot answers used 58,263 more input tokens and 7,919 more source
characters, but 13,456 fewer output tokens. With repairs included, the revised
run needed 38 fewer responses, 498,170 fewer input tokens, 36,400 fewer output
tokens, and 25,336 fewer source characters. Fewer repairs contribute to those
cumulative reductions; they do not mean initial programs became shorter.

[`REPORT.md`](REPORT.md) and `summary.json` preserve the frozen follow-up
reporter's exact output. [`accounting.json`](accounting.json) independently
checks every retained response and reports complete follow-up consumption.
Do not add one-shot and repair-budget columns: they overlap. Code sizes include
failed candidates, and summed call latency is not parallel wall-clock duration.
Benchmark authoring, reference validation, and orchestration usage are excluded.

Both Jett cohorts have complete recorded usage. The three interrupted original
calls belonged to Rust/Python, not Jett. Across the whole two-campaign series,
[`series-accounting.json`](series-accounting.json) counts 1,363 distinct saved
responses exactly once: 18,561,421 input tokens, including 15,816,192 cached;
749,901 output tokens, including 448,057 reasoning; 18,545.665 seconds of summed
call latency; and 1,159,306 generated source characters/UTF-8 bytes. The three
lost original attempts still have unknown overhead. Complete series consumption
remains unavailable, not equal to these known saved-response totals.

## What changed

Only `.agents/skills/jett-programming/SKILL.md` and its language reference
changed among the 1,470 frozen source inputs. The revision clarifies:

- Borrowing iteration with `for item in view items` and ownership preservation.
- Canonical string interpolation and implemented string API signatures.
- Local equality-based nonzero proofs and `ok`/`fail` result construction.
- Per-function complexity budgeting before branch-heavy bodies are written.

The skill bundle grew from 8,723 to 11,245 UTF-8 bytes. These are general,
documented language rules, not algorithms, task-specific recipes, hidden cases,
or expected answers. All five fenced syntax anchors compiled without diagnostics
and passed format checks. Four neutral verification blocks checked the relevant
ownership, arithmetic, and string behavior. Their source is in the evidence
archive's `skill-checks/` directory.

Independent provenance checks verified all 100 task-prompt prefixes, system
instructions, adapters, starters, and model settings against the original Jett
skill treatment. Only the hashed skill suffix changes. Configuration values
differ only in benchmark version/subset identity. A fresh local baseline gate
passed all 500 task/language pairs with unchanged toolchain versions; old results
were not copied to satisfy the new gate.

The grading image derives from the exact original image with two skill-file
COPY layers. All 12 original layers, runtime configuration, and the compiler
binary are preserved. Its complete 1,470-file snapshot matches the follow-up
manifest. Candidates were each graded in a separate no-network container with
2 CPUs, 2 GiB memory, 256 PIDs, dropped capabilities, and no new privileges.

Source revision: `9e3690e14ce70c454eb1bd2d1ce5386bb30c15e9`.
Follow-up grading image:
`sha256:ee84de2c6178dabe8532b57402d6d9c271991d71caf3cb40e9c2bbad03c1e4ae`.
The original image is preserved as
`sha256:5cd341ed3f4062c4c56ab6d267111041aa375c2e39ae20db1770d2b46ee02e80`.

## Evidence and reproduction

`campaign-evidence.zip` has 298 byte-verified entries: 268 canonical campaign
files and 30 supporting files. It includes all initial/repair responses, grades,
event traces and attempt receipts; frozen plans/configuration; the exact report;
29 reconstructed repair plans; fresh baseline evidence; operator launch logs,
image verification and the derived Dockerfile; neutral skill checks; protocol
files; and the host-side audit/orchestration helpers.

`frozen-inputs.zip` preserves all 1,470 exact source inputs, including the ignored
Cargo lockfile and revised skill. `archive-manifest.json` records every evidence
entry hash and the hashes of both archives and report/accounting artifacts.
Git attributes preserve artifact bytes, including intentional CRLF line endings.

- Campaign evidence SHA-256:
  `b1c4bb7e1ee81e846ec8316f1ed9c9eed6e8bb1bb422428ec4f9474aff0307ff`.
- Frozen inputs SHA-256:
  `3d08e1a5a62f6efe4f0d10f07e761f38c825378dbc5010f213112cf598fc0b56`.

The final read-only audit passed complete coverage, failed-only pairing, terminal
grades, image identity, event-derived usage, unique response IDs, zero observed
tool calls, exact attempt coverage, and all frozen hashes. Independent metric
and paired-outcome recomputation matched the original and revised cells.
A separate review of all 29 repair plans found 22 candidate compiler diagnostics
and seven exact normalized messages. All 52 diagnostic locations/excerpts matched
submitted source; no hidden source or fixture leakage was observed. This is a
bounded audit, not a universal guarantee about the heuristic sanitizer.

For a no-model-call evidence audit, use an isolated checkout: materialize the
source archive's `inputs/` tree, copy `operator-tools/` from the evidence archive
into `tools/`, and extract its `campaign/` directory under `target/jett-bench/`.
Run `python tools/bench_campaign_audit.py PATH_TO_EXTRACTED_CAMPAIGN` and save JSON
outside the audited directory. Never execute generated candidates on the host.
The large container images are retained locally, not uploaded in these archives;
their SHA-256 identifiers are local image IDs, not registry download URLs.
Exact sandbox replay needs those images; byte-level evidence auditing does not.

## Interpretation

The observed improvement is substantial on this development suite, but this is
one observation per task/treatment on a rolling model alias. The revision was
informed by public diagnostics from these tasks; it is not held-out validation.
Guidance content and length changed together, and sampling, task order, caching,
and timing can also affect results. The five regressions at each budget remain
part of the comparison. No compiler, task, grader, or other language skill was
changed to improve the score.

The original `text_percent_decode` unused-variant wording caveat remains as
disclosed in the original report. Both versions preserve its frozen contract
and observed score. Further skill changes or clearer future task editions would
require new versioned experiments, not edits to these published results.
