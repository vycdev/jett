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
    work_budget_exceeded(limit: int64)
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

Patterns are UTF-8 Jett strings. Parsing operates on Unicode scalar tokens and
uses this complete EBNF; quoted punctuation denotes that exact ASCII scalar:

```text
pattern       ::= flags? alternation EOF
flags         ::= "(?" flag_sequence ")"
flag_sequence ::= "i" | "m" | "s" | "im" | "is" | "ms" | "ims"
alternation   ::= concatenation ("|" concatenation)*
concatenation ::= repetition*
repetition    ::= atom quantifier?
quantifier    ::= ("?" | "*" | "+" | bounded) "?"?
bounded       ::= "{" decimal ("}" | "," decimal? "}")
decimal       ::= "0" | nonzero_digit digit*
atom          ::= literal_egc | "." | "^" | "$" | shorthand | class
                | "(" alternation ")"
                | "(?:" alternation ")"
                | "(?P<" name ">" alternation ")"
name          ::= ascii_letter (ascii_letter | digit | "_")*
class         ::= "[" "^"? class_item+ "]"
class_item    ::= class_scalar ("-" class_scalar)? | shorthand
shorthand     ::= "\\d" | "\\D" | "\\w" | "\\W" | "\\s" | "\\S"
```

`digit`, `nonzero_digit`, and `ascii_letter` are respectively `[0-9]`,
`[1-9]`, and `[A-Za-z]`. Spaces in the grammar are presentation only; the
pattern language ignores no whitespace. Empty concatenations are valid, so an
empty pattern, group, or alternation branch is valid. `flags` is recognized
only at scalar offset zero, appears at most once, is non-empty, contains each
flag at most once, and uses the displayed `i`, then `m`, then `s` order.
At any atom position, `(?:` and `(?P<` select the two group productions. Any
other `(?` followed by a non-empty run of lowercase ASCII letters and then `)`
or EOF is "flags-looking": it is checked as the optional flags form at offset
zero and is the non-leading-flags error elsewhere. Thus `(?i` is a malformed
leading flag group, while bare `(?`, `a(?`, and `(?P` are unsupported group
prefixes. Every other unrecognized `(?` prefix is also an unsupported
construct. This lookahead is classification only and does not skip an earlier
lexical error.

Outside a bracket class, the raw ASCII metacharacters are
`\\ . ^ $ | ? * + ( ) [ ] { }`. Any other scalar is literal. A backslash may
introduce exactly a shorthand, one of the decoded control scalars `\\n`, `\\r`,
`\\t`, `\\f`, `\\a` (U+0007), or `\\e` (U+001B), or one of these escaped
literal metacharacters:

```text
\\\\ \\. \\^ \\$ \\| \\? \\* \\+ \\( \\) \\[ \\] \\{ \\}
```

A maximal sequence of raw or decoded literal scalars is segmented into
extended grapheme clusters by the pinned algorithm below; each resulting
cluster is one `literal_egc` atom. Structural tokens terminate a literal
sequence. Consequently, `ab+` means `a` followed by one-or-more `b`, while a
base scalar followed by a combining scalar in the same literal sequence is one
quantifiable atom.

Inside a class, raw `]`, `-`, and `\\` are structural; raw `^` is structural
only immediately after `[`. The literal spellings of those four scalars are
`\\]`, `\\-`, `\\\\`, and `\\^`. The six shorthands and decoded control
escapes are also allowed. Every other scalar is a `class_scalar`. A range must
have two literal scalar endpoints: a shorthand cannot be an endpoint, and a
decoded endpoint must still be exactly one scalar. Empty classes, negated empty
classes, descending scalar ranges, and an unescaped `-` without both endpoints
are invalid. A class matches an input grapheme only when the grapheme consists
of exactly one scalar accepted by a member or range; negation reverses that
single-scalar predicate and therefore still cannot match a multi-scalar
grapheme.

Quantifier decimals use ASCII digits with no sign, separator, whitespace, or
leading zero except the single spelling `0`. They are parsed with checked
unsigned arithmetic. `{m,n}` requires `m <= n`; `{m,}` is unbounded above.
`{,n}`, a second quantifier on the same atom, and a quantifier without an atom
are invalid. Adding one `?` makes any quantifier lazy; no further suffix is
accepted.

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

