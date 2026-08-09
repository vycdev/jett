use crate::{NamespaceSpan, Project, SourceFile};
use jett_common::{FileId, SymbolInterner};
use std::fs;
use std::path::{Path, PathBuf};

/// Error returned when project discovery fails.
#[derive(Debug)]
pub enum DiscoverError {
    /// No jett.proj file found walking up from the given path.
    NoProjectFile(PathBuf),
    /// No Jett source files found in the project directory.
    NoSourceFiles(PathBuf),
    /// Failed to read a file.
    IoError(PathBuf, std::io::Error),
    /// The jett.proj file is missing required fields.
    InvalidProjectFile(String),
}

impl std::fmt::Display for DiscoverError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DiscoverError::NoProjectFile(path) => {
                write!(f, "no jett.proj found starting from {}", path.display())
            }
            DiscoverError::NoSourceFiles(path) => {
                write!(f, "no .jett source files found in {}", path.display())
            }
            DiscoverError::IoError(path, err) => {
                write!(f, "failed to read {}: {}", path.display(), err)
            }
            DiscoverError::InvalidProjectFile(msg) => {
                write!(f, "invalid jett.proj: {}", msg)
            }
        }
    }
}

/// Discover a Jett project starting from the given path.
///
/// 1. Walk up from `start_path` to find `jett.proj`.
/// 2. Parse the project file (TOON format: `key: value`).
/// 3. Recursively discover all `.jett` files.
/// 4. Pre-scan each file for namespace declarations.
pub fn discover_project(
    start_path: &Path,
    interner: &mut SymbolInterner,
) -> Result<Project, DiscoverError> {
    let project_dir = find_project_root(start_path)?;
    let proj_path = project_dir.join("jett.proj");

    let proj_content =
        fs::read_to_string(&proj_path).map_err(|e| DiscoverError::IoError(proj_path.clone(), e))?;

    let (name, version, entry) = parse_project_file(&proj_content)?;
    let entry = Path::new(&entry);

    let jett_files = find_jett_files(&project_dir)?;
    if jett_files.is_empty() {
        return Err(DiscoverError::NoSourceFiles(project_dir));
    }

    let mut files = Vec::new();
    let mut entry_file = None;

    for (index, path) in jett_files.iter().enumerate() {
        let id = FileId::new(index as u32);
        let content =
            fs::read_to_string(path).map_err(|e| DiscoverError::IoError(path.clone(), e))?;

        let namespaces = prescan_namespaces(&content, interner);

        if path
            .strip_prefix(&project_dir)
            .is_ok_and(|relative| relative == entry)
        {
            entry_file = Some(id);
        }

        files.push(SourceFile {
            id,
            path: path.clone(),
            content,
            namespaces,
        });
    }

    let entry_file = entry_file.unwrap_or_else(|| {
        // If no entry file matched, default to first file
        FileId::new(0)
    });

    Ok(Project {
        name,
        version,
        entry_file,
        files,
    })
}

/// Walk up from `start_path` to find a directory containing `jett.proj`.
fn find_project_root(start_path: &Path) -> Result<PathBuf, DiscoverError> {
    let start = if start_path.is_file() {
        start_path.parent().unwrap_or(start_path).to_path_buf()
    } else {
        start_path.to_path_buf()
    };

    let mut current = start.as_path();
    loop {
        if current.join("jett.proj").exists() {
            return Ok(current.to_path_buf());
        }
        match current.parent() {
            Some(parent) => current = parent,
            None => {
                return Err(DiscoverError::NoProjectFile(start));
            }
        }
    }
}

/// Parse a TOON-format project file.
/// Expected format:
/// ```text
/// name: myproject
/// version: 0.1.0
/// entry: src/main.jett
/// ```
fn parse_project_file(content: &str) -> Result<(String, String, String), DiscoverError> {
    let mut name = None;
    let mut version = None;
    let mut entry = None;

    for line in content.split(|character| character == '\n' || character == '\r') {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some((key, value)) = line.split_once(':') {
            let key = key.trim();
            let value = value.trim();
            match key {
                "name" => name = Some(value.to_string()),
                "version" => version = Some(value.to_string()),
                "entry" => entry = Some(value.to_string()),
                _ => {} // ignore unknown keys
            }
        }
    }

    let name =
        name.ok_or_else(|| DiscoverError::InvalidProjectFile("missing 'name' field".to_string()))?;
    let version = version.unwrap_or_else(|| "0.1.0".to_string());
    let entry = entry.unwrap_or_else(|| "src/main.jett".to_string());

    Ok((name, version, entry))
}

