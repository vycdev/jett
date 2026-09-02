use crate::{Diagnostic, Severity};

/// Compute 1-based line and column from a byte offset in the source text.
/// Returns (line, column) where both are 1-based.
pub fn line_col(source: &str, byte_offset: u32) -> (usize, usize) {
    let offset = byte_offset as usize;
    let clamped = offset.min(source.len());
    let mut line = 1;
    let mut col = 1;
    let mut chars = source.char_indices().peekable();

    while let Some((index, ch)) = chars.next() {
        if index >= clamped {
            break;
        }
        match ch {
            '\r' => {
                line += 1;
                col = 1;
                // Treat CRLF as one line ending rather than two.
                if let Some(&(next_index, next_ch)) = chars.peek() {
                    if next_ch == '\n' && next_index < clamped {
                        chars.next();
                    }
                }
            }
            '\n' => {
                line += 1;
                col = 1;
            }
            _ => col += 1,
        }
    }
    (line, col)
}

/// Return the contents of a 1-based line number from the source text.
fn get_source_line(source: &str, line_number: usize) -> &str {
    if line_number == 0 {
        return "";
    }

    let bytes = source.as_bytes();
    let mut current_line = 1;
    let mut line_start = 0;
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'\n' || bytes[index] == b'\r' {
            if current_line == line_number {
                return &source[line_start..index];
            }
            current_line += 1;
            if bytes[index] == b'\r' && bytes.get(index + 1) == Some(&b'\n') {
                index += 1;
            }
            line_start = index + 1;
        }
        index += 1;
    }

    if current_line == line_number {
        &source[line_start..]
    } else {
        ""
    }
}

fn span_underline_len(
    source: &str,
    source_line: &str,
    column: usize,
    start: u32,
    end: u32,
) -> usize {
    let start = (start as usize).min(source.len());
    let end = (end as usize).min(source.len());
    let span_len = source
        .get(start..end)
        .map(|span| {
            span.chars()
                .take_while(|ch| *ch != '\n' && *ch != '\r')
                .count()
        })
        .unwrap_or(0)
        .max(1);
    let line_remaining = source_line.chars().count().saturating_sub(column - 1);

    span_len.min(line_remaining).max(1)
}

fn underline_padding(source_line: &str, column: usize) -> String {
    source_line
        .chars()
        .take(column.saturating_sub(1))
        .map(|ch| if ch == '\t' { '\t' } else { ' ' })
        .collect()
}

