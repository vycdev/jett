# CSV Format and Failure Contract

Status: records the interpreter-backed behavior implemented before this
contract was completed on 2026-08-19. Future execution backends must preserve
the same public contract.

## Context

Jett's initial `csv` surface began as three infallible compiler-owned names.
The interpreter accepted several malformed quote forms, silently truncated
header rows through pairwise zipping, retained a leading UTF-8 byte order mark,
and did not identify which behavior was portable language policy.

The implemented surface now uses source-owned public declarations over small
private trusted kernels. This record fixes the dialect, failure model, row
shape, and source/runtime boundary selected by the implementation work for
[#137](https://github.com/vycdev/jett/issues/137). It is deliberately narrower
than a general tabular-data or reflected-decoding API.

## Public Surface

The declarations in `stdlib/csv.jett` are canonical:

```jett
namespace csv

export function parse(value: string) returns result[list[list[string]], string]
export function stringify(rows: list[list[string]]) returns string
export function parse_with_header(value: string) returns result[list[map[string, string]], string]
```

All three operations are pure, deterministic, capability-free, and independent
of the host platform. Parsing returns either the complete value or one handled
`string` error; it never returns partial rows. `stringify` consumes its list
argument under the ordinary Jett ownership rules.

## Dialect

Jett uses one comma-separated UTF-8 text dialect inspired by RFC 4180, with
these explicit choices:

- the delimiter is exactly `,`;
- a field may be unquoted or double-quoted;
- a quote opens a quoted field only when it is the field's first character;
- `""` inside a quoted field represents one literal `"`;
- after a closing quote, only a comma, a record ending, or end of input is
  valid;
- LF and CRLF are accepted as record endings outside quoted fields;
- a bare CR outside a quoted field is invalid;
- LF, CRLF, and bare CR inside quoted fields are field data and are preserved
  exactly;
- unquoted leading and trailing whitespace is data and is never trimmed;
- Unicode scalar values are preserved without normalization, case folding, or
  locale conversion.

This is not a claim of strict RFC 4180 conformance. In particular, LF input is
accepted, raw rows may be ragged, empty input has no records, and the canonical
writer always uses LF.

## Records and Fields

`csv.parse` preserves the shape present in the input:

- `""` contains zero records;
- one leading U+FEFF byte order mark is ignored before parsing, while U+FEFF at
  any later position remains field data;
- a physical blank line is one record containing one empty field;
- commas preserve empty fields, including leading, adjacent, and trailing empty
  fields;
- raw rows do not need equal widths;
- a final LF or CRLF terminates the current record but does not create an extra
  empty record after it;
- absence of a final record ending is valid;
- quoted commas, quotes, and line endings are returned as field data after
  unescaping doubled quotes.

The BOM rule applies once to the complete input, before header processing. It
does not strip U+FEFF from every record or field.

Every parsed physical record has at least one field. The type accepted by
`stringify` can still contain a zero-field row, and the writer emits both that
row and a one-empty-field row as an empty output line. Consequently the initial
surface does not promise structural round trips for zero-field rows, a sole
blank record, or a final blank record: the empty string parses as zero records,
and a final line ending does not create another record. Callers that must retain
those distinctions need an application-level shape convention.

## Header Parsing

`csv.parse_with_header` first applies the same raw parser, then interprets the
first record as headers:

- empty input succeeds with an empty list;
- every header must be non-empty;
- header names use exact, case-sensitive string equality and must be unique;
- every subsequent record must have exactly the header width;
- a blank data record therefore succeeds only for a one-column header;
- each data row becomes a `map[string, string]` pairing each header with the
  field in the same position;
- a header-only input succeeds with no data rows.

Raw `csv.parse` intentionally preserves ragged rows. Width validation belongs
to `parse_with_header`; the raw parser does not infer a schema or silently pad,
truncate, or discard fields.

## Canonical String Output

`csv.stringify` emits one deterministic spelling:

- fields are joined with `,` and records with LF;
- there is no final newline;
- a field is quoted exactly when it contains a comma, quote, LF, or CR;
- an empty field is not otherwise quoted;
- quotes inside a quoted field are doubled;
- spaces and other Unicode text are emitted unchanged;
- an empty list emits the empty string.

The writer does not preserve an input BOM or an input choice of LF versus CRLF.
It emits canonical data rather than source formatting. Except for the blank and
zero-field ambiguities above, parsing and then stringifying valid records
preserves field values and row shape while normalizing record endings, quoting,
and the optional leading BOM.

## Error Contract

The initial public error representation is `string`, matching other fallible
stdlib operations. Locations are one-based. Parsing reports the current record
and field; header validation reports either the header field or data-record
number.

The stable malformed-input categories are:

```text
CSV parse error at record <record>, field <field>: unterminated quoted field
CSV parse error at record <record>, field <field>: unexpected quote in unquoted field
CSV parse error at record <record>, field <field>: unexpected character after closing quote
CSV parse error at record <record>, field <field>: bare carriage return; use LF or CRLF record endings
CSV header error at field <field>: header must not be empty
CSV header error at field <field>: duplicate header '<header>'
CSV header error at record <record>: expected <expected> fields, got <actual>
```

The duplicate-header category inserts the exact header string between the
single quotes shown above without escaping, including any quote or line-ending
characters in that header. This is the implemented deterministic text shape,
not a format intended for machine parsing.

The parser scans left to right and returns the first malformed record/field
condition. `parse_with_header` completes raw syntax parsing before validating
headers and row widths, then validates headers from left to right and data rows
from top to bottom. Future structured diagnostics require a separate migration;
backends must not expose host parser wording, byte offsets, or debug formatting
through this `string` result.

Ordinary allocation and execution limits remain runtime concerns. Invalid CSV
is a handled domain error. A missing trusted kernel or an internal invariant
failure is a compiler/runtime defect and must not be reported as malformed
input.

## Source and Runtime Boundary

Public names, signatures, ownership, and result types belong to trusted
compiler-shipped `stdlib/csv.jett`. The compiler does not own public
`csv.parse`, `csv.stringify`, or `csv.parse_with_header` signatures.

The interpreter currently retains private trusted kernels for raw parsing,
quoting, header validation, width validation, and header-map construction
because Jett source cannot yet express these operations clearly within the
language's complexity policy. Those kernels are implementation details:

- project code cannot import or call them;
- matching source spellings do not grant trust;
- project and dependency declarations cannot contribute to the compiler-shipped
  `csv` namespace;
- dispatch depends on resolved trusted stdlib origin, not a public name alone.

Public wrappers and validation should remain in `.jett` source whenever the
language can express them safely. Future backends may use a runtime library,
but its permissive defaults must be constrained to this contract.

## Compatibility and Exclusions

The former infallible signatures are not compatibility aliases. Callers must
handle parse failures explicitly. The selected migration is complete for the
interpreter-backed compiler: source declarations own the public surface and the
runtime exposes only private trusted kernels.

This contract intentionally excludes:

- typed or reflected `csv.parse_rows[T]`;
- schema inference or automatic scalar conversion;
- alternate delimiters, configurable quote characters, or dialect modes;
- streaming parsers or writers;
- filesystem, network, or other I/O;
- locale-aware conversion;
- preserving original CSV formatting.

A future typed-row API must use the canonical reflected-construction mechanism
and remain separate from raw CSV parsing. It must not add a CSV-specific
construction primitive or silently change this raw format/failure contract.

## Required Regression Matrix

The shared interpreter and future-backend conformance corpus must cover:

- empty input, one blank line, internal blank records, and final LF/CRLF;
- leading, adjacent, and trailing empty fields;
- ragged raw rows and exact-width header rows;
- quoted commas, doubled quotes, LF/CRLF/bare-CR field data, and Unicode;
- one leading BOM ignored and a later U+FEFF preserved;
- unquoted leading/trailing whitespace preserved;
- each malformed quote category and bare CR outside quotes;
- empty and duplicate headers, including exact case-sensitive comparison;
- short, long, and blank data rows under header parsing;
- canonical quoting, LF output, and no final newline;
- the documented empty-list, blank-record, trailing-blank, and zero-field-row
  serialization ambiguities;
- rejection of untrusted private-kernel calls and confirmation that public
  signatures resolve from `stdlib/csv.jett`;
- equivalent values and error categories across every execution backend.

Future HIR, MIR, bytecode, and native work must carry this corpus forward rather
than inheriting a host CSV library's dialect.
