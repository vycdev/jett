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
Clock authority is likewise a distinct context-bound token. The native entry
grants it only for checked Clock parameters; `Clock.__now` validates the token
and samples the wall clock through the same pre-epoch flooring and int64
millisecond-range kernel as the interpreter. The native launcher can opt into
a per-context scripted provider through `JETT_NATIVE_TEST_CLOCK_SCRIPT_V1` for
deterministic parity runs. An empty script means an exhausted provider; absent
configuration keeps the production wall clock. Native Clock reads match the
interpreter's sample conversion, unavailability, and exhaustion failures.
Checking for leftover scripted samples after a successful run remains pending.
Random authority is a separate context-bound token. Its provider is seeded once
from OS entropy when the native entry grants Random, or configured before entry
from `JETT_NATIVE_TEST_RANDOM_SCRIPT_V1` for deterministic parity runs. The
bounded-integer, unit-float, and boolean leaves share their provider logic with
the interpreter, including rejection sampling and invalid/exhausted script
failures. Collection algorithms remain in `stdlib/random.jett`. Exact
consumption of a successful native test script remains pending.
Environment authority is another context-bound token. The shared runtime
captures immutable user arguments and environment entries before entering
`main`; a test-only `JETT_NATIVE_TEST_ENVIRONMENT_SNAPSHOT_V1` channel can
replace that snapshot without changing the host process. `Environment.__args`
returns a fresh owned string list. `Environment.__get` returns an owned
`result[optional[string], string]`, preserving missing, invalid-name, and
invalid-Unicode outcomes. The two native Environment fixtures pass, while
platform-raw capture details remain to be audited against the design note.

Generated functions and all source control flow remain Cranelift machine code.
Copyable string arguments are borrowed by the native call ABI; callee owning
locals retain them. String expression results are owned. Results transfer one
reference to the caller. Assignment evaluates its RHS before releasing its old
slot. Full-expression temporaries have zero-initialized frame slots; all are
released at expression completion and on terminal failure. MIR liveness and
initialization analysis defines local cleanup points, including loop backedges
and early exits. Zero slots permit conditional initialization without releasing
an uninitialized value. The initial string slice did not establish move-only ownership. Later sections
describe bytes, sums, lists and user structs; resource ownership remains unproven.

Native string search shares one linear grapheme-boundary scanner with split.
`string.index_of` returns an optional int64 handle with a non-owning index
payload, including index zero for an empty needle. `string.count` returns the
number of non-overlapping matches and zero for an empty needle. The scanner
rejects byte matches that begin or end inside a grapheme, matching the
interpreter's string contract.

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
Checked `float64.from_int64` and `int64.from_float64` return ordinary result
values. They accept only exactly representable inputs, including the signed
minimum boundary, and return the interpreter's error strings for precision
loss, fractional values, non-finite values, or overflow.

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

Encoding leaves borrow their byte or string input and return an owned string
or an owned `result` with an owned byte/string payload. Native and interpreter
call the same base64, strict hex, URL, and form kernels, including validation
precedence and UTF-8 errors. Failed sum allocation drops the newly created
payload before propagating terminal failure.

CSV leaves parse strict records into nested owned string lists or header maps,
and stringify checked nested string lists. The interpreter and native runtime
share the parser, quoting, and header-validation rules. Native construction
releases partial rows, fields, maps, and result payloads if an allocation
boundary fails; the runtime fault-injection test checks each boundary.

Secret and refinement wrappers retain their underlying native representation.
The checked base-to-secret lift is recorded in expression types before MIR
lowering, so local storage, calls, and aggregate fields agree on the wrapper.
Native redaction returns the fixed `***` string after evaluating its operand.
Native secret comparison accepts the checked string and bytes payload shapes,
returns false on a public length mismatch, and uses constant-time byte
comparison for equal lengths. Wrapper-aware ownership retains and releases
strings and transfers or clones owned bytes and aggregates as their base types.
Explicit `declassify` transfers the same underlying value after the verifier
checks its source `secret[T]` and destination `T`. Private crypto leaves borrow
checked bytes and return owned digests; the interpreter and native runtime use
the same SHA-256, SHA-512, MD5, and HMAC-SHA-256 kernels behind public `.jett`
wrappers. The HMAC result retains its secret type in MIR and native ownership.

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
Nested payloads work. Result/optional handlers inside direct-call arguments
also lower through `view` wrappers; preceding owned arguments are staged in
checked evaluation order before the handler's CFG branch. A native differential
fixture checks present/default branches and named-argument side-effect order.
Handlers nested inside other expression operands, refinement handlers, and
match/aggregate lowering remain separate prerequisites and are still rejected
by native validation rather than executed eagerly.

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

