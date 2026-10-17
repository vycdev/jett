# Owned mutable field updates

Status: selected contract, 2026-10-15. The user delegated the choice to fit Jett's
philosophy; checked local field updates are the selected rule. Implementation and
verified acceptance are tracked in the [native acceptance audit](native_acceptance_audit.md).
This contract note grants no execution evidence.

Jett is imperative, immutable by default, with explicit local `mutable` bindings.
An update to one field should identify that logical change without reconstructing
every unrelated field. `record.field = value` is the sole field-write spelling;
there is no mutable reference, setter hook, structural cast or alternate builder.
Its value semantics are replacement within one owned local, conceptually consuming
the old root and rebinding the updated root. A consuming function can update its
own mutable owned parameter and return it; it cannot mutate the caller's view.

## Admitted places and invariants

An assignment place is an exact resolved live mutable owned binding followed by a
finite checked field path. Parentheses do not create a different place. Ordinary
struct paths retain exact nominal/generic owner and field identities. A machine
field is eligible only when existing state qualification or checked narrowing
establishes its exact state and field. The state and tag are unchanged: an update
is not a transition. A nested record or exact-state machine is still part of the
same root ownership obligation. Mutable actor state may use the same rule only
through its existing owned handler binding, never through a remote actor handle.

Compiler-produced reflection metadata remains inspect-and-pass data. A field
path must not enter the canonical builtin TypeInfo, TypeField, TypeBitfield,
TypeBitfieldField, TypeMachine, TypeMachineState, TypeMachineTransition or
TypeVariant owner to alter its fields. Capture these exact builtin identities
when their type definitions are created; preserve the marker through type-table
cloning and canonical remapping, and revalidate it in every checked path. Aliases
and secret qualification do not erase this boundary. A user record may replace
a complete metadata field with another genuine metadata value; unrelated user
structs with similar names remain ordinary values.

This preserves the existing compiler-produced metadata contract and constructor
gate. Allowing edits to a genuine metadata record would permit manufacturing
different metadata after that gate. Separate mutable-observer provenance would
add another authority mechanism; the selected local user-value update rule does
not introduce it. This authority preservation was selected before its Rust
implementation on 2026-10-15; it is not an execution or bypass claim.

Immutable bindings, views and view-derived aliases, temporary/call-produced
targets, uninitialized or consumed roots, and owners with outstanding dependent
views cannot be written. A `mutable` flag is not ownership proof. Aliases and
generic/scoped contexts must be resolved before deciding whether the place is
owned; qualification must not be peeled to manufacture permission. No caller
mutation or escaping mutable borrow is introduced.

Do not traverse through an enclosing nominal refinement to change its fields.
Reconstruct and validate the complete refined value, then replace that value.
This applies to a refined root and a refined intermediate owner, including
aliases to either. A field whose own type is a refinement may be replaced as one
whole value under the existing checked refinement-conversion/handling rules;
its proof may not be assumed from the base carrier. Unrelated sibling refinements
remain intact. Updates must invalidate obsolete value/proof facts for the changed
place and affected root; no stale nonzero, narrowing or reflected-owner fact can
authorize a later operation.

Bitfields retain constructor-and-rebind updates. No bitfield projected write,
implicit mask/truncation or new width-validation rule is introduced. Enum payload
updates use ordinary owned bindings obtained through existing match semantics;
this contract adds no direct variant selector or state transition spelling.

## Readiness, order and replacement

Static ownership/place violations are compile errors. Pending depth is runtime
data as well as checked task state: existing admitted transport stays admitted.
Updating a pending root or intermediate owner uses the existing typed pending-
inspection refusal; no implicit join, payload observation or new blanket Source
rejection is added. Existing static pending-use restrictions remain intact.
Once an ancestor is ready, an untouched or newly supplied child may retain its
own pending depth under the existing transport rules.

Resolve the fixed place and check ancestor readiness/state before evaluating the
RHS. Evaluate the RHS exactly once using existing Source order and type, view,
move and refinement rules. It may read the old root through a temporary view that
ends before replacement, but cannot consume or overwrite the root needed for
commit. The root identity/generation and path must still be valid at commit.
An RHS failure or early handler exit changes no target field and retires every
newly acquired temporary exactly once. Allocation/validation that can fail must
finish before old storage is displaced.

On success, transfer the RHS into the selected field and retire the displaced
field exactly once. Preserve every other field, exact concrete/generic identity,
state, secret qualification and task depth without implicit Source cloning,
declassification, predicate replay or Resource authority. No partially moved
root is visible. Existing explicitly cloned values remain logically independent;
shared physical storage must use safe reconstruction or copy-on-write rather
than expose writes through an old clone or view.

If displaced cleanup itself fails, retain the established terminal cleanup/error
precedence and retire remaining/new obligations once; do not pretend the old
value was restored or publish an unvalidated replacement. Resource-bearing
updates require their authenticated custody/loan/replacement protocol. Ordinary
record support cannot bypass that protocol; incomplete Resource transport stays
an implementation obligation, not a new semantic exclusion.

Ordinary structs and exact-state machine owners have bounded checked reference,
required-comptime and native acceptance, including named/generic, inline and
reflected-iteration bodies. Checked actor-handler updates to ordinary owned state
have separate bounded reference and MSVC native acceptance under original handler
identity and persistent-state ownership. See the [actor handoff](actor_handler_field_updates.md)
for its exact controls and limits. Authenticated Resource-bearing replacement,
wider reflected/generic and frozen-graph compositions, real asynchronous
scheduling, and full workspace/platform acceptance remain separate obligations.
The native audit keeps the executed matrices distinct.

## Verification handoff

Start from `crates/jett_driver/tests/projected_assignment.rs` and
`tests/native/projected_read_controls.jett`. Preserve original rejection and
read/reconstruction witnesses. Add genuine normally checked execution controls
for local/owned-parameter nested scalar and owning fields, clone independence,
failure-before-commit, exact machine state, actor handler state, refinement and
bitfield rejection, and consumed/borrowed/temporary roots. Include pending root,
intermediate owner and stored-child cases without joining implicitly. Check stale
nonzero/refinement facts, views, generic/alias context, branch/loop lifetimes and
exact cleanup. Host-built carriers do not establish Source acceptance.

Reference, required comptime, native main and applicable verify/property roots
must consume the same checked place contract. Record actual Source-deleted linked
Debug/Release results, Source policy and optimization separately. Full affected
phase, workspace and supported-host acceptance remain independent gates.
