# Native Resource GNU acceptance gate

This change prepares a private GNU Resource acceptance gate. Full GNU Rust
archive builds and Source execution remain unverified until the new CI steps pass.
The existing production package and Source-absent installed jobs remain required.

## Trust contract

`tools/measure_resource_native_archives.py prepare` builds fresh Debug and Release
archives on the exact `x86_64-unknown-linux-gnu` host. Cargo receives
`--cfg test --cfg jett_resource_native_test_archive` only after `cargo rustc --`
for the selected runtime package. Private archive target directories, the gate's
compiler target directory, and production package artifacts stay separate.

The helper measures unchanged runtime/Cargo inputs, compiler and linker identity,
actual ordered `native-static-libs` arguments including duplicates, complete
symbol inventories, Cargo fingerprints, and ELF observations. Each archive must
contain the exact 37 Resource leaves, eight private test exports, and one
`main`. A separate C probe observes the dynamic GNU CRT and `-no-pie` linker
contract; that probe does not establish Resource Source execution.

Rustup proxy invocation paths are preserved while executable bytes are measured.
Compiler overrides and wrappers are rejected, Cargo uses the measured compiler,
and actual verbose commands must match its identity. Cargo output is forced
uncolored through `CARGO_TERM_COLOR=never` and `--color never`, so command and
library observations are parsed without discarding unexpected escape bytes.
Raw fingerprint bytes and all provenance records are retained for review.

The separate `run` command receives the independently measured receipt digest
and embeds it in the compiled test binary. Runtime environment supplies only
the receipt path; it cannot replace the expected digest. The existing local MSVC
receipt pin remains the fallback for compilation without a CI pin. The harness
rehashes runtime/archive and measured linker bytes before each link, and the
helper revalidates archives, nested provenance, and Source/compiler inputs before
recording acceptance.

The ignored lifecycle test executes the complete shared Source corpus in both
profiles. Every execution deletes its primary and synthetic stdlib Source before
launch, then checks exact events, messages, statuses, consumed script, and zero
obligations before teardown. CI retains these measurements and execution evidence
separately from production packages.

## Invocation

On the supported GNU host, use a fresh output directory and a fresh gate target:

```text
python tools/measure_resource_native_archives.py prepare --output target/native-resource-gnu
python tools/measure_resource_native_archives.py run --receipt RECEIPT_PATH --expected-sha256 MEASURED_DIGEST --target-dir target/native-resource-gnu-gate
```

Preparation reports its receipt path and digest. CI obtains them through the
preparation step's outputs; execution must not derive its own expected pin from
an arbitrary runtime receipt.

## Actual verification

At parent `d1ffd08c`, the revised harness passes all 73 accepted Source scenarios
in Debug and Release on MSVC: **146 Source-deleted executions**, in 533.06 seconds
(`bbaf50`, `target/gnu-private-resource-msvc-accepted-source-regression.log` in
the main workspace). The runtime-environment forged-pin refusal test also passes.
The final helper-only color change does not alter this Rust harness.

Fifteen focused helper groups pass on Windows and the existing Ubuntu WSL host.
Python syntax, Rust formatting, and diff whitespace checks pass. Real GNU
`cc`/`ar`/`nm` controls accept the exact stub symbol inventory, reject duplicate
definitions, and observe dynamic CRT linking through `readelf`; their evidence is
retained under `target/gnu-private-resource-host-tools-real/` in the main workspace.
Those C controls are not Rust Resource archive or Source acceptance.

No accessible GNU Rust toolchain or installed strict Python checker was available
for this local verification. **Full GNU Rust Source execution and strict Python
type checking remain unverified.** Broader language, provider, concurrency, and
coherent-revision platform requirements remain in the native parity plan.