New/length/append/get and primitive sort are typed native leaves. Sort reorders
the uniquely owned list in place, comparing signed and unsigned widths,
floating-point values, bools, and owned strings with interpreter semantics.
First/last/is_empty/reverse/repeat and math.sum run their actual compiled Jett
bodies. list.sum[int64] is a typed wrapping numeric leaf. Contextual conversion
of an empty `list[never]` to an owned element layout remains guarded. No
name-dispatch or loop interpreter is introduced.

Before native validation, a MIR sequence pass materializes supported iterables
in a preheader exactly once, creates a cursor and length, and replaces ForEach
with ordinary Branch, SequenceGet, increment, and existing exit/backedge CFG.
Direct string iteration uses typed scalar-count and scalar-at leaves, so each
loop item matches the interpreter's Unicode scalar `for` semantics. This is
distinct from `string.chars`, which returns grapheme clusters. The string
source remains owned by its frame slot while each yielded scalar is a new
owned string handle.
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

An unconstrained `list()` may have the uninhabited element type `never`. Its
native handle has no owned elements and supports empty-list length and emptiness
checks. The backend still rejects a value-producing `never` projection and a
conversion into another list element layout until it can update nested owner
metadata without leaking later inserted values.

String chars/words/lines/split are typed segmentation leaves returning genuinely
owned list[string] storage. Partial construction releases every inserted string
and the list on allocation failure. Join borrows its native list input for the
leaf; source-level consumption still moves the list into a tracked caller temporary,
which is destroyed at full-expression cleanup. Grapheme split requires both ends
of every delimiter match to be grapheme boundaries, including overlapping rejected
byte matches. Empty delimiters preserve endpoint empty strings. CR/LF/CRLF line
segmentation matches the interpreter. split_max, reverse and list.skip remain
compiled Jett control flow, not runtime implementations of their source bodies.

Primitive sets have distinct context-associated owner handles. Integer and bool
elements compare their checked scalar bits; string elements compare UTF-8 content
while retaining separate owner references. Add preserves insertion order, drops a
consumed duplicate string, and leaves both inputs owned by the caller if capacity
allocation fails. Remove drops only the removed owner. Clone retains each string
and rolls back partial construction on failure. Native sequence lowering uses
initialized element slots for consuming iteration, so an early exit drops only
the elements still in the set. Borrowed iteration clones strings without moving
the source set. The native/interpreter fixture covers these paths, including
duplicate content and cleanup at context destruction. Primitive-backed
refinement elements remain guarded.

Native maps use distinct ordered owner handles with exact key and value
ownership flags. Literal construction appends every source entry, including
duplicate keys; `map.insert` replaces the first matching value and preserves
the earlier key, while `map.remove` removes every matching entry. String keys
compare by content. `map.from_lists` zips to the shorter input, replaces
duplicate keys, and consumes both lists only after a successful result.
Lookup clones owned values into an optional sum. Explicit map clone recursively
clones owned entries and releases a partial prefix on failure. Consuming
iteration transfers each key and value separately; an interrupted iteration
drops only fields still owned by the map. Borrowed iteration clones supported
scalar and string fields. Native/interpreter execution fixtures and runtime
cleanup tests cover these paths. Primitive-backed refinement keys, projected
move-only views, and callback-bearing map helpers remain guarded.

The remaining string intrinsics now have typed native leaves. Replace reuses
the grapheme split matcher and checks output capacity before assembly; empty
needles preserve the interpreter's leading, between-grapheme and trailing
replacement positions. Slugify and first-grapheme case changes match the
interpreter's Unicode behavior. Existing run-pass string fixtures now emit
native objects, and dedicated linked fixtures compare observable output with
the interpreter.

## Native user struct ownership (phase 4)

User structs, including concrete generic instances, use distinct context-bound
move-only handles. A record has fixed indexed field slots, each with a payload
carrier, ownership category and initialized flag. The compiler validates field
count, exact field types, owner type and lexical evaluation permutation against
the checked type table. Native code owns the empty record before evaluating
fields, initializes slots in source order, and transfers each field owner only
after the typed initialization leaf succeeds. Partial-construction failure drops
only initialized fields and the still-owned expression temporaries.

