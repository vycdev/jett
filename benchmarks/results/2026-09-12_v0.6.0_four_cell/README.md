# 100-problem Luna four-cell benchmark v0.6.0

This complete development run contains 100 tasks across five languages and
two context treatments: 1,000 retained initial responses, followed by exactly
one repair for each of the 234 initial failures. The model was
`gpt-5.6-luna`, medium reasoning, through ChatGPT-authenticated Codex CLI
`0.153.4`. No API-billed requests were used.

Three original calls were interrupted without saved answers or event traces.
The user explicitly approved repeating those exact prompts once. The retained
replacements are scored once; the original receipts and approval are preserved.
Their missing usage is unknown, not zero, so complete campaign consumption is
unavailable. All figures below describe retained responses only.

## Outcomes

Every denominator is 100. The repair budget includes the original candidate;
an initially passing candidate receives no second prompt.

| Language | No skill, one-shot | No skill, after repair | Skill, one-shot | Skill, after repair |
| --- | ---: | ---: | ---: | ---: |
| Go | 94 | 97 | 95 | 98 |
| Jett | 4 | 5 | 33 | 66 |
| Python | 76 | 96 | 92 | 97 |
| Rust | 90 | 97 | 94 | 95 |
| TypeScript | 94 | 97 | 94 | 97 |

Overall, 766 initial responses passed; 79 of 234 repairs passed, producing
845/1,000 final passes. The Jett skill in this original run is the v0.5.4
guidance. Its separate full-100-task revision experiment is not part of these
scores and must be reported independently.

[`REPORT.md`](REPORT.md) preserves the frozen reporter's full 20-cell table,
including input, cached-input, output, and reasoning tokens; summed model-call
latency; and generated code characters and UTF-8 bytes. `summary.json` is the
unchanged machine-readable report.

## Accounting and the three interruptions

[`accounting.json`](accounting.json) independently audits the complete campaign
and counts each of the 1,234 retained responses exactly once. Known usage is:

| Metric | Retained-response total |
| --- | ---: |
| Input tokens | 16,566,404 |
| Cached input tokens, included in input | 14,347,008 |
| Output tokens | 640,383 |
| Reasoning tokens, included in output | 377,465 |
| Summed call latency | 16,012.357 seconds |
| Generated source characters / UTF-8 bytes | 993,944 / 993,944 |

These are not complete campaign-consumption totals. The three lost original
calls were `score_lines` / Rust / skill, `score_lines` / Python / no skill,
and `text_ipv4_parse` / Python / skill. Their server completion, tool use,
tokens, and latency cannot be recovered. The raw replacements link to the
original receipt hashes and the user's "Yes, repeat and disclose" approval.
`total_campaign_consumption.complete` is therefore `false`, with total metrics
explicitly `null`.

Do not add one-shot and repair-budget cells: that double-counts initial calls.
Legacy aggregate rollups in `summary.json` describe initial responses, not the
entire campaign. Summed call latency is not parallel elapsed time. Code sizes
include every extracted candidate, including failed programs; extraction
failures retain response usage but have no measured source size. Benchmark
authoring, reference validation, and orchestration work are outside these model
response totals.

## Evidence and integrity

`campaign-evidence.zip` contains 2,492 byte-verified entries: the complete
canonical campaign plus the deterministically reconstructed repair plans.
It retains frozen configuration and initial plans, all initial and repair
responses and grades, all event traces and attempt receipts, recovery approval
and interruption records, the image identity, logs, and the original report.
`archive-manifest.json` records every entry hash, the accounting-file hash, and
the archive identity. Per-directory Git attributes preserve artifact bytes
across checkout newline settings.

Archive SHA-256:
`446bf9108a7a112673b57cfb39c75d4607047bb0edb70624cc11000aa7416a04`.

The final read-only audit checked complete task/context coverage, exact
failed-only repair pairs, terminal grades and boolean pass flags, raw-to-grade
identity and usage, strict event metrics and unique response IDs, zero observed
tool calls, exact attempt coverage, and all approved recovery links. All 1,470
frozen input hashes and the pinned grading image matched. Historical comparison
also preserved all 741 canonical files from the interrupted checkpoint, with
its original 365 raw rows and six grades remaining exact journal prefixes.

An independent check recomputed all 20 result cells and their usage and code
totals. A separate fixed-campaign repair review checked all 234 plans: 162
compiler diagnostics, 71 exact normalized messages, and one policy rule stated
in the public prompt. All 2,176 referenced diagnostic locations were within
the prior candidate source; source excerpts matched except one ordinary Rust
type-annotation suggestion. No hidden source or fixture leakage was observed.
The sanitizer remains heuristic, so this is not a universal leakage guarantee.

Source revision: `bc6f96a`.
Grading image:
`sha256:5cd341ed3f4062c4c56ab6d267111041aa375c2e39ae20db1770d2b46ee02e80`.
Each candidate was graded in its own no-network container with 2 CPUs,
2 GiB memory, 256 PIDs, dropped capabilities, and no new privileges.

The companion [`frozen inputs`](../2026-09-12_v0.6.0_frozen_inputs/README.md)
archive preserves all exact source files, including the ignored Cargo lockfile.
[`Reference validation`](../2026-09-12_v0.6.0_reference_validation/README.md)
preserves all 500 local baseline passes and all 500 container-validated cells,
including the original Go timeout and its successful isolated recheck.
The [`interrupted checkpoint`](../2026-09-12_v0.6.0_interrupted_checkpoint/README.md)
remains unchanged historical evidence.

## Reproduction and limits

Use an isolated checkout containing the published host-side audit helpers,
then materialize the companion archive's `inputs/` files into it. Extract
`campaign/` from this archive under that checkout's `target/jett-bench/`.
Run `python tools/bench_campaign_audit.py PATH_TO_EXTRACTED_CAMPAIGN` to audit
the retained evidence without model calls or grading. Save its JSON outside
the extracted campaign directory. The frozen reporter can also regenerate
the summary from those retained rows. Never execute generated candidates on
the host; use the pinned isolated grading environment for a grading recheck.

This is one observation per task/context on a rolling model alias and a
development suite used for skill improvement, not an untouched held-out
population estimate or a stable language ranking. Repeated evaluation can
reflect sampling and time effects as well as guidance changes. The unused
`Empty` variant in `text_percent_decode` is a wording-clarity caveat for a later
task edition; the frozen contract and all observed scores were retained.
