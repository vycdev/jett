# Benchmark protocol v0.5.4

This protocol inherits v0.5.3 and changes only the serialized Jett programming
skill. The targeted v0.5.3 2x2 run compared zero-shot and skill-assisted Jett
under one-shot and one compile-repair prompt. Zero-shot passed 0/10 initially
and 1/10 after repair; skill-assisted passed 9/10 initially and remained 9/10
after its sole repair failed.

That repair addressed the reported complexity problem but used the reserved
built-in type word `result` as a parameter name. The v0.5.4 skill therefore
adds the general lexical rule that keywords and built-in type spellings cannot
be identifiers.

No task, adapter, grader, language rule, or non-Jett skill changes. Historical
v0.5.3 artifacts remain immutable. New rows use benchmark version
`0.5.4-pilot`, subset `jett-v0.5.4`, and the new recorded skill hash.
