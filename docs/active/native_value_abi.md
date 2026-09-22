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
