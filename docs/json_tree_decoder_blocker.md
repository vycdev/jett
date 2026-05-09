# JsonTree Decoder Blocker

This note records the next JSON handoff problem found while staging a typed
decoder over the self-hosted `JsonTree` parser.

## Attempted Shape

The intended next layer mirrors the current `JsonValue` decoder:

```jett
function json_decode_tree_reflected[T](view raw: JsonTree) returns result[T, string]:
    ...
```

It would use the stdlib `json_tree_*` accessors instead of Rust-backed
`json.*` raw accessors, then feed typed values into `TypeConstruction` for
structs, bitfields, and enum payloads.

## What Worked

- `json_tree_parse(raw)` handles scalar, array, and object trees.
- `json_tree_*` traversal helpers work for kind checks, field/index lookup,
  lengths, keys, and scalar casts.
- Direct scalar decoding through a generic helper body works when the caller
  already has a `JsonTree` value.

## What Failed

Two wrapper/decoder shapes caused interpreter stack overflows:

- A generic wrapper that parsed and decoded in one function:
  `json_tree_parse_reflected[T](raw: string)`.
- A reflected record decoder over `JsonTree` that used `type.fields[T]()` plus
  `TypeConstruction`, analogous to the working `JsonValue` decoder.

The same parser and accessors pass when exercised directly in run-pass fixtures,
so the likely issue is not the JSON scanner itself. The failure appears around
the interaction of recursive user enum values (`JsonTree`), generic reflected
decoding, and/or construction inside the comptime interpreter.

## Recommended Next Investigation

1. Add a Rust unit test in `jett_comptime` that calls a minimal generic stdlib
   function taking `view JsonTree` and returning `T`.
2. Minimize from scalar `T = string` to struct `T = Profile` to find whether the
   overflow begins at generic dispatch, `type.fields[T]()`, `TypeConstruction`,
   or recursive `JsonTree` value cloning.
3. Only after that, reintroduce `json_decode_tree_reflected[T]` in stdlib.

Until this is isolated, keep the stable boundary as:

- self-hosted `JsonTree` parse/serialize/traversal in `.jett`,
- existing typed `json.parse[T]` decoder over Rust-backed `JsonValue`.
