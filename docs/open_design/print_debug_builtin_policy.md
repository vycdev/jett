# Print Debug Builtin Policy

Status: decided by [#8](https://github.com/vycdev/jett/issues/8). The language
policy, release-mode diagnostics, and dedicated debug-event channel are
implemented. Hidden-secret observation remains a separate open policy.

`print` and `println` remain a stable compatibility surface as compiler-owned,
debug-only builtins. There is no scheduled removal, but they are not ordinary
output APIs and must never become an implicit spelling of `Stdout.write`.

## Current Implementation

The checker accepts checked non-secret value arguments, returns `nothing`,
and does not classify either builtin as capability-requiring. Formatting and
ownership remain unchanged. Arguments finish evaluation before one complete
event is emitted; failure in a later argument publishes no portion of that call.

The interpreter records explicit Trace, Breakpoint, Print, and Println events in
one ordered buffer. Event text contains exactly the operation's intended bytes,
including partial prints and embedded CR/LF. Silent compilation and capture
workers never write those bytes to process output. Human runtime entrypoints and
the native DebugPrint leaf emit them to diagnostic stderr. Capability-backed
`Stdout.write` retains application stdout.

Driver captures preserve comptime, frontend verification, and runtime phases.
Comptime observations are separate from baked values. Verification retains
actual block/trial events, including first failure; shrinking and private replay
stay isolated. Tools retain preceding observations on later build, lowering,
linking, or runtime failure without rerunning workers. Agent renderers escape
exact text with an explicit phase and kind; a printed trace prefix or protocol
header is ordinary print text.

Failed comptime recovery records actual attempted expression/context pairs
without inserting a baked value. Duplicate checked contexts and namespace
initializer markers do not repeat the error or its observations. Native property
replay failures retain the original suite streams separately from the actionable
replay cause; private runtime replay streams do not become public observations.
This retention applies when the original attempt returned captured process
output. An initial execution error or timeout retains its existing typed error
contract; it does not invent a completed original-suite capture record.

`--release` still rejects both names with E0362, checks their arguments, and
preserves existing output artifacts. This channel change does not select the
separate policy for secrets hidden by interfaces or builders.

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

`tests/run_pass/stdlib_loading.jett` keeps its no-capability `println` failure
fallback as a debug event. Programs and tools that previously captured print
text as application stdout must now read the diagnostic event capture. Native
transport uses exact stderr bytes, and agent mode represents events in escaped
structured rows.

Production examples and APIs must use `Stdout.write`. Code that needs structured
debug facts should prefer `trace` or `breakpoint`; unstructured `print` calls
are for short-lived inspection and simple fixtures only. Future mode
diagnostics must say that directly instead of suggesting an implicit
capability or silently changing the call's meaning.

## Conformance

Interpreter and driver tests cover exact spacing and terminators, empty events,
partial and multiline text, Unicode, explicit kinds, and application/debug
separation. A failed later argument retains preceding events while publishing
none of the failed call. Dedicated native transport cases compare exact stderr
with the ordered runtime event capture after source deletion, including event
and terminal-error adjacency, ownership cleanup, and compiled suites.

Comptime and verification capture tests cover successful and failed evaluation,
baked-value reads, property generation and trial order, and private shrinking.
CLI renderer tests preserve phase and kind on success and failure and escape
printed text resembling protocol headers. Existing direct-secret rejection and
release E0362 gates remain separate, including typechecking rejected arguments
and preserving existing artifacts.

The remaining hidden-secret observation choice is recorded in
[debug_print_hidden_secrets.md](debug_print_hidden_secrets.md). No additional
capability-free output APIs or implicit application I/O are introduced here.
