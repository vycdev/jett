# Benchmark protocol v0.5.2

This protocol inherits v0.5.1 and changes only the serialized Jett programming
skill. The targeted v0.5.1 smoke run removed the prior mutable-local failure
pattern and passed 8/10 tasks. Its two remaining submissions failed because
ordinary call arguments were wrapped across physical lines. One repair retained
that form; the other corrected it but guessed an unsupported integer-rendering
function.

The v0.5.2 Jett skill therefore adds two general, compiler-backed anchors:

- ordinary function and constructor argument lists stay on one physical line;
  long calls use typed intermediate values instead of newline continuation;
- decimal integer rendering uses `string.from_int64(value)`.

No task, adapter, grader, language rule, or non-Jett skill changes. Historical
v0.5.1 smoke artifacts remain immutable. New rows use benchmark version
`0.5.2-pilot`, subset `jett-v0.5.2`, and the new recorded skill hash.
