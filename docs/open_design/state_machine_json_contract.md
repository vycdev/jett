# State Machine JSON Contract

Status: implemented for parse and serialize; migration annotations remain open.

Jett now has checked reflection for state-machine kind tags, declared states,
state payload fields, transition edges, active state values, and active payload
field reads. JSON serialization emits the envelope shape below, and JSON parsing
consumes the same shape through the checked `type.construct_machine_start[T]`
builder path followed by `type.construct_put` and `type.construct_finish`.

## Current Invariant

- `json.serialize` and `json.serialize_public` accept bare `Machine` and
  `Machine at state` targets, including nested machine fields inside structs
  and containers. Bare targets require every state's serialized payload to
  be JSON-compatible; a qualified target checks only its selected payload.
- `json.parse` and `json.parse_exact` accept bare `Machine` and
  `Machine at state` targets, including nested machine fields inside structs
  and containers. Bare targets require every state's payload to be
  JSON-compatible; a qualified target checks only its selected payload.
- The wire envelope has exactly two canonical keys, `state` and `payload`.
  Serializers do not emit a machine type tag, and exact parsing rejects extra
  envelope keys such as `type`.
- Both parse modes require the envelope to be an object with a string `state`
  tag and an object `payload`; even lenient parsing rejects missing state tags,
  unknown state tags, missing payloads, and non-object payloads.
- Machine reflection is available through `type.machine_layout[T]()`,
  `type.machine_states[T]()`, and `type.machine_transitions[T]()`.
- Value-level machine reflection is available through
  `type.machine_state_value[T](view value)` and
  `type.machine_field_value[T, U](view value, view field)`, which lets
  serializers discover the active state and payload values without dynamic
  field-name strings.
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
unknown or missing `state` tag. It also requires `payload` to be present as an
object, including for unit states, so every machine snapshot has one canonical
envelope shape. The malformed-envelope cases are pinned in
`tests/run_pass/json_parse_machine_envelope.jett`.

Missing optional payload fields should use the same `none` default as struct
decoding. Missing required payload fields should fail.

## Private Map Decoder Boundary

JSON object map keys are exact strings, including transparent aliases that
resolve to string. Non-string map targets, including integer-keyed maps in a
bare machine or the selected state, retain frontend E0343 rejection. This
public domain does not change when a qualified request has an unrelated state
with a non-JSON payload.

`type.machine_states[Machine at state]()` still exposes all declared states.
Top-level aliases remain non-matching aggregate probes with empty metadata;
the source aliases preserved below are field-level TypeInfo identities.
Nested `state.fields` loops must therefore have checked type and source-alias
candidates for every declared field. Direct active-state field loops retain
their selected payload candidates. Omitting other-state candidates masked the
old private map decoder's incompatible return: it constructed
`map[string, Val]` even when its actual T had non-string keys.

The selected repair uses a closed, alias-aware key guard before the private
decoder's typed string-map body. Unsupported keys produce a failure rather than
a key cast or a dummy value. Valid public string-map parsing and the canonical
wire envelope are unchanged.

This intentionally changes error precedence for malformed qualified input
that names another declared state's unsupported map payload. If field decoding
is reached, the private unsupported-key error precedes map construction and
the eventual qualified-builder state mismatch. Pre-change reference
characterization showed empty and nonempty map objects reaching that final
state mismatch; a numeric payload reached the map's expected-object error.
Lenient parsing now rejects unsupported keys before that object-kind check.
Missing required-field lookup still runs before this decoder, and exact parsing
retains its separate object/unknown-field validation before decoding. Fresh
reference characterization of the regression Session records
`cached.entries: JSON object maps require string keys, got int64` for empty
and nonempty other-state map objects. Lenient numeric payloads reach the
same key guard; exact numeric payloads first report
`entries: expected object, got number`. Missing entries still fail the
required-field lookup. All 22 linked qualified-JSON groups passed in
`target/native-json-qualified-driver-final.log`, including these literal
first-error controls in direct and pipeline forms. The later inactive-payload
decoder group also passed separately. The final-source JSON family gate passes
all 39 tests, including the 23 new groups, in
`target/native-json-qualified-json-family-final.log` (438.34 seconds).
The frozen `23ac34c4` complete workspace and doc-tests also pass in
`target/native-workspace-23ac34c4.log`, including all 346 native tests and
182 object obligations. Supported-host acceptance for that revision and
acceptance for later test additions remain independent gates.

This is a bounded private type-correctness repair selected before guard
integration. It does not add an early machine-state gate, filter public
metadata, change global builder behavior, coerce carriers, or admit non-string
public map targets. Migration annotations and broader JSON questions remain
separate.

The private fallback decoder also uses the visible canonical condition
`type.name[T]() == "json.JsonTree"` before raw-tree copying. This reuses
checked static selection, keeping its typed raw return body out of unrelated
inactive callable, interface, and actor field specializations. Ordinary
pure predicate calls are not folded to determine source validity or skip
typechecking. One additional group over those three inactive payloads passes
all four direct JSON APIs for the valid selected state in both runtime
profiles after source removal. It does not broaden the public JSON domain
or establish malformed-input behavior for those unsupported payloads.

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

## Current Decisions

- Do not include the machine type name in the envelope. The static parse target
  already selects the machine owner, and embedding the source name would make
  namespace moves and refactors part of the wire contract.
- Keep machine JSON enabled for every machine whose payload fields are
  JSON-compatible. This matches structs, enums, bitfields, and containers:
  serializability is a property the checker can prove from the shape.
- Treat migration as a future annotation problem, not a parser guess. Payload
  fields already have `serialize "..."`; state rename aliases or machine-level
  schema versions should be designed explicitly before they affect parsing.

## Open Questions

- Should machine declarations eventually support explicit JSON policy
  annotations, such as disabling the default envelope or naming a schema
  version?
- How should a future schema migration story rename states without weakening
  the one-canonical-spelling rule?
