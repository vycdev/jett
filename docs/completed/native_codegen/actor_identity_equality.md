# Native actor identity equality

The reference evaluator and native backend implement live actor identity
comparison. `==` and `!=` require the same exact checked nominal actor type.
Cloning preserves instance identity, while separate spawns remain unequal even
when their state matches. `!=` negates the identity comparison.

Both runtime IDs must belong to the current actor registry and have complete,
live backing records. Equality compares identity without reading actor state.
Enum payload traversal applies this rule through supported nested containers.
An explicit struct `Equatable.equals` method still controls struct comparison.

Each pending handle records its own depth and refers to the base actor instance.
`run` creates an alias; `join` reaches the same ready instance. Wrapping one
handle leaves ready aliases unchanged. Nested enum comparison checks identity
and pending depth. Direct comparison of pending actors retains the existing
unsupported binary-operation error.

## Verification

Local MSVC static-CRT verification on 2026-10-12 passes the complete runtime
(301), interpreter (564), and code generator (118) unit suites, plus three actor
object and nominal-type rejection tests. The actor native suite runs nine
executables per runtime profile after deleting their Source: seven original
and supplemental debug-policy cases, and a production-output case with both
Jett optimization settings. Debug and Release runtime profiles both pass.

The production case observes direct identity, nested list/optional enum
payloads, pending aliases and depths, state shared after joining, and explicit
struct equality that ignores its actor field. Runtime tests additionally cover
foreign and stale IDs, incomplete backing records, pending allocation failure,
and single state retirement.

The six saved fixture Sources remain unchanged. These supplemental `tests/native`
controls leave the fixed `tests/native_parity.json` inventory at 207 fixtures
and 182 object obligations. Full predecessor, GNU, and installed-distribution
acceptance remain part of the native codegen completion work. Escaping comptime
actor graph import is tracked separately.
