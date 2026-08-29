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

The initial cache does not persist Salsa state, parse or build failures,
whole-project checked results, runtime values, profiler data, HIR, MIR, LLVM IR,
interpreter bytecode, or native objects. It may retain a completed successful
parse query reached during a build that later fails, because that immutable
source fact is not a cached build outcome. Later compiler representations may
add new artifact kinds only after they have stable ownership and canonical wire
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
6. Only a complete successful parse query for the current source revision may
   publish. Cancellation or staleness observed at the final prepublication
   check, or a panic before installation, writes no reusable object. A signal
   racing after that check may leave the complete independent parse fact, and a
   later aggregate build failure or cancellation does not invalidate it.
7. An artifact never grants compiler-shipped stdlib provenance, trusted-hook
   authority, namespace ownership, a capability, or permission to skip policy
   checking.
8. Cache controls, location, size, hit state, and cleanup timing are
   non-semantic orchestration settings and never query-key inputs.

A cache implementation must keep an uncached path in ordinary tests. The cache
is an optimization defect if disabling or deleting it changes compiler output.
Parse-object publication is query-scoped and is not staged until whole-build
success. Build failures themselves are never cached.

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
    artifact_kind: u64-byte-length + UTF-8
    artifact_schema: u32
    compiler_compatibility_id: u64-byte-length + bytes
    semantic_field_count: u32
    semantic_fields:
        tag: u32
        value: u64-byte-length + bytes
```

Integers are unsigned little-endian. Each schema assigns nonzero tags and
requires fields in strictly increasing numeric-tag order; v1 has no optional
or repeated semantic tag. Counts and lengths are checked against the remaining
record and allocation bounds before conversion or allocation. An unknown,
zero, duplicate, missing, or out-of-order tag, trailing bytes, invalid UTF-8 in
text fields, or integer overflow rejects the key record. A future need for an
optional field requires a new artifact schema that defines its exact tag and
presence rule; readers do not guess must-understand bits.

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

`jett.parse-file.v1` uses `key_format_version = 1`,
`artifact_kind = "jett.parse-file.v1"`, `artifact_schema = 1`, and an exactly
32-byte `compiler_compatibility_id`. It has `semantic_field_count = 3` and
exactly these tagged `semantic_fields`:

1. tag `1`, source length: a value length of `8`, followed by the unsigned
   64-bit little-endian exact source byte count;
2. tag `2`, source digest: a value length of `32`, followed by SHA-256 of the
   exact UTF-8 source bytes;
3. tag `3`, parser policy: a value length of `4`, followed by an unsigned
   32-bit little-endian policy revision. Revision zero means there is no
   parser policy revision outside `compiler_compatibility_id`.

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

The digest preimage is a canonical record rather than concatenated path and
content text:

```text
CompatibilityInputRecord:
    magic = "jett-compat-input"
    format_version: u32 = 1
    domain: u32
    entry_count: u32
    entries sorted by logical_path UTF-8 bytes:
        logical_path: u64-byte-length + normalized UTF-8 bytes
        content: u64-byte-length + exact bytes
