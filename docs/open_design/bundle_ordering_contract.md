# Bundle Ordering Contract

Status: implemented. `jett bundle` uses resolver-derived dependency edges,
deterministic whole-file ordering, structured cycle diagnostics, and the
validation-first write contract below. Future module/import registry work may
replace the dependency source without changing this bundling contract.

## Problem

Jett source has strict top-to-bottom declaration rules inside a file, while the
project driver can discover sibling files and resolve exported project
definitions across that file set. A bundle that merely concatenates files in
path order could change semantics: a call that is legal in a multi-file project
may become a forward reference in the generated single file.

For an LLM-oriented language, this is the wrong failure mode. Bundling should
not create hidden ordering puzzles or require agents to infer why distributed
code differs from source-project code.

## Conservative Contract

The implementation remains validation-first:

- Preserve each source file's internal declaration order exactly.
- Use deterministic whole-file dependency order as the candidate bundle order.
- Parse, resolve, and typecheck the generated bundle before writing or reporting
  success.
- If ordering or validation fails, report structured bundle diagnostics and
  leave the output file untouched.
- Do not introduce a bundler-only relaxation of forward-reference rules.

This keeps the command honest: it either produces a distributable file with the
same compiler policy as ordinary source, or it explains why the current project
layout cannot be represented as a single file yet.

## Implemented Dependency Ordering

Tracked by [#13](https://github.com/vycdev/jett/issues/13).

The bundler parses every project file with a distinct `FileId`, merges those
modules for name resolution, and derives file edges from the resolver's
canonical reference-to-definition mapping. Resolver-owned E0205 definition
labels supply the same edge when a strict forward reference is intentionally
absent from the successful-resolution map. This avoids a second approximation
of Jett expressions, types, namespace qualification, or aliases inside the
bundler.

The graph is topologically sorted with lexical path order as the deterministic
tie-breaker. This preserves Jett's local readability constraints:

- It reorders whole files only when doing so does not split declarations.
- It does not reorder declarations within a file.
- It emits an agent-readable manifest table that maps source files to
  output line ranges.
- If two files are mutually dependent in a way that requires interleaving
  declarations, the bundler fails and recommends extracting shared
  definitions into an earlier namespace/file.

After ordering, the ordinary single-source build remains authoritative. Parse,
resolution, type, or policy errors that are unrelated to cross-file ordering
are reported by candidate validation, and the output is written only after
that validation succeeds.

## Agent Output Shape

The `--agent` form should be deterministic TOON:

```toon
status: ok
project_root: path
output: dist/library.jett
files: 3
bundled_files[3]{path,start_line,end_line}:
  src/core.jett,1,42
  src/json.jett,44,97
  src/public_api.jett,99,120
```

On failure, use `status: error` with structured ordering or candidate-validation
diagnostics, not a prose-only explanation.
