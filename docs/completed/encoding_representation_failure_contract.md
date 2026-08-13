# Encoding Representation and Failure Contract

Status: implemented for the interpreter-backed compiler on 2026-08-13.
Future native backends must carry the same contract forward.

## Context

Jett currently exposes six hardcoded `string -> string` operations:
`encoding.base64_encode`, `base64_decode`, `hex_encode`, `hex_decode`,
`url_encode`, and `url_decode`. Their checker signatures cannot describe the
runtime failures of malformed Base64, malformed hex, or decoded bytes that are
not UTF-8. The binary codecs also force arbitrary bytes through `string`.

The current URL pair is internally inconsistent: encoding emits `%20` for a
space, while decoding treats `+` as a space and accepts malformed percent
escapes. There is no compiler-shipped `stdlib/encoding.jett`, so public names,
signatures, and implementation dispatch all remain compiler-owned transitional
technical debt.

This record selects a byte-native binary surface, strict fallible decoders, and
separate URL-percent and form-component semantics. It does not implement the
migration, redesign `bytes`, parse complete URLs or query maps, or add Base64URL,
compression, or character-set conversion.

## Public Surface

The declarations are:

```jett
namespace encoding

export function base64_encode(view bytes_value: bytes) returns string
export function base64_decode(encoded: string) returns result[bytes, string]

export function hex_encode(view bytes_value: bytes) returns string
export function hex_decode(encoded: string) returns result[bytes, string]

export function url_encode(value: string) returns string
export function url_decode(encoded: string) returns result[string, string]

export function form_encode(value: string) returns string
export function form_decode(encoded: string) returns result[string, string]
```

All eight operations are pure, deterministic, capability-free, and
non-mutating. The viewed byte input is borrowed and remains usable after the call.
Decoders return the complete decoded value or a `string` error; they never
return partial output and never replace malformed input silently.

The six existing names remain canonical, but the four binary-codec signatures
are deliberately migrated rather than overloaded. Text callers must make UTF-8
conversion explicit:

```jett
bytes raw = bytes.from_string(text)
string encoded = encoding.base64_encode(view raw)
bytes decoded = encoding.base64_decode(encoded) handle error:
    return fail(error)
string restored = bytes.to_string(decoded) handle error:
    return fail(error)
```

The old infallible `string -> string` decoder signatures are not compatibility
guarantees. They must remain unchanged only until the source declarations,
runtime kernels, diagnostics, and call-site migration land together.

## Base64 Contract

`base64_encode` uses the standard padded Base64 form from RFC 4648:

- the alphabet is `A-Z`, `a-z`, `0-9`, `+`, and `/`;
- output uses `=` padding when required;
- output has no whitespace, separators, or line wrapping;
- empty bytes encode to the empty string.

`base64_decode` accepts only that canonical family. It rejects:

- input whose length is not a multiple of four;
- whitespace anywhere;
- Base64URL characters `-` and `_`;
- padding outside the final quartet or more than two padding characters;
- data after padding;
- non-zero unused bits in the final data character.

Empty input is valid. Successful output is arbitrary `bytes`, including NUL and
values above `0x7f`; no UTF-8 check is performed. Base64URL and unpadded Base64
require separately named future APIs rather than permissive modes on this pair.

## Hex Contract

`hex_encode` emits exactly two lowercase ASCII hexadecimal digits per input byte,
with no prefix, whitespace, or separators. Empty bytes encode to the empty
string.

`hex_decode` accepts uppercase or lowercase ASCII digits so external values can
be consumed without normalization. It rejects odd digit counts, whitespace,
separators, `0x`/`0X` prefixes, and every non-hexadecimal character. Successful
output is arbitrary `bytes` and is not required to be UTF-8.

Accepting uppercase input does not create a second canonical output spelling.
`encoding.hex_encode` always emits lowercase. The existing `bytes.from_hex`
prefix behavior is a separate legacy surface and must not silently broaden this
new contract; a future consolidation should choose one documented migration.

## URL Percent-Component Contract

`url_encode` and `url_decode` operate on one textual URL component, not an entire
URL, path, query, fragment, or key/value map.

Encoding first reads the exact UTF-8 bytes of the input. Only RFC 3986
unreserved bytes remain literal:

```text
A-Z a-z 0-9 - . _ ~
```

Every other byte is percent-encoded with uppercase hexadecimal. A space is
`%20`; a literal plus is `%2B`. No Unicode normalization or locale processing
occurs.

Decoding applies these rules:

- every `%` must be followed by exactly two hexadecimal digits;
- uppercase and lowercase escape digits are accepted;
- `+` is an ordinary literal plus and is never converted to a space;
- unescaped UTF-8 text may remain literal;
- the complete decoded byte sequence must be valid UTF-8.

A malformed or truncated escape and invalid decoded UTF-8 are errors. This
replaces the current lenient malformed-escape and form-style `+` behavior.

## Form-Component Contract

`form_encode` and `form_decode` separately model one
`application/x-www-form-urlencoded` component. They do not split or join `&` or
`=`, construct maps, choose duplicate-key behavior, or reorder fields.

Encoding uses exact UTF-8 bytes. ASCII alphanumeric bytes and `*`, `-`, `.`, and
`_` remain literal. A space becomes `+`, a literal plus becomes `%2B`, and every
other byte is percent-encoded with uppercase hexadecimal. In particular, `~`
is `%7E` in this form contract even though it is unreserved by the URL-percent
contract.

Decoding converts `+` to a space, validates every percent escape, accepts either
hexadecimal case in escapes, and requires the final bytes to be valid UTF-8.
Malformed input returns an error rather than partial or replacement text.

## Error Contract

