# Breakpoint Pause and Inspection Protocol

## Status

Decided for the first interactive debugger implementation. This document closes
the protocol-design scope tracked by
[#41](https://github.com/vycdev/jett/issues/41); implementation remains staged
because the native runtime, HIR, and MIR do not exist yet.

The current interpreter behavior—evaluating the optional condition and emitting
one snapshot of visible binding names and types—is the stage-zero compatibility
baseline. It is not the completed interactive behavior.

## Goals and Boundaries

A breakpoint session must let an agent inspect one stable program state without
turning debugging into a capability escape or a second compiler protocol. The
v1 contract therefore provides:

- deterministic pause, query, continue, disconnect, and failure states;
- one shared operation model used by the interpreter and future native runtime;
- structured TOON requests and responses with request correlation;
- binding metadata, selected values, expression evaluation, and stack/source
  context;
- the existing ASP diagnostic schema for compiler-produced failures.

This contract does not define source-level stepping, mutation, watchpoints,
remote debugging, MCP tools, HIR/MIR layout, or a general runtime reflection
API. Those can be added as new operations without changing the v1 lifecycle.

## Lifecycle

An interactive debug launch creates one `session_id` and its control endpoint
before user code starts. If endpoint setup or descriptor publication fails, the
launch fails before executing user code; a capability-exempt breakpoint must
never silently fall back to a remotely reachable listener.

The session has the following state machine:

```text
starting -> running -> pausing -> paused -> resuming -> running
                    \-> failed ---------------------> closed
running/paused ----- disconnect --------------------> closed
running ------------ program exit -----------------> closed
```

The rules are:

1. A conditional breakpoint evaluates its condition in normal program order.
   `false` produces no pause and no protocol event. A condition failure is an
   ordinary runtime/compiler failure, not a debugger query failure.
2. A hit requests a **process-scoped pause**. The interpreter stops immediately
   after evaluating the condition. A future concurrent runtime stops scheduling
   Jett tasks and waits for tasks already at scheduler safe points before
   exposing state. External OS work may finish, but its completion cannot run
   Jett callbacks while the process is paused.
3. Once quiescent, the runtime allocates a monotonically increasing `pause_id`
   within the session. Values and frame identifiers are valid only for that
   pause.
4. One authenticated controller uses two logical request lanes. The command
   lane accepts at most one request at a time and preserves request order. The
   event lane permits at most one outstanding `wait`, so a controller blocked
   waiting for an event can still send `continue` or `disconnect` on the
   command lane. Request IDs are unique across both lanes within the session.
   An exact duplicate returns the cached response; a reused ID with different
   content is a protocol error.
5. `continue` acknowledges the request, invalidates all pause-scoped handles,
   and then resumes scheduling. The next hit receives a new `pause_id`.
6. `disconnect` acknowledges, resumes if currently paused, invalidates the
   session token, removes the control descriptor, and closes the endpoint.
7. An unexpected controller disconnect starts a bounded grace period. The
   default policy is `resume`, so an abandoned agent cannot leave the target
   hung forever. Launchers may select `abort`, but not an unbounded pause.
8. A target failure returns a terminal `failed` event when possible, then closes
   the session. Closing the target process or endpoint also invalidates every
   outstanding request and handle.

Only one controller is allowed in v1. Multi-client observation would require an
explicit ordering and authorization design.

## Transport and Discovery

The protocol is a transport-independent operation layer with **loopback HTTP as
the first transport**. Stdio is not the initial transport because a Jett program
may legitimately own stdin and stdout; multiplexing debugger frames with program
I/O would make both interfaces ambiguous.

The debug launcher:

- binds an ephemeral port on an exact loopback address (`127.0.0.1` or `::1`),
  never a wildcard or non-loopback address;
- generates a fresh, high-entropy bearer token for each process;
- atomically writes a small TOON control descriptor containing the protocol
  version, endpoint, token, process ID, and canonical project root to a
  launcher-selected private file;
- creates that file without following symlinks and with owner-only permissions
  (mode `0600` on Unix and the equivalent owner ACL on Windows);
- rejects an existing descriptor path and deletes the descriptor on shutdown.

The token is sent in an authorization header, never in a URL, command-line
argument, diagnostic, trace row, or ordinary program log. The server compares
it without content-dependent early exit, rejects browser-origin requests, does
not enable CORS, validates the HTTP `Host` as loopback, limits request sizes,
and accepts only TOON operation bodies. Authentication failure reveals no
session metadata.

The endpoint exposes compiler-owned breakpoint operations only. The
`breakpoint` keyword's capability exemption authorizes this private control
plane; it does not create a `Network` value, authorize arbitrary sockets, or
make networking available to user code.

A later Unix-domain-socket, named-pipe, or MCP adapter may wrap the same
operation layer. It must preserve the envelope, authorization, lifecycle, and
path rules rather than inventing debugger semantics in the transport.

## Source and Path Authorization

A session is scoped to the exact compilation manifest used for the running
program. Locations use normalized project-relative display paths plus stable
source IDs. Dependency and compiler-shipped stdlib sources are identified as
such and remain read-only.

Debugger operations never accept an arbitrary filesystem path. Source context
comes from the compiler's loaded source map, not a fresh filesystem read, and a
requested source ID must belong to the session manifest. Absolute paths,
`..` traversal, symlink escapes, files added after launch, and paths outside the
project/dependency/stdlib manifest are rejected. This keeps the control plane
from becoming a capability-free file reader.

## Request and Response Envelope

Every operation is an HTTP request carrying a TOON document. Paused-state
commands identify the active pause:

```toon
protocol: jett.breakpoint.v1
session_id: 7f0c...
pause_id: 3
request_id: 8
operation: bindings
arguments:
  frame_id: frame-0
```

Every response repeats `protocol`, `session_id`, and `request_id`, applies the
`pause_id` presence rules below, and has exactly one of `result` or `failure`:

```toon
protocol: jett.breakpoint.v1
session_id: 7f0c...
pause_id: 3
request_id: 8
status: ok
result:
  bindings[3]{name,type,availability,secret}:
    order,Order,consumed,false
    validated,ValidatedOrder,owned,false
    api_key,secret[string],owned,true
```

```toon
protocol: jett.breakpoint.v1
session_id: 7f0c...
pause_id: 3
request_id: 9
status: error
failure:
  code: BP1004
  kind: unavailable_binding
  message: binding `order` was consumed before this breakpoint
```

`status` is `ok` or `error`. Protocol/runtime failures use stable `BP` codes and
one of these v1 kinds: `invalid_request`, `unauthorized`, `stale_pause`,
`unknown_frame`, `unknown_binding`, `unavailable_binding`, `forbidden_query`,
`invalid_expression`, `evaluation_limit`, `target_failed`, or `internal`.

If parsing or type-checking an `evaluate` expression produces compiler
`Diagnostic` values, `failure.kind` is `invalid_expression` and the response
embeds the shared ASP diagnostic collection owned by
[#35](https://github.com/vycdev/jett/issues/35). The breakpoint protocol does
not redefine diagnostic ranges, labels, scope, constraints, or suggested fixes.
Operational failures with no compiler diagnostic use only the `failure` object.

`pause_id` has state-dependent presence rather than being a universal envelope
field:

- `bindings`, `value`, `evaluate`, `stack`, and `continue` require the current
  `pause_id`;
- `wait` always omits `pause_id` in its request. Its `paused` and `continued`
  event responses include the pause they describe; terminal events include it
  only when they close an active pause;
- `disconnect` requires the current `pause_id` when paused and omits it when
  running. Its acknowledgement mirrors that presence.

A missing required pause, a pause supplied where it must be absent, or a pause
that is no longer current is `invalid_request` or `stale_pause` as applicable.
This lets a controller close a running session without inventing a pause while
still protecting commands that act on inspected state.

## Operations

V1 defines seven operations:

| Operation | State | Result |
|---|---|---|
| `wait` | running or paused | The next queued `paused`, `continued`, `failed`, or `exited` event. It omits `pause_id` in the request and may long-poll on the event lane. |
| `bindings` | paused | Binding names, declared types, availability, and secret flags for one frame; values are not included. |
| `value` | paused | A bounded structured rendering of one available, non-secret binding. |
| `evaluate` | paused | The type and bounded structured value of one non-destructive expression. |
| `stack` | paused | Ordered frames with function/namespace, source ID, path, and range. |
| `continue` | paused | An acknowledgement followed by resume. |
| `disconnect` | running or paused | An acknowledgement followed by resume-if-needed and session closure. |

Lifecycle events are queued once in order for the controller. A `wait` drains
the next event whether it was already queued or arrives while polling. The
controller may keep one `wait` outstanding while issuing a command; for
example, a paused controller can long-poll on the event lane, send `continue`
on the command lane, receive the command acknowledgement, and then receive the
corresponding `continued` event from `wait`. A second simultaneous `wait` is an
`invalid_request`. Command requests remain serialized, so this exception does
not introduce competing mutations or ambiguous command order.

`wait` returns the pause summary that agents need before choosing a query:

```toon
protocol: jett.breakpoint.v1
session_id: 7f0c...
pause_id: 3
request_id: 1
status: ok
result:
  event: paused
  reason: conditional_breakpoint
  function: process_order
  frame_id: frame-0
  source_id: project:src/orders.jett
  path: src/orders.jett
  line: 6
  column: 5
  bindings[3]{name,type,availability,secret}:
    order,Order,consumed,false
    validated,ValidatedOrder,owned,false
    api_key,secret[string],owned,true
```

Responses apply deterministic depth, element-count, byte, and evaluation-step
limits. Truncation is explicit (`truncated: true` with the applicable limit),
never silent. Map/set renderings use the language's canonical deterministic
debug order.

## Inspection and Evaluation Policy

Inspection cannot weaken the source language:

- `owned` and `view` bindings may be inspected; `consumed` and `uninitialized`
  bindings expose metadata only.
- Capability values expose their declared type and availability only. OS
  handles, permission internals, and resource contents are never rendered.
- A `secret[T]`, or any aggregate whose checked type contains a secret, exposes
  type/availability metadata only. `value` returns `forbidden_query` rather
  than length, fields, or a partial rendering.
- Debug expressions cannot use `declassify`, perform assignment, move or consume
  values, invoke capability operations, spawn work, send actor messages, run
  FFI, or perform I/O. Capability exemption applies to the control plane, not
  evaluated source.
- Expressions are checked in the selected lexical frame with implicit `view`
  semantics. Calls are limited to checked pure functions and safe secret
  operations such as `secret.redact`; results containing secrets remain hidden.
- Evaluation runs against an immutable snapshot in a bounded scratch arena.
  Allocations and temporary values are discarded, no writes are committed, and
  a step/time/size limit produces `evaluation_limit`. A future native target
  uses a debugger expression interpreter over checked metadata rather than
  executing arbitrary target-machine calls.

These rules preserve the existing design promise that breakpoint evaluation is
non-destructive while making its security boundary explicit.

## Complete Example

Given a conditional source breakpoint:

```jett
function process_batch(view orders: list[Order]) returns nothing:
    for order in view orders:
        breakpoint order.total > 1000.0
        process_order(view order)
```

A `false` condition emits nothing. On a true condition, `wait` returns a pause
summary for this frame:

```toon
protocol: jett.breakpoint.v1
session_id: 7f0c...
pause_id: 3
request_id: 1
status: ok
result:
  event: paused
  reason: conditional_breakpoint
  function: process_batch
  frame_id: frame-0
  source_id: project:src/orders.jett
  path: src/orders.jett
  line: 3
  column: 9
  bindings[2]{name,type,availability,secret}:
    orders,list[Order],view,false
    order,Order,view,false
```

The controller can inspect one binding:

```toon
protocol: jett.breakpoint.v1
session_id: 7f0c...
pause_id: 3
request_id: 2
operation: value
arguments:
  frame_id: frame-0
  name: order
```

```toon
protocol: jett.breakpoint.v1
session_id: 7f0c...
pause_id: 3
request_id: 2
status: ok
result:
  type: Order
  value:
    total: 1200.0
  truncated: false
```

It can then evaluate a non-destructive expression:

```toon
protocol: jett.breakpoint.v1
session_id: 7f0c...
pause_id: 3
request_id: 3
operation: evaluate
arguments:
  frame_id: frame-0
  expression: order.total > 1500.0
```

```toon
protocol: jett.breakpoint.v1
session_id: 7f0c...
pause_id: 3
request_id: 3
status: ok
result:
  type: bool
  value: false
  truncated: false
```

A forbidden secret query fails without exposing the value:

```toon
protocol: jett.breakpoint.v1
session_id: 7f0c...
pause_id: 3
request_id: 4
status: error
failure:
  code: BP1007
  kind: forbidden_query
  message: secret-bearing values cannot be rendered by breakpoint inspection
```

Finally, the controller resumes execution:

```toon
protocol: jett.breakpoint.v1
session_id: 7f0c...
pause_id: 3
request_id: 5
operation: continue
arguments: {}
```

The successful response contains `result.event: continued`; all handles from
`pause_id: 3` are then stale.

## Implementation Stages

Each stage is independently testable and must preserve earlier behavior:

1. **Protocol model:** add typed request/response, lifecycle, validation,
   authentication, source-manifest, and deterministic TOON renderer tests in a
   compiler-owned debug module. No network listener is needed for this stage.
2. **Interpreter pause:** add loopback HTTP and private descriptor handling to
   the CLI/driver, then connect the current tree-walking interpreter to `wait`,
   `bindings`, `value`, bounded `evaluate`, `continue`, and `disconnect`.
   Fixture tests cover bare/conditional hits, false conditions, stale handles,
   malformed/unauthorized requests, running and paused disconnect envelopes,
   an outstanding `wait` completed by a concurrent `continue`, rejection of a
   second outstanding `wait`, disconnect policy, secrets, and release stripping.
3. **Interpreter stack/source context:** expose stable frame/source IDs from the
   interpreter and source map, with manifest/path escape tests. This may follow
   stage 2 if only `frame-0` is initially available.
4. **Native lowering:** after #20 and #22 establish HIR/MIR, lower breakpoint
   safe points and checked scope metadata. The future runtime quiesces Jett
   tasks and adapts native values to the same operation model; it does not own a
   different wire schema or expression policy.
5. **Optional adapters:** local sockets, named pipes, or MCP may wrap the shared
   operations only after their transport-specific authorization is specified.

A stage is complete only when debug and release behavior, lifecycle cleanup,
and the security rejection cases are covered. A snapshot-only debug line is not
an implementation of the interactive protocol.
