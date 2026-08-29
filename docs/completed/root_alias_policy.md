# Root Alias Policy

Status: completed. Root type aliases are rejected across project, dependency,
and compiler-shipped standard library source.

## Decision

Jett keeps exported declarations under their canonical namespace. There is no
accepted `export root type` form and no prelude mechanism that injects aliases
into the global type namespace.

The compiler still parses the retired spelling far enough to report focused
`E0209` diagnostics. It never registers the declaration in resolver,
typechecker, interpreter, completion, or query state.

This policy keeps public names searchable and prevents a second spelling from
competing with canonical names such as `json.JsonTree`, `path.Path`, or
`time.Timestamp`.

The later [module and trusted-origin contract](module_import_trusted_origin_contract.md)
keeps this decision intact: its fixed foundational prelude points to canonical
declarations and cannot inject root type aliases or alternate reflected
identities.

## JsonValue Retirement

The only implemented root alias was the compiler-shipped compatibility bridge:

```jett
export root type JsonValue = json.JsonTree
```

That declaration and its compiler allowlist have been removed. Bare
`JsonValue` is now unknown in all source contexts. Existing code must migrate to
`json.JsonTree`; the namespaced `json.JsonValue` compatibility alias has also
been removed.

Conformance coverage pins that root exports fail with `E0209`, bare
`JsonValue` stays out of completions, and `json.JsonTree` remains the canonical
raw JSON type.
