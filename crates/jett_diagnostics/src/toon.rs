use crate::render::line_col;
use crate::{Diagnostic, Severity};

/// Render a slice of diagnostics as a TOON agent payload.
///
/// Output format (matching the ASP spec from the design doc):
/// ```text
/// status: error
/// total: N
/// errors: N
/// warnings: N
/// diagnostics[N]{code,severity,message,file,line,column}:
///   E0012,error,message here,src/file.jett,23,41
/// ```
///
/// When there are no errors, outputs:
/// ```text
/// status: ok
/// total: 0
/// errors: 0
/// warnings: 0
/// diagnostics[0]{code,severity,message,file,line,column}:
/// ```
pub fn render_toon(diagnostics: &[Diagnostic], source: &str, file_path: &str) -> String {
    let error_count = diagnostics
        .iter()
        .filter(|d| d.severity == Severity::Error)
        .count();
    let warning_count = diagnostics
        .iter()
        .filter(|d| d.severity == Severity::Warning)
        .count();

    let mut out = String::new();
    if error_count > 0 {
        out.push_str("status: error\n");
    } else {
        out.push_str("status: ok\n");
    }

    let count = diagnostics.len();
    out.push_str(&format!("total: {}\n", count));
    out.push_str(&format!("errors: {}\n", error_count));
    out.push_str(&format!("warnings: {}\n", warning_count));
    out.push_str(&format!(
        "diagnostics[{}]{{code,severity,message,file,line,column}}:\n",
        count
    ));

    for diag in diagnostics {
        let severity_str = match diag.severity {
            Severity::Error => "error",
            Severity::Warning => "warning",
            Severity::Info => "info",
        };

        let (line, col) = line_col(source, diag.span.start);

        out.push_str(&format!(
            "  {},{},{},{},{},{}\n",
            diag.code,
            severity_str,
            escape_toon_scalar(&diag.message),
            escape_toon_scalar(file_path),
            line,
            col
        ));
    }

    // Include suggested fixes if any diagnostic has one
    for diag in diagnostics {
        if let Some(ref fix) = diag.suggested_fix {
            let (fix_line, _) = line_col(source, fix.span.start);
            out.push_str("suggested_fix:\n");
            out.push_str("  action: replace\n");
            out.push_str(&format!("  line: {}\n", fix_line));
            out.push_str(&format!(
                "  old_text: {}\n",
                escape_toon_scalar(&fix.old_text)
            ));
            out.push_str(&format!(
                "  new_text: {}\n",
                escape_toon_scalar(&fix.new_text)
            ));
            out.push_str(&format!(
                "  explanation: {}\n",
                escape_toon_scalar(&fix.explanation)
            ));
            break; // Only include the first suggested fix at the top level
        }
    }

    out
}

fn escape_toon_scalar(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('\r', "\\r")
        .replace('\n', "\\n")
        .replace(',', "\\,")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Diagnostic;
    use jett_common::{FileId, Span};

    #[test]
    fn toon_ok_when_no_diagnostics() {
        let result = render_toon(&[], "", "test.jett");
        assert_eq!(
            result,
            "status: ok\ntotal: 0\nerrors: 0\nwarnings: 0\ndiagnostics[0]{code,severity,message,file,line,column}:\n"
        );
    }

    #[test]
    fn toon_ok_when_only_warnings() {
        let source = "function foo() returns nothing:\n    int64 x = 42\n";
        let file_id = FileId::new(0);
        let diags = vec![Diagnostic::warning(
            100,
            "unused variable",
            Span::new(file_id, 41, 42),
        )];
        let result = render_toon(&diags, source, "test.jett");
        assert_eq!(
            result,
            "status: ok\ntotal: 1\nerrors: 0\nwarnings: 1\ndiagnostics[1]{code,severity,message,file,line,column}:\n  E0100,warning,unused variable,test.jett,2,10\n"
        );
    }

    #[test]
    fn toon_error_single() {
        let source = "function main() returns int64:\n    return a + b\n";
        let file_id = FileId::new(0);
        let diags = vec![Diagnostic::error(
            300,
            "type mismatch: expected int64 got string",
            Span::new(file_id, 41, 46),
        )];

        let result = render_toon(&diags, source, "test.jett");

        assert!(result.starts_with("status: error\n"));
        assert!(result.contains("total: 1\n"));
        assert!(result.contains("errors: 1\n"));
        assert!(result.contains("warnings: 0\n"));
        assert!(result.contains("diagnostics[1]{code,severity,message,file,line,column}:"));
        assert!(
            result.contains("E0300,error,type mismatch: expected int64 got string,test.jett,2,")
        );
    }

    #[test]
    fn toon_error_with_suggested_fix() {
        let source = "function main() returns int64:\n    return a + b\n";
        let file_id = FileId::new(0);
        let diags = vec![
            Diagnostic::error(300, "type mismatch", Span::new(file_id, 41, 46)).with_fix(
                Span::new(file_id, 41, 46),
                "a + b",
                "a + int64.from_string(b)",
                "use explicit conversion",
            ),
        ];

        let result = render_toon(&diags, source, "test.jett");

        assert!(result.contains("suggested_fix:"));
        assert!(result.contains("action: replace"));
        assert!(result.contains("old_text: a + b"));
        assert!(result.contains("new_text: a + int64.from_string(b)"));
        assert!(result.contains("explanation: use explicit conversion"));
    }

    #[test]
    fn toon_multiple_errors() {
        let source = "aaa\nbbb\nccc\n";
        let file_id = FileId::new(0);
        let diags = vec![
            Diagnostic::error(1, "first error", Span::new(file_id, 0, 3)),
            Diagnostic::error(2, "second error", Span::new(file_id, 4, 7)),
        ];

        let result = render_toon(&diags, source, "test.jett");

        assert!(result.contains("diagnostics[2]{code,severity,message,file,line,column}:"));
        assert!(result.contains("E0001,error,first error,test.jett,1,1"));
        assert!(result.contains("E0002,error,second error,test.jett,2,1"));
    }
}
