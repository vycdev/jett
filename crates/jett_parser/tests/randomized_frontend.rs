use std::panic::{AssertUnwindSafe, catch_unwind};

use jett_common::{FileId, Span};
use jett_diagnostics::Diagnostic;

const MAX_SOURCE_BYTES: usize = 1_024;
const RANDOM_CASES_PER_SEED: usize = 256;
const SEEDS: [u64; 4] = [
    0x4a45_5454_4652_4f4e,
    0x0123_4567_89ab_cdef,
    0xa5a5_5a5a_dead_beef,
    0xffff_ffff_ffff_ffff,
];

const EDGE_CASES: &[&str] = &[
    "",
    "\0",
    "\u{1f}\u{7f}",
    "\tfunction main():\n   return",
    "function main():\n  return\n        return",
    "\r\n    \r\n\t# comment\r\n",
    "\"unterminated",
    "\"interpolation ${",
    "\"interpolation ${value",
    "\"interpolation ${\"nested",
    "function",
    "function main(",
    "struct Value:\n    field:",
    "if true:\n    return\nelse",
    "namespace 世界\nfunction λ() returns string:\n    return \"🦀 café\"",
    "# 😀\u{0301}\u{200d} comment\nfunction main():\n    return",
];

const FRAGMENTS: &[&str] = &[
    "\n",
    "\r\n",
    "\r",
    "\t",
    " ",
    "   ",
    "    ",
    "# comment",
    "function ",
    "struct ",
    "namespace ",
    "return ",
    "if ",
    "else:",
    "\"",
    "\\",
    "${",
    "}",
    "(",
    ")",
    "[",
    "]",
    ":",
    ",",
    "世界",
    "λ",
    "🦀",
    "e\u{301}",
    "\0",
    "\u{1b}",
];

#[test]
fn curated_malformed_and_unicode_sources_are_panic_safe() {
    for (index, source) in EDGE_CASES.iter().enumerate() {
        run_case(source, &format!("edge case {index}"));
    }
}

#[test]
fn deterministic_random_utf8_is_panic_safe_and_has_valid_spans() {
    for seed in SEEDS {
        let mut random = DeterministicRandom::new(seed);
        for case_index in 0..RANDOM_CASES_PER_SEED {
            let source = random_source(&mut random);
            run_case(&source, &format!("seed 0x{seed:016x}, case {case_index}"));
        }
    }
}

fn file() -> FileId {
    FileId::new(17)
}

fn run_case(source: &str, case: &str) {
    assert!(
        source.len() <= MAX_SOURCE_BYTES,
        "generator exceeded its byte bound for {case}"
    );

    let outcome = catch_unwind(AssertUnwindSafe(|| check_frontend(source, case)));
    assert!(
        outcome.is_ok(),
        "frontend panicked for {case}; source = {:?}",
        source.escape_debug().to_string()
    );
}

fn check_frontend(source: &str, case: &str) {
    let lexed = jett_lexer::tokenize(source, file());

    for (index, token) in lexed.tokens.iter().enumerate() {
        assert_source_span(source, token.span, &format!("token {index}"), case);
    }
    assert_spans_follow_source_order(
        lexed.tokens.iter().map(|token| token.span),
        "tokens",
        source,
        case,
    );

    for (index, comment) in lexed.comments.iter().enumerate() {
        assert_source_span(source, comment.span, &format!("comment {index}"), case);
    }
    assert_spans_follow_source_order(
        lexed.comments.iter().map(|comment| comment.span),
        "comments",
        source,
        case,
    );

    for (index, error) in lexed.errors.iter().enumerate() {
        assert_source_span(source, error.span, &format!("lexer error {index}"), case);
    }

    let parsed = jett_parser::parse(source, file());
    assert_source_span(source, parsed.module.span, "module", case);
    for (index, diagnostic) in parsed.errors.iter().enumerate() {
        assert_diagnostic_spans(source, diagnostic, index, case);
    }
}

fn assert_diagnostic_spans(source: &str, diagnostic: &Diagnostic, index: usize, case: &str) {
    assert_source_span(
        source,
        diagnostic.span,
        &format!("diagnostic {index}"),
        case,
    );
    for (label_index, label) in diagnostic.labels.iter().enumerate() {
        assert_source_span(
            source,
            label.span,
            &format!("diagnostic {index} label {label_index}"),
            case,
        );
    }
    if let Some(fix) = &diagnostic.suggested_fix {
        assert_source_span(
            source,
            fix.span,
            &format!("diagnostic {index} suggested fix"),
            case,
        );
    }
}

fn assert_source_span(source: &str, span: Span, kind: &str, case: &str) {
    let start = span.start as usize;
    let end = span.end as usize;
    assert_eq!(span.file, file(), "wrong file ID for {kind} in {case}");
    assert!(
        start <= end,
        "reversed {kind} span {start}..{end} in {case}"
    );
    assert!(
        end <= source.len(),
        "out-of-bounds {kind} span {start}..{end} for {} bytes in {case}",
        source.len()
    );
    assert!(
        source.is_char_boundary(start),
        "{kind} span starts inside UTF-8 at byte {start} in {case}"
    );
    assert!(
        source.is_char_boundary(end),
        "{kind} span ends inside UTF-8 at byte {end} in {case}"
    );
    let _ = &source[start..end];
}

fn assert_spans_follow_source_order(
    spans: impl Iterator<Item = Span>,
    kind: &str,
    source: &str,
    case: &str,
) {
    let mut previous_start = 0;
    for span in spans {
        let start = span.start as usize;
        assert!(
            start >= previous_start,
            "{kind} move backward from byte {previous_start} to {start} in {case}; source = {:?}",
            source.escape_debug().to_string()
        );
        previous_start = start;
    }
}

fn random_source(random: &mut DeterministicRandom) -> String {
    let target_parts = random.usize(129);
    let mut source = String::new();

    for _ in 0..target_parts {
        if random.usize(3) == 0 {
            let fragment = FRAGMENTS[random.usize(FRAGMENTS.len())];
            if source.len() + fragment.len() <= MAX_SOURCE_BYTES {
                source.push_str(fragment);
            }
        } else {
            let character = random_scalar(random);
            if source.len() + character.len_utf8() <= MAX_SOURCE_BYTES {
                source.push(character);
            }
        }
    }

    source
}

fn random_scalar(random: &mut DeterministicRandom) -> char {
    match random.usize(5) {
        0 => char::from_u32(random.usize(0x80) as u32).expect("ASCII is valid Unicode"),
        1 => char::from_u32(0x80 + random.usize(0x780) as u32)
            .expect("selected two-byte scalar is valid Unicode"),
        2 => loop {
            let candidate = 0x800 + random.usize(0xf800) as u32;
            if let Some(character) = char::from_u32(candidate) {
                break character;
            }
        },
        3 => char::from_u32(0x1_0000 + random.usize(0x10_0000) as u32)
            .expect("selected supplementary scalar is valid Unicode"),
        _ => loop {
            if let Some(character) = char::from_u32(random.usize(0x11_0000) as u32) {
                break character;
            }
        },
    }
}

struct DeterministicRandom {
    state: u64,
}

impl DeterministicRandom {
    fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    fn next_u64(&mut self) -> u64 {
        // SplitMix64 is small, stable, and sufficient for deterministic test generation.
        self.state = self.state.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut value = self.state;
        value = (value ^ (value >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        value = (value ^ (value >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        value ^ (value >> 31)
    }

    fn usize(&mut self, upper_exclusive: usize) -> usize {
        (self.next_u64() % upper_exclusive as u64) as usize
    }
}
