# Content-Addressed Compilation Cache Contract

Status: selected design; implementation not started. This document closes the
design scope tracked by [#153](https://github.com/vycdev/jett/issues/153).

## Purpose And Scope

Jett needs persistent compiler reuse without making cache contents, host paths,
worker timing, or old compiler state part of source meaning. The cache is a
local performance layer. A clean compilation remains the authority, and every
valid program must produce the same result with an empty, warm, read-only,
corrupt, or disabled cache.

The first persistent artifact is deliberately narrow:

```text
exact source bytes
    -> successful whole-file parse
    -> canonical serialized ParsedFile artifact
```

This builds on the implemented `jett_query` whole-file parse boundary. A cache
hit reconstructs the parser-owned direct AST and non-error lexer/parser
diagnostics for the caller's current `FileKey` and diagnostic `FileId`.
Resolution, type checking, comptime evaluation, verification, interpretation,
and current `BuildResult` values still execute normally.

The initial cache does not persist Salsa state, failed parses or builds,
whole-project checked results, runtime values, profiler data, HIR, MIR, LLVM IR,
interpreter bytecode, or native objects. Later compiler representations may add
new artifact kinds only after they have stable ownership and canonical wire
schemas.

Remote or shared caches, distributed compilation, package registries,
dependency fetching, and dependency-integrity policy are out of scope.

## Why Parsing Is The First Layer

The current compiler has one stable query-owned result that can be isolated
from process-local state: `ParsedFile` for one exact source revision. Its AST
and diagnostics are immutable and owned by that result. Source parsing depends
on exact text and frontend policy, not on unrelated project files, build mode,
worker count, target, or output path.

Current later-phase values are not safe persistent artifacts:

- `BuildResult` is a command-facing aggregate, not a checked-program format;
- spans and expression-type/comptime maps contain process-local `FileId` values;
- resolver definitions, type identities, and interned symbols are assigned in
  process-owned tables;
- project support modules are still assembled into disposable direct ASTs;
- HIR, MIR, code generation, and native artifact formats are not implemented;
- verify and comptime execution belong to a complete current revision and are
  not smuggled across invocations as remembered success.

Persisting those values by serializing their current Rust memory shape would
turn incidental implementation details into compatibility promises. The first
stage instead proves keying, validation, rehydration, publication, and cleanup
on the smallest useful immutable result.

A later checked-program or backend cache must be justified by measurements and
receive its own stable artifact kind. It does not replace or silently widen the
parse artifact.

## Correctness Boundary

Persistent caching follows these invariants:

1. Cache lookup happens only after discovery has supplied current source bytes,
   logical source identity, provenance, and semantic compiler options.
2. A hit is accepted only after the object name, envelope, per-user
   authenticator, key record, payload, and current inputs all validate.
3. A miss or any cache operational failure falls back to ordinary compilation.
4. Cache I/O cannot turn accepted source into rejected source or the reverse.
5. Human, `--agent`, LSP, and ASP consumers receive the same compiler facts and
   diagnostic order whether the artifact was decoded or freshly computed.
6. Cancelled, stale, failed, panicked, or partially published work writes no
   reusable object.
7. An artifact never grants compiler-shipped stdlib provenance, trusted-hook
   authority, namespace ownership, a capability, or permission to skip policy
   checking.
8. Cache controls, location, size, hit state, and cleanup timing are
   non-semantic orchestration settings and never query-key inputs.

A cache implementation must keep an uncached path in ordinary tests. The cache
is an optimization defect if disabling or deleting it changes compiler output.

## Artifact Kinds And Dependency Graph

Every persistent object has a distinct artifact-kind tag. The selected initial
tag is:

```text
jett.parse-file.v1
```

Future kinds use separate tags and schemas, for example checked summaries, HIR,
MIR, interpreter bytecode, or target objects. These names are illustrative
until those representations exist; `jett.parse-file.v1` must not carry a
later-phase payload under the old tag.

An aggregate artifact records the keys of every artifact or canonical input it
consumed. Its key is computed from those dependency keys plus its own semantic
inputs. The dependency list is ordered by the language's canonical manifest or
work-plan order, never filesystem traversal, hash-map iteration, worker
completion, or cache lookup order.

A cache hit at one layer does not imply a hit above it. Editing one source file
may reuse parse artifacts for all unchanged files while correctly invalidating
the project semantic result that depends on the changed manifest.

## Digest And Canonical Key Encoding

Cache object identifiers use SHA-256 and lowercase hexadecimal. This is an
internal compiler digest, not a call to the Jett `crypto` stdlib and not a
password-security claim. The compiler implementation uses a reviewed Rust
cryptographic digest implementation so cache identity does not depend on
executing Jett source.

The preimage is a canonical binary key record, not delimiter-concatenated text:

```text
CacheKeyRecord:
    magic = "jett-cache-key"
    key_format_version: u32
    artifact_kind: length-prefixed UTF-8
    artifact_schema: u32
    compiler_compatibility_id: length-prefixed bytes
    semantic_fields: ordered tagged length-prefixed byte strings
```

Integers are unsigned little-endian. Lengths are checked before allocation.
Tags are unique within a schema. Unknown required tags, duplicate tags,
non-canonical ordering, trailing bytes, invalid UTF-8 in text fields, and
integer overflow reject the key record.

The object key is:

```text
sha256(canonical CacheKeyRecord bytes)
```

The envelope stores the full key record and digest. A reader recomputes the
digest and requires it to match both the requested key and the object filename.
SHA-256 collision resistance is part of the content-addressing assumption, but
a digest match never bypasses authentication or structural validation. An
unkeyed digest detects accidental corruption; it does not prove that a writer
authorized by the current user produced the AST payload.

## Initial Parse Key

`jett.parse-file.v1` has exactly these tagged `semantic_fields`:

1. exact source length as an unsigned 64-bit byte count;
2. SHA-256 digest of the exact UTF-8 source bytes;
3. lexical/parser policy version when such policy exists outside the compiler
   compatibility identity.

The fixed `artifact_kind`, `artifact_schema`, and
`compiler_compatibility_id` members of `CacheKeyRecord` already participate in
the object key. They are encoded once in those fixed positions and are not
duplicated in `semantic_fields`, so a record cannot carry two conflicting
schema or compatibility values.

The complete source bytes are already available to the caller and are checked
against the recorded length and digest before decoding. Newline spelling,
Unicode normalization, comments, and whitespace are not normalized: parsing
uses the exact bytes provided by discovery or the editor.

The initial parse key deliberately excludes:

- absolute checkout and cache paths;
- normalized logical path and source origin, because current parsing does not
  branch on them;
- process-local `FileId`, Salsa intern IDs, AST addresses, and arena indices;
- project name, entry file, manifest membership, and unrelated source files;
- stdlib manifest order and project configuration;
- debug/release mode, target, backend, linker, and toolchain;
- worker count, completion order, renderer, terminal color, output path, clock,
  process ID, hostname, and environment variables.

Excluding logical path and origin allows identical source bytes to share one
parse payload. Rehydration always applies the caller's current `FileKey`,
`FileId`, and provenance. If a future lexer or parser makes origin, path,
edition, target, or another setting semantic, that setting becomes a tagged
field in a new parse schema before the behavior ships.

## Compiler Compatibility Identity

The cache must invalidate when compiler code or data that can change an
artifact changes. Package version alone is insufficient during development,
and a process-local pointer or executable timestamp is not reproducible.

Each compiler build therefore exposes a deterministic
`compiler_compatibility_id` derived from:

- the Jett compiler package version;
- a source revision or reproducible source-tree digest for the crates that
  produce or consume the artifact;
- the locked versions and enabled semantic features of those crates;
- the bundled stdlib digest for artifact layers that consume stdlib semantics;
- the artifact schema and compiler policy revision.

Official releases record this identity in release metadata. A development build
without a trustworthy source revision computes a deterministic digest from the
relevant source and lock inputs; if the build cannot provide one, persistent
cache reads and writes are disabled rather than sharing under an ambiguous
`dev` identity.

A source-tree digest uses normalized repository-relative logical paths and
exact file contents in canonical order. Checkout roots, file timestamps,
directory-enumeration order, and other host metadata are excluded, so the same
compiler source produces the same identity after relocation.

The identity is scoped by artifact layer. A CLI-rendering-only change need not
invalidate parse objects, while a lexer, parser, AST, diagnostic, or decoder
change does. Implementations may conservatively invalidate more often. They may
never reuse across a change that can alter the reconstructed compiler fact.

## Future Layer Keys

Each future artifact kind documents every semantic input before it can be
persisted. The following rules apply:

- A project semantic artifact includes the ordered manifest of source origin,
  normalized logical path, source digest, and dependency artifact key.
- Compiler-shipped stdlib input includes its trusted origin, compatibility
  identity, ordered depth-then-lexical fragment manifest, and exact contents.
- Parsed `jett.proj` fields enter only the layers whose behavior they affect;
  raw presentation-only configuration does not.
- Build/test and debug/release modes enter the first layer whose checked policy
  differs.
- Target triple, data layout, CPU features, backend identity and options,
  runtime ABI, and toolchain/linker identities enter target-dependent lowering
  and native-object keys. They do not preemptively invalidate parsing.
- Output filenames and directories are publication destinations, not artifact
  identities, unless their value is deliberately embedded in a future output.
- Environment, clock, network, host load, worker count, and scheduling order are
  forbidden implicit inputs. Any future legitimate semantic host input must be
  captured explicitly before query execution and included canonically.

A key schema is reviewed like a public compiler policy. Adding an invalidating
input requires a schema or compatibility revision; silently reading ambient
state is not acceptable.

## Canonical Parse Artifact

The parse payload uses a compiler-owned canonical binary schema. It is not
`bincode`, Rust ABI layout, an arbitrary `serde` derive over AST structs, a Salsa
snapshot, or a dump of process memory.

The payload contains:

- the direct AST node kinds and fields required by the parser's public result;
- identifier and literal text as exact length-prefixed UTF-8 or byte data;
- source byte ranges as checked start/end offsets relative to the one source;
- stable diagnostic codes, severities, messages, labels, and suggested fixes
  for non-error diagnostics produced by a successful parse;
- explicit option, list, and enum tags;
- no source authority or trusted status.

The payload does not contain:

- `FileKey`, `FileId`, absolute or logical paths, source origin, or cache paths;
- pointers, `Arc` identities, vtables, Rust enum discriminants, native `usize`,
  hash-table capacity or iteration order, or Salsa handles;
- source text duplicated as an authority for the caller's current input;
- resolver definitions, type IDs, reflection metadata, comptime values, or
  interpreter state.

Lists preserve parser order. Any map-like field is serialized as entries sorted
by its documented canonical structural key. Numeric values use fixed widths and
little-endian encoding. Floating-point payloads, if a future AST representation
stores parsed floats rather than token text, use exact IEEE bits with one
specified NaN policy.

The decoder reconstructs fresh owned AST values. It binds every span and
non-error diagnostic to the caller's current `FileId`. The current source map
and logical path remain caller-owned, so diagnostics from a relocated checkout
report the current path rather than a cached path.

## Envelope And Validation

Each object is one immutable envelope:

```text
CacheEnvelope:
    magic = "JETTCACHE"
    envelope_version: u32
    artifact_kind
    artifact_schema
    key_digest[32]
    canonical_key_record
    uncompressed_payload_length: u64
    stored_payload_length: u64
    payload_digest[32]  # SHA-256 of the canonical uncompressed payload
    payload
    authenticator[32]
```

The envelope encoding is canonical for its version: fields appear in the order
above, integers are unsigned little-endian, variable-width fields are preceded
by their checked byte lengths, and padding or trailing bytes are forbidden.
The authenticator is the final 32 bytes.

Compression may be added only as an envelope-versioned storage choice with a
fixed algorithm identifier and strict decoded-size bound. Compression bytes do
not enter semantic keys. `stored_payload_length` bounds the bytes present in the
envelope, while `uncompressed_payload_length` and `payload_digest` authenticate
the canonical bytes produced after decoding. The initial implementation may
remain uncompressed, in which case both lengths are equal and the stored bytes
are the canonical payload.

A reader performs all of these checks before returning a hit:

1. open a regular file without following a final symlink where the platform
   supports that guarantee;
2. enforce fixed header and declared-size bounds before allocation;
3. validate magic, envelope version, artifact kind, and schema;
4. parse the canonical key record, require its artifact kind and schema to
   equal the envelope and requested artifact, and recompute its SHA-256 digest;
5. require the digest to match the requested key and filename;
6. verify the per-user object authenticator before decoding the payload;
7. require source length/digest and all current semantic inputs to match;
8. verify stored and decoded payload lengths and the payload digest;
9. decode with bounded nesting and collection counts;
10. require every source range to be ordered, within the current source length,
   and valid for the field's UTF-8 boundary requirements;
11. reject unknown required node tags, duplicate fields, non-canonical map
    order, trailing bytes, and impossible AST invariants.

An individual object is never allowed to request more than 64 MiB of decoded
memory in the initial cache. Artifacts exceeding that storage bound compile
normally and are not cached. Decoder recursion is replaced by an explicit
bounded stack or enforces a documented maximum below process stack risk.

After decode, optional debug builds may re-run structural AST validation. A hit
is published only after the same invariants required by a fresh parser result
hold.

## Success, Failure, And Corruption Outcomes

Cache lookup returns one internal outcome:

```text
Hit(artifact)
Miss
Incompatible
Corrupt
Unavailable
```

Only `Hit` changes performance. Every other outcome runs the ordinary compiler
path.

- **Miss:** no object exists for the requested key.
- **Incompatible:** a recognized object uses another envelope, artifact, or
  schema version. Versioned directories should make this uncommon.
- **Corrupt:** the authenticator, name, digest, lengths, structure, spans, or
  payload fail validation.
- **Unavailable:** permissions, I/O, storage, lock, or platform support prevent
  safe cache use.

Incompatible or corrupt objects are best-effort removed or quarantined only
when the process owns a writable cache. Cleanup failure is not a compiler
failure. The compiler does not repeatedly retry a bad object during one
invocation.

The initial cache stores only fully completed parse results without error
severity diagnostics. Parse failures, resolution/type errors, failed verify or
comptime execution, internal compiler failures, panics, cancellation, stale LSP
revisions, and interrupted publication are not negative cache entries. A future
negative cache requires a separate policy for diagnostic compatibility and may
not be inferred from this design.

## Provenance And Trust

A cache directory is untrusted input even when it is user-private. Another
process, an old compiler, a restored backup, filesystem corruption, or a project
with access to an explicitly shared directory may modify it.

To prevent a writer with cache-directory access alone from substituting a
different structurally valid AST for the same source key, every object carries
an HMAC-SHA-256 authenticator. Its input is defined exactly as:

```text
HMAC-SHA-256(
    user_cache_key,
    "jett-cache-object-auth-v1" || canonical CacheEnvelope bytes through payload,
)
```

The covered envelope bytes include the key record, lengths, payload digest, and
stored payload exactly once; only the final authenticator field is absent. The
32-byte `user_cache_key` lives in the current user's private Jett configuration
directory, outside the deletable cache root.

The first read-write cache use creates this key from the operating system's
cryptographically secure random source with create-new and user-private
permissions. Concurrent creators read the winner. A read-only process without
an existing key treats all objects as misses. Removing or rotating the key
invalidates old objects without affecting compilation. The key and MAC never
appear in diagnostics, event records, object names, source manifests, or agent
output.

A reader computes the MAC before interpreting AST bytes and compares the
computed and stored tags in constant time. A missing or incorrect MAC is
corruption, even when all public hashes and node invariants look valid. This
keeps a copied, shared, or attacker-written cache from changing compiler facts
unless that writer also has access to the current user's separate private
authentication key. Compromise of the whole OS user account, remote-cache
identities, multi-user key distribution, and hardware key protection remain
outside this local cache contract.

An authenticated decoded object proves only that bytes passed the current
user's MAC, structural, and digest checks. It does not prove:

- that a source is compiler-shipped stdlib;
- that a private hook is trusted;
- that project or dependency namespace ownership is valid;
- that a dependency came from a reviewed Git commit;
- that a capability is available;
- that later semantic checks may be skipped.

Current discovery assigns `SourceOrigin`; stdlib loading proves compiler-shipped
origin; resolution and checking enforce namespace and trusted-hook policy. A
cached parse payload is rebound beneath those current facts.

The cache never loads executable code, dynamic libraries, allocator state, raw
pointers, file descriptors, opaque runtime-resource handles, or host commands.
Native objects, when eventually supported, remain data until a validated
current build plan selects and publishes them under a target/toolchain key.

## Source Privacy

Parse artifacts contain identifiers, literals, and other source-derived data.
The default cache is private to the current OS user and is never uploaded. On
Unix-like systems, newly created cache directories and files request modes
`0700` and `0600`; implementations apply the closest user-private equivalent on
other platforms.

Cache errors and telemetry must not print source text, literal contents, full
object payloads, tokens, environment variables, or home-directory paths.
Human-readable diagnostics may identify an object by a shortened digest and
cache outcome. Agent output receives no source-derived cache detail beyond what
the ordinary compiler result already exposes.

An explicitly shared cache directory does not weaken validation or provenance
rules. Remote caches, multi-user trust, signing, encryption at rest, and tenant
isolation require a separate contract.

## Location And Controls

The default root follows the platform's per-user cache convention:

- Linux and other XDG systems: `$XDG_CACHE_HOME/jett/compiler`, falling back to
  `$HOME/.cache/jett/compiler`;
- macOS: `$HOME/Library/Caches/jett/compiler`;
- Windows: the current user's local application-data cache directory under
  `Jett/compiler`.

The cache does not default inside the source tree and is not discovered as a
project or dependency. Repository copies, bundles, and source manifests never
include it.

The first implementation exposes explicit orchestration controls equivalent to:

```text
--cache off
--cache read
--cache read-write
--cache-dir PATH
```

Normal CLI use defaults to `read-write`; tests that assert compiler behavior
default to `off` unless they are cache tests. LSP may use the same per-user
store but retains its in-process database as the first reuse layer. ASP and CLI
one-shot invocations may reuse persistent objects across processes.

`--cache-dir` changes storage only. `--agent`, renderer selection, terminal
color, and worker count do not change cache keys or cached compiler facts. No
source-level operation can inspect, flush, seed, or depend on the compiler
cache.

## Filesystem Layout

Objects are immutable and addressed by digest:

```text
<root>/
    v1/
        objects/
            sha256/
                ab/
                    abcdef...64-hex-digits.jca
        tmp/
        state/
            gc.lock
```

The two-character fan-out limits directory size. A filename not matching its
canonical lowercase digest is ignored by readers and eligible for cleanup.
Temporary files live on the same filesystem as final objects so publication can
use an atomic rename.

Artifact schema and compiler compatibility remain inside keys and envelopes.
The root `v1` separates incompatible storage protocols; a new protocol uses a
new directory instead of guessing how to read old bytes.

## Atomic And Concurrent Publication

A writer follows this sequence:

1. compute the complete artifact in memory from one successful current
   revision;
2. encode, authenticate, and validate its envelope before touching the final
   path;
3. create a unique user-private temporary file with create-new semantics;
4. write all bytes, flush the file, and close it;
5. check for cancellation or stale revision before publication;
6. atomically install the temporary file at the digest path without replacing a
   valid existing immutable object;
7. if another process won the race, validate the winner and discard the
   temporary file;
8. best-effort sync the containing directory when the platform supports durable
   directory metadata.

A reader opens only the final object path. It never reads a temporary file and
never waits for a writer lock.

Two writers for the same key must produce byte-identical canonical payloads. A
race winner is not trusted merely because it arrived first. If the existing
object is invalid, one process may quarantine it and retry one atomic
publication; implementations avoid unbounded replacement loops.

Objects for different keys require no shared publication lock. Garbage
collection has one non-blocking `gc.lock`; failure to acquire it skips cleanup.
It never blocks a build and never authorizes partial-object reads.

Cancellation after an immutable object was fully and atomically published need
not remove that object if the object came from a complete successful query fact
and is independent of the cancelled aggregate. Cancellation or staleness before
that point removes only the writer's temporary file and publishes nothing.

## Lifecycle And Eviction

The default cache budget is 2 GiB. After a successful write, a process may
attempt cleanup when the store exceeds that budget. Cleanup acquires the
non-blocking GC lock and evicts to 75 percent of the budget.

Recency metadata is performance-only. A successful hit may best-effort update
an object's access time at most once per 24 hours. Cleanup sorts candidates by
recorded access time, then full digest as the deterministic tie-breaker.
Missing, invalid, or equal timestamps use the digest order. The clock used for
cleanup never enters compiler keys or results.

Cleanup first removes abandoned temporary files older than 24 hours and invalid
filenames. It then removes incompatible, corrupt, and oldest valid objects until
under the target. An object opened by a reader may survive unlinking through
ordinary filesystem semantics; platforms that prevent deletion simply skip it.

Budget calculation, timestamp failure, full disks, read-only filesystems, and
cleanup errors never fail compilation. A user can delete the entire cache while
no compiler process depends on it; correctness is equivalent to a cold miss.

Tests inject filesystem and clock behavior rather than sleeping or depending on
host access times.

## Diagnostics And Observability

Ordinary successful commands do not print cache hits or misses. Cache status is
available through explicit verbose/debug instrumentation and test-only event
observers, with stable categories rather than timing claims:

```text
parse_lookup_hit
parse_lookup_miss
parse_lookup_incompatible
parse_lookup_corrupt
parse_lookup_unavailable
parse_write_published
parse_write_raced
parse_write_skipped
```

The observer records artifact kind and digest, never AST payload or source text.
Cache metrics do not alter human or agent diagnostic order. Elapsed time is
useful for profiling but is not proof of a hit; tests assert event categories
and query execution counts.

A cache warning is emitted only for an explicit cache-management or strict
diagnostic command. Normal builds silently fall back on transient cache I/O,
corruption, or quota failures so optimization state does not create noisy or
non-reproducible build output.

## Relationship To Salsa And Parallel Compilation

Salsa memoization remains the authoritative in-process revision graph. The
persistent store does not serialize the Salsa database or restore tracked
handles. It may supply an immutable value at a query implementation boundary,
after which Salsa owns ordinary dependency tracking for the current process.

The initial lookup order is:

```text
Salsa memo for current revision
    -> persistent parse artifact
    -> fresh lexer/parser execution
```

A persistent hit counts as execution of the query implementation for current
Salsa dependency purposes but avoids fresh parsing. Cache tests distinguish
Salsa memo hits from persistent hits.

Parallel workers may look up independent objects concurrently. They return
owned values to the coordinator, which retains canonical manifest and diagnostic
ordering. Workers never publish project success, diagnostics, or output files.
Only a complete artifact from one revision reaches the atomic writer boundary.
Worker count and race order cannot affect keys or bytes.

The deterministic parallel compilation contract remains authoritative for
cancelled, stale, failed, and partially merged stages. Persistent caching does
not weaken its atomic publication boundary.

## Future Checked And Backend Artifacts

A later artifact layer must satisfy all of these gates before implementation:

- an immutable query-owned compiler representation exists;
- every persisted identity is stable across processes and ordinary edits;
- a canonical schema exists independently of Rust layout;
- all semantic inputs and dependency keys are enumerated;
- source maps and diagnostics rehydrate against current logical sources;
- trusted stdlib, namespace, capability, secret, and ownership checks cannot be
  inherited from an unvalidated blob;
- clean-versus-warm equivalence and corruption fallback tests pass;
- measured reuse justifies serialization and validation cost.

Checked-program artifacts include canonical type and declaration identities,
not process-local `TypeId`, `DefId`, span-keyed hash maps, or interner numbers.
HIR and MIR artifacts wait for #20 and #22 to select their ownership and
identity boundaries. Native artifacts include exact target, backend, runtime
ABI, code-generation options, and linker/toolchain compatibility.

A future native cache may reuse validated objects but the current build still
owns final output publication. It writes the requested destination only after
all current dependencies succeed, using the same validation-first atomic output
rules as an uncached build.

## Bounded Implementation Sequence

1. **Protocol and decoder foundation**
   - add an internal cache module with canonical key/envelope codecs;
   - expose compiler compatibility identity;
   - create and protect the per-user authentication key and verify object MACs;
   - implement bounded, panic-free untrusted decoding and filesystem layout;
   - add injected filesystem, clock, cancellation, and event-observer seams.
2. **Parse artifact codec**
   - define stable direct-AST node tags and relative source spans;
   - round-trip every parser node used by current fixtures;
   - rebind one payload to different current `FileKey`/`FileId` values;
   - reject malformed and semantically impossible payloads.
3. **Read-through integration**
   - consult persistent storage inside the `parse_file` query implementation;
   - preserve Salsa revision dependencies and current source ownership;
   - fall back silently on miss, incompatibility, corruption, or unavailable
     storage;
   - keep ordinary builds and tests able to force cache off.
4. **Atomic writes and lifecycle**
   - publish only complete successful parse artifacts;
   - implement race-safe immutable writes, temporary cleanup, budget accounting,
     deterministic eviction tie-breaks, and non-blocking GC;
   - expose explicit cache mode/directory controls.
5. **Measure before widening**
   - compare encode/decode cost with fresh parsing on representative projects;
   - verify cross-process reuse and cold/warm equivalence;
   - propose a separate artifact kind only when a stable later representation
     and measured benefit exist.

Each stage keeps the compiler correct with cache mode `off`. Shipping a codec
without read-through integration is not a claimed compiler speedup; observing a
file on disk is not proof that a later compiler query reused it.

## Required Verification Matrix

### Keys and compatibility

- equal source and compiler inputs produce equal keys across processes,
  checkout locations, and worker counts;
- one-byte source, parser policy, schema, relevant compiler source/dependency,
  or compatibility change produces a different key;
- path and origin changes reuse the parse payload while rehydrating current
  identity and provenance;
- target, output path, renderer, color, and worker count do not change the parse
  key;
- future aggregate keys change for ordered source/dependency/stdlib manifest,
  semantic configuration, mode, target, backend, and toolchain changes exactly
  where those inputs become relevant.

### Codec and validation

- every current lexer/parser AST shape round-trips structurally;
- exact source ranges, literals, identifiers, and non-error diagnostics survive;
- decoded spans bind to the current `FileId` and current logical source path;
- fixed key identity fields cannot be duplicated, and an envelope/key-record
  artifact-kind or schema mismatch is rejected;
- truncated headers, oversized lengths, integer overflow, unknown tags,
  duplicate fields, invalid UTF-8, invalid spans, impossible nodes, trailing
  bytes, wrong names, missing/forged MACs, and key/payload digest mismatches are
  rejected without panic or excessive allocation;
- authenticator test vectors pin the domain separator and prove every stored
  envelope byte through the payload is covered exactly once;
- deterministic encoding is byte-identical across repeated runs;
- fuzzed arbitrary object bytes terminate safely under the size and nesting
  bounds.

### Correctness and invalidation

- empty, warm, read-only, corrupt, unavailable, and disabled caches produce the
  same accepted/rejected source, diagnostics, and agent output;
- an unchanged second process records a persistent parse hit and avoids fresh
  lexer/parser work;
- editing file A misses only A while unchanged file B remains reusable;
- adding, removing, renaming, or relocating a file preserves correct project,
  namespace, source-map, and stdlib provenance behavior;
- project config and stdlib manifest ordering invalidate later semantic work but
  do not spuriously change independent parse keys;
- no cached parse skips resolution, type checking, comptime, verify, capability,
  secret, ownership, namespace, or trusted-origin policy.

### Failure and concurrency

- failed parses and builds, panics, cancelled work, and stale LSP revisions
  publish no initial-cache object;
- interruption at every write step leaves either no final object or one complete
  valid object;
- concurrent identical writers produce one valid immutable object and both
  callers continue correctly;
- concurrent readers never observe temporary or partial bytes;
- a corrupt race winner is rejected and bounded recovery does not loop;
- read-only directories, permission errors, full disks, failed flush/rename,
  deleted objects, and cleanup-lock contention fall back without changing build
  results;
- parallel lookup and deliberately varied completion order retain canonical
  module and diagnostic ordering.

### Lifecycle and privacy

- default paths and permissions follow the current user's cache convention;
- concurrent authentication-key creation selects one private key, key rotation
  invalidates old objects safely, and the key never enters cache logs or agent
  output;
- source scans, bundles, and project manifests never include cache objects;
- size accounting triggers non-blocking eviction to the selected target with
  digest tie-breaks;
- injected-time tests cover access throttling and temporary-file expiry without
  sleeps;
- logs and agent output never expose source payloads, literals, tokens, or full
  private cache paths.

A cache-hit claim is valid only when the persistent event observer shows a hit,
fresh parse execution is absent, and ordinary compiler output still matches an
uncached run.

## Deferred Scope

This contract does not define or implement:

- remote, distributed, organization-wide, or multi-tenant caches;
- artifact upload/download protocols, signatures, transparency logs, or remote
  attestation;
- cache encryption, source deduplication privacy across users, or tenant quotas;
- persistent Salsa database serialization;
- failed-build or diagnostic negative caching;
- checked-program, HIR, MIR, LLVM, bytecode, native-object, linker, test-result,
  profiler, or runtime-value artifacts;
- package registries, dependency download, lockfiles, or dependency-integrity
  hashing;
- compiler behavior that depends on cache availability;
- performance promises before representative measurement.

Those features must preserve the local cache's explicit inputs, canonical
identity, untrusted decoding, current-provenance checks, and clean-build
fallback rather than widening trust in stored bytes.
