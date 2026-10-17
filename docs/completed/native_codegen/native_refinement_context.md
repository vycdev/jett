# Refinement predicate declaration contexts

A refinement predicate is checked where its type is declared. Its private helper
functions, interfaces, and type names must retain that declaration context when
the value crosses a boundary elsewhere. Native predicate functions already
have their declaration namespace. The interpreter previously evaluated the
predicate in the caller's namespace, so an exported refinement using
`Named.name(view value)` failed when validated by another namespace.

Predicate evaluation now isolates caller locals, block-local `use` aliases,
generic arguments, scoped type bindings, and checked function maps. The only
runtime binding supplied to the predicate is its typed base `value`. Its checked
declaration facts and explicit comptime constants remain available. The base
type resolves in that same declaration context before evaluating inherited
constraints. The caller's complete context is restored on success and failure.

The checker gives `value` the fully coarsened base type (and removes the outer
secret wrapper for predicate evaluation). Binding an intermediate refinement
instead changes interface dispatch inside an inherited predicate. The
interpreter uses the same predicate input type as the checker and native
predicate function.

Differential regressions cover private helper lookup, interface-backed
refinements, inherited predicates, caller shadowing, generic and lexical type
bindings, explicit comptime predicates, and caller execution after a rejected
boundary. This implements ordinary lexical scope; it adds no dynamic name
lookup or caller capture to the language.

The regression also includes a parenthesized explicit comptime expression.
HIR preserves the original comptime span as its evaluated-value lookup key
separately from the enclosing expression span, which parentheses may widen.

## Refinements of whole sum values

A refinement predicate receives its declared base value. A refinement whose base
is `optional[Named]` or `result[Named, string]` therefore validates the whole sum,
including its absent or failure branch. Refined struct-field construction already
implements this boundary. Direct bindings previously routed a `handle error:`
to sum extraction first, so the checker rejected the same base value and the
interpreter/HIR selected the wrong operation. The boundary now follows the
declared refinement base consistently.

Direct refinement boundaries must take precedence when the checked source is an
exact ancestor/base accepted by the refinement. An outer result or optional
whose payload is the desired value still uses ordinary sum extraction; this does
not implicitly combine extraction and validation or introduce another spelling.
The error binding for refinement validation is always a string, independently
of a result base's own error type. Linked tests must cover both branches,
inherited refinements, interface payload owners, callbacks, pending values, and
runtime/comptime construction. `interface_refined_sums` supplies that coverage
in debug/release executables after source removal, including rejection of a
fresh pending input and ordinary extraction of already refined payloads.

The pending input exposed a separate extraction bug: native MIR sum tag reads
did not check outer pending depth. `SumHandleTag` now checks before returning the
tag and formats the pending value with its checked layout. It preserves the sum
and owned payloads on failure. A ready sum may still contain a pending payload;
only the outer depth prevents extraction. `pending_sum_handle_failure` checks
present/absent optionals, both result branches, nested depth, a record payload,
exact terminal diagnostics, and cleanup in both profiles.
