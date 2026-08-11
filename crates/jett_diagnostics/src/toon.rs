use crate::render::line_col;
use crate::{Diagnostic, Severity};
use jett_common::{FileId, Span};

/// Source text and display path for one file in a multi-file diagnostic payload.
#[derive(Debug, Clone, Copy)]
pub struct ToonSource<'a> {
    pub file_id: FileId,
    pub source: &'a str,
    pub file_path: &'a str,
}

/// Render a slice of diagnostics as a TOON agent payload.
///
/// Output format (matching the ASP spec from the design doc):
/// ```text
/// status: error
/// file: src/file.jett
/// total: N
/// errors: N
/// warnings: N
/// infos: N
/// diagnostics[N]{code,severity,message,file,line,column,end_line,end_column}:
///   E0012,error,message here,src/file.jett,23,41,23,50
/// ```
///
/// When there are no errors, outputs:
/// ```text
/// status: ok
/// file: src/file.jett
/// total: 0
/// errors: 0
/// warnings: 0
/// infos: 0
/// diagnostics[0]{code,severity,message,file,line,column,end_line,end_column}:
/// ```
pub fn render_toon(diagnostics: &[Diagnostic], source: &str, file_path: &str) -> String {
    render_toon_inner(
        diagnostics,
        source,
        file_path,
        |_| Some((source, file_path)),
        |_| true,
    )
}

