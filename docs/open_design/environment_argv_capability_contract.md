# Environment and Argument Capability Contract

Status: proposed decision for [#94](https://github.com/vycdev/jett/issues/94).
Implementation and stdlib extraction remain pending.

## Context

Jett currently exposes capability-free `os.env(key)` and `os.args()` builtins.
The interpreter reads `std::env` directly on every call, `os.env` collapses a
missing variable and a non-Unicode value into the same `none`, and `os.args`
includes the executable name. Because these operations are not classified as
effects, pure functions and `verify` blocks can observe the host process.

The language design already names `Environment` as the capability for ambient
process inputs, uses `Environment.get(view env, key)` and
`Environment.args(view env)`, and states that arguments exclude the executable
name. A partial checker signature for `Environment.args` exists, but it has no
runtime implementation or injected environment value. This record completes
that contract before the transitional builtins are replaced.

## Decision Summary

The canonical public operations are:

```text
Environment.get(view env: Environment, key: string)
    returns result[optional[string], string]
Environment.args(view env: Environment)
    returns list[string]
```

Both operations read an immutable launch snapshot carried by `Environment`.
They are effects even though repeated reads of one snapshot are stable. Only
`main` receives the capability from the runtime; other functions borrow it
with `view Environment`.

`os.env` and `os.args` are removed when this surface lands. Capability-free
compatibility wrappers are not retained because they would preserve hidden
host access and a second spelling for the same operations.

Issue #94 describes the target in places as a source-owned public `os` surface.
This decision interprets that acceptance boundary as the ambient OS-input
surface being replaced: the declarations are source-owned as required, but use
the already documented `Environment` capability namespace rather than retaining
`os` as a second public namespace.

## Environment Lookup

`Environment.get` distinguishes ordinary absence from invalid host data:

- a missing variable returns `ok(none)`;
- a present value that is valid Jett text returns `ok(some(value))`;
- an invalid variable name returns
  `fail("Environment.get: invalid variable name")`;
- a present value that cannot be represented as a Jett `string` returns
  `fail("Environment.get: value is not valid Unicode")`.

The error strings do not contain the key, value, host error code, or platform
wording. This prevents an error path from unexpectedly exposing a secret key
name or environment value. The runtime must not replace invalid data with
lossy text or collapse it into `none`.

The first contract has no raw-byte environment API and no variable enumeration
operation. A future byte-native API needs a separate design for key encoding,
platform differences, secret handling, and portability; it must not overload
`Environment.get` with a second return representation.

Variable names are passed to the target platform without case normalization.
Lookup follows the host environment's native name comparison: case-sensitive
on Unix-like targets and case-insensitive on Windows. Portable programs should
use the documented spelling of each variable. Empty names and names containing
`=` or a NUL code point are invalid on every target and fail before lookup.
Other target-specific rejected names map to the same stable invalid-name error.

On Unix-like targets the runner freezes the raw `environ` byte entries before
entering Jett. An entry without `=` is ignored. If the host supplies duplicate
names, the first captured entry wins, matching ordinary `getenv` traversal.
Names that are not valid UTF-8 cannot equal a Jett `string` key and are not
queryable through this API; their values do not make unrelated lookups fail.
When a valid UTF-8 name matches, its raw value is decoded on lookup and invalid
UTF-8 produces the stable invalid-Unicode value failure above.

On Windows the runner freezes the wide environment block returned by
`GetEnvironmentStringsW` and releases the native block after copying it. Hidden
drive-current-directory entries whose names begin with `=` are not part of the
public environment map. Name matching uses invariant case-insensitive Windows
environment semantics, with the first captured duplicate winning. Raw UTF-16 is
retained in the snapshot so an unpaired surrogate in a matched value produces
the stable invalid-Unicode failure rather than lossy replacement. A name with
an unpaired surrogate cannot match a Jett `string` key and is not queryable.

## Argument Semantics

`Environment.args` returns the user arguments supplied to the Jett program:

- the executable or launcher name is excluded;
- argument order is preserved;
- empty arguments are preserved;
- no shell splitting, quote removal, environment expansion, Unicode
  normalization, or locale conversion occurs in Jett;
- no user arguments produce an empty list;
- each call returns an independent list value, so changing the returned list
  cannot change the capability snapshot or a later result.

Jett `string` values require valid Unicode. The runtime validates the complete
argument vector while constructing the launch context, before entering `main`.
If any argument cannot be represented as a Jett string, the run fails with the
stable runtime error `Environment: argument is not valid Unicode`. It must not
panic, replace bytes, skip the argument, or enter `main` with a partial vector.
This launch failure is not a source-level `result`: a program cannot usefully
recover after its own invocation data failed the language representation
boundary.

The runner, not source code, removes the executable name. On Unix-like targets
the runtime copies the native `argv` entries supplied to the process entry
point, validates each user entry as UTF-8, and excludes `argv[0]`. On Windows it
parses the wide string from `GetCommandLineW` with `CommandLineToArgvW`, excludes
element zero, and converts the remaining UTF-16 entries without lossy
replacement. This pins Windows quote, backslash, and empty-argument behavior to
one platform contract instead of whichever C runtime launched a backend.
Test harnesses inject only the user-argument portion of the snapshot, matching
what `Environment.args` returns, and include conformance cases for quoted,
backslash-containing, and empty Windows arguments.

## Snapshot and Host Boundary

A production runner captures arguments and the environment once while creating
the runtime context. `Environment` identifies that immutable snapshot. Public
operations never call ambient process APIs directly after `main` starts.

The snapshot requirement provides a local, deterministic meaning for a
capability value:

- repeated reads through the same capability observe the same launch data;
- environment changes made by native libraries or child-process machinery after
  launch are not visible;
- independent run contexts do not share injected data;
- capability handles passed to actors refer to the same immutable snapshot;
- tests can inject exact arguments and environment entries without changing the
  test runner's real process environment.

The production runtime freezes the raw platform environment before entering
`main`. It may defer Unicode decoding and map-index construction, but it cannot
defer copying raw entries or use a fresh host lookup for any
`Environment.get` call.

A WASI runner copies its injected argument and environment vectors at instance
startup and applies the same user-argument, Unicode, and lookup outcomes. A
WebAssembly host that cannot provide those vectors cannot construct an
`Environment`; requesting one in `main` fails before entry with
`Environment: launch data unavailable`. Browser-specific ambient globals are
not an implicit fallback.

A future API for mutating the process environment is out of scope. It would be
a separate effect with process-wide concurrency and child-inheritance policy;
it must not mutate an existing `Environment` snapshot.

## Capability, Purity, and Comptime

`Environment` follows Jett's ordinary capability rules:

- `main` owns the runtime-provided capability;
- ordinary functions borrow it as `view Environment`;
- a helper that reads arguments or a variable propagates that capability
  requirement to its callers;
- capability-free functions cannot call either operation and remain pure;
- `verify`, property generation, and comptime evaluation cannot access
  `Environment`, directly or through a helper;
- compilation must never read build-host arguments or environment variables to
  produce an artifact.

Both operations are classified as effects at their public declarations or
trusted hooks. A compiler-shipped source body does not make a host operation
pure. Capability checks happen before private runtime dispatch.

Environment entries may contain credentials. Reading a value returns an
ordinary `string`, matching existing capability APIs; the type system does not
infer secrecy from a variable name. Official examples should avoid printing
environment values, errors must not include them, and applications should wrap
credential-bearing values in `secret[string]` at their trust boundary. This
contract does not add automatic secret tainting or a secret-variable registry.

## Public Source and Private Runtime Boundary

Public declarations belong in trusted compiler-shipped `.jett` source under the
`Environment` namespace. The intended shape is:

```jett
namespace Environment

export function get(view env: Environment, key: string) returns result[optional[string], string]:
    return environment_get_kernel(view env, key)

export function args(view env: Environment) returns list[string]:
    return environment_args_kernel(view env)
```

The two `_kernel` spellings are pseudocode for private trusted runtime hooks,
not public source names. They read only the injected snapshot. Project and
dependency code cannot resolve, replace, or spoof them, and cannot reopen the
compiler-shipped `Environment` namespace.

The end state keeps public names, signatures, documentation, and result policy
in source. Rust interpreter or future backend code owns only:

- launch-time capture and validation;
- opaque snapshot storage and capability identity;
- platform-native variable-name lookup;
- conversion of invalid names or values into the stable contract outcomes;
- private hook dispatch based on trusted compiler-shipped origin.

The current hardcoded `os.*` checker signatures, direct `std::env` interpreter
arms, and placeholder `Environment.args` signature are transitional technical
debt. Trusted-origin and module-loading mechanics coordinate with
[#3](https://github.com/vycdev/jett/issues/3), but that larger import design
does not change this public API.

## Compatibility Policy

The old ambient operations are removed rather than overloaded:

```jett
# Before
optional[string] value = os.env("CONFIG_PATH")
list[string] args = os.args()

# After
optional[string] value = Environment.get(view env, "CONFIG_PATH") handle error:
    return nothing
list[string] args = Environment.args(view env)
```

The checker should retain focused migration diagnostics for the removed names:

- `os.env(key)` suggests `Environment.get(view env, key)` and notes the new
  `result[optional[string], string]` return type;
- `os.args()` suggests `Environment.args(view env)` and notes that the
  executable name is no longer included.

There are no deprecated wrappers, root aliases, or `os` overloads. One
canonical spelling keeps capability effects visible and makes query output
unambiguous.

## Scope Boundaries

This contract covers read-only launch environment variables and user arguments.
It does not define:

- process spawning, exit status, signals, working-directory changes, or process
  environment mutation;
- filesystem access or executable lookup through `PATH`;
- shell parsing, command-line option parsing, response files, or configuration
  precedence;
- byte-native environment values or argument vectors;
- HIR, MIR, bytecode, LLVM, or platform ABI lowering beyond preserving the
  explicit capability and snapshot behavior.

Process creation remains under the separate `Process` capability. Filesystem
operations remain under `Filesystem`; the `os` module must not become a route
around those capabilities.

## Implementation Slices

1. **Pin and inject the launch snapshot**
   - introduce a runtime environment provider with production and test
     snapshots;
   - validate argv before `main`, exclude the executable name, and preserve
     ordering and empty arguments;
   - distinguish missing, invalid-name, and invalid-value environment lookups;
   - cover POSIX malformed/duplicate entries, Windows wide environment and
     command-line parsing, and injected WASI launch vectors;
   - remove direct ambient `std::env` reads from operation dispatch.
2. **Enforce the capability effect**
   - complete the checked `Environment` parameter type;
   - classify direct and transitive calls as effects;
   - reject access from capability-free functions, `verify`, properties, and
     comptime evaluation;
   - pass an opaque environment capability rather than `Value::Nothing` to the
     runtime entry point.
3. **Extract the public source surface**
   - add compiler-shipped declarations and trusted private hooks;
   - add signature/source-query and namespace-protection regressions;
   - remove hardcoded `os.env`, `os.args`, and placeholder public signature
     knowledge;
   - add focused diagnostics for old names.
4. **Preserve the contract in later backends**
   - carry the explicit capability and snapshot identity through HIR and MIR;
   - inject equivalent launch data in interpreter, bytecode, and native runners;
   - run the same platform-neutral conformance scenarios for every backend.

## Required Regression Matrix

- Both public operations require a visible `view Environment` capability.
- Direct and transitive capability-free calls are rejected.
- `verify`, property, and comptime calls fail without reading the host process.
- Missing variables return `ok(none)`; present valid values return
  `ok(some(value))`.
- Invalid names and non-Unicode values produce their distinct stable failures.
- POSIX malformed/non-Unicode names and duplicate entries follow the frozen
  first-entry policy without poisoning unrelated lookups.
- Errors do not reveal keys, values, or host wording.
- Arguments exclude the executable name and preserve order and empty entries.
- Windows quote/backslash parsing and WASI injection produce the same logical
  user-argument model as Unix entry-point vectors.
- No user arguments return an empty list.
- Invalid argument text prevents entry into `main` with the stable launch error.
- Repeated calls observe one immutable snapshot and return independent lists.
- Independent runtime contexts and injected test snapshots remain isolated.
- Removed `os.env` and `os.args` calls receive migration diagnostics.
- Public signatures resolve to compiler-shipped source; private hooks and the
  `Environment` namespace cannot be spoofed by project code.
- Future interpreter, bytecode, and native runners satisfy the same observable
  contract without exposing platform error codes or ambient host reads.
