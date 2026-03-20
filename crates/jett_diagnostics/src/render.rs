use crate::{Diagnostic, Severity};

/// Compute 1-based line and column from a byte offset in the source text.
/// Returns (line, column) where both are 1-based.
pub fn line_col(source: &str, byte_offset: u32) -> (usize, usize) {
    let offset = byte_offset as usize;
    let clamped = offset.min(source.len());
    let mut line = 1;
    let mut col = 1;
    for (i, ch) in source.char_indices() {
        if i >= clamped {
            break;
        }
        if ch == '\n' {
            line += 1;
            col = 1;
        } else {
            col += 1;
        }
    }
    (line, col)
}

/// Return the contents of a 1-based line number from the source text.
fn get_source_line(source: &str, line_number: usize) -> &str {
    source.lines().nth(line_number - 1).unwrap_or("")
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
    out.push_str(&format!("{}[{}]: {}\n", severity_str, diag.code, diag.message));

    let (line, col) = line_col(source, diag.span.start);

    // Location line: --> file:line:col
    out.push_str(&format!("  --> {}:{}:{}\n", file_path, line, col));

    // Determine gutter width based on line number
    let gutter_width = line.to_string().len();

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

    // Underline with carets
    // Compute the start column within this line (1-based) and the span length
    let span_len = if diag.span.end > diag.span.start {
        (diag.span.end - diag.span.start) as usize
    } else {
        1
    };

    // Clamp underline length to not exceed end of the source line
    let underline_len = span_len.min(source_line.len().saturating_sub(col - 1)).max(1);

    // Build the underline: spaces up to col, then carets
    let padding = " ".repeat(col - 1);
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
        let lbl_span_len = if label.span.end > label.span.start {
            (label.span.end - label.span.start) as usize
        } else {
            1
        };
        let lbl_underline_len = lbl_span_len
            .min(lbl_source_line.len().saturating_sub(lbl_col - 1))
            .max(1);

        out.push_str(&format!(
            "{:>width$} | {}\n",
            lbl_line,
            lbl_source_line,
            width = gutter_width + 1
        ));
        out.push_str(&format!(
            "{} | {}{} {}\n",
            " ".repeat(gutter_width + 1),
            " ".repeat(lbl_col - 1),
            "^".repeat(lbl_underline_len),
            label.message
        ));
    }

    // Suggested fix
    if let Some(ref fix) = diag.suggested_fix {
        out.push_str(&format!(
            "   hint: {}\n",
            fix.explanation
        ));
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
        .with_label(
            Span::new(file_id, 41, 46),
            "int64 + string is not allowed",
        )
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
}
