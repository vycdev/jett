# Breakpoint bindings across function calls

Decision: the one-line debug snapshot contains only bindings visible in the
current lexical function frame, including its parameters, captured bindings,
and active nested scopes. Source-level shadowing remains prohibited. Caller locals
belong to separate frames and are not flattened into this snapshot. The
separate interactive debugger protocol may expose explicitly identified stack
frames; that does not change the one-line snapshot's lexical scope.

Implementation must use the same lexical scope boundary as ordinary name
lookup in the interpreter and the checked HIR binding snapshot in native code.
Differential tests cover direct and indirect nested calls, nested-scope exit,
same-named bindings in separate functions, returning to a caller, and captured
closure bindings.

## Original gap

The interpreter's one-line `breakpoint` snapshot currently walks every active
scope in `Interpreter::hit_breakpoint`. At a breakpoint in a callee, this can
include caller locals that the callee cannot name in Jett source. A native
breakpoint uses the checked HIR binding snapshot for its current function, so
it reports only the callee's visible bindings. The difference is reproducible
by calling a function containing `breakpoint true` after declaring a local in
`main`.

The original language and protocol question was whether a breakpoint inspects only
the current lexical frame, or exposes a call-stack view with separately named
frames. Flattening all frames into one binding map loses shadowed caller values
and gives caller locals the appearance of being in scope in the callee. The
decision above preserves the checked HIR snapshot and brings interpreter
snapshots into agreement. `tests/native/breakpoint_lexical_frames.jett` pins
direct and indirect calls, captures, nested scopes, and return to the caller.
