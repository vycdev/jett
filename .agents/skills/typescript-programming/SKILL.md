---
name: typescript-programming
description: Implement, modify, review, or repair strict TypeScript programs with type-driven modeling and project checks. Use when the requested deliverable is TypeScript source.
---

# TypeScript Programming

Use strict TypeScript types to preserve domain information through implementation.

## Workflow

1. Inspect the requested public interface, nearby TypeScript source, and the effective `tsconfig.json`.
2. Read [references/language.md](references/language.md) when modeling unions, narrowing, immutability, or checker diagnostics.
3. Derive precise parameter, return, collection, and state types before implementation. Split work at typed helper boundaries.
4. Preserve project conventions and narrow through control flow; do not escape through `any`, unchecked assertions, or suppression comments.
5. Format, type-check, and test with the project commands. Repair the earliest useful diagnostic before proceeding.

## Boundary

This is a general TypeScript programming skill. Do not add algorithms, requirements, edge cases, or examples taken from evaluation tasks. During an evaluation, use only its public prompt, supplied source, this skill, and public compiler feedback; never inspect hidden graders or repository-owned solutions.
