use jett_common::{FileId, STDLIB_FILE_ID_START};
use jett_diagnostics::Diagnostic;
use jett_parser::ast::Module;
use salsa::Setter as _;
use std::collections::HashMap;
use std::fmt;
use std::sync::{Arc, Mutex};

/// The authority that supplied a source file.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum SourceOrigin {
    Project,
    Dependency(String),
    Stdlib,
}

/// Stable source identity within a query database.
///
/// The unsafe lifetime adapter is bounded by `QueryDatabase`: interned slots are
/// never reclaimed, and handles are not serialized or accepted by another
/// database.
#[salsa::interned(unsafe(no_lifetime), revisions = usize::MAX, debug)]
pub struct FileKey {
    #[returns(clone)]
    pub origin: SourceOrigin,
    #[returns(deref)]
    pub logical_path: String,
}

/// Stable identity for project-level query inputs.
#[salsa::interned(unsafe(no_lifetime), revisions = usize::MAX)]
pub struct ProjectKey {
    #[returns(deref)]
    pub name: String,
}

/// Stable identity for compiler-shipped standard-library inputs.
#[salsa::interned(unsafe(no_lifetime), revisions = usize::MAX)]
pub struct StdlibKey {
    #[returns(deref)]
    pub version: String,
}

/// Mutable text and diagnostic handle for one logical source file.
#[salsa::input]
pub struct SourceFile {
    #[returns(copy)]
    pub key: FileKey,
    #[returns(copy)]
    pub file_id: FileId,
    #[returns(clone)]
    pub text: Arc<str>,
}

/// Project-level inputs that must not become dependencies of `parse_file`.
#[salsa::input]
pub struct ProjectManifest {
    #[returns(copy)]
    pub project: ProjectKey,
    #[returns(clone)]
    pub files: Vec<SourceFile>,
    #[returns(clone)]
    pub config: Arc<str>,
}

/// Immutable owned output of the whole-file parse query.
#[derive(Debug, Clone)]
pub struct ParsedFile {
    pub key: FileKey,
    pub file_id: FileId,
    pub module: Module,
    pub diagnostics: Vec<Diagnostic>,
}

#[salsa::db]
pub trait Db: salsa::Database {
    fn record_parse_execution(&self, key: FileKey);
}

/// Memoized whole-file parsing. Item-level semantic incrementality is
/// deliberately outside this first query boundary.
#[salsa::tracked(returns(clone), no_eq)]
pub fn parse_file(db: &dyn Db, source: SourceFile) -> Arc<ParsedFile> {
    let key = source.key(db);
    db.record_parse_execution(key);
    let parsed = jett_parser::parse(&source.text(db), source.file_id(db));
    Arc::new(ParsedFile {
        key,
        file_id: source.file_id(db),
        module: parsed.module,
        diagnostics: parsed.errors,
    })
}

/// Manifest-dependent orchestration over independently memoized file parses.
#[salsa::tracked(returns(clone), no_eq)]
pub fn parse_project(db: &dyn Db, manifest: ProjectManifest) -> Vec<Arc<ParsedFile>> {
    // Read project configuration at the orchestration boundary. It may affect
    // later project policy, but it must not invalidate any individual parse.
    let _ = manifest.project(db);
    let _ = manifest.config(db);
    manifest
        .files(db)
        .into_iter()
        .map(|file| parse_file(db, file))
        .collect()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FileRegistryError {
    AbsolutePath(String),
    EscapesOrigin(String),
    EmptyPath,
    FileIdSpaceExhausted(SourceOrigin),
}

impl fmt::Display for FileRegistryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AbsolutePath(path) => {
                write!(formatter, "logical source path must be relative: {path}")
            }
            Self::EscapesOrigin(path) => {
                write!(formatter, "logical source path escapes its origin: {path}")
            }
            Self::EmptyPath => write!(formatter, "logical source path must not be empty"),
            Self::FileIdSpaceExhausted(origin) => {
                write!(
                    formatter,
                    "diagnostic file-id space exhausted for {origin:?}"
                )
            }
        }
    }
}

impl std::error::Error for FileRegistryError {}

/// Normalize a logical source path without consulting the ambient filesystem.
pub fn normalize_logical_path(path: &str) -> Result<String, FileRegistryError> {
    let path = path.replace('\\', "/");
    if path.starts_with('/') || path.as_bytes().get(1) == Some(&b':') {
        return Err(FileRegistryError::AbsolutePath(path));
    }

    let mut components = Vec::new();
    for component in path.split('/') {
        match component {
            "" | "." => {}
            ".." => return Err(FileRegistryError::EscapesOrigin(path)),
            component => components.push(component),
        }
    }
    if components.is_empty() {
        return Err(FileRegistryError::EmptyPath);
    }
    Ok(components.join("/"))
}

