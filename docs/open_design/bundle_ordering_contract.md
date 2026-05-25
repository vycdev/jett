# Bundle Ordering Contract

Status: partially implemented. The first `jett bundle` implementation uses the
validation-first contract below. Dependency-aware reordering remains open.

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

The first implementation should be validation-first:

- Preserve each source file's internal declaration order exactly.
- Use a deterministic project-file order only as a candidate bundle order.
- Parse, resolve, and typecheck the generated bundle before writing or reporting
  success.
- If validation fails because of declaration order, report structured bundle
  diagnostics and leave the output file untouched.
- Do not introduce a bundler-only relaxation of forward-reference rules.

This keeps the command honest: it either produces a distributable file with the
same compiler policy as ordinary source, or it explains why the current project
layout cannot be represented as a single file yet.

## Future Direction

A later bundler can perform dependency-aware ordering, but only if it preserves
Jett's local readability constraints:

- It may reorder whole files only when doing so does not split declarations.
- It must not reorder declarations within a file.
- It should emit an agent-readable manifest table that maps source files to
  output line ranges.
- If two files are mutually dependent in a way that requires interleaving
  declarations, the bundler should fail and recommend extracting shared
  definitions into an earlier namespace/file.

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

On failure, use `status: error` with diagnostics from validating the candidate
bundle, not a prose-only explanation.
