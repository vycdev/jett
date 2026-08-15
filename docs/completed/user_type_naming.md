# User-Defined Type Naming

Status: completed. Jett keeps lowercase built-in type names and requires one
PascalCase form for every user-defined type declaration.

## Decision

The following declaration names must use PascalCase:

- structs;
- enums;
- interfaces;
- machines;
- actors;
- bitfields;
- type aliases and refinements.

A valid name begins with an ASCII uppercase letter and contains only ASCII
letters or digits. Underscores are not allowed. Initialisms and digits are
accepted because their internal word boundaries cannot be inferred reliably:

```jett
type UserID = string

struct Utf8Value:
    text: string
```

Built-in primitives and generic type constructors retain their canonical
lowercase spellings, such as `int64`, `string`, `list[T]`, and `result[T, E]`.
Compiler-provided capabilities such as `Stdout` and `Filesystem` follow the
type form. Functions, values, fields, variants, states, and ordinary namespaces
remain `snake_case`.

This category split is deliberate rather than an exception to the one-form
rule: an identifier's role selects exactly one spelling convention, and the
capitalized form makes declared types visible while scanning unfamiliar code.

## Enforcement

The resolver validates names during top-level declaration registration so the
same rule covers every declared type category and both project and stdlib
source. E0212 points to the declaration and suggests a PascalCase replacement.
The invalid declaration is still registered after the diagnostic, preventing
follow-on undefined-name noise in references to it.

Coverage rejects lowercase and underscore-separated declarations across every
type category. The existing compiled corpus supplies broad positive coverage;
focused resolver tests pin accepted initialism and digit forms.
