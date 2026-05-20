# JSON Trusted Hooks Across Backends

Status: open design.

The interpreter and typechecker now treat compiler-shipped JSON stdlib hooks as
trusted implementation details. Future native or bytecode backends need the same
boundary, but it should not become a source-level trust feature.

## Current Invariant

- `export` controls source visibility only.
- Trusted origin is attached by the compiler/driver when loading bundled stdlib
  files, not by syntax in `.jett` source.
- Private JSON helpers may be compiler delegation targets when they come from
  trusted bundled stdlib modules.
- User or project functions cannot satisfy trusted JSON hook names, even if they
  use the same namespace and function names.
- Re-registering a hook name from untrusted source clears trusted dispatch for
  that name instead of inheriting trust.
- Root aliases such as bare `JsonValue` are compatibility visibility entries;
  they do not make any implementation trusted.

## Backend Requirement

When JSON public APIs are lowered outside the comptime interpreter, the backend
must receive enough metadata to distinguish:

- public source calls such as `json.parse[T](raw)`,
- compiler-owned policy gates for parse/serialize operations,
- trusted private stdlib hook implementations,
- ordinary user functions that merely have matching names.

The backend should not inspect source spelling alone. A project-defined
`namespace json` or future import/prelude alias must not gain access to private
JSON hook dispatch.

## Preferred Shape

Carry trusted-origin identity on lowered function symbols or module records.
The compiler-owned hook table in `jett_common::json` should remain the single
mapping from public facade names to private hook names. Backends can lower public
JSON calls through that table only when the selected target symbol has trusted
stdlib origin.

This keeps the rule aligned across:

- the comptime interpreter,
- a future bytecode interpreter backend,
- a future LLVM/native backend,
- tooling that performs hover, completion, or policy validation.

## Alternatives

1. Lower from trusted symbol metadata. This is the preferred path because it
   follows the existing driver/typechecker/interpreter model.
2. Generate a sealed stdlib manifest during driver loading. This could work, but
   it risks becoming a second source of truth unless it is derived from the same
   symbol metadata.
3. Add source syntax such as `trusted function`. Do not do this for now. Trust is
   a compiler packaging property, not a library authoring feature.

## Tests To Keep Mirrored

Future backend work should mirror the existing interpreter/typechecker boundary:

- spoofed project `namespace json` declarations collide or fail,
- private JSON hooks remain inaccessible to ordinary source,
- untrusted hook registration does not satisfy public bridge dispatch,
- public `json.parse`, `json.parse_exact`, `json.serialize`, and
  `json.serialize_public` still enforce compiler-owned policy gates,
- `json.JsonTree`, `json.JsonValue`, and bare `JsonValue` keep the staged
  compatibility relation without trusting user-defined lookalike types.

## Open Questions

- How will the future import/prelude system represent bundled stdlib identity?
- Should native codegen call reflected hooks directly, lower through a runtime
  ABI, or specialize common JSON shapes after typechecking?
- Done for stdlib-loaded code: bare `JsonValue` now reflects through the root
  alias to `json.JsonTree`. The legacy `TypePrimitive.json_value_type` tag is
  only a bootstrap/no-stdlib fallback.
- Should the hook table remain JSON-specific, or become a general compiler
  policy-hook registry once other stdlib features need the same treatment?
