# Breakpoint bindings across function calls

The interpreter's one-line `breakpoint` snapshot currently walks every active
scope in `Interpreter::hit_breakpoint`. At a breakpoint in a callee, this can
include caller locals that the callee cannot name in Jett source. A native
breakpoint uses the checked HIR binding snapshot for its current function, so
it reports only the callee's visible bindings. The difference is reproducible
by calling a function containing `breakpoint true` after declaring a local in
`main`.

The language and protocol need to decide whether a breakpoint inspects only
the current lexical frame, or exposes a call-stack view with separately named
frames. Flattening all frames into one binding map loses shadowed caller values
and gives caller locals the appearance of being in scope in the callee. Until
that policy is decided, native code keeps the checked HIR snapshot; it does
not fabricate access to caller frames. A differential fixture should pin the
chosen behavior in both interpreter and native execution.
