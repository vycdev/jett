# State Machine Type Model

Status: implemented for the checked type model, interpreter execution,
reflection metadata, namespace-qualified use, branch narrowing, and JSON
parse/serialize. Exact-state-only, local-variable-only branch narrowing is
established; other future policy questions remain open.

Jett's design treats state machines as a core language feature. This note now
records the implemented model and the remaining policy boundaries so future
work does not accidentally reintroduce one-off interpreter lookup rules.

## Current State

Implemented:

- The parser accepts `machine Name:` declarations with `states:` and
  `transitions:` blocks.
- The parser has a distinct `TypeExpr::StateQualified(base, state, span)` form
  for type-position `Machine at state`, including signatures, fields, generic
  type arguments, and local variable declarations.
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
- A positive `if value at state:` check narrows a bare or state-qualified
  machine variable to `Machine at state` for that branch, so state payload
  fields and transitions are available only under a visible guard. Narrowing is
  permanently limited to a bare local variable; guards over field paths such as
  `holder.current at logged_in` do not narrow the later field access.
  Reassigning that narrowed local to another state inside the branch is rejected
  rather than widening the local in place; `state_machine_narrowed_assignment_to_other_state.jett`
  pins this conservative fact lifetime.
- For bare machines, an `if` / `else if` chain that excludes all but one
  declared state narrows later branches, including `else if` branches guarded
  by unrelated conditions, and the final `else` branch to that single remaining
  state. Other negative branches remain opaque until the branch facts prove a
  specific state. Facts from different machine variables do not combine into a
  narrowed owner; `state_machine_cross_variable_guard_no_narrowing.jett` pins
  that a guard on one local does not expose payload fields on another local of
  the same machine type.
- For `if not (value at state):`, the immediate `else` branch narrows to the
  checked state. For bare two-state machines, the guarded branch also narrows
  to the other declared state. Multi-state negative guarded branches remain
  opaque. This is the permanent exact-state-only policy, not a staging gap:
  negative guards do not create implicit union-state types.
- State-specific payload field access is checked through ordinary field access
  on `Machine at state` values; bare `Machine` values do not expose payload
  fields.
- State-qualified values can flow into bare `Machine` expectations, which
  allows APIs to erase state and regain precision through explicit `at` guards.
  Bare `Machine` values do not flow back into `Machine at state` parameters
  without a visible guard, even when their construction site was precise; this
  keeps erasure explicit at API boundaries.
- `export machine` is parsed and feeds namespace visibility, so exported
  namespaced machines can be used through qualified names and function-local
  namespace aliases.
- The comptime interpreter can register machines, construct values with
  `MachineName(state, ...)`, check `value at state`, and run
  `MachineName.transition(value, target, ...)`.
- The comptime interpreter can read state payload fields from machine values,
  so `verify` execution matches the checked `Machine at state` field model.
- Reflection reports machines with `TypeInfo.kind == "machine"` and
  `TypeKind.machine_type`, and state-qualified machine values with
  `TypeInfo.kind == "machine_state"` and `TypeKind.machine_state_type`.
  `type.machine_layout[T]()`, `type.machine_states[T]()`, and
  `type.machine_transitions[T]()` expose checked state payload fields and legal
  transition edges. Reflected machine layouts use `states` and `edges`; edges
  use `source` and `target` field names to avoid reserved syntax tokens.
- JSON compiler policy now allows `json.serialize`, `json.serialize_public`,
  `json.parse`, and `json.parse_exact` for `Machine` and `Machine at state`
  targets through the explicit state/payload envelope when every payload field
  is JSON-compatible.
- Interpreter unit tests cover construction, valid transitions, rejected
  invalid transitions, and `at` checks through hand-built AST modules.

Future policy work:

- Design any future machine JSON policy annotations and schema migration
  support, especially state renames, without weakening the
  one-canonical-spelling rule. The current JSON envelope contract is recorded
  in `state_machine_json_contract.md`.

## Why This Was Not A Namespace Patch

The namespace question and the machine type question are coupled, but the type
question was more fundamental.

A small namespace-only patch could have made
`billing.Payment.transition(...)` work in the interpreter while still leaving
the checker unable to prove that:

