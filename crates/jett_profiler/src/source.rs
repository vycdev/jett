const MAX_EXCERPT_BYTES: usize = 160;
const REDACTED_LITERAL: &str = "<redacted-literal>";
const SECRET_EXPRESSION: &str = "<secret-expression>";

/// Checked source facts required before a profiler may expose a source excerpt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SourceExcerptMetadata {
    /// The source belongs to the loaded project, dependency, or stdlib manifest.
    pub manifest_authorized: bool,
    /// Whether the checked type of the excerpt contains secret data.
    ///
    /// `None` means checked metadata is unavailable, so no excerpt is safe.
    pub contains_secret: Option<bool>,
}

/// A bounded profiler source excerpt and whether sanitization changed it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SanitizedSourceExcerpt {
    pub code: String,
    pub source_redacted: bool,
}

/// Sanitize one manifest-backed, checked source span for profiler output.
///
/// The scanner is deliberately conservative. If it cannot prove that every
/// quoted literal is bounded and non-interpolated, it withholds the excerpt
/// instead of risking disclosure through malformed or nested source text.
pub fn sanitize_source_excerpt(
    source: &str,
    metadata: SourceExcerptMetadata,
) -> Option<SanitizedSourceExcerpt> {
    if !metadata.manifest_authorized {
        return None;
    }
    match metadata.contains_secret {
        None => return None,
        Some(true) => {
            return Some(SanitizedSourceExcerpt {
                code: SECRET_EXPRESSION.to_string(),
                source_redacted: true,
            });
        }
        Some(false) => {}
    }

    let mut chars = source.chars().peekable();
    let mut output = String::with_capacity(source.len().min(MAX_EXCERPT_BYTES));
    let mut redacted = false;
    let mut truncated = false;

    while let Some(character) = chars.next() {
        if character == '#' {
            while output.ends_with(char::is_whitespace) {
                output.pop();
            }
            redacted = true;
            break;
        }

        if character == 'b' && chars.peek() == Some(&'"') {
            chars.next();
            consume_quoted_literal(&mut chars)?;
            push_bounded(&mut output, REDACTED_LITERAL, &mut truncated);
            redacted = true;
            continue;
        }

        if character == '"' {
            consume_quoted_literal(&mut chars)?;
            push_bounded(&mut output, REDACTED_LITERAL, &mut truncated);
            redacted = true;
            continue;
        }

        if character.is_control() {
            let mut escaped = String::new();
            push_escaped_control(&mut escaped, character);
            push_bounded(&mut output, &escaped, &mut truncated);
            redacted = true;
        } else {
            let mut bytes = [0; 4];
            push_bounded(
                &mut output,
                character.encode_utf8(&mut bytes),
                &mut truncated,
            );
        }
    }

    Some(SanitizedSourceExcerpt {
        code: output,
        source_redacted: redacted || truncated,
    })
}

fn push_bounded(output: &mut String, text: &str, truncated: &mut bool) {
    if *truncated {
        return;
    }
    let mut boundary = text.len().min(MAX_EXCERPT_BYTES - output.len());
    while !text.is_char_boundary(boundary) {
        boundary -= 1;
    }
    output.push_str(&text[..boundary]);
    *truncated = boundary != text.len();
}

fn consume_quoted_literal<I>(chars: &mut std::iter::Peekable<I>) -> Option<()>
where
    I: Iterator<Item = char>,
{
    let mut escaped = false;
    for character in chars.by_ref() {
        if escaped {
            escaped = false;
            continue;
        }
        match character {
            '\\' => escaped = true,
            '"' => return Some(()),
            // Interpolated strings require the compiler lexer to identify
            // nested expression literals safely. Withhold until that token
            // stream is available to the profiler adapter.
            '{' => return None,
            _ => {}
        }
    }
    None
}

fn push_escaped_control(output: &mut String, character: char) {
    match character {
        '\n' => output.push_str("\\n"),
        '\r' => output.push_str("\\r"),
        '\t' => output.push_str("\\t"),
        other => output.push_str(&format!("\\u{{{:x}}}", other as u32)),
    }
}
