# Pure-Only Comptime Function Boundary

Status: completed. Semantic purity alone determines whether a Jett function is
eligible to execute in a comptime context.

## Decision

Any pure function may run at comptime, including functions from the current
project, dependencies, and the standard library. Jett has no separate
comptime-safe annotation, declaration category, or function allowlist.
Explicit comptime syntax selects the evaluation site; it does not grant a
function permission to execute there.

A function that accepts any capability is ineligible. The compiler does not
provide `Filesystem`, `Network`, `Clock`, `Random`, `Environment`, `Process`,
`Foreign`, or any other runtime capability during compilation. This excludes
I/O and ambient machine observations even when they would be technically
possible for the compiler process to perform.

Capability requirements propagate through calls. A function without
capability parameters cannot call a capability-bearing function, so a pure
wrapper cannot conceal an impure operation from the comptime boundary.

## Current Enforcement

`verify` blocks and explicit `comptime expression` sites are executable
value-level comptime entrypoints. The type checker classifies every named call
from its semantic capability signature. Pure calls proceed without any
origin-based filtering; impure calls produce E0501 in verify blocks or E0504 in
explicit expressions before the interpreter runs. The same check applies to
pipeline steps. Existing E0500 propagation prevents transitive capability
access.

The comptime interpreter registers the merged module, so eligible project,
dependency, and source-defined stdlib functions use the same execution path.
Explicit comptime calls reuse this purity result rather than introducing a
second eligibility table.

## Coverage

Integration coverage executes composed project helpers and ordinary string
stdlib functions from a `verify` block. Type-checker coverage accepts user pure
helpers and rejects a user function with a capability parameter. Clock and
random verify fixtures pin capability-backed builtins as unavailable.
Explicit-expression fixtures additionally pin closed-value evaluation,
capability rejection, and the distinction between required comptime evaluation
and optional optimizer folding.
