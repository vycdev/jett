# Secret values in trace and breakpoint output

The debugger inspection contract says secret-bearing values expose metadata but
not values. The current interpreter's ordinary `Value` formatting is also used
by `trace` and can print the underlying value of `secret[T]`, including when it
is inserted into a `TypeConstruction` builder. The native debug layout does not
support `secret[T]`, so mirroring the interpreter here would create a new output
path for secret data.

Decide whether trace and breakpoint should reject secret-bearing values or
render a fixed redacted marker, and whether the rule applies recursively to
builders and aggregates. Then align interpreter, checker, native codegen, and
debugger inspection. Until that decision, native builder tracing only formats
fields with supported non-secret debug layouts.
