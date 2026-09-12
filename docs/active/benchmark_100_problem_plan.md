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

- Numerical, text, and structured task batches are being authored in parallel.
- The campaign runner freezes inputs, journals completed rows, resumes missing
  work, stops new dispatch on infrastructure failure, and produces complete
  four-cell reports only when all expected rows and repair pairings exist.
- Compiler and subscription availability are being revalidated against the
  updated repository before any large model run.
