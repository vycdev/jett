# State Machine JSON Contract

Status: partially implemented.

Jett now has checked reflection for state-machine kind tags, declared states,
state payload fields, transition edges, active state values, and active payload
field reads. JSON serialization now uses that reflection to emit the envelope
shape below. JSON parsing still remains blocked until reflected construction for
machine values has an equally explicit checked path.

## Current Invariant

- `json.serialize` and `json.serialize_public` accept bare `Machine` and
  `Machine at state` targets, including nested machine fields inside structs
  and containers, when all serialized payload fields are JSON-compatible.
- `json.parse` and `json.parse_exact` still reject bare `Machine` and
  `Machine at state` targets, including nested machine fields inside structs
  and containers.
- Machine reflection is available through `type.machine_layout[T]()`,
  `type.machine_states[T]()`, and `type.machine_transitions[T]()`.
- Value-level machine reflection is available through
  `type.machine_state_value[T](view value)` and
  `type.machine_field_value[T, U](view value, view field)`, which is enough for
  future serializers to discover the active state and payload values without
  dynamic field-name strings.
- Reflection metadata uses ordinary field names (`states`, `edges`, `source`,
  `target`) instead of reserved state-machine syntax tokens.

## Preferred Wire Shape

Use an envelope object:

```json
{
  "state": "logged_in",
  "payload": {
    "user_id": "alice"
  }
}
```

Reasons:

- It avoids collisions between a state payload field and the reserved state tag.
- It gives agents one canonical shape for every machine, including unit states.
- It keeps future metadata such as schema versioning or machine namespace out of
  the payload object.
- It lets exact parsing validate envelope keys separately from payload keys.

Unit states should serialize with an empty payload object:

```json
{ "state": "guest", "payload": {} }
```

## Parse Semantics

For `json.parse[Machine]`, any declared state tag is accepted and the result is
the bare machine value at that runtime state.

For `json.parse[Machine at state]`, the input must carry exactly that state tag.
Returning a different state would violate the type annotation, so it should be a
parse error, not a later transition check.

For `json.parse_exact`, exactness should apply at two levels:

- the envelope may contain only `state` and `payload`,
- the payload may contain only fields declared on the tagged state, using each
  field's JSON `serialize_name`.

The lenient `json.parse` path may ignore unknown envelope keys and unknown
payload keys, matching the existing struct policy, but it must not ignore an
unknown or missing `state` tag.

## Serialize Semantics

`json.serialize[Machine]` and `json.serialize[Machine at state]` emit the same
envelope shape. The state-qualified type is a static precision fact, not a
different wire format.

`json.serialize_public` projects state payload fields by the same public record
rules used for structs:

- omit secret-bearing payload fields that can be projected away,
- reject secret wrappers and secret-bearing enum payloads that cannot be
  projected through record fields,
- preserve the state tag so receivers know which payload schema applies.

## Transition Edges

Transition edges are reflection metadata, not wire validation. Parsing an
enveloped state-machine value should validate that the state exists and that the
payload matches that state's fields. It should not require proof that a previous
state could transition to the parsed state, because JSON documents do not carry
history.

If a future protocol wants transition-aware messages, that should be a separate
message type or generated event shape, not ordinary `json.parse[Machine]`.

## Open Questions

- Should the envelope include a machine type name for debugging, or would that
  make refactors too brittle?
- Should `payload` be required for unit states, or can `{ "state": "guest" }`
  be accepted by lenient parse?
- Should machine JSON be opt-in per machine declaration, or enabled for every
  machine whose payload fields are JSON-compatible?
- How should a future schema migration story rename states or payload fields
  without weakening the one-canonical-spelling rule?
