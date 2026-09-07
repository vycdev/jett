---
name: go-programming
description: Implement, modify, review, or repair Go programs with concrete types and standard Go tooling. Use when the requested deliverable is Go source.
---

# Go Programming

Use concrete Go types and small explicit interfaces to keep program states visible.

## Workflow

1. Inspect the requested public interface, nearby Go source, and the module's declared Go version.
2. Read [references/language.md](references/language.md) when modeling variants, ownership, errors, or compiler diagnostics.
3. Derive concrete parameter, return, collection, and state types before implementation. Split work at typed helper boundaries.
4. Preserve package conventions; avoid `any`, reflection, panic, or conversion merely to bypass a design mismatch.
5. Format, vet or compile, and test with the project commands. Repair the earliest useful diagnostic before proceeding.

## Boundary

This is a general Go programming skill. Do not add algorithms, requirements, edge cases, or examples taken from evaluation tasks. During an evaluation, use only its public prompt, supplied source, this skill, and public compiler feedback; never inspect hidden graders or repository-owned solutions.
