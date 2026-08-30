---
name: python-programming
description: Implement, modify, review, or repair typed Python programs with project type checking and tests. Use when the requested deliverable is Python source, especially under strict static checking.
---

# Python Programming

Use Python's type system as a design tool while preserving ordinary readable Python.

## Workflow

1. Inspect the requested public interface, nearby Python source, and `pyproject.toml` or checker configuration.
2. Read [references/language.md](references/language.md) when modeling closed data, strict typing, or checker diagnostics.
3. Derive precise parameter, return, collection, and state types before implementation. Split work at typed helper boundaries.
4. Preserve project conventions and narrow through control flow; do not silence useful diagnostics with `Any`, casts, or ignores.
5. Format, type-check, and test with the project commands. Repair the earliest useful diagnostic before proceeding.

## Boundary

This is a general Python programming skill. Do not add algorithms, requirements, edge cases, or examples taken from evaluation tasks. During an evaluation, use only its public prompt, supplied source, this skill, and public compiler feedback; never inspect hidden graders or repository-owned solutions.