Each shorthand is a class predicate and therefore consumes only a one-scalar
EGC. In particular, `\\s` does not split or consume the two-scalar CRLF EGC;
the literal sequence `\\r\\n` matches that complete EGC.

`.` consumes one complete grapheme, except a line-ending grapheme unless `s` is
enabled. `^` and `$` are zero-width assertions. Numbered captures are assigned
in opening-parenthesis order, including named groups; non-capturing groups do
not consume a number.

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
- engine-specific directives or embedded code;
- any group introduced by `(?` other than the exact leading flag group,
  `(?:...)`, or `(?P<name>...)`.

Numeric escapes are omitted so agents cannot accidentally express byte or
scalar arithmetic through the regex surface. Unicode property classes and word
boundaries require a later contract that pins their Unicode data and grapheme
interaction.

## Unicode, Case, and Line Endings

Jett performs no NFC, NFD, compatibility normalization, locale conversion, or
line-ending rewriting before matching. Canonically equivalent but differently
encoded text is distinct unless the pattern explicitly accepts both forms.

Regex v1 pins Unicode 17.0.0, Unicode Standard Annex #29 revision 47, default
extended-grapheme-cluster rules, and no tailoring. The normative segmentation
inputs are the Unicode 17.0.0 files
`ucd/auxiliary/GraphemeBreakProperty.txt`,
`ucd/auxiliary/GraphemeBreakTest.txt`, `ucd/DerivedCoreProperties.txt` for
`Indic_Conjunct_Break`, and `ucd/emoji/emoji-data.txt` for
`Extended_Pictographic`. This is the same segmentation contract used by
`string.chars`, `string.char_count`, and `string.slice`; a backend must not use
a different regex-local segmenter.

The compiler distribution records those four paths, `unicode_version =
"17.0.0"`, `uax29_revision = 47`, `segmentation =
"extended_grapheme_cluster_default_no_tailoring"`, and a SHA-256 digest for
each vendored file in `unicode/regex-unicode-v1.toml`. A build fails if a file,
version, or digest differs. Passing every non-comment vector in
`GraphemeBreakTest.txt` is a release gate. The manifest is normative; a host
library version is not.

The manifest has schema ID `jett.regex-unicode.v1`. Its `files` array is in the
path order shown above and later appends `ucd/CaseFolding.txt`; every entry has
exactly `path` and `sha256`, with the digest encoded as 64 lowercase hexadecimal
digits. Its top-level fields are exactly `schema`, `unicode_version`,
`uax29_revision`, `segmentation`, `case_folding`,
`case_folding_statuses`, and `files`. It records `case_folding =
"default_simple"` and `case_folding_statuses = ["C", "S"]`. Unknown schema IDs,
missing/extra paths,
duplicate paths, reordered paths, or unknown fields fail the trusted build so
two backends cannot silently interpret different manifests.

Case-sensitive literals compare exact scalar sequences inside complete
graphemes. Regex v1 case folding uses Unicode 17.0.0
`ucd/CaseFolding.txt`, whose path and SHA-256 also appear in
`regex-unicode-v1.toml`. For `i`, segment pattern and input first, then map each
scalar of a literal EGC independently through the default simple fold: use only
status `C` and `S` entries, ignore `F` and Turkic `T` entries, and map an absent
scalar to itself. Concatenate the mapped scalars and compare those sequences
exactly without normalization or re-segmentation. Simple folding is
scalar-to-scalar, so it neither expands a literal nor crosses an existing EGC
boundary. The same rule applies to literal scalars produced by escapes.

ASCII bracket classes, ranges, and shorthands are not widened by `i`; callers
use literal alternation when they need a case-insensitive non-ASCII set before
Unicode classes are standardized. Folding never uses the host locale.

A line ending is one of LF, CRLF, or bare CR. CRLF is treated as one line-ending
grapheme for `.`, `^`, `$`, and empty-match advancement. In default mode, `^`
and `$` match only the beginning and end of the complete input. In `m` mode,
`^` also matches after a line ending and `$` before a line ending. `$` has no
special implicit match before a final line ending unless `m` is enabled.

