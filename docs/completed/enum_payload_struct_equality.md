# Explicit struct equality inside enum payloads

Status: implemented on 2026-09-30 following the user's explicit language
decision. Frontend, interpreter/comptime, and linked native execution enforce
this contract. This closes this equality gate, not the complete native audit.

## Contract

Enum comparison preserves variant/state/branch, length, collection order, pending
depth, IEEE floating-point behavior, and left-to-right short-circuiting. Nested
user structs require exact explicit two-view `Equatable.equals` methods. The
frontend walks the type graph with cycle detection and reports E0358 for a
missing implementation, including through containers, machines, and refinements.
Traversal stops at each custom struct method: ignored fields neither gain an
equality requirement nor receive structural comparison. Inequality negates the
complete result. Direct collection/sum equality still reports E0376; ordinary
bitfields retain their existing structural path. Actor and erased-interface
equality decisions remain separate.

## Compiler and runtime handoff

Checked owner types and exact body identities become typed HIR/MIR function
targets. Backend validation requires two exact struct view parameters, bool
return, and no captures. Reachability retains implicit dependency/stdlib methods.
Traversal order determines descriptors and generated branches, preserving
deterministic object emission. The graphics callback effect audit follows these
implicit methods through nested payloads and concrete generic comparisons.

The interpreter uses an explicit work stack and source method dispatch. Native
code owns resumable comparison cursors through additive `EnumEqualityStart`,
`EnumEqualityNext`, `EnumEqualityArgument`, and `EnumEqualityAnswer` leaves.
The `JQ` version-1 descriptor maps opaque record nodes to generated call branches.
Leaves release the runtime context lock before generated code invokes methods.
Nested comparisons use independent cursors. Matching pending structs receive
temporary zero-depth view shells borrowing their original fields; ignored fields
are neither cloned nor traversed. Success, inequality, allocation failure, and
method failure retire cursors and shells. Pending method results fail with
`Equatable.equals must return bool`.

## Verification

- Missing methods fail for both operators, concrete generic bodies, and closed
  required comptime comparisons. Graphics rejects effects hidden in nested
  enum equality methods.
- `tests/native/enum_struct_equality.jett` covers custom identity distinct from
  field equality, ignored structs/functions, concrete generic owners, namespace
  aliases, nested method comparisons, lists, maps, both result branches,
  optionals, machines, refinements, pending depth, and 160-level recursive enums.
  Required comptime evaluation follows the same explicit method contract.
- Linked debug/release artifacts execute after source removal. Both operators
  preserve ordered operand evaluation, false-prefix and length/tag short-circuiting,
  method failures, and pending bool result failures. Terminal status/output
  agrees with the interpreter and context cleanup succeeds.
- Runtime tests cover independent nested cursors, opaque/malformed ignored
  fields, partial view allocation rollback, pending result cleanup, and 4,096
  recursive levels. Existing cycle, IEEE, shared-child, and malformed suffix
  tests remain green.
- Object tests retain implicit stdlib method reachability, deterministic bytes,
  checked target signatures, and no structural fallback when metadata is missing.
  Compiler phase suites and all 589 frontend fixtures pass.

Full native inventory and platform CI remain separate acceptance gates in
`docs/active/native_acceptance_audit.md`.
