use jett_profiler::{SanitizedSourceExcerpt, SourceExcerptMetadata, sanitize_source_excerpt};

fn metadata(contains_secret: Option<bool>) -> SourceExcerptMetadata {
    SourceExcerptMetadata {
        manifest_authorized: true,
        contains_secret,
    }
}

#[test]
fn redacts_string_literal_contents() {
    let excerpt = sanitize_source_excerpt("log.emit(\"api-key\")", metadata(Some(false)));

    assert_eq!(
        excerpt,
        Some(SanitizedSourceExcerpt {
            code: "log.emit(<redacted-literal>)".to_string(),
            source_redacted: true,
        })
    );
}

#[test]
fn redacts_byte_literal_contents() {
    let excerpt = sanitize_source_excerpt("bytes = b\"\\x00\\xff\"", metadata(Some(false)));

    assert_eq!(
        excerpt,
        Some(SanitizedSourceExcerpt {
            code: "bytes = <redacted-literal>".to_string(),
            source_redacted: true,
        })
    );
}

#[test]
fn omits_comments() {
    let excerpt = sanitize_source_excerpt(
        "log.emit(value) # never expose the comment",
        metadata(Some(false)),
    );

    assert_eq!(
        excerpt,
        Some(SanitizedSourceExcerpt {
            code: "log.emit(value)".to_string(),
            source_redacted: true,
        })
    );
}

#[test]
fn replaces_secret_typed_spans_without_scanning_their_source() {
    let excerpt = sanitize_source_excerpt("token.value", metadata(Some(true)));

    assert_eq!(
        excerpt,
        Some(SanitizedSourceExcerpt {
            code: "<secret-expression>".to_string(),
            source_redacted: true,
        })
    );
}

#[test]
fn withholds_excerpts_without_authorization_or_checked_metadata() {
    assert_eq!(
        sanitize_source_excerpt(
            "safe_name",
            SourceExcerptMetadata {
                manifest_authorized: false,
                contains_secret: Some(false),
            },
        ),
        None
    );
    assert_eq!(sanitize_source_excerpt("safe_name", metadata(None)), None);
    assert_eq!(
        sanitize_source_excerpt("\"unterminated", metadata(Some(false))),
        None
    );
}

#[test]
fn escapes_controls_and_limits_output_at_a_utf8_boundary() {
    let controls = sanitize_source_excerpt("status\tready\n", metadata(Some(false)))
        .expect("authorized checked source");
    assert_eq!(controls.code, "status\\tready\\n");
    assert!(controls.source_redacted);

    let long_source = "é".repeat(81);
    let limited = sanitize_source_excerpt(&long_source, metadata(Some(false)))
        .expect("authorized checked source");
    assert_eq!(limited.code, "é".repeat(80));
    assert_eq!(limited.code.len(), 160);
    assert!(limited.source_redacted);
}
