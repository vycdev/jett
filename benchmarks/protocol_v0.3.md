# Benchmark protocol v0.3

This protocol retains the v0.2 isolation, backend separation, static checks,
policy preflight, and analysis rules. It adds recursive data and maintenance.
The onboarding treatment uses the language reference sheets at v0.4.

## Matrix

The pilot has 6 tasks x 5 languages x 2 tracks x 3 reasoning levels x 3
repetitions = 540 rows. The Codex-subscription calibration slice is 60 fresh
medium-reasoning rows with one repetition and no repairs.

## Recursive expression task

`recursive_expression` supplies equivalent recursive expression and typed
result models. It checks nested evaluation, propagation of division failure,
and signed integer division truncated toward zero. Static and policy checks
continue to forbid type erasure, catch-all branches, and panic/throw shortcuts.

## State-evolution task

`account_state_evolution` measures maintenance rather than greenfield coding.
Every language receives equivalent working starter source. The requested new
payload-bearing state must be added without regressing existing labels or
transitions. The prompt asks for a complete replacement file, and each result
records the starter digest.

## Interpretation

These tasks reduce the earlier ceiling effect, but v0.3 remains a calibration
suite. Compare languages separately by task and track before pooling results.
The maintenance task especially measures both comprehension of existing code
and the static model's help during a change; it is not interchangeable with a
zero-context implementation task.
