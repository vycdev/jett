# Secrets hidden from debug observations

## Decided: accepted debug Print/Println values use typed redaction

Non-release `print` and `println` recursively redact secrets hidden by otherwise
accepted interface, refinement, aggregate, or reflected builder values. This
retains the existing public-value admission policy. Rejecting every interface or
builder would remove accepted public programs without proving non-disclosure.

The canonical marker is `[redacted]`, shared with typed trace/reflection output
and the native debug schema. Rendering visits the secret qualifier before its
payload and inner pending wrappers. Public text equal to the marker remains
public. Concrete nominal/generic identities and stored builder field types
supply the observation graph; structural field similarity never supplies a
missing owner or field contract. An interpreter observation with missing
composite/field metadata produces a conservative redacted subtree instead of
exposing raw payloads. Such fallback is not evidence that checked metadata
propagation is complete. Native malformed schema/handle guards remain errors;
this fallback does not convert them into redacted successes.

Checked caller argument types are retained before argument evaluation, including
nested generic/scoped/reflected calls and ownership transfer. Each type remains
paired with its actual value. Values are evaluated once in source order; the
call emits one complete event only after every argument succeeds. `view`, owned
argument consumption, spacing, newline and diagnostic stream behavior remain
unchanged. Formatting invokes no getter, Displayable method or callback and does
not inspect closure captures or actor state.

Direct secret arguments remain compile-time errors, as do the existing Resource
root restrictions. Source release checking retains E0362 for both functions.
Debug/Release runtime archive profiles do not authorize Source release calls.
Native typed redaction and concrete schema checks remain independent defenses.

The historical reference interpreter rendered a raw secret field from a record
erased to an interface, while the native formatter already used typed redaction.
That disclosure is a defect observation, not a parity oracle. Compare exact
public and secret-bearing Source output through real reference execution and
Source-deleted linked executables; object emission alone does not close the gap.

## Pending-value terminal diagnostics

Print/Println rendering and terminal failure output are separate observation
paths. The direct-field diagnostic successor below applies the existing secret
non-disclosure contract to its demonstrated receiver failure. Other terminal
paths retain their existing native typed guards and need separate parity evidence.

Three original checked pending-sum probes demonstrated a reference disclosure:
`01_pending_secret_error.jett`, `02_pending_optional_secret_field.jett`, and
`03_pipeline_pending_secret_error.jett`. They reached the terminal failure after
only `before\n` stdout and no debug events, exposing the secret child through
raw `Value` display. Their original Source bytes remain unchanged.

Direct target types are now captured before evaluation, and pipeline steps carry
their full result type separately from the handler error-child type. Unsupported
sum receivers render from their original wrapped values before consumption,
preserving public pending depth and redacting only secret subtrees. Ready sums,
handler error typing, earlier target failures and once-only effects retain their
behavior; diagnostics neither join nor declassify values.

All 659 Comptime tests pass. Six permanent controls cover nine programs and pass
18 Source-deleted native executions against normal Debug and Release runtimes.
They include the original three leak witnesses, ready/error-order controls, a
parenthesized view of a named callable, a named view-parameter pipeline, and a
retained view-parameter callback. Both existing public borrowed-sum controls pass
all twelve source cases and 24 Source-deleted native executions with exact public
messages. Source policy stays at its default independently of runtime profile
for the new secret controls. These bounded results do not establish every other
terminal diagnostic or full workspace/platform acceptance; see the
[native audit](../active/native_acceptance_audit.md#typed-pending-sum-terminal-diagnostics).

Direct field reads from a pending `secret[Struct]` previously exposed raw
reference data after stripping the typed wrapper, while native field readiness
rendered `[redacted]`. The direct-field diagnostic successor retains the checked
receiver type and renders the original wrapped value before consuming it, applying
the same redaction without joining pending values or changing failure order.
The original `07_pending_secret_struct_receiver.jett` source is retained; its gate
requires one shared redacted failure message and passes with both normal runtime
profiles after Source deletion. All seven secret-owner controls and eight adjacent
native debug/pending controls pass. This repair does not establish parity for the
separate `handle` or pipeline failure paths.

The accepted-print controls compare exact reference events and native output for
public and secret-bearing erased records, generic fields, incomplete builders,
nested containers and pending payloads. Nine programs pass 18 Source-deleted
executions across Debug and Release runtime profiles. Public text equal to
`[redacted]` remains ordinary public text.