- `Payment at pending` is a real type,
- `Payment.transition(pay, authorized, ...)` is legal from the current state,
- target-state payload fields are complete and correctly typed,
- a function requiring `Payment at captured` cannot receive `Payment at pending`,
- fields only available in one state cannot be read from another state.

That would have made the surface look stronger than it was, which is exactly
the kind of mismatch Jett is trying to avoid for agent-written code.

## Implemented Shape

The type layer uses explicit machine records, not a stringly side table.

Representation:

```text
Type::Machine(MachineId)
Type::MachineState { machine: MachineId, state: MachineStateId }
```

The checked machine metadata includes:

- canonical qualified machine name,
- ordered states,
- per-state payload fields,
- declared transition edges keyed by per-machine state ids,
- namespace/export visibility through the existing resolver policy,
- reflection metadata for serialization policy. The current reflection slice
  exposes the high-level `machine` and `machine_state` kind tags plus checked
  state payload and transition edge metadata.

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
   including local variable declarations, and remains distinct from
   expression-level `expr at state`.
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
   not narrow arbitrary expressions, but a positive `if name at state:` guard
   narrows that variable in the guarded branch. The compile-fail fixture
   `state_machine_field_path_guard_no_narrowing.jett` pins the current
   field-path boundary.
7. Typecheck state-specific payload field access. Done:
   - `Machine at state` values expose only that state's payload fields,
   - bare `Machine` values remain opaque,
   - fields from other states are rejected by ordinary member diagnostics.
8. After the checked model exists, add namespace fixtures. Done:
   - qualified machine construction,
   - `use`-alias machine construction,
   - duplicate-leaf machines,
   - transition error diagnostics with canonical names.
9. Add branch-local state narrowing. Done:
   - `if session at logged_in:` narrows `session` to `Session at logged_in` in
     that branch,
   - for bare machines, later `else if` branches and a final `else` branch
     narrow when the preceding `if` / `else if` chain has excluded every other
     declared state for the same local variable,
   - for `if not (session at logged_in):`, the immediate `else` branch narrows
     back to `logged_in`, and two-state bare machines also narrow the guarded
     branch to the only other state,
   - state-qualified values satisfy bare machine expectations,
   - bare machine values require an explicit `at` guard before they satisfy a
     state-qualified parameter,
   - narrowed bare machine values can call checked transitions.
   - assigning a narrowed local to a different state is rejected rather than
     silently widening the branch fact.
10. Add machine reflection. Done:
    - `type.info[Machine]()` reports kind `machine`,
    - `type.info[Machine at state]()` reports kind `machine_state`,
    - `type.kind_tag` exposes structured `machine_type` and
      `machine_state_type` tags,
    - `type.machine_layout`, `type.machine_states`, and
      `type.machine_transitions` expose state payload fields and transition
      edges with reserved-safe field names,
    - `type.machine_state_value` exposes the active state metadata for a
      concrete machine value, and `type.machine_field_value` reads active-state
      payload values through reflected `TypeField` metadata,
    - non-machine top-level types intentionally return empty machine metadata
      from those shape-specific probes, so generic reflection code can inspect
      `type.kind_tag` and then choose a shape without adding effect handling.
11. Add machine JSON through reflected construction. Done:
    - `json.serialize` / `json.serialize_public` emit the explicit
      state/payload envelope for bare and state-qualified machines,
    - `json.parse` / `json.parse_exact` consume the same envelope for bare and
      state-qualified machines,
    - wrapper and container traversal descends into machine payload fields and
      still rejects any payload type that has no JSON decoding.

## Open Questions

- Which future standard APIs should intentionally erase state to bare
  `Machine`, and which should preserve a precise `Machine at state` type? The
  current source-level rule is explicit: signatures that mention bare `Machine`
  erase state, and callers must use an `at` guard to pass the value back into an
  exact-state parameter.
- Should machine declarations eventually support explicit JSON policy
  annotations or state-rename migration metadata? The default envelope is now
  enabled for every machine whose payload fields are JSON-compatible.
- Should transition effects eventually become generated stdlib-like functions,
  or should the current compiler-checked static method surface remain the
  canonical spelling?

## Recommendation

Keep new machine features tied to checked machine owners rather than interpreter
lookup rules. Branch facts remain a narrow state-machine guard feature. Expose
only exact single-state facts that make payload fields locally unambiguous;
broader flow-sensitive type work must not implicitly introduce multi-state set
facts.
