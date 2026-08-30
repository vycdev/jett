---
name: jett-programming
description: Implement, modify, review, or repair Jett source with the Jett compiler and type-driven workflow. Use for .jett programs and Jett language questions; do not use merely because the compiler implementation is written in Rust.
---

# Jett Programming

Program against the implemented Jett surface, not remembered or proposed syntax.

## Workflow

1. Inspect the requested public interface, nearby `.jett` source, and project configuration.
2. Read [references/language.md](references/language.md) before writing unfamiliar Jett syntax or repairing a compiler error.
3. Derive concrete types and ownership transfers first. Split work at typed helper boundaries before adding branches.
4. Preserve top-to-bottom declaration order, explicit effects, public namespace boundaries, and move-only values.
5. Format, build, and test with the commands in the reference. Repair the earliest useful diagnostic before proceeding.

Prefer compiler-accepted local precedent over speculative design documentation. If the requested behavior is not established, record the design question instead of inventing a new language rule in source.

## Boundary

This is a general Jett programming skill. Do not add algorithms, requirements, edge cases, or examples taken from evaluation tasks. During an evaluation, use only its public prompt, supplied source, this skill, and public compiler feedback; never inspect hidden graders or repository-owned solutions.
