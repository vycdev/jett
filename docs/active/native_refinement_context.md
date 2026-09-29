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
