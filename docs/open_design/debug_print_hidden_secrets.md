# Secrets hidden from a debug print's static type

Direct secret arguments to `print` and `println` are compile-time errors. A
secret-bearing record erased to an interface can nevertheless pass the current
frontend output checks. The interpreter formats its raw `Value` and exposes the
field. Reflected construction builders can likewise carry values that their
static `TypeConstruction` type does not describe.

The native aggregate print implementation reuses the typed debug formatter,
which conservatively redacts secret fields. This creates an explicit remaining
parity gap with the interpreter; matching public-value printing does not close it.

The prior redaction decision explicitly covered trace and breakpoint output.
The requested follow-up decision is whether to extend that behavior to hidden
secret fields in otherwise accepted debug print values, or to reject printing
values whose hidden payload may contain secrets. Direct secret arguments remain
compile-time errors in either case, and release builds continue to reject both
debug print functions.

Redaction would use retained concrete identities and builder field metadata,
including nested containers, captures, pending values, and interface payloads.
Rejection needs a defined static rule for interfaces and builders; rejecting
every such value also rejects public instances that currently print successfully.
Do not weaken native redaction to reproduce the interpreter's disclosure.

Pending-sum extraction errors are another raw-value observation boundary. The
reference `handle` diagnostic uses raw `Value` display, while the native pending
check uses the checked layout and preserves redaction. Public pending values
have differential coverage; hidden secret payloads need the same explicit
observation-policy decision before their parity can be claimed.

After the decision, compare source checking and native/interpreter output for
public and secret-bearing erased records, generic fields, incomplete builders,
nested containers, and pending payloads. Public text equal to `[redacted]` must
remain ordinary public text.
