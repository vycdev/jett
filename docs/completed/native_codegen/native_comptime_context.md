# Explicit comptime values in checked type contexts

Explicit comptime expressions in a generic body are evaluated for each checked
instantiation. For example, `comptime type.name[T]()` must produce `int64` in
`label[int64]()` and `string` in `label[string]()`. A source span alone cannot
identify the result. The existing global span cache instead produces `T` for
both calls, contradicting the documented generic specialization contract.

The evaluation identity must retain the checked generic arguments and
reflection-visible specialization, plus the complete lexical `comptime type`
binding chain. Source aliases that share a canonical type must remain distinct
when reflection distinguishes them. Inline functions inherit their enclosing
type context, while ordinary calls isolate the caller's bindings.

Evaluation receives type bindings and checked expression facts, never runtime
parameters, locals, or captures. A generic expression referencing a runtime
parameter therefore still fails compilation. Only checked generic instances
are evaluated; an uninstantiated template supplies no concrete value.

The interpreter and native constant materializer must select the same context.
HIR retains lexical type bindings at the explicit expression, including scoped
branches whose enclosing function has no generic arguments. Missing evaluated
values are compilation failures, never permission to execute source comptime
computations at runtime. Nested explicit expressions evaluated while building
an outer constant use that evaluation's type context.

Validation must distinguish multiple generic instantiations and aliases at one
source span, lexical reflected bindings, nested binding chains, inline functions,
and forbidden runtime dependencies. Existing closed value, reflection, callback,
builder, and fixture suites remain regression gates.

The implementation now keys evaluated values by that complete context and
selects them identically in the interpreter and native HIR baker.
`tests/native/contextual_comptime_values.jett` covers distinct generic arguments,
aliases, reflected record fields, nested lexical bindings, live and baked
callbacks, checked type guards, runtime guards, nested explicit expressions,
generic forwarding, and isolation from caller type arguments. A compile-fail
fixture rejects a generic runtime parameter used as a comptime value.

Validation passed: the new linked differential fixture, 451 compiler-phase
tests, all 583 frontend fixtures, 11 existing native comptime tests, 11 native
interface tests, and workspace documentation tests. These checks establish this
context fix; they do not close the remaining native parity release gates.