/// Render diagnostics whose spans may refer to more than one source file.
///
/// Diagnostic and label locations are resolved against the source matching
/// each span's file id. Suggested fixes retain the existing single-file schema,
/// so fixes outside `primary_file` are omitted rather than attributed to the
/// wrong file.
pub fn render_toon_with_sources(
    diagnostics: &[Diagnostic],
    primary_file: FileId,
    sources: &[ToonSource<'_>],
) -> String {
    let primary_source = sources.iter().find(|source| source.file_id == primary_file);
    let source = primary_source.map_or("", |source| source.source);
    let file_path = primary_source.map_or("unknown", |source| source.file_path);

    render_toon_inner(
        diagnostics,
        source,
        file_path,
        |file_id| {
            sources
                .iter()
                .find(|source| source.file_id == file_id)
                .map(|source| (source.source, source.file_path))
        },
        |span| span.file == primary_file,
    )
}

fn render_toon_inner<'a>(
    diagnostics: &[Diagnostic],
    source: &'a str,
    file_path: &'a str,
    source_for_file: impl Fn(FileId) -> Option<(&'a str, &'a str)>,
    include_fix: impl Fn(Span) -> bool,
) -> String {
    let error_count = diagnostics
        .iter()
        .filter(|d| d.severity == Severity::Error)
        .count();
    let warning_count = diagnostics
        .iter()
        .filter(|d| d.severity == Severity::Warning)
        .count();
    let info_count = diagnostics
        .iter()
        .filter(|d| d.severity == Severity::Info)
        .count();

    let mut out = String::new();
    if error_count > 0 {
        out.push_str("status: error\n");
    } else {
        out.push_str("status: ok\n");
    }
    out.push_str(&format!("file: {}\n", escape_toon_scalar(file_path)));

    let count = diagnostics.len();
    out.push_str(&format!("total: {}\n", count));
    out.push_str(&format!("errors: {}\n", error_count));
    out.push_str(&format!("warnings: {}\n", warning_count));
    out.push_str(&format!("infos: {}\n", info_count));
    out.push_str(&format!(
        "diagnostics[{}]{{code,severity,message,file,line,column,end_line,end_column}}:\n",
        count
    ));

    for diag in diagnostics {
        let severity_str = match diag.severity {
            Severity::Error => "error",
            Severity::Warning => "warning",
            Severity::Info => "info",
        };

        let (span_file_path, line, col, end_line, end_col) =
            if let Some((span_source, span_file_path)) = source_for_file(diag.span.file) {
                let (line, col) = line_col(span_source, diag.span.start);
                let (end_line, end_col) = line_col(span_source, diag.span.end);
                (span_file_path, line, col, end_line, end_col)
            } else {
                ("unknown", 0, 0, 0, 0)
            };

        out.push_str(&format!(
            "  {},{},{},{},{},{},{},{}\n",
            diag.code,
            severity_str,
            escape_toon_scalar(&diag.message),
            escape_toon_scalar(span_file_path),
            line,
            col,
            end_line,
            end_col
        ));
    }

    let label_count: usize = diagnostics.iter().map(|diag| diag.labels.len()).sum();
    out.push_str(&format!(
        "labels[{}]{{code,message,file,line,column,end_line,end_column}}:\n",
        label_count
    ));
    for diag in diagnostics {
        for label in &diag.labels {
            let (label_file_path, line, col, end_line, end_col) =
                if let Some((label_source, label_file_path)) = source_for_file(label.span.file) {
                    let (line, col) = line_col(label_source, label.span.start);
                    let (end_line, end_col) = line_col(label_source, label.span.end);
                    (label_file_path, line, col, end_line, end_col)
                } else {
                    ("unknown", 0, 0, 0, 0)
                };
            out.push_str(&format!(
                "  {},{},{},{},{},{},{}\n",
                diag.code,
                escape_toon_scalar(&label.message),
                escape_toon_scalar(label_file_path),
                line,
                col,
                end_line,
                end_col
            ));
        }
    }

    let fix_count = diagnostics
        .iter()
        .filter(|diag| {
            diag.suggested_fix
                .as_ref()
                .is_some_and(|fix| include_fix(fix.span))
        })
        .count();
    out.push_str(&format!(
        "suggested_fixes[{}]{{code,line,column,old_text,new_text,explanation}}:\n",
        fix_count
    ));
    for diag in diagnostics {
        if let Some(ref fix) = diag.suggested_fix {
            if !include_fix(fix.span) {
                continue;
            }
            let fix_source = source_for_file(fix.span.file)
                .map(|(source, _)| source)
                .unwrap_or(source);
            let (fix_line, fix_col) = line_col(fix_source, fix.span.start);
            out.push_str(&format!(
                "  {},{},{},{},{},{}\n",
                diag.code,
                fix_line,
                fix_col,
                escape_toon_scalar(&fix.old_text),
                escape_toon_scalar(&fix.new_text),
                escape_toon_scalar(&fix.explanation)
            ));
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
            "status: ok\nfile: test.jett\ntotal: 0\nerrors: 0\nwarnings: 0\ninfos: 0\ndiagnostics[0]{code,severity,message,file,line,column,end_line,end_column}:\nlabels[0]{code,message,file,line,column,end_line,end_column}:\nsuggested_fixes[0]{code,line,column,old_text,new_text,explanation}:\n"
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
            "status: ok\nfile: test.jett\ntotal: 1\nerrors: 0\nwarnings: 1\ninfos: 0\ndiagnostics[1]{code,severity,message,file,line,column,end_line,end_column}:\n  E0100,warning,unused variable,test.jett,2,10,2,11\nlabels[0]{code,message,file,line,column,end_line,end_column}:\nsuggested_fixes[0]{code,line,column,old_text,new_text,explanation}:\n"
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
        assert!(result.contains("file: test.jett\n"));
        assert!(result.contains("total: 1\n"));
        assert!(result.contains("errors: 1\n"));
        assert!(result.contains("warnings: 0\n"));
        assert!(result.contains("infos: 0\n"));
        assert!(result.contains(
            "diagnostics[1]{code,severity,message,file,line,column,end_line,end_column}:"
        ));
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

        assert!(
            result.contains("suggested_fixes[1]{code,line,column,old_text,new_text,explanation}:")
        );
        assert!(
            result.contains("E0300,2,11,a + b,a + int64.from_string(b),use explicit conversion")
        );
    }

    #[test]
    fn toon_error_with_label() {
        let source = "first\nsecond\n";
        let file_id = FileId::new(0);
        let diags = vec![
            Diagnostic::error(201, "duplicate definition", Span::new(file_id, 6, 12))
                .with_label(Span::new(file_id, 0, 5), "original definition"),
        ];

        let result = render_toon(&diags, source, "test.jett");

        assert!(result.contains("labels[1]{code,message,file,line,column,end_line,end_column}:"));
        assert!(result.contains("E0201,original definition,test.jett,1,1,1,6"));
    }

    #[test]
    fn toon_multi_file_locations_use_their_matching_sources() {
        let requested_file = FileId::new(0);
        let support_file = FileId::new(1);
        let requested_source = "use hidden\n";
        let support_source = "first\nfunction hidden\n";
        let diagnostics = vec![
            Diagnostic::error(207, "private declaration", Span::new(requested_file, 4, 10))
                .with_label(Span::new(support_file, 6, 21), "declared private here")
                .with_fix(
                    Span::new(support_file, 6, 14),
                    "function",
                    "export function",
                    "export the declaration",
                ),
        ];
        let sources = [
            ToonSource {
                file_id: requested_file,
                source: requested_source,
                file_path: "main.jett",
            },
            ToonSource {
                file_id: support_file,
                source: support_source,
                file_path: "support.jett",
            },
        ];

        let result = render_toon_with_sources(&diagnostics, requested_file, &sources);

        assert!(result.contains("E0207,error,private declaration,main.jett,1,5,1,11"));
        assert!(result.contains("E0207,declared private here,support.jett,2,1,2,16"));
        assert!(result.contains("suggested_fixes[0]"));
        assert!(!result.contains("export the declaration"));
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

        assert!(result.contains(
            "diagnostics[2]{code,severity,message,file,line,column,end_line,end_column}:"
        ));
        assert!(result.contains("E0001,error,first error,test.jett,1,1,1,4"));
        assert!(result.contains("E0002,error,second error,test.jett,2,1,2,4"));
    }
}
