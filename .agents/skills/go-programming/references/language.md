# Go language and tooling reference

Use `go.mod`, package-local conventions, and the repository's supported toolchain as authority.

## Type model

- Prefer concrete types. Introduce an interface only at the consumer boundary that needs multiple implementations.
- Use named types and constants when primitive values represent a closed domain.
- Use structs for records and explicit result structs or `(value, ok)` for optional outcomes according to the surrounding API.
- Use `map[K]struct{}` for a set when no project abstraction exists.
- Preserve element and map types; do not replace useful information with `any`, `interface{}`, or reflection.

Go does not prove exhaustive switches. List every known constant or concrete variant explicitly and omit `default` when visibility of new cases matters. For payload-bearing closed alternatives, follow the project's private-marker interface pattern or another established representation.

## Ownership and failure

Values are copied on assignment, while slices and maps are descriptors over shared storage. Make mutation and aliasing intentional. Copy a slice or map only when the caller requires independent storage.

Return errors explicitly and handle them near the call. Wrap errors only with useful operation context and preserve identity when callers may inspect it. Do not use `panic` for ordinary invalid input or expected failure.

## Verification loop

Run the narrowest relevant standard commands:

```text
gofmt -w file.go
go test ./path/to/package
go test ./...
go vet ./...
```

Use the module's configured linters when present. Fix parse and package errors before type errors, then run focused tests before the whole module.

## Provenance

This reference follows the Go language specification and standard Go tooling. It intentionally contains no evaluation task, solution, hidden case, or failure-specific recipe.