The initial error type is `string`, matching Jett's existing fallible stdlib
convention. Error messages are deterministic, prefixed by the public operation,
and never include the supplied payload. Implementations expose these stable
categories:

```text
encoding.base64_decode: invalid length
encoding.base64_decode: invalid character
encoding.base64_decode: invalid padding
encoding.base64_decode: non-zero trailing bits
encoding.hex_decode: odd-length hex string
encoding.hex_decode: invalid hex characters
encoding.url_decode: malformed percent escape
encoding.url_decode: decoded bytes are not valid UTF-8
encoding.form_decode: malformed percent escape
encoding.form_decode: decoded bytes are not valid UTF-8
```

Backends may use richer internal errors, but host-library wording, offsets,
platform details, and debug formatting must not escape through the public API.
A future structured error type requires a separate compatibility decision; it
must not be added inconsistently to individual decoders.

Validation order is part of deterministic behavior. Base64 checks total length,
then alphabet, then padding placement/count, then unused trailing bits. Hex
checks digit-count parity before digit validity. URL and form decoders scan
escapes left to right and report the first malformed escape before validating
the complete decoded bytes as UTF-8. Tests with more than one defect must pin
these precedence rules across backends.

Ordinary allocation or execution limits still apply. Invalid data is a handled
domain error, while a missing trusted kernel or internal invariant failure is a
compiler/runtime defect and must not be disguised as malformed user input.

## Source and Runtime Boundary

Every public `encoding.*` declaration belongs in trusted compiler-shipped
`.jett` source. The source layer owns public names, parameter order, `view`
ownership, result types, and stable error mapping. The compiler must not retain a
permanent hardcoded table of public encoding names or signatures.

Low-level byte loops may initially remain private trusted runtime kernels where
source code cannot yet express them clearly or efficiently. Those kernels are
implementation details: project code cannot import them, spoof them with a
lookalike declaration, or gain trust by declaring `namespace encoding`.
Dispatch must depend on resolved trusted origin, not source spelling alone.
The implementation slice must register each private hook in the same centralized
trusted-origin mechanism used for compiler-shipped stdlib hooks; hook names are
selected with that implementation and are never exported public API. Resolution
tests must prove that the public signatures originate in `stdlib/encoding.jett`,
project `namespace encoding` declarations cannot replace or shadow them,
untrusted matching spellings cannot invoke a hook, and private hook declarations
are inaccessible to project code.

Compositional validation and wrappers should move to real Jett bodies as soon as
they fit the language and function-complexity policy. The long-term boundary is
ordinary source-owned public declarations over only the smallest necessary byte
kernels, not a permanent public builtin exception.

## Backend Handoff

HIR, MIR, interpreter, bytecode, and native lowering must preserve:

- exact argument and result types and non-consuming `view` behavior;
- every byte value from `0x00` through `0xff` without UTF-8 coercion in binary
  codecs;
- strict Base64 padding and trailing-bit validation;
- lowercase hex and uppercase percent-escape canonical output;
- the distinct `+`/space policies of URL and form components;
- UTF-8 validation only where the public result is `string`;
- deterministic operation-prefixed errors with no host-library leakage.

Backends may call a runtime ABI or use an audited library, but must not inherit a
provider's permissive defaults. Compiler/runtime dependencies must be pinned,
license-compatible, and covered by the same vectors. Backend work follows the
HIR and MIR boundaries tracked by [#20](https://github.com/vycdev/jett/issues/20)
and [#22](https://github.com/vycdev/jett/issues/22), but those phases do not
block the interpreter-facing source extraction.

"No application dependencies" means callers do not select, install, or observe
a codec provider. It does not prohibit an audited compiler/runtime dependency;
any such dependency remains an implementation detail subject to the constraints
above.

## Implemented Slices and Backend Follow-up

1. **Pin and correct interpreter runtime behavior — complete**
   - add known vectors and malformed-input regressions for the current kernels;
   - make percent decoding strict and separate URL `+` from form `+`;
   - make Base64 padding and unused-bit checks canonical.
2. **Migrate binary representations and failures — complete**
   - change Base64 and hex to byte-native encode/decode signatures;
   - return `result` from every decoder and migrate existing fixtures/callers;
   - add form component operations independently of URL component operations.
3. **Extract public declarations — complete**
   - add compiler-shipped `stdlib/encoding.jett` declarations and wrappers;
   - retain only private trusted byte kernels;
   - remove hardcoded public signature knowledge and reject project namespace
     collisions or lookalike hooks.
4. **Carry the contract through future backends**
   - share the same vector and malformed-input corpus across execution engines;
   - verify backend dependencies do not broaden accepted input or error text.

## Required Regression Matrix

- RFC 4648 Base64 vectors for empty input and `f`, `fo`, `foo`, `foob`, `fooba`,
  and `foobar`.
- Arbitrary binary round trips containing NUL, `0x80`, and `0xff`.
- Rejection of Base64 whitespace, URL-safe alphabet, invalid length, misplaced
  padding, and non-zero trailing bits.
- Hex empty/binary vectors, lowercase canonical output, mixed-case acceptance,
  and rejection of prefixes, odd length, whitespace, and invalid digits.
- URL unreserved bytes, `%20` spaces, literal `+`, `%2B`, uppercase output,
  Unicode round trips, malformed escapes, and invalid UTF-8.
- Form `+` spaces, `%2B` literal plus, `%7E`, Unicode round trips, malformed
  escapes, and invalid UTF-8.
- Compile failures for wrong text/byte arguments and decoder results used without
  `handle error:`.
- Ownership regressions that reuse input `bytes` after each `view` encoder call.
- Project declarations cannot claim the stdlib namespace or private hooks.
- Multi-fault decoder inputs follow the specified validation precedence.
- Interpreter and future backend outputs and error categories match exactly.
