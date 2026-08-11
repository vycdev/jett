# Print Debug Builtin Policy

Status: decided by [#8](https://github.com/vycdev/jett/issues/8). The language
policy is settled; mode diagnostics and backend conformance are not implemented
yet.

`print` and `println` remain a stable compatibility surface as compiler-owned,
debug-only builtins. There is no scheduled removal, but they are not ordinary
output APIs and must never become an implicit spelling of `Stdout.write`.

## Current Implementation

The checker currently accepts arbitrary non-secret value arguments, returns
`nothing`, and does not classify either builtin as capability-requiring. The
interpreter formats arguments with spaces, adds a newline only for `println`,
and uses the same stdout path as `Stdout.write`. Depending on the entrypoint,
that path is either written directly to process stdout or captured into the
combined run output.

Those shared-channel details are transitional, not the stable contract.
`--release` is currently parsed without a release-specific checker/backend
policy, and verify execution does not yet isolate debug text from agent output.
The requirements below apply when those mode and backend boundaries are added.

## Decision

Jett keeps the two builtins as debugging and smoke-test instrumentation:

- `print(...)` and `println(...)` accept arbitrary checked value arguments and
  return `nothing`. Composite rendering is tooling output, not a stable
  serialization format.
- They require no capability because their output is a compiler-owned debug
  observation, not semantic program stdout.
- They remain output boundaries for `secret[T]`; direct and refinement-wrapped
  secrets must be rejected before evaluation.
- `print` separates arguments with one space and adds no trailing newline;
  `println` uses the same rendering and appends one newline.
- A debug-enabled entrypoint must preserve relative execution order among
  `print` and `println` events. Jett code cannot read the text or use it as a
  semantic program result.
- Non-release `verify` and comptime execution may use the builtins only when the
  entrypoint captures them as debug events. Agent output must represent those
  events structurally or isolate them from its protocol text.
- Once the release policy reaches the checker/backend boundary,
  `jett build --release` must reject both names with a focused diagnostic:
  debug printing is unavailable in release mode; use `Stdout.write` for
  application output or `trace` / `breakpoint` for structured debugging.
  Rejection rather than silent stripping avoids hiding argument-evaluation
  failures.
- A future native or bytecode backend may support the builtins only in a
  non-release debug mode and through a compiler-owned diagnostic channel. A
  backend without that channel must reject them explicitly; it must not lower
  them to ambient process stdout.

This is a narrow exception to the signature-visible capability rule, shared in
spirit with `trace` and `breakpoint`. A function containing these calls remains
free of semantic program I/O, but a debug-enabled toolchain may observe its
debug events and must not cache, duplicate, or reorder those events as though
they were ordinary pure expressions. Release behavior is explicit rejection,
so optimized production code never depends on them.

Ordinary output remains capability-bearing:

```jett
function emit(view stdout: Stdout, message: string) returns nothing:
    Stdout.write(view stdout, message)
```

## Compatibility And Migration

The current interpreter behavior remains valid while the dedicated debug-event
channel is pending. In particular, `tests/run_pass/stdlib_loading.jett` may keep
its no-capability `println` fallback until the loader smoke test has a dedicated
diagnostic assertion.

Production examples and APIs must use `Stdout.write`. Code that needs structured
debug facts should prefer `trace` or `breakpoint`; unstructured `print` calls
are for short-lived inspection and simple fixtures only. Future mode
diagnostics must say that directly instead of suggesting an implicit
capability or silently changing the call's meaning.

## Conformance Work

Existing coverage includes `stdout_output_can_be_captured` in
`crates/jett_comptime/src/interpreter.rs` for spacing, newline, and current
shared-channel order,
`tests/compile_fail/secret_print.jett` for bare secrets, and
`tests/compile_fail/refined_secret_exposure.jett` for refinement-wrapped
secrets. The stdlib fallback lives in `tests/run_pass/stdlib_loading.jett`.

The remaining policy coverage is:

1. Add a driver fixture for multiple `print` / `println` calls and their
   relative captured order.
2. Add verify/comptime coverage once their debug events are isolated from plain
   and agent protocol output.
3. Add release compile-fail fixtures for both names when `--release` reaches
   the checker/backend policy boundary.
4. Add backend conformance tests proving that unsupported debug builds reject
   the calls and that supported debug builds use a diagnostic channel rather
   than application `Stdout`.
5. Add agent-output coverage that distinguishes captured debug events from
   capability-backed application output.

Until those phases exist, the conservative implementation rule is: do not add
more capability-free output APIs, do not advertise `print` or `println` as
logging, and do not lower either name as ordinary native I/O.
