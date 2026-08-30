# Crypto Hashing and Security Contract

Status: SHA-256/legacy-MD5 surface implemented on 2026-08-13; SHA-512 implemented on 2026-08-30.
HMAC names remain reserved and undiscoverable until separately implemented.

## Context

Before extraction, Jett exposed `crypto.sha256(string) -> string` and
`crypto.md5(string) -> string` through hardcoded checker and interpreter arms.
Both operations hash the UTF-8 bytes of the input and return lowercase
hexadecimal text. That transitional state did not match the source-owned stdlib
target recorded in the architecture; `stdlib/crypto.jett` now owns both public
declarations and their byte/hex composition.

The language design also names SHA-512 and HMAC without selecting signatures or
security policy. That leaves agents without a stable answer for input encoding,
output casing, password handling, secret propagation, or future backend
behavior. This record fixes the initial text-digest surface, classifies the
planned algorithms, and reserves one binary HMAC boundary without implementing
new algorithms.

## Implemented Initial Public Surface

The stable first slice keeps the two implemented text-digest spellings. In API
signature notation (not complete source bodies):

```text
crypto.sha256(input: string) returns string
crypto.md5(input: string) returns string
```

For both functions:

- the input is the exact UTF-8 byte sequence of the Jett string;
- no Unicode normalization, line-ending conversion, trailing NUL, or other
  preprocessing is performed;
- the output is lowercase ASCII hexadecimal with no prefix or separators;
- SHA-256 output is exactly 64 characters and MD5 output is exactly 32
  characters;
- the operation is total after type checking and returns no recoverable domain
  error.

These signatures remain text-specific. Jett does not overload them for `bytes`
and does not implicitly convert between `string` and `bytes`. A future raw-byte
digest API must use distinct queryable names and receive a separate design
record; it must not silently change the meaning of `crypto.sha256` or
`crypto.md5`.

## Algorithm Classification

### SHA-256

`crypto.sha256` computes the SHA-256 digest defined by FIPS 180-4. It is the
canonical initial digest for content identifiers, integrity checks against a
trusted expected digest, and other non-password hashing in Jett.

SHA-256 is not encryption, a signature, a message-authentication code, a source
of randomness, or a password-hashing function. An unkeyed digest does not prove
who produced a message and does not protect a mutable digest supplied by an
attacker.

### MD5

`crypto.md5` remains only for compatibility with legacy protocols, file formats,
and non-adversarial checksums that explicitly require MD5. MD5's collision
resistance is broken, and it must not be used for signatures, certificates,
authentication, security-sensitive integrity decisions, content trust, or
password storage.

The existing name remains stable so old formats can be implemented without an
alias. New examples and generated code should prefer SHA-256. Keeping the
function does not make a security claim about MD5.

### SHA-512

SHA-512 is implemented with the text form:

```text
crypto.sha512(input: string) returns string
```

It follows the same exact-UTF-8 and lowercase-hex rules as SHA-256 and returns
exactly 128 characters. The name must not appear in source query results until
an implementation and known-vector tests ship together.

### HMAC

HMAC is also planned rather than implemented. Its first reserved operation is
HMAC-SHA-256 with a binary, secret key and binary message:

```text
crypto.hmac_sha256(
    view key: secret[bytes],
    view message: bytes,
) returns secret[bytes]
```

The argument order is always key first, message second. HMAC-SHA-256 applies the
RFC 2104 key rules with a 64-byte block: keys longer than 64 bytes are first
replaced by their 32-byte SHA-256 digest, and shorter keys (including the empty
key) are zero-padded for the inner and outer computations. The message is its
exact byte sequence and may be empty. These cases are total and do not produce
source-visible errors beyond ordinary runtime resource limits.

The result is the raw 32-byte RFC 2104/RFC 4231 authentication tag, not
hexadecimal text. The return value stays secret under Jett's taint policy; code
normally compares it with a compatible `secret[bytes]` by using
`secret.compare`. Publishing or encoding a tag requires an explicit, auditable
`declassify` step.

A future HMAC-SHA-512 addition uses the same key-first order and raw secret-byte
result under the name `crypto.hmac_sha512`. No generic
`hmac(algorithm, key, message)` dispatcher is introduced: distinct names keep
the algorithm visible to agents and reviewers. Neither HMAC name is discoverable
as a supported declaration until its runtime kernel and RFC vectors land.

## Failure, Purity, and Determinism

Hashing and HMAC are pure deterministic transformations. They require no
capability and are permitted in pure functions and verify blocks. Equal byte
inputs under the same named algorithm always produce equal outputs across
interpreter, bytecode, and native backends.

The selected operations have no data-dependent `result` failure. A missing or
faulting trusted kernel is a compiler/runtime contract failure, not a domain
error that Jett callers can handle. Implementations must not expose
host-library error strings or platform-dependent failure behavior through these
signatures.

Purity does not promise unbounded resource use. Ordinary runtime allocation and
execution limits still apply. Hashing a value must not read entropy, host state,
time, environment variables, or locale data.

## Secret Taint and Comparison

Hashing is not declassification. Jett's ordinary pure-call lifting applies:

```jett
secret[string] token = load_token()
secret[string] digest = crypto.sha256(token)
```

The digest remains `secret[string]` because it is derived from secret input.
Public input produces public output. The same rule applies to MD5 and a future
SHA-512 implementation. A digest may reveal information about a low-entropy
secret through offline guessing, so an agent must not use hashing as a way to
make a secret safe to log or serialize.

