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

## Linear bytes places (phase 3)

CopyValuePlan retains its copy-only public guard. MoveValuePlan additionally
proves definite availability over the MIR CFG, with intersection at joins and
loop backedges, ordered operand evaluation, and call-bounded view loans. It
rejects consuming a view, escaping a view into an owner, moving a borrowed
argument during later argument evaluation, and reading a moved place. It does
not yet accept aggregates, sums, resources, or hidden handler control flow.

Bytes use distinct context-associated handles backed by owned mutable Vec<u8>
storage, not UTF-8 strings and not reference-counted owners. Explicit clone
allocates independent storage. A local move clears its materialized source
slot and creates one owning expression slot. Zero is the drop flag for an
uninitialized/moved slot. Assignment evaluates the RHS before dropping the old
owner. Owned call arguments transfer only after every argument is evaluated;
temporary owners remain available to failure cleanup until that point. Views
borrow a local or an owning temporary through the call only. Callees own owned
parameters and never destroy view parameters. Owned returns detach their slot
before frame cleanup. Leaf byte kernels borrow their inputs for the call;
compiled stdlib bodies implement language-level consumption and control flow.

Bytes new/from_string/slice/concat, explicit clone, length and to_hex execute
natively. Concat creates real checked-capacity byte storage. No byte mutation
syntax is added. Byte temporaries add one structural slot per allocating call,
intrinsic, explicit clone, or local move; local views add none. Existing string
structural costs are unchanged. Cleanup accepts both owned families, including
terminal failure, and byte creation/destruction counters must balance in
addition to both registries being empty. Runtime tests separately prove byte
storage independence, exact destruction and byte-only leak detection. This is
not evidence for resource finalizers or full native parity.

## Result and optional ownership and handlers

Native sums have separate context-associated, move-only handles. Their internal
records contain a genuine discriminant (0: fail/none, 1: ok/some), a 64-bit
payload carrier, and an owned-payload flag. Fixed-width scalars preserve bits;
float32/64 are bitcast, never numerically cast into the carrier. Nothing/none
have no owned payload. Owning payloads transfer into the record only on
successful construction. Sum destruction recursively drops the active payload
only; explicit clone recursively clones bytes/sums and retains copyable strings.
Taking a payload checks the expected tag, destroys the outer record, and
transfers the payload to a new initialized place without destroying it. A failed
tag check leaves the record available to terminal cleanup. Sum allocation and
destruction counters are checked independently from string and byte registries.

Statement-root result/optional handles now lower to SumTag, ordinary Branch,
SumTake, and explicit continuation blocks. Handler default yields to that
continuation; return exits the enclosing function; break/continue retain their
enclosing loop edges. Failure branches without an error binding drop the sum
normally. Terminal runtime failure still bypasses these language-level branches.
The move fixed point covers the extracted edges and transferred payload places.
Nested payloads work; handlers nested inside arbitrary expression operands,
refinement handlers, and match/aggregate lowering remain separate prerequisites
and are still rejected by native validation rather than executed eagerly.

All original bytes_operations fixture functions execute in a supplemental
native main, including binary conversion errors and borrowed observer aliases.
int64/uint64/float64 string parsing produces ordinary tagged result data; the
three original math MIN runtime contracts now execute using that path. Full
fixture denominators and unsupported families remain unchanged.

Cleanup-operation failure is recorded separately from the first terminal error.
Even after all registries empty, a double drop forces context destruction to
fail and launcher exit 72; the original runtime failure diagnostic is retained.
Named argument tests distinguish lexical evaluation from parameter order and
prove nested views end before a later legitimate move, while same-call active
loans reject conflicting moves.

## Initial list layouts and compiled iteration

Native list handles own a homogeneous vector of payload carriers plus the
checked element ownership category. List construction owns its partially built
list before evaluating any elements. Append mutates uniquely owned storage,
transferring the element and returning the same owning list on success; failed
append leaves both input owners with the caller. Get/first/last return genuine
optional data and clone the selected element. Explicit list clone recursively
clones owned elements. Lists have separate creation/destruction counters and
leak checks, including lists with no owning elements. Scalar, string, bytes,
sum and nested-list elements are supported; named structs/maps/sets are not.

New/length/append/get are typed native leaves. First/last/is_empty/reverse/repeat
and math.sum run their actual compiled Jett bodies. list.sum[int64] is a typed
wrapping numeric leaf. No name-dispatch or loop interpreter is introduced.

Before native validation, a MIR sequence pass materializes supported iterables
in a preheader exactly once, creates a cursor and length, and replaces ForEach
with ordinary Branch, SequenceGet, increment, and existing exit/backedge CFG.
Consuming iteration transfers scalar/string and move-only elements from initialized
element places. Each list slot has an independent initialized flag; a take clears
only that slot after bounds/initialization checks. Partial-list destruction visits
only remaining initialized slots, including early return/break and terminal failure.
Borrowed iteration currently yields copyable scalar/string elements only; projected
move-only views remain rejected until projection lifetimes are implemented. A borrowed container has an explicit token from preheader to
exit; ownership analysis unions active tokens across edges and preserves nested
loans of the same owner. Moving or overwriting an actively iterated owner is
rejected; moving after the exit is valid. Return/break/continue use ordinary
frame cleanup and loop exit edges. Exact effects prove iterable evaluation once.

Sequence preheaders are selected from CFG predecessors reachable from function entry
without crossing the loop header, not from numeric block order. The lowering
requires a unique unconditional entry edge; ambiguous shapes remain rejected.
Regression coverage relocates the preheader after the loop, re-enters loops from
an outer loop, checks empty inputs, nested loans and handled loop exits, and
unpacks float32/int8/uint8 elements. Test-only allocation fault injection confirms
partial nested list clone releases its cloned prefix without consuming sources.

The typed int64 range leaf builds owned scalar list storage with checked capacity
and checked progression at signed boundaries. One/two-argument defaults are emitted
constants, after source arguments are evaluated in their checked order. Zero step
and oversized output are terminal failures (not ordinary result.fail), matching
interpreter diagnostics and unwinding live bytes/sum/list owners in callers.
