# JsonValue To JsonTree Transition

Status: the compiler-owned primitive and bare compatibility alias are retired.
Only the namespaced `json.JsonValue` compatibility alias remains under review.

## Canonical Raw JSON Type

New and migrated Jett source must use:

```jett
json.JsonTree raw = json.parse_raw(text) handle error:
    return fail(error)
```

`json.JsonTree` is an ordinary stdlib enum and the native raw JSON runtime
carrier. Parsing, raw access, serialization, reflection, container traversal,
and reflected construction use that checked type.

## Retired Surfaces

- Bare `JsonValue` is unknown in all source contexts.
- `export root type` is rejected with `E0209`; there is no root alias or prelude
  injection mechanism.
- The compiler has no `Type::JsonValue`, `TypeInterner::JSON_VALUE`,
  `TypePrimitive.json_value_type`, hidden primitive fallback, or compatibility
  type table.
- Completions and hover do not advertise the bare spelling.
- Compatibility/parity fixtures have migrated to `json.JsonTree`.

The historical implementation and primitive retirement are recorded in
[JsonValue primitive tag retirement](../completed/json_value_primitive_tag_retirement.md)
and [Root alias policy](../completed/root_alias_policy.md).

## Remaining Namespaced Alias

The standard library still contains:

```jett
namespace json
export type JsonValue = JsonTree
```

This produces only `json.JsonValue`. It is an ordinary exported namespaced alias
with alias reflection metadata and no special trust or runtime behavior.

The remaining decision is whether to keep that friendly namespaced alias or
remove it so `json.JsonTree` becomes the sole raw JSON spelling. Until that is
decided, compatibility coverage should remain concentrated and general docs and
examples should teach only `json.JsonTree`.

## Invariants

- Public JSON policy gates continue to use trusted stdlib origin, never alias
  spelling, as their authority boundary.
- `json.parse[json.JsonTree]`, `json.parse_exact[json.JsonTree]`,
  `json.serialize[json.JsonTree]`, and `json.serialize_public[json.JsonTree]`
  use the same normal reflected enum path as other supported types.
- Raw facade functions accept or return `json.JsonTree` directly.
- User types named `JsonTree` outside namespace `json` remain unrelated.
- A future removal of `json.JsonValue` must not reintroduce a primitive, root
  alias, compiler compatibility table, or alternate runtime carrier.

## Remaining Work

1. Decide whether `json.JsonValue` remains a friendly namespaced alias or is
   removed in favor of one canonical spelling.
2. If removed, migrate the remaining namespaced-alias fixtures and documentation
   in one change.
3. Keep the trusted-hook and future-backend work independent from public type
   spelling.
