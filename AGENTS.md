# Agent Guide

This repo is for Jett, a general-purpose language shaped around how coding
agents read, patch, type-check, and repair code. When changing the compiler or
stdlib, preserve that purpose: one canonical spelling, local context, explicit
effects, small bounded functions, and compiler-enforced policy.

## Non-Obvious Project Rules

- If a language rule is unclear, do not invent behavior in Rust first. Capture
  the tradeoff in `docs/open_design/` or an active design note, then keep the
  implementation conservative.
- Keep `docs/design.md` and `docs/architecture.md` aligned with any language,
  namespace, stdlib-loading, or compiler-policy change.
- `docs/active/` is live planning, `docs/open_design/` is unresolved design,
  and `docs/completed/` is historical handoff material.
- Jett has strict top-to-bottom declaration rules. Forward references are
  allowed only through `mutual:`.
- A named struct or enum may refer to itself when it has a finite base value;
  this is compiler-managed indirection, not a forward-reference exception for
  another type. Do not add `box[T]` or `mutual` type declarations. Model shared
  or cyclic graphs with explicit IDs and collections.
- Namespaced declarations are private by default. Mark only the intended public
  API with `export`.
- Project and dependency namespaces must remain unique. Only compiler-shipped
  stdlib files may use namespace fragments, and duplicate declarations inside
  the merged namespace must still fail.
- Stdlib code is not exempt from function complexity limits. Split large
  functions by policy or data shape instead of deepening branch ladders.
- Prefer real `.jett` stdlib implementations over Rust builtins once the
  language can express the behavior safely.
- The JSON stdlib lives in numbered `stdlib/json/` fragments because the loader
  sorts paths lexically and current semantics still depend on declaration order.
- Public `json.parse`, `json.parse_exact`, `json.serialize`, and
  `json.serialize_public` still carry compiler-owned policy gates; trusted
  stdlib hooks provide their implementation bodies.
- `json.JsonTree` is the sole native raw JSON representation. Bare
  `JsonValue`, `json.JsonValue`, and root type aliases are rejected.
- Reflected construction has one canonical source form: the explicit
  `TypeConstruction` builder lifecycle. Do not add a parallel construction
  block, contextual `provide` statement, or callback spelling.

## Verification

Use focused tests first, then broaden when a change crosses compiler phases.
Common commands:

```bash
cargo fmt
cargo test -q -p jett_driver run_pass_json
cargo test -q -p jett_driver compile_fail_namespace
cargo test -q -p jett_driver run_pass_namespace
cargo test -q -p jett_resolve
cargo test -q -p jett_typecheck
cargo test -q -p jett_comptime
cargo test -q -p jett_driver
cargo test -q
```

For Jett source formatting:

```bash
cargo run -q -p jett_cli -- format path/to/file.jett
cargo run -q -p jett_cli -- format --check path/to/file.jett
```

## Commit And Push

- After each completed change, run `git diff --check`, commit the coherent
  change, and push it.
- Do not bundle unrelated work into the same commit.
- If verification could not be run, say that in the final response and in the
  commit context when relevant.
