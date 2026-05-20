# JsonTree Decoder Staging Notes

This note records the JSON handoff problem found while staging a typed decoder
over the self-hosted `JsonTree` parser, plus the current resolution.

## Current Shape

The staged layer replaced the old `JsonValue` decoder shape with a native
`JsonTree` decoder:

```jett
function json_decode_tree_reflected[T](view raw: JsonTree) returns result[T, string]:
    ...
```

It uses the stdlib `json_tree_*` accessors instead of Rust-backed `json.*` raw
accessors, then feeds typed values into `TypeConstruction` for structs,
bitfields, and enum payloads.

## What Worked

- `json_tree_parse(raw)` handles scalar, array, and object trees.
- `json_tree_*` traversal helpers work for kind checks, field/index lookup,
  lengths, keys, and scalar casts.
- Direct scalar decoding through a generic helper body works when the caller has
  a `JsonTree` value.
- `json_tree_parse_reflected[T](raw)` now covers nested structs, serialize-name
  mapping, lists, maps, sets, optionals, results, secrets, refinements, enums,
  and bitfields in the run-pass suite.

## Original Failure

Two wrapper/decoder shapes caused interpreter stack overflows:

- A generic wrapper that parsed and decoded in one function:
  `json_tree_parse_reflected[T](raw: string)`.
- A reflected record decoder over `JsonTree` that used `type.fields[T]()` plus
  `TypeConstruction`, analogous to the working `JsonValue` decoder.

The same parser and accessors pass when exercised directly in run-pass fixtures,
so the likely issue is not the JSON scanner itself. The failure appears around
the interaction of recursive user enum values (`JsonTree`), generic reflected
decoding, and/or construction inside the comptime interpreter.

## Resolution

The overflow was reproduced on the normal `run_file` path with compound
`JsonTree` parsing before reflected decoding was involved. Verify blocks already
ran on an explicit larger stack; runtime `main` did not. Running `main` through
the same explicit stack strategy removed that blocker, and the reflected tree
decoder now passes fixture coverage.

## Follow-Up Resolution

The raw bridge is no longer Rust-backed in `jett_comptime`: `json.parse_raw`
delegates to `json_tree_parse`, raw accessors dispatch native `JsonTree` values
through trusted stdlib hooks, and `JsonValue` is source-compatible with the
stdlib `json.JsonTree` enum. In stdlib-loaded code that compatibility now flows
through a root source alias; only bootstrap/no-stdlib fallback still uses the
legacy primitive.

The stable boundary is now:

- self-hosted `JsonTree` parse/serialize/traversal in `.jett`;
- typed `json_tree_parse_reflected[T]` over `JsonTree`;
- public `json.parse[T]` delegates to the `JsonTree` reflected path for typed
  targets;
- public `json.parse[JsonValue]` and `json.parse_raw` preserve the `JsonValue`
  spelling but execute on native `JsonTree` values.

Long term, `JsonValue` should become a compatibility spelling for the native
`JsonTree` representation. See `/docs/active/json_value_transition_plan.md`.
