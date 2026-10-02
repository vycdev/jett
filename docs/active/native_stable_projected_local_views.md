# Typed stable projected local views

Rule Set 24 already permits readonly, non-owning field views. Native support
must preserve the exact checked field type and the lifetime of its immutable
local or view-parameter owner. The characterized `view packet.data` case passes
checking and reference execution, but native lowering lost its stable binding
origin before this repair.

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

Temporary roots, mutable backing chains, projected mutation, and owner changes
after creating an alias retain their current implementation boundaries. This
repair does not select lifetime extension or loan expiry; those questions stay
in the existing local-view and projected-assignment design notes.

Verification must cover direct and nested paths, forwarded aliases, view
parameters, exact checked contexts, clone independence, owner liveness, and
success/failure cleanup with linked debug/release native execution.

The integrated focused checkpoint passes all 896 compiler-phase library tests
and all 12 local-alias native driver tests, including the eight new linked
cases, matching profiles, source removal, and terminal-failure cleanup. Exact
logs and the remaining whole-workspace/platform obligations are recorded in
`native_local_view_aliases.md`. State-qualified machine field paths remain a
confirmed separate native gap rather than part of this ordinary-struct result.
