# Actor handles escaping explicit comptime evaluation

A closed `comptime spawn Counter()` currently passes checking and evaluation.
The saved value contains an actor ID from the evaluation interpreter, but its
actor instance and state are discarded. Reference execution then fails on the
first message with `unknown actor instance #0`; native materialization cannot
reconstruct an actor from that ID either.

Pure code can use a capability-free actor entirely during evaluation and return
an ordinary value. The unresolved boundary is a handle escaping in the result,
including inside an aggregate, pending value, interface, or closure capture.

Two possible contracts are:

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
