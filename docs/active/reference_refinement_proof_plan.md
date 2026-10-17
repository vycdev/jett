# Reference refinement proof transport

An established refinement may be reused through an unchanged checked value.
A checked result annotation alone cannot establish that invariant for fresh
data. Runtime `Typed` labels also cannot prove a replacement payload: raw calls
and property shrinking can preserve the label while changing the data.

The current repair transports actual source-expression facts through named and
inline calls, parameters, returns, reordered arguments, and pipelines. Raw
invocation trees and forced validators disable that reuse transitively. Checked
program and zero-argument verify entries have explicit trusted entry bridges;
entry annotations do not synthesize argument proofs.

Three producer defects must be closed before committing that optimization:
private list sum fallback values, inline closures whose return annotations were
discarded, and reflected fields requested under unproved refinements. Reflection
must establish new requested invariants before publishing reusable result facts,
including refinements inside admitted builtin containers. New invariants beneath
secret qualifiers require a protected observer contract; conservatively refuse
that proof rather than run a public predicate on a hidden payload.
Use the actual declared field schema for exact and ancestor reuse; inspect only
active sum payloads and preserve collection order, pending depth, and source
ownership. Raw and metadata-free execution retains its old consuming boundaries.
Exact schemas preserve pending values. A changed nested invariant under a pending
container fails without an implicit join. Changed callable and nominal generic
requests introducing refinements also fail until conversion contracts are
selected. These refusals do not claim complete reflected-type support.
This is enforcement of existing value invariants, not a choice of which reflected
requests or list sum domains should be admitted. Those admission questions remain
in `docs/open_design/reflected_field_request_compatibility.md` and
`docs/open_design/list_sum_type_contract.md`.

Canonical type owners must also remain stable after resolution. Written aliases
resolve in their lexical context once; canonical checked, captured, generic, and
reflected owners must not be resolved again through a caller's shadowing import.
Keep body imports available and isolate only canonical normalization, validation,
and selection operations. Written view wrappers affect borrowing metadata, while
their underlying refinement still requires validation at raw boundaries.

Verification includes focused interpreter controls, real frontend-admitted
probes, linked debug/release calls after source removal, comptime and native
verify/property bodies, changed-candidate shrinking, and final workspace and
supported-host distribution gates. Native validation of newly refined returns
is a separate implementation slice; supplemental tests do not change the fixed
acceptance inventory denominator.
