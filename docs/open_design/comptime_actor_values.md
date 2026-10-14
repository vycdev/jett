# Actor handles escaping explicit comptime evaluation

Before graph integration, a closed `comptime spawn Counter()` passed checking
and evaluation but saved only an actor ID. Its interpreter instance and state
were discarded. Reference execution failed on the first message with
`unknown actor instance #0`, and native materialization could not reconstruct
the actor from that ID. These are historical pre-graph observations.

Pure code can use a capability-free actor entirely during evaluation and return
an ordinary value. At that checkpoint, the unresolved boundary was a handle
escaping in the result, including inside an aggregate, pending value,
interface, or admitted capture. The selected contract below preserves the
evaluated graph without expanding Source admission.

The alternatives considered were:

- Reject escaping actor handles during compilation, consistently in reference
  execution and native builds, while permitting internal actor use whose final
  result contains no actor handles.
- Preserve and materialize the evaluated actor graph. This needs defined
  identity, shared-reference, state, capture, and repeated-evaluation semantics;
  copying an integer ID or rerunning the original spawn is insufficient.

Do not silently execute the original comptime expression at runtime. The chosen
contract must cover direct and nested escaping values and an actor used entirely
inside an expression that returns a scalar.

`tests/native/comptime_actor_computation.jett` verifies the independent supported
case: a capability-free actor is spawned, updated, and queried entirely inside
a pure function. Its scalar result agrees between ordinary native execution
and explicit comptime materialization. This does not decide escaping handles.

## Selected graph contract and current bounded acceptance

The observations and alternatives above record the pre-graph implementation.
The selected direction preserves pure closed results as immutable frozen actor
graph templates, taken at successful required-expression completion. Every
runtime evaluation imports a fresh graph, including repeated calls and loop
activations. Bound aliases share the imported instances; a graph-wide memo
preserves sharing and keeps distinct seed actors distinct.

Import restores evaluated state and admitted typed captures without replaying
initializers, handlers, the original expression, source predicates or compiler
observations. Exact actor, interface, refinement and callback identities and
private checked required-value joins remain intact. It creates no global mutable
actor state and clones no runtime capability authority or Resource custody.
Unpublished partial imports retire completely before publication. The scalar
`tests/native/comptime_actor_computation.jett` control remains independent.

This choice preserves current Source admission. Actor closure capture remains
move-only and rejected with E0402; admitted primitive captures retain their
existing capture/context witness. Reflected callable transport preserves its
checked plan and authenticated source. No witness reset or new Actor-capture
resealing policy is introduced. Future admission of such captures would require
a separate checked transformation design before implementation.

The contract is integrated for the current sequential evaluator. Current local
MSVC acceptance passes 32 Source-deleted graph executions across the normal
Debug/Release runtime profiles with unchanged compiler and launcher guards.
The receipt is `target/native/actor-graph-native-acceptance-v6/after.json`, SHA256
`ecbba9f7539f250d8c48be4ee9ef9256e918bf95ab2d1731c39ee10460366d76`.
These bounded results do not establish whole-language native parity. Wider
admitted graph compositions, complete workspace and exact-revision supported-host
CI, GNU and installed-distribution acceptance retain their separate obligations.
Future asynchronous mailbox/task freezing remains a separate design. See
[the graph integration note](../active/native_comptime_actor_graph.md).
