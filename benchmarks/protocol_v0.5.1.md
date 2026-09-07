# Benchmark protocol v0.5.1

This protocol inherits the v0.5 tasks, adapters, matrix, isolation, grading,
repair, parity, deterministic-materialization, and contamination rules. It
changes only the Jett programming-skill treatment after the v0.5.0 calibration
identified a documentation defect.

The Jett skill now gives compiler-checked syntax anchors for:

- the different declaration forms used by fields, parameters, and locals;
- mutable local rebinding without `let`, `var`, or colon syntax;
- qualified enum construction and unqualified exhaustive match arms;
- optional and result handling;
- view-based collection observation, explicit cloning, and consuming updates;
- splitting complex control flow into typed helpers declared before callers.

The examples are generic language examples derived from public documentation,
compiler signatures, and accepted standard-library source. They contain no
benchmark task, fixture, hidden grader, observed solution, or task-specific
repair advice. Every Jett code fence was format-checked and compiled before
this revision was activated.

The matrix remains 1,350 study rows, with a balanced 150-row medium
Codex-subscription calibration slice. Historical v0.5.0 artifacts remain
immutable; new rows use benchmark version `0.5.1-pilot`, subset
`jett-v0.5.1`, and the new skill hash recorded by the harness.
