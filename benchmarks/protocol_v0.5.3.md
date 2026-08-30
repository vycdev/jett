# Benchmark protocol v0.5.3

This protocol inherits v0.5.2 and changes only the serialized Jett programming
skill. The targeted v0.5.2 smoke run passed 6/10 tasks initially and repaired
all four failures after one compiler-feedback prompt. Both failures targeted by
v0.5.2 passed in the initial run.

The remaining failures exposed two general onboarding gaps:

- map membership uses `map.has` or `map.contains_key`; there is no
  `map.contains`;
- nested matches and every `and` or `or` condition contribute decision points
  to the function complexity limit, so helpers should be extracted before
  writing a full branch matrix.

No task, adapter, grader, language rule, or non-Jett skill changes. Historical
v0.5.2 smoke artifacts remain immutable. New rows use benchmark version
`0.5.3-pilot`, subset `jett-v0.5.3`, and the new recorded skill hash.
