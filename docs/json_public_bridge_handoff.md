# JSON Public Bridge Handoff

This note captures the remaining decision before the reflected `.jett`
implementation can own public names such as `json.parse[T](raw)` and
`json.serialize_public[T](view value)` without losing the compiler-owned policy
gate around those calls.

## Current State

- `stdlib/json.jett` now declares `namespace json`.
- The implementation hooks still use prefixed names:
  - `json.json_serialize_reflected[T](view value)`
  - `json.json_serialize_public_reflected[T](view value)`
  - `json.json_decode_reflected[T](raw)`
  - `json.json_parse_reflected[T](raw)`
- The module also declares public wrapper names:
  - `json.parse[T](raw)`
  - `json.serialize[T](view value)`
  - `json.serialize_public[T](view value)`
- The public `json.parse`, `json.serialize`, and `json.serialize_public` names
  are still compiler-known builtins for typechecking and call dispatch.
- The interpreter delegates `json.parse[T]`, `json.serialize[T]`, and
  `json.serialize_public[T]` through the internal reflected hook names only
  when those registry entries are trusted compiler-shipped stdlib functions.
  The old typed Rust fallback has been removed; Rust remains the substrate for
  raw `JsonValue` parsing and accessors.
- The typechecker enforces policy for those public names:
  - `json.parse[T]` must have one type argument and returns `result[T, string]`.
  - non-string JSON map keys are rejected.
  - serialization requires `view` for non-copy compound values.
  - full serialization rejects secret-containing values.
  - public serialization rejects a top-level secret wrapper and omits secret
    fields in the implementation.
- Qualified user functions now typecheck and run. Compiler-owned builtins win
  over ordinary functions with the same dotted name, matching the interpreter's
  hardcoded dispatch order.

That split is intentional for now: ordinary user or stdlib code should not
silently replace the security-sensitive public JSON bridge.

## The Tension

Jett wants stdlib code to be readable and auditable, especially for agent use.
JSON is an excellent target for that because reflection now exposes enough
metadata to walk and construct most values.

But public JSON functions are also policy boundaries. If `json.parse` became
an ordinary user-resolved function too early, we could accidentally lose:

- handled-result requirements,
- secret output checks,
- `view` requirements,
- map-key restrictions,
- eventual refinement and construction validation guarantees,
- the clear distinction between full serialization and public projection.

## Options

### 1. Keep Public JSON Rust-Backed

Keep `json.parse`, `json.serialize`, and `json.serialize_public` in Rust until
the module system has visibility/export rules and namespace-qualified types.

This is safest, but it delays the primary goal: a readable stdlib JSON
implementation.

### 2. Let `namespace json` Functions Override Builtins

Define ordinary stdlib functions named `parse`, `serialize`, and
`serialize_public`, then make user functions win over builtins for dotted
calls.

This remains too risky:
the public JSON names would stop being compiler-owned policy boundaries before
the language has a way to express all of those policies in ordinary function
types.

### 3. Compiler-Owned Signature, Stdlib-Owned Body

Keep the public names as compiler-known policy gates, but delegate their runtime
body to trusted staged stdlib functions where possible.

For example:

- Typechecking `json.parse[T](raw)` remains hardcoded.
- Runtime execution of `json.parse[T](raw)` can parse raw JSON and call
  `json.json_decode_reflected[T](raw_value)` or directly call
  `json.json_parse_reflected[T](raw)`.
- Typechecking `json.serialize_public[T](view value)` remains hardcoded.
- Runtime execution of `json.serialize_public[T](view value)` can call
  `json.json_serialize_public_reflected[T](view value)`.
- Typechecking `json.serialize[T](view value)` remains hardcoded, including the
  secret-containing-type rejection.
- Runtime execution of `json.serialize[T](view value)` can call
  `json.json_serialize_reflected[T](view value)` after the compiler policy gate.

This keeps the security/ergonomics contract stable while moving implementation
logic into `.jett`.

## Recommendation

Use option 3 as the next implementation stage.

The public bridge should be treated like a compiler-checked facade over
stdlib-owned implementation. That gives us readable JSON code without pretending
that ordinary Jett functions can yet express every policy attached to the
public JSON surface.

Suggested sequence:

1. Keep the Rust-backed `json.parse_raw` and `JsonValue` accessors.
2. Change the interpreter's public `json.serialize_public[T]` path to delegate
   to `json.json_serialize_public_reflected[T]` when the stdlib function is
   registered. Done.
3. Keep typechecker policy for `json.serialize_public[T]` unchanged. Done.
4. Change the interpreter's public `json.parse[T]` path to delegate to
   `json.json_parse_reflected[T]` after preserving public-facing error
   expectations while retaining useful field-path context. Done.
5. Keep typechecker policy for `json.parse[T]` unchanged. Done.
6. Add `json.json_serialize_reflected[T]` and delegate full
   `json.serialize[T]` for non-secret-containing types, while keeping secret
   rejection as compiler/typechecker policy and a defensive interpreter guard.
   Done.
7. Add parity tests for nested structs, aliases/refinements, enums, bitfields,
   bytes, secrets, optionals, results, lists, sets, and `map[string, V]`.
   Done for the representative bridge set.
8. Add public wrapper names in `stdlib/json.jett`, but keep compiler-owned
   builtin precedence in the typechecker and interpreter. Done.
9. Route the compiler-owned interpreter bridge through the internal reflected
   hook names rather than through public wrapper names. This keeps public
   wrappers readable in source while avoiding any future ambiguity between
   trusted compiler-shipped stdlib functions and user/project `namespace json`
   declarations. Done.
10. Remove the old typed Rust parse/serialize fallback from public JSON calls.
    Public typed JSON now depends on the stdlib reflected hooks; raw JSON
    primitives stay Rust-backed. Done.
11. Require the bridge target to be a trusted compiler-shipped stdlib registry
    entry, not just any function with the same qualified name. Done for the
    interpreter.
12. Later, after visibility/export and policy-bearing stdlib declarations exist,
   reconsider whether `parse`, `serialize`, and `serialize_public` can become
   ordinary exported functions.

## Open Questions

- Should public JSON builtins continue delegating through callable stdlib
  wrapper names, or should there eventually be an internal, non-user-callable
  hook name once visibility exists?
- Should compile-time and future runtime/codegen use the same delegation path,
  or should codegen lower public JSON calls differently?
- Should public parse keep the reflected decoder's richer field-path context as
  the stable user-facing contract, or should it exactly preserve the older
  Rust bridge messages?