Changing any manifest value is an explicit regex and string compatibility
change recorded in release notes and conformance fixtures. A backend may use a
library only when it verifies Unicode 17.0.0 behavior against the manifest and
the conformance files; an accidental host-library update cannot change results.

## Selection, Ordering, and Empty Matches

Search is leftmost-first:

1. choose the earliest grapheme boundary at which any match begins;
2. at that boundary, alternation tries branches from left to right;
3. greedy quantifiers take the longest match allowed by the chosen branch;
4. lazy quantifiers take the shortest match allowed by the chosen branch;
5. resolve those choices with the ordered tagged-NFA priority below, without a
   backtracking search tree.

This ordering is observable in `find`, `captures`, and `find_all` and must not be
inherited from whichever host engine is convenient. Capture values are those
from the selected path. A group repeated by a quantifier records its final
participating iteration.

An empty pattern is valid and matches an empty string at the first grapheme
boundary. `find_all` is strictly non-overlapping. Its cursor starts at boundary
zero; every search starts at or after that cursor, while `^` and `$` continue to
refer to the original complete input. After emitting a non-empty span `s..e`,
the next cursor is `e`. After emitting an empty span `e..e`, the next cursor is
`e + 1` when `e` is before the final grapheme boundary. An empty match at the
final boundary is emitted once and terminates iteration. A search with no match
terminates iteration. The operation never rescans a boundary before the cursor,
never returns overlapping spans, never emits two empty matches at one boundary,
and permits adjacent non-empty matches.

For example, an empty pattern over a value containing two grapheme clusters
produces spans `0..0`, `1..1`, and `2..2`. This rule also applies when an
alternation or quantifier selects an empty branch.

## Failure Contract

Pattern positions in `invalid_pattern` are zero-based grapheme indices in the
pattern, never byte or scalar offsets. For a scalar-level error, the position is
the index of the EGC containing that scalar. End-of-pattern errors use
`string.char_count(pattern)`. The stable `message` values are categories, not
host parser prose:

```text
unexpected token
unclosed group
unclosed character class
empty character class
invalid escape
unsupported construct
invalid quantifier
quantifier range is reversed
character class range is reversed
capture name is invalid
capture name is duplicated
too many capture groups
flag group must be leading and canonical
```

The parser is a scalar-tokenized, left-to-right recursive-descent parser for the
EBNF above. It emits exactly one error according to this table; no backend may
recover and choose a later error:

| Condition | Position | Message |
| --- | --- | --- |
| at an alternation/concatenation position, a raw `]` or `}`, or a raw `)` with no open group body to close | that token | `unexpected token` |
| EOF while a group body is open | EOF | `unclosed group` |
| EOF while a class is open | EOF | `unclosed character class` |
| `[]` or `[^]` | closing `]` | `empty character class` |
| `\\` at EOF, or followed by any scalar that is neither permitted nor an ASCII letter/digit | the `\\` | `invalid escape` |
| `\\` followed by an unrecognized ASCII letter/digit, or an unrecognized `(?` group form that is not flags-looking | the `\\` or `(` | `unsupported construct` |
| a quantifier without an atom, a second quantifier, a malformed decimal/bound, or an endpoint above 1,000,000 | the first scalar of that quantifier | `invalid quantifier` |
| `{m,n}` with `m > n` | the `{` | `quantifier range is reversed` |
| a class `x-y` with scalar `x > y` | the `-` | `character class range is reversed` |
| an unescaped class `-` lacking either endpoint, or a shorthand used as a range endpoint | the `-` | `unexpected token` |
| an empty name, a scalar outside `[A-Za-z][A-Za-z0-9_]*` before `>`, or EOF before `>` | that scalar, `>` for an empty name, or EOF | `capture name is invalid` |
| a second declaration of a name | the first scalar of that name | `capture name is duplicated` |
| capture group 65 | its opening `(` | `too many capture groups` |
| a flags-looking `(?...)` after offset zero, or a leading flags form not exactly one of the seven EBNF spellings | its opening `(` | `flag group must be leading and canonical` |

While parsing an escape, quantifier, name header, flags form, or class range,
that subproduction owns its punctuation and EOF; its specific row is reported
before a surrounding open group's/class's EOF error or the generic structural
token row. At any other token rejected by the EBNF, use `unexpected token` at
that token. The rows are disjoint after this contextual ownership rule. The
public error must not include dependency names, debug formatting, byte offsets,
stack sizes, or fragments of potentially secret input. It may identify the
pattern position but does not repeat pattern text.

