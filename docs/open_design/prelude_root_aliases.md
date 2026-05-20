# Prelude and Root Alias Design

Jett now has namespaced exports and a narrow source-level root alias mechanism
for the bundled stdlib `JsonValue` compatibility spelling. This note records
that staged design and the broader prelude/root-alias questions that remain
open.

This matters because `stdlib/json/` can now express:

```jett
namespace json

export type JsonValue = JsonTree
```

That gives users `json.JsonValue`, while the implemented root export:

```jett
export root type JsonValue = json.JsonTree
```

makes the legacy bare spelling visible in ordinary stdlib-loaded source. The
remaining compiler-owned piece is the bootstrap/no-stdlib fallback primitive,
not the normal stdlib-loaded name.

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

The current narrow implementation does not require a separate
`stdlib/prelude.jett` file yet: the compiler allowlists
`export root type JsonValue = json.JsonTree` inside the compiler-shipped
`stdlib/json/` fragments. A future prelude file remains the broader
organization question if more root aliases are ever allowed.

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
- takes precedence over the legacy built-in `JsonValue` primitive when the
  bundled stdlib alias is present; the primitive remains a fallback only when
  the alias is absent,
- visible to completion/hover as a prelude alias,
- reflected as an alias when named through the source alias,
- does not change compiler-owned trust for JSON bridge hooks.

`JsonValue` has moved to a source-owned stdlib root alias in normal
stdlib-loaded code, while `TypePrimitive.json_value_type` remains only as a
bootstrap/no-stdlib fallback during the transition.

## Implementation Staging Notes

The first narrow slice is implemented for `JsonValue`. It deliberately does not
remove the compiler-owned `JsonValue` primitive fallback in the same change.

The important staged invariant is:

- Done: `export root type JsonValue = json.JsonTree` makes the compatibility
  spelling visible as source-owned stdlib API.
- Done: in stdlib-loaded code, `JsonValue` resolves and reflects through the
  root alias rather than through the legacy primitive.
- Done: `json.JsonValue` remains the ordinary namespaced alias whose reflection
  is an alias to `json.JsonTree`.
- Done: the extra compiler-owned `JsonValue -> json.JsonTree` compatibility
  table was removed; normal compatibility now comes from the stdlib root alias.
  User enums named `JsonTree` remain unrelated.

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
5. Done: switch stdlib-loaded `JsonValue` reflection to alias metadata while
   keeping the legacy primitive fallback for bootstrap/no-stdlib contexts.

Required tests for that first slice:

- parser accepts `export root type JsonValue = json.JsonTree`,
- parser or resolver rejects non-type `export root` items and project-file
  `export root type`,
- bare `JsonValue` is visible from ordinary source without relying on namespace
  leaf leakage,
- `prelude.JsonValue` is explicitly rejected; the compatibility alias is the
  bare root name `JsonValue`, not a `prelude` namespace member.
- ordinary namespaced exports still do not leak flat names,
- `JsonValue` and `json.JsonTree` assignment/container compatibility still
  works,
- `type.info[JsonValue]()` and `type.info[json.JsonValue]()` both report alias
  metadata for `json.JsonTree`,
- completions include the root `JsonValue` alias while private stdlib JSON hooks
  remain hidden.

Status: parser, resolver, stdlib declaration, completion, compatibility, and
reflection-staging coverage are in place for `JsonValue`. The remaining design
work is broader prelude policy: whether more root aliases should ever exist, how
they are documented in generated API references, and when the legacy
`TypePrimitive.json_value_type` compatibility tag can be retired.
