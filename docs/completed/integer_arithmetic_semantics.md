# Integer Arithmetic Semantics

Status: completed for the interpreter-backed compiler.

## Decision

Fixed-width integer arithmetic is total after type checking. Addition,
subtraction, multiplication, and negation wrap modulo the checked primitive
width. Division and modulo wrap at the same width, including the signed
minimum divided by `-1`, but their divisor must be statically proven nonzero.
There are no runtime integer-overflow or division-by-zero exceptions and no
arithmetic `result` values.

Floating-point arithmetic follows IEEE behavior. In particular, division by
zero produces infinity or NaN instead of a language-level failure.

This contract favors predictable execution for generated programs. Code that
needs a mathematical non-overflow invariant expresses its permitted values as
refinement types; wrapping remains the primitive operation outside that domain.

## Nonzero Proof Boundary

The checker accepts intentionally local evidence:

- a nonzero integer literal;
- an integer refinement whose constraint excludes zero;
- an immutable binding initialized from already proven evidence;
- the appropriate branch of a visible `value == 0` or `value != 0` condition,
  including conjunctions, disjunctions, negation, loops, and a simple early
  return guard.

Assignment clears facts about a mutable binding. Arbitrary helper predicates,
detached booleans, and hidden control flow do not create proof evidence. This
keeps the rule readable at the use site and avoids a second annotation system.
The public `math.mod` entrypoint uses the same divisor rule as the `modulo`
operator.

## Runtime Boundary

The interpreter normalizes an arithmetic result using the type already checked
for that expression. This matters for the smaller integer primitives, which
share an `int64` carrier internally, and for `uint64`, which has a distinct
full-range carrier. Trusted integer aggregation and math kernels use matching
wrapping operations so public helpers do not reintroduce overflow failures.

Zero-divisor checks remain as internal interpreter invariants for malformed or
unchecked AST execution. Accepted Jett source cannot reach them.

## Coverage

Fixtures pin every primitive-width wrapping boundary, signed minimum divided
by `-1`, wrapping math and list aggregation, IEEE infinity and NaN, refinement
and visible-guard proofs, and E0361 diagnostics for unproven operator and
`math.mod` divisors. Interpreter unit tests pin the unchecked internal edge
cases separately.