#[salsa::db]
pub struct QueryDatabase {
    storage: salsa::Storage<Self>,
    files: HashMap<FileKey, SourceFile>,
    next_project_file_id: u32,
    next_stdlib_file_id: u32,
    parse_executions: Arc<Mutex<HashMap<FileKey, usize>>>,
}

impl Default for QueryDatabase {
    fn default() -> Self {
        Self {
            storage: salsa::Storage::default(),
            files: HashMap::new(),
            next_project_file_id: 0,
            next_stdlib_file_id: STDLIB_FILE_ID_START,
            parse_executions: Arc::default(),
        }
    }
}

impl QueryDatabase {
    /// Insert a logical source or update the existing input while preserving its
    /// interned key and diagnostic handle.
    pub fn upsert_source(
        &mut self,
        origin: SourceOrigin,
        logical_path: &str,
        text: impl Into<Arc<str>>,
    ) -> Result<SourceFile, FileRegistryError> {
        let logical_path = normalize_logical_path(logical_path)?;
        let key = FileKey::new(self, origin.clone(), logical_path);
        let text = text.into();
        if let Some(source) = self.files.get(&key).copied() {
            if source.text(self) != text {
                source.set_text(self).to(text);
            }
            return Ok(source);
        }

        let file_id = self.allocate_file_id(&origin)?;
        let source = SourceFile::new(self, key, file_id, text);
        self.files.insert(key, source);
        Ok(source)
    }

    pub fn source(&self, key: FileKey) -> Option<SourceFile> {
        self.files.get(&key).copied()
    }

    pub fn parse(&self, source: SourceFile) -> Arc<ParsedFile> {
        parse_file(self, source)
    }

    /// Number of actual executions, excluding Salsa memo reuse.
    pub fn parse_execution_count(&self, key: FileKey) -> usize {
        self.parse_executions
            .lock()
            .expect("parse execution observer poisoned")
            .get(&key)
            .copied()
            .unwrap_or(0)
    }

    fn allocate_file_id(&mut self, origin: &SourceOrigin) -> Result<FileId, FileRegistryError> {
        let next = match origin {
            SourceOrigin::Stdlib => &mut self.next_stdlib_file_id,
            SourceOrigin::Project | SourceOrigin::Dependency(_) => &mut self.next_project_file_id,
        };
        if matches!(origin, SourceOrigin::Stdlib) {
            let file_id = FileId::new(*next);
            *next = next
                .checked_add(1)
                .ok_or_else(|| FileRegistryError::FileIdSpaceExhausted(origin.clone()))?;
            Ok(file_id)
        } else if *next < STDLIB_FILE_ID_START {
            let file_id = FileId::new(*next);
            *next += 1;
            Ok(file_id)
        } else {
            Err(FileRegistryError::FileIdSpaceExhausted(origin.clone()))
        }
    }
}

#[salsa::db]
impl salsa::Database for QueryDatabase {}

