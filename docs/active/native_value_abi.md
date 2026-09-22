# Native value ABI and ownership slice

This is an implementation contract, not a full-parity claim.

Immutable UTF-8 strings use nonzero u64 context-associated handles. Zero is an
uninitialized slot, never a language string. Each owning slot holds one reference.
Literal creation, concatenation and formatting create one reference; retain
creates another; release removes it immediately and destroys the last reference.
Borrowed inputs are valid only for a call. Handles cannot cross contexts and are
never reused. No interpreter Value, source, AST or HIR enters the runtime.
Stdout authority is a separate context-bound token, supplied only for checked
Stdout entry parameters and passed explicitly to printing leaf operations.

Generated functions and all source control flow remain Cranelift machine code.
Copyable string arguments are borrowed by the native call ABI; callee owning
locals retain them. String expression results are owned. Results transfer one
reference to the caller. Assignment evaluates its RHS before releasing its old
slot. Full-expression temporaries have zero-initialized frame slots; all are
released at expression completion and on terminal failure. MIR liveness and
initialization analysis defines local cleanup points, including loop backedges
and early exits. Zero slots permit conditional initialization without releasing
an uninitialized value. This slice does not establish move-only resource or
aggregate ownership, and those types remain rejected.

A context-local first terminal failure (fixed status plus static message) is
separate from Jett result.fail. Fallible leaf and compiled calls are followed
by a failure edge, which releases live temporaries/locals before returning a
zero placeholder. Callers must inspect failure before using that placeholder.
The entry wrapper returns failure status and the launcher renders the message
before context destruction. Cleanup does not overwrite the first failure.
Context destruction checks that all owning values have been released; it must
not silently hide compiler leaks by acting as a program-long value arena.

Runtime exports use fixed-width scalar parameters and typed leaf operations;
no universal operation/name dispatcher. Panics are contained at each C boundary.
The existing ABI v1 lifecycle remains compatible; added leaf signatures are
shared by the runtime and emitter. Layout/tag tests pin the added contract.

Temporary capacity is a structural MIR bound on emitter ownership operations,
not a multiplier on string-typed expressions. Literals, string-local retains,
string-call results and string-producing intrinsics each own one slot.
Interpolation owns an initial empty string, then one concatenation per segment
and one formatting slot per scalar segment (text segments own a literal).
Print owns its empty accumulator, argument formatting/concatenations, two slots
per separator, and two for a newline. Child costs accumulate recursively until
full-expression cleanup; both short-circuit branches get distinct frame slots.
The maximum full-expression cost determines the frame capacity.

## Float32 oracle correction

The design defines float32 as 32-bit floating point, with IEEE arithmetic.
Retaining parsed f64 precision in the interpreter contradicts that contract:
`(value + 1.0) - 1.0` at float32 must round the intermediate addition, not
just the final result. Native already emits typed f32 instructions. The
interpreter therefore must normalize float32 at its existing checked-expression
and declared-type boundaries (including literal, assignment, argument, return,
and explicit comptime evaluation), just as it already normalizes narrow integers.
The internal Value::Float64 f64 carrier may represent a widened f32, but cannot
retain extra precision for a checked float32 expression. Alias/refinement base
type resolution remains centralized in the existing normalization boundary.

Two possible formatting changes were considered: dedicated shortest-f32 decimal
formatting, or retaining the current shared f64-carrier display. This fix retains
the existing display: normalize the value first, then format the exactly widened
f32 with the same f64 conversion used by native and interpreter. Thus float32
0.1 displays as 0.10000000149011612, while float64 0.1 still displays as 0.1.
This does not introduce a new shortest-f32 formatting policy, disable formatting,
or treat the old unnormalized f64 oracle as evidence of correct float32 behavior.
Regression tests compare actual executable bytes to the corrected checked oracle
and pin formatting-dependent string equality/control flow, including comptime.


`print` and `println` retain their existing language diagnostic-output permission:
their dedicated leaf receives the explicit runtime context, not a fabricated
Stdout token. `Stdout.write` requires the separate granted token and rejects a
token from another context. Ordinary user code cannot construct that token.

Typed numeric leaves preserve fixed-width wrapping, IEEE values, and exact
library error text. Integer division syntax remains governed by the frontend
nonzero proof; leaf errors such as invalid `math.clamp` bounds are terminal.

Cleanup evidence is executable: destruction rejects a nonempty value registry,
and launcher tests prove that a deliberately leaked owner overrides entry failure
with exit 72. An expected runtime failure counts in the exhaustive audit only when
its output/message match and exit 71 establishes successful checked destruction.
Resource finalizer instrumentation must be extended before move-only resource
families can use this gate.
