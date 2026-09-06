---
name: rust-programming
description: Implement, modify, review, or repair Rust programs with ownership-aware type-driven design and Cargo checks. Use when the requested deliverable is Rust source.
---

# Rust Programming

Use Rust's types, ownership, and exhaustive matching to make invalid states difficult to express.

## Workflow

1. Inspect the requested public interface, nearby Rust source, and the crate edition and feature configuration.
2. Read [references/language.md](references/language.md) when modeling enums, borrowing, errors, or compiler diagnostics.
3. Derive precise parameter, return, collection, and state types before implementation. Split work at typed helper boundaries.
4. Preserve crate conventions; borrow when ownership is not required and do not use cloning, panic, unsafe, or casts to hide a design mismatch.
5. Format, check, lint when configured, and test with Cargo. Repair the earliest useful diagnostic before proceeding.

## Boundary

This is a general Rust programming skill. Do not add algorithms, requirements, edge cases, or examples taken from evaluation tasks. During an evaluation, use only its public prompt, supplied source, this skill, and public compiler feedback; never inspect hidden graders or repository-owned solutions.