Scalar field bits use the existing exact payload packing (including float32,
float64 and narrow integers). Field access implicitly borrows its parent.
Scalar fields copy and string fields retain; move-only fields remain bounded
views, not implicit deep copies or partial moves. A nested projection keeps its
root owner live. Call loans prevent moving that root during evaluation of a
later argument. Explicit clone recursively copies the selected owned field or
whole record, and leaves the source intact. A failed deep clone destroys its
initialized cloned prefix. Whole-record calls, returns, rebindings and cleanup
use the same materialized owner slots and transfer rules as bytes/sums/lists.
Struct owners also work inside sums and consuming lists. The runtime checks
record creation/destruction counts independently, even for empty records.

The checker records the exact Equatable.equals method identity for struct
comparison operators in per-body facts. HIR lowers equality to that direct
compiled call with its two view parameters; inequality negates its result.
Native primitive comparison rejects raw struct operands. No structural equality,
handle equality or method-name dispatcher is used.

Enums use the same owned record handle with a dense variant tag in slot zero.
Payloads occupy subsequent typed slots. Construction owns the partial record
before evaluating payloads; a matched arm transfers bound fields out, then
drops the remaining record. Clone and cleanup recursively handle owned
payloads, including nested enums. Switch validation rejects duplicate, invalid
and non-exhaustive arms. Native/interpreter differential fixtures cover unit,
scalar, string and recursive owned payloads, multiple bindings and an `other`
arm. Equality compares tags for unit enums; payload-enum equality remains
guarded until typed field comparison is implemented.
Tag comparison borrows unit-enum operands, including projected aggregate fields,
so it leaves their owners available after the expression.

Bitfield values use the same typed record storage for fields, including owned
payload fields. Native construction, field projection, clone and cleanup match
the interpreter in a dedicated fixture. `to_bytes` writes checked field widths,
network or native byte order, enum discriminants and trailing `list[uint8]`
payloads to owned bytes. `from_bytes` decodes checked widths, enum tags and
trailing payloads into owned records; invalid bytes return matching error strings.
Native/interpreter fixtures cover roundtrips and decode failures, while runtime
allocation-boundary tests cover partial-owner cleanup. Dynamic field-width
validation still requires bitfield-specific native lowering.

Machine values also use owned records: slot zero stores the dense state tag,
followed by that state's payload fields. Construction initializes an empty
record before evaluating payloads; a transition consumes its source and builds
the declared target state. State tests borrow the record. Reading a field of a
state-qualified machine projects the corresponding payload slot, and narrowing
a bare machine local checks its tag before projection. Clone and destruction
recursively handle owned payloads. The dedicated native/interpreter fixture
covers construction, transition, two- and three-state branches, payload reads,
and bare-machine narrowing; the runtime guard test rejects a mismatched tag
while preserving cleanup.

This is not full aggregate parity. Refinement-validating constructors, owned
escape of a move-only projected view (without clone), field-place assignment,
borrowed iteration yielding compound views, payload-enum equality, and bitfield
width validation remain unsupported. Composite explicit comptime constants still require baking. Source
validity and interpreter field-copy behavior do not authorize native implicit
copies of move-only fields; those unresolved ownership paths stay guarded.

Borrowed native list and map iteration over move-only elements materializes an
owned deep clone for each loop binding; the source collection retains its
owner through the loop. A switch over a borrowed enum likewise matches an
owned clone, preserving the caller's value and existing payload-transfer
semantics. An explicit `view` passed to an owned direct-call parameter is
cloned at that call boundary. MIR ownership analysis and temporary capacity
account for the clone; an ordinary move-only field projection into an owner
still requires an explicit source `clone`.

Borrowed sequence tokens now end on each CFG edge leaving the loop region,
including handler default edges that bypass the ordinary loop exit. Split edges
end only that loop token, preserving outer loans. Nested iterable element IDs
are checked before sequence preprocessing resolves them.

Evidence includes existing generic_struct and explicit_struct_equality fixture
bodies executed with supplemental mains, nested owners and early returns,
partial construction terminal cleanup (exit 71), runtime clone fault boundaries,
malformed MIR rejection, and one generated object executed with four distinct
process inputs through a test-only scalar input adapter. Allocation tests cover
returned leaf failures, not allocator abort, OS termination or resource finalizers.
