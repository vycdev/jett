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
- The v0.6.0 four-cell campaign has 1,000 frozen initial prompts. Generation
  initially stopped with 365 saved responses and three interrupted calls
  without saved answers; all 365 saved responses are graded. The first six pipeline-check
  responses (five passes, one test failure) remain in the campaign unchanged.
  The user approved repeating only those three prompts with explicit disclosure;
  all three replacements are now saved and graded, and initial generation has
  resumed. The original interrupted receipts and all saved responses remain
  unchanged. Remaining generation, paired repair, reporting, and the skill-revision
  follow-up remain unfinished; no partial sample is a final language ranking.

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

Use the host-side staging operator for checked synchronization (substitute the
two campaign directories for `SOURCE` and `STAGING`):

```text
python tools/bench_campaign_staging.py snapshot SOURCE STAGING --stage initial
python tools/jett_bench_campaign.py grade STAGING --image jett-bench:0.6.0 --jobs 3
python tools/bench_campaign_staging.py merge SOURCE STAGING --stage initial
```

Snapshot only after the previous staging batch finishes. It requires all prior
staging responses to be graded and unchanged, and reads the source journal once
without taking its generator-owned lock. A partial last JSONL row is rejected;
retry the snapshot later without changing or reissuing any model call.

Merge requires generation for the entire selected stage to be complete and
both campaigns to be unlocked. It checks exact frozen metadata, prompt IDs,
source/event hashes, the pinned image, and identical overlapping assessments.
It appends only missing validated grades while preserving existing canonical
grade bytes. Then the unchanged campaign grader can fill any remaining grades.
For repairs, use `--stage repair` after both directories have identical complete
initial grading. This operator never generates responses or executes candidates;
the frozen generation/grading/reporting helpers and image remain unchanged.

The frozen source revision is `bc6f96a`. The grading image is
`sha256:5cd341ed3f4062c4c56ab6d267111041aa375c2e39ae20db1770d2b46ee02e80`
(local tag `jett-bench:0.6.0`); all 1,470 frozen input files matched the image.
Their exact bytes, including the Git-ignored `Cargo.lock`, are also preserved
in `benchmarks/results/2026-09-12_v0.6.0_reference_validation/frozen-inputs.zip`.
Every archived entry was verified against the campaign's input hash manifest.

Use the already installed Codex CLI **0.153.4** through task-local PATH
selection on this machine. The older npm CLI 0.145.0 cannot parse an app-written
experimental configuration field during `login status`; it made no model calls.
The newer CLI confirms ChatGPT login. User configuration and credentials were
not edited or copied, and each evaluated call still ignores user configuration
and rules. Every raw row records the actual CLI version and subscription backend.

## In-flight verification (2026-09-12)

At this earlier in-flight checkpoint, the original generator was live and
incremental staging had completed 271 initial assessments. Later interruption
details below supersede its live status. Never restart a generator while its
process or execution handle is confirmed active.

- A read-only audit compared all 254 responses present at that audit's snapshot
  with their retained event traces. Every input, cached-input, output, and
  reasoning token count matched; every event hash matched, and each trace had
  exactly one completed turn. All rows used `codex_subscription` and CLI
  `0.153.4`. Cache-write input tokens were zero in all those traces; this extra
  field remains in the raw events rather than the result table.
- The focused campaign, attempt-preservation, and grading suites passed all
  29 tests: `python -m unittest tools.tests.test_jett_bench_campaign
  tools.tests.test_jett_bench_attempts tools.tests.test_jett_bench_grading -q`.
- A two-task contract spot-check found no reason to discard or regrade results.
  Several `data_ledger_audit` submissions incorrectly include the starting
  balance in a minimum explicitly defined over balances after entries. A
  contract-based arithmetic check reproduced all 28 shared expected reports.
  Several `text_percent_decode` submissions add an empty-input rejection not
  specified by its malformed/range rejection rules. Its unused `Empty` variant
  is a wording-clarity caveat to disclose, not an established contradiction or
  cross-language grader difference. Preserve the frozen task and its scores;
  consider clearer wording in a later task edition, outside this comparison.

## Interrupted-process checkpoint (2026-09-12)

At 06:26:41 UTC, direct process inspection confirmed that generator PID 51024
and its scoped children were absent; the former execution handle was also
unavailable. There were 365 unique saved initial rows, 368 attempt receipts,
and 365 event files. All saved responses now have staging grades, verified
against their exact raw rows and the pinned image. The six canonical grades
agree exactly with their staging counterparts.

Three unjournaled receipts remain unchanged with `status: started`:
`score_lines` / Rust / skill-assisted; `score_lines` / Python / no skill;
and `text_ipv4_parse` / Python / skill-assisted. Their matching event files
are absent; three timestamp-associated temporary directories are empty. An
independent audit found no recoverable answer or usage. The cause of process
termination and server-side completion remain unknown. Do not infer zero
tokens or fabricate failed candidates from these infrastructure interruptions.

The frozen subprocess runner buffers events until it returns or handles an
error. Abrupt termination can lose those buffered events. The existing resume
guard correctly refuses all unjournaled attempts, regardless of status. No
replacement-sampling policy existed for this case, so a user decision was
requested before repeating the three exact prompts. At that checkpoint, no new
model calls had been made since the interruption. Keep the guard and original receipts intact;
any authorized recovery must separately link and disclose replacement attempts
and keep interrupted-call consumption unavailable.

