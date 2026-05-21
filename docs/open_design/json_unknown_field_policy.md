# JSON Unknown Field Policy

Typed `json.parse[T]` currently ignores object fields that do not correspond to
reflected `TypeField.serialize_name` entries on `T`.

This is pinned by `tests/run_pass/json_unknown_fields.jett` for both top-level
and nested structs.

## Why It Matters

Unknown fields can mean different things:

- harmless forward-compatible data from a newer producer,
- a user typo such as `display_name` instead of `displayName`,
- an unexpected API response shape,
- data that should be rejected for security or audit reasons.

The language should avoid making LLM-generated parsers silently accept mistakes
when the program expects an exact contract. At the same time, JSON is often used
at compatibility boundaries where ignoring new fields is useful.

## Options

### Option A: Keep Default Parse Lenient

`json.parse[T]` ignores unknown object fields forever.

Pros:

- matches the current behavior,
- good for forward-compatible API clients,
- avoids adding policy syntax.

Cons:

- typos in input fields can be missed,
- exact config parsing requires custom validation,
- less aligned with Jett's bias toward explicit correctness.

### Option B: Make Default Parse Exact

`json.parse[T]` rejects any unknown field.

Pros:

- catches typos and unexpected response shapes early,
- strongly typed parse means "the JSON shape matches the type",
- better for config files and security-sensitive input.

Cons:

- breaks current behavior,
- worse for forward-compatible clients,
- needs a story for intentionally ignored fields.

### Option C: Add Explicit Parse Modes

Keep the current `json.parse[T]` behavior and add an exact variant:

```jett
function parse_exact[T](raw: string) returns result[T, string]
```

Later, field or type annotations could choose policy locally if the language
wants that extra control:

```jett
struct User serialize_unknown_fields "reject":
    id: string
```

Pros:

- preserves compatibility,
- gives exact contract parsing a direct API,
- keeps the public default stable while the stdlib matures.

Cons:

- more API surface,
- LLMs must learn when to choose exact parsing,
- implementing exact parse efficiently needs an object-key comparison helper.

## Recommendation

Prefer Option C for now.

That path is implemented: a private reflected validator checks object keys
against `type.fields[T]()` using `serialize_name`, and the public
`json.parse_exact[T](raw)` wrapper exposes the exact mode while
`json.parse[T]` stays lenient.

Status: implemented. `json.parse_exact[T](raw)` is a compiler-policy public
bridge backed by trusted stdlib hooks, and
`tests/run_pass/json_parse_exact.jett`,
`tests/run_pass/json_parse_exact_container_edges.jett`, and
`tests/run_pass/json_parse_exact_secret_edges.jett` pin exact top-level,
nested, list, map-value, set, result, enum, bitfield, secret-wrapper, and raw
boundary validation. The fixtures also pin raw-target exact parsing for
`json.JsonTree`, bare `JsonValue`, and `json.JsonValue`; exact unknown-field
validation does not inspect inside raw tree targets. The current
`json.parse[T]` behavior remains lenient and is still pinned by
`tests/run_pass/json_unknown_fields.jett`. Current docs describe `parse` as the
compatibility-oriented path and `parse_exact` as the preferred path for config
files, protocol messages, tests, and other closed input contracts.
