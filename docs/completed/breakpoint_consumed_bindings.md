# Consumed bindings in breakpoint snapshots

Decision: omit bindings that have already been consumed by a move. This applies
to the current function's visible bindings, including captures, and avoids
reading values whose ownership has transferred elsewhere. A mutable binding
becomes observable again after assignment initializes it. Pending values are
still observable while owned; running or joining a task follows ordinary
consumption rules. Public trace remains a checked read of its explicitly named
binding.

The compiler records binding availability at each breakpoint and supplies the
same exclusions to HIR and the interpreter, including comptime and test runners.
Transfers at binding and destructuring sites affect observation without widening
source ownership permissions. Branch joins omit bindings unavailable on any
continuing path. Mutable reassignment restores observation. Copyable aliases
use checked expression types, and actor handlers establish a separate lexical
frame for state, capability, message, and local bindings.

`tests/native/breakpoint_consumed_bindings.jett` compares linked native and
interpreter output for transfers, calls, branch joins, loops, handled failures,
reassignment, copyable aliases, and actor handlers. Existing actor observation
expectations omit a handle already transferred into a list.
