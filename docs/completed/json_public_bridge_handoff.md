# JSON Public Bridge Handoff

This note records the completed staging choice for public JSON calls: the
compiler still owns the policy-bearing public API, while trusted stdlib `.jett`
functions own the runtime implementation body.

## Current State

- `stdlib/json/` fragments declare `namespace json`.
- Public source-level wrappers are exported for:
  - `json.parse[T](raw)`
  - `json.parse_exact[T](raw)`
  - `json.serialize[T](view value)`
  - `json.serialize_public[T](view value)`
- Those public names remain compiler-known policy gates for typechecking and
  runtime dispatch.
- The interpreter requires trusted compiler-shipped stdlib hooks behind each
  public bridge, then executes the trusted exported public wrapper:
  - `json.parse` requires `json.json_parse_reflected`
  - `json.parse_exact` requires `json.json_parse_exact_reflected`
  - `json.serialize` requires `json.json_serialize_reflected`
  - `json.serialize_public` requires `json.json_serialize_public_reflected`
- Raw JSON access also routes through trusted stdlib `JsonTree` hooks such as
  `json.json_tree_parse`, `json.json_tree_serialize`, `json.json_tree_field`,
  `json.json_tree_index`, and scalar-cast helpers by way of exported public raw
  facade wrappers.
- `json.JsonTree` is the self-hosted raw JSON representation. The former bare
  and namespaced `JsonValue` aliases and legacy compiler-owned fallback have
  since been retired.
- The old typed Rust parse/serialize fallback has been removed from public JSON
  calls. Raw JSON execution in `jett_comptime` now uses native `JsonTree`
  values and trusted stdlib facade hooks rather than a separate Rust JSON
  implementation path.

The split is intentional: ordinary user or project code should not silently
replace the security-sensitive public JSON bridge.

## Policy Still Owned By The Compiler

The typechecker keeps the public JSON contract stable:

- `json.parse[T]` and `json.parse_exact[T]` require one type argument and return
  `result[T, string]`.
- `json.parse[T]` is lenient for unknown object fields.
- `json.parse_exact[T]` rejects unknown object fields recursively for closed
  contracts.
- Non-string JSON map keys are rejected.
- Serialization requires `view` for non-copy compound values.
- Full serialization rejects secret-containing values.
- Public serialization rejects a top-level secret wrapper and omits secret
  fields in the stdlib implementation.

Those checks are not yet expressible as ordinary stdlib function types, so they
remain compiler-owned while the implementation body is readable `.jett` code.

## Completed Sequence

1. Added trusted stdlib hook registration for compiler-shipped modules.
2. Routed `json.serialize_public[T]` through
   `json.json_serialize_public_reflected[T]`.
3. Routed lenient `json.parse[T]` through `json.json_parse_reflected[T]`.
4. Routed full `json.serialize[T]` through `json.json_serialize_reflected[T]`
   while retaining the secret exposure policy gate.
5. Added exported wrapper declarations in `stdlib/json/`, while keeping
   builtin precedence in the typechecker and interpreter.
6. Removed the old typed Rust parse/serialize fallback from public JSON calls.
7. Replaced the raw `JsonValue` runtime path with the self-hosted `JsonTree`
   parser, serializer, traversal helpers, and scalar casts.
8. Added `json.parse_exact[T]` as a second public parse bridge backed by
   `json.json_parse_exact_reflected[T]`.
9. Centralized the public typed JSON bridge hook mapping in `jett_common`.
   Later raw facade hook dispatch was removed, leaving only the raw public-name
   set needed for wrapper precedence and ownership.
10. Switched interpreter bridge execution to trusted exported public wrappers
    while still requiring the trusted private hook table.
11. Added run-pass and driver coverage for typed parse, exact parse, full
    serialization, public serialization, raw facade delegation, completions,
    hover, and trusted-hook spoofing protections. Transitional `JsonValue`
    compatibility coverage was removed when the aliases retired.

## Remaining Follow-Up

- The bootstrap/no-stdlib `JsonValue` primitive fallback and
  `TypePrimitive.json_value_type` tag are retired. See
  `docs/completed/json_value_primitive_tag_retirement.md`.
- Decide whether future codegen should use the same trusted hook table as the
  interpreter or lower public JSON calls through another backend-specific path.
- Keep tightening `JsonTree` parser diagnostics and malformed-input parity as
  the self-hosted parser becomes the only raw JSON implementation path.
- Revisit whether compiler-owned JSON policy gates can become ordinary exported
  stdlib declarations after the module/import/prelude story can represent
  policy-bearing constraints.