HMAC accepts a secret key explicitly and returns a secret tag. The HMAC kernel
must not require source code to declassify the key. `secret.compare` remains the
only selected constant-time comparison surface for compatible secret strings
and bytes; its fixed-length behavior and backend obligations are defined by
[#33](https://github.com/vycdev/jett/issues/33). This contract does not widen or
reimplement that comparison boundary.

The initial digest kernels make no timing-resistance claim for secret input.
Their output and control flow must still preserve taint, but callers must not
infer that plain SHA or MD5 is a constant-time secret-processing primitive.
HMAC implementations require review for key-dependent timing and memory
handling before their declarations become public.

## Password and Security Non-Claims

None of `sha256`, `sha512`, `md5`, or HMAC is a password storage API. They do not
provide salting, configurable work factors, memory hardness, credential format
versioning, or password verification policy. Documentation and examples must
not spell password storage as `crypto.sha256(password)`.

Until Jett selects a dedicated password-KDF API, applications should use an
externally reviewed password service or binding that provides a modern,
parameterized password hashing scheme. Adding Argon2, scrypt, bcrypt, PBKDF2, or
a password-record type requires a separate design and dependency review.

This module also makes no claim to provide encryption, signatures, key
exchange, key generation, certificate validation, entropy, or a general
cryptography framework. Randomness and entropy remain under
[#67](https://github.com/vycdev/jett/issues/67).

## Compatibility Policy

The exact UTF-8 input and lowercase fixed-width hexadecimal output of
`crypto.sha256` and `crypto.md5` are compatibility guarantees. Implementations
may replace their internal algorithm code only when the same standard vectors
and cross-backend fixtures pass byte-for-byte.

The public names have one canonical spelling. Jett does not add aliases such as
`sha_256`, uppercase output variants, implicit Base64 output, or a configurable
algorithm string. Representation conversion belongs in `encoding` or `bytes`
and remains explicit at the call site.

Adding SHA-512 or HMAC is additive. Removing MD5 would require a future language
compatibility decision and migration path; its insecure classification alone
does not authorize silently changing or deleting it.

## Source and Runtime Boundary

Every public `crypto.*` declaration belongs in trusted compiler-shipped `.jett`
source. Public names and signatures must not remain in a permanent checker table.
The source wrappers own the documented public spelling, parameter order, secret
types, and output representation.

Digest compression and HMAC processing remain private trusted runtime kernels.
They are appropriate runtime work because handwritten source implementations
would add complexity, make optimization-sensitive security review harder, and
needlessly expose low-level block processing. A source wrapper may convert the
kernel's fixed-size bytes to lowercase hexadecimal for the text digest API, but
project code cannot import, declare, or spoof a trusted kernel.

The public API adds no application dependency. The current in-tree Rust kernels
may be retained for the first extraction. Any future compiler/runtime dependency
must be pinned, audited, license-compatible, and produce the same contract; its
types, errors, and provider-specific behavior must not leak into Jett source.
This is an implementation review decision, not a change to the public API.

The former hardcoded public checker signatures and interpreter dispatch have
been replaced by source-owned declarations plus trusted-origin private hooks,
following the stdlib namespace and origin rules tracked by
[#3](https://github.com/vycdev/jett/issues/3).

## Future Backend Handoff

HIR and MIR must retain the selected algorithm identity, exact byte input, fixed
output representation, and secret taint. A native or bytecode backend may call a
runtime ABI or inline an implementation, but it must not:

- normalize or transcode the UTF-8 input;
- emit uppercase, variable-width, prefixed, or platform-dependent text;
- turn a secret-derived result into a public value;
- lower HMAC to an unkeyed digest construction;
- substitute ordinary equality for the separate `secret.compare` boundary.

Backend work remains downstream of [#20](https://github.com/vycdev/jett/issues/20)
and [#22](https://github.com/vycdev/jett/issues/22). Those phases are not
prerequisites for extracting the interpreter-facing source wrappers.

## Implementation Slices

1. **Pin the current text digests — complete**
   - extend SHA-256 and MD5 vectors beyond empty input and `abc`;
   - add UTF-8, embedded-NUL, fixed output length, lowercase-only, and type-query
     coverage;
   - add compile checks showing secret input produces secret output.
2. **Extract the public declarations — complete**
   - add trusted compiler-shipped `stdlib/crypto.jett` wrappers;
   - keep SHA-256 and MD5 processing behind private runtime hooks;
   - remove hardcoded public signature knowledge from the checker and verify
     project code cannot claim or reopen `namespace crypto`.
3. **Add SHA-512 independently — complete**
   - implement the reserved text signature and fixed 128-character output;
   - land FIPS known vectors and cross-backend parity with the declaration.
4. **Add HMAC independently — future additive work**
   - implement key-first HMAC-SHA-256 over secret key bytes and message bytes;
   - add RFC 4231 vectors, long-key coverage, empty key/message coverage, secret
     taint checks, and `secret.compare` integration;
   - expose the declaration only after the kernel and tests are complete.
5. **Preserve the contract in later backends**
   - share vectors across interpreter, bytecode, and native execution;
   - audit optimized HMAC key handling and secret comparison boundaries.

## Required Regression Matrix

- SHA-256 and MD5 match published empty, short, multi-block, and Unicode text
  vectors.
- Embedded NUL and non-ASCII strings hash their exact UTF-8 bytes.
- SHA-256 emits 64 lowercase hex characters; MD5 emits 32; SHA-512 emits 128.
- Wrong argument and result types fail during checking rather than at runtime.
- Hashing `secret[string]` returns `secret[string]` and cannot be printed,
  interpolated, or serialized without explicit policy.
- Hashing is deterministic and requires no capability in pure/verify contexts.
- MD5 documentation and generated examples never present it as secure.
- Future HMAC tests pin key-first ordering, raw output bytes, RFC vectors,
  long-key processing, taint preservation, and comparison through
  `secret.compare`.
- Interpreter, bytecode, and native backends produce byte-identical results.