Limit errors are checked in this order for every operation:

1. pattern grapheme count;
2. pattern syntax and capture count;
3. canonical compiled-state preflight;
4. input grapheme count;
5. canonical work-budget preflight;
6. `find_all` result count while collecting matches.

`is_match`, `find`, and `captures` cannot produce `too_many_matches` because they
stop after the first selected match. Allocation failure, a missing trusted
kernel, or an internal engine invariant failure is a compiler/runtime defect,
not `invalid_pattern`.

`find_all` checks the result limit immediately after selecting each match and
before appending it. Selection of match 100,001 returns
`too_many_matches(100000)` and discards the in-progress result; the operation
never returns a truncated or partial list.

## Resource and Complexity Limits

The initial portable limits are:

```text
pattern length                 4,096 grapheme clusters
capturing groups                  64
bounded repetition endpoint  1,000,000
compiled automaton states        65,536
input length                  1,000,000 grapheme clusters
matching work units          100,000,000
find_all returned matches       100,000
```

The corresponding error variant contains the applicable limit. More than 64
captures is `invalid_pattern(..., "too many capture groups")`; an oversized
repetition endpoint is `invalid_pattern(..., "invalid quantifier")`.

### Canonical automaton and state preflight

Limit checking uses a canonical ordered tagged Thompson NFA with these
instructions: `Consume(predicate)`, `AssertStart`, `AssertEnd`,
`Split(preferred, alternate)`, `SaveStart(group)`, `SaveEnd(group)`, and
`Accept`. Every instruction counts as one state. Concatenation is represented
by patched fallthrough/next targets and adds no jump state. `A|B|C` desugars to
`Split(A, Split(B, C))`, with the earlier source branch preferred. `X?` is
`Split(X, epsilon)`, `X*` is a preferred `X` edge whose fragment loops to the
split plus an epsilon exit, and `X+` is `X` followed by that loop/exit split.
A lazy suffix swaps the two edges of every split introduced by that quantifier;
it does not swap alternation splits. A capture wraps its body in `SaveStart`
and `SaveEnd`. Search-start injection is runner behavior, not a compiled state.

Let `C(X)` be the canonical state count of AST fragment `X`, calculated before
allocation or optimization:

```text
C(empty)                    = 0
C(literal/dot/class/shorthand) = 1
C(^ or $)                   = 1
C(concat(X1..Xk))           = sum(C(Xi))
C(alt(X1..Xk))              = sum(C(Xi)) + (k - 1)
C(capturing_group(X))       = C(X) + 2
C(non_capturing_group(X))   = C(X)
C(X?) = C(X*) = C(X+)       = C(X) + 1
C(X{m})                     = m * C(X)
C(X{m,n})                   = n * C(X) + (n - m)
C(X{m,})                    = (m + 1) * C(X) + 1
C(program(X))               = C(X) + 1  // final Accept
```

`alt` with one branch adds zero; the parser's valid empty branch therefore
follows the `C(empty)` rule. Bounded expansion clones the complete fragment,
including its capture tags and identifiers. `{m,n}` is `m` required clones
followed by `n-m` optional clones; `{m,}` is `m` required clones followed by one
starred clone. A lazy suffix reverses each optional/loop split preference but
does not change the recurrence. These rules also define nullable and zero-count
forms; epsilon closure must de-duplicate cycles.

The compiler evaluates the recurrence with checked unsigned 64-bit add and
multiply operations before cloning any fragment. As soon as an operation
overflows or the partial/final program count exceeds 65,536, it returns
`compiled_pattern_too_large(65536)`. Thus `{1000000}` cannot cause a million
temporary nodes before rejection. A backend may encode or optimize the accepted
program differently, but it must run this exact preflight on the unoptimized
AST and preserve the ordered priorities and capture tags.

### Deterministic execution budget

