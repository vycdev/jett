# Explicit Comptime Expression Semantics

Status: completed. Required compile-time value evaluation uses one explicit
source form: `comptime expression`.

## Decision

An expression prefixed with `comptime` must be evaluated during compilation and
its resulting value is baked into the program. The expression must be closed
and pure. It may use literals and call any pure project, dependency, or standard
library function, but it cannot read runtime parameters or locals and cannot
call a capability-bearing function.

Ordinary pure calls remain runtime calls in source semantics. A later optimizer
may fold them, but folding is invisible: it cannot add or remove diagnostics or
change whether a program compiles. Jett therefore has one canonical way to ask
for required build-time value evaluation and no implicit semantic threshold.

## Implementation

The parser represents `comptime expression` directly in the expression AST and
continues to distinguish the existing `comptime type Name = ...:` statement.
The type checker reuses semantic function purity for the new context and emits
E0504 for direct impure calls. Transitive impurity remains rejected by the
ordinary pure-call rule.

After successful type checking, the driver walks all executable expressions in
the merged module. Each explicit site is evaluated by the comptime interpreter
with checked reflection and expression-type metadata but an empty lexical
environment. Failure to produce a closed value emits E9001. Successful values
are stored by source span in the build result, then installed into the runtime
interpreter so execution reads the baked value rather than rerunning the inner
expression.

## Coverage

Coverage pins parser disambiguation, a namespaced pure call baked to `42`,
runtime consumption of the baked value, E9001 for a runtime-local dependency,
E0504 for capability access, and successful compilation of an unused ordinary
pure call that would recurse indefinitely if implicitly executed.
