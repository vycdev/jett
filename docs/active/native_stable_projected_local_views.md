# Typed stable projected local views

Rule Set 24 already permits readonly, non-owning field views. Native support
must preserve the exact checked field type and the lifetime of its backing
owner. Alias slots remain immutable; a mutable terminal must be an exact checked
owned local or owned parameter, while view-parameter backing stays immutable.
The mutable-owner extension below has focused MSVC acceptance. The characterized
`view packet.data` case passes checking and reference execution, but native
lowering lost its stable binding origin before this repair.

Keep `CheckedViewSource::Binding` and `Local.view_source` as the immediate
binding dependency. The typed initializer's existing ordinary-struct `Field`
nodes provide the canonical finite projection path; do not duplicate field
indices in local metadata. Validate the actual source-local type, every field
owner and index, intermediate and endpoint types, and the terminating local ID.
Carrier-preserving wrappers retain the existing representation checks.
They also preserve nominal qualification: only existing outer secret promotion
may be implicit. `coarsen` follows the declared refinement ancestor chain and
`declassify` removes the exact outer secret layer. A view or empty conversion
cannot introduce, remove, or substitute a refinement.

The whole owner's type can differ from the borrowed endpoint's type. Removing
that old equality check requires proving every borrowed initializer, including
unused locals and unreachable MIR blocks, before optimization can remove it.
Generated functions must discard copied origins that belong to another body;
borrowed captures retain their existing rejection.

Existing persistent owner dependencies, liveness, and non-owning alias storage
apply to projected aliases. Ordinary field copies and explicit clones remain
owned. Bytes and lists are executable witnesses, not an endpoint allowlist.
There is no runtime ABI change or implicit allocation.

Temporary roots, mutable alias slots, mutable view-parameter backing and owner
changes with dependent aliases retain their implementation boundaries. Absent or
mismatched selected reflected root facts and borrowed-sum Handle backing keep
separate proof limits. The mutable-owned-backing extension selected no lifetime extension or
loan expiry. The later [temporary-backing contract](native_temporary_local_views.md)
selects lexical whole-owner retention; its implementation remains pending.
General loan expiry and other unresolved boundaries stay in the local-view note.

Verification must cover direct and nested paths, forwarded aliases, view
parameters, exact checked contexts, clone independence, owner liveness, and
success/failure cleanup with linked debug/release native execution.

The ordinary-struct focused checkpoint passed all 896 compiler-phase library
tests and 12 local-alias native tests; the later frozen a0bf12df workspace
passed all local gates at its 378-test corpus. The
[exact-state follow-up](native_local_view_aliases.md#exact-state-qualified-machine-field-follow-up)
now extends the same proof to declared MachineState payloads. Its current
261 HIR/MIR/codegen library tests pass, as do 26 distinct local-alias tests
across a 25-test batch and a separate typed-endpoint test. Exact scope/logs
are in that note. The new 392-test frozen workspace and supported-host
acceptance remain required. This preserves exact state/nominal qualification
and current owner-change boundaries. At that checkpoint, bare flow-narrowed,
mutable or temporary origins and broader projected owner/endpoint combinations
were outside the bounded proof. Those historical counts do not validate the
mutable-owned-backing follow-up.


## Mutable owned backing follow-up

The extension authenticates a mutable terminal through its exact original
DefId-to-LocalId mapping and active concrete owned binding fact. It does not
infer ownership from absent alias metadata, a name, or flat global facts.
Direct and projected aliases retain their immutable immediate dependency chain;
all field-owner/index/endpoint, nominal qualification and initializer checks
remain mandatory, including unused and unreachable aliases. Existing persistent
loan and liveness planning still blocks every owner change while a reachable
alias may exist, even after its last read.

The unchanged original unused Source08 and observable nested bytes/list
forwarding and copy-independence Source11 pass reference checks and four
Source-deleted executions in paired Source/runtime Debug and Release. The
[native audit](native_acceptance_audit.md#mutable-owned-local-view-backing-bounded-msvc-acceptance)
records the full selected matrix: 31 tests, 64 executions and six separate
publication-boundary checks, alongside 753 affected library tests.
Mutable View-parameter roots and temporary-backed aliases remain unsupported
native compositions; they are not new Source bans. Mutable owned borrowed-sum
Handle backing has separate [bounded receipt acceptance](native_mutable_borrowed_sum_backings.md),
which do not infer ownership from absent alias metadata. Absent or mismatched selected reflected root facts remain refusal cases.
No runtime ABI, implicit clone,
lifetime extension or loan-expiry change is made. The current bounded acceptance
is separate from the historical receipts above and does not complete native parity.


## Reflected outer-owner fact transport

The producer retains only exact transitive dependencies of its
actual local-view facts from the selected concrete body map. It follows the
original binding DefId to its declaration and copies the matching fact without
changing ownership mode, type or mutability. Missing/mismatched facts and cycles
remain conservative; no flat-map fallback or ownership inference is added.

HIR and MIR admission, finite field proof, immutable alias slots, persistent
loans, backing liveness and cleanup guards are unchanged. Source12 targets direct,
generic repeated reflected and nested alias forwarding contexts. Genuine scoped
fact tests distinguish scalar copies from aggregate views and refuse a corrupted
child map despite a valid enclosing or other concrete-instance fact.

Source12 passes reference checks and two Source-deleted executions in paired
Source/runtime Debug and Release. The
[native audit](native_acceptance_audit.md#reflected-local-view-backing-facts-bounded-msvc-acceptance)
records the 1,797-test affected-library result and the complete selected matrix:
32 tests, 66 executions and six separate publication-boundary checks. Earlier
first-slice receipts retain their own scope. No lifetime, borrowed-sum,
Resource, Runtime ABI or archive change is included.


## Temporary backing decision

The selected [temporary local-view contract](native_temporary_local_views.md)
retains an ordinary owned temporary's whole exact result, evaluated once, in
compiler-owned storage through the alias declaration's original lexical scope.
The finite typed projection and immutable forwarding remain nonowning; no
last-use expiry, implicit clone or invented Source DefId is allowed. Cleanup
must retire the backing exactly once on each scope exit, after acquiring any
owned exit result, while preserving copyable endpoints, pending/error order
and escape/loan rules. Operation-bounded call staging stays separate and no
actor/Resource authority follows. Implementation, unchanged Source09 execution
and reference/native cleanup gates are pending. None of the historical or
current stable-owner receipts above establish temporary-backed execution.
