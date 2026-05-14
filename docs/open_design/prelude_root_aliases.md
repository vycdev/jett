# Prelude and Root Alias Design

Jett now has namespaced exports, but it does not yet have a source-level way for
stdlib code to introduce an unqualified root name such as `JsonValue`.

This matters because `stdlib/json.jett` can now express:

```jett
namespace json

export type JsonValue = JsonTree
```

That gives users `json.JsonValue`, but the legacy spelling is still bare
`JsonValue`. Today that bare name is compiler-owned compatibility metadata, not
ordinary stdlib source.

## Requirements

- Keep exported namespaced declarations out of the global flat scope by default.
- Preserve canonical qualified names such as `json.JsonTree`.
- Let the stdlib provide a small, deliberate set of unqualified convenience
  names.
- Avoid a syntax that user/project files can abuse to pollute global scope.
- Keep compiler trust separate from source visibility. A prelude export should
  make a name visible; it should not make an implementation trusted.
- Make the behavior easy for tools and LLMs to discover.

## Option A: Hardcoded Compiler Prelude

The compiler keeps a table of root names:

```text
JsonValue -> json.JsonTree
```

Pros:

- simple and safe,
- good for compatibility aliases,
- no new syntax.

Cons:

- keeps language-facing names in Rust,
- does not scale to ordinary stdlib conveniences,
- repeats the problem that `JsonTree` is trying to escape.

## Option B: `stdlib/prelude.jett`

Add a compiler-shipped prelude module that may export root-level aliases:

```jett
namespace prelude

export root type JsonValue = json.JsonTree
```

Only compiler-shipped stdlib files may use `export root`. Project files cannot.
The driver loads these root exports after stdlib declarations and before user
files.

Pros:

- moves compatibility spellings into source,
- keeps root pollution centralized and auditable,
- gives tooling one place to show prelude names,
- keeps ordinary namespace exports private to their namespace.

Cons:

- adds a new export mode,
- needs resolver/typechecker/interpreter/LSP support,
- must carefully reject project-defined root exports.

## Option C: File-Local `use prelude`

Keep stdlib names qualified by default and require users to opt into prelude
imports:

```jett
use prelude
```

Pros:

- explicit at each file,
- no global namespace mutation.

Cons:

- Jett currently keeps `use` inside functions/blocks, not file scope,
- every file needs boilerplate for names that are meant to feel built-in,
- compatibility with old bare `JsonValue` still needs a migration bridge.

## Recommendation

Prefer Option B, but stage it narrowly.

The first implementation should support only compiler-shipped stdlib root type
aliases. It should not allow root functions, variables, or project-defined root
exports yet.

Suggested first syntax:

```jett
export root type JsonValue = json.JsonTree
```

Staging rules:

- valid only in compiler-shipped stdlib files,
- valid only for `type` aliases in the first pass,
- inserted into root type lookup after built-in primitives and before user
  declarations,
- visible to completion/hover as a prelude alias,
- reflected as an alias when named through the source alias,
- does not change compiler-owned trust for JSON bridge hooks.

`JsonValue` can then move from a compiler-seeded compatibility alias toward a
source-owned prelude alias, while keeping `TypePrimitive.json_value_type` for one
compatibility stage.

## Implementation Staging Notes

The first narrow slice is implemented for `JsonValue`. It deliberately does not
remove the compiler-owned `JsonValue` primitive behavior in the same change.

The important staged invariant is:

- Done: `export root type JsonValue = json.JsonTree` makes the compatibility
  spelling visible as source-owned stdlib API.
- `JsonValue` may still resolve through the legacy compatibility path for one
  release window so `type.info[JsonValue]()` continues to report
  `TypePrimitive.json_value_type`.
- Done: `json.JsonValue` remains the ordinary namespaced alias whose reflection
  is an alias to `json.JsonTree`.
- The `JsonValue -> json.JsonTree` compatibility relation remains restricted to
  the bundled stdlib `json.JsonTree`, not any user enum named `JsonTree`.

Concretely, a safe implementation should:

1. Done: parser support for `export root type`, with parser rejection for
   non-type `export root` items.
2. Done: resolver support is stdlib-only, type-alias-only, and allowlisted to
   `JsonValue`; project-file root aliases are rejected.
3. Done: root exports register as root-scope names rather than
   namespace-private or namespace-public names; otherwise namespace visibility
   diagnostics would treat `JsonValue` as an external `prelude.JsonValue`
   access.
4. Keep source visibility separate from trusted origin. A root alias should not
   make any function implementation trusted.
5. Done for the first slice: keep the legacy `JsonValue` reflection behavior
   until a later explicit
   migration decides whether to deprecate or remove
   `TypePrimitive.json_value_type`.

Required tests for that first slice:

- parser accepts `export root type JsonValue = json.JsonTree`,
- parser or resolver rejects non-type `export root` items and project-file
  `export root type`,
- bare `JsonValue` is visible from ordinary source without relying on namespace
  leaf leakage,
- `prelude.JsonValue` behavior is explicitly decided and pinned,
- ordinary namespaced exports still do not leak flat names,
- `JsonValue` and `json.JsonTree` assignment/container compatibility still
  works,
- `type.info[JsonValue]()` and `type.info[json.JsonValue]()` keep their staged
  distinct behavior,
- completions include the root `JsonValue` alias while private stdlib JSON hooks
  remain hidden.

Status: parser, resolver, stdlib declaration, completion, compatibility, and
reflection-staging coverage are in place for `JsonValue`. The remaining design
work is broader prelude policy: whether more root aliases should ever exist, how
they are documented in generated API references, and when the legacy
`TypePrimitive.json_value_type` compatibility tag can be retired.
