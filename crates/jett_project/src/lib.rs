use jett_common::{FileId, Symbol};
use std::path::PathBuf;

/// A namespace declaration found during pre-scan.
#[derive(Debug, Clone)]
pub struct NamespaceSpan {
    pub name: Symbol,
    pub byte_offset: u32,
}

/// A source file in the project.
#[derive(Debug)]
pub struct SourceFile {
    pub id: FileId,
    pub path: PathBuf,
    pub content: String,
    pub namespaces: Vec<NamespaceSpan>,
}

/// The parsed project metadata and all discovered source files.
#[derive(Debug)]
pub struct Project {
    pub name: String,
    pub version: String,
    pub entry_file: FileId,
    pub files: Vec<SourceFile>,
}
