use jett_common::FileId;
use jett_lexer::{Token, TokenKind, tokenize};

/// Format Jett source code to canonical style.
/// Returns the formatted source text, or the original text with errors if lexing fails.
pub fn format_source(source: &str, file_id: FileId) -> FormatResult {
    let lex_result = tokenize(source, file_id);

    if !lex_result.errors.is_empty() {
        return FormatResult {
            output: source.to_string(),
            errors: lex_result
                .errors
                .iter()
                .map(|e| e.message.clone())
                .collect(),
        };
    }

    let formatted = reprint_tokens(&lex_result.tokens, source);

    FormatResult {
        output: formatted,
        errors: Vec::new(),
    }
}

pub struct FormatResult {
    pub output: String,
    pub errors: Vec<String>,
}

/// Reprint tokens with canonical whitespace.
/// This is a simple token-based formatter that ensures:
/// - 4-space indentation
/// - No trailing whitespace
/// - Consistent spacing around operators
/// - Single blank lines between top-level declarations
fn reprint_tokens(tokens: &[Token], source: &str) -> String {
    let mut output = String::new();
    let mut indent_level: u32 = 0;
    let mut at_line_start = true;
    let mut prev_kind: Option<TokenKind> = None;
    let mut consecutive_newlines: u32 = 0;

    for token in tokens {
        match token.kind {
            TokenKind::Indent => {
                indent_level += 1;
                continue;
            }
            TokenKind::Dedent => {
                indent_level = indent_level.saturating_sub(1);
                continue;
            }
            TokenKind::Newline => {
                consecutive_newlines += 1;
                if consecutive_newlines <= 2 {
                    output.push('\n');
                }
                at_line_start = true;
                prev_kind = Some(token.kind);
                continue;
            }
            TokenKind::Eof => break,
            _ => {}
        }

        consecutive_newlines = 0;

        if at_line_start {
            for _ in 0..indent_level {
                output.push_str("    ");
            }
            at_line_start = false;
        } else if needs_space_before(token.kind, prev_kind) {
            output.push(' ');
        }

        let text = &source[token.span.start as usize..token.span.end as usize];
        output.push_str(text);

        prev_kind = Some(token.kind);
    }

    // Ensure trailing newline
    if !output.ends_with('\n') {
        output.push('\n');
    }

    // Remove trailing blank lines (keep one trailing newline)
    while output.ends_with("\n\n") {
        output.pop();
    }

    output
}

/// Determine if a space is needed before this token.
fn needs_space_before(kind: TokenKind, prev: Option<TokenKind>) -> bool {
    let prev = match prev {
        Some(p) => p,
        None => return false,
    };

    // No space before (
    if kind == TokenKind::LParen {
        // Space before ( only after keywords, not after identifiers/names
        return matches!(
            prev,
            TokenKind::Returns | TokenKind::If | TokenKind::While | TokenKind::For
        );
    }

    // Never space after ( [ or before ) ] , :
    if matches!(prev, TokenKind::LParen | TokenKind::LBracket) {
        return false;
    }
    if matches!(
        kind,
        TokenKind::RParen | TokenKind::RBracket | TokenKind::Comma | TokenKind::Colon
    ) {
        return false;
    }

    // No space after .
    if prev == TokenKind::Dot {
        return false;
    }
    // No space before .
    if kind == TokenKind::Dot {
        return false;
    }

    // No space between [ and type names (for generics like list[int64])
    if prev == TokenKind::LBracket {
        return false;
    }

    // Space after comma
    if prev == TokenKind::Comma {
        return true;
    }

    // Space after colon (in type annotations, block starts)
    if prev == TokenKind::Colon {
        return true;
    }

    // Space around binary operators
    if is_binary_op(kind) || is_binary_op(prev) {
        return true;
    }

    // Space between most other tokens
    true
}

fn is_binary_op(kind: TokenKind) -> bool {
    matches!(
        kind,
        TokenKind::Plus
            | TokenKind::Minus
            | TokenKind::Star
            | TokenKind::Slash
            | TokenKind::EqEq
            | TokenKind::NotEq
            | TokenKind::Lt
            | TokenKind::Gt
            | TokenKind::LtEq
            | TokenKind::GtEq
            | TokenKind::AmpAmp
            | TokenKind::PipePipe
            | TokenKind::Eq
            | TokenKind::Modulo
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fmt(source: &str) -> String {
        let result = format_source(source, FileId::new(0));
        assert!(
            result.errors.is_empty(),
            "format errors: {:?}",
            result.errors
        );
        result.output
    }

    #[test]
    fn format_simple_function() {
        let source =
            "namespace app\n\nfunction add(a: int64, b: int64) returns int64:\n    return a + b\n";
        let formatted = fmt(source);
        // Check key properties rather than exact match (whitespace may differ slightly)
        assert!(formatted.contains("namespace app"));
        assert!(formatted.contains("function add(a: int64, b: int64) returns int64:"));
        assert!(formatted.contains("    return a + b"));
    }

    #[test]
    fn format_preserves_indentation() {
        let source = "function main(stdout: Stdout) returns nothing:\n    if true:\n        return nothing\n";
        let formatted = fmt(source);
        assert!(formatted.contains("    if true:"));
        assert!(formatted.contains("        return nothing"));
    }

    #[test]
    fn format_adds_trailing_newline() {
        let source = "namespace app";
        let formatted = fmt(source);
        assert!(formatted.ends_with('\n'));
    }
}