Execution uses ordered epsilon closure. For a fixed input boundary, each
instruction is activated at most once per search phase; when paths meet at an
instruction, the runner keeps the higher-priority path and its capture tags.
The closure marks an instruction visited before expanding it. A loop path that
returns to an already visited instruction in the same boundary and phase
without consuming an EGC is discarded, even when that loop edge is preferred;
the previously enqueued exit path remains eligible. Therefore an optional
zero-width loop iteration never commits capture tags: `X*` and the starred tail
of `X{m,}` retain the captures from before that attempted iteration, while the
mandatory first iteration of `X+` does commit. Finite optional clones in `?` or
`{m,n}` are not loop-backs and still participate according to their greedy or
lazy split priority. Discarding a loop path discards every capture-tag write
made by that iteration, including writes in nested groups. This first-visit rule
is the tie-break among equal-length nullable paths and eliminates an infinite
family of ever-more-preferred empty iterations.

Path priority first prefers the earlier injected start boundary, then compares
the traversed split-edge ranks lexicographically, where preferred is `0` and
alternate is `1`. An `Accept` candidate is final only after every live path
with higher priority can no longer accept; this preserves greedy continuation
without changing leftmost branch priority. There are at most two phases at a
boundary: the forward search/start-injection phase and, only after a match
ending there, one `find_all` continuation phase. The continuation does not
search any earlier boundary. This ordered-NFA rule, not a host backtracking
stack, defines selection.

After input segmentation and before execution, every operation computes:

```text
N = input grapheme count
S = canonical compiled program state count, including Accept
work_units = checked_mul(checked_mul(N + 1, S), 2)
```

`N + 1` includes the final boundary; the factor two covers search-start
injection and a `find_all` continuation at the same boundary. Addition and
multiplication use checked unsigned 64-bit arithmetic. Overflow or a value
greater than 100,000,000 returns
`work_budget_exceeded(100000000)` before any match is attempted. A conforming
runner must schedule and de-duplicate states so that all search phases of the
operation together fit that bound; it cannot restart a suffix search or charge
an unbounded set of capture histories. At the maximum independent input and
state limits, this preflight therefore rejects the roughly 65-billion-state
cross-product instead of attempting it.

Compilation and matching must not expose catastrophic exponential
backtracking. A host engine is usable only if an adapter implements the exact
parser and preflights above, enforces Jett's grapheme and ordered-selection
semantics, and normalizes all public errors. Relying on a host timeout or a
backend-specific step counter is not conforming.

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
- exact parser errors/positions, canonical NFA counts, and work preflight;
- the Unicode manifest, flags, per-scalar folding, and line-ending behavior;
- leftmost-first alternation, greediness, captures, and no-match values;
- empty-match progress and final-boundary behavior;
- every limit, precedence rule, and failure category;
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

## Canonical Conformance Vectors

These vectors are normative. Pattern positions and spans are EGC indices.

| Pattern | Canonical program states, including `Accept` | Result |
| --- | ---: | --- |
| empty | 1 | accepted |
| `a` | 2 | accepted |
| <code>a&#124;b</code> | 4 | accepted |
| `(a)` | 4 | accepted |
| `a?`, `a*`, or `a+` | 3 | accepted |
| `a{2}` | 3 | accepted |
| `a{2,4}` | 7 | accepted |
| `a{2,}` | 5 | accepted |
| `(ab){2,3}` | 14 | accepted |
| `a{65535}` | 65,536 | accepted |
| `a{65536}` | 65,537 | `compiled_pattern_too_large(65536)` |

