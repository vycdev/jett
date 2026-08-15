# JsonValue To JsonTree Transition

Status: completed. `json.JsonTree` is the sole raw JSON type spelling.

## Final State

- Bare `JsonValue` is unknown in every source context.
- `json.JsonValue` is unknown; the stdlib no longer exports the namespaced
  compatibility alias.
- `export root type` is rejected with `E0209` and never enters compiler state.
- The compiler has no `Type::JsonValue`, `TypeInterner::JSON_VALUE`,
  `TypePrimitive.json_value_type`, hidden primitive fallback, or compatibility
  type table.
- Completions and hover advertise only `json.JsonTree` for raw JSON.
- Reflection reports `json.JsonTree` directly as an enum, never as an alias.

All parsing, raw access, serialization, reflection, container traversal, and
reflected construction use the ordinary stdlib `json.JsonTree` enum carrier.
Existing code must migrate both former spellings:

```jett
JsonValue       # retired
json.JsonValue  # retired
json.JsonTree   # canonical
```

The primitive retirement and root alias policy are recorded separately in
[JsonValue primitive tag retirement](json_value_primitive_tag_retirement.md)
and [Root alias policy](root_alias_policy.md).

## Invariants

- Public JSON policy gates use trusted stdlib origin, never type spelling, as
  their authority boundary.
- Raw facade functions accept or return `json.JsonTree` directly.
- User-defined aliases to `json.JsonTree` remain ordinary local or namespaced
  type aliases with normal alias reflection behavior.
- User types named `JsonTree` outside namespace `json` remain unrelated.
- No future compatibility layer should reintroduce an alternate raw JSON
  carrier or globally injected type name.
