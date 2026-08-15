# JsonValue Primitive Tag Retirement

Status: completed. Compiler-owned `JsonValue` type identity and the bare source
alias have both been retired.

## Final State

- `json.JsonTree` is the native raw JSON representation and canonical source
  spelling.
- `jett_types` has no `Type::JsonValue` or `TypeInterner::JSON_VALUE`.
- `TypePrimitive` has no `json_value_type` variant.
- Direct/no-stdlib reflection does not synthesize JSON meaning for the name
  `JsonValue`.
- Bare `JsonValue` is unknown even when the bundled standard library is loaded.
- Root type aliases are rejected with `E0209` and never enter compiler state.
- `json.JsonValue` remains an ordinary exported namespaced alias to
  `json.JsonTree` for now; it has alias reflection metadata, not primitive
  identity.

Typed JSON parsing, raw access, serialization, reflection, and construction all
operate on the normal `json.JsonTree` enum carrier. There is no compiler-owned
compatibility table or hidden runtime representation for the retired name.

The remaining question—whether the namespaced `json.JsonValue` alias should
also be removed—is tracked in the active JsonTree transition record and does
not reopen the primitive or root-alias decisions.
