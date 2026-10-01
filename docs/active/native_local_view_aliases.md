# Native local view aliases

Local aliases of views are an admitted source form with a bounded native
implementation. This note records its handoff and remaining limits; it adds no
ownership policy or claim of complete native coverage.

## Settled behavior and current gap

[Rule Set 24](../design.md#rule-set-24-read-only-views-solving-the-memory-borrowing-problem)
makes views read-only borrows: they can be read, passed as views, and explicitly
cloned into ownership. They cannot be consumed or escape their scope. For example:

```jett
function duplicate(view source: list[int64]) returns list[int64]:
    list[int64] borrowed = view source
    list[int64] forwarded = borrowed
    return clone forwarded
```

Previously the checker tracked these declarations by resolved identity without
transporting their borrow mode into HIR. MIR analyzed `Let` as an owning
expression and gave ordinary aggregate locals owning drop slots. Allowing the
initializer alone would transfer or drop a borrowed handle incorrectly.
Implicitly copyable results, explicit clones, and ordinary owned field copies
retain their existing behavior.

## First implementation domain

The implementation admits direct aliases and forwarded aliases rooted in an
immutable owner or immutable view parameter, used for borrowed reads, view
calls, and explicit clones.
Preserve the checked type, including secret and refinement identity. Do not infer
view-type semantics from named aliases or turn arbitrary call results into views.

Carrier-preserving `coarsen` and `declassify` bindings retain their checked backing
chain through MIR lowering. Only a destination with a valid borrowed origin and
a validated transparent initializer takes this path. Ordinary owning expressions
keep their existing snapshots; an explicit `clone`, handler, or allocating
conversion cannot be reinterpreted as an alias initializer.

1. Export typed binding facts through ordinary, generic, and reflected comptime
   body handoffs. Facts must belong to the concrete instantiation: the same
   generic source span can describe a scalar copy or an aggregate borrow.
2. Represent borrowed local bindings and their source owners explicitly in HIR
   and MIR. Forwarded aliases retain the original owner dependency. Preserve
   these facts through handler extraction, generated functions, and ID remapping.
3. Carry persistent loan dependencies across statements and CFG edges. Extend
   owner liveness for alias reads, validate owner availability, and preserve
   borrowing in matches, sum handling, iteration, and debug observations. A view
   binding must never become an owning move merely because its name is local.
4. Give borrowed handles non-owning native storage. Only actual owners enter
   drop planning; normal exits, branches, loops, and terminal failures must clean
   them exactly once after dependent reads. Existing borrowed-handle operations
   may suffice, but runtime requirements must be verified rather than assumed.

Validate alias chains, lexical shadowing, generic specializations, nested control
flow, pending values, clone independence, debug reads, and failure cleanup with
linked reference/native comparisons and ownership-plan regressions.

MIR retains a may-created alias set across CFG joins. Once a reachable path has
created an alias, consuming or rebinding its root produces an explicit native
implementation error, including after the last alias read. This conservative
boundary avoids choosing lexical or last-use loan expiry. Alias reads keep
their immediate origins and ultimate owner live; aliases never receive owning
drop slots. Function descriptor aliases follow the same rule even though native
function storage otherwise permits copied descriptors.

Handler staging follows exact formal view modes for direct and indirect calls.
It snapshots a borrowed argument before a later handler and observes a callback
after its arguments. Qualified string and descriptor snapshots acquire their
own retained or cloned handles. Ordinary copy-owned alias expressions also
acquire ownership before entering a list, sum, or record, while borrowed
arguments keep raw handles. Plain string view expressions preserve existing
copy semantics in owning contexts.

## Boundaries requiring a contract

The existing [projected assignment note](../open_design/projected_field_assignment.md)
states that declaration facts are not complete borrow-provenance analysis and
that mutation with outstanding views needs a selected contract. Keep these
related questions explicit before extending the first domain:

- Rebinding or consuming an owner while an alias exists, including whether loan
  expiry for source validity is lexical or based on last use.
- Rebinding a mutable alias itself; current declaration facts remain read-only
  and do not define a general lifetime transition.
- Views of temporaries, projected temporaries, or allocating conversions. If
  lifetime extension is selected, they need a hidden owner evaluated once and
  kept alive for dependent aliases, not an implicit clone of every view.

Some such forms are already admitted by the frontend. Acceptance and incidental
interpreter cloning do not settle their ownership contract. Keep these forms
visible as unimplemented native cases; do not silently redefine them, implement
projected mutation, or count the first domain as complete view support.