```

V1 assigns domain `1` to compiler source, `2` to locked dependency and semantic
feature/configuration inputs, and `3` to bundled-stdlib source. Integers are
unsigned little-endian. Logical paths are relative UTF-8, use `/`, preserve
repository case, and reject empty segments, `.`, `..`, NUL, and absolute forms;
sorting compares their exact UTF-8 bytes. Duplicate logical paths, an unknown
or zero domain, invalid normalized paths, count/length overflow, and trailing
bytes are rejected. SHA-256 of the domain-1 record is the source-tree component of
`compiler_compatibility_id`; domains 2 and 3 supply their corresponding digest
components. No path/content boundary or input class can be reinterpreted by
concatenation.

The compatibility ID itself is SHA-256 of a second canonical record with fixed
magic `jett-compiler-compat`, `format_version = 1` as a `u32`, a `u32`
component count, and strictly increasing `u32` component tags followed by
`u64`-length-prefixed bytes. V1 assigns tag `1` to UTF-8 package version, `2`
to source identity, `3` to the locked-dependency/feature digest, `4` to an
applicable bundled-stdlib digest, `5` to UTF-8 artifact kind, `6` to the
four-byte little-endian artifact schema, and `7` to the four-byte
little-endian compiler-policy revision. Source identity begins with a one-byte
discriminator: `1` plus a `u64` length and UTF-8 revision, or `2` plus the
32-byte domain-1 `CompatibilityInputRecord` digest. Tags `3` and `4`, when
present, contain the 32-byte domain-2 and domain-3 digests respectively.

For `jett.parse-file.v1`, `component_count = 6` and the required tags are
`1, 2, 3, 5, 6, 7`; it does not consume stdlib semantics and therefore omits
tag `4`. A future artifact layer that consumes stdlib fixes tag `4` as required
in its own compatibility schema. Missing required, unexpected, duplicate, or
out-of-order tags, invalid fixed value lengths, and trailing bytes reject the
record. This record uses the same unsigned-little-endian and bounded-length
rules as `CacheKeyRecord`.

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
above; integers are unsigned little-endian; `artifact_kind` and
`canonical_key_record` have unsigned 64-bit byte-length prefixes; and
`stored_payload_length` is the payload's length prefix. Fixed digests have
exactly 32 bytes. All lengths are bounded and checked before allocation.
Padding, alternate integer widths, and trailing bytes are forbidden. The
authenticator is the final 32 bytes.

The initial object uses `envelope_version = 1`, exact artifact kind
`jett.parse-file.v1`, and `artifact_schema = 1`. Its length-prefixed key record
must consume exactly its declared bytes, its payload must consume exactly
`stored_payload_length`, and end-of-file must follow the 32-byte authenticator.

Compression may be added only as an envelope-versioned storage choice with a
fixed algorithm identifier and strict decoded-size bound. Compression bytes do
not enter semantic keys. `stored_payload_length` bounds the bytes present in the
envelope, while `uncompressed_payload_length` and `payload_digest` authenticate
the canonical bytes produced after decoding. The initial implementation may
remain uncompressed, in which case both lengths are equal and the stored bytes
are the canonical payload.

A reader performs all of these checks before returning a hit:

1. open the final object as a no-follow regular file relative to its pinned
   validated fan-out handle; a platform without that guarantee returns
   `Unavailable`;
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

The platform-selected private configuration directory is pinned independently
of `--cache-dir` and uses the same handle-relative, no-follow component checks
as cache confinement below. The key filename is fixed as
`cache-auth-v1.key`. A reader opens it with
no-follow semantics and accepts only a regular file owned by the current user,
with the platform's user-private permissions, whose complete length is exactly
32 bytes. On Unix the containing configuration directory and key request modes
`0700` and `0600`; on Windows the DACL must deny write/read access to unrelated
user principals under the platform's user-private policy. A wrong type, owner,
permissions, or length is never truncated, padded, or used as key material.

First-use creation never exposes the final filename while bytes are partial:

1. acquire a non-blocking OS file lock on a no-follow regular
   `cache-auth-v1.init.lock` in the pinned private configuration directory;
2. re-check for a valid winner;
3. generate 32 bytes from the operating system's cryptographically secure
   random source;
4. create a unique no-follow temporary regular file in that directory with
   create-new and user-private permissions;
5. write exactly 32 bytes, flush them durably, close the file, and re-open it to
   validate type, ownership, permissions, and bytes;
6. atomically install it at `cache-auth-v1.key` with no-replace semantics and
   best-effort sync the directory;
7. if another process won, delete the temporary file and validate the winner.

A crash can leave only a uniquely named temporary file, never a partially
written final key. Initialization examines at most 64 names matching the exact
key-temporary grammar and removes at most 16 validated no-follow regular
temporaries per invocation; all others are ignored for a later bounded pass.
For a malformed legacy/final key, the process holding the initialization lock
renames it to one bounded quarantine name and retries the full protocol once;
failure, another malformed winner, or inability to guarantee atomic no-clobber
publication disables persistent cache reads and writes for that invocation.
Recovery never loops and never overwrites a valid key. Rotating or recovering
the key invalidates old objects but cannot change compilation results.

Concurrent creators validate the one installed winner. A read-only process
without an existing valid key treats all objects as misses and never creates or
repairs key state. A platform that cannot provide the required private-file,
no-follow, lock, durable-temp, and atomic no-replace guarantees disables the
persistent cache. The key and MAC never appear in diagnostics, event records,
object names, source manifests, or agent output.

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

### Path confinement

Every cache operation is confined beneath one pinned root-directory handle.
The implementation opens and validates that root once, rejects a root that is
not a directory or is itself a symbolic link, junction, or other reparse point,
and retains the handle for the invocation. Every fixed child component
(`v1`, `objects`, `sha256`, the two-character fan-out, `tmp`, and `state`) is
then opened or created relative to its already validated parent handle with
no-follow semantics and verified as the expected directory type. Object,
temporary, cursor, and lock files are likewise opened relative to those handles
and must be regular files. Validation followed by path-string reopening is not
allowed.

On Unix-like hosts this requires handle-relative operations with protections
equivalent to `openat`/`openat2` plus `O_NOFOLLOW`, beneath-only resolution, and
`renameat`-family publication. On Windows it requires directory-relative or
equivalently pinned-handle operations that open reparse points themselves and
reject symlinks, mount points, and junctions before use. A platform that cannot
guarantee confinement and no-follow behavior treats persistent caching as
`Unavailable`; it must not fall back to concatenated path traversal.

All publication, quarantine, cleanup, lock, and metadata operations use the
same pinned handles. Renames require source and destination handles beneath the
same validated root. A name read from the filesystem is never used as a path:
it is first validated against the exact digest, fan-out, temporary-name, lock,
or cursor grammar. Replacing any intermediate path after handles are open
cannot redirect the operation outside the root.

Garbage collection enumerates only the known validated directories. It never
recurses into an unexpected directory, follows a directory entry, resolves a
link target, or deletes a non-regular entry. Unknown directories and links are
ignored and reported only through bounded debug instrumentation. Cache HMACs
authenticate object bytes; they are not authorization to traverse or mutate an
unvalidated filesystem path.

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

If atomic same-filesystem no-replace installation cannot be guaranteed, the
writer reports `parse_write_skipped` and leaves the cache read-only for that
invocation; it never emulates publication with delete-then-rename or overwrite.

A reader opens only the final object through its validated fan-out directory
handle with no-follow semantics. It never reads a temporary file, reopens an
object by path string, or waits for a writer lock.

Two writers for the same key must produce byte-identical canonical payloads. A
race winner is not trusted merely because it arrived first. If the existing
object is invalid, one process may quarantine it and retry one atomic
publication; implementations avoid unbounded replacement loops.

Objects for different keys require no shared publication lock. Garbage
collection has one handle-relative no-follow `gc.lock`; failure to acquire its
non-blocking OS lock skips cleanup. It follows the hard lifecycle budgets below,
never gates the compiler result, and never authorizes partial-object reads.

The final prepublication cancellation/staleness check is the writer's
linearization point. If it observes either state, the writer removes its
temporary file and does not install it. A signal racing after that check may
leave the complete immutable parse object; the writer need not remove it because
the fact is independent of the cancelled aggregate.

## Lifecycle And Eviction

The default cache budget is a soft 2 GiB target. After a successful write, a
process may schedule cleanup after the compiler result and output publication
are finalized, or run it on a bounded background worker. Cleanup is never in a
parse/query critical path. It acquires the handle-relative non-blocking GC lock;
failure to acquire it skips the pass.

One cleanup pass has all of these hard budgets:

```text
elapsed monotonic time                 50 ms
directory entries examined           10,000
candidate records retained             4,096
candidate metadata                    16 MiB
object bytes read for validation      64 MiB
filesystem removals or quarantines     1,024
```

The pass checks the time and remaining work budgets before every new filesystem
operation and schedules no further I/O after a limit is reached. One already
issued host I/O may complete after the deadline, but no unbounded loop or
candidate allocation continues. Hitting a limit records a bounded
`cache_gc_incomplete` debug event, atomically stores a validated continuation
cursor, releases the lock, and returns without affecting the command result.
The cursor contains only a scan generation, fan-out index, last validated
filename, and checked cumulative byte count in a fixed-size versioned record;
it is untrusted performance state, so malformed, overflowing, or stale data
resets scanning safely and is never interpreted as a path or trusted quota
fact.

Cleanup incrementally walks only known fan-out and temporary directories from
the cursor. It retains at most the bounded candidate count in an oldest-first
heap ordered by recorded access time and then digest. Missing, invalid, or equal
timestamps use digest order. A complete series of passes evicts toward 75
percent of the target; one pass is not required to scan the store or reach that
level. An attacker-created excess may therefore keep the store above target,
but cannot force unbounded scan, memory, validation, or deletion work during a
compiler command.

Recency metadata is performance-only. A successful hit may best-effort update
an object's access time at most once per 24 hours. The clock used for cleanup
and its injected deadline never enter compiler keys or results.

Within the same budgets, cleanup first considers no-follow regular temporary
files older than 24 hours and invalid regular object filenames, then
incompatible, corrupt, and oldest valid objects. It never deletes an unknown
directory, link, junction, reparse point, lock, cursor, or authentication key.
An object opened by a reader may survive unlinking through ordinary filesystem
semantics; platforms that prevent deletion simply skip it.

Size accounting is incremental and bounded by the same cursor. An incomplete
estimate schedules later passes rather than claiming the target was met. Budget
calculation, deadline expiry, timestamp failure, full disks, read-only
filesystems, and cleanup errors never fail compilation. A user can delete the
entire cache while no compiler process depends on it; correctness is equivalent
to a cold miss.

Tests inject filesystem and monotonic-clock behavior rather than sleeping or
depending on host access times.

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
cache_gc_complete
cache_gc_incomplete
cache_key_recovered
```

