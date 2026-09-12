# Jett benchmark subset v0.6.0

The first 100-problem campaign uses the implemented compiler and stdlib captured
by its input hashes, with the general Jett skill content from v0.5.4. The task
expansion exercises bounded pure functions, closed data, optional values,
lists/maps/sets, strings, numeric operations, and typed maintenance. Every
public adapter must have a passing baseline on that exact compiler.

This benchmark version does not change the language. Proposed behavior in
`docs/open_design/` is not available merely because a prompt might benefit from
it. The source tree and accepted baselines, not remembered syntax, establish
what can be implemented. Candidate compilation and hidden verification are
now separate grading phases as described in `protocol_v0.6.0.md`.

The later Jett-skill follow-up must use a separate benchmark version and
recorded skill hash while keeping problems, other skills, compiler, and graders
fixed. No task solution or task-specific recipe may enter a language skill.
