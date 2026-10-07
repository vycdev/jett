# Ordinary borrowed optional/result payloads

## Source-only baseline

This packet isolates the existing public `optional[list[int64]]` and
`result[list[int64], string]` language family. It changes no compiler, runtime,
stdlib, language policy, or production representation. The baseline production
revision is `35bef5ea63cc8c28eb0c45edcf15fcd827a8f3b9`, in the separate
`codex/ordinary-borrowed-sums` worktree.

The canonical payload alias is an explicit outer View of a Handle whose target
is an explicit View of one stable immutable sum local or View parameter:

```jett
list[int64] alias = view((view source) handle:
    return nothing
)
```

A Result uses `handle error:`. The checked alias retains the exact backing
binding. The occupied arm exposes a nonowning payload; the absent arm ends with
Return and creates no payload alias. A nested expression-level Handle can have
its own scalar Default. This packet does not select an alternate outer Default
payload lifetime, owner mutation, owner consumption after alias creation, or
loan expiry.

## Frozen packet scope

The 12 independent Sources are in
`crates/jett_driver/tests/native_conformance/ordinary_borrowed_sums/`.

| Source | Concrete behavior | Reference outcome |
|---|---|---|
| `01_optional_branches.jett` | Some/None across written and bare owned actuals to a View sum formal; repeated occupied aliases; nested scalar Handle Default inside the None continuation | success, exact branch markers, lengths and sums |
| `02_result_branches.jett` | Ok/Fail across written and bare owned actuals; repeated occupied aliases and exact string failure payload | success, exact branch markers, lengths and sums |
| `03_owned_scoped_forwarded.jett` | Immutable owned sum roots, forwarded/nested/loop aliases, explicit independent clones and repeated backing reads; scalar/list generic instances | success, exact sums and generic reachability markers |
| `04_named_prefix_return.jett` | Both kinds; original alias View actual evaluated before a later handler Return, with lexical named order differing from formal order; retained caller roots read again | success, exact argument-order and return markers; consuming body absent |
| `05_optional_terminal_prefix.jett` | Optional alias View actual staged before a later terminal argument failure; other owners retained | exact terminal failure after the occupied alias prefix |
| `06_result_terminal_prefix.jett` | Result counterpart of Source05 | exact terminal failure after the occupied alias prefix |
| `07_outer_pending_some.jett` | Twice-pending Some sum | Handle fails before handler or alias observation |
| `08_outer_pending_none.jett` | Twice-pending None sum | same boundary, no failure continuation |
| `09_outer_pending_ok.jett` | Twice-pending Ok sum | same boundary |
| `10_outer_pending_fail.jett` | Twice-pending Fail sum with string companion | same boundary |
| `11_pending_optional_payload.jett` | Ready Some containing a twice-pending list; alias forwarding succeeds | payload observer fails after `afteralias` |
| `12_pending_result_payload.jett` | Ready Ok counterpart of Source11 | same boundary |

Sources01/02 pass owned sum bindings bare only at their last use. An existing
borrowed payload alias must be passed with explicit `view`; an initial draft's
bare alias call was correctly rejected by E0401 and repaired before freezing.
Source03's generic helper deliberately returns a reachability marker: its
generic rows establish alias initializer admission, not observation of generic
payload contents. Concrete list payloads are observed separately by length and
numeric sum. Identical Source outputs alone do not prove absence of an implicit
clone; implementation acceptance must also preserve the checked nonowning origin
and ownership facts.

## Actual reference evidence

The focused reference test is
`ordinary_borrowed_sums_reference_packet_matches_exact_source_outcomes`, in
`crates/jett_driver/tests/native_conformance/ordinary_borrowed_sums.rs`.
Every Source first passes `build_file_with_options`, then executes through
`run_file_capture_outcome`. The final formatted packet passed all 12 cases:
four success outcomes and eight exact runtime failures. The runner checks the
whole stdout, exact terminal message, and empty runtime/frontend debug streams.

Sources05/06 end with
`runtime error: list.__remove_at: index -1 out of bounds`. Sources07-10 preserve
the exact nested pending Some/None/Ok/Fail rendering in the existing Handle
error. Sources11/12 end with
`runtime error: list.__sum: argument must be a list`, after `afteralias\n`.
The Rust packet lists every exact stdout/error expectation.

All 12 Sources were formatted with the built Jett CLI, then passed
`jett format --check`. The Rust module was formatted with
`rustfmt --edition 2024 --config skip_children=true`.

## Native baseline and acceptance gate

The focused native test is
`ordinary_borrowed_sums_native_packet_matches_reference_without_source`.
For each genuine Source, it repeats the checked reference run, attempts native
object emission with both `BuildOptions { release: false }` and
`BuildOptions { release: true }`, and records every refusal. Object preflight
reaches the actual native frontend/MIR/verifier before building a runtime
archive. A refused object is a failing parity result, not an expected passing
assertion or an ignored test.

If emission succeeds, the same gate builds a profile-matched linked executable,
deletes its temporary Source, asserts Source absence, executes the bounded
binary, and compares its status, stdout and stderr to the reference. Temporary
Sources are deleted even when object emission refuses. Baseline refusals produce
no binary, so Source-independent binary execution and matching remain unproven
until this gate passes.

The final native baseline reached all 24 profile attempts against unchanged
production bytes: all 12 Sources refused in both profiles. Every actual refusal
reports
`NativeBuildError::Codegen` containing
`CodegenError::UnsupportedMir { construct: "failure handler", ... }`.
The native command exited 101 with 24 refusals after 34.44 seconds. No native
object or binary was produced and no native binary was executed. The gate remains
failing; no MIR/codegen/runtime fix is included here.

## Evidence storage and reproduction

Only this Cargo target was used:

```text
C:/Users/Vycto/Documents/GitHub/jett/target/native-ordinary-borrowed-sum-baseline-build
```

Its `baseline-evidence/` directory contains the raw final reference/native
process logs, exact copied Source/Rust/documentation bytes, and SHA-256 manifests.
The production manifest covers all crate `src/` files, crate manifests/build
scripts, stdlib files, and root Cargo manifest/lockfile: 332 files. Its initial
SHA-256 is
`842a746b3a70128be7c4de351e2e97a8b07154ea013426d250e859f31e88ee9d`.
The final production manifest matches it byte-for-byte.

From the isolated worktree, with `CARGO_TARGET_DIR` set to the path above:

```text
cargo test -q -p jett_driver --test native_conformance ordinary_borrowed_sums_reference_packet_matches_exact_source_outcomes -- --nocapture
cargo test -q -p jett_driver --test native_conformance ordinary_borrowed_sums_native_packet_matches_reference_without_source -- --nocapture
```

An initial sandboxed run could compile the driver test but could not scan this
worktree's stdlib because directory access was denied. The final authorized
worktree runs used the same dedicated target with expanded filesystem access;
that environment failure is not language or native parity evidence. Existing
compiler warnings are retained in the logs. No existing primary-worktree paths,
commits, pushes, PRs, or native coverage denominators were changed.
