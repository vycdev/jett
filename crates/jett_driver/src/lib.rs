use jett_common::FileId;
use jett_fmt::{format_source, FormatResult};
use std::fs;
use std::path::Path;

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
