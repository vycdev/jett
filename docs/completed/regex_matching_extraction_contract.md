# Regular Expression Matching and Extraction Contract

Status: accepted design; implementation is pending. This contract selects the
portable initial surface for [#140](https://github.com/vycdev/jett/issues/140).
Future execution backends must preserve the same observable behavior.

## Context

Jett reserves the `regex` namespace but has no declarations, checker signatures,
runtime dispatch, engine dependency, or fixtures for it. The standard library
needs regular expressions for bounded text recognition and extraction when the
ordinary `string` helpers are insufficient, without exposing host byte offsets,
backtracking behavior, or engine-specific syntax.

This contract selects a deliberately small, deterministic, linear-time pattern
language. It does not make Rust's `regex` crate, RE2, PCRE, or any other host
engine part of Jett's public compatibility surface.

## Public Surface

The canonical declarations belong in compiler-shipped `stdlib/regex.jett`.
They appear in this order because `CaptureSet` refers to `Match`:

```jett
namespace regex

export struct Match:
    value: string
    start: int64
    end: int64

export struct CaptureSet:
    whole: Match
    groups: list[optional[Match]]
    named: map[string, optional[Match]]

export enum RegexError:
    invalid_pattern(position: int64, message: string)
    pattern_too_large(limit: int64)
    compiled_pattern_too_large(limit: int64)
    input_too_large(limit: int64)
    too_many_matches(limit: int64)

export function is_match(value: string, pattern: string) returns result[bool, RegexError]
export function find(value: string, pattern: string) returns result[optional[Match], RegexError]
export function captures(value: string, pattern: string) returns result[optional[CaptureSet], RegexError]
export function find_all(value: string, pattern: string) returns result[list[Match], RegexError]
```

All four operations are pure, deterministic, capability-free, and
platform-independent. They compile the supplied pattern for that call, then
search `value`. A malformed or over-limit pattern is a handled `RegexError`; no
match is a successful `false`, `none`, or empty list rather than an error.

The initial surface has no public compiled-pattern value. Adding one would need
a separate decision about ownership, cloning, equality, memory accounting, and
serialization. Implementations may cache immutable compiled programs privately,
but cache presence, capacity, and eviction must not change results or errors.

There are no duplicate `_with_options` operations. Flags use the one pattern
syntax defined below.

## Match and Capture Values

`Match.start` is an inclusive extended-grapheme-cluster index in `value` and
`Match.end` is the exclusive index. The invariant is:

```text
string.slice(value, match.start, match.end) == match.value
```

No public field contains a UTF-8 byte offset or Unicode scalar-value offset.
Every returned boundary is valid for `string.slice`.

`captures` returns the first match and its groups:

- `whole` is the complete match;
- `groups[0]` corresponds to numbered capture group 1;
- every capturing group occupies one list entry in opening-parenthesis order;
- a participating group contains `some(Match)`;
- an optional group that did not participate contains `none`;
- `named` contains every declared named group, including a `none` value when it
  did not participate;
- named groups also retain their numbered position in `groups`;
- duplicate group names are invalid.

`find_all` returns complete matches only. Callers needing captures for repeated
matches can process bounded input slices in a later compositional helper; this
first surface does not duplicate a second list-of-capture-sets operation.

Returned strings are owned values independent of `value`. Match and capture
structs receive no implicit structural equality; ordinary Jett rules still
require an explicit `Equatable.equals` implementation if a caller-defined type
needs equality involving them.

## Portable Pattern Language

Patterns are UTF-8 Jett strings. The initial grammar supports:

- exact literal text and the escapes `\\n`, `\\r`, `\\t`, `\\f`, `\\a`, `\\e`,
  `\\\\`, and escaped metacharacters;
- concatenation and alternation with `|`;
- `.` for one extended grapheme cluster, except a line-ending grapheme unless
  dot-all mode is enabled;
- start/end anchors `^` and `$`;
- grouping `(...)` and non-capturing grouping `(?:...)`;
- named groups `(?P<name>...)`, where names match `[A-Za-z][A-Za-z0-9_]*`;
- greedy quantifiers `?`, `*`, `+`, `{m}`, `{m,}`, and `{m,n}`;
- lazy forms made by adding `?` to a quantifier;
- bracket classes, negated bracket classes, and inclusive ranges;
- ASCII shorthands `\\d`, `\\D`, `\\w`, `\\W`, `\\s`, and `\\S`;
- one optional leading flag group `(?ims)` containing each selected flag at
  most once and in exactly that canonical order.

The flags are:

- `i`: Unicode simple case-insensitive matching for literals;
- `m`: `^` and `$` also match immediately after and before a line ending;
- `s`: `.` includes line-ending grapheme clusters.

There are no scoped or negative flag groups in the initial grammar. `x`,
locale, ASCII-mode, ungreedy, and normalization flags are unsupported.

The shorthand classes are fixed and do not depend on locale or a host Unicode
library:

```text
\\d = [0-9]
\\w = [A-Za-z0-9_]
\\s = space, horizontal tab, LF, CR, form feed, or vertical tab
```

Bracket class members and range endpoints each denote one Unicode scalar value.
A class matches a grapheme cluster only when that cluster consists of exactly
one scalar value accepted by the class. Literal atoms may contain complete
multi-scalar grapheme clusters, and `.` always consumes one complete grapheme
cluster. This preserves Jett's opaque, grapheme-indexed string model rather than
letting a regex split a user-visible character.

Pattern parsing segments literal runs by the same Unicode extended-grapheme
algorithm used by `string.chars` and `string.slice`. A pattern containing an
isolated combining mark may therefore match only when that mark is itself a
complete grapheme in the input; it cannot select the combining component of a
larger grapheme.

The following constructs are explicitly unsupported and produce
`invalid_pattern` rather than being interpreted differently by different
engines:

- backreferences;
- lookahead or lookbehind;
- conditionals, subroutine calls, recursion, or balancing groups;
- atomic groups and possessive quantifiers;
- inline comments and free-spacing mode;
- Unicode property escapes such as `\\p{...}` and `\\P{...}`;
- hexadecimal, octal, byte, or code-point numeric escapes;
- word-boundary anchors such as `\\b` and `\\B`;
- engine-specific directives or embedded code.

Numeric escapes are omitted so agents cannot accidentally express byte or
scalar arithmetic through the regex surface. Unicode property classes and word
boundaries require a later contract that pins their Unicode data and grapheme
interaction.

## Unicode, Case, and Line Endings

Jett performs no NFC, NFD, compatibility normalization, locale conversion, or
line-ending rewriting before matching. Canonically equivalent but differently
encoded text is distinct unless the pattern explicitly accepts both forms.

Case-sensitive literals compare exact Unicode scalar sequences inside complete
grapheme clusters. Under `i`, literals use Unicode simple case folding from the
Unicode data version pinned by the Jett toolchain release. Folding never uses
the host locale. It does not apply full multi-character expansions, so a single
literal grapheme does not expand into several graphemes. ASCII bracket classes,
ranges, and shorthands are not widened by `i`; callers use literal alternation
when they need a case-insensitive non-ASCII set before Unicode classes are
standardized.

A line ending is one of LF, CRLF, or bare CR. CRLF is treated as one line-ending
grapheme for `.`, `^`, `$`, and empty-match advancement. In default mode, `^`
and `$` match only the beginning and end of the complete input. In `m` mode,
`^` also matches after a line ending and `$` before a line ending. `$` has no
special implicit match before a final line ending unless `m` is enabled.

Every Jett release must pin the grapheme segmentation and simple-folding Unicode
data used by all backends. Upgrading that data is an explicit compatibility
change recorded in release notes and conformance fixtures, not an accidental
host-library update.

## Selection, Ordering, and Empty Matches

Search is leftmost-first:

1. choose the earliest grapheme boundary at which any match begins;
2. at that boundary, alternation tries branches from left to right;
3. greedy quantifiers take the longest match allowed by the chosen branch;
4. lazy quantifiers take the shortest match allowed by the chosen branch;
5. backtrack only through the finite ordered choices needed to reproduce these
   semantics; execution must still satisfy the linear-time resource rule below.

This ordering is observable in `find`, `captures`, and `find_all` and must not be
inherited from whichever host engine is convenient. Capture values are those
from the selected path. A group repeated by a quantifier records its final
participating iteration.

An empty pattern is valid and matches an empty string at the first grapheme
boundary. `find_all` includes empty matches. After emitting an empty match it
advances by one whole grapheme before searching again; an empty match at end of
input is emitted once and terminates iteration. It never emits two empty matches
at the same boundary. Non-empty adjacent matches are allowed.

For example, an empty pattern over a value containing two grapheme clusters
produces spans `0..0`, `1..1`, and `2..2`. This rule also applies when an
alternation or quantifier selects an empty branch.

## Failure Contract

Pattern positions in `invalid_pattern` are zero-based grapheme indices in the
pattern, never byte offsets. End-of-pattern errors use
`string.char_count(pattern)`. The stable `message` values are categories, not
host parser prose:

```text
unexpected token
unexpected end of pattern
unclosed group
unclosed character class
invalid escape
unsupported construct
invalid quantifier
quantifier range is reversed
capture name is invalid
capture name is duplicated
too many capture groups
flag group must be leading and canonical
```

When several syntax errors are possible, the parser reports the first one found
by a left-to-right parse. The public error must not include dependency names,
debug formatting, byte offsets, stack sizes, or fragments of potentially secret
input. It may identify the pattern position but does not repeat pattern text.

Limit errors are checked in this order for every operation:

1. pattern grapheme count;
2. pattern syntax and capture count;
3. compiled-state count;
4. input grapheme count;
5. `find_all` result count while collecting matches.

`is_match`, `find`, and `captures` cannot produce `too_many_matches` because they
stop after the first selected match. Allocation failure, a missing trusted
kernel, or an internal engine invariant failure is a compiler/runtime defect,
not `invalid_pattern`.

## Resource and Complexity Limits

The initial portable limits are:

```text
pattern length                 4,096 grapheme clusters
capturing groups                  64
bounded repetition endpoint  1,000,000
compiled automaton states        65,536
input length                  1,000,000 grapheme clusters
find_all returned matches       100,000
```

The corresponding error variant contains the applicable limit. More than 64
captures is `invalid_pattern(..., "too many capture groups")`; an oversized
repetition endpoint is `invalid_pattern(..., "invalid quantifier")`.

The state count is measured after expanding bounded repetition into Jett's
canonical Thompson NFA. Each consuming atom, anchor assertion, branch/split,
capture-start tag, capture-end tag, and final accept instruction counts as one
state. Quantifiers compile to the ordinary split and loop instructions; they do
not receive an engine-specific compressed exemption. Implementations may store
or optimize the program differently, but limit checking uses this canonical
count before optimization.

Compilation and matching must use an automaton or equivalent algorithm whose
worst-case work is linear in the input length times the compiled-state count.
An implementation must not expose catastrophic exponential backtracking. A host
engine is usable only if an adapter rejects unsupported syntax, enforces Jett's
grapheme semantics and limits, and normalizes all public errors.

Limits are measured in Jett grapheme clusters or engine-independent automaton
states, not host bytes, stack depth, or allocation sizes. Backends may impose a
lower process-wide memory limit only as a runtime failure affecting the run; they
must not silently return no match or a different `RegexError` for a conforming
input within these limits.

## Replacement and Splitting

Regex replacement and splitting are deferred. The ordinary `string.replace`
and `string.split` remain canonical for literal operations.

A future regex replacement contract must decide capture expansion syntax,
literal-dollar escaping, unmatched groups, empty-match advancement, output-size
limits, and secret handling. A future split contract must decide leading,
trailing, and adjacent empty fields and whether captures enter the result. No
private initial kernel should become a de facto public spelling before those
choices are made.

## Purity, Verify, and Comptime

All operations are semantically pure. They require no capability and perform no
I/O, ambient lookup, randomness, clock read, or host-locale access. They are
therefore eligible in `verify` and in explicit `comptime` expressions under the
same closure and purity rules as every other pure function.

Runtime and comptime evaluation use the same pattern grammar, Unicode data,
limits, match ordering, and errors. Constant evaluation must not call a host
regex service with different defaults. An over-limit comptime operation returns
the same handled `RegexError` when the source handles it; it is not granted
larger limits merely because the compiler has more memory.

Ordinary optimizer folding remains optional and cannot change source validity.

## Source and Runtime Boundary

Public types, declarations, result handling, and compositional policy belong in
trusted compiler-shipped `.jett` source. The compiler must not permanently own
public signatures merely because the first implementation needs a private
engine kernel.

Private trusted kernels may own:

- syntax validation and automaton construction;
- grapheme-aware execution;
- capture span production;
- conversion of private spans into source-level `Match` and `CaptureSet` values.

Private spans may use byte offsets internally, but they must be validated and
converted to grapheme indices before crossing the trusted boundary. Project and
dependency code cannot call private kernels, contribute to the compiler-shipped
`regex` namespace, or gain trust by declaring a matching qualified name.
Dispatch depends on resolved trusted stdlib origin.

A private compiled-pattern cache must be semantically invisible, isolated
between compiler/runtime contexts where necessary, bounded independently of
program results, and safe to discard at any time. Pattern text or input text
must not appear in cache diagnostics or compiler telemetry by default.

## Future Backend Handoff

HIR and MIR must represent public calls and handled `RegexError` values without
embedding a Rust-engine object or host ABI. Interpreter, bytecode, native, and
WASM runners may use different engines, but shared conformance fixtures must pin:

- accepted and rejected syntax;
- grapheme-only boundaries and indices;
- flag, Unicode, case-folding, and line-ending behavior;
- leftmost-first alternation, greediness, captures, and no-match values;
- empty-match progress and final-boundary behavior;
- every limit and failure category;
- pure verify/comptime equivalence;
- rejection of untrusted private-kernel calls.

A backend lacking an engine that can satisfy the contract must report the target
as unsupported at build/run setup. It must not quietly offer a reduced dialect
or scalar-indexed matches.

## Implementation Stages

1. **Source declarations and checker boundary**
   - add the public types and four declarations in `stdlib/regex.jett`;
   - reserve only the private trusted hooks actually used by those bodies;
   - reject project namespace fragments and spoofed kernels under existing
     stdlib-origin rules.
2. **Portable parser and interpreter engine**
   - implement the selected grammar, ordered matching, captures, and limits;
   - convert internal spans to grapheme indices;
   - normalize all failures to `RegexError`.
3. **Comptime and conformance integration**
   - make the same pure kernels available to required comptime evaluation;
   - share fixtures between runtime and comptime paths.
4. **Later backend handoff**
   - carry calls through HIR/MIR and run the same conformance corpus;
   - document any target that cannot yet provide a conforming engine.

No stage introduces replacement, splitting, public compiled patterns, Unicode
property classes, lookaround, or backreferences.

## Required Regression Matrix

The implementation must cover at least:

- literal, concatenation, alternation, anchors, classes, each quantifier, greedy
  and lazy selection, and the three leading flags;
- rejection of each unsupported construct and each malformed syntax category;
- false/`none`/empty-list no-match results without an error;
- numbered, named, nested, repeated, and unmatched optional captures;
- exact grapheme-index spans for ASCII, non-ASCII, emoji, combining sequences,
  and CRLF;
- proof that dot and classes cannot split a multi-scalar grapheme;
- exact matching without normalization and pinned simple case folding;
- leftmost-first alternatives and final-iteration repeated captures;
- empty input, empty pattern, empty alternatives, adjacent matches, and
  end-of-input empty-match termination;
- every pattern, state, input, capture, repetition, and result limit at and just
  beyond its boundary;
- adversarial nested repetition completing within the linear-time policy;
- identical runtime, verify, and explicit comptime values/errors;
- trusted source ownership and rejection of project calls to private hooks;
- equivalent fixtures across every implemented execution backend.