/// Recursively find all `.jett` files in a directory.
fn find_jett_files(dir: &Path) -> Result<Vec<PathBuf>, DiscoverError> {
    let mut files = Vec::new();
    collect_jett_files(dir, &mut files)?;
    files.sort(); // deterministic order
    Ok(files)
}

fn collect_jett_files(dir: &Path, files: &mut Vec<PathBuf>) -> Result<(), DiscoverError> {
    let entries = fs::read_dir(dir).map_err(|e| DiscoverError::IoError(dir.to_path_buf(), e))?;

    for entry in entries {
        let entry = entry.map_err(|e| DiscoverError::IoError(dir.to_path_buf(), e))?;
        let path = entry.path();
        let file_type = entry
            .file_type()
            .map_err(|e| DiscoverError::IoError(path.clone(), e))?;

        if file_type.is_symlink() {
            // Source-file symlinks are supported, but following directory
            // symlinks could recursively pull in linked trees or create a cycle.
            if path.is_file() && path.extension().and_then(|e| e.to_str()) == Some("jett") {
                files.push(path);
            }
        } else if file_type.is_dir() {
            // Skip hidden directories and target/
            let dir_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if !dir_name.starts_with('.') && dir_name != "target" {
                collect_jett_files(&path, files)?;
            }
        } else if path.extension().and_then(|e| e.to_str()) == Some("jett") {
            files.push(path);
        }
    }

    Ok(())
}

/// Quick scan of a file to extract `namespace` declarations.
/// Looks for lines matching `namespace <name>` at the start of a line.
fn prescan_namespaces(content: &str, interner: &mut SymbolInterner) -> Vec<NamespaceSpan> {
    let mut namespaces = Vec::new();

    for (byte_offset, line) in line_offsets(content) {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("namespace ") {
            let name = rest.trim();
            if !name.is_empty() && !name.contains(' ') {
                namespaces.push(NamespaceSpan {
                    name: interner.intern(name),
                    byte_offset: byte_offset as u32,
                });
            }
        }
    }

    namespaces
}