/// Render a single diagnostic with source context into a human-readable string.
///
/// Output format:
/// ```text
/// error[E0300]: type mismatch: expected int64, got string
///   --> tests/compile_fail/type_mismatch.jett:4:12
///    |
///  4 |     return a + b
///    |            ^^^^^ int64 + string is not allowed
///    |
///    hint: use explicit conversion with int64.from_string(b)
/// ```
pub fn render_diagnostic(diag: &Diagnostic, source: &str, file_path: &str) -> String {
    let mut out = String::new();

    // Severity prefix
    let severity_str = match diag.severity {
        Severity::Error => "error",
        Severity::Warning => "warning",
        Severity::Info => "info",
    };

    // Header line: error[E0300]: message
    out.push_str(&format!(
        "{}[{}]: {}\n",
        severity_str, diag.code, diag.message
    ));

    let (line, col) = line_col(source, diag.span.start);

    // Location line: --> file:line:col
    out.push_str(&format!("  --> {}:{}:{}\n", file_path, line, col));

    // Determine gutter width from every rendered line number so secondary labels align.
    let gutter_width = std::iter::once(line)
        .chain(
            diag.labels
                .iter()
                .map(|label| line_col(source, label.span.start).0),
        )
        .max()
        .unwrap_or(line)
        .to_string()
        .len();

    // Empty gutter line
    out.push_str(&format!("{} |\n", " ".repeat(gutter_width + 1)));

    // Source line
    let source_line = get_source_line(source, line);
    out.push_str(&format!(
        "{:>width$} | {}\n",
        line,
        source_line,
        width = gutter_width + 1
    ));

    // Underline with carets, clamped to the current source line.
    let underline_len =
        span_underline_len(source, source_line, col, diag.span.start, diag.span.end);

    // Build the underline: spaces up to col, then carets
    let padding = underline_padding(source_line, col);
    let carets = "^".repeat(underline_len);

    // Primary label message: use the first label if present, otherwise empty
    let primary_message = if !diag.labels.is_empty() {
        format!(" {}", diag.labels[0].message)
    } else {
        String::new()
    };

    out.push_str(&format!(
        "{} | {}{}{}\n",
        " ".repeat(gutter_width + 1),
        padding,
        carets,
        primary_message
    ));

    // Empty gutter line after underline
    out.push_str(&format!("{} |\n", " ".repeat(gutter_width + 1)));

    // Additional labels (secondary, starting from index 1)
    for label in diag.labels.iter().skip(1) {
        let (lbl_line, lbl_col) = line_col(source, label.span.start);
        let lbl_source_line = get_source_line(source, lbl_line);
        let lbl_underline_len = span_underline_len(
            source,
            lbl_source_line,
            lbl_col,
            label.span.start,
            label.span.end,
        );

        out.push_str(&format!(
            "{:>width$} | {}\n",
            lbl_line,
            lbl_source_line,
            width = gutter_width + 1
        ));
        out.push_str(&format!(
            "{} | {}{} {}\n",
            " ".repeat(gutter_width + 1),
            underline_padding(lbl_source_line, lbl_col),
            "^".repeat(lbl_underline_len),
            label.message
        ));
    }

    // Suggested fix
    if let Some(ref fix) = diag.suggested_fix {
        out.push_str(&format!("   hint: {}\n", fix.explanation));
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Diagnostic;
    use jett_common::{FileId, Span};

    #[test]
    fn line_col_first_char() {
        let source = "hello\nworld\n";
        assert_eq!(line_col(source, 0), (1, 1));
    }

    #[test]
    fn line_col_middle_of_first_line() {
        let source = "hello\nworld\n";
        assert_eq!(line_col(source, 3), (1, 4));
    }

    #[test]
    fn line_col_start_of_second_line() {
        let source = "hello\nworld\n";
        // byte 6 is 'w' on line 2
        assert_eq!(line_col(source, 6), (2, 1));
    }

    #[test]
    fn line_col_middle_of_second_line() {
        let source = "hello\nworld\n";
        // byte 8 is 'r' on line 2, col 3
        assert_eq!(line_col(source, 8), (2, 3));
    }

    #[test]
    fn line_col_past_end() {
        let source = "ab";
        // offset 10 is past end, should clamp
        assert_eq!(line_col(source, 10), (1, 3));
    }

    #[test]
    fn render_basic_error() {
        let source = "function main() returns int64:\n    return a + b\n";
        let file_id = FileId::new(0);
        let diag = Diagnostic::error(
            300,
            "type mismatch: expected int64, got string",
            Span::new(file_id, 41, 46),
        )
        .with_label(Span::new(file_id, 41, 46), "int64 + string is not allowed")
        .with_fix(
            Span::new(file_id, 41, 46),
            "a + b",
            "a + int64.from_string(b)",
            "use explicit conversion with int64.from_string(b)",
        );

        let rendered = render_diagnostic(&diag, source, "tests/compile_fail/type_mismatch.jett");

        // Check header
        assert!(rendered.contains("error[E0300]: type mismatch: expected int64, got string"));
        // Check file location
        assert!(rendered.contains("--> tests/compile_fail/type_mismatch.jett:2:"));
        // Check source line is present
        assert!(rendered.contains("return a + b"));
        // Check carets are present
        assert!(rendered.contains("^^^^^"));
        // Check label message
        assert!(rendered.contains("int64 + string is not allowed"));
        // Check hint
        assert!(rendered.contains("hint: use explicit conversion with int64.from_string(b)"));
    }

    #[test]
    fn render_warning_without_labels() {
        let source = "function foo() returns nothing:\n    int64 x = 42\n";
        let file_id = FileId::new(0);
        let diag = Diagnostic::warning(100, "unused variable: x", Span::new(file_id, 41, 42));

        let rendered = render_diagnostic(&diag, source, "test.jett");

        assert!(rendered.contains("warning[E0100]: unused variable: x"));
        assert!(rendered.contains("--> test.jett:2:"));
        assert!(rendered.contains("^"));
    }

    #[test]
    fn render_diagnostic_handles_lone_carriage_return_lines() {
        let source = "first\rsecond\rthird";
        let file_id = FileId::new(0);
        let diag = Diagnostic::error(300, "invalid value", Span::new(file_id, 6, 12));

        let rendered = render_diagnostic(&diag, source, "test.jett");

        assert!(rendered.contains("--> test.jett:2:1"));
        assert!(rendered.contains("2 | second"));
    }

    #[test]
    fn render_unicode_spans_use_character_width() {
        let source = "let π = λ\n";
        let file_id = FileId::new(0);
        let diag = Diagnostic::error(300, "invalid value", Span::new(file_id, 4, 6))
            .with_label(Span::new(file_id, 4, 6), "primary")
            .with_label(Span::new(file_id, 9, 11), "secondary");

        let rendered = render_diagnostic(&diag, source, "test.jett");
        let primary = rendered
            .lines()
            .find(|line| line.ends_with("primary"))
            .expect("primary label should be rendered");
        let secondary = rendered
            .lines()
            .find(|line| line.ends_with("secondary"))
            .expect("secondary label should be rendered");

        assert_eq!(primary.matches('^').count(), 1);
        assert_eq!(secondary.matches('^').count(), 1);
    }

    #[test]
    fn render_diagnostic_preserves_tabs_before_underlines() {
        let source = "let\tvalue\n";
        let file_id = FileId::new(0);
        let diag = Diagnostic::error(300, "tabs are not allowed", Span::new(file_id, 4, 9))
            .with_label(Span::new(file_id, 4, 9), "invalid token");

        let rendered = render_diagnostic(&diag, source, "test.jett");
        let underline = rendered
            .lines()
            .find(|line| line.ends_with("invalid token"))
            .expect("primary underline should be rendered");
        let padding = underline
            .strip_prefix("   | ")
            .expect("underline should include its gutter")
            .strip_suffix("^^^^^ invalid token")
            .expect("underline should include its carets and label");

        assert_eq!(padding, "   \t");
    }

    #[test]
    fn render_secondary_labels_align_with_wide_line_numbers() {
        let source = format!("{}last\n", "first\n".repeat(99));
        let file_id = FileId::new(0);
        let secondary_start = (source.len() - 5) as u32;
        let diag = Diagnostic::error(300, "multiple locations", Span::new(file_id, 0, 1))
            .with_label(Span::new(file_id, 0, 1), "primary")
            .with_label(
                Span::new(file_id, secondary_start, secondary_start + 4),
                "secondary",
            );

        let rendered = render_diagnostic(&diag, &source, "test.jett");
        let source_lines: Vec<_> = rendered.lines().collect();
        let primary_source = source_lines
            .iter()
            .find(|line| line.contains(" | first"))
            .expect("primary source line should be rendered");
        let secondary_source = source_lines
            .iter()
            .find(|line| line.contains(" | last"))
            .expect("secondary source line should be rendered");
        let secondary_underline = source_lines
            .iter()
            .find(|line| line.ends_with("secondary"))
            .expect("secondary label should be rendered");

        assert_eq!(
            primary_source.find('|'),
            secondary_source.find('|'),
            "source gutters should share one column"
        );
        assert_eq!(
            secondary_source.find('|'),
            secondary_underline.find('|'),
            "secondary underline should share the source gutter"
        );
    }
}
