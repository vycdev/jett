# Jett benchmark subset v0.2

This capability profile extends `jett-v0.1` for the typed-domain benchmark.
It does not broaden the older tasks or imply that every implemented Jett
feature is part of the experiment.

## Added to v0.2

- closed user-defined enums with and without payloads;
- exhaustive `match` statements with payload destructuring;
- small typed helper functions declared before their callers;
- enum values as parameters and return values;
- typed accepted/rejected outcomes modeled as a closed enum.

The `order_lifecycle` task requires exact state, event, error, and outcome
types. Its grader covers the full 5 x 4 state/event product. Catch-all match
arms are forbidden so adding a variant cannot be silently absorbed by a
default branch.

## Retained exclusions

The v0.1 exclusions still apply unless listed above. In particular this task
does not use globals, nested declarations, capabilities, I/O, concurrency,
reflection, collections, mutation, or external effects.

## Submission contract

A Jett submission is one complete UTF-8 file in `namespace benchmark`. It must
retain every requested enum and function declaration. The grader appends
private `verify` blocks, builds the combined source, and runs those blocks.
Compiler diagnostics, forbidden type-policy patterns, failed assertions,
timeouts, and unexpected exits are distinct failures.

Any change to the declared variants, transition table, static policy, or hidden
grader increments the task version. Any Jett syntax or semantic change that can
alter a valid answer increments this subset version.
