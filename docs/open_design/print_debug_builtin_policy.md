# Print Debug Builtin Policy

Status: open. The current interpreter supports `print` and `println` as
capability-free debugging helpers, but the stable language design prefers
`Stdout.write(view stdout, text)` for ordinary output.

## Current Behavior

- `print(...)` and `println(...)` accept value arguments and return `nothing`.
- They are typechecked as output boundaries for `secret[T]`, so direct secret
  values cannot be printed.
- They are not currently treated as impure builtins by the checker, and they do
  not require a `Stdout` capability parameter.
- The interpreter writes them through the same captured stdout path used by
  driver tests.
- Existing stdlib-loading smoke code uses `println` as a low-friction fallback
  diagnostic inside a `main()` with no capabilities.

## Why This Needs A Deliberate Rule

Jett's capability model is one of its main LLM-facing guarantees: a function
signature should reveal whether the function performs I/O. Ordinary output is
therefore spelled with an explicit capability:

```jett
function emit(view stdout: Stdout, message: string) returns nothing:
    Stdout.write(view stdout, message)
```

A global `println` is convenient, but it can blur that guarantee if it becomes
the canonical way to write logs. Agents would learn two output stories:

- production output needs `Stdout`,
- debug output can silently write without one.

That split should be intentional if it remains.

## Options

1. Treat `print` and `println` as debug-only compiler builtins.
   They stay capability-free, remain secret-output boundaries, and are
   stripped or rejected in release/native builds. This mirrors the special
   treatment planned for `trace` and `breakpoint`, but it needs explicit mode
   diagnostics so agents do not use it for product behavior.

2. Treat `print` and `println` as ordinary impure output builtins.
   They would require capability visibility, either by becoming wrappers over
   `Stdout.write` or by being rejected unless a `Stdout` value is in scope. This
   keeps the effect model simple but removes the current no-capability smoke
   helper.

3. Retire `print` and `println` from the stable surface.
   Keep `Stdout.write` for output and prefer `trace` / `breakpoint` for
   debugging. This is the cleanest capability story, but it requires migration
   for existing smoke fixtures and any user examples.

## Recommendation

Tracked by [#8](https://github.com/vycdev/jett/issues/8) for the stable
capability, mode, and compatibility policy.

Keep the implementation conservative until the debug/release mode boundary is
real. Do not silently make global `println` the blessed logging API.

For now:

- use `Stdout.write` in design examples and production-style fixtures,
- keep `print` / `println` as secret-blocked smoke/debug helpers,
- do not add more public stdlib APIs that depend on capability-free output,
- before native codegen treats them as real I/O, decide whether they are
  debug-only builtins, capability wrappers, or retired compatibility helpers.