#[salsa::db]
impl Db for QueryDatabase {
    fn record_parse_execution(&self, key: FileKey) {
        *self
            .parse_executions
            .lock()
            .expect("parse execution observer poisoned")
            .entry(key)
            .or_default() += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn source(db: &mut QueryDatabase, path: &str, text: &str) -> SourceFile {
        db.upsert_source(SourceOrigin::Project, path, Arc::<str>::from(text))
            .unwrap()
    }

    #[test]
    fn normalizes_relative_logical_paths() {
        assert_eq!(
            normalize_logical_path("src/./api\\main.jett").unwrap(),
            "src/api/main.jett"
        );
        assert!(normalize_logical_path("../main.jett").is_err());
        assert!(normalize_logical_path("/tmp/main.jett").is_err());
        assert!(normalize_logical_path("C:\\tmp\\main.jett").is_err());
    }

    #[test]
    fn unchanged_requests_execute_parse_once() {
        let mut db = QueryDatabase::default();
        let file = source(
            &mut db,
            "main.jett",
            "function main():\n    return nothing\n",
        );
        let first = db.parse(file);
        let second = db.parse(file);
        assert!(Arc::ptr_eq(&first, &second));
        assert_eq!(db.parse_execution_count(file.key(&db)), 1);
    }

    #[test]
    fn editing_one_file_does_not_reparse_another() {
        let mut db = QueryDatabase::default();
        let file_a = source(&mut db, "a.jett", "function a():\n    return nothing\n");
        let file_b = source(&mut db, "b.jett", "function b():\n    return nothing\n");
        db.parse(file_a);
        db.parse(file_b);

        let same_a = source(&mut db, "a.jett", "function a():\n    return 1\n");
        db.parse(same_a);
        db.parse(file_b);
        assert!(file_a == same_a);
        assert_eq!(db.parse_execution_count(file_a.key(&db)), 2);
        assert_eq!(db.parse_execution_count(file_b.key(&db)), 1);
    }

    #[test]
    fn manifest_and_configuration_changes_reuse_file_parses() {
        let mut db = QueryDatabase::default();
        let project = ProjectKey::new(&db, "demo");
        let file_a = source(&mut db, "a.jett", "function a():\n    return nothing\n");
        let manifest = ProjectManifest::new(&db, project, vec![file_a], Arc::from("debug"));
        parse_project(&db, manifest);

        let file_b = source(&mut db, "b.jett", "function b():\n    return nothing\n");
        manifest.set_files(&mut db).to(vec![file_a, file_b]);
        parse_project(&db, manifest);
        manifest.set_config(&mut db).to(Arc::from("release"));
        parse_project(&db, manifest);
        manifest.set_files(&mut db).to(vec![file_a]);
        parse_project(&db, manifest);

        assert_eq!(db.parse_execution_count(file_a.key(&db)), 1);
        assert_eq!(db.parse_execution_count(file_b.key(&db)), 1);
    }

    #[test]
    fn equal_paths_from_different_origins_have_distinct_identity() {
        let mut db = QueryDatabase::default();
        let project = source(&mut db, "src/main.jett", "");
        let dependency = db
            .upsert_source(
                SourceOrigin::Dependency("vendor/example".into()),
                "src/main.jett",
                Arc::from(""),
            )
            .unwrap();
        let stdlib = db
            .upsert_source(SourceOrigin::Stdlib, "src/main.jett", Arc::from(""))
            .unwrap();

        assert_ne!(project.key(&db), dependency.key(&db));
        assert_ne!(dependency.key(&db), stdlib.key(&db));
        assert_eq!(project.file_id(&db).index(), 0);
        assert_eq!(dependency.file_id(&db).index(), 1);
        assert_eq!(stdlib.file_id(&db).index(), STDLIB_FILE_ID_START);
    }

    #[test]
    fn stdlib_edit_does_not_reparse_project_source() {
        let mut db = QueryDatabase::default();
        let project = source(
            &mut db,
            "main.jett",
            "function main():\n    return nothing\n",
        );
        let stdlib = db
            .upsert_source(
                SourceOrigin::Stdlib,
                "json/01_tree.jett",
                Arc::from("namespace json\n"),
            )
            .unwrap();
        db.parse(project);
        db.parse(stdlib);

        let same_stdlib = db
            .upsert_source(
                SourceOrigin::Stdlib,
                "json/01_tree.jett",
                Arc::from("namespace json\nexport function tree():\n    return nothing\n"),
            )
            .unwrap();
        db.parse(same_stdlib);
        db.parse(project);

        assert_eq!(db.parse_execution_count(project.key(&db)), 1);
        assert_eq!(db.parse_execution_count(stdlib.key(&db)), 2);
    }

    #[test]
    fn malformed_non_entry_file_retains_its_diagnostic_handle() {
        let mut db = QueryDatabase::default();
        let _entry = source(
            &mut db,
            "main.jett",
            "function main():\n    return nothing\n",
        );
        let broken = source(&mut db, "broken.jett", "function broken(");
        let parsed = db.parse(broken);
        assert!(!parsed.diagnostics.is_empty());
        assert!(parsed.diagnostics.iter().all(|diagnostic| {
            diagnostic.span.file == parsed.file_id
                && diagnostic
                    .labels
                    .iter()
                    .all(|label| label.span.file == parsed.file_id)
        }));
        assert_eq!(broken.file_id(&db).index(), 1);
    }

    #[test]
    fn adding_a_file_preserves_existing_keys_and_handles() {
        let mut db = QueryDatabase::default();
        let original = source(&mut db, "z.jett", "function z():\n    return nothing\n");
        let original_key = original.key(&db);
        let original_id = original.file_id(&db);
        db.parse(original);

        let _earlier = source(&mut db, "a.jett", "function a():\n    return nothing\n");
        let same = source(&mut db, "z.jett", "function z():\n    return nothing\n");
        db.parse(same);
        assert_eq!(same.key(&db), original_key);
        assert_eq!(same.file_id(&db), original_id);
        assert_eq!(db.parse_execution_count(original_key), 1);
    }

    #[test]
    fn parse_diagnostic_order_is_stable_across_databases() {
        fn diagnostic_signature() -> Vec<(u16, String, u32, u32)> {
            let mut db = QueryDatabase::default();
            let broken = source(&mut db, "broken.jett", "function broken(\n@");
            db.parse(broken)
                .diagnostics
                .iter()
                .map(|diagnostic| {
                    (
                        diagnostic.code.code(),
                        diagnostic.message.clone(),
                        diagnostic.span.start,
                        diagnostic.span.end,
                    )
                })
                .collect()
        }

        let first = diagnostic_signature();
        assert!(!first.is_empty());
        assert_eq!(first, diagnostic_signature());
    }
}
