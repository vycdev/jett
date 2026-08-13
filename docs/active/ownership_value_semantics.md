# Ownership and Value Semantics

Status: decisions in progress.

This record contains only decisions explicitly accepted for initial Jett
versions. Undecided value categories are not implied by these rules.

## Confirmed Decisions

### Numeric primitives and `bool`

All signed and unsigned integer types, floating-point types, and `bool` are
implicitly copyable. Using one in an assignment, argument, or return does not
invalidate the original binding.

### `nothing`

`nothing` is implicitly copyable. It carries no data or ownership.

### `string`

`string` is immutable and implicitly copyable. Reassigning one binding never
changes another binding copied from it:

```jett
string a = "hello"
string b = a
a = string.concat(a, "!")
# a is "hello!" and b is "hello"
```

The runtime may share immutable string storage. Reference counting or another
storage strategy is an implementation detail and must not change observable
value semantics.

### `bytes`

`bytes` is move-only. Reading without transfer requires `view`; independent
duplication requires explicit `clone`. This leaves room for efficient mutable
byte buffers without implicit aliasing or hidden copy-on-write behavior.

### Collections

`list`, `map`, and `set` are move-only. Reading without transfer requires
`view`; independent duplication requires explicit `clone`.

### Structs

All structs are move-only, regardless of whether their fields are copyable.
Independent duplication requires explicit `clone`.

### Enums

All enums are move-only, including enums whose variants have no payload fields.
Copyability is not inferred from the current variant shapes, so adding a payload
cannot silently change the ownership semantics of an existing enum.

### Capabilities

Capability values are move-only. Authority cannot be duplicated implicitly.

### Actor handles

Actor handles are move-only. Creating another sender requires explicit `clone`,
so duplicating access to an actor is visible in source.

### Task handles

Task handles are move-only. A task handle has one owner responsible for awaiting
or cancelling the task.

### Resource handles

Resource handles, including files and sockets, are move-only. A resource may
offer a resource-specific explicit duplication operation when that operation is
valid, but resource handles are never duplicated implicitly.

### Closure captures

Closures may capture only implicitly copyable values. Each captured value is
copied into the closure. Capturing a move-only value is a compile error; such a
value must instead be passed explicitly as a parameter.

Compiler enforcement and existing closure fixtures must be aligned with this
rule before it is considered implemented.

### Cross-context transfer

Passing a move-only value into a task or actor message transfers ownership; the
sender can no longer use it. Implicitly copyable values are copied instead and
remain usable by the sender. Views cannot be sent to actors. A scoped task may
borrow a view only until that task is joined or cancelled.

### Local mutation

`mutable` permits explicit rebinding within the current scope; it does not create
mutable references. Functions transform caller-owned data by consuming it and
returning a replacement. Views are always read-only. The compiler may reuse
uniquely owned storage in place, but that optimization does not change these
observable semantics.

## Open Decisions

No unresolved items remain from this decision pass. Any newly identified
ownership question must first be checked against the established design and
architecture rules.
