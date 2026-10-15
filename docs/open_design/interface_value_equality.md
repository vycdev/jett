# Equality of erased interface values

## Selected contract

`==` and `!=` are rejected at compile time when an operand's comparison type
is an interface, including the `Equatable` interface itself. Transparent aliases,
refinements and secret wrappers cannot restore an equality operation after
erasure. Concrete generic instantiations, inline function bodies and explicit
comptime expressions use the same rule. The diagnostic is E0376.

An enum's implicit payload comparison also cannot reach an interface value,
including through containers, machines and refinement wrappers. The recursive
check stops at a user struct's explicit `Equatable.equals` method: that method
owns the comparison and may deliberately ignore an interface-typed field.

Compare concrete values before erasure, or call an explicit domain comparison
function or interface method. Such a method can expose and compare an intended
primitive key; unrelated implementations may share that domain contract without
acquiring a general dynamic equality operation. Ordinary secrecy and ownership
rules still apply to these calls and their results.

## Rationale

Jett keeps policy explicit and locally visible. An interface says which
operations its implementation supplies; erasing a concrete type must not add a
comparison whose behavior depends on a hidden payload. Dynamic equality would
need additional rules for different concrete types, unsupported payloads,
secrets, pending values and method dispatch. Rejecting the operator provides one
checked rule without introducing a second equality protocol.

This preserves exact concrete `Equatable.equals` dispatch, the rule that `!=`
negates that method, and actor identity comparison. There is no handle-address
comparison or structural fallback for user structs. Direct collection equality
continues to report E0376 with its existing contents-comparison diagnostic.

## Previous behavior

The checker accepted same-interface operands without an equality contract.
Native verification rejected the binary expression, while the interpreter
discarded concrete metadata and attempted the ordinary payload operation. Two
erased integers could compare successfully; unrelated payloads failed at
runtime. That accidental behavior is replaced by the compile-time rule above.

The decision is recorded here to preserve the original design discussion;
this document no longer describes an open policy choice. Driver fixtures cover
the rejected source forms and positive explicit comparisons, and native
conformance checks frontend rejection before artifact publication plus linked
positive comparisons with the source removed.
