# Lexical backing for temporary local views

Status: selected language contract, 2026-10-15. Compiler implementation and
reference/native cleanup verification remain pending. This decision adds no
execution credit and changes none of the historical local-view receipts.

## Selected rule and tradeoff

An admitted immutable source local initialized by a view of an ordinary owned
temporary retains the whole exact producer result in compiler-owned storage.
Evaluate the producer once at its original initializer position and retain that
owner until the declaration's original lexical scope exits. Immutable forwarding
creates nonowning aliases to the same backing; it does not duplicate the owner
or extend the originating scope. Even an unused alias or one read only early in
the scope keeps that backing until scope exit.

Scope-based freeing in [Rule Set 10](../design.md) and Rule 24's lexical escape
bound support this choice. Previously, the local-view note left both temporary
extension and last-use expiry unresolved. Lexical retention keeps cleanup local
and explicit without introducing last-use loan analysis. It may retain unused
owned storage longer than a last-use scheme. That is the selected tradeoff;
keeping only a projected field or cloning it would change the ownership model.

The whole owner retires exactly once on normal scope completion or an exit by
Return, Respond, Default, Break, Continue or terminal failure. Acquire any owned
exit result, including an explicit clone, before retiring backing it may read.
End dependent aliases before dropping the owner. Loop iterations create separate
backing activations; backedges retire that iteration's owners, and other exits
retire only crossed scopes. Unselected branches create no owner. Existing cleanup
ordering and primary-error/cleanup precedence remain authoritative.

No last-use expiry, implicit clone, runtime owner pin or fake Source DefId is
introduced. Copyable endpoints retain their ordinary copy behavior. Existing
escape and dependent-loan rules, typed field paths, pending checks and failure
order remain unchanged. Views gain no permission to return, capture, enter an
owning aggregate, cross threads or outlive their originating scope.

This local rule does not grant actor-state lifetime or Resource custody. Mutable
alias transitions, allocating conversions without an exact owned-producer proof,
borrowed-sum alternate backing and other current implementation limits remain
separate. It adds no Source ban to compensate for missing native support.

## Checked initializer and generated owner proof

`CheckedViewSource::Other` records an unstable expression origin; it is not an
ownership or lifetime certificate. Lowering must construct one private checked
association joining:

- The authentic program/body and concrete ordinary, generic or reflected
  context, original alias declaration, immutable binding type and lexical scope.
- The original written View and exact owned producer occurrence, whole-owner
  type, checked target/construction facts, conversions and argument order.
- Every finite projection step's checked owner, field identity and endpoint
  type, preserving secret/refinement/interface identity and established machine
  state qualification without inferring authority from physical layout.
- One dominating generated owning initializer and the nonowning alias chain.
  The original whole result moves there once; it is not copied or re-evaluated.
- The source scope and each dynamic activation, current HIR/MIR headers and
  remaps, and every normal/abrupt cleanup edge. Validate these before pruning,
  including unused aliases and unreachable metadata.

A matching name, span, type or LocalId cannot substitute for this proof.
Generated storage has private compiler identity, not an invented resolved Source
declaration. Synthetic call/Handle extraction scopes must neither shorten nor
extend its lifetime. Alias reads retain the whole owner's dependency, but
last-use liveness cannot release it early. Only the hidden owner has an owning
cleanup slot. Failed initialization must distinguish vacant, acquired and retired
storage and must not publish an uninitialized alias.

Reference execution must retain the checked scope-owned obligation; incidental
Value cloning is not lifetime evidence. Native emission uses ordinary owning
storage for the whole producer and nonowning alias storage. Pending ancestors
still fail at the original projection before later effects; no implicit join or
metadata erasure is introduced. Exact checked proofs remain required for every
admitted executable body context.

## Separate operation-bounded call views

[Internal call-view staging](native_scoped_call_view_staging.md) keeps its own
temporary through one complete consuming operation. Its BeginCallView/EndCallView
rules are unchanged. Ending an inner call loan cannot retire a persistent local
alias's backing. The new local rule cannot lengthen an existing call stage, and
a call-stage End cannot be used as this source local's scope-exit cleanup proof.
Shared root/projection validation is useful only with distinct lifetime proofs.

## Pending Source and cleanup gates

Keep `crates/jett_driver/tests/native_conformance/local_view_aliases/09_temporary_root_boundary.jett`
unchanged. It declares `bytes borrowed = view make_packet().data` and never reads
the alias. The selected backing is the whole returned Packet and its owned bytes
until main's lexical scope exits. Expected stdout/stderr and debug events are
empty. The current native refusal remains an implementation gap; the existing
frontend-admission/publication-refusal test supplies no successful execution
receipt. Empty-output success alone would not prove lifetime or no hidden clone.

Required gates remain unexecuted for this contract:

- Preserve Source09, then add producer-once markers, later direct/forwarded
  reads, an unviewed owned sibling, and explicit-clone independence.
- Exercise nested scopes, skipped branches and loop fallthrough/Continue/Break,
  retaining backing after the last read until its original scope exits.
- Acquire an owned `return clone borrowed` or reply before backing retirement;
  preserve negative consumption/capture/escape controls and early Handle exits.
- Preserve pending-root and later-failure order, exact terminal diagnostics and
  cleanup of partial producers, whole owners, siblings and unrelated earlier
  values. Retire each owner once on success and failure without changing error
  precedence.
- Validate genuine checked reference execution and Source-deleted native
  Debug/Release runs. Inspect ownership/drop plans and measure existing private
  allocation/destruction counters: producer output cannot prove absence of
  hidden cloning. Require no leaked owners after teardown.
- Use authentic proof mutations and runtime fault injection for wrong context,
  producer/type/field/scope, stale remaps, missing/duplicate initialization and
  early/missing/double cleanup. Refusals grant no execution credit. Preserve the
  existing call-stage temporary controls and their independent End behavior.

The [local-view note](native_local_view_aliases.md) and
[stable projected-view note](native_stable_projected_local_views.md) retain their
bounded implementation status and historical receipts. This selected decision
does not complete temporary-backed execution or whole-language native parity.
