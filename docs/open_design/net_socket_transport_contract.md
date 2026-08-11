# Initial `net.socket` Transport Contract

Status: proposed for issue [#104](https://github.com/vycdev/jett/issues/104).
No socket runtime or public API should land until this contract and the opaque
resource representation it requires are accepted.

## Decision Summary

The first `net.socket` slice is portable TCP client and listener support:

- one validated endpoint model for ASCII hostnames and IPv4/IPv6 literals;
- owned, linear `TcpStream` and `TcpListener` handles;
- atomic `connect` and `listen`, plus `accept`, bounded reads, partial writes,
  directional shutdown, endpoint inspection, and explicit close;
- explicit per-operation deadlines, with structured-concurrency cancellation
  remaining distinct from socket failures;
- `Network.allow` as an exact, outbound-only destination restriction;
- public declarations in compiler-shipped `.jett` source over private trusted
  DNS, socket, timer, and event-loop kernels.

UDP, Unix-domain sockets, TLS, QUIC, multicast, socket options, descriptor
interop, and server-framework policy are deferred. The initial HTTP client
tracked by [#101](https://github.com/vycdev/jett/issues/101) may later use this
TCP boundary, but HTTP does not expose socket handles or socket errors.

## Why TCP First

TCP exercises the lifecycle and ownership questions that the runtime must solve
without also requiring a datagram truncation, peer-address, multicast, or
broadcast policy. A single stream/listener slice is enough to establish:

- name resolution and connection permission checks;
- owned OS-resource handles;
- readiness-based reads, writes, accepts, deadlines, and cancellation;
- partial progress, EOF, peer reset, and backpressure;
- deterministic fake-kernel and loopback testing.

Adding TCP and UDP together would make the first contract larger without
sharing a useful public I/O shape beyond endpoint validation.

## Public Semantic Model

The intended declaration order and signatures are shown below. They define the
public contract, not a promise that the current compiler can already declare an
opaque runtime-backed type with this exact source syntax.

```jett
namespace net.socket

export struct SocketEndpoint:
    host: string
    port: int64

export enum SocketDeadline:
    no_deadline
    after_ms(milliseconds: int64)

export enum ShutdownDirection:
    read
    write
    both

export enum SocketOperation:
    connect
    listen
    accept
    read
    write
    shutdown

export enum SocketError:
    invalid_endpoint(message: string)
    invalid_argument(operation: SocketOperation, message: string)
    permission_denied(operation: SocketOperation, message: string)
    resolution_failed(message: string)
    address_in_use(message: string)
    connection_refused(message: string)
    timeout(operation: SocketOperation)
    operation_in_progress(operation: SocketOperation)
    peer_reset(message: string)
    closed
    io(operation: SocketOperation, message: string)

export enum SocketRead:
    data(value: bytes)
    eof

# Opaque, owned, linear runtime-backed resource types.
export struct TcpStream
export struct TcpListener

export function endpoint(host: string, port: int64) returns result[SocketEndpoint, SocketError]
export function connect(view net: Network, view endpoint: SocketEndpoint, view deadline: SocketDeadline) returns result[TcpStream, SocketError]
export function listen(view net: Network, view endpoint: SocketEndpoint, backlog: int64) returns result[TcpListener, SocketError]
export function accept(view net: Network, view listener: TcpListener, view deadline: SocketDeadline) returns result[TcpStream, SocketError]
export function read(view net: Network, view stream: TcpStream, max_bytes: int64, view deadline: SocketDeadline) returns result[SocketRead, SocketError]
export function write(view net: Network, view stream: TcpStream, view data: bytes, view deadline: SocketDeadline) returns result[int64, SocketError]
export function shutdown(view net: Network, view stream: TcpStream, direction: ShutdownDirection) returns result[nothing, SocketError]
export function local_endpoint(view stream: TcpStream) returns result[SocketEndpoint, SocketError]
export function peer_endpoint(view stream: TcpStream) returns result[SocketEndpoint, SocketError]
export function close_stream(view net: Network, stream: TcpStream) returns nothing
export function close_listener(view net: Network, listener: TcpListener) returns nothing
```

`TcpStream` and `TcpListener` need an opaque source declaration mechanism before
implementation. Their fields and OS identifiers must not be constructible,
inspectable, serializable, clonable, or comparable by user code. Resolving that
representation is an implementation prerequisite, not permission to expose a
public Rust builtin surface: the public names and signatures still belong in
compiler-shipped `.jett` source.

The shown `SocketEndpoint` fields are its semantic data, but public code must
not be able to bypass `socket.endpoint` validation through a generated struct
constructor. The source representation must provide public read access with
namespace-private construction, or use an opaque value plus accessors. This is
also an implementation prerequisite rather than an invitation to weaken the
validation contract.

## Endpoint Rules

`SocketEndpoint` is a validated value produced by `socket.endpoint`:

- `host` is either an ASCII DNS hostname, an IPv4 literal, or an IPv6 literal;
- DNS names are case-insensitive and stored as lowercase ASCII;
- IP literals are stored in canonical text form; IPv6 brackets are URL syntax
  and are rejected rather than stored;
- empty names, whitespace, URI schemes, paths, user information, and embedded
  ports are rejected as `invalid_endpoint`;
- `port` is in `0..=65535`; `connect` rejects port `0`, while `listen` treats
  port `0` as an explicit request for a host-selected ephemeral port;
- `listen` requires `backlog > 0`; other values are `invalid_argument` for the
  listen operation;
- `local_endpoint` returns the selected numeric address and actual port, so a
  caller that listens on port `0` can discover the bound endpoint;
- scoped IPv6 zone identifiers are deferred because their portable identity and
  permission spelling are not yet defined.

`connect` resolves a hostname inside the private runtime kernel and attempts the
returned addresses in runtime-defined order. The public error does not expose
platform error numbers or DNS-library details. `listen` accepts only numeric IP
literals in the first slice. Callers use `127.0.0.1` or `::1` for loopback and
`0.0.0.0` or `::` for wildcard binding; an empty host never means wildcard.

There is no separately exposed `bind` operation in the first slice. `listen`
atomically creates a socket, binds it, and starts listening so source code never
holds a bound-but-not-listening intermediate state. `SocketOperation.listen`
covers both the bind and listen system steps; the private kernel may keep those
steps separate internally.

## Capability Policy

Every operation that can wait for or move network data takes `view Network`.
The stream/listener handle identifies the resource; it does not grant ambient
permission to create or operate unrelated sockets.

`Network.allow(net, host)` has a deliberately narrow first-slice meaning:

1. `host` is one exact ASCII hostname or canonical numeric IP literal. Wildcards,
   suffix matches, CIDR ranges, and embedded ports are not accepted.
2. A restricted capability is outbound-only. `listen` fails with
   `permission_denied(SocketOperation.listen, ...)` before any OS bind.
3. `connect` checks a hostname before DNS. DNS is performed only for an allowed
   hostname, and only the addresses returned for that checked request may be
   attempted. This prevents resolving an unapproved name and avoids treating a
   later DNS result as new ambient authority.
4. A numeric destination must exactly match the allowed canonical IP literal.
5. A full, unnarrowed `Network` may connect or listen subject to host OS policy.
6. `accept` requires the same listener authority recorded by `listen`; the
   supplied capability must permit listener operations under that authority.
   A listener cannot be paired with an unrelated or outbound-only capability.

Port-scoped and listen-scoped capability narrowing may be added later with new,
explicit narrowing operations. They are not inferred from strings in this
slice.

## Lifecycle And Ownership

`TcpStream` and `TcpListener` are owned linear resources:

- `connect`, `listen`, and `accept` return one owned handle;
- the handles cannot be cloned;
- passing a handle without `view` transfers it, including into an actor message;
- I/O and endpoint inspection borrow a handle with `view` and never consume it;
- `close_stream` and `close_listener` consume the handle and are idempotent only
  in the ownership sense: a consumed handle cannot be named for a second call;
- dropping an owned handle at scope cleanup closes it as a last-resort resource
  release, while explicit close communicates the intended lifetime locally;
- closing a listener does not close streams previously returned by `accept`;
- `shutdown` changes the OS stream direction but does not consume or close the
  handle. A shut-down direction remains shut down.

The initial slice does not split a stream into independently owned read and
write halves. At most one socket operation may be pending on a handle at a
time. If another read, write, shutdown, or accept starts before the first
operation completes, it fails with `operation_in_progress` without disturbing
the pending operation. A future split operation needs its own design for
ownership, simultaneous read/write, shutdown, and reunification.

## I/O, Deadlines, And Failure Values

A public socket call has synchronous source semantics. There is no `async`
function color: a caller uses `run`, `join`, and `cancel` when concurrency is
wanted. Under an actor or task runtime, the private kernel submits readiness
work to the event loop and yields rather than blocking a scheduler thread.

`SocketDeadline.after_ms` is relative to the start of that operation and must be
strictly positive. Zero or a negative value is `invalid_argument` for the
requested operation. `no_deadline` is explicit at the call site; there is no
hidden default timeout. A timeout is reported as `SocketError.timeout` with the
operation that expired.

Cancellation follows Jett's existing capability-checkpoint rule. Cancelling a
task wakes a pending socket operation, unregisters its event-loop interest, and
causes the task to complete through the built-in `CancelledError` path observed
at `join`. Cancellation is not collapsed into `SocketError.timeout` and does
not implicitly close a borrowed stream or listener.

Read and write progress is explicit:

- `read` requires `max_bytes > 0`, returns at most that many bytes, and may
  return fewer as soon as data is available;
- `SocketRead.data` always contains at least one byte;
- an orderly peer shutdown is `SocketRead.eof`, never empty data and never an
  error;
- a reset is `peer_reset`; bytes delivered before a later reset remain valid;
- `write` returns the positive number of bytes accepted and may be smaller than
  the input under backpressure;
- writing empty bytes succeeds with `0`; a non-empty successful write never
  returns `0`;
- failures do not report uncommitted bytes as written. A caller that needs
  application-level retry identity must add it in its protocol.

`close_*` consumes the resource and returns `nothing`. Close-time platform
errors are not portable or recoverable after ownership is surrendered, so they
are not exposed. The runtime may record them in debug tracing, but source code
cannot branch on them.

## Runtime And Stdlib Boundary

Compiler-shipped `.jett` source owns:

- all public `net.socket` declarations and signatures;
- endpoint validation and canonicalization that can be expressed safely;
- future compositional helpers once their whole-operation deadline behavior is
  expressible without private public-name dispatch;
- mapping private kernel failures into the stable `SocketError` model.

Private trusted runtime kernels own only behavior that requires host access:

- DNS resolution;
- socket creation, bind/listen/connect/accept, shutdown, and close;
- readiness registration, timers, cancellation wakeups, and byte transfer;
- translation from platform errors into a small internal error vocabulary;
- opaque handle storage and authority provenance.

The compiler must not retain hardcoded knowledge of public `net.socket` names
or signatures. Interpreter, native, and future backends implement the same
private hook contract. Kernel hooks are unavailable to project/dependency code
and are not an alternate public API.

## Deterministic Verification

The first implementation should be staged and tested without ambient network
access:

1. Add source declarations, endpoint tests, signature-query coverage, and
   compile-fail tests for private hooks and invalid project namespace claims.
2. Add a deterministic fake socket kernel to the interpreter test harness. It
   scripts DNS answers, partial writes, EOF, reset, timeout, cancellation, and
   permission denial without opening a host socket.
3. Implement interpreter hooks and run the same source-level behavior fixtures
   against the fake kernel.
4. Add native/event-loop hooks per supported host and run focused loopback-only
   integration tests on IPv4 and IPv6 where the host reports IPv6 available.
5. Expose `net.http` over the socket boundary only after #101 settles HTTP,
   HTTPS, redirect, body-limit, and cancellation policy.

Required behavior coverage includes endpoint validation, connect port `0`,
listen port `0`, narrowed-capability DNS ordering, denied bind-before-syscall,
accept authority mismatch, operation-in-progress rejection, partial read/write,
EOF versus reset, directional shutdown, cancellation without implicit close,
explicit close, scope cleanup, and backend-equivalent error variants.

## Deferred Scope

The following require separate design pressure and do not silently inherit TCP
semantics:

- UDP and datagram truncation/source-address behavior;
- Unix-domain and other local IPC sockets;
- TLS identities, trust stores, ALPN, and certificate errors;
- QUIC and multiplexed streams;
- proxy discovery, Happy Eyeballs ordering guarantees, and DNS caching;
- keepalive, no-delay, buffer sizing, reuse flags, and other socket options;
- `write_all` and other multi-operation helpers until a monotonic
  whole-operation deadline can be represented in public source;
- split read/write ownership and concurrent access to one stream;
- raw descriptors, C interop, and adoption of externally created sockets;
- port-, CIDR-, wildcard-, and listener-scoped capability narrowing;
- HTTP servers, WebSockets, and application server frameworks.