/// Iterate over lines with their byte offsets.
fn line_offsets(content: &str) -> impl Iterator<Item = (usize, &str)> {
    content.split_inclusive('\n').scan(0, |offset, line| {
        let start = *offset;
        *offset += line.len();
        let line = if let Some(line) = line.strip_suffix('\n') {
            line.strip_suffix('\r').unwrap_or(line)
        } else {
            line
        };
        Some((start, line))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn parse_project_file_basic() {
        let content = "name: myproject\nversion: 0.1.0\nentry: src/main.jett\n";
        let (name, version, entry) = parse_project_file(content).unwrap();
        assert_eq!(name, "myproject");
        assert_eq!(version, "0.1.0");
        assert_eq!(entry, "src/main.jett");
    }

    #[test]
    fn parse_project_file_with_comments() {
        let content = "# Project config\nname: test\n# comment\nversion: 1.0.0\n";
        let (name, version, _) = parse_project_file(content).unwrap();
        assert_eq!(name, "test");
        assert_eq!(version, "1.0.0");
    }

    #[test]
    fn parse_project_file_accepts_lone_carriage_return_lines() {
        let content = "name: test\rversion: 1.0.0\rentry: src/main.jett\r";
        let (name, version, entry) = parse_project_file(content).unwrap();
        assert_eq!(name, "test");
        assert_eq!(version, "1.0.0");
        assert_eq!(entry, "src/main.jett");
    }

    #[test]
    fn parse_project_file_missing_name() {
        let content = "version: 1.0.0\n";
        assert!(parse_project_file(content).is_err());
    }

    #[test]
    fn prescan_namespaces_basic() {
        let mut interner = SymbolInterner::new();
        let content = "namespace auth\n\nfunction login():\n    return true\n";
        let ns = prescan_namespaces(content, &mut interner);
        assert_eq!(ns.len(), 1);
        assert_eq!(interner.resolve(ns[0].name), "auth");
        assert_eq!(ns[0].byte_offset, 0);
    }

    #[test]
    fn prescan_namespaces_multiple() {
        let mut interner = SymbolInterner::new();
        let content = "namespace models\n\nstruct User:\n    name: string\n\nnamespace auth\n\nfunction login():\n    return true\n";
        let ns = prescan_namespaces(content, &mut interner);
        assert_eq!(ns.len(), 2);
        assert_eq!(interner.resolve(ns[0].name), "models");
        assert_eq!(interner.resolve(ns[1].name), "auth");
    }

    #[test]
    fn prescan_namespaces_tracks_crlf_byte_offsets() {
        let mut interner = SymbolInterner::new();
        let content =
            "namespace models\r\n\r\nstruct User:\r\n    name: string\r\n\r\nnamespace auth\r\n";
        let ns = prescan_namespaces(content, &mut interner);
        assert_eq!(ns.len(), 2);
        assert_eq!(ns[0].byte_offset, 0);
        assert_eq!(
            ns[1].byte_offset as usize,
            content.find("namespace auth").unwrap()
        );
    }

    #[test]
    fn prescan_namespaces_dotted() {
        let mut interner = SymbolInterner::new();
        let content = "namespace net.http.server\n";
        let ns = prescan_namespaces(content, &mut interner);
        assert_eq!(ns.len(), 1);
        assert_eq!(interner.resolve(ns[0].name), "net.http.server");
    }

    #[test]
    fn discover_project_integration() {
        // Create a temporary project
        let tmp = std::env::temp_dir().join("jett_test_project");
        let _ = fs::remove_dir_all(&tmp);
        fs::create_dir_all(tmp.join("src")).unwrap();
        fs::create_dir_all(tmp.join("vendor/src")).unwrap();

        fs::write(
            tmp.join("jett.proj"),
            "name: testproject\nversion: 0.1.0\nentry: src/main.jett\n",
        )
        .unwrap();
        fs::write(
            tmp.join("src/main.jett"),
            "namespace app\n\nfunction main(stdout: Stdout) returns nothing:\n    Stdout.write(view stdout, \"hello\")\n",
        )
        .unwrap();
        fs::write(
            tmp.join("vendor/src/main.jett"),
            "namespace vendor\n\nfunction helper() returns nothing:\n    return nothing\n",
        )
        .unwrap();

        let mut interner = SymbolInterner::new();
        let project = discover_project(&tmp, &mut interner).unwrap();

        assert_eq!(project.name, "testproject");
        assert_eq!(project.version, "0.1.0");
        assert_eq!(project.files.len(), 2);
        assert_eq!(
            project.files[project.entry_file.index() as usize].path,
            tmp.join("src/main.jett")
        );
        assert_eq!(project.files[0].namespaces.len(), 1);
        assert_eq!(interner.resolve(project.files[0].namespaces[0].name), "app");

        // Cleanup
        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn discover_project_rejects_project_without_source_files() {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system time should be after the Unix epoch")
            .as_nanos();
        let tmp = std::env::temp_dir().join(format!("jett_empty_project_{nanos}"));
        fs::create_dir_all(&tmp).unwrap();
        fs::write(tmp.join("jett.proj"), "name: empty\n").unwrap();

        let mut interner = SymbolInterner::new();
        let error = discover_project(&tmp, &mut interner).unwrap_err();

        let _ = fs::remove_dir_all(&tmp);
        match error {
            DiscoverError::NoSourceFiles(path) => assert_eq!(path, tmp),
            other => panic!("expected no-source-files error, got {other}"),
        }
    }

    #[cfg(unix)]
    #[test]
    fn discover_project_ignores_symlinked_source_directories() {
        use std::os::unix::fs::symlink;

        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system time should be after the Unix epoch")
            .as_nanos();
        let tmp = std::env::temp_dir().join(format!("jett_symlink_project_{nanos}"));
        let external = std::env::temp_dir().join(format!("jett_symlink_external_{nanos}"));
        fs::create_dir_all(tmp.join("src")).unwrap();
        fs::create_dir_all(&external).unwrap();
        fs::write(tmp.join("jett.proj"), "name: symlinked\n").unwrap();
        fs::write(tmp.join("src/main.jett"), "namespace app\n").unwrap();
        fs::write(external.join("outside.jett"), "namespace outside\n").unwrap();
        symlink(&external, tmp.join("linked")).unwrap();

        let mut interner = SymbolInterner::new();
        let project = discover_project(&tmp, &mut interner).unwrap();

        let _ = fs::remove_dir_all(&tmp);
        let _ = fs::remove_dir_all(&external);
        assert_eq!(project.files.len(), 1);
        assert_eq!(project.files[0].path, tmp.join("src/main.jett"));
    }
}
