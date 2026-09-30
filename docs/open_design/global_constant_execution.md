# Global constant execution

Global constants are permitted by the design and frontend, subject to declaration
order and namespace isolation. Neither execution handoff implements them today.
A same-namespace `int64 answer = 42` followed by an entry point reading `answer`
passes checking but fails reference execution with `undefined variable 'answer'`.
The interpreter registers declarations without initializing top-level variables,
and HIR accepts identifier values only for locals or checked functions.

Implementing execution requires a common contract for both backends. Ordinary
pure initializers must not become required compile-time evaluation merely to
make native constant emission easier: only explicit `comptime` selects that
requirement, and optimization must not change source validity. Runtime
initialization, if selected, must run in declaration order before entry without
module-import effects or runtime capabilities, with failures and cleanup defined.

The unresolved ownership rule is whether global constants support only
implicitly copyable values, or also move-only immutable data through explicit
`view`/`clone`. Do not implicitly clone a move-only global on every read, silently
consume shared storage, or introduce mutable references. Structured values need
a defined owner lifetime and cleanup; opaque resources, actors, pending tasks,
and erased payloads additionally need scrutiny for hidden state or authority.
Callable constants must retain declaration context and their permitted captures.

No option is selected by this note. Native parity cannot be claimed for global
reads based on successful checking alone.
