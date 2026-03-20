use jett_common::FileId;
use jett_diagnostics::Diagnostic;
use jett_fmt::{format_source, FormatResult};
use jett_parser::parse;
use jett_resolve::resolve;
use jett_typecheck::check;
use std::fs;
use std::path::Path;

/// Result of compiling a single file.
pub struct BuildResult {
    pub diagnostics: Vec<Diagnostic>,
    pub has_errors: bool,
}

/// Run the full compilation pipeline on a single file: lex → parse → resolve → typecheck.
/// Does not produce executable output yet — just validates the source.
pub fn build_file(path: &Path) -> BuildResult {
    let source = match fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) => {
            return BuildResult {
                diagnostics: vec![Diagnostic::error(
                    0,
                    format!("failed to read {}: {}", path.display(), e),
                    jett_common::Span::new(FileId::new(0), 0, 0),
                )],
                has_errors: true,
            };
        }
    };

    let file_id = FileId::new(0);
    let mut all_diagnostics = Vec::new();

    // Phase 1+2: Lex + Parse (parse internally calls tokenize)
    let parse_result = parse(&source, file_id);
    all_diagnostics.extend(parse_result.errors.clone());

    // If there are parse errors, stop here — resolve/typecheck won't produce useful results
    let has_parse_errors = all_diagnostics.iter().any(|d| d.severity == jett_diagnostics::Severity::Error);
    if has_parse_errors {
        return BuildResult {
            has_errors: true,
            diagnostics: all_diagnostics,
        };
    }

    // Phase 3: Resolve names
    let resolve_result = resolve(&parse_result.module);
    all_diagnostics.extend(resolve_result.diagnostics.clone());

    let has_resolve_errors = all_diagnostics.iter().any(|d| d.severity == jett_diagnostics::Severity::Error);
    if has_resolve_errors {
        return BuildResult {
            has_errors: true,
            diagnostics: all_diagnostics,
        };
    }

    // Phase 4: Type check
    let check_result = check(&parse_result.module, &resolve_result);
    all_diagnostics.extend(check_result.diagnostics.clone());

    let has_errors = all_diagnostics.iter().any(|d| d.severity == jett_diagnostics::Severity::Error);

    BuildResult {
        has_errors,
        diagnostics: all_diagnostics,
    }
}

/// Format a single .jett file and return the formatted source.
pub fn format_file(path: &Path) -> Result<FormatResult, String> {
    let source = fs::read_to_string(path)
        .map_err(|e| format!("failed to read {}: {}", path.display(), e))?;

    let file_id = FileId::new(0);
    Ok(format_source(&source, file_id))
}

/// Format a .jett file in place (overwrite with formatted version).
pub fn format_file_in_place(path: &Path) -> Result<(), String> {
    let result = format_file(path)?;

    if !result.errors.is_empty() {
        return Err(format!(
            "cannot format {} — lexer errors:\n{}",
            path.display(),
            result.errors.join("\n")
        ));
    }

    fs::write(path, &result.output)
        .map_err(|e| format!("failed to write {}: {}", path.display(), e))?;

    Ok(())
}
