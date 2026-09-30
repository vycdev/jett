# Explicit struct equality inside enum payloads

Status: source-language decision accepted on 2026-09-30; implementation remains
open. The user selected exact explicit `Equatable.equals` calls for user structs
nested inside enum payloads. Native parity cannot be declared until the frontend,
interpreter, and linked native behavior enforce this rule.

## Required semantics

Direct user-struct equality already requires an exact explicit two-view
`Equatable.equals` implementation. Enum equality must preserve that same method
boundary, including structs reached through nested enums, lists, map values,
optionals, results, machines, and representation-transparent wrappers. A nested
struct without the required implementation must fail in the frontend. The
compiler must stop at a struct's explicit method rather than recursively compare
its fields or require equality for fields that its method intentionally ignores.
The inequality operator negates the complete equality result.

Existing aggregate comparisons retain variant/state/branch checks, collection
order and length checks, pending-depth distinctions, IEEE float behavior, and
left-to-right short-circuit evaluation. Direct collection and sum equality remains
rejected under the separate E0376 policy. Actor and erased-interface equality
decisions remain separate; this work must not introduce implicit equality for
those types.

## Authoritative current gap

The frontend currently checks only direct struct operands. The interpreter's
enum path delegates to `Value::PartialEq`, which compares nested struct fields
and bypasses explicit methods. Native `equality_layout` rejects user structs.
The iterative runtime traversal already preserves short-circuit ordering for
supported aggregate payloads; its structural record path is also used for
bitfields and must not be confused with source user-struct equality.

## Compiler and runtime handoff

- Frontend validation must walk the enum payload type graph with cycle detection
  and retain the exact checked method identities for every custom leaf. Concrete
  generic instantiations and scoped reflection bodies need the same handoff.
- HIR/MIR must carry typed method targets and preserve their reachability,
  signatures, borrow modes, canonical symbols, and failure cleanup. Codegen must
  not rediscover implementations from source names or field structure.
- The interpreter must traverse nested payloads through its source method
  dispatch, including required comptime evaluation, instead of using structural
  `Value` equality for user structs.
- Native traversal must invoke compiled methods outside the runtime context's
  locked leaf operation. A resumable comparison cursor is a candidate for
  retaining the existing iterative traversal while yielding custom comparisons
  to generated code. Nested comparisons need independent state, and every
  cursor and temporary owner must retire on success, inequality, or failure.
- User structs are opaque equality leaves. A method that ignores a field must
  not inspect, compare, or fail because of that field's payload or debug layout.

## Required verification

Cover missing implementations, both operators, generic owners, namespace aliases,
and methods whose result differs from field equality. Compare interpreter,
comptime, native debug, and native release results for direct and nested payloads,
including lists, maps, optional/result branches, and recursive enums. Verify
operand evaluation once in source order, method short-circuit ordering, pending
depth distinctions, method failures, and owned-value cleanup. Deep finite values
must retain the existing traversal guarantees. Linked artifacts must execute
after source removal, and the full frontend/native inventory must remain green.