The stale lock was moved, with its bytes and hash preserved, to
`target/jett-bench/v0.6.0-four-cell/recovery/terminated-generator.lock.json`
after another direct PID-absence check. The recovery receipt records the
evidence and pending decision. The published partial evidence archive lives in
`benchmarks/results/2026-09-12_v0.6.0_interrupted_checkpoint/`; it is not a
completed four-cell result. The full goal remains open.

The user subsequently answered **"Yes, repeat and disclose"**. A separate
`recovery/2026-09-12_approved_repetition.json` decision binds that approval to
the three exact original receipt and prompt hashes. The earlier interruption
receipt and published checkpoint remain immutable historical evidence. Each
replacement must carry a link to its original attempt and this decision;
another interrupted replacement would require a new explicit decision, not an
automatic third draw. Completed-response usage must remain distinguished from
unknown interrupted-call overhead in the eventual report.

All three approved repetitions have now completed. Their new response rows
link to the approval and original attempt hashes, with the original calls'
usage and server completion explicitly unavailable. The first 365 raw-response
rows match the published checkpoint byte for byte; all three original receipts
remain unchanged. There are now 368 saved and staging-graded responses at this
recovery checkpoint, including the three replacements, which all passed.

The remaining 632 initial prompts resumed through the unchanged generator
at 06:48:49 UTC as detached process 61200. Do not restart it while it is live.
The host-side supervisor can observe that process and complete the handoff:

```powershell
powershell.exe -NoProfile -File tools/bench_campaign_continue.ps1 `
  -Campaign target/jett-bench/v0.6.0-four-cell `
  -Staging target/jett-bench/v0.6.0-four-cell-staging `
  -Image jett-bench:0.6.0 -InitialGeneratorId 61200 -ConfirmSubscriptionUsage
```

Use the same task-local CLI PATH and a hidden detached process with persistent,
previously unused stdout/stderr logs. The supervisor first finishes the existing
staging batch, then snapshots and grades completed responses while the observed
generator is live. It never restarts initial generation. Only after the full
initial merge and grading checks succeed does it generate one repair for each
failed initial response, grade those repairs, and write the original report.
The skill follow-up remains a separate required phase.

Every failed command stops the supervisor, including a snapshot that races with
a partial JSONL append. Inspect its logs and actual process state before
restarting the supervisor; never infer that a missing execution handle means a
generator has stopped. A supervisor restart resumes saved grading. The unchanged
generation guard still refuses an unjournaled model attempt; do not remove that
evidence or authorize another repetition implicitly.

## Final publication audit

After both campaign stages, grading, and reporting have finished, run the
separate read-only audit before publication:

```text
python tools/bench_campaign_audit.py target/jett-bench/v0.6.0-four-cell
```

Save successful JSON output outside the audited campaign directory. The audit
rejects an active lock and changing evidence, verifies the complete task matrix
and failed-only repair pairs, and checks completed grades, image identity,
raw-to-grade fields, strict retained event metrics, response IDs, attempt
coverage, and exact recovery approvals. It never calls models or graders and
does not modify the evidence. It also supports the separate 100-row Jett skill
follow-up by deriving its expected matrix from that campaign's manifest.

The JSON includes all four cells and accounting over unique saved initial and
repair responses. Do not sum all four-cell usage rows: repair-budget cells
already include the initial calls. The legacy aggregate rollups describe
initial responses only and are not complete campaign consumption.

Publish the frozen report and summary unchanged, alongside the audit JSON,
an explanatory README, and hash-verified raw evidence. For this recovered
campaign, the three authorized replacement responses count once; their three
lost predecessors retain unknown token use, latency, tool use, and server
completion. Complete campaign consumption is therefore unavailable, not the
sum of saved-response usage. Missing optional response metrics remain null;
extraction failures retain response usage but have no measured source size.

## Skill follow-up execution constraints

Complete and publish the original report before editing any frozen input.
Keep its immutable image: the current-checkout reporter correctly refuses to
report an old campaign after a skill changes. The frozen reporter inside the
old image can reproduce that campaign's report from its retained evidence.

For the follow-up, use a new benchmark version, subset, configuration, baseline
directory, campaign directory, and image. Keep all five languages in the
configuration; select only Jett and its skill context at preparation time:

```text
python tools/jett_bench.py --config NEW_CONFIG validate
python tools/jett_bench_campaign.py verify-baselines NEW_BASELINES --config NEW_CONFIG --jobs 3
python tools/jett_bench_campaign.py prepare NEW_CAMPAIGN --config NEW_CONFIG --baselines NEW_BASELINES --expected-tasks 100 --language jett --track skill_assisted
python tools/jett_bench_campaign.py generate NEW_CAMPAIGN --jobs 3 --confirm-subscription-usage
python tools/jett_bench_campaign.py grade NEW_CAMPAIGN --image NEW_IMAGE --jobs 3
python tools/jett_bench_campaign.py generate NEW_CAMPAIGN --stage repair --jobs 3 --confirm-subscription-usage
python tools/jett_bench_campaign.py grade NEW_CAMPAIGN --stage repair --image NEW_IMAGE --jobs 3
python tools/jett_bench_campaign.py report NEW_CAMPAIGN
```

Replace the uppercase path/image placeholders; use the task-local CLI selection
above for generation. A skill-only change still requires a fresh 500-baseline
gate and image because their full input manifests include skills. Keep compiler,
task, grader, and toolchain behavior equivalent across the skill comparison.

Do not combine both revisions with the generic `aggregate` command: its grouping
dimensions do not distinguish benchmark versions or skill hashes. Publish each
campaign summary separately, then compare matching task outcomes explicitly,
including gains, regressions, and unchanged outcomes for both prompt budgets.
