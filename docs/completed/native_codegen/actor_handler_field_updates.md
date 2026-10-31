# Checked actor handler field updates

Status: implemented with bounded MSVC acceptance, 2026-10-15. This note records
the actor-context contract under [owned mutable field updates](owned_mutable_field_updates.md).
The [native audit](native_acceptance_audit.md#checked-actor-handler-field-updates-bounded-msvc-acceptance)
retains the exact execution matrix and its remaining limits.

The existing actor rules already select the behavior: an actor owns its private
state, a message transfers its owned arguments, and a response transfers an owned
result. A mutable owned state binding may update a checked record field using
the same place, readiness, reservation and replacement rules as an ordinary
mutable local. An actor handle never exposes remote state as a writable place.

## Checked identity and persistent bindings

A handler is identified by the retained actor declaration, its canonical actor
type and its original handler ordinal. It has no invented function definition.
The original Send or Ask expression, exact argument acquisition and order,
response type, and caller context must join that handler. Handler body facts are
selected under that handler's own context, including nested checked scopes.

Capabilities and state occupy the actor's captured scope. Message parameters and
handler locals occupy a child scope, matching the resolver. The existing
no-shadowing rule rejects a message parameter that reuses a state
name. Physical state publication still joins exact captured slots instead of
reconstructing ownership from an innermost name lookup. State, capability and
message roles retain their exact
resolved declaration, type, access mode and mutability.

An accepting ownership traversal must retain diagnostics and check persistent
capture availability at each real normal exit, after acquiring the response.
The previous breakpoint observer is not an ownership admission proof. An early
Respond inside a field RHS abandons the field commit and preserves the same
persistent state obligation. Copy values use their normal copy acquisition;
move-only replies require ownership, and borrowed replies cannot escape.

A persistent state view, if established by an admitted initializer, must retain
its actual checked backing owner and loan dependency. It cannot be installed as
owned data or as an unbound view. Initializer order, scope and provenance must
be preserved by both backends; this note introduces no broader view admission.

## Terminal and unresolved response exits

A handler whose original declaration has no `responds` may terminate with a
checked nothing-valued Return. Its canonical response type must be nothing; the
operand must be absent or have the exact checked nothing type and cannot escape
a view. Actual operand effects run before persistent capture validation. A Return
inside a field RHS abandons that field commit, and a successful handler exit
publishes the retained captures. Source16 exercises a nothing-valued helper after
state updates, an early Return inside a field Handle, and independent instances.

The design spells replies with Respond. The current checker also observes Return
and fallthrough in responding handlers, including `responds nothing`, without a
checked response protocol. This is an existing gap, not authorization to give Return a new meaning or to
add a blanket Source ban. Keep these exits explicit and withhold an actor
execution receipt when its response/persistent-state contract is unproven.
Their complete language and backend handling remains an open obligation.

## Publication and verification

Native publication must establish an occupied, live actor field before replacing
it. The separate ordinary `ActorReplaceOccupied` no-cleanup swap transfers the
incoming field and returns the displaced value. The caller clears the transferred
staging slot before separately retiring the displaced owner. A failure before the swap leaves ownership unchanged; a
failure during displaced cleanup must not restore a second incoming owner.
The ordinary struct replacement leaf remains unable to mutate an actor base.

The unchanged original24 and actor Source03 execute alongside repeated owning
updates, independent instances, ordinary helpers, persistent direct and projected
views, terminal Respond, owning replies and exact nothing returns from
nonresponding handlers. Separate checker/HIR/MIR/reference controls cover invalid
identity, role and context joins, persistent capture availability and loans.

Eleven native tests pass with twenty-four Source-deleted executions and forty-eight
publication refusals. Refusals and runtime symbol measurements supply no execution
credit. Main and suite controls pair Source Debug/default with Runtime Debug and
Source Release/optimized with Runtime Release. The original nested receiver keeps
Debug Source policy against both runtime profiles. This is not the full profile
cross-product. Existing original initializer/capture Graph regressions also pass
with Source removed, using the Debug runtime and both Source options; they do not
grant live Resource custody or general handler Resource authority.

Resource custody, wider frozen-graph and generic/reflected compositions, real
asynchronous scheduling and full workspace/platform acceptance remain separate
obligations. The unresolved response exits above remain open.
