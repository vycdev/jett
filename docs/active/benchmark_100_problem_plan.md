# 100-problem benchmark expansion

The user goal is to build the benchmark to 100 distinct problems, execute it,
and improve results through a general Jett programming skill. Earlier agreement
requires language parity, subscription-only Luna use, token and code-size
reporting, and four comparable cells: with/without skill, each one-shot and
with one compiler-feedback repair. Passing initial answers stop after one
prompt. The onboarding-sheet track remains available but is outside this run.

## Completion requirements

1. Keep the existing ten tasks and add 90 distinct tasks: 30 numerical and
   sequence tasks, 30 text/parsing tasks, and 30 structured-data/state/graph
   tasks. Each task has a precise public contract and five equivalent adapters.
2. Validate all 500 repository-owned baselines with static checks and hidden
   behavioral tests. New tasks have at least ten deterministic cases each;
   shared language-neutral fixtures make expected results auditable.
3. Freeze the version, all prompts, source/skill/task hashes, and grading image.
   Generate 1,000 initial Luna medium responses: 100 tasks x five languages x
   two context tracks. Use ChatGPT subscription authentication exclusively.
4. Grade each submitted program in a separate no-network container. Give every
   failed initial candidate exactly one repair prompt containing public
   compiler feedback (or normalized feedback when private grader locations
   cannot be separated), then grade repairs with the same image.
5. Publish all four cells per language with pass counts, input/cached/output/
   reasoning tokens, model latency, code characters and UTF-8 bytes. Repair
   cells include cumulative usage. Missing metrics remain unavailable.
6. Inspect Jett failures, improve only general documented language/tooling
   guidance, version the changed skill, and rerun the Jett skill treatment
   over the full 100-task set with paired repair. Report both revisions,
   including regressions or lack of improvement.
7. Preserve raw responses, event hashes, task/skill identity, successful and
   failed results, verification, and interpretation. Commit and push coherent
   changes. Do not call the goal complete before the full run and skill
   follow-up are verified.

## Integrity and interpretation

Task authors may inspect baselines and hidden tests. Evaluated model sessions
receive only their frozen public prompt and optional language skill; use fresh
empty sessions and reject observed tool use. No algorithm, fixture, expected
answer, or task-specific repair recipe may enter any skill. All languages
receive the same type-driven guidance within their skill context.

The ten existing tasks and subsequent inspected failures are development
evidence. Repeated evaluation on this suite is not an untouched held-out
estimate; one observation per cell and a rolling model alias limit claims.
Future independent tasks and repeated samples are needed for broad rankings.

## Current work

- The 100-task catalog is committed. Its 90 additions contain 1,621 distinct
  shared fixtures, with independent oracles or hand-written expected values.
- The frozen local gate passed all 500 task/language baselines. Container
  verification also covers all 500: 499 passed initially, and one Go reference
  compile timeout passed when rechecked in its own 2-CPU container. The
  original timeout and the recheck are both retained. The original reference
  runner initially shared 2 CPUs between two workers, then received 4 CPUs;
  generated submissions always have separate 2-CPU containers.
- The campaign runner freezes inputs, journals completed rows, resumes missing
  work, stops new dispatch on infrastructure failure, and produces complete
  four-cell reports only when all expected rows and repair pairings exist.
- The v0.6.0 four-cell campaign has 1,000 frozen initial prompts. Luna generation
  is running; the first six responses were graded successfully as a pipeline
  check (five passes, one test failure), and are retained in the full campaign.
  These six rows are not a language comparison. Full initial grading, paired
  repair, reporting, and the skill-revision follow-up remain pending.

## Run checkpoint (2026-09-12)

All paths below are relative to the repository and contain generated evidence:

- `target/jett-bench/v0.6.0-frozen-baselines/`: completed local gate and manifest.
- `target/jett-bench/v0.6.0-container-baselines/`: all original container rows,
  `isolated-recheck/recheck.jsonl`, and combined `verification.json`.
- `target/jett-bench/v0.6.0-four-cell/`: authoritative frozen campaign, raw
  responses, event traces, and initial grading journal.
- `target/jett-bench/v0.6.0-four-cell-staging/`: a separate copy of the exact
  frozen plan/config/image, used to grade completed initial responses while
  generation holds the authoritative campaign lock. Refresh its raw-response
  snapshot only between grading batches. Do not send model calls from staging.

After generation stops, verify staging's prompt IDs/hashes and generation
digests against the authoritative raw rows before merging its grading journal.
Already graded rows must agree exactly; never regrade them to select a better
outcome. Complete any missing initial grades before constructing repair prompts.
Retain event traces in the authoritative campaign for the final integrity check.

The frozen source revision is `bc6f96a`. The grading image is
`sha256:5cd341ed3f4062c4c56ab6d267111041aa375c2e39ae20db1770d2b46ee02e80`
(local tag `jett-bench:0.6.0`); all 1,470 frozen input files matched the image.

Use the already installed Codex CLI **0.153.4** through task-local PATH
selection on this machine. The older npm CLI 0.145.0 cannot parse an app-written
experimental configuration field during `login status`; it made no model calls.
The newer CLI confirms ChatGPT login. User configuration and credentials were
not edited or copied, and each evaluated call still ignores user configuration
and rules. Every raw row records the actual CLI version and subscription backend.
