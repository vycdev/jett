# Jett benchmark subset v0.3

This capability profile extends `jett-v0.2` with recursion and source
maintenance. Earlier task behavior and exclusions remain unchanged.

## Added to v0.3

- directly recursive enum payloads;
- recursive functions over move-only enum values;
- nested propagation of typed success/error outcomes;
- integer division, including truncation toward zero;
- complete-file evolution from supplied starter source.

`recursive_expression` evaluates a typed expression tree and propagates
`division_by_zero` through nested operations. `account_state_evolution` starts
with a three-variant account model and requires adding a payload-bearing state,
updating existing matches, and implementing one new transition.

## Submission contract

The v0.2 Jett contract still applies. Maintenance prompts additionally include
the exact starter file and require one complete replacement file. The harness
records its SHA-256 digest. Starter files and public signatures are prompt
material; hidden graders remain excluded.

Any change to recursion semantics, division behavior, starter source, required
declarations, policy, or hidden grading increments the affected version.
