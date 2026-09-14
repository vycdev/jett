# Benchmark results

Each result directory is immutable after publication and contains a manifest,
machine-readable rows, an aggregate summary, and a short interpretation. New
runs use new directories; corrected data receives a new run identifier rather
than silently replacing an existing artifact.

Backends are separate experimental treatments. Do not pool Codex subscription,
Responses API, or future local-model rows unless a protocol explicitly defines
that comparison.

The typed-domain follow-up is in
`2026-08-30_codex_luna_medium_v0.2_calibration/`. It records 40 subscription
cells plus a manual compliance audit that found one Go lifecycle grader false
positive. Treat it as calibration evidence only; lifecycle task v1.0.0 is not
suitable for a larger study.

The ten-task calibration is in
`2026-08-30_codex_luna_medium_v0.4_calibration/`. It records 100 subscription
cells, an 80/100 static-check result, token and code-size totals, and three
behaviorally correct Python rows rejected by a strict-checker style diagnostic.

Its paired one-prompt repair pass is in
`2026-08-30_codex_luna_medium_v0.4_compile_repair/`. Ten of the 20 initial
failures were repaired, producing a 90/100 pass-after-repair result. All seven
established-language failures were repaired; Jett repaired 3 of 13.

The parity programming-skill calibration is in
`2026-08-30_codex_luna_medium_v0.5_calibration/`. It records 150 subscription
cells across zero-shot, onboarding, and skill-assisted tracks. Initial grading
passed 116/150. The established-language skill track tied onboarding at 37/40,
but Jett's skill track passed only 1/10 because its reference lacked a concrete
mutable-local example and repeatedly induced invalid declaration syntax.

Its paired repair pass is in
`2026-08-30_codex_luna_medium_v0.5_compile_repair/`. Fifteen of 34 failures
were repaired, producing a 131/150 final result. Jett ended at 14/30; the four
established languages ended at 117/120.

The corrected Jett-skill smoke run is in
`2026-08-30_codex_luna_medium_v0.5.1_jett_skill_smoke/`. It passed 8/10 initial
tasks, and the prior mutable-local failure pattern disappeared. Its paired
repair pass is in
`2026-08-30_codex_luna_medium_v0.5.1_jett_skill_smoke_compile_repair/`; neither
of the two remaining parse failures repaired. Those findings produced the
versioned v0.5.2 skill correction.

The v0.5.2 follow-up is in
`2026-08-30_codex_luna_medium_v0.5.2_jett_skill_smoke/`. It passed 6/10
initial tasks; both failures targeted by v0.5.2 now passed. Its paired repair
pass is in
`2026-08-30_codex_luna_medium_v0.5.2_jett_skill_smoke_compile_repair/` and
repaired all four failures, producing 10/10 pass-after-repair. The failures
exposed map-membership and complexity-accounting gaps, producing v0.5.3.

The controlled v0.5.3 Jett 2x2 comparison is in
`2026-08-30_codex_luna_medium_v0.5.3_jett_2x2/`. Zero-shot passed 0/10
one-shot and 1/10 after repair. Skill-assisted passed 9/10 one-shot and 9/10
after repair. Its sole repair used reserved word `result` as an identifier,
producing the v0.5.4 lexical clarification.

The 100-problem v0.6.0 reference checks are in
`2026-09-12_v0.6.0_reference_validation/`, with all 500 task/language cells
validated locally and in containers. Its exact 1,470 source inputs, including
the ignored Cargo lockfile, are preserved separately in
`2026-09-12_v0.6.0_frozen_inputs/`.

The immutable `2026-09-12_v0.6.0_interrupted_checkpoint/` preserves the original
365 saved responses and three interrupted attempts without recoverable answers.
The user subsequently approved repeating those exact three prompts once, with
disclosure and unchanged original evidence. The checkpoint is not a final ranking.

The complete 100-problem four-cell campaign is in
[`2026-09-12_v0.6.0_four_cell/`](2026-09-12_v0.6.0_four_cell/README.md).
It preserves all 1,000 initial responses and 234 failed-only repairs, with
766/1,000 initial and 845/1,000 final passes. Jett's original skill treatment
passed 33/100 initially and 66/100 after repair. The independent audit, all
20 metric cells, and complete raw evidence are published together; consumption
for the three lost original calls remains unknown.

The completed full-100-task Jett skill follow-up is in
[`2026-09-12_v0.6.1_jett_skill/`](2026-09-12_v0.6.1_jett_skill/README.md).
It passed 71/100 one-shot and 87/100 after repair, compared with 33/100 and
66/100 for the original skill. Paired gains/regressions were 43/5 one-shot
and 26/5 after repair. It preserves all 129 responses, a fresh 500-reference
gate, exact frozen inputs, full metrics, and row-level comparisons. The two
campaigns are reported separately; their combined consumption ledger counts
1,363 saved responses once while retaining unknown overhead for the three
interrupted originals.
