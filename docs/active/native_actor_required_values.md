# Actor-owned required values

The language already permits a closed, pure explicit `comptime` expression in an
actor initializer or receive body. This work repairs compiler provenance; it does
not select a new language rule or permit runtime actor storage in a closed value.

The unchanged responding-handler witness is
`crates/jett_comptime/src/resource_execution/fixtures/actor_handler_required_callable.jett`.
Its Source SHA-256 is
`ce01b25e20a4955067e87fbc7389f1957b21fd10de6e76575e337f2f98111665`.
Before this frontend change, Debug and Release CLI checks reached required-value
discovery and reported E9001, `resource execution has no exact checked body`, at
its `comptime stored()` expression. The receive body is retained checked Source;
the worker simply did not enumerate actor owners.

The first change projects metadata for the exact original actor initializer or
handler occurrence. It retains the checked program Arc, actor DefId/ActorId,
handler ordinal, physical original expression, complete selected lexical scopes
and result TypeId. An actor handler has no generic-function projection. Its root
and scoped type-name projections come from the checked handler body, and cached
proofs preserve the original actor owner instead of pretending it is a function.

This required-metadata route does not install a Resource execution cursor.
`validate_executable_cursor` and `install_function_body` continue to reject an
ActorHandler root with `WrongPurpose`. Copied AST nodes, foreign sessions,
changed owners/ordinals, mismatched scope paths and runtime entry remain refused.
The new controls also retain closed-worker rejection of free actor state and
typechecker rejection of capability-requiring calls in required expressions.
Frontend controls evaluate the complete unchanged witness in both checking modes,
but do not execute its runtime main.

## Required-worker closure environments

Closedness also applies when required evaluation constructs a lazy inline
function without invoking its body. Before constructing that value, retained
ExplicitComptime and NamespaceConstant workers match the full original
parameters, return annotation and body in the same checked program. They inspect
its resolved Variable/Param identities: external lexical bindings must exist in
the actual captured interpreter scopes. Own formals and body-local declarations
remain deferred, and exact namespace values, named functions and type
declarations keep their existing registry roles. A same-name namespace constant
cannot supply a missing actor state field or handler parameter. Nested lazy
bodies cannot hide an unavailable outer capture.

This generic worker check preserves pure local/parameter captures, own lambda
formals and locals, exact named/constant references and genuine nested returned
closures. It does not call the constants-fallback variable lookup, install a
runtime ActorHandler cursor, mint Resource custody or renew a native snapshot.
It enforces the existing closed-expression rule without selecting new Source
behavior.

## Accepted frontend checkpoint

The successor focused gate passes **eight Rust tests**, each exercising Debug
and Release checking modes. It includes the complete unchanged responding
handler, distinct initializer owners, exact metadata refusals, closedness and
capability rejection, checked lexical type scopes, unavailable returned/nested
captures and retained pure captures. The focused log is
`target/native/actor-required-frontend-focused-v2.log`, SHA-256
`1ccd5c4983869f82ee69ab396a7036280459278926f781fe9cc095920ba1da81`.
At that frontend checkpoint, broader gates passed **1,615 distinct Rust tests**: 691 Comptime,
286 HIR, 603 frontend/reference fixtures, 26 CLI unit tests and nine CLI capture
tests. The eight focused controls are included in the Comptime total. The CLI
build succeeds and its Cargo JSON identifies the exact executable checked below.

That checkpoint's rebuilt CLI passed **12 expected checks** across Debug and Release
checking modes: the unchanged original witness and the pure-capture control
each exit zero; all four unavailable-capture controls each exit one with E9001.
The original witness has no errors and retains one unused-variable warning.
These are frontend checks; runtime main is not executed. The exact 1,075
compiler/Source input hashes remain unchanged.

Frontend checkpoint logs under `target/native/` are:

- `actor-required-frontend-core-v2.log`, SHA-256
  `f6c053d6f676397918d50e6bb86b9b100a05850b5ca9da6b85699210f58cbc59`.
- `actor-required-frontend-fixtures-v2.log`, SHA-256
  `a48c70eea4da541ca856acb98f3e19584d4dda964208a352acea07b8bcda9a6a`.
- `actor-required-frontend-cli-v2.log`, SHA-256
  `b2fe9d9afea22edd9fd695310a70b7da9ba93a3928b5eed8d1ad6c871020f43c`.
- `actor-required-frontend-cli-build-v2.log`, SHA-256
  `6547c594cf5de096cf1ce9e3707bd4c08b65feebd3f1ad9b27de918ff27c6d66`.

The frontend CLI receipt is
`native-codegen-recovery-2026-10-09/actor-required-frontend-cli-v3/check-result.json`,
SHA-256 `e6c1c28dbb746559d8f39c265a09a3a1995c3e9d5fbc9069214c6efe022170c1`.
It pins executable SHA-256
`01e5e6cac3ef94c89ac465c354c0673a8a55adfa3e2cfcf9a7e07f617bf061ae`
and all 12 Source/log hashes. The compiler-input map is SHA-256
`bc346d6b796710cf17c6c9064b12b1c34bc0c4896d95a5417507ff79bbfc48f3`.

The genuine returned-capture negative control first failed with zero diagnostics
where one was required; its log is
`target/native/actor-required-free-capture-red-v1.log`, SHA-256
`224ac709a668bc9c1591d648d4d9866d725234c574f35f8d9fb2e37dbe2246eb`.
Exact-current-image v2 checks also confirmed that defect. Earlier direct CLI v1
checks used a stale image and supply no current acceptance or refusal evidence.
These discovery runs receive no successor acceptance credit.

Frontend cache construction and checking alone supplied no HIR or native actor
acceptance. The separate [checked actor required-bake transition](native_actor_required_bake.md)
now joins exact actor owners and renews complete snapshots from the actual
checked materialization receipt. The HIR owner matcher no longer declines an
actor owner merely because its metadata kind is ActorInitializer or ActorHandler.
The later actual-sealer receipt also renews the complete baked execution table
from its exact token edits while validating unchanged full actor snapshots. It
does not admit actor-body-changing sealing deltas. The original frontend evidence
above remains a historical checkpoint.

The remaining boundaries are independent:

1. Canonical callable sealing that changes an actor body must renew that same
   full snapshot using the actual bound-to-sealed receipt and authenticated
   source lineage. It must preserve every canonical conversion, temporary-view
   and peer transition proof. A plan moved into a generated adapter need not
   change the handler snapshot; no unconditional sealing failure is inferred.
2. Actor-owned Graph producer transitions remain separate from ordinary checked
   required baking.
3. In a retained program with Resource hooks, actor-owned required evaluation
   still reaches the unchanged runtime cursor gate. That requires a separate
   exact occurrence-bound execution route. Temporarily disabling transport would
   bypass transitive absence/custody handling and is not an acceptable fallback.
4. The [paired ordinary native checkpoint](native_actor_required_bake.md) now
   records two Source-deleted executions of the whole unchanged witness with
   `observed:5\n`, empty program stderr and exit zero in matched Debug/Release.
   Physical lifetime, queue/callback ownership and same-context reentry still
   need their own observations; ordinary output alone cannot establish them or
   complete the native-codegen goal.
