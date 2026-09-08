# Jett benchmark subset v0.1

This is a frozen capability profile for the first LLM pilot. It is narrower
than the language specification and does not imply that other Jett features are
unstable or removed.

## Included

- one namespace per submitted file;
- top-to-bottom function declarations;
- `int64`, `bool`, `string`, and `list[int64]`;
- typed parameters, local variables, and return values;
- arithmetic, comparisons, boolean operators, and the `modulo` keyword;
- `if`/`else`, `for value in values`, `range`, and `while`;
- assignment, `return`, string literals, and `assert` in `verify` blocks;
- pure deterministic functions with no capabilities or external effects.

## Excluded from v0.1

- globals, nested named declarations, concurrency, async, capabilities, I/O;
- structs, enums, maps, sets, optionals, results, generics, closures, and
  reflection;
- mutation of list contents and dependence on iteration order outside lists;
- compiler plugins, FFI, network access, randomness, and clocks;
- performance tasks or tasks that rely on integer overflow.

## Submission contract

A submission is one complete UTF-8 source file. It must retain the requested
namespace and public function name/signature. The grader appends private
`verify` blocks after the submission, consistent with Jett's declaration order.
It invokes only the requested function and treats any compiler diagnostic,
failed assertion, timeout, or unexpected process exit as failure.

The v0.1 integer domain excludes `int64` overflow and the minimum signed value
when absolute value is required. String outputs use exact lowercase ASCII
spellings supplied by the task.

## Change policy

Any semantic, syntax, compiler, stdlib, or grader change that can alter an
answer increments the subset or task version. Old result rows continue to point
to this document and their exact git revision.
