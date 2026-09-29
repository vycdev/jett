# Secret values in trace and breakpoint output

Decision: trace and one-line breakpoint snapshots render each `secret[T]`
value as the fixed marker `[redacted]`. The rule applies through aliases and
refinements and recursively inside collections, sums, structs, enums, machines,
and reflected construction builders. Public fields retain their normal debug
representation. The marker is independent of the secret payload, including
its pending depth; a pending wrapper outside a secret retains its ordinary
debug representation. Formatting never consumes or declassifies a value.

This decision does not relax the interactive debugger's metadata-only
inspection policy for secret-bearing values, nor the print/serialization
boundaries. Runtime diagnostics that render typed values must use the same
redaction when native reflection reports them.

## Implementation and evidence

Interpreter debug formatting uses binding and reflected field types rather than
untyped `Value` formatting. Inferred loop, match, and handler bindings retain
metadata; concrete generic invocations supply types for ambiguous source spans.
Native debug layouts use a zero-payload Redacted node and do not dereference the
secret payload. Builder metadata carries the same layouts. Checked aliases
survive in binding labels separately from runtime canonical type identity.

`tests/native/debug_secret_values.jett` compares interpreter/native output for
scalar, pending, alias/refinement, generic, collection, recursive aggregate,
closure, inferred-binding, pipeline, and builder cases. Seven secret-bearing
pending reflected owner/builder failure fixtures compare error output. Runtime
tests prove redaction ignores invalid hidden handles and large pending depths,
does not confer equality, and does not mistake public marker text for a secret.
