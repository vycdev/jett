# Typed Python language and tooling reference

Use the project's supported Python version and effective checker configuration as authority.

## Type model

- Annotate public functions and non-obvious helpers completely.
- Use parameterized collections such as `list[T]`, `set[T]`, and `dict[K, V]`; avoid untyped containers.
- Express absence as `T | None` and narrow it explicitly.
- Model records with typed dataclasses or the project's established record form.
- Model closed alternatives with enums and precise unions. Keep each payload on the class that owns it.
- Preserve narrow types through branches. Avoid `Any`, `cast`, `# type: ignore`, or broad base classes merely to silence a mismatch.

Python integers are arbitrary precision. When an interface promises a fixed-width range or canonical text, enforce that contract explicitly.

## Control flow and failure

Let `isinstance`, `is None`, pattern matching, and enum comparisons narrow values. Structure closed-domain code so each branch preserves the checker's proven type.

Use the project's exception or result convention. Catch only failures the public contract expects, and do not convert programmer errors into silent defaults. Keep mutation local and return new values when the surrounding API is value-oriented.

## Verification loop

Discover project commands first. Typical strict checks are:

```text
python -m compileall path
pyright
python -m pytest
python -m ruff check .
python -m ruff format --check .
```

Use only configured tools. Fix syntax and import errors before type errors, then run focused tests before broad tests. A checker diagnostic is a design signal; narrow the model instead of suppressing the diagnostic.

## Provenance

This reference follows Python 3.12 typing and standard project tooling. It intentionally contains no evaluation task, solution, hidden case, or failure-specific recipe.
