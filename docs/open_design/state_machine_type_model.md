# State Machine Type Model

Jett's design treats state machines as a core language feature, but the current
implementation is still mid-extraction from a parser/interpreter prototype into
a fully checked type-system feature.

This note records the gap so namespace work does not accidentally paper over it
with one-off lookup rules.

## Current State

Implemented:

- The parser accepts `machine Name:` declarations with `states:` and
  `transitions:` blocks.
- The parser has a distinct `TypeExpr::StateQualified(base, state, span)` form
  for type-position `Machine at state`.
- The resolver declares machine names, including namespaced machine names.
- `jett_types` has checked-machine metadata handles and explicit
  `Type::Machine` / `Type::MachineState` variants.
- The typechecker predeclares and finishes machine declarations as named types,
  including duplicate-state checks, transition endpoint checks, and state
  payload field type resolution.
- Machine construction calls are checked for an initial state label and payload
  arity/types, and return the corresponding `Machine at state` type.
- Machine transition calls are checked for source owner/state, declared edge,
  target payload arity/types, and return the target `Machine at state` type.
- `expr at state` is checked as a boolean state test on machine values, with
  diagnostics for non-machine values and states not declared on the machine.
- State-specific payload field access is checked through ordinary field access
  on `Machine at state` values; bare `Machine` values do not expose payload
  fields.
- `export machine` is parsed and feeds namespace visibility, so exported
  namespaced machines can be used through qualified names and function-local
  namespace aliases.
- The comptime interpreter can register machines, construct values with
  `MachineName(state, ...)`, check `value at state`, and run
  `MachineName.transition(value, target, ...)`.
- The comptime interpreter can read state payload fields from machine values,
  so `verify` execution matches the checked `Machine at state` field model.
- Interpreter unit tests cover construction, valid transitions, rejected
  invalid transitions, and `at` checks through hand-built AST modules.

Not implemented:

- Machine reflection and JSON integration are not modeled yet.

## Why This Should Not Be A Namespace Patch

The namespace question and the machine type question are coupled, but the type
question is more fundamental.

A small namespace-only patch could make `billing.Payment.transition(...)` work
in the interpreter while still leaving the checker unable to prove that:

- `Payment at pending` is a real type,
- `Payment.transition(pay, authorized, ...)` is legal from the current state,
- target-state payload fields are complete and correctly typed,
- a function requiring `Payment at captured` cannot receive `Payment at pending`,
- fields only available in one state cannot be read from another state.

That would make the surface look stronger than it is, which is exactly the kind
of mismatch Jett is trying to avoid for agent-written code.

## Target Shape

The type layer likely needs explicit machine records, not a stringly side table.

Possible representation:

```text
Type::Machine(MachineId)
Type::MachineState { machine: MachineId, state: MachineStateId }
```

The checked machine metadata includes, or should include as checker integration
lands:

- canonical qualified machine name,
- ordered states,
- per-state payload fields,
- declared transition edges keyed by per-machine state ids,
- namespace/export visibility through the existing resolver policy,
- reflection hooks later, if state machines become serializable or inspectable.

The base metadata now lives beside struct/enum/actor definitions in
`jett_types`, not only inside the interpreter, and the checker populates it from
source declarations.

## Parser Syntax Gap

Expression syntax already has `expr at state` as a runtime state check.

Type syntax now has a distinct representation for signatures such as:

```jett
function capture(payment: Payment at authorized) returns Payment at captured:
    ...
```

`TypeExpr::StateQualified(base, state, span)` keeps type-level `at` separate
from expression-level `at` and lets the typechecker attach the state identifier
to a known machine owner.

## Staging Plan

1. Add machine definitions to `jett_types` and the type interner. Done:
   `MachineId`, `MachineStateId`, `MachineDef`, `MachineStateDef`,
   `MachineTransitionDef`, `Type::Machine`, and `Type::MachineState`.
2. Add a state-qualified `TypeExpr` form for `Machine at state`. Done:
   `TypeExpr::StateQualified(base, state, span)` is parsed in type positions
   and remains distinct from expression-level `expr at state`.
3. Typecheck machine declarations. Done for the current machine surface:
   - duplicate states,
   - transition endpoints exist,
   - per-state field types resolve,
   - namespace ownership matches other top-level types.
4. Typecheck machine construction. Done:
   - first argument is a declared state,
   - payload fields match that state's declared fields,
   - return type is `Machine at state`.
5. Typecheck transitions. Done:
   - source value has the same machine owner,
   - current state and target state form a declared transition edge,
   - target payload fields match the target state's fields,
   - return type is `Machine at target`.
6. Typecheck `expr at state` as a bool check on machine values. Done. It does
   not narrow branch-local state yet.
7. Typecheck state-specific payload field access. Done:
   - `Machine at state` values expose only that state's payload fields,
   - bare `Machine` values remain opaque,
   - fields from other states are rejected by ordinary member diagnostics.
8. After the checked model exists, add namespace fixtures. Done:
   - qualified machine construction,
   - `use`-alias machine construction,
   - duplicate-leaf machines,
   - transition error diagnostics with canonical names.

## Open Questions

- Should bare machine variables ever have type `Machine`, or should every value
  always carry a known state type?
- Should `expr at state` narrow the type inside an `if`, or remain a pure bool
  operation?
- Should `expr at state` eventually provide a branch-local state-qualified view
  that exposes payload fields inside the guarded branch?
- Should machines participate in reflection and JSON serialization immediately,
  or wait until the core type model is stable?
- Should machine transitions be ordinary static methods, compiler intrinsics, or
  stdlib-like generated functions?

## Recommendation

Implement the checked type model before adding more source-level machine
fixtures. Namespaced machine support should be a consequence of canonical
machine owners, not a separate interpreter lookup rule.
