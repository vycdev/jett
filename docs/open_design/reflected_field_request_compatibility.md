# Reflected field request compatibility

Reflected field reads validate the metadata owner, member, field index, and
requested type before reading a payload. The design describes the requested
type as matching the field type. Both execution paths currently also admit
some refinement/base requests when their underlying representation and secrecy
agree. That broader compatibility rule still needs a precise language contract;
it does not establish arbitrary nominal casts or implicit coarsening.

A concrete accepted case exposed a separate native dispatch defect. If a record,
enum, or machine stores `Selected = Named where true`, and `Selected` has its own `Named`
implementation, reading the field as `Named` retains `Selected` dispatch in the
reference evaluator. Native reflection previously read the underlying interface
handle as though it were already the requested erased value, so it dispatched
through the refinement's base payload instead. Ordinary checked conversion of
`Selected` to `Named` already preserves the refinement owner.

For a request admitted by the existing reflection checks, native extraction
must preserve the actual declared field owner when converting it to an erased
interface. The field's representation alone cannot identify its method owner.
This follows the established nominal dispatch rule and does not broaden which
requests pass the metadata/type checks. Exact-type reads, explicit `coarsen`,
pending validation, source ownership, and failure order retain their contracts.

The same conversion must respect the directional secret qualification rule.
For `secret[Selected]` to `secret[Named]`, the shared outer qualifier protects
the destination while `Selected` remains the concrete method owner. Peel source
qualifiers only while peeling destination qualifiers, and stop at a nominal
refinement. A secret source converted to an unqualified interface retains its
secret owner, allowing an explicit implementation for `secret[T]`; surplus
source qualifiers and secret fields retain their redacted debug layouts.

The remaining admission question is whether reflected requests require exact
semantic types, allow a defined refinement/base relation, or use another checked
conversion relation. In particular, equal runtime carriers do not prove that
unrelated nominal refinements satisfy one another's invariants. Keep this
question distinct from preserving identity for the currently admitted interface
read.
