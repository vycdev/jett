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
changes with dependent aliases retain their implementation boundaries. Missing
reflected outer-owner facts and borrowed-sum Handle backing keep separate proof
limits. The mutable-owned-backing extension selects no lifetime extension or
loan expiry; those questions stay in the existing local-view design note.

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
Mutable View-parameter roots, temporary-backed aliases, missing reflected outer
root facts and mutable borrowed-sum Handle backing remain unsupported native
compositions; they are not new Source bans. No runtime ABI, implicit clone,
lifetime extension or loan-expiry change is made. The current bounded acceptance
is separate from the historical receipts above and does not complete native parity.
