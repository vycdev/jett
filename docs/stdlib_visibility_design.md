# Stdlib Visibility and Trusted Origins

This note records the visibility work that blocks a fully ordinary
`stdlib/json.jett` public API.

## Problem

Jett currently has namespaces, but it does not yet have public/private exports
or a trusted-source identity for compiler-shipped stdlib modules.

That matters for JSON because the module needs both:

- public functions such as `json.parse`, `json.serialize`, and
  `json.serialize_public`,
- many helper functions such as `json_decode_record_reflected` and
  `json_stdlib_quote`,
- compiler-owned policy gates around public JSON calls,
- a way for the interpreter to know that a helper came from bundled stdlib code,
  not from a project file with the same qualified name.

The current staging model keeps public JSON names compiler-owned and routes
runtime bodies through internal reflected hook names. That is intentionally
conservative.

## Current Namespace Leakage

Namespaced declarations are registered under qualified names, for example
`json.parse`, but they are also staged with a flat alias in several places so
unqualified calls inside the same namespace keep working.

That is useful during bootstrap, but it is not the long-term export model:

- a helper from `namespace json` can appear as an unqualified global name,
- helpers are public by convention rather than by declaration,
- compiler-shipped stdlib and project code are not identified differently once
  registered in the interpreter,
- a future change that lets ordinary functions override builtins could weaken
  JSON policy if there is no trusted-origin check.

## Requirements

The eventual module system should support:

- unqualified references within the current namespace,
- qualified references from outside the namespace,
- explicit public exports for user-facing API,
- private or module-local helpers,
- a trusted-origin marker for compiler-shipped support modules,
- no accidental global flat aliases for namespaced declarations,
- compiler policy gates that can call trusted stdlib bodies without trusting
  arbitrary user/project definitions.

## Options

### 1. Export By Default, Private Helpers

All declarations are public unless marked private.

This is simple, but it fits Jett poorly: helper-heavy modules such as `json`
would expose too much by default, and LLM-generated code would need to remember
to hide helpers.

### 2. Private By Default, Explicit `export`

Top-level declarations inside a namespace are module-private unless marked with
an `export` keyword.

This is safer for stdlib and large projects. It also fits the "one canonical
form" principle: public API is mechanically searchable by `export`.

Example shape:

```jett
namespace json

export function parse[T](raw: string) returns result[T, string]:
    return json_parse_reflected[T](raw)

function json_parse_reflected[T](raw: string) returns result[T, string]:
    ...
```

Open syntax decision: whether `export` prefixes declarations, or whether a
namespace has an explicit export list.

### 3. Separate Trusted Stdlib Registry

Keep language visibility for later, but have the driver/interpreter register
compiler-shipped modules in a separate trusted registry.

This directly solves public JSON bridge trust, but not helper visibility. It is
a useful staging step, not the whole module story.

## Recommendation

Use two stages:

1. Add trusted stdlib identity before allowing public JSON bridges to target
   public wrapper names. Until then, bridge to internal hook names only.
2. Add explicit exports, probably private-by-default within namespaces, before
   treating `stdlib/json.jett` as a clean public module.

Do not remove compiler-owned JSON policy checks until both stages exist.

## JSON-Specific Staging

For now:

- public `json.parse`, `json.serialize`, and `json.serialize_public` remain
  compiler-owned policy gates,
- the interpreter delegates to `json.json_parse_reflected`,
  `json.json_serialize_reflected`, and
  `json.json_serialize_public_reflected`,
- public wrappers in `stdlib/json.jett` stay as readable declarations and a
  preview of the eventual API,
- helper names remain prefixed to reduce collisions, but that is convention,
  not real privacy.

Before changing this, add a trusted stdlib registration path or implement
exports/private helpers.