Parse-object events record artifact kind and digest, never AST payload or source
text. Lifecycle events record only a stable outcome/reason and bounded public
counts; they never expose key bytes, host paths, directory entries, or cursor
contents. Cache metrics do not alter human or agent diagnostic order. Elapsed
time is useful for profiling but is not proof of a hit; tests assert event
categories and query execution counts.

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
   - atomically create, validate, recover, and protect the per-user
     authentication key and verify object MACs;
   - implement bounded, panic-free untrusted decoding plus pinned-handle,
     no-follow filesystem confinement;
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
     deterministic eviction tie-breaks, and hard-bounded incremental GC;
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
- golden byte vectors pin every v1 magic value, integer width/endianness,
  component/tag ID, count, length prefix, field order, discriminator, and exact
  end-of-record boundary for key, compatibility-input, compatibility-ID, and
  envelope records;
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

- failed or panicked parse queries, and cancellation or staleness observed at
  the final prepublication check, publish no initial-cache object;
- cancellation or staleness racing immediately after the final check may leave
  exactly one complete authenticated parse object and never a partial object;
- a complete parse fact published before a later resolution/type/build failure
  or aggregate cancellation remains valid and is reusable, while no failed
  build result is cached;
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
- root and every fixed child component reject intermediate/final symlinks,
  junctions, reparse points, type swaps, and rename races; object, temporary,
  cursor, lock, quarantine, and GC operations remain confined to pinned handles;
- GC skips links and unknown directories, cannot delete outside the root, and
  stops independently at every entry, candidate, metadata-byte, object-byte,
  mutation, and injected-time budget boundary while preserving a validated
  continuation cursor;
- concurrent authentication-key creation selects one complete private key;
  crash points before and during temporary write/install expose no partial final
  key, and malformed length/type/owner/permission cases take the bounded locked
  quarantine-and-retry path or disable caching;
- key rotation invalidates old objects safely, and the key never enters cache
  logs or agent output;
- source scans, bundles, and project manifests never include cache objects;
- incremental size accounting and bounded candidate heaps move toward the soft
  target with deterministic digest tie-breaks without gating compiler results;
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
