# Strict TypeScript language and tooling reference

Use the repository's effective `tsconfig.json`, package manager, and target as authority.

## Type model

- Keep `strict`, `noImplicitReturns`, and related configured checks effective.
- Model closed alternatives with discriminated unions whose literal tag owns the associated payload.
- Narrow with tag checks, `switch`, property checks, and user-defined predicates only when necessary.
- Use `readonly` fields and `ReadonlyArray<T>` where mutation is not part of the interface.
- Use `T | null` or `T | undefined` according to the existing contract; do not introduce both casually.
- Preserve generic parameters and precise collection types. Avoid `any`, unchecked `as` assertions, `@ts-ignore`, and wrapper types that erase domain information.

`bigint` and `number` are distinct. Use `n` literals and do not mix their arithmetic. Treat parsing and fixed-range constraints as explicit fallible boundaries.

## Control flow and failure

Return directly from exhaustive union cases so the checker can prove completion. If the project uses an exhaustiveness helper, pass the narrowed `never`; otherwise follow existing style without adding a catch-all that hides new variants.

Catch only expected conversion or boundary failures. Check that a caught value has the needed shape before reading it. Prefer typed result unions when that is the surrounding API.

## Verification loop

Use the repository's package manager. Typical checks are:

```text
npx tsc --noEmit
npm test
npm run lint
npm run format -- --check
```

Do not invent a standalone compiler command when a project script carries required flags. Fix parse and module errors before type errors, then run focused tests before broad tests.

## Provenance

This reference follows strict TypeScript and standard compiler tooling. It intentionally contains no evaluation task, solution, hidden case, or failure-specific recipe.
