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

## Requested invariants are not producer proof

The frontend currently accepts each request below for a field whose stored
integer is `0`, where `Positive = int64 where value > 0`. Before establishing
the requested invariants at the reflected producer, reference probes returned
`0` through a checked `Positive` consumer in all five cases. A checked call's
declared result type therefore cannot, by itself, prove the cloned payload.

| Actual declared field | Requested type | Later exposure |
| --- | --- | --- |
| `optional[int64]` | `optional[Positive]` | Handle the occupied success arm |
| `list[int64]` | `list[Positive]` | Read an element |
| `secret[int64]` | `secret[Positive]` | Explicitly declassify the result |
| `function() returns int64` | `function() returns Positive` | Invoke the callback |
| `Box[int64]` | `Box[Positive]` | Read `Box.value` |

For currently admitted ready builtin wrappers, a bounded producer safeguard
can establish newly requested leaf refinements before publishing checked proof.
Use actual declared schemas to retain exact invariants or skip an established
ancestor prefix. Visit occupied optional/result arms and collection members in
their existing order, preserve the source, and leave inactive arms alone.
Runtime type labels are not evidence for a replaced payload.

Repeated alias or refinement arguments normalize independently within this
existing relation. The reference comparator now tracks only the active expansion
path; a completed sibling is not a recursive re-entry. This repairs repeated
map/result slot comparison without selecting broader admission or converting
nominal owners. Native raw carrier compatibility now follows the five admitted
builtin shapes, retaining matching secrecy and exact primitive, nominal, and
callable identities. Checked producer trees establish their new refinements
after complete occupied-shape preflight. This implements the bounded ready
builtin safeguard without selecting broader requested-type admission.

Pending containers require a separate conversion contract. An exact
actual/requested schema preserves pending values unchanged. Establishing a
changed nested invariant must not implicitly join the container or inspect its
pending payload; refuse unsupported proof before exposing a trusted result.
This does not select broader requested-type admission or pending conversion
semantics.

New invariants beneath `Secret` also require a protected observation contract.
Running `Positive`'s public predicate during a request for `secret[Positive]`
can trace its publicly typed input before the source's explicit `declassify`.
The existing rule allowing validation of a declared secret-backed refinement
does not select that new reflected conversion. Conservatively refuse this proof
establishment without rendering the value, while retaining exact qualified
reads. Keep the hidden-observer policy in
[debug_print_hidden_secrets.md](debug_print_hidden_secrets.md) open.

Changed callable results now have the selected lazy return-validation adapter
contract below. A descriptor alone still cannot prove a future result. Changed
nominal generic instances need a separate selected conversion contract; walking
builtin wrappers does not validate arbitrary nominal fields. Their unsupported
proof establishment remains an explicit feature gap, not a claim of complete
reflected read coverage.

The same observer concern applies to a successful summation fallback whose
selected type is one or more secret qualifiers around a refinement. Refusing
that unsupported proof does not define the still-open summation domain, add
container-valued summation, or change existing arithmetic and shape errors.

### Selected lazy reflected callable return validation

A checked reflected callable request may establish supported new return
refinements through an owned lazy adapter. Exact owner, member, field ordinal,
pending-selector and declared-schema checks precede construction. The adapter
retains the original stored signature, source body, captures, debug identity
and pending state; constructing or reading it invokes neither callback nor
predicate. Requested parameter, view and capability contracts must satisfy the
existing ordinary callable adapter relation. A descriptor or requested type
annotation grants no proof, effect authority or nominal conversion.

Each invocation evaluates its actuals once, invokes the retained source once
under the original signature, and preserves the source's ordinary validation.
Only after the source result exists does the supported typed produced-refinement
conversion establish the unproved requested return suffix. Failed validation
uses the existing terminal refinement-boundary error and publishes no trusted
target result. Exact established prefixes are reused. The adapter never joins
implicitly or inspects pending payload; a pending result requiring a new check
reaches the existing pending-inspection refusal before predicates or consumers,
while exact already-proved pending transport remains unchanged.

The selected contract now has an isolated checked/reference/HIR/MIR/native
prototype. Eighteen genuine Sources include the two byte-exact original wider
and exact callback probes, named and captured callbacks, lazy reads, repeated
calls, invalid results, selector precedence, recursive optional results and
ready, pending, exact and ancestor Nothing/refined-unit returns. All eighteen
reference entries and host objects pass; all thirty-six Source-deleted linked
executions pass against freshly built normal MSVC Debug/Release Main archives.
Source checking uses release=false independently of runtime artifact profile.
The native assertions compare exact stdout, stderr and exit status. The public
ordinary launcher does not expose context-retirement counters.

Current isolated authentication gates also pass thirteen reference, five HIR
and six MIR tests. Original closure body and typed capture/context witnesses
are constructed at the trusted capture site. Admission and invocation compare
against those retained witnesses; mutating a same-span body, checked context,
signature, field ordinal, Source owner or predicate cannot mint a replacement
proof. Ordinary required constants and refinements retain their original
checking and materialization path; ancestor return transport uses the already
proved type relation without a new pending inspection or predicate.

This is target-only acceptance of the exact saved candidate. Managed
integration, broad workspace and successor CI checks remain pending. New
nominal/secret proof establishment, protected secret observation, generic and
other reflected forms, and authenticated generated verify/property-suite
transitions remain full-goal obligations. No Source exclusion, fixture
denominator change or broad language-completion claim follows from this gate.
