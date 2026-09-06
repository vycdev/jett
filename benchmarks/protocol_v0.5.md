# Benchmark protocol v0.5

This protocol retains the v0.4 tasks, adapters, isolation, grading, and repair
rules. It adds a parity-matched programming-skill treatment.

## Matrix

The pilot has 10 tasks x 5 languages x 3 context tracks x 3 reasoning levels x
3 repetitions = 1,350 rows. The balanced Codex-subscription calibration slice
contains 150 medium-reasoning rows with one repetition and no repair prompts.

The context tracks are:

- `zero_shot`: public task and required declaration only;
- `onboarding`: the compact language reference and common type-driven guidance;
- `skill_assisted`: common type-driven guidance plus the complete versioned
  programming skill for that language.

## Skill parity

Jett, Python, TypeScript, Go, and Rust each have a real repo-scoped Codex skill
under `.agents/skills/`. Every skill has the same minimum capability structure:

- a discriminating programming-only description;
- a type-driven implementation and compiler-repair workflow;
- explicit evaluation-contamination boundaries;
- a language and verification reference;
- UI metadata with implicit invocation left enabled.

Equal structure does not mean equal byte count. Jett needs more explicit syntax
because the model has little prior exposure. The harness records `skill_bytes`
and `skill_sha256` on every skill-assisted row so that context exposure remains
auditable.

## Deterministic evaluation materialization

The benchmark serializes `SKILL.md` and sorted Markdown references directly
into the skill-assisted prompt. This avoids making automatic skill-routing or
filesystem tool use an uncontrolled variable. It tests the exact skill content
in one generation prompt; the optional paired compile-and-repair stage remains
a separate second prompt.

## Contamination boundary

Skills are authored from public language documentation, compiler help, and
ordinary language knowledge. They may not contain task identifiers, task
requirements, repository baselines, starter implementations, hidden graders,
observed benchmark solutions, or failure-specific recipes. Validation checks
the common structure, scans task identifiers, and verifies that no complete
baseline, starter, or hidden source appears in a skill bundle.
