use jett_regex::{CompileError, InvalidPattern, compile_pattern};

#[test]
fn computes_canonical_program_state_counts() {
    let cases = [
        ("", 1),
        ("a", 2),
        ("a|b", 4),
        ("(a)", 4),
        ("a?", 3),
        ("a*", 3),
        ("a+", 3),
        ("a{2}", 3),
        ("a{2,4}", 7),
        ("a{2,}", 5),
        ("(ab){2,3}", 14),
    ];

    for (pattern, expected) in cases {
        let compiled = compile_pattern(pattern).expect(pattern);
        assert_eq!(compiled.state_count(), expected, "{pattern}");
    }
}

#[test]
fn reports_contract_diagnostics_at_grapheme_positions() {
    let cases = [
        (
            ")",
            InvalidPattern {
                position: 0,
                message: "unexpected token",
            },
        ),
        (
            "(a",
            InvalidPattern {
                position: 2,
                message: "unclosed group",
            },
        ),
        (
            "[]",
            InvalidPattern {
                position: 1,
                message: "empty character class",
            },
        ),
        (
            "a{3,2}",
            InvalidPattern {
                position: 1,
                message: "quantifier range is reversed",
            },
        ),
        (
            "(?mi)a",
            InvalidPattern {
                position: 0,
                message: "flag group must be leading and canonical",
            },
        ),
        (
            "(?P<x>a)(?P<x>b)",
            InvalidPattern {
                position: 12,
                message: "capture name is duplicated",
            },
        ),
    ];

    for (pattern, expected) in cases {
        assert_eq!(
            compile_pattern(pattern),
            Err(CompileError::InvalidPattern(expected)),
            "{pattern}"
        );
    }
}

#[test]
fn records_flags_and_capture_metadata() {
    let compiled = compile_pattern("(?ims)(a)(?:b)(?P<label>c)").unwrap();

    assert!(compiled.flags().case_insensitive);
    assert!(compiled.flags().multi_line);
    assert!(compiled.flags().dot_matches_line_endings);
    assert_eq!(compiled.capture_count(), 2);
    assert_eq!(compiled.named_captures(), &[("label".to_string(), 2)]);
}

#[test]
fn treats_literal_sequences_as_grapheme_atoms() {
    let combining_cluster = compile_pattern("a\u{301}").unwrap();
    let escaped_line_feed = compile_pattern(r"\n").unwrap();
    let scalar_class = compile_pattern(r"[^a-z\d]").unwrap();

    assert_eq!(combining_cluster.state_count(), 2);
    assert_eq!(escaped_line_feed.state_count(), 2);
    assert_eq!(scalar_class.state_count(), 2);
}

#[test]
fn rejects_unsupported_and_malformed_contract_forms() {
    let cases = [
        (r"\q", 0, "unsupported construct"),
        (r"\@", 0, "invalid escape"),
        ("(?=a)", 0, "unsupported construct"),
        ("a**", 2, "invalid quantifier"),
        ("[z-a]", 2, "character class range is reversed"),
        (r"[\d-a]", 3, "unexpected token"),
        ("(?P<1>x)", 4, "capture name is invalid"),
        ("a{65536})", 8, "unexpected token"),
        ("a{65536}x)", 9, "unexpected token"),
    ];

    for (pattern, position, message) in cases {
        assert_eq!(
            compile_pattern(pattern),
            Err(CompileError::InvalidPattern(InvalidPattern {
                position,
                message,
            })),
            "{pattern}"
        );
    }
}

#[test]
fn rejects_patterns_and_programs_above_portable_limits() {
    assert_eq!(
        compile_pattern(&"a".repeat(4_097)),
        Err(CompileError::PatternTooLarge { limit: 4_096 })
    );
    assert_eq!(compile_pattern("a{65535}").unwrap().state_count(), 65_536);
    assert_eq!(
        compile_pattern("a{65536}"),
        Err(CompileError::CompiledPatternTooLarge { limit: 65_536 })
    );
}