| Invalid pattern | Position | Message |
| --- | ---: | --- |
| `)` | 0 | `unexpected token` |
| `(a` | 2 | `unclosed group` |
| `[a` | 2 | `unclosed character class` |
| `[]` | 1 | `empty character class` |
| `\@` | 0 | `invalid escape` |
| a trailing `\` | its position | `invalid escape` |
| `\q` or `\1` | 0 | `unsupported construct` |
| `(?` | 0 | `unsupported construct` |
| `(?=a)` | 0 | `unsupported construct` |
| `(?mi)a` | 0 | `flag group must be leading and canonical` |
| `(?i` | 0 | `flag group must be leading and canonical` |
| `a(?i)b` | 1 | `flag group must be leading and canonical` |
| `a{02}` | 1 | `invalid quantifier` |
| `a{3,2}` | 1 | `quantifier range is reversed` |
| `a**` | 2 | `invalid quantifier` |
| `[z-a]` | 2 | `character class range is reversed` |
| `[\d-a]` | 3 | `unexpected token` |
| `(?P<1>x)` | 4 | `capture name is invalid` |
| `(?P<` | 4 | `capture name is invalid` |
| `(?P<x` | 5 | `capture name is invalid` |
| `(?P<x)` or `(?P<x]` | 5 | `capture name is invalid` |
| `(?P<x>a)(?P<x>b)` | 12 | `capture name is duplicated` |

For `find_all`, these complete span sequences are fixed:

| Value | Pattern | Spans |
| --- | --- | --- |
| `aaa` | `aa` | `0..2` (the overlapping `1..3` is not returned) |
| `aaaa` | `aa` | `0..2`, `2..4` |
| two EGCs | empty | `0..0`, `1..1`, `2..2` |
| `ab` | `a*` | `0..1`, `1..1`, `2..2` |
| `aa` | `^a` | `0..1` only; the continuation keeps anchors absolute |

On empty input, `captures("", "()*")` returns an empty whole match with group
1 `none`, because the optional zero-width loop is discarded. `()+` returns
group 1 as `some(0..0)` from its mandatory first iteration, and `()?` also
returns `some(0..0)` because its one finite optional clone is not a loop-back.
Both `(a?)*` and `(a?)*?` return group 1 as `none`; changing greedy to lazy
does not authorize same-boundary re-entry. Nested `((?:)*)*` also terminates
with group 1 `none`. The same first-visit rule is applied independently at
every nested loop instruction. On input `a`, `(a?)*` returns group 1 as
`some("a", 0..1)`; the discarded trailing empty attempt cannot overwrite it.
On empty input, lazy finite `()??` returns group 1 `none`, and nested `(()?)*`
returns both groups as `none` because the discarded outer iteration discards
all of its nested tag writes.

Unicode vectors include `(?i)K` matching U+212A KELVIN SIGN, `(?i)A` plus
U+0301 matching `a` plus U+0301 after both sides have already been segmented,
`(?i)ß` not matching `ss`, `(?i)İ` not matching `i`, and `(?i)[A]` not
matching `a`. U+00E9 does not match `e` plus U+0301 without an explicit
alternative. The vendored Unicode suite must additionally reproduce every
Unicode 17.0.0 `GraphemeBreakTest.txt` boundary.

For the work preflight, pattern `a{99}` has `S = 100`. An input of 499,999 EGCs
computes exactly 100,000,000 work units and passes that gate; 500,000 EGCs
computes 100,000,200 and returns `work_budget_exceeded(100000000)` before
matching. A 1,000,001-EGC input instead returns `input_too_large(1000000)`
first. Implementations must construct boundary vectors without relying on
wall-clock timeouts.

## Required Regression Matrix

The implementation must cover at least:

- every EBNF production, lexical escape, class edge case, decimal spelling,
  flags spelling, and stable error/position row above;
- literal, concatenation, alternation, absolute/multiline anchors, classes, each
  quantifier, greedy and lazy selection, and the three leading flags;
- rejection of each unsupported construct and each malformed syntax category;
- false/`none`/empty-list no-match results without an error;
- numbered, named, nested, repeated, and unmatched optional captures;
- exact grapheme-index spans for ASCII, non-ASCII, emoji, combining sequences,
  and CRLF;
- proof that dot and classes cannot split a multi-scalar grapheme;
- Unicode 17.0.0 UAX #29 revision 47 conformance, manifest digest rejection,
  exact matching without normalization, and every `C`/`S` simple-fold vector;
- leftmost-first alternatives and final-iteration repeated captures;
- empty input, empty pattern, empty alternatives, non-overlapping and adjacent
  matches, absolute anchors after continuation, and final empty termination;
- the state recurrence for every AST form, checked arithmetic before bounded
  cloning, and exact 65,536-state boundary vectors;
- every pattern, state, input, work, capture, repetition, and result limit at
  and just beyond its boundary, including error precedence;
- adversarial nullable/nested repetition under ordered epsilon de-duplication
  and checked work rejection of a large input/state cross-product;
- identical runtime, verify, and explicit comptime values/errors;
- trusted source ownership and rejection of project calls to private hooks;
- equivalent fixtures across every implemented execution backend.
