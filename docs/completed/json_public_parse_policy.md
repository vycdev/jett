# JSON Public Parse Policy

This note records the current recommendation for how `json.parse[T]` should
interact with `secret[T]` and `json.serialize_public[T]`.

## Context

Jett has two different JSON concerns:

- Full typed input: `json.parse[T](raw)` builds a value of `T` from an input
  string. If `T` contains `secret` fields, parsing is allowed because input is
  not an exposure boundary. The resulting value remains secret-typed.
- Public output: `json.serialize_public[T](view value)` emits only non-secret
  fields. This is an exposure boundary, so it must avoid reading or writing
  secret data.

Those APIs are intentionally not symmetric. A public JSON object emitted from a
secret-containing type may not contain enough data to reconstruct the original
type, and inventing missing secrets would be unsound.

## Options

### 1. No Public Parse API

Keep only `json.parse[T](raw)` for typed input. It parses all required fields,
including secret fields, and preserves secret typing. Public output remains an
output-only projection.

Callers that want to parse public data should define a public DTO without secret
fields, or parse into a type whose secret fields are optional and explicitly
present in the input as `null`/`some` according to ordinary rules.

Pros:

- No silent defaults for required secrets.
- No ambiguity about whether secret keys are ignored, rejected, or required.
- Keeps the security story simple: parse is not an exposure boundary; serialize
  is.

Cons:

- `serialize_public[T]` is not a round-trip partner for `parse[T]` when `T`
  contains required secret data.
- Users may need small public DTOs for API responses.

### 2. `json.parse_public[T]` Rejects Secret-Containing Targets

Add a public parse API, but make it reject any target type containing secret
data. It would be equivalent to `json.parse[T]` for fully public targets, with a
clearer name at call sites where public input is expected.

Pros:

- No accidental reconstruction of partial secret-containing values.
- Gives codebases a searchable marker for public input boundaries.

Cons:

- It is mostly a naming/audit API, not a capability unlock.
- It adds another JSON entry point before the stdlib implementation has settled.

### 3. `json.parse_public[T]` Omits Secret Fields

Parse non-secret fields and somehow fill or ignore secret fields.

Pros:

- Superficially symmetric with `serialize_public[T]`.

Cons:

- Required secret fields have no principled default.
- Ignoring present secret keys can hide input mistakes.
- Rejecting present secret keys but defaulting absent ones creates confusing
  partial construction semantics.
- This drifts toward structural projection without a real projection type.

## Recommendation

Do not add `json.parse_public[T]` yet.

For now, the rule should stay:

- `json.parse[T]` is full typed input and may construct secret fields.
- `json.serialize[T]` rejects secret-containing values.
- `json.serialize_public[T]` omits secret-containing fields without reading
  them.

If a future audit-oriented public parse API is useful, prefer option 2: reject
secret-containing targets outright. Avoid option 3 unless the language first
gains an explicit public projection type, because partial construction of `T`
would make required secret fields look initialized when they are not.

## Implications For Stdlib JSON

The current reflected decoder prototype should keep mirroring `json.parse[T]`:
decode secret fields as required fields and assign inner values into the secret
wrapper. The reflected serializer prototype should remain public-only unless a
separate audited declassification story is designed.
