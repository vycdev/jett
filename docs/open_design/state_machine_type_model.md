# State Machine Type Model

Jett's design treats state machines as a core language feature, but the current
implementation is still a parser/interpreter prototype rather than a checked
type-system feature.

This note records the gap so namespace work does not accidentally paper over it
with one-off lookup rules.

## Current State

Implemented:

- The parser accepts `machine Name:` declarations with `states:` and
  `transitions:` blocks.
- The resolver declares machine names, including namespaced machine names.
- `jett_types` has checked-machine metadata handles and explicit
  `Type::Machine` / `Type::MachineState` variants.
- The comptime interpreter can register machines, construct values with
  `MachineName(state, ...)`, check `value at state`, and run
  `MachineName.transition(value, target, ...)`.
- Interpreter unit tests cover construction, valid transitions, rejected
  invalid transitions, and `at` checks through hand-built AST modules.

Not implemented:

- `TypeExpr` has no state-qualified form for `Machine at state`.
- The typechecker does not predeclare, finish, or check machine declarations as
  named types.
- Construction and transition calls are not checked against machine state
  field types or declared transition edges.
- State-specific field access is not modeled.
- Namespace-qualified machine fixtures do not yet run through the same checked
  surface as structs, enums, actors, interfaces, and JSON-facing types.

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
`jett_types`, not only inside the interpreter. The checker still needs to
populate it from source declarations.

## Parser Syntax Gap

Expression syntax already has `expr at state` as a runtime state check.

Type syntax still needs a distinct representation for signatures such as:

```jett
function capture(payment: Payment at authorized) returns Payment at captured:
    ...
```

The parser may need a `TypeExpr::StateQualified(base, state, span)` variant.
That keeps type-level `at` separate from expression-level `at` and lets the
typechecker attach the state identifier to a known machine owner.

## Staging Plan

1. Add machine definitions to `jett_types` and the type interner. Done:
   `MachineId`, `MachineStateId`, `MachineDef`, `MachineStateDef`,
   `MachineTransitionDef`, `Type::Machine`, and `Type::MachineState`.
2. Add a state-qualified `TypeExpr` form for `Machine at state`. Done:
   `TypeExpr::StateQualified(base, state, span)` is parsed in type positions
   and remains distinct from expression-level `expr at state`.
3. Typecheck machine declarations:
   - duplicate states,
   - transition endpoints exist,
   - per-state field types resolve,
   - namespace/export visibility matches other top-level types.
4. Typecheck machine construction:
   - first argument is a declared state,
   - payload fields match that state's declared fields,
   - return type is `Machine at state`.
5. Typecheck transitions:
   - source value has the same machine owner,
   - current state and target state form a declared transition edge,
   - target payload fields match the target state's fields,
   - return type is `Machine at target`.
6. Typecheck `expr at state` as either:
   - a bool check on any `Machine` value, or
   - a refinement/narrowing operation if Jett later supports branch-local state
     narrowing.
7. After the checked model exists, add namespace fixtures:
   - qualified machine construction,
   - `use`-alias machine construction,
   - duplicate-leaf machines,
   - transition error diagnostics with canonical names.

## Open Questions

- Should bare machine variables ever have type `Machine`, or should every value
  always carry a known state type?
- Should `expr at state` narrow the type inside an `if`, or remain a pure bool
  operation?
- Should state payload fields use ordinary field access, or require a
  state-qualified view first?
- Should machines participate in reflection and JSON serialization immediately,
  or wait until the core type model is stable?
- Should machine transitions be ordinary static methods, compiler intrinsics, or
  stdlib-like generated functions?

## Recommendation

Implement the checked type model before adding more source-level machine
fixtures. Namespaced machine support should be a consequence of canonical
machine owners, not a separate interpreter lookup rule.
