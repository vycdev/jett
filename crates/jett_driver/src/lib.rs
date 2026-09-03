use jett_common::{FileId, STDLIB_FILE_ID_START, Span};
use jett_comptime::evaluate_explicit_comptime_expressions;
use jett_comptime::value::Value;
use jett_comptime::verify::{
    run_verify_blocks_detailed_with_metadata_and_expression_types,
    run_verify_blocks_with_metadata_and_expression_types,
};
pub use jett_comptime::{
    ClockTestSample, EnvironmentTestEntry, EnvironmentTestSnapshot, EnvironmentTestText,
    RandomTestSample,
};
use jett_diagnostics::Diagnostic;
use jett_fmt::{FormatResult, format_source};
use jett_parser::ast::{FunctionDecl, FunctionDef, Item, Module, Param, TypeExpr};
use jett_parser::{ParseResult, parse};
use jett_query::{QueryDatabase, SourceOrigin};
use jett_resolve::resolve;
use jett_typecheck::{CheckOptions, CheckResult, check, check_with_options};
use jett_types::ReflectionMetadata;
use std::borrow::Cow;
use std::collections::{BTreeSet, HashMap, HashSet};
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;
use std::thread;

const RUNTIME_STACK_SIZE: usize = 8 * 1024 * 1024;

struct DiscoveredModules {
    modules: Vec<Module>,
    diagnostics: Vec<Diagnostic>,
    files: HashMap<FileId, PathBuf>,
    sources: HashMap<FileId, String>,
}

impl DiscoveredModules {
    fn extend(&mut self, other: DiscoveredModules) {
        self.modules.extend(other.modules);
        self.diagnostics.extend(other.diagnostics);
        self.files.extend(other.files);
        self.sources.extend(other.sources);
    }
}

/// Result of compiling a single file.
pub struct BuildResult {
    pub diagnostics: Vec<Diagnostic>,
    pub has_errors: bool,
    /// The source text that was compiled (for diagnostic rendering).
    pub source: String,
    /// The file path that was compiled (for diagnostic rendering).
    pub file_path: String,
    /// Checked reflection metadata for runtime reflection/JSON hooks.
    pub reflection_metadata: Option<Arc<ReflectionMetadata>>,
    /// Checked expression type names for runtime normalization at expression-only sites.
    pub checked_expression_types: Option<Arc<HashMap<Span, String>>>,
    /// Values baked by explicit `comptime` expressions.
    pub explicit_comptime_values: Option<Arc<HashMap<Span, Value>>>,
}

/// Mode-specific options for a build.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct BuildOptions {
    /// Apply release-only checker and backend policy.
    pub release: bool,
}

/// Captured output from running a Jett program.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunOutput {
    pub stdout: String,
    pub debug_output: Vec<String>,
}

/// A single definition visible through the namespace query surface.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QueryDefinition {
    pub name: String,
    pub kind: jett_resolve::scope::DefKind,
    pub namespace: Option<String>,
    pub visibility: jett_resolve::scope::DefVisibility,
    pub file_path: String,
    pub line: u32,
    pub column: u32,
    pub end_line: u32,
    pub end_column: u32,
}

/// Result of `jett query --agent --namespaces`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NamespaceQueryResult {
    pub definitions: Vec<QueryDefinition>,
}

/// A top-level symbol declared in a single source file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileSymbolQueryEntry {
    pub name: String,
    pub kind: String,
    pub namespace: Option<String>,
    pub visibility: jett_resolve::scope::DefVisibility,
    pub signature: Option<String>,
    pub line: u32,
    pub column: u32,
    pub end_line: u32,
    pub end_column: u32,
}

/// Result of `jett query --agent --symbols file.jett`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileSymbolsQueryResult {
    pub file_path: String,
    pub symbols: Vec<FileSymbolQueryEntry>,
}

/// Source text and display path retained for a compiler query diagnostic.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QueryDiagnosticSource {
    pub file_id: FileId,
    pub source: String,
    pub file_path: String,
}

/// A compiler query failure that retains diagnostics and source context when available.
#[derive(Debug, Clone)]
pub struct QueryError {
    message: String,
    diagnostics: Vec<Diagnostic>,
    primary_file: Option<FileId>,
    sources: Vec<QueryDiagnosticSource>,
}

impl QueryError {
    fn operational(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            diagnostics: Vec::new(),
            primary_file: None,
            sources: Vec::new(),
        }
    }

    fn compilation(
        message: impl Into<String>,
        diagnostics: Vec<Diagnostic>,
        source: String,
        file_path: String,
    ) -> Self {
        Self::compilation_with_sources(
            message,
            diagnostics,
            FileId::new(0),
            vec![QueryDiagnosticSource {
                file_id: FileId::new(0),
                source,
                file_path,
            }],
        )
    }

    fn compilation_with_sources(
        message: impl Into<String>,
        diagnostics: Vec<Diagnostic>,
        primary_file: FileId,
        sources: Vec<QueryDiagnosticSource>,
    ) -> Self {
        Self {
            message: message.into(),
            diagnostics,
            primary_file: Some(primary_file),
            sources,
        }
    }

    /// Return the compiler diagnostics and requested primary source, if this
    /// query reached the compiler and failed there. Call `diagnostic_sources`
    /// when diagnostic or label spans may refer to support files.
    pub fn diagnostic_context(&self) -> Option<(&[Diagnostic], &str, &str)> {
        let primary_file = self.primary_file?;
        let source = self
            .sources
            .iter()
            .find(|source| source.file_id == primary_file)?;
        Some((&self.diagnostics, &source.source, &source.file_path))
    }

    /// Return compiler diagnostics with every retained source file.
    pub fn diagnostic_sources(&self) -> Option<(&[Diagnostic], FileId, &[QueryDiagnosticSource])> {
        Some((&self.diagnostics, self.primary_file?, &self.sources))
    }
}

impl std::fmt::Display for QueryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)?;
        for diagnostic in self
            .diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.severity == jett_diagnostics::Severity::Error)
        {
            write!(f, "\n{}: {}", diagnostic.code, diagnostic.message)?;
        }
        Ok(())
    }
}

impl std::error::Error for QueryError {}

/// Error returned by a detailed file-symbol query.
pub type FileSymbolsQueryError = QueryError;

/// Error returned by a detailed type-at query.
pub type TypeAtQueryError = QueryError;

/// Error returned by a detailed definition-at query.
pub type DefinitionAtQueryError = QueryError;

/// Error returned by a detailed references-at query.
pub type ReferencesAtQueryError = QueryError;

/// Result of `jett query --agent --type-at file:line:column`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeAtQueryResult {
    pub file_path: String,
    pub line: u32,
    pub column: u32,
    pub type_name: Option<String>,
    pub span_line: Option<u32>,
    pub span_column: Option<u32>,
    pub span_end_line: Option<u32>,
    pub span_end_column: Option<u32>,
}

/// The resolved declaration target for a definition-at query.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DefinitionQueryTarget {
    pub name: String,
    pub kind: jett_resolve::scope::DefKind,
    pub namespace: Option<String>,
    pub visibility: jett_resolve::scope::DefVisibility,
    pub file_path: String,
    pub line: u32,
    pub column: u32,
    pub end_line: u32,
    pub end_column: u32,
}

/// Result of `jett query --agent --definition-at file:line:column`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DefinitionAtQueryResult {
    pub file_path: String,
    pub line: u32,
    pub column: u32,
    pub target: Option<DefinitionQueryTarget>,
}

/// A single use site returned by a references-at query.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReferenceQueryEntry {
    pub file_path: String,
    pub line: u32,
    pub column: u32,
    pub end_line: u32,
    pub end_column: u32,
}

/// Result of `jett query --agent --references-at file:line:column`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReferencesAtQueryResult {
    pub file_path: String,
    pub line: u32,
    pub column: u32,
    pub target: Option<DefinitionQueryTarget>,
    pub references: Vec<ReferenceQueryEntry>,
}

/// A single completion candidate visible at a source position.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompletionQueryEntry {
    pub name: String,
    pub kind: jett_resolve::scope::DefKind,
    pub namespace: Option<String>,
    pub visibility: jett_resolve::scope::DefVisibility,
    pub file_path: String,
    pub line: u32,
    pub column: u32,
    pub end_line: u32,
    pub end_column: u32,
    pub match_kind: CompletionMatchKind,
    pub rank: u32,
    pub signature: Option<String>,
}

/// How a completion candidate matched the cursor prefix.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum CompletionMatchKind {
    EmptyPrefix,
    Exact,
    QualifiedPrefix,
    LeafPrefix,
}

/// Result of `jett query --agent --complete-at file:line:column`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompletionsQueryResult {
    pub file_path: String,
    pub line: u32,
    pub column: u32,
    pub prefix: String,
    pub candidates: Vec<CompletionQueryEntry>,
}

/// A single parameter in a queried function signature.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignatureParam {
    pub name: String,
    pub type_name: String,
    pub view: bool,
    pub mutable: bool,
}

/// Result of `jett query --agent --signature function.name`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignatureQueryResult {
    pub name: String,
    pub type_params: Vec<String>,
    pub params: Vec<SignatureParam>,
    pub return_type: String,
    pub file_path: String,
}

#[derive(Clone)]
struct RunOptions {
    capture_stdout: bool,
    emit_runtime_debug: bool,
    random_test_samples: Option<Vec<RandomTestSample>>,
    clock_test_samples: Option<Vec<ClockTestSample>>,
    environment_test_snapshot: Option<EnvironmentTestSnapshot>,
}

fn parse_source_with_query(source: &str, file_path: &str) -> ParseResult {
    let logical_path = Path::new(file_path)
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
        .unwrap_or("main.jett");
    let mut database = QueryDatabase::default();
    let source_file = database
        .upsert_source(
            SourceOrigin::Project,
            logical_path,
            Arc::<str>::from(source),
        )
        .expect("a single file name is a valid logical source path");
    let parsed = database.parse(source_file);
    ParseResult {
        module: parsed.module.clone(),
        errors: parsed.diagnostics.clone(),
    }
}

/// Run the full compilation pipeline on in-memory source text.
/// Used by the LSP server to validate documents without touching the filesystem.
pub fn build_source(source: &str, file_path: &str) -> BuildResult {
    let mut all_diagnostics = Vec::new();

    // Phase 1+2: Lex + Parse
    let mut parse_result = parse_source_with_query(source, file_path);
    all_diagnostics.extend(parse_result.errors.clone());

    let has_parse_errors = has_error_diagnostics(&all_diagnostics);
    if has_parse_errors {
        return BuildResult {
            has_errors: true,
            diagnostics: all_diagnostics,
            source: source.to_string(),
            file_path: file_path.to_string(),
            reflection_metadata: None,
            checked_expression_types: None,
            explicit_comptime_values: None,
        };
    }

    // Phase 3: Resolve names
    let support_modules = discover_stdlib_modules_with_diagnostics();
    all_diagnostics.extend(support_modules.diagnostics);
    if has_error_diagnostics(&all_diagnostics) {
        return BuildResult {
            has_errors: true,
            diagnostics: all_diagnostics,
            source: source.to_string(),
            file_path: file_path.to_string(),
            reflection_metadata: None,
            checked_expression_types: None,
            explicit_comptime_values: None,
        };
    }
    prepend_support_modules(&mut parse_result.module, support_modules.modules);

    let resolve_result = resolve(&parse_result.module);
    all_diagnostics.extend(resolve_result.diagnostics.clone());

    let has_resolve_errors = has_error_diagnostics(&all_diagnostics);
    if has_resolve_errors {
        return BuildResult {
            has_errors: true,
            diagnostics: all_diagnostics,
            source: source.to_string(),
            file_path: file_path.to_string(),
            reflection_metadata: None,
            checked_expression_types: None,
            explicit_comptime_values: None,
        };
    }

    // Phase 4: Type check
    let check_result = check(&parse_result.module, &resolve_result);
    all_diagnostics.extend(check_result.diagnostics.clone());

    let has_typecheck_errors = has_error_diagnostics(&all_diagnostics);
    if has_typecheck_errors {
        return BuildResult {
            has_errors: true,
            diagnostics: all_diagnostics,
            source: source.to_string(),
            file_path: file_path.to_string(),
            reflection_metadata: None,
            checked_expression_types: None,
            explicit_comptime_values: None,
        };
    }

    // Phase 5: Execute verify blocks at compile time
    let reflection_metadata = check_result.reflection_metadata.clone();
    let checked_expression_types = Arc::new(expression_type_names(&check_result));
    let (explicit_comptime_values, comptime_diagnostics) = evaluate_explicit_comptime_expressions(
        &parse_result.module,
        reflection_metadata.clone(),
        checked_expression_types.clone(),
    );
    all_diagnostics.extend(comptime_diagnostics);
    let verify_diagnostics = run_verify_blocks_with_metadata_and_expression_types(
        &parse_result.module,
        check_result.reflection_metadata,
        checked_expression_types.clone(),
    );
    all_diagnostics.extend(verify_diagnostics);

    let has_errors = has_error_diagnostics(&all_diagnostics);

    BuildResult {
        has_errors,
        diagnostics: all_diagnostics,
        source: source.to_string(),
        file_path: file_path.to_string(),
        reflection_metadata: Some(reflection_metadata),
        checked_expression_types: Some(checked_expression_types),
        explicit_comptime_values: Some(Arc::new(explicit_comptime_values)),
    }
}

/// Return the inferred type name at the given (1-based) line and column in `source`.
/// Returns `None` if the position is outside any typed expression or if the file
/// does not compile cleanly past the parse phase.
pub fn hover_type(source: &str, line: u32, col: u32) -> Option<String> {
    let file_id = FileId::new(0);

    // Convert 1-based (line, col) to a byte offset.
    let offset = line_col_to_offset(source, line, col)?;

    let mut parse_result = parse(source, file_id);
    if parse_result
        .errors
        .iter()
        .any(|d| d.severity == jett_diagnostics::Severity::Error)
    {
        return None;
    }

    prepend_support_modules(&mut parse_result.module, discover_stdlib_modules());

    let resolve_result = resolve(&parse_result.module);
    if resolve_result
        .diagnostics
        .iter()
        .any(|d| d.severity == jett_diagnostics::Severity::Error)
    {
        return None;
    }

    let check_result = check(&parse_result.module, &resolve_result);

    // Find the smallest span in type_map that contains `offset`.
    let mut best: Option<(u32, jett_types::TypeId)> = None;
    for (span, ty_id) in &check_result.type_map {
        if span.file == file_id && span.start <= offset && offset <= span.end {
            let len = span.end - span.start;
            if best.is_none() || len < best.unwrap().0 {
                best = Some((len, *ty_id));
            }
        }
    }

    best.map(|(_, ty_id)| check_result.interner.type_name(ty_id))
}

/// Return the inferred type name at a source position in a file.
///
/// This query parses, resolves, and typechecks with stdlib plus sibling project
/// modules, but it does not execute verify/property blocks.
pub fn query_type_at(path: &Path, line: u32, column: u32) -> Result<TypeAtQueryResult, String> {
    query_type_at_detailed(path, line, column).map_err(|error| error.to_string())
}

/// Return the inferred type while retaining compiler diagnostics on failure.
pub fn query_type_at_detailed(
    path: &Path,
    line: u32,
    column: u32,
) -> Result<TypeAtQueryResult, TypeAtQueryError> {
    let source = fs::read_to_string(path).map_err(|error| {
        TypeAtQueryError::operational(format!("failed to read {}: {}", path.display(), error))
    })?;
    let file_path = path.display().to_string();
    let file_id = FileId::new(0);
    let Some(offset) = line_col_to_offset(&source, line, column) else {
        return Err(TypeAtQueryError::operational(format!(
            "position {line}:{column} is outside {}",
            path.display()
        )));
    };

    let mut parse_result = parse(&source, file_id);
    if has_error_diagnostics(&parse_result.errors) {
        return Err(TypeAtQueryError::compilation(
            "parse errors:",
            parse_result.errors,
            source,
            file_path,
        ));
    }

    let mut support_modules = discover_stdlib_modules_with_diagnostics();
    support_modules.extend(discover_project_modules_with_diagnostics(path));
    let support_errors = error_messages_from_diagnostics(&support_modules.diagnostics);
    if !support_errors.is_empty() {
        let diagnostic_sources = query_diagnostic_sources(
            file_id,
            &source,
            &file_path,
            &support_modules,
            &support_modules.diagnostics,
        );
        if diagnostics_have_source_context(&support_modules.diagnostics, &diagnostic_sources) {
            return Err(TypeAtQueryError::compilation_with_sources(
                "support parse errors:",
                support_modules.diagnostics,
                file_id,
                diagnostic_sources,
            ));
        }
        return Err(TypeAtQueryError::operational(format!(
            "support parse errors:\n{}",
            support_errors.join("\n")
        )));
    }
    prepend_support_modules(
        &mut parse_result.module,
        std::mem::take(&mut support_modules.modules),
    );

    let resolve_result = resolve(&parse_result.module);
    let resolve_errors = error_messages_from_diagnostics(&resolve_result.diagnostics);
    if !resolve_errors.is_empty() {
        let diagnostic_sources = query_diagnostic_sources(
            file_id,
            &source,
            &file_path,
            &support_modules,
            &resolve_result.diagnostics,
        );
        if diagnostics_have_source_context(&resolve_result.diagnostics, &diagnostic_sources) {
            return Err(TypeAtQueryError::compilation_with_sources(
                "resolution errors:",
                resolve_result.diagnostics,
                file_id,
                diagnostic_sources,
            ));
        }
        return Err(TypeAtQueryError::operational(format!(
            "resolution errors:\n{}",
            resolve_errors.join("\n")
        )));
    }

    let check_result = check(&parse_result.module, &resolve_result);
    let type_errors = error_messages_from_diagnostics(&check_result.diagnostics);
    if !type_errors.is_empty() {
        let diagnostic_sources = query_diagnostic_sources(
            file_id,
            &source,
            &file_path,
            &support_modules,
            &check_result.diagnostics,
        );
        if diagnostics_have_source_context(&check_result.diagnostics, &diagnostic_sources) {
            return Err(TypeAtQueryError::compilation_with_sources(
                "type errors:",
                check_result.diagnostics,
                file_id,
                diagnostic_sources,
            ));
        }
        return Err(TypeAtQueryError::operational(format!(
            "type errors:\n{}",
            type_errors.join("\n")
        )));
    }

    let mut best: Option<(u32, Span, jett_types::TypeId)> = None;
    for (span, ty_id) in &check_result.type_map {
        if span.file == file_id && span.start <= offset && offset <= span.end {
            let len = span.end - span.start;
            if best.is_none() || len < best.unwrap().0 {
                best = Some((len, *span, *ty_id));
            }
        }
    }
    let (type_name, span_line, span_column, span_end_line, span_end_column) =
        if let Some((_, span, ty_id)) = best {
            let (span_line, span_column) = jett_diagnostics::render::line_col(&source, span.start);
            let (span_end_line, span_end_column) =
                jett_diagnostics::render::line_col(&source, span.end);
            (
                Some(check_result.interner.type_name(ty_id)),
                Some(span_line as u32),
                Some(span_column as u32),
                Some(span_end_line as u32),
                Some(span_end_column as u32),
            )
        } else {
            (None, None, None, None, None)
        };

    Ok(TypeAtQueryResult {
        file_path,
        line,
        column,
        type_name,
        span_line,
        span_column,
        span_end_line,
        span_end_column,
    })
}

/// Return the resolved definition target at a source position in a file.
///
/// This query parses and resolves with stdlib plus sibling project modules, but
/// it does not typecheck or execute verify/property blocks.
pub fn query_definition_at(
    path: &Path,
    line: u32,
    column: u32,
) -> Result<DefinitionAtQueryResult, String> {
    query_definition_at_detailed(path, line, column).map_err(|error| error.to_string())
}

/// Return the resolved definition while retaining compiler diagnostics on failure.
pub fn query_definition_at_detailed(
    path: &Path,
    line: u32,
    column: u32,
) -> Result<DefinitionAtQueryResult, DefinitionAtQueryError> {
    let source = fs::read_to_string(path).map_err(|error| {
        DefinitionAtQueryError::operational(format!("failed to read {}: {}", path.display(), error))
    })?;
    let file_path = path.display().to_string();
    let file_id = FileId::new(0);
    let Some(offset) = line_col_to_offset(&source, line, column) else {
        return Err(DefinitionAtQueryError::operational(format!(
            "position {line}:{column} is outside {}",
            path.display()
        )));
    };

    let mut parse_result = parse(&source, file_id);
    if has_error_diagnostics(&parse_result.errors) {
        return Err(DefinitionAtQueryError::compilation(
            "parse errors:",
            parse_result.errors,
            source,
            file_path,
        ));
    }

    let mut support_modules = discover_stdlib_modules_with_diagnostics();
    support_modules.extend(discover_project_modules_with_diagnostics(path));
    let support_errors = error_messages_from_diagnostics(&support_modules.diagnostics);
    if !support_errors.is_empty() {
        return Err(DefinitionAtQueryError::operational(format!(
            "support parse errors:\n{}",
            support_errors.join("\n")
        )));
    }

    let mut file_paths = support_modules.files.clone();
    let display_path = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    file_paths.insert(file_id, display_path);

    prepend_support_modules(
        &mut parse_result.module,
        std::mem::take(&mut support_modules.modules),
    );

    let resolve_result = resolve(&parse_result.module);
    let resolve_errors = error_messages_from_diagnostics(&resolve_result.diagnostics);
    if !resolve_errors.is_empty() {
        let diagnostic_sources = query_diagnostic_sources(
            file_id,
            &source,
            &file_path,
            &support_modules,
            &resolve_result.diagnostics,
        );
        if diagnostics_have_source_context(&resolve_result.diagnostics, &diagnostic_sources) {
            return Err(DefinitionAtQueryError::compilation_with_sources(
                "resolution errors:",
                resolve_result.diagnostics,
                file_id,
                diagnostic_sources,
            ));
        }
        return Err(DefinitionAtQueryError::operational(format!(
            "resolution errors:\n{}",
            resolve_errors.join("\n")
        )));
    }

    let mut best_def: Option<(u32, jett_resolve::scope::DefId)> = None;
    for (span, def_id) in &resolve_result.resolutions {
        if span.file == file_id && span.start <= offset && offset <= span.end {
            let len = span.end - span.start;
            if best_def.is_none() || len < best_def.unwrap().0 {
                best_def = Some((len, *def_id));
            }
        }
    }

    let target = best_def.and_then(|(_, def_id)| {
        let def = resolve_result.scope_table.def(def_id);
        let (file_path, line, column, end_line, end_column) =
            span_location(Some(&source), def.span, &file_paths)?;
        Some(DefinitionQueryTarget {
            name: def.name.clone(),
            kind: def.kind,
            namespace: def.namespace.clone(),
            visibility: def.visibility,
            file_path,
            line,
            column,
            end_line,
            end_column,
        })
    });

    Ok(DefinitionAtQueryResult {
        file_path,
        line,
        column,
        target,
    })
}

/// Return all resolver-visible references to the symbol at a source position.
///
/// This uses the same resolver map as definition-at and returns use sites only;
/// the declaration itself is reported separately as `target`.
pub fn query_references_at(
    path: &Path,
    line: u32,
    column: u32,
) -> Result<ReferencesAtQueryResult, String> {
    query_references_at_detailed(path, line, column).map_err(|error| error.to_string())
}

/// Return resolver-visible references while retaining compiler diagnostics on failure.
pub fn query_references_at_detailed(
    path: &Path,
    line: u32,
    column: u32,
) -> Result<ReferencesAtQueryResult, ReferencesAtQueryError> {
    let source = fs::read_to_string(path).map_err(|error| {
        ReferencesAtQueryError::operational(format!("failed to read {}: {}", path.display(), error))
    })?;
    let file_path = path.display().to_string();
    let file_id = FileId::new(0);
    let Some(offset) = line_col_to_offset(&source, line, column) else {
        return Err(ReferencesAtQueryError::operational(format!(
            "position {line}:{column} is outside {}",
            path.display()
        )));
    };

    let mut parse_result = parse(&source, file_id);
    if has_error_diagnostics(&parse_result.errors) {
        return Err(ReferencesAtQueryError::compilation(
            "parse errors:",
            parse_result.errors,
            source,
            file_path,
        ));
    }

    let mut support_modules = discover_stdlib_modules_with_diagnostics();
    support_modules.extend(discover_project_modules_with_diagnostics(path));
    let support_errors = error_messages_from_diagnostics(&support_modules.diagnostics);
    if !support_errors.is_empty() {
        return Err(ReferencesAtQueryError::operational(format!(
            "support parse errors:\n{}",
            support_errors.join("\n")
        )));
    }

    let mut file_paths = support_modules.files.clone();
    let display_path = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    file_paths.insert(file_id, display_path);

    prepend_support_modules(
        &mut parse_result.module,
        std::mem::take(&mut support_modules.modules),
    );

    let resolve_result = resolve(&parse_result.module);
    let resolve_errors = error_messages_from_diagnostics(&resolve_result.diagnostics);
    if !resolve_errors.is_empty() {
        let diagnostic_sources = query_diagnostic_sources(
            file_id,
            &source,
            &file_path,
            &support_modules,
            &resolve_result.diagnostics,
        );
        if diagnostics_have_source_context(&resolve_result.diagnostics, &diagnostic_sources) {
            return Err(ReferencesAtQueryError::compilation_with_sources(
                "resolution errors:",
                resolve_result.diagnostics,
                file_id,
                diagnostic_sources,
            ));
        }
        return Err(ReferencesAtQueryError::operational(format!(
            "resolution errors:\n{}",
            resolve_errors.join("\n")
        )));
    }

    let Some((_, target_def_id)) = best_resolved_definition_at(&resolve_result, file_id, offset)
    else {
        return Ok(ReferencesAtQueryResult {
            file_path,
            line,
            column,
            target: None,
            references: Vec::new(),
        });
    };

    let def = resolve_result.scope_table.def(target_def_id);
    let target = span_location(Some(&source), def.span, &file_paths).map(
        |(file_path, line, column, end_line, end_column)| DefinitionQueryTarget {
            name: def.name.clone(),
            kind: def.kind,
            namespace: def.namespace.clone(),
            visibility: def.visibility,
            file_path,
            line,
            column,
            end_line,
            end_column,
        },
    );

    let mut references = Vec::new();
    for (span, def_id) in &resolve_result.resolutions {
        if *def_id == target_def_id
            && let Some((file_path, line, column, end_line, end_column)) =
                span_location(Some(&source), *span, &file_paths)
        {
            references.push(ReferenceQueryEntry {
                file_path,
                line,
                column,
                end_line,
                end_column,
            });
        }
    }
    references.sort_by(|left, right| {
        left.file_path
            .cmp(&right.file_path)
            .then_with(|| left.line.cmp(&right.line))
            .then_with(|| left.column.cmp(&right.column))
    });

    Ok(ReferencesAtQueryResult {
        file_path,
        line,
        column,
        target,
        references,
    })
}

/// Return a list of (name, kind) completion candidates visible in `source`.
/// Runs parse + resolve and collects all definitions from the scope table.
pub fn completions(source: &str) -> Vec<(String, jett_resolve::scope::DefKind)> {
    completions_for_namespace(source, None)
}

/// Return completion candidates visible at the given (1-based) line and column.
pub fn completions_at(
    source: &str,
    line: u32,
    col: u32,
) -> Vec<(String, jett_resolve::scope::DefKind)> {
    let file_id = FileId::new(0);
    let Some(offset) = line_col_to_offset(source, line, col) else {
        return Vec::new();
    };

    let parsed = parse(source, file_id);
    let current_namespace = namespace_at_offset(&parsed.module, file_id, offset);
    let support_modules = discover_stdlib_modules();
    let current_namespace = current_namespace
        .filter(|namespace| !support_modules_declare_namespace(&support_modules, namespace));
    completions_for_namespace_with_support(source, current_namespace.as_deref(), support_modules)
}

/// Return completion candidates visible at a source position in a file.
pub fn query_completions_at(
    path: &Path,
    line: u32,
    column: u32,
) -> Result<CompletionsQueryResult, String> {
    let source = fs::read_to_string(path)
        .map_err(|e| format!("failed to read {}: {}", path.display(), e))?;
    let file_path = path.display().to_string();
    let file_id = FileId::new(0);
    let Some(offset) = line_col_to_offset(&source, line, column) else {
        return Err(format!(
            "position {line}:{column} is outside {}",
            path.display()
        ));
    };
    let prefix = completion_prefix_at(&source, offset);

    let parsed = parse(&source, file_id);
    let parse_errors = error_messages_from_diagnostics(&parsed.errors);
    if !parse_errors.is_empty() {
        return Err(format!("parse errors:\n{}", parse_errors.join("\n")));
    }

    let mut support_modules = discover_stdlib_modules_with_diagnostics();
    support_modules.extend(discover_project_modules_with_diagnostics(path));
    let support_errors = error_messages_from_diagnostics(&support_modules.diagnostics);
    if !support_errors.is_empty() {
        return Err(format!(
            "support parse errors:\n{}",
            support_errors.join("\n")
        ));
    }

    let mut file_paths = support_modules.files.clone();
    let display_path = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    file_paths.insert(file_id, display_path);

    let mut definitions = query_builtin_definitions();
    for module in &support_modules.modules {
        append_module_query_definitions(&mut definitions, module, None, &file_paths);
    }
    append_module_query_definitions(&mut definitions, &parsed.module, Some(&source), &file_paths);

    let mut signatures = HashMap::new();
    for module in &support_modules.modules {
        append_module_signature_displays(&mut signatures, module, &file_paths);
    }
    append_module_signature_displays(&mut signatures, &parsed.module, &file_paths);

    let mut candidates: Vec<CompletionQueryEntry> = definitions
        .into_iter()
        .filter_map(|definition| {
            let match_kind = completion_match_kind(&definition.name, &prefix)?;
            Some(CompletionQueryEntry {
                signature: signatures.get(&definition.name).cloned(),
                name: definition.name,
                kind: definition.kind,
                namespace: definition.namespace,
                visibility: definition.visibility,
                file_path: definition.file_path,
                line: definition.line,
                column: definition.column,
                end_line: definition.end_line,
                end_column: definition.end_column,
                match_kind,
                rank: completion_rank(match_kind),
            })
        })
        .collect();
    candidates.sort_by(|left, right| {
        left.rank
            .cmp(&right.rank)
            .then_with(|| left.name.cmp(&right.name))
            .then_with(|| query_kind_name(left.kind).cmp(query_kind_name(right.kind)))
    });
    candidates.dedup_by(|left, right| left.name == right.name && left.kind == right.kind);

    Ok(CompletionsQueryResult {
        file_path,
        line,
        column,
        prefix,
        candidates,
    })
}

/// Return the source-level signature for a public function.
pub fn query_signature(
    start_dir: &Path,
    function_name: &str,
) -> Result<Option<SignatureQueryResult>, String> {
    let mut support_modules = discover_stdlib_modules_with_diagnostics();
    support_modules.extend(discover_query_project_modules_with_diagnostics(start_dir));

    let support_errors = error_messages_from_diagnostics(&support_modules.diagnostics);
    if !support_errors.is_empty() {
        return Err(format!(
            "query support parse errors:\n{}",
            support_errors.join("\n")
        ));
    }

    for module in &support_modules.modules {
        if let Some(signature) =
            module_signature_query_result(module, &support_modules.files, function_name)
        {
            return Ok(Some(signature));
        }
    }

    Ok(None)
}

fn module_signature_query_result(
    module: &Module,
    file_paths: &HashMap<FileId, PathBuf>,
    function_name: &str,
) -> Option<SignatureQueryResult> {
    let mut current_namespace: Option<String> = None;
    for item in &module.items {
        match item {
            Item::Namespace(ns) => current_namespace = Some(ns.name.name.clone()),
            Item::Function(func) => {
                if let Some(signature) =
                    function_signature_query_result(func, current_namespace.as_deref(), file_paths)
                    && signature.name == function_name
                {
                    return Some(signature);
                }
            }
            Item::Mutual(block) => {
                for decl in &block.declarations {
                    if let Some(signature) = function_decl_signature_query_result(
                        decl,
                        current_namespace.as_deref(),
                        file_paths,
                    ) && signature.name == function_name
                    {
                        return Some(signature);
                    }
                }
            }
            Item::Interface(_)
            | Item::Implement(_)
            | Item::Struct(_)
            | Item::Bitfield(_)
            | Item::Enum(_)
            | Item::Machine(_)
            | Item::Actor(_)
            | Item::VarDecl(_)
            | Item::Verify(_)
            | Item::Property(_)
            | Item::Resource(_)
            | Item::TypeAlias(_) => {}
        }
    }
    None
}

fn append_module_signature_displays(
    signatures: &mut HashMap<String, String>,
    module: &Module,
    file_paths: &HashMap<FileId, PathBuf>,
) {
    let mut current_namespace: Option<String> = None;
    for item in &module.items {
        match item {
            Item::Namespace(ns) => current_namespace = Some(ns.name.name.clone()),
            Item::Function(func) => {
                if let Some(signature) =
                    function_signature_query_result(func, current_namespace.as_deref(), file_paths)
                {
                    signatures
                        .entry(signature.name.clone())
                        .or_insert_with(|| signature_display(&signature));
                }
            }
            Item::Mutual(block) => {
                for decl in &block.declarations {
                    if let Some(signature) = function_decl_signature_query_result(
                        decl,
                        current_namespace.as_deref(),
                        file_paths,
                    ) {
                        signatures
                            .entry(signature.name.clone())
                            .or_insert_with(|| signature_display(&signature));
                    }
                }
            }
            Item::Interface(_)
            | Item::Implement(_)
            | Item::Struct(_)
            | Item::Bitfield(_)
            | Item::Enum(_)
            | Item::Machine(_)
            | Item::Actor(_)
            | Item::VarDecl(_)
            | Item::Verify(_)
            | Item::Property(_)
            | Item::Resource(_)
            | Item::TypeAlias(_) => {}
        }
    }
}

fn function_signature_query_result(
    func: &FunctionDef,
    namespace: Option<&str>,
    file_paths: &HashMap<FileId, PathBuf>,
) -> Option<SignatureQueryResult> {
    if namespace.is_some() && !func.exported {
        return None;
    }

    Some(signature_query_result(
        &func.name.name,
        &func.type_params,
        &func.params,
        func.return_type.as_ref(),
        namespace,
        func.span.file,
        file_paths,
    ))
}

fn function_decl_signature_query_result(
    decl: &FunctionDecl,
    namespace: Option<&str>,
    file_paths: &HashMap<FileId, PathBuf>,
) -> Option<SignatureQueryResult> {
    if namespace.is_some() && !decl.exported {
        return None;
    }

    Some(signature_query_result(
        &decl.name.name,
        &decl.type_params,
        &decl.params,
        decl.return_type.as_ref(),
        namespace,
        decl.span.file,
        file_paths,
    ))
}

fn signature_display(signature: &SignatureQueryResult) -> String {
    let type_params = if signature.type_params.is_empty() {
        String::new()
    } else {
        format!("[{}]", signature.type_params.join(", "))
    };
    let params: Vec<String> = signature
        .params
        .iter()
        .map(|param| {
            let mut prefix = String::new();
            if param.view {
                prefix.push_str("view ");
            }
            if param.mutable {
                prefix.push_str("mutable ");
            }
            format!("{prefix}{}: {}", param.name, param.type_name)
        })
        .collect();
    format!(
        "{}{}({}) returns {}",
        signature.name,
        type_params,
        params.join(", "),
        signature.return_type
    )
}

fn signature_query_result(
    leaf_name: &str,
    type_params: &[jett_parser::ast::Ident],
    params: &[Param],
    return_type: Option<&TypeExpr>,
    namespace: Option<&str>,
    file: FileId,
    file_paths: &HashMap<FileId, PathBuf>,
) -> SignatureQueryResult {
    let name = namespace
        .map(|namespace| format!("{namespace}.{leaf_name}"))
        .unwrap_or_else(|| leaf_name.to_string());
    let type_params: Vec<String> = type_params.iter().map(|param| param.name.clone()).collect();
    let params = params
        .iter()
        .map(|param| SignatureParam {
            name: param.name.name.clone(),
            type_name: signature_type_expr_name(&param.ty, namespace, &type_params),
            view: param.view,
            mutable: param.mutable,
        })
        .collect();
    let return_type = return_type
        .map(|ty| signature_type_expr_name(ty, namespace, &type_params))
        .unwrap_or_else(|| "nothing".to_string());

    SignatureQueryResult {
        name,
        type_params,
        params,
        return_type,
        file_path: query_file_path(file, file_paths),
    }
}

fn signature_type_expr_name(
    ty: &TypeExpr,
    namespace: Option<&str>,
    type_params: &[String],
) -> String {
    match ty {
        TypeExpr::Named(ident) => signature_named_type_name(&ident.name, namespace, type_params),
        TypeExpr::Generic(name, args, _) => {
            let name = signature_generic_type_name(&name.name, namespace);
            let args: Vec<String> = args
                .iter()
                .map(|arg| signature_type_expr_name(arg, namespace, type_params))
                .collect();
            format!("{}[{}]", name, args.join(", "))
        }
        TypeExpr::View(inner, _) => {
            format!(
                "view {}",
                signature_type_expr_name(inner, namespace, type_params)
            )
        }
        TypeExpr::StateQualified(inner, state, _) => {
            format!(
                "{} at {}",
                signature_type_expr_name(inner, namespace, type_params),
                state.name
            )
        }
        TypeExpr::Function(params, ret, _) => {
            let params: Vec<String> = params
                .iter()
                .map(|param| signature_type_expr_name(param, namespace, type_params))
                .collect();
            format!(
                "function({}) returns {}",
                params.join(", "),
                signature_type_expr_name(ret, namespace, type_params)
            )
        }
    }
}

fn signature_named_type_name(
    name: &str,
    namespace: Option<&str>,
    type_params: &[String],
) -> String {
    if name.contains('.')
        || type_params.iter().any(|type_param| type_param == name)
        || signature_builtin_type_name(name)
    {
        return name.to_string();
    }

    namespace
        .map(|namespace| format!("{namespace}.{name}"))
        .unwrap_or_else(|| name.to_string())
}

fn signature_generic_type_name(name: &str, namespace: Option<&str>) -> String {
    if name.contains('.') || signature_builtin_generic_type_name(name) {
        return name.to_string();
    }

    namespace
        .map(|namespace| format!("{namespace}.{name}"))
        .unwrap_or_else(|| name.to_string())
}

fn signature_builtin_type_name(name: &str) -> bool {
    matches!(
        name,
        "int8"
            | "int16"
            | "int32"
            | "int64"
            | "uint8"
            | "uint16"
            | "uint32"
            | "uint64"
            | "float32"
            | "float64"
            | "string"
            | "bool"
            | "bytes"
            | "nothing"
            | "TypeConstruction"
            | "TypeInfo"
            | "TypeKind"
            | "TypePrimitive"
            | "TypeField"
            | "TypeBitfield"
            | "TypeBitfieldField"
            | "TypeBitfieldFieldShape"
            | "TypeMachine"
            | "TypeMachineState"
            | "TypeMachineTransition"
            | "TypeVariant"
            | "Stdout"
            | "Stderr"
            | "Stdin"
            | "Filesystem"
            | "Network"
            | "Clock"
            | "Random"
            | "Process"
            | "Environment"
    )
}

fn signature_builtin_generic_type_name(name: &str) -> bool {
    matches!(
        name,
        "list" | "map" | "set" | "optional" | "result" | "secret"
    )
}

fn completions_for_namespace(
    source: &str,
    current_namespace: Option<&str>,
) -> Vec<(String, jett_resolve::scope::DefKind)> {
    completions_for_namespace_with_support(source, current_namespace, discover_stdlib_modules())
}

fn completions_for_namespace_with_support(
    source: &str,
    current_namespace: Option<&str>,
    support_modules: Vec<Module>,
) -> Vec<(String, jett_resolve::scope::DefKind)> {
    use jett_resolve::scope::DefVisibility;

    let file_id = FileId::new(0);
    let mut parse_result = parse(source, file_id);
    if parse_result
        .errors
        .iter()
        .any(|d| d.severity == jett_diagnostics::Severity::Error)
    {
        return Vec::new();
    }
    prepend_support_modules(&mut parse_result.module, support_modules);

    let resolve_result = resolve(&parse_result.module);
    resolve_result
        .scope_table
        .definitions
        .iter()
        .filter(|def| {
            def.namespace.is_none()
                || def.visibility == DefVisibility::Public
                || def.namespace.as_deref() == current_namespace
        })
        .map(|def| (def.name.clone(), def.kind))
        .collect()
}

fn completion_prefix_at(source: &str, offset: u32) -> String {
    let mut end = offset as usize;
    if end > source.len() {
        end = source.len();
    }
    while end > 0 && !source.is_char_boundary(end) {
        end -= 1;
    }

    let mut start = end;
    while start > 0 {
        let Some(ch) = source[..start].chars().next_back() else {
            break;
        };
        if ch.is_ascii_alphanumeric() || ch == '_' || ch == '.' {
            start -= ch.len_utf8();
        } else {
            break;
        }
    }

    source[start..end].to_string()
}

fn completion_match_kind(name: &str, prefix: &str) -> Option<CompletionMatchKind> {
    if prefix.is_empty() {
        return Some(CompletionMatchKind::EmptyPrefix);
    }

    let leaf = name.rsplit_once('.').map_or(name, |(_, leaf)| leaf);
    if name == prefix || leaf == prefix {
        return Some(CompletionMatchKind::Exact);
    }
    if name.starts_with(prefix) {
        return Some(CompletionMatchKind::QualifiedPrefix);
    }
    if prefix.contains('.') {
        return None;
    }

    leaf.starts_with(prefix)
        .then_some(CompletionMatchKind::LeafPrefix)
}

fn completion_rank(match_kind: CompletionMatchKind) -> u32 {
    match match_kind {
        CompletionMatchKind::Exact => 0,
        CompletionMatchKind::QualifiedPrefix => 10,
        CompletionMatchKind::LeafPrefix => 20,
        CompletionMatchKind::EmptyPrefix => 100,
    }
}

/// Return the public namespace and definition registry available from `start_dir`.
///
/// If `start_dir` is inside a project, project `.jett` files are included with
/// compiler-shipped stdlib modules. Without a `jett.proj`, the query still
/// returns stdlib and language built-ins so agents can discover the base surface.
pub fn query_namespaces(start_dir: &Path) -> Result<NamespaceQueryResult, String> {
    let mut support_modules = discover_stdlib_modules_with_diagnostics();
    support_modules.extend(discover_query_project_modules_with_diagnostics(start_dir));

    let support_errors = error_messages_from_diagnostics(&support_modules.diagnostics);
    if !support_errors.is_empty() {
        return Err(format!(
            "query support parse errors:\n{}",
            support_errors.join("\n")
        ));
    }

    let mut definitions = query_builtin_definitions();
    for module in &support_modules.modules {
        append_module_query_definitions(&mut definitions, module, None, &support_modules.files);
    }

    definitions.sort_by(|left, right| {
        left.namespace
            .cmp(&right.namespace)
            .then_with(|| left.name.cmp(&right.name))
            .then_with(|| query_kind_name(left.kind).cmp(query_kind_name(right.kind)))
    });
    definitions.dedup_by(|left, right| {
        left.name == right.name
            && left.kind == right.kind
            && left.namespace == right.namespace
            && left.visibility == right.visibility
    });

    Ok(NamespaceQueryResult { definitions })
}

/// Return a file-local outline of top-level declarations, including private
/// symbols that are intentionally omitted from the global namespace query.
pub fn query_file_symbols(path: &Path) -> Result<FileSymbolsQueryResult, String> {
    query_file_symbols_detailed(path).map_err(|error| error.to_string())
}

/// Return a file-local outline while retaining compiler diagnostics on failure.
pub fn query_file_symbols_detailed(
    path: &Path,
) -> Result<FileSymbolsQueryResult, FileSymbolsQueryError> {
    let source = fs::read_to_string(path).map_err(|error| {
        FileSymbolsQueryError::operational(format!("failed to read {}: {}", path.display(), error))
    })?;
    let file_path = path.display().to_string();
    let display_path = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    query_source_file_symbols_with_path(&source, file_path, display_path)
}

/// Return a file-local outline from source text that may not exist on disk.
///
/// LSP clients use this entry point so document symbols reflect the latest
/// in-memory document revision rather than a stale saved file.
pub fn query_source_file_symbols(
    source: &str,
    file_path: &str,
) -> Result<FileSymbolsQueryResult, FileSymbolsQueryError> {
    query_source_file_symbols_with_path(source, file_path.to_string(), PathBuf::from(file_path))
}

fn query_source_file_symbols_with_path(
    source: &str,
    file_path: String,
    display_path: PathBuf,
) -> Result<FileSymbolsQueryResult, FileSymbolsQueryError> {
    let parsed = parse(source, FileId::new(0));
    if has_error_diagnostics(&parsed.errors) {
        return Err(FileSymbolsQueryError::compilation(
            "parse errors:",
            parsed.errors,
            source.to_string(),
            file_path,
        ));
    }

    let mut symbols = Vec::new();
    let mut file_paths = HashMap::new();
    file_paths.insert(FileId::new(0), display_path);
    append_file_symbol_query_entries(&mut symbols, &parsed.module, source, &file_paths);
    Ok(FileSymbolsQueryResult { file_path, symbols })
}

fn append_file_symbol_query_entries(
    symbols: &mut Vec<FileSymbolQueryEntry>,
    module: &Module,
    source: &str,
    file_paths: &HashMap<FileId, PathBuf>,
) {
    let mut current_namespace: Option<String> = None;
    for item in &module.items {
        match item {
            Item::Namespace(ns) => {
                current_namespace = Some(ns.name.name.clone());
                push_file_symbol_query_entry(
                    symbols,
                    ns.name.name.clone(),
                    "namespace",
                    None,
                    jett_resolve::scope::DefVisibility::Public,
                    None,
                    ns.name.span,
                    source,
                );
            }
            Item::Function(func) => {
                let signature = file_symbol_function_signature(
                    &func.name.name,
                    &func.type_params,
                    &func.params,
                    func.return_type.as_ref(),
                    current_namespace.as_deref(),
                    func.span.file,
                    file_paths,
                );
                push_file_symbol_query_entry(
                    symbols,
                    file_symbol_name(&func.name.name, current_namespace.as_deref()),
                    "function",
                    current_namespace.clone(),
                    file_symbol_visibility(current_namespace.as_deref(), func.exported),
                    Some(signature),
                    func.name.span,
                    source,
                );
            }
            Item::Mutual(block) => {
                for decl in &block.declarations {
                    let signature = file_symbol_function_signature(
                        &decl.name.name,
                        &decl.type_params,
                        &decl.params,
                        decl.return_type.as_ref(),
                        current_namespace.as_deref(),
                        decl.span.file,
                        file_paths,
                    );
                    push_file_symbol_query_entry(
                        symbols,
                        file_symbol_name(&decl.name.name, current_namespace.as_deref()),
                        "function",
                        current_namespace.clone(),
                        file_symbol_visibility(current_namespace.as_deref(), decl.exported),
                        Some(signature),
                        decl.name.span,
                        source,
                    );
                }
            }
            Item::Interface(interface) => push_file_symbol_query_entry(
                symbols,
                file_symbol_name(&interface.name.name, current_namespace.as_deref()),
                "interface",
                current_namespace.clone(),
                file_symbol_visibility(current_namespace.as_deref(), interface.exported),
                None,
                interface.name.span,
                source,
            ),
            Item::Implement(block) => push_file_symbol_query_entry(
                symbols,
                format!(
                    "implement {} for {}",
                    block.interface_name.name,
                    type_expr_name(&block.for_type)
                ),
                "implement",
                current_namespace.clone(),
                jett_resolve::scope::DefVisibility::Private,
                None,
                block.interface_name.span,
                source,
            ),
            Item::Struct(strukt) => push_file_symbol_query_entry(
                symbols,
                file_symbol_name(&strukt.name.name, current_namespace.as_deref()),
                "struct",
                current_namespace.clone(),
                file_symbol_visibility(current_namespace.as_deref(), strukt.exported),
                None,
                strukt.name.span,
                source,
            ),
            Item::Bitfield(bitfield) => push_file_symbol_query_entry(
                symbols,
                file_symbol_name(&bitfield.name.name, current_namespace.as_deref()),
                "bitfield",
                current_namespace.clone(),
                file_symbol_visibility(current_namespace.as_deref(), bitfield.exported),
                None,
                bitfield.name.span,
                source,
            ),
            Item::Enum(enm) => push_file_symbol_query_entry(
                symbols,
                file_symbol_name(&enm.name.name, current_namespace.as_deref()),
                "enum",
                current_namespace.clone(),
                file_symbol_visibility(current_namespace.as_deref(), enm.exported),
                None,
                enm.name.span,
                source,
            ),
            Item::Machine(machine) => push_file_symbol_query_entry(
                symbols,
                file_symbol_name(&machine.name.name, current_namespace.as_deref()),
                "machine",
                current_namespace.clone(),
                file_symbol_visibility(current_namespace.as_deref(), machine.exported),
                None,
                machine.name.span,
                source,
            ),
            Item::Actor(actor) => push_file_symbol_query_entry(
                symbols,
                file_symbol_name(&actor.name.name, current_namespace.as_deref()),
                "actor",
                current_namespace.clone(),
                file_symbol_visibility(current_namespace.as_deref(), actor.exported),
                None,
                actor.name.span,
                source,
            ),
            Item::VarDecl(decl) => push_file_symbol_query_entry(
                symbols,
                file_symbol_name(&decl.name.name, current_namespace.as_deref()),
                "variable",
                current_namespace.clone(),
                jett_resolve::scope::DefVisibility::Private,
                None,
                decl.name.span,
                source,
            ),
            Item::Verify(verify) => push_file_symbol_query_entry(
                symbols,
                file_symbol_name(&verify.name.name, current_namespace.as_deref()),
                "verify",
                current_namespace.clone(),
                jett_resolve::scope::DefVisibility::Private,
                None,
                verify.name.span,
                source,
            ),
            Item::Property(prop) => push_file_symbol_query_entry(
                symbols,
                file_symbol_name(&prop.name.name, current_namespace.as_deref()),
                "property",
                current_namespace.clone(),
                jett_resolve::scope::DefVisibility::Private,
                None,
                prop.name.span,
                source,
            ),
            Item::Resource(resource) => push_file_symbol_query_entry(
                symbols,
                file_symbol_name(&resource.name.name, current_namespace.as_deref()),
                "resource",
                current_namespace.clone(),
                file_symbol_visibility(current_namespace.as_deref(), resource.exported),
                None,
                resource.name.span,
                source,
            ),
            Item::TypeAlias(alias) => push_file_symbol_query_entry(
                symbols,
                file_symbol_name(&alias.name.name, current_namespace.as_deref()),
                "type",
                current_namespace.clone(),
                file_symbol_visibility(current_namespace.as_deref(), alias.exported),
                None,
                alias.name.span,
                source,
            ),
        }
    }
}

fn push_file_symbol_query_entry(
    symbols: &mut Vec<FileSymbolQueryEntry>,
    name: String,
    kind: &str,
    namespace: Option<String>,
    visibility: jett_resolve::scope::DefVisibility,
    signature: Option<String>,
    span: Span,
    source: &str,
) {
    let (line, column) = jett_diagnostics::render::line_col(source, span.start);
    let (end_line, end_column) = jett_diagnostics::render::line_col(source, span.end);
    symbols.push(FileSymbolQueryEntry {
        name,
        kind: kind.to_string(),
        namespace,
        visibility,
        signature,
        line: line as u32,
        column: column as u32,
        end_line: end_line as u32,
        end_column: end_column as u32,
    });
}

fn file_symbol_name(leaf_name: &str, namespace: Option<&str>) -> String {
    namespace
        .map(|namespace| format!("{namespace}.{leaf_name}"))
        .unwrap_or_else(|| leaf_name.to_string())
}

fn file_symbol_visibility(
    namespace: Option<&str>,
    exported: bool,
) -> jett_resolve::scope::DefVisibility {
    if namespace.is_none() || exported {
        jett_resolve::scope::DefVisibility::Public
    } else {
        jett_resolve::scope::DefVisibility::Private
    }
}

fn file_symbol_function_signature(
    leaf_name: &str,
    type_params: &[jett_parser::ast::Ident],
    params: &[Param],
    return_type: Option<&TypeExpr>,
    namespace: Option<&str>,
    file: FileId,
    file_paths: &HashMap<FileId, PathBuf>,
) -> String {
    let signature = signature_query_result(
        leaf_name,
        type_params,
        params,
        return_type,
        namespace,
        file,
        file_paths,
    );
    signature_display(&signature)
}

fn query_builtin_definitions() -> Vec<QueryDefinition> {
    use jett_resolve::scope::DefVisibility;

    let module = Module {
        items: Vec::new(),
        span: Span::new(FileId::new(0), 0, 0),
    };
    let resolve_result = resolve(&module);
    resolve_result
        .scope_table
        .definitions
        .iter()
        .filter(|def| def.visibility == DefVisibility::Public && query_surface_kind(def.kind))
        .map(|def| QueryDefinition {
            name: def.name.clone(),
            kind: def.kind,
            namespace: def.namespace.clone(),
            visibility: def.visibility,
            file_path: "builtin".to_string(),
            line: 0,
            column: 0,
            end_line: 0,
            end_column: 0,
        })
        .collect()
}

fn append_module_query_definitions(
    definitions: &mut Vec<QueryDefinition>,
    module: &Module,
    current_source: Option<&str>,
    file_paths: &HashMap<FileId, PathBuf>,
) {
    use jett_resolve::scope::DefKind;

    let mut current_namespace: Option<String> = None;
    for item in &module.items {
        match item {
            Item::Namespace(ns) => {
                current_namespace = Some(ns.name.name.clone());
                push_query_definition(
                    definitions,
                    ns.name.name.clone(),
                    DefKind::Namespace,
                    None,
                    ns.name.span,
                    current_source,
                    file_paths,
                );
            }
            Item::Function(func) => push_exported_query_definition(
                definitions,
                &func.name.name,
                DefKind::Function,
                current_namespace.as_deref(),
                func.exported,
                func.name.span,
                current_source,
                file_paths,
            ),
            Item::Mutual(block) => {
                for decl in &block.declarations {
                    push_exported_query_definition(
                        definitions,
                        &decl.name.name,
                        DefKind::Function,
                        current_namespace.as_deref(),
                        decl.exported,
                        decl.name.span,
                        current_source,
                        file_paths,
                    );
                }
            }
            Item::Interface(interface) => push_exported_query_definition(
                definitions,
                &interface.name.name,
                DefKind::Interface,
                current_namespace.as_deref(),
                interface.exported,
                interface.name.span,
                current_source,
                file_paths,
            ),
            Item::Struct(strukt) => push_exported_query_definition(
                definitions,
                &strukt.name.name,
                DefKind::Struct,
                current_namespace.as_deref(),
                strukt.exported,
                strukt.name.span,
                current_source,
                file_paths,
            ),
            Item::Bitfield(bitfield) => push_exported_query_definition(
                definitions,
                &bitfield.name.name,
                DefKind::Bitfield,
                current_namespace.as_deref(),
                bitfield.exported,
                bitfield.name.span,
                current_source,
                file_paths,
            ),
            Item::Enum(enm) => push_exported_query_definition(
                definitions,
                &enm.name.name,
                DefKind::Enum,
                current_namespace.as_deref(),
                enm.exported,
                enm.name.span,
                current_source,
                file_paths,
            ),
            Item::Machine(machine) => push_exported_query_definition(
                definitions,
                &machine.name.name,
                DefKind::Machine,
                current_namespace.as_deref(),
                machine.exported,
                machine.name.span,
                current_source,
                file_paths,
            ),
            Item::Actor(actor) => push_exported_query_definition(
                definitions,
                &actor.name.name,
                DefKind::Actor,
                current_namespace.as_deref(),
                actor.exported,
                actor.name.span,
                current_source,
                file_paths,
            ),
            Item::Resource(resource) => push_exported_query_definition(
                definitions,
                &resource.name.name,
                DefKind::Resource,
                current_namespace.as_deref(),
                resource.exported,
                resource.name.span,
                current_source,
                file_paths,
            ),
            Item::TypeAlias(alias) => {
                push_exported_query_definition(
                    definitions,
                    &alias.name.name,
                    DefKind::Type,
                    current_namespace.as_deref(),
                    alias.exported,
                    alias.name.span,
                    current_source,
                    file_paths,
                );
            }
            Item::Implement(_) | Item::VarDecl(_) | Item::Verify(_) | Item::Property(_) => {}
        }
    }
}

fn push_exported_query_definition(
    definitions: &mut Vec<QueryDefinition>,
    leaf_name: &str,
    kind: jett_resolve::scope::DefKind,
    namespace: Option<&str>,
    exported: bool,
    span: Span,
    current_source: Option<&str>,
    file_paths: &HashMap<FileId, PathBuf>,
) {
    if namespace.is_some() && !exported {
        return;
    }

    let name = namespace
        .map(|namespace| format!("{namespace}.{leaf_name}"))
        .unwrap_or_else(|| leaf_name.to_string());
    push_query_definition(
        definitions,
        name,
        kind,
        namespace.map(str::to_string),
        span,
        current_source,
        file_paths,
    );
}

fn push_query_definition(
    definitions: &mut Vec<QueryDefinition>,
    name: String,
    kind: jett_resolve::scope::DefKind,
    namespace: Option<String>,
    span: Span,
    current_source: Option<&str>,
    file_paths: &HashMap<FileId, PathBuf>,
) {
    let (file_path, line, column, end_line, end_column) =
        span_location(current_source, span, file_paths)
            .unwrap_or_else(|| (query_file_path(span.file, file_paths), 0, 0, 0, 0));
    definitions.push(QueryDefinition {
        name,
        kind,
        namespace,
        visibility: jett_resolve::scope::DefVisibility::Public,
        file_path,
        line,
        column,
        end_line,
        end_column,
    });
}

fn query_surface_kind(kind: jett_resolve::scope::DefKind) -> bool {
    use jett_resolve::scope::DefKind;

    matches!(
        kind,
        DefKind::Function
            | DefKind::Interface
            | DefKind::Struct
            | DefKind::Bitfield
            | DefKind::Enum
            | DefKind::Machine
            | DefKind::Actor
            | DefKind::Type
            | DefKind::Constant
            | DefKind::Namespace
    )
}

fn query_file_path(file: FileId, file_paths: &HashMap<FileId, PathBuf>) -> String {
    file_paths
        .get(&file)
        .map(|path| display_query_path(path))
        .unwrap_or_else(|| "builtin".to_string())
}

fn display_query_path(path: &Path) -> String {
    let displayed = path.display().to_string();
    displayed
        .strip_prefix(r"\\?\")
        .unwrap_or(&displayed)
        .to_string()
}

fn best_resolved_definition_at(
    resolve_result: &jett_resolve::ResolveResult,
    file_id: FileId,
    offset: u32,
) -> Option<(u32, jett_resolve::scope::DefId)> {
    let mut best_def: Option<(u32, jett_resolve::scope::DefId)> = None;
    for (span, def_id) in &resolve_result.resolutions {
        if span.file == file_id && span.start <= offset && offset <= span.end {
            let len = span.end - span.start;
            if best_def.is_none() || len < best_def.unwrap().0 {
                best_def = Some((len, *def_id));
            }
        }
    }
    for definition in &resolve_result.scope_table.definitions {
        let span = definition.span;
        if span.file == file_id && span.start <= offset && offset <= span.end {
            let len = span.end - span.start;
            if best_def.is_none() || len < best_def.unwrap().0 {
                best_def = Some((len, definition.id));
            }
        }
    }
    best_def
}

fn span_location(
    current_source: Option<&str>,
    span: Span,
    file_paths: &HashMap<FileId, PathBuf>,
) -> Option<(String, u32, u32, u32, u32)> {
    if span.start == 0 && span.end == 0 {
        return Some(("builtin".to_string(), 0, 0, 0, 0));
    }

    let source = if span.file == FileId::new(0) {
        Cow::Borrowed(current_source?)
    } else {
        Cow::Owned(fs::read_to_string(file_paths.get(&span.file)?).ok()?)
    };
    let (line, column) = jett_diagnostics::render::line_col(&source, span.start);
    let (end_line, end_column) = jett_diagnostics::render::line_col(&source, span.end);
    Some((
        query_file_path(span.file, file_paths),
        line as u32,
        column as u32,
        end_line as u32,
        end_column as u32,
    ))
}

/// Stable text label for a resolved definition kind.
pub fn query_kind_name(kind: jett_resolve::scope::DefKind) -> &'static str {
    use jett_resolve::scope::DefKind;

    match kind {
        DefKind::Function => "function",
        DefKind::Interface => "interface",
        DefKind::Struct => "struct",
        DefKind::Bitfield => "bitfield",
        DefKind::Enum => "enum",
        DefKind::Machine => "machine",
        DefKind::Actor => "actor",
        DefKind::Resource => "resource",
        DefKind::Variable => "variable",
        DefKind::Param => "param",
        DefKind::Type => "type",
        DefKind::Constant => "constant",
        DefKind::Namespace => "namespace",
    }
}

/// Stable text label for a resolved definition visibility.
pub fn query_visibility_name(visibility: jett_resolve::scope::DefVisibility) -> &'static str {
    use jett_resolve::scope::DefVisibility;

    match visibility {
        DefVisibility::Public => "public",
        DefVisibility::Private => "private",
    }
}

/// Stable text label for a completion prefix match kind.
pub fn completion_match_kind_name(match_kind: CompletionMatchKind) -> &'static str {
    match match_kind {
        CompletionMatchKind::EmptyPrefix => "empty_prefix",
        CompletionMatchKind::Exact => "exact",
        CompletionMatchKind::QualifiedPrefix => "qualified_prefix",
        CompletionMatchKind::LeafPrefix => "leaf_prefix",
    }
}

fn support_modules_declare_namespace(modules: &[Module], namespace: &str) -> bool {
    modules.iter().any(|module| {
        module.items.iter().any(|item| match item {
            Item::Namespace(ns) => ns.span.file.is_stdlib() && ns.name.name == namespace,
            _ => false,
        })
    })
}

fn namespace_at_offset(module: &Module, file_id: FileId, offset: u32) -> Option<String> {
    let mut current_namespace = None;
    for item in &module.items {
        if item_file(item) != file_id {
            continue;
        }
        if item_span(item).start > offset {
            break;
        }
        if let Item::Namespace(ns) = item {
            current_namespace = Some(ns.name.name.clone());
        }
    }
    current_namespace
}

fn item_span(item: &Item) -> jett_common::Span {
    match item {
        Item::Namespace(ns) => ns.span,
        Item::Function(func) => func.span,
        Item::Mutual(block) => block.span,
        Item::Interface(interface) => interface.span,
        Item::Implement(block) => block.span,
        Item::Struct(strukt) => strukt.span,
        Item::Bitfield(bitfield) => bitfield.span,
        Item::Enum(enm) => enm.span,
        Item::Machine(machine) => machine.span,
        Item::Actor(actor) => actor.span,
        Item::VarDecl(decl) => decl.span,
        Item::Verify(verify) => verify.span,
        Item::Property(prop) => prop.span,
        Item::Resource(resource) => resource.span,
        Item::TypeAlias(alias) => alias.span,
    }
}

/// Return the byte span of the definition of the symbol at the given (1-based)
/// line and column in `source`.  Returns `None` if no definition is found.
pub fn goto_definition(source: &str, line: u32, col: u32) -> Option<(u32, u32)> {
    let file_id = FileId::new(0);

    let offset = line_col_to_offset(source, line, col)?;

    let mut parse_result = parse(source, file_id);
    if parse_result
        .errors
        .iter()
        .any(|d| d.severity == jett_diagnostics::Severity::Error)
    {
        return None;
    }

    prepend_support_modules(&mut parse_result.module, discover_stdlib_modules());

    let resolve_result = resolve(&parse_result.module);
    let best_def = best_resolved_definition_at(&resolve_result, file_id, offset);

    best_def.and_then(|(_, def_id)| {
        let def_info = resolve_result.scope_table.def(def_id);
        if def_info.span.file == file_id {
            Some((def_info.span.start, def_info.span.end))
        } else {
            None
        }
    })
}

/// Return byte spans for every use of the symbol selected at the given
/// one-based line and column in `source`.
pub fn references_at(source: &str, line: u32, col: u32) -> Vec<(u32, u32)> {
    let file_id = FileId::new(0);
    let Some(offset) = line_col_to_offset(source, line, col) else {
        return Vec::new();
    };

    let mut parse_result = parse(source, file_id);
    if parse_result
        .errors
        .iter()
        .any(|diagnostic| diagnostic.severity == jett_diagnostics::Severity::Error)
    {
        return Vec::new();
    }

    prepend_support_modules(&mut parse_result.module, discover_stdlib_modules());
    let resolve_result = resolve(&parse_result.module);
    let Some((_, target_def_id)) = best_resolved_definition_at(&resolve_result, file_id, offset)
    else {
        return Vec::new();
    };

    let mut references = resolve_result
        .resolutions
        .iter()
        .filter_map(|(span, def_id)| {
            (*def_id == target_def_id && span.file == file_id).then_some((span.start, span.end))
        })
        .collect::<Vec<_>>();
    references.sort_unstable();
    references
}

/// Return the byte bounds of a 1-based logical source line.
///
/// Jett accepts LF, CRLF, and lone CR line endings. The returned range excludes
/// the line ending itself so query columns cannot address either byte of it.
fn source_line_bounds(source: &str, line: u32) -> Option<(usize, usize)> {
    if line == 0 {
        return None;
    }

    let bytes = source.as_bytes();
    let mut current_line = 1_u32;
    let mut line_start = 0_usize;
    let mut index = 0_usize;

    while current_line < line {
        match *bytes.get(index)? {
            b'\n' => {
                index += 1;
                line_start = index;
                current_line += 1;
            }
            b'\r' => {
                index += 1;
                if bytes.get(index) == Some(&b'\n') {
                    index += 1;
                }
                line_start = index;
                current_line += 1;
            }
            _ => index += 1,
        }
    }

    let mut line_end = line_start;
    while let Some(byte) = bytes.get(line_end) {
        if matches!(byte, b'\n' | b'\r') {
            break;
        }
        line_end += 1;
    }
    Some((line_start, line_end))
}

/// Convert a 1-based line+column to a byte offset in `source`.
fn line_col_to_offset(source: &str, line: u32, col: u32) -> Option<u32> {
    if col == 0 {
        return None;
    }

    let (line_start, line_end) = source_line_bounds(source, line)?;
    let line_source = &source[line_start..line_end];
    let column_offset = (col - 1) as usize;
    if column_offset > line_source.chars().count() {
        return None;
    }

    let byte_offset = line_source
        .char_indices()
        .nth(column_offset)
        .map_or(line_source.len(), |(offset, _)| offset);
    u32::try_from(line_start + byte_offset).ok()
}

/// Run the full compilation pipeline on a single file: lex → parse → resolve → typecheck.
/// Does not produce executable output yet — just validates the source.
pub fn build_file(path: &Path) -> BuildResult {
    build_file_with_options(path, BuildOptions::default())
}

/// Run the full compilation pipeline with mode-specific build policy.
pub fn build_file_with_options(path: &Path, options: BuildOptions) -> BuildResult {
    build_file_inner(path, true, options)
}

fn build_file_inner(path: &Path, include_project: bool, options: BuildOptions) -> BuildResult {
    let file_path_str = path.display().to_string();

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
                source: String::new(),
                file_path: file_path_str,
                reflection_metadata: None,
                checked_expression_types: None,
                explicit_comptime_values: None,
            };
        }
    };

    let mut all_diagnostics = Vec::new();

    // Phase 1+2: Lex + Parse (parse internally calls tokenize)
    let mut parse_result = parse_source_with_query(&source, &file_path_str);
    all_diagnostics.extend(parse_result.errors.clone());

    // If there are parse errors, stop here — resolve/typecheck won't produce useful results
    let has_parse_errors = has_error_diagnostics(&all_diagnostics);
    if has_parse_errors {
        return BuildResult {
            has_errors: true,
            diagnostics: all_diagnostics,
            source,
            file_path: file_path_str,
            reflection_metadata: None,
            checked_expression_types: None,
            explicit_comptime_values: None,
        };
    }

    // Multi-file: prepend stdlib and sibling project modules so
    // resolver/typechecker can see cross-file definitions (functions, types,
    // etc.).
    let mut support_modules = discover_stdlib_modules_with_diagnostics();
    if include_project {
        support_modules.extend(discover_project_modules_with_diagnostics(path));
    }
    all_diagnostics.extend(support_modules.diagnostics);
    if has_error_diagnostics(&all_diagnostics) {
        return BuildResult {
            has_errors: true,
            diagnostics: all_diagnostics,
            source,
            file_path: file_path_str,
            reflection_metadata: None,
            checked_expression_types: None,
            explicit_comptime_values: None,
        };
    }
    prepend_support_modules(&mut parse_result.module, support_modules.modules);

    // Phase 3: Resolve names
    let resolve_result = resolve(&parse_result.module);
    all_diagnostics.extend(resolve_result.diagnostics.clone());

    let has_resolve_errors = has_error_diagnostics(&all_diagnostics);
    if has_resolve_errors {
        return BuildResult {
            has_errors: true,
            diagnostics: all_diagnostics,
            source,
            file_path: file_path_str,
            reflection_metadata: None,
            checked_expression_types: None,
            explicit_comptime_values: None,
        };
    }

    // Phase 4: Type check
    let check_result = check_with_options(
        &parse_result.module,
        &resolve_result,
        CheckOptions {
            release: options.release,
        },
    );
    all_diagnostics.extend(check_result.diagnostics.clone());

    let has_typecheck_errors = has_error_diagnostics(&all_diagnostics);
    if has_typecheck_errors {
        return BuildResult {
            has_errors: true,
            diagnostics: all_diagnostics,
            source,
            file_path: file_path_str,
            reflection_metadata: None,
            checked_expression_types: None,
            explicit_comptime_values: None,
        };
    }

    // Phase 5: Execute verify blocks at compile time
    let reflection_metadata = check_result.reflection_metadata.clone();
    let checked_expression_types = Arc::new(expression_type_names(&check_result));
    let (explicit_comptime_values, comptime_diagnostics) = evaluate_explicit_comptime_expressions(
        &parse_result.module,
        reflection_metadata.clone(),
        checked_expression_types.clone(),
    );
    all_diagnostics.extend(comptime_diagnostics);
    let verify_diagnostics = run_verify_blocks_with_metadata_and_expression_types(
        &parse_result.module,
        check_result.reflection_metadata,
        checked_expression_types.clone(),
    );
    all_diagnostics.extend(verify_diagnostics);

    let has_errors = has_error_diagnostics(&all_diagnostics);

    BuildResult {
        has_errors,
        diagnostics: all_diagnostics,
        source,
        file_path: file_path_str,
        reflection_metadata: Some(reflection_metadata),
        checked_expression_types: Some(checked_expression_types),
        explicit_comptime_values: Some(Arc::new(explicit_comptime_values)),
    }
}

/// Register all items from a parsed module into an interpreter.
fn register_module_items(
    interp: &mut jett_comptime::interpreter::Interpreter,
    module: &jett_parser::ast::Module,
) {
    interp.register_module(module);
}

fn item_file(item: &Item) -> FileId {
    match item {
        Item::Namespace(ns) => ns.span.file,
        Item::Function(func) => func.span.file,
        Item::Mutual(block) => block.span.file,
        Item::Interface(interface) => interface.span.file,
        Item::Implement(block) => block.span.file,
        Item::Struct(strukt) => strukt.span.file,
        Item::Bitfield(bitfield) => bitfield.span.file,
        Item::Enum(enm) => enm.span.file,
        Item::Machine(machine) => machine.span.file,
        Item::Actor(actor) => actor.span.file,
        Item::VarDecl(decl) => decl.span.file,
        Item::Verify(verify) => verify.span.file,
        Item::Property(prop) => prop.span.file,
        Item::Resource(resource) => resource.span.file,
        Item::TypeAlias(alias) => alias.span.file,
    }
}

fn has_error_diagnostics(diagnostics: &[Diagnostic]) -> bool {
    diagnostics
        .iter()
        .any(|d| d.severity == jett_diagnostics::Severity::Error)
}

fn diagnostics_have_source_context(
    diagnostics: &[Diagnostic],
    sources: &[QueryDiagnosticSource],
) -> bool {
    diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.severity == jett_diagnostics::Severity::Error)
        .all(|diagnostic| {
            sources
                .iter()
                .any(|source| source.file_id == diagnostic.span.file)
        })
}

fn query_diagnostic_sources(
    primary_file: FileId,
    primary_source: &str,
    primary_path: &str,
    support_modules: &DiscoveredModules,
    diagnostics: &[Diagnostic],
) -> Vec<QueryDiagnosticSource> {
    let mut sources = vec![QueryDiagnosticSource {
        file_id: primary_file,
        source: primary_source.to_string(),
        file_path: primary_path.to_string(),
    }];
    let mut support_file_ids = Vec::new();
    for diagnostic in diagnostics {
        support_file_ids.push(diagnostic.span.file);
        support_file_ids.extend(diagnostic.labels.iter().map(|label| label.span.file));
        if let Some(fix) = &diagnostic.suggested_fix {
            support_file_ids.push(fix.span.file);
        }
    }
    support_file_ids.sort_by_key(|file_id| file_id.index());
    support_file_ids.dedup_by_key(|file_id| file_id.index());
    for file_id in support_file_ids {
        if file_id == primary_file {
            continue;
        }
        let Some(source) = support_modules.sources.get(&file_id) else {
            continue;
        };
        let Some(file_path) = support_modules.files.get(&file_id) else {
            continue;
        };
        sources.push(QueryDiagnosticSource {
            file_id,
            source: source.clone(),
            file_path: display_query_path(file_path),
        });
    }
    sources
}

fn expression_type_names(check_result: &CheckResult) -> HashMap<Span, String> {
    check_result
        .type_map
        .iter()
        .map(|(span, ty_id)| (*span, check_result.interner.type_name(*ty_id)))
        .collect()
}

fn update_current_namespace(
    item: &Item,
    current_file: &mut Option<FileId>,
    current_namespace: &mut Option<String>,
) {
    let file = item_file(item);
    if current_file.is_some_and(|current| current != file) {
        *current_namespace = None;
    }
    *current_file = Some(file);

    if let Item::Namespace(ns) = item {
        *current_namespace = Some(ns.name.name.clone());
    }
}

fn find_main_function(module: &Module) -> Option<(Option<String>, &FunctionDef)> {
    let mut current_file = None;
    let mut current_namespace = None;

    for item in &module.items {
        update_current_namespace(item, &mut current_file, &mut current_namespace);
        if let Item::Function(func) = item
            && func.name.name == "main"
        {
            return Some((current_namespace.clone(), func));
        }
    }

    None
}

fn prepend_support_modules(module: &mut Module, support_modules: Vec<Module>) {
    if support_modules.is_empty() {
        return;
    }

    let mut merged_items = Vec::new();
    for support in support_modules {
        merged_items.extend(support.items);
    }
    merged_items.append(&mut module.items);
    module.items = merged_items;
}

/// Discover and parse compiler-shipped stdlib modules.
fn discover_stdlib_modules() -> Vec<Module> {
    discover_stdlib_modules_with_diagnostics().modules
}

fn discover_stdlib_modules_with_diagnostics() -> DiscoveredModules {
    discover_modules_in_dir(&stdlib_root(), None, STDLIB_FILE_ID_START, "stdlib")
}

fn stdlib_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("stdlib")
}

/// Discover and parse all sibling .jett files in the project (if a jett.proj exists).
/// Returns parsed modules for files other than the entry file.
fn discover_project_modules(entry_path: &Path) -> Vec<Module> {
    discover_project_modules_with_diagnostics(entry_path).modules
}

fn discover_project_modules_with_diagnostics(entry_path: &Path) -> DiscoveredModules {
    let canon = entry_path.canonicalize().ok();
    let project_root = find_project_root(entry_path).ok();
    let Some(root) = project_root else {
        return DiscoveredModules {
            modules: Vec::new(),
            diagnostics: Vec::new(),
            files: HashMap::new(),
            sources: HashMap::new(),
        };
    };
    discover_modules_in_dir(&root, canon.as_deref(), 1, "project")
}

fn discover_query_project_modules_with_diagnostics(start_dir: &Path) -> DiscoveredModules {
    let Ok(root) = find_project_root(start_dir) else {
        return DiscoveredModules {
            modules: Vec::new(),
            diagnostics: Vec::new(),
            files: HashMap::new(),
            sources: HashMap::new(),
        };
    };
    discover_modules_in_dir(&root, None, 1, "project")
}

fn discover_modules_in_dir(
    root: &Path,
    skip_canon: Option<&Path>,
    start_file_id: u32,
    module_kind: &str,
) -> DiscoveredModules {
    let mut files = Vec::new();
    if let Err(err) = collect_jett_files(root, &mut files) {
        return DiscoveredModules {
            modules: Vec::new(),
            diagnostics: vec![Diagnostic::error(
                0,
                format!(
                    "failed to scan {module_kind} modules in {}: {err}",
                    root.display()
                ),
                jett_common::Span::new(FileId::new(start_file_id), 0, 0),
            )],
            files: HashMap::new(),
            sources: HashMap::new(),
        };
    }
    if module_kind == "stdlib" {
        // Foundational root modules must be declared before nested namespace
        // fragments (notably json/), while fragments keep lexical order.
        files.sort_by(|left, right| {
            let left_depth = left
                .strip_prefix(root)
                .map_or(usize::MAX, |path| path.components().count());
            let right_depth = right
                .strip_prefix(root)
                .map_or(usize::MAX, |path| path.components().count());
            left_depth.cmp(&right_depth).then_with(|| left.cmp(right))
        });
    } else {
        files.sort();
    }

    let mut modules = Vec::new();
    let mut diagnostics = Vec::new();
    let mut module_files = HashMap::new();
    let mut module_sources = HashMap::new();
    for (idx, file_path) in files.iter().enumerate() {
        // Skip the entry file when parsing project siblings.
        let should_skip = skip_canon
            .map(|skip| file_path.canonicalize().ok().as_deref() == Some(skip))
            .unwrap_or(false);
        if should_skip {
            continue;
        }
        let file_id = FileId::new(start_file_id + idx as u32);
        let source = match fs::read_to_string(file_path) {
            Ok(source) => source,
            Err(err) => {
                diagnostics.push(Diagnostic::error(
                    0,
                    format!(
                        "failed to read {module_kind} module {}: {err}",
                        file_path.display()
                    ),
                    jett_common::Span::new(file_id, 0, 0),
                ));
                continue;
            }
        };
        let display_path = file_path
            .canonicalize()
            .unwrap_or_else(|_| file_path.clone());
        module_files.insert(file_id, display_path);
        module_sources.insert(file_id, source.clone());
        let parsed = parse(&source, file_id);
        if has_error_diagnostics(&parsed.errors) {
            for mut diagnostic in parsed.errors {
                if diagnostic.severity == jett_diagnostics::Severity::Error {
                    diagnostic.message = format!(
                        "failed to parse {module_kind} module {}: {}",
                        file_path.display(),
                        diagnostic.message
                    );
                    diagnostics.push(diagnostic);
                }
            }
        } else {
            modules.push(parsed.module);
        }
    }
    DiscoveredModules {
        modules,
        diagnostics,
        files: module_files,
        sources: module_sources,
    }
}

/// Run a .jett file using the tree-walking interpreter.
/// First validates (lex → parse → resolve → typecheck → verify), then executes main().
/// If a jett.proj exists, also loads sibling .jett files so cross-file calls work.
pub fn run_file(path: &Path) -> Result<(), String> {
    run_file_with_options(
        path,
        RunOptions {
            capture_stdout: false,
            emit_runtime_debug: true,
            random_test_samples: None,
            clock_test_samples: None,
            environment_test_snapshot: None,
        },
    )
    .map(|_| ())
}

/// Run a .jett file and capture runtime stdout produced by `Stdout.write`,
/// `print`, and `println`.
pub fn run_file_capture_stdout(path: &Path) -> Result<String, String> {
    run_file_with_options(
        path,
        RunOptions {
            capture_stdout: true,
            emit_runtime_debug: false,
            random_test_samples: None,
            clock_test_samples: None,
            environment_test_snapshot: None,
        },
    )
    .map(|output| output.stdout)
}

/// Run a .jett file and capture stdout plus trace/breakpoint debug lines.
pub fn run_file_capture_output(path: &Path) -> Result<RunOutput, String> {
    run_file_with_options(
        path,
        RunOptions {
            capture_stdout: true,
            emit_runtime_debug: false,
            random_test_samples: None,
            clock_test_samples: None,
            environment_test_snapshot: None,
        },
    )
}

/// Run with a backend-neutral scripted Random provider for deterministic tests.
pub fn run_file_with_random_test_samples(
    path: &Path,
    samples: Vec<RandomTestSample>,
) -> Result<(), String> {
    run_file_with_options(
        path,
        RunOptions {
            capture_stdout: false,
            emit_runtime_debug: false,
            random_test_samples: Some(samples),
            clock_test_samples: None,
            environment_test_snapshot: None,
        },
    )
    .map(|_| ())
}

/// Run with scripted Random samples and capture stdout for deterministic tests.
pub fn run_file_capture_stdout_with_random_test_samples(
    path: &Path,
    samples: Vec<RandomTestSample>,
) -> Result<String, String> {
    run_file_with_options(
        path,
        RunOptions {
            capture_stdout: true,
            emit_runtime_debug: false,
            random_test_samples: Some(samples),
            clock_test_samples: None,
            environment_test_snapshot: None,
        },
    )
    .map(|output| output.stdout)
}

/// Run with a backend-neutral scripted wall clock for deterministic tests.
pub fn run_file_with_clock_test_samples(
    path: &Path,
    samples: Vec<ClockTestSample>,
) -> Result<(), String> {
    run_file_with_options(
        path,
        RunOptions {
            capture_stdout: false,
            emit_runtime_debug: false,
            random_test_samples: None,
            clock_test_samples: Some(samples),
            environment_test_snapshot: None,
        },
    )
    .map(|_| ())
}

/// Run with scripted wall-clock samples and capture stdout.
pub fn run_file_capture_stdout_with_clock_test_samples(
    path: &Path,
    samples: Vec<ClockTestSample>,
) -> Result<String, String> {
    run_file_with_options(
        path,
        RunOptions {
            capture_stdout: true,
            emit_runtime_debug: false,
            random_test_samples: None,
            clock_test_samples: Some(samples),
            environment_test_snapshot: None,
        },
    )
    .map(|output| output.stdout)
}

/// Run with an isolated Environment launch snapshot and capture stdout.
pub fn run_file_capture_stdout_with_environment_test_snapshot(
    path: &Path,
    snapshot: EnvironmentTestSnapshot,
) -> Result<String, String> {
    run_file_with_options(
        path,
        RunOptions {
            capture_stdout: true,
            emit_runtime_debug: false,
            random_test_samples: None,
            clock_test_samples: None,
            environment_test_snapshot: Some(snapshot),
        },
    )
    .map(|output| output.stdout)
}

/// Run with an isolated Environment launch snapshot.
pub fn run_file_with_environment_test_snapshot(
    path: &Path,
    snapshot: EnvironmentTestSnapshot,
) -> Result<(), String> {
    run_file_with_options(
        path,
        RunOptions {
            capture_stdout: false,
            emit_runtime_debug: false,
            random_test_samples: None,
            clock_test_samples: None,
            environment_test_snapshot: Some(snapshot),
        },
    )
    .map(|_| ())
}

fn run_file_with_options(path: &Path, options: RunOptions) -> Result<RunOutput, String> {
    let thread_path = path.to_path_buf();
    let fallback_path = thread_path.clone();
    let fallback_options = options.clone();
    match thread::Builder::new()
        .name("jett-runtime".to_string())
        .stack_size(RUNTIME_STACK_SIZE)
        .spawn(move || run_file_inner(&thread_path, options))
    {
        Ok(handle) => match handle.join() {
            Ok(result) => result,
            Err(payload) => std::panic::resume_unwind(payload),
        },
        Err(_) => run_file_inner(&fallback_path, fallback_options),
    }
}

fn run_file_inner(path: &Path, options: RunOptions) -> Result<RunOutput, String> {
    let build = build_file(path);

    if build.has_errors {
        let errors: Vec<String> = build
            .diagnostics
            .iter()
            .filter(|d| d.severity == jett_diagnostics::Severity::Error)
            .map(|d| format!("{}: {}", d.code, d.message))
            .collect();
        return Err(format!(
            "cannot run — compilation errors:\n{}",
            errors.join("\n")
        ));
    }

    // Parse again to get the module for interpretation
    let source = fs::read_to_string(path)
        .map_err(|e| format!("failed to read {}: {}", path.display(), e))?;
    let file_id = FileId::new(0);
    let parse_result = parse(&source, file_id);
    let module = parse_result.module;

    let Some((main_namespace, main_func)) = find_main_function(&module) else {
        return Err("runtime error: no `main` function found".to_string());
    };

    let main_args = default_runtime_args_for_main(main_func)?;

    use jett_comptime::interpreter::Interpreter;
    let mut interp = if options.emit_runtime_debug {
        Interpreter::new_runtime()
    } else {
        Interpreter::new()
    };
    if main_func
        .params
        .iter()
        .any(|param| type_expr_name(&param.ty) == "Random")
    {
        if let Some(samples) = options.random_test_samples.clone() {
            interp.set_random_test_samples(samples);
        } else {
            interp.initialize_random_provider()?;
        }
    }
    if main_func
        .params
        .iter()
        .any(|param| type_expr_name(&param.ty) == "Clock")
    {
        if let Some(samples) = options.clock_test_samples.clone() {
            interp.set_clock_test_samples(samples);
        } else {
            interp.initialize_clock_provider();
        }
    }
    if main_func
        .params
        .iter()
        .any(|param| type_expr_name(&param.ty) == "Environment")
    {
        if let Some(snapshot) = options.environment_test_snapshot.clone() {
            interp
                .set_environment_test_snapshot(snapshot)
                .map_err(|error| format!("runtime error: {error}"))?;
        } else {
            interp
                .initialize_environment_provider()
                .map_err(|error| format!("runtime error: {error}"))?;
        }
    }
    if let Some(metadata) = build.reflection_metadata.clone() {
        interp.set_reflection_metadata(metadata);
    }
    if let Some(expression_types) = build.checked_expression_types.clone() {
        interp.set_checked_expression_types(expression_types);
    }
    if let Some(values) = build.explicit_comptime_values.clone() {
        interp.set_explicit_comptime_values(values);
    }
    if options.capture_stdout {
        interp.enable_stdout_capture();
    }

    // Register compiler-shipped stdlib modules before project and entry files.
    for module in discover_stdlib_modules() {
        register_module_items(&mut interp, &module);
    }

    // Register items from sibling project files first (so they're available to main file).
    let sibling_modules = discover_project_modules(path);
    for module in &sibling_modules {
        register_module_items(&mut interp, module);
    }

    // Register items from the entry file (may override sibling definitions).
    register_module_items(&mut interp, &module);

    // Call main()
    match interp.call_function_in_namespace(main_namespace.as_deref(), "main", main_args) {
        Ok(_) => Ok(RunOutput {
            stdout: interp.take_stdout_output(),
            debug_output: interp.take_debug_output(),
        }),
        Err(e) => Err(format!("runtime error: {}", e)),
    }
}

fn default_runtime_args_for_main(main: &FunctionDef) -> Result<Vec<Value>, String> {
    main.params
        .iter()
        .map(default_runtime_arg_for_param)
        .collect()
}

fn default_runtime_arg_for_param(param: &Param) -> Result<Value, String> {
    if type_expr_name(&param.ty) == "Environment" {
        return Ok(Value::Capability("Environment".to_string()));
    }
    if type_expr_is_capability(&param.ty) {
        return Ok(Value::Nothing);
    }

    Err(format!(
        "runtime error: `main` parameter `{}` has unsupported type `{}`; only zero-argument or capability-only `main` functions can be run right now",
        param.name.name,
        type_expr_name(&param.ty)
    ))
}

fn type_expr_is_capability(ty: &TypeExpr) -> bool {
    match ty {
        TypeExpr::Named(ident) => matches!(
            ident.name.as_str(),
            "Stdout"
                | "Stderr"
                | "Stdin"
                | "Filesystem"
                | "Network"
                | "Clock"
                | "Random"
                | "Process"
                | "Environment"
        ),
        TypeExpr::View(inner, _) => type_expr_is_capability(inner),
        TypeExpr::StateQualified(inner, _, _) => type_expr_is_capability(inner),
        TypeExpr::Generic(_, _, _) => false,
        TypeExpr::Function(_, _, _) => false,
    }
}

fn type_expr_name(ty: &TypeExpr) -> String {
    match ty {
        TypeExpr::Named(ident) => ident.name.clone(),
        TypeExpr::Generic(name, args, _) => {
            let args: Vec<String> = args.iter().map(type_expr_name).collect();
            format!("{}[{}]", name.name, args.join(", "))
        }
        TypeExpr::View(inner, _) => format!("view {}", type_expr_name(inner)),
        TypeExpr::StateQualified(inner, state, _) => {
            format!("{} at {}", type_expr_name(inner), state.name)
        }
        TypeExpr::Function(params, ret, _) => {
            let params: Vec<String> = params.iter().map(type_expr_name).collect();
            format!(
                "function({}) returns {}",
                params.join(", "),
                type_expr_name(ret)
            )
        }
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

// ---------------------------------------------------------------------------
// Test
// ---------------------------------------------------------------------------

/// A single block result in a test run.
pub struct TestBlockResult {
    pub name: String,
    pub passed: bool,
    pub error: Option<String>,
    pub is_property: bool,
    pub iterations: Option<usize>,
    pub line: u32,
    pub column: u32,
    pub end_line: u32,
    pub end_column: u32,
}

/// Result of running `jett test` on a single file.
pub struct TestResult {
    pub total: usize,
    pub passed: usize,
    pub failed: usize,
    /// The file that was tested.
    pub file_path: String,
    /// Per-block results.
    pub blocks: Vec<TestBlockResult>,
}

/// Result of running `jett test` across an entire project.
pub struct ProjectTestResult {
    pub total_files: usize,
    pub total_blocks: usize,
    pub total_passed: usize,
    pub total_failed: usize,
    /// Per-file results.
    pub file_results: Vec<TestResult>,
}

/// One source file included in a generated bundle.
pub struct BundleFileResult {
    pub path: String,
    pub start_line: u32,
    pub end_line: u32,
}

/// Result of bundling a project into one distributable file.
pub struct BundleResult {
    pub project_root: String,
    pub output_path: String,
    pub files: Vec<BundleFileResult>,
}

/// A bundle failure, optionally retaining validation or ordering diagnostics
/// for structured agent output.
pub struct BundleError {
    message: String,
    details: Option<BundleErrorDetails>,
}

enum BundleErrorDetails {
    Validation(BuildResult),
    Ordering(BuildResult),
}

impl BundleError {
    fn from_validation(validation: BuildResult) -> Option<Self> {
        if !validation.has_errors {
            return None;
        }
        let errors = error_messages_from_diagnostics(&validation.diagnostics);
        Some(Self {
            message: format!("candidate bundle failed validation:\n{}", errors.join("\n")),
            details: Some(BundleErrorDetails::Validation(validation)),
        })
    }

    pub fn validation_result(&self) -> Option<&BuildResult> {
        match &self.details {
            Some(BundleErrorDetails::Validation(validation)) => Some(validation),
            _ => None,
        }
    }

    pub fn diagnostic_result(&self) -> Option<&BuildResult> {
        match &self.details {
            Some(BundleErrorDetails::Validation(result) | BundleErrorDetails::Ordering(result)) => {
                Some(result)
            }
            None => None,
        }
    }

    pub fn kind_name(&self) -> Option<&'static str> {
        match &self.details {
            Some(BundleErrorDetails::Validation(_)) => Some("validation"),
            Some(BundleErrorDetails::Ordering(_)) => Some("ordering"),
            None => None,
        }
    }
}

impl From<String> for BundleError {
    fn from(message: String) -> Self {
        Self {
            message,
            details: None,
        }
    }
}

impl std::fmt::Display for BundleError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::fmt::Debug for BundleError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(self, f)
    }
}

impl std::error::Error for BundleError {}

/// Parse a .jett file and run all verify blocks, reporting per-block results.
pub fn test_file(path: &Path) -> Result<TestResult, String> {
    let source = fs::read_to_string(path)
        .map_err(|e| format!("failed to read {}: {}", path.display(), e))?;

    let file_id = FileId::new(0);
    let mut parse_result = parse(&source, file_id);

    // If there are parse errors, report and bail.
    let has_parse_errors = parse_result
        .errors
        .iter()
        .any(|d| d.severity == jett_diagnostics::Severity::Error);
    if has_parse_errors {
        let msgs: Vec<String> = parse_result
            .errors
            .iter()
            .filter(|d| d.severity == jett_diagnostics::Severity::Error)
            .map(|d| format!("{}: {}", d.code, d.message))
            .collect();
        return Err(format!("parse errors:\n{}", msgs.join("\n")));
    }

    let mut support_modules = discover_stdlib_modules_with_diagnostics();
    support_modules.extend(discover_project_modules_with_diagnostics(path));
    let support_errors = error_messages_from_diagnostics(&support_modules.diagnostics);
    if !support_errors.is_empty() {
        return Err(format!(
            "support parse errors:\n{}",
            support_errors.join("\n")
        ));
    }
    strip_test_items_from_support_modules(&mut support_modules.modules);
    prepend_support_modules(&mut parse_result.module, support_modules.modules);

    let resolve_result = resolve(&parse_result.module);
    let resolve_errors = error_messages_from_diagnostics(&resolve_result.diagnostics);
    if !resolve_errors.is_empty() {
        return Err(format!("resolution errors:\n{}", resolve_errors.join("\n")));
    }

    let check_result = check(&parse_result.module, &resolve_result);
    let type_errors = error_messages_from_diagnostics(&check_result.diagnostics);
    if !type_errors.is_empty() {
        return Err(format!("type errors:\n{}", type_errors.join("\n")));
    }

    let checked_expression_types = Arc::new(expression_type_names(&check_result));
    let results = run_verify_blocks_detailed_with_metadata_and_expression_types(
        &parse_result.module,
        Some(check_result.reflection_metadata),
        Some(checked_expression_types),
    );

    let total = results.len();
    let passed = results.iter().filter(|r| r.passed).count();
    let failed = total - passed;

    let blocks = results
        .into_iter()
        .map(|r| {
            let (line, column) = jett_diagnostics::render::line_col(&source, r.span.start);
            let (end_line, end_column) = jett_diagnostics::render::line_col(&source, r.span.end);
            TestBlockResult {
                name: r.name,
                passed: r.passed,
                error: r.error,
                is_property: r.is_property,
                iterations: r.iterations,
                line: line as u32,
                column: column as u32,
                end_line: end_line as u32,
                end_column: end_column as u32,
            }
        })
        .collect();

    Ok(TestResult {
        total,
        passed,
        failed,
        file_path: path.display().to_string(),
        blocks,
    })
}

fn strip_test_items_from_support_modules(modules: &mut [Module]) {
    for module in modules {
        module
            .items
            .retain(|item| !matches!(item, Item::Verify(_) | Item::Property(_)));
    }
}

fn error_messages_from_diagnostics(diagnostics: &[Diagnostic]) -> Vec<String> {
    diagnostics
        .iter()
        .filter(|d| d.severity == jett_diagnostics::Severity::Error)
        .map(|d| format!("{}: {}", d.code, d.message))
        .collect()
}

fn normalize_path_lexically(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Prefix(prefix) => normalized.push(prefix.as_os_str()),
            Component::RootDir => normalized.push(component.as_os_str()),
            Component::CurDir => {}
            Component::ParentDir => match normalized.components().next_back() {
                Some(Component::Normal(_)) => {
                    normalized.pop();
                }
                Some(Component::RootDir | Component::Prefix(_)) => {}
                _ => normalized.push(component.as_os_str()),
            },
            Component::Normal(_) => normalized.push(component.as_os_str()),
        }
    }
    normalized
}

/// Resolve the existing portion of a path before normalizing missing suffixes.
/// This preserves symlink identity while allowing aliases such as
/// `missing/../output.jett` to identify an existing output.
fn output_path_identity(path: &Path) -> PathBuf {
    if let Ok(canonical) = path.canonicalize() {
        return canonical;
    }

    let mut ancestor = path.to_path_buf();
    let mut suffix = Vec::new();
    while let Some(component) = ancestor.components().next_back() {
        if matches!(component, Component::Prefix(_) | Component::RootDir) {
            break;
        }
        suffix.push(component.as_os_str().to_os_string());
        if !ancestor.pop() {
            break;
        }

        let canonical_ancestor = if ancestor.as_os_str().is_empty() {
            Path::new(".").canonicalize()
        } else {
            ancestor.canonicalize()
        };
        if let Ok(mut identity) = canonical_ancestor {
            for component in suffix.iter().rev() {
                identity.push(component);
            }
            let normalized = normalize_path_lexically(&identity);
            return normalized.canonicalize().unwrap_or(normalized);
        }
    }

    let normalized = normalize_path_lexically(path);
    normalized.canonicalize().unwrap_or(normalized)
}

#[cfg(unix)]
fn existing_paths_share_file_identity(left: &Path, right: &Path) -> bool {
    use std::os::unix::fs::MetadataExt;

    let Ok(left_metadata) = fs::metadata(left) else {
        return false;
    };
    let Ok(right_metadata) = fs::metadata(right) else {
        return false;
    };

    left_metadata.dev() == right_metadata.dev() && left_metadata.ino() == right_metadata.ino()
}

#[cfg(windows)]
fn existing_paths_share_file_identity(left: &Path, right: &Path) -> bool {
    same_file::is_same_file(left, right).unwrap_or(false)
}

#[cfg(not(any(unix, windows)))]
fn existing_paths_share_file_identity(_left: &Path, _right: &Path) -> bool {
    false
}

/// Discover all `.jett` files under a project root (walks up from `start_dir`
/// to find `jett.proj`, then collects all `.jett` files in the project) and
/// run verify blocks in each one.
pub fn test_project(start_dir: &Path) -> Result<ProjectTestResult, String> {
    let project_dir = find_project_root(start_dir)?;
    let mut files = Vec::new();
    collect_jett_files(&project_dir, &mut files)
        .map_err(|e| format!("error scanning project: {e}"))?;

    if files.is_empty() {
        return Err(format!(
            "no .jett files found in project at {}",
            project_dir.display()
        ));
    }

    files.sort();

    let mut file_results = Vec::new();
    for file_path in &files {
        file_results.push(test_file(file_path)?);
    }

    let total_files = file_results.len();
    let total_blocks: usize = file_results.iter().map(|r| r.total).sum();
    let total_passed: usize = file_results.iter().map(|r| r.passed).sum();
    let total_failed: usize = file_results.iter().map(|r| r.failed).sum();

    Ok(ProjectTestResult {
        total_files,
        total_blocks,
        total_passed,
        total_failed,
        file_results,
    })
}

/// Bundle all project `.jett` files into a single file, then validate it before
/// writing the output path.
pub fn bundle_project(start_dir: &Path, output: &Path) -> Result<BundleResult, String> {
    bundle_project_detailed(start_dir, output).map_err(|error| error.to_string())
}

struct BundleSourceFile {
    path: PathBuf,
    source: String,
    module: Module,
    file_id: FileId,
}

fn bundle_dependency_order(
    files: &[BundleSourceFile],
    project_dir: &Path,
) -> Result<Vec<usize>, BundleError> {
    let mut merged = Module {
        items: Vec::new(),
        span: Span::new(FileId::new(0), 0, 0),
    };
    for file in files {
        merged.items.extend(file.module.items.clone());
    }

    let resolved = resolve(&merged);
    let file_indices: HashMap<FileId, usize> = files
        .iter()
        .enumerate()
        .map(|(index, file)| (file.file_id, index))
        .collect();
    let mut dependencies = vec![HashSet::new(); files.len()];
    let mut indegrees = vec![0_usize; files.len()];

    let mut add_dependency = |definition_file: FileId, reference_file: FileId| {
        let (Some(&definition_index), Some(&reference_index)) = (
            file_indices.get(&definition_file),
            file_indices.get(&reference_file),
        ) else {
            return;
        };
        if definition_index != reference_index
            && dependencies[definition_index].insert(reference_index)
        {
            indegrees[reference_index] += 1;
        }
    };

    // Successful resolver references provide the canonical definition-to-use
    // relationship. Forward references are deliberately absent from this map,
    // so recover their definition spans from E0205's compiler-owned label.
    for (reference_span, definition_id) in &resolved.resolutions {
        let definition = resolved.scope_table.def(*definition_id);
        add_dependency(definition.span.file, reference_span.file);
    }
    for diagnostic in &resolved.diagnostics {
        if diagnostic.code.code() == 205
            && let Some(definition) = diagnostic.labels.first()
        {
            add_dependency(definition.span.file, diagnostic.span.file);
        }
    }

    let mut ready: BTreeSet<usize> = indegrees
        .iter()
        .enumerate()
        .filter_map(|(index, indegree)| (*indegree == 0).then_some(index))
        .collect();
    let mut ordered = Vec::with_capacity(files.len());
    while let Some(index) = ready.pop_first() {
        ordered.push(index);
        let mut consumers: Vec<_> = dependencies[index].iter().copied().collect();
        consumers.sort_unstable();
        for consumer in consumers {
            indegrees[consumer] -= 1;
            if indegrees[consumer] == 0 {
                ready.insert(consumer);
            }
        }
    }

    if ordered.len() == files.len() {
        return Ok(ordered);
    }

    let cycle_files: Vec<_> = (0..files.len())
        .filter(|&index| bundle_file_is_in_cycle(index, &dependencies))
        .map(|index| {
            display_query_path(
                files[index]
                    .path
                    .strip_prefix(project_dir)
                    .unwrap_or(&files[index].path),
            )
        })
        .collect();
    let message = format!(
        "bundle ordering cycle requires declaration interleaving between: {}. Extract shared declarations into an earlier file",
        cycle_files.join(", ")
    );
    Err(bundle_ordering_error(
        message,
        String::new(),
        project_dir.display().to_string(),
        Span::new(FileId::new(0), 0, 0),
    ))
}

fn bundle_ordering_error(
    message: String,
    source: String,
    file_path: String,
    span: Span,
) -> BundleError {
    let diagnostic = Diagnostic::error(0, &message, span);
    BundleError {
        message,
        details: Some(BundleErrorDetails::Ordering(BuildResult {
            diagnostics: vec![diagnostic],
            has_errors: true,
            source,
            file_path,
            reflection_metadata: None,
            checked_expression_types: None,
            explicit_comptime_values: None,
        })),
    }
}

fn validate_bundle_namespace_boundaries(
    files: &[BundleSourceFile],
    file_order: &[usize],
    project_dir: &Path,
) -> Result<(), BundleError> {
    let mut bundled_namespace = None;
    for &index in file_order {
        let file = &files[index];
        let mut source_namespace = None;
        for item in &file.module.items {
            if let Item::Namespace(namespace) = item {
                source_namespace = Some(namespace.name.name.as_str());
                bundled_namespace = source_namespace;
            } else if source_namespace != bundled_namespace {
                let relative = file.path.strip_prefix(project_dir).unwrap_or(&file.path);
                let message = format!(
                    "bundle ordering cannot preserve root-namespace declarations in {} after namespace `{}`; add an explicit namespace to that file or move its root declarations before namespaced files",
                    display_query_path(relative),
                    bundled_namespace.expect("namespace mismatch should retain a namespace")
                );
                return Err(bundle_ordering_error(
                    message,
                    file.source.clone(),
                    file.path.display().to_string(),
                    Span::new(file.file_id, 0, 0),
                ));
            }
        }
    }
    Ok(())
}

fn bundle_file_is_in_cycle(start: usize, dependencies: &[HashSet<usize>]) -> bool {
    let mut pending: Vec<_> = dependencies[start].iter().copied().collect();
    let mut visited = HashSet::new();
    while let Some(index) = pending.pop() {
        if index == start {
            return true;
        }
        if visited.insert(index) {
            pending.extend(dependencies[index].iter().copied());
        }
    }
    false
}

/// Bundle a project while retaining candidate-validation diagnostics on error.
pub fn bundle_project_detailed(
    start_dir: &Path,
    output: &Path,
) -> Result<BundleResult, BundleError> {
    let project_dir = find_project_root(start_dir)?;
    let output_abs = if output.is_absolute() {
        output.to_path_buf()
    } else {
        project_dir.join(output)
    };
    let output_identity = output_path_identity(&output_abs);

    let mut files = Vec::new();
    collect_jett_files(&project_dir, &mut files)
        .map_err(|e| format!("error scanning project: {e}"))?;
    files.sort();
    for file in &files {
        let file_identity = file.canonicalize().unwrap_or_else(|_| file.clone());
        if file_identity != output_identity
            && existing_paths_share_file_identity(file, &output_identity)
        {
            return Err(format!(
                "bundle output {} is a hard-link alias of source {}; refusing to overwrite the source",
                output_abs.display(),
                file.display()
            )
            .into());
        }
    }
    files.retain(|path| path.canonicalize().unwrap_or_else(|_| path.clone()) != output_identity);

    if files.is_empty() {
        return Err(format!(
            "no .jett files found in project at {}",
            project_dir.display()
        )
        .into());
    }

    let mut source_files = Vec::with_capacity(files.len());
    let mut has_parse_errors = false;
    for (index, path) in files.into_iter().enumerate() {
        let source = fs::read_to_string(&path)
            .map_err(|e| format!("failed to read {}: {}", path.display(), e))?;
        let file_id = FileId::new(index as u32 + 1);
        let parsed = parse(&source, file_id);
        has_parse_errors |= has_error_diagnostics(&parsed.errors);
        source_files.push(BundleSourceFile {
            path,
            source,
            module: parsed.module,
            file_id,
        });
    }
    // Keep malformed projects on the lexical candidate path so the ordinary
    // build reports authoritative parser diagnostics before ordering analysis.
    let file_order = if has_parse_errors {
        (0..source_files.len()).collect()
    } else {
        let order = bundle_dependency_order(&source_files, &project_dir)?;
        validate_bundle_namespace_boundaries(&source_files, &order, &project_dir)?;
        order
    };

    let mut bundled = String::new();
    let mut current_line = 1_u32;
    bundled.push_str("# Generated by jett bundle.\n");
    bundled.push_str(&format!("# Project root: {}\n\n", project_dir.display()));
    current_line += 3;

    let mut bundled_files = Vec::new();
    for index in file_order {
        let file = &source_files[index];
        let relative = file.path.strip_prefix(&project_dir).unwrap_or(&file.path);
        bundled.push_str(&format!("# --- file: {} ---\n", relative.display()));
        current_line += 1;

        let start_line = current_line;
        let source_line_count = file.source.lines().count().max(1) as u32;
        bundled.push_str(&file.source);
        if !file.source.ends_with('\n') {
            bundled.push('\n');
        }
        let end_line = start_line + source_line_count - 1;
        current_line = end_line + 1;
        bundled.push('\n');
        current_line += 1;

        bundled_files.push(BundleFileResult {
            path: display_query_path(relative),
            start_line,
            end_line,
        });
    }

    let validation = build_source(&bundled, &output_abs.display().to_string());
    if validation.has_errors {
        return Err(
            BundleError::from_validation(validation).expect("candidate validation reported errors")
        );
    }

    if let Some(parent) = output_abs.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent)
            .map_err(|e| format!("failed to create {}: {}", parent.display(), e))?;
    }
    fs::write(&output_abs, bundled)
        .map_err(|e| format!("failed to write {}: {}", output_abs.display(), e))?;

    Ok(BundleResult {
        project_root: project_dir.display().to_string(),
        output_path: output_abs.display().to_string(),
        files: bundled_files,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_test_dir(name: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be after epoch")
            .as_nanos();
        std::env::temp_dir().join(format!("{name}_{nanos}"))
    }

    #[test]
    fn support_module_parse_errors_are_reported() {
        let root = temp_test_dir("jett_driver_support_parse_errors");
        fs::create_dir_all(&root).expect("temp support dir should be created");
        let broken = root.join("broken.jett");
        fs::write(&broken, "namespace broken\nfunction nope(\n")
            .expect("broken support fixture should be written");

        let discovered = discover_modules_in_dir(&root, None, STDLIB_FILE_ID_START, "stdlib");
        let errors = error_messages_from_diagnostics(&discovered.diagnostics);

        fs::remove_dir_all(&root).expect("temp support dir should be removed");

        assert!(
            discovered.modules.is_empty(),
            "parse-broken support file should not be loaded"
        );
        assert!(
            errors
                .iter()
                .any(|error| error.contains("failed to parse stdlib module")
                    && error.contains("broken.jett")),
            "expected support parse diagnostic to mention the broken stdlib module, got {errors:?}"
        );
        assert!(
            errors.iter().all(|error| !error.contains("undefined name")),
            "support parse errors should surface before resolver fallout, got {errors:?}"
        );
    }

    #[test]
    fn query_namespaces_lists_public_project_definitions() {
        let root = temp_test_dir("jett_driver_query_namespaces");
        fs::create_dir_all(&root).expect("temp project dir should be created");
        fs::write(root.join("jett.proj"), "name: query_fixture\n")
            .expect("project marker should be written");
        fs::write(
            root.join("api.jett"),
            "namespace api\n\nexport function login() returns int64:\n    return 1\n\nfunction hidden() returns int64:\n    return 2\n\nexport struct User:\n    id: int64\n",
        )
        .expect("query fixture should be written");

        let result = query_namespaces(&root).expect("query should succeed");

        fs::remove_dir_all(&root).expect("temp project dir should be removed");

        assert!(
            result
                .definitions
                .iter()
                .any(|def| def.name == "api" && query_kind_name(def.kind) == "namespace"),
            "expected namespace row in query result"
        );
        let login = result
            .definitions
            .iter()
            .find(|def| def.name == "api.login")
            .expect("expected exported function row in query result");
        assert_eq!(query_kind_name(login.kind), "function");
        assert_eq!(login.namespace.as_deref(), Some("api"));
        assert_eq!(
            (login.line, login.column, login.end_line, login.end_column),
            (3, 17, 3, 22)
        );

        let user = result
            .definitions
            .iter()
            .find(|def| def.name == "api.User")
            .expect("expected exported struct row in query result");
        assert_eq!(query_kind_name(user.kind), "struct");
        assert_eq!(user.namespace.as_deref(), Some("api"));
        assert_eq!(
            (user.line, user.column, user.end_line, user.end_column),
            (9, 15, 9, 19)
        );
        assert!(
            result
                .definitions
                .iter()
                .all(|def| def.name != "api.hidden"),
            "private namespaced definitions should not appear in global query results"
        );
    }

    #[test]
    fn query_file_symbols_lists_private_and_public_declarations() {
        let root = temp_test_dir("jett_driver_query_file_symbols");
        fs::create_dir_all(&root).expect("temp query dir should be created");
        let file = root.join("api.jett");
        fs::write(
            &file,
            "namespace api\n\nexport function login() returns int64:\n    return 1\n\nfunction hidden() returns int64:\n    return 2\n\nverify api_checks:\n    assert login() == 1\n",
        )
        .expect("symbols fixture should be written");

        let result = query_file_symbols(&file).expect("symbols query should succeed");

        fs::remove_dir_all(&root).expect("temp query dir should be removed");

        let login = result
            .symbols
            .iter()
            .find(|symbol| symbol.name == "api.login")
            .expect("expected exported function in file symbols");
        assert_eq!(login.kind, "function");
        assert_eq!(login.visibility, jett_resolve::scope::DefVisibility::Public);
        assert_eq!(
            (login.line, login.column, login.end_line, login.end_column),
            (3, 17, 3, 22)
        );

        let hidden = result
            .symbols
            .iter()
            .find(|symbol| symbol.name == "api.hidden")
            .expect("expected private function in file symbols");
        assert_eq!(hidden.kind, "function");
        assert_eq!(
            hidden.visibility,
            jett_resolve::scope::DefVisibility::Private
        );
        assert_eq!(
            hidden.signature.as_deref(),
            Some("api.hidden() returns int64")
        );
        assert_eq!(
            (
                hidden.line,
                hidden.column,
                hidden.end_line,
                hidden.end_column
            ),
            (6, 10, 6, 16)
        );
        assert!(
            result
                .symbols
                .iter()
                .any(|symbol| symbol.name == "api.api_checks" && symbol.kind == "verify"),
            "expected verify block in file symbols, got {:?}",
            result.symbols
        );
    }

    #[test]
    fn query_source_file_symbols_uses_unsaved_source_text() {
        let source = "namespace api\n\nexport function login() returns int64:\n    return 1\n";

        let result = query_source_file_symbols(source, "untitled:api.jett")
            .expect("source symbols query should succeed");

        assert_eq!(result.file_path, "untitled:api.jett");
        assert!(
            result
                .symbols
                .iter()
                .any(|symbol| symbol.name == "api.login" && symbol.kind == "function")
        );
    }

    #[test]
    fn query_file_symbols_preserves_parse_diagnostics() {
        let root = temp_test_dir("jett_driver_query_file_symbols_error");
        fs::create_dir_all(&root).expect("temp query dir should be created");
        let file = root.join("broken.jett");
        let source = "function broken( returns int64:\n    return 1\n";
        fs::write(&file, source).expect("broken symbols fixture should be written");

        let error = query_file_symbols_detailed(&file).expect_err("symbols query should fail");

        fs::remove_dir_all(&root).expect("temp query dir should be removed");

        let (diagnostics, diagnostic_source, diagnostic_path) = error
            .diagnostic_context()
            .expect("parse failure should retain diagnostic context");
        assert_eq!(diagnostic_source, source);
        assert_eq!(diagnostic_path, file.display().to_string());
        assert!(has_error_diagnostics(diagnostics));
        assert!(error.to_string().starts_with("parse errors:\nE"));
    }

    #[test]
    fn query_type_at_returns_type_for_file_position() {
        let root = temp_test_dir("jett_driver_query_type_at");
        fs::create_dir_all(&root).expect("temp query dir should be created");
        let file = root.join("main.jett");
        fs::write(
            &file,
            "namespace app\n\nfunction main() returns nothing:\n    int64 total = 1 + 2\n    return nothing\n",
        )
        .expect("query type fixture should be written");

        let result = query_type_at(&file, 4, 19).expect("type query should succeed");
        let outside_error =
            query_type_at(&file, 4, 999).expect_err("out-of-range column should fail");

        fs::remove_dir_all(&root).expect("temp query dir should be removed");

        assert!(outside_error.contains("position 4:999 is outside"));
        assert_eq!(result.type_name, Some("int64".to_string()));
        assert_eq!(
            (
                result.span_line,
                result.span_column,
                result.span_end_line,
                result.span_end_column
            ),
            (Some(4), Some(19), Some(4), Some(20))
        );
    }

    #[test]
    fn query_type_at_handles_lone_carriage_return_lines() {
        let root = temp_test_dir("jett_driver_query_type_at_lone_cr");
        fs::create_dir_all(&root).expect("temp query dir should be created");
        let file = root.join("main.jett");
        fs::write(
            &file,
            "namespace app\r\rfunction main() returns nothing:\r    int64 total = 1 + 2\r    return nothing\r",
        )
        .expect("lone-CR query fixture should be written");

        let result = query_type_at(&file, 4, 19).expect("type query should succeed");

        fs::remove_dir_all(&root).expect("temp query dir should be removed");

        assert_eq!(result.type_name, Some("int64".to_string()));
        assert_eq!(
            (
                result.span_line,
                result.span_column,
                result.span_end_line,
                result.span_end_column
            ),
            (Some(4), Some(19), Some(4), Some(20))
        );
    }

    #[test]
    fn query_type_at_preserves_parse_and_type_diagnostics() {
        let root = temp_test_dir("jett_driver_query_type_at_errors");
        fs::create_dir_all(&root).expect("temp query dir should be created");
        let parse_file = root.join("parse_error.jett");
        let parse_source = "function broken( returns int64:\n    return 1\n";
        fs::write(&parse_file, parse_source).expect("parse-error fixture should be written");
        let type_file = root.join("type_error.jett");
        let type_source =
            "function main() returns nothing:\n    int64 value = \"wrong\"\n    return nothing\n";
        fs::write(&type_file, type_source).expect("type-error fixture should be written");

        let parse_error = query_type_at_detailed(&parse_file, 1, 1)
            .expect_err("type query should retain parse errors");
        let type_error = query_type_at_detailed(&type_file, 2, 11)
            .expect_err("type query should retain type errors");

        fs::remove_dir_all(&root).expect("temp query dir should be removed");

        for (error, source, file, message) in [
            (parse_error, parse_source, &parse_file, "parse errors:"),
            (type_error, type_source, &type_file, "type errors:"),
        ] {
            let (diagnostics, diagnostic_source, diagnostic_path) = error
                .diagnostic_context()
                .expect("compiler failure should retain diagnostic context");
            assert_eq!(diagnostic_source, source);
            assert_eq!(diagnostic_path, file.display().to_string());
            assert!(has_error_diagnostics(diagnostics));
            assert!(error.to_string().starts_with(message));
        }
    }

    #[test]
    fn query_type_at_preserves_support_parse_diagnostics() {
        let root = temp_test_dir("jett_driver_query_type_at_support_error");
        fs::create_dir_all(&root).expect("temp query dir should be created");
        fs::write(root.join("jett.proj"), "name: query_fixture\n")
            .expect("project marker should be written");
        let support_file = root.join("broken.jett");
        let support_source = "function broken( returns int64:\n    return 1\n";
        fs::write(&support_file, support_source).expect("broken support file should be written");
        let requested_file = root.join("main.jett");
        fs::write(
            &requested_file,
            "function main() returns nothing:\n    return nothing\n",
        )
        .expect("query fixture should be written");

        let error = query_type_at_detailed(&requested_file, 1, 10)
            .expect_err("support parse failure should fail the query");

        fs::remove_dir_all(&root).expect("temp query dir should be removed");

        let (diagnostics, primary_file, sources) = error
            .diagnostic_sources()
            .expect("support parse failure should retain source files");
        let support_diagnostic = diagnostics
            .iter()
            .find(|diagnostic| diagnostic.span.file != primary_file)
            .expect("support diagnostic should retain its file id");
        let diagnostic_source = sources
            .iter()
            .find(|source| source.file_id == support_diagnostic.span.file)
            .expect("support diagnostic source should be retained");
        assert!(diagnostic_source.file_path.ends_with("broken.jett"));
        assert_eq!(diagnostic_source.source, support_source);
        assert!(error.to_string().starts_with("support parse errors:"));
    }

    #[test]
    fn query_type_at_preserves_cross_file_resolution_labels() {
        let root = temp_test_dir("jett_driver_query_type_at_cross_file_error");
        fs::create_dir_all(&root).expect("temp query dir should be created");
        fs::write(root.join("jett.proj"), "name: query_fixture\n")
            .expect("project marker should be written");
        let support_file = root.join("api.jett");
        fs::write(
            &support_file,
            "namespace api\n\nfunction hidden() returns int64:\n    return 1\n",
        )
        .expect("support fixture should be written");
        let requested_file = root.join("main.jett");
        fs::write(
            &requested_file,
            "namespace app\n\nfunction main() returns nothing:\n    int64 value = api.hidden()\n    return nothing\n",
        )
        .expect("query fixture should be written");

        let error = query_type_at_detailed(&requested_file, 4, 25)
            .expect_err("private cross-file use should fail resolution");

        fs::remove_dir_all(&root).expect("temp query dir should be removed");

        let (diagnostics, primary_file, sources) = error
            .diagnostic_sources()
            .expect("resolution failure should retain source files");
        let cross_file_label = diagnostics
            .iter()
            .flat_map(|diagnostic| &diagnostic.labels)
            .find(|label| label.span.file != primary_file)
            .expect("resolution diagnostic should retain its cross-file label");
        let label_source = sources
            .iter()
            .find(|source| source.file_id == cross_file_label.span.file)
            .expect("cross-file label source should be retained");
        assert!(label_source.file_path.ends_with("api.jett"));
        assert!(label_source.source.contains("function hidden"));
    }

    #[test]
    fn line_col_to_offset_rejects_columns_past_line_end() {
        let source = "alpha\nβeta\n";

        assert_eq!(line_col_to_offset(source, 1, 1), Some(0));
        assert_eq!(line_col_to_offset(source, 1, 6), Some(5));
        assert_eq!(line_col_to_offset(source, 1, 7), None);
        assert_eq!(line_col_to_offset(source, 2, 1), Some(6));
        assert_eq!(line_col_to_offset(source, 2, 5), Some(11));
        assert_eq!(line_col_to_offset(source, 2, 6), None);
        assert_eq!(line_col_to_offset(source, 3, 1), Some(12));
        assert_eq!(line_col_to_offset(source, 3, 2), None);
    }

    #[test]
    fn query_definition_at_returns_cross_file_definition() {
        let root = temp_test_dir("jett_driver_query_definition_at");
        fs::create_dir_all(&root).expect("temp query dir should be created");
        fs::write(root.join("jett.proj"), "name: query_fixture\n")
            .expect("project marker should be written");
        let models = root.join("models.jett");
        fs::write(
            &models,
            "namespace models\n\nexport struct User:\n    id: int64\n",
        )
        .expect("models fixture should be written");
        let file = root.join("main.jett");
        fs::write(
            &file,
            "namespace app\n\nfunction make() returns models.User:\n    use models\n    models.User user = models.User(id: 1)\n    return user\n",
        )
        .expect("main fixture should be written");

        let result = query_definition_at(&file, 5, 12).expect("definition query should succeed");

        fs::remove_dir_all(&root).expect("temp query dir should be removed");

        let target = result.target.expect("definition target should be found");
        assert_eq!(target.name, "models.User");
        assert_eq!(query_kind_name(target.kind), "struct");
        assert_eq!(target.namespace.as_deref(), Some("models"));
        assert!(
            target.file_path.ends_with("models.jett"),
            "expected target file to be models.jett, got {}",
            target.file_path
        );
        assert_eq!(
            (
                target.line,
                target.column,
                target.end_line,
                target.end_column
            ),
            (3, 15, 3, 19)
        );
    }

    #[test]
    fn query_references_at_returns_cross_file_use_sites() {
        let root = temp_test_dir("jett_driver_query_references_at");
        fs::create_dir_all(&root).expect("temp query dir should be created");
        fs::write(root.join("jett.proj"), "name: query_fixture\n")
            .expect("project marker should be written");
        fs::write(
            root.join("util.jett"),
            "namespace util\n\nexport function helper(n: int64) returns int64:\n    return n\n",
        )
        .expect("support fixture should be written");
        let file = root.join("main.jett");
        fs::write(
            &file,
            "namespace app\n\nfunction main() returns int64:\n    use util\n    int64 a = util.helper(1)\n    int64 b = util.helper(2)\n    return a + b\n",
        )
        .expect("main fixture should be written");

        let result = query_references_at(&file, 5, 15).expect("references query should succeed");

        fs::remove_dir_all(&root).expect("temp query dir should be removed");

        let target = result.target.expect("reference target should be found");
        assert_eq!(target.name, "util.helper");
        assert_eq!(query_kind_name(target.kind), "function");
        assert_eq!(result.references.len(), 2);
        assert!(
            result.references.iter().any(|reference| (
                reference.line,
                reference.column,
                reference.end_line,
                reference.end_column
            ) == (5, 15, 5, 26)),
            "expected first call site in references, got {:?}",
            result.references
        );
        assert!(
            result.references.iter().any(|reference| (
                reference.line,
                reference.column,
                reference.end_line,
                reference.end_column
            ) == (6, 15, 6, 26)),
            "expected second call site in references, got {:?}",
            result.references
        );
    }

    #[test]
    fn query_completions_at_includes_project_definitions() {
        let root = temp_test_dir("jett_driver_query_completions_at");
        fs::create_dir_all(&root).expect("temp query dir should be created");
        fs::write(root.join("jett.proj"), "name: query_fixture\n")
            .expect("project marker should be written");
        fs::write(
            root.join("util.jett"),
            "namespace util\n\nexport function helper() returns int64:\n    return 1\n",
        )
        .expect("support fixture should be written");
        let file = root.join("main.jett");
        fs::write(
            &file,
            "namespace app\n\nfunction main() returns nothing:\n    return nothing\n",
        )
        .expect("main fixture should be written");

        let result = query_completions_at(&file, 4, 5).expect("completion query should succeed");

        fs::remove_dir_all(&root).expect("temp query dir should be removed");

        assert_eq!(result.prefix, "");
        let helper = result
            .candidates
            .iter()
            .find(|candidate| {
                candidate.name == "util.helper" && query_kind_name(candidate.kind) == "function"
            })
            .expect("expected completion query to include exported project helper");
        assert_eq!(helper.namespace.as_deref(), Some("util"));
        assert_eq!(
            helper.visibility,
            jett_resolve::scope::DefVisibility::Public
        );
        assert!(
            helper.file_path.ends_with("util.jett"),
            "expected helper source file, got {}",
            helper.file_path
        );
        assert_eq!(
            (
                helper.line,
                helper.column,
                helper.end_line,
                helper.end_column
            ),
            (3, 17, 3, 23)
        );
        assert_eq!(helper.match_kind, CompletionMatchKind::EmptyPrefix);
        assert_eq!(helper.rank, 100);
    }

    #[test]
    fn query_completions_at_filters_by_cursor_prefix() {
        let root = temp_test_dir("jett_driver_query_completions_prefix");
        fs::create_dir_all(&root).expect("temp query dir should be created");
        fs::write(root.join("jett.proj"), "name: query_fixture\n")
            .expect("project marker should be written");
        fs::write(
            root.join("util.jett"),
            "namespace util\n\nexport function helper(n: int64) returns int64:\n    return n\n",
        )
        .expect("support fixture should be written");
        fs::write(
            root.join("other.jett"),
            "namespace other\n\nexport function helper(n: int64) returns int64:\n    return n\n",
        )
        .expect("other fixture should be written");
        let file = root.join("main.jett");
        fs::write(
            &file,
            "namespace app\n\nfunction main() returns int64:\n    int64 value = util.helper(1)\n    return value\n",
        )
        .expect("main fixture should be written");

        let result = query_completions_at(&file, 4, 21).expect("completion query should succeed");

        fs::remove_dir_all(&root).expect("temp query dir should be removed");

        assert_eq!(result.prefix, "ut");
        assert!(
            result
                .candidates
                .iter()
                .any(|candidate| candidate.name == "util.helper"),
            "expected util.helper for prefix `ut`, got {:?}",
            result.candidates
        );
        let util_helper = result
            .candidates
            .iter()
            .find(|candidate| candidate.name == "util.helper")
            .expect("expected util.helper candidate");
        assert_eq!(
            (
                util_helper.line,
                util_helper.column,
                util_helper.end_line,
                util_helper.end_column
            ),
            (3, 17, 3, 23)
        );
        assert_eq!(util_helper.match_kind, CompletionMatchKind::QualifiedPrefix);
        assert_eq!(util_helper.rank, 10);
        assert!(
            result
                .candidates
                .iter()
                .all(|candidate| candidate.name.starts_with("ut")
                    || candidate
                        .name
                        .rsplit_once('.')
                        .is_some_and(|(_, leaf)| leaf.starts_with("ut"))),
            "expected all candidates to match prefix `ut`, got {:?}",
            result.candidates
        );
    }

    #[test]
    fn query_completions_at_ranks_leaf_prefix_matches() {
        let root = temp_test_dir("jett_driver_query_completions_leaf_prefix");
        fs::create_dir_all(&root).expect("temp query dir should be created");
        fs::write(root.join("jett.proj"), "name: query_fixture\n")
            .expect("project marker should be written");
        fs::write(
            root.join("util.jett"),
            "namespace util\n\nexport function helper(n: int64) returns int64:\n    return n\n",
        )
        .expect("support fixture should be written");
        let file = root.join("main.jett");
        fs::write(
            &file,
            "namespace app\n\nfunction main() returns int64:\n    int64 value = hel\n    return value\n",
        )
        .expect("main fixture should be written");

        let result = query_completions_at(&file, 4, 22).expect("completion query should succeed");

        fs::remove_dir_all(&root).expect("temp query dir should be removed");

        assert_eq!(result.prefix, "hel");
        let helper = result
            .candidates
            .iter()
            .find(|candidate| candidate.name == "util.helper")
            .expect("expected util.helper candidate");
        assert_eq!(
            (
                helper.line,
                helper.column,
                helper.end_line,
                helper.end_column
            ),
            (3, 17, 3, 23)
        );
        assert_eq!(helper.match_kind, CompletionMatchKind::LeafPrefix);
        assert_eq!(helper.rank, 20);
    }

    #[test]
    fn query_signature_reports_stdlib_function_signature() {
        let result = query_signature(Path::new("."), "json.parse")
            .expect("signature query should succeed")
            .expect("json.parse signature should be found");

        assert_eq!(result.name, "json.parse");
        assert_eq!(result.type_params, vec!["T".to_string()]);
        assert_eq!(result.params.len(), 1);
        assert_eq!(result.params[0].name, "raw");
        assert_eq!(result.params[0].type_name, "string");
        assert_eq!(result.return_type, "result[T, string]");

        let raw_result = query_signature(Path::new("."), "json.parse_raw")
            .expect("signature query should succeed")
            .expect("json.parse_raw signature should be found");
        assert_eq!(raw_result.return_type, "result[json.JsonTree, string]");
    }

    #[test]
    fn query_signature_reports_source_defined_math_helpers() {
        let expected = [
            ("math.is_even", "int64", "bool"),
            ("math.is_odd", "int64", "bool"),
            ("math.sign", "int64", "int64"),
            ("math.to_radians", "float64", "float64"),
            ("math.to_degrees", "float64", "float64"),
            ("math.sum", "list[int64]", "int64"),
        ];

        for (name, param_type, return_type) in expected {
            let result = query_signature(Path::new("."), name)
                .expect("signature query should succeed")
                .unwrap_or_else(|| panic!("{name} signature should be found"));

            assert_eq!(result.params.len(), 1);
            assert_eq!(result.params[0].type_name, param_type);
            assert_eq!(result.return_type, return_type);
            assert!(
                result
                    .file_path
                    .replace('\\', "/")
                    .ends_with("stdlib/math.jett"),
                "{name} should resolve to compiler-shipped source, got {}",
                result.file_path
            );
        }
    }

    #[test]
    fn query_signature_reports_source_owned_math_surface() {
        let functions = [
            "abs",
            "min",
            "max",
            "sqrt",
            "pow",
            "floor",
            "ceil",
            "round",
            "clamp",
            "log",
            "log2",
            "log10",
            "average",
            "median",
            "pi",
            "e",
            "sin",
            "cos",
            "tan",
            "mod",
            "gcd",
            "lcm",
            "factorial",
            "is_even",
            "is_odd",
            "sign",
            "to_radians",
            "to_degrees",
            "sum",
        ];

        for function in functions {
            let name = format!("math.{function}");
            let result = query_signature(Path::new("."), &name)
                .expect("math signature query should succeed")
                .unwrap_or_else(|| panic!("{name} signature should be found"));
            assert!(
                result
                    .file_path
                    .replace('\\', "/")
                    .ends_with("/stdlib/math.jett"),
                "{name} should resolve to compiler-shipped source, got {}",
                result.file_path
            );
        }
    }

    #[test]
    fn query_signature_reports_extracted_string_helper_source() {
        let result = query_signature(Path::new("."), "string.reverse")
            .expect("signature query should succeed")
            .expect("string.reverse signature should be found");

        assert_eq!(result.name, "string.reverse");
        assert!(result.type_params.is_empty());
        assert_eq!(result.params.len(), 1);
        assert_eq!(result.params[0].name, "value");
        assert_eq!(result.params[0].type_name, "string");
        assert_eq!(result.return_type, "string");
        assert!(
            result
                .file_path
                .replace('\\', "/")
                .ends_with("stdlib/string.jett")
        );
    }

    #[test]
    fn query_signature_reports_source_owned_string_surface() {
        let functions = [
            "from_int64",
            "from_uint64",
            "from_float64",
            "from_bool",
            "char_count",
            "length",
            "is_empty",
            "is_not_empty",
            "chars",
            "slice",
            "index_of",
            "count",
            "contains",
            "starts_with",
            "ends_with",
            "take_chars",
            "take_last_chars",
            "drop_chars",
            "char_at",
            "trim",
            "trim_start",
            "trim_end",
            "upper",
            "lower",
            "replace",
            "split",
            "join",
            "repeat",
            "reverse",
            "after",
            "before",
            "between",
            "pad_left",
            "pad_start",
            "pad_end",
            "slugify",
            "truncate",
            "words",
            "lines",
            "to_upper_first",
            "to_lower_first",
            "center",
            "ljust",
            "rjust",
            "remove_prefix",
            "remove_suffix",
            "zfill",
            "is_numeric",
            "is_alpha",
        ];

        for function in functions {
            let name = format!("string.{function}");
            let result = query_signature(Path::new("."), &name)
                .expect("string signature query should succeed")
                .unwrap_or_else(|| panic!("{name} signature should be found"));
            assert!(
                result
                    .file_path
                    .replace('\\', "/")
                    .ends_with("/stdlib/string.jett"),
                "{name} should resolve to compiler-shipped source, got {}",
                result.file_path
            );
        }
    }

    #[test]
    fn query_signature_reports_extracted_list_helper_source() {
        let result = query_signature(Path::new("."), "list.first")
            .expect("signature query should succeed")
            .expect("list.first signature should be found");

        assert_eq!(result.name, "list.first");
        assert_eq!(result.type_params, vec!["T".to_string()]);
        assert_eq!(result.params.len(), 1);
        assert_eq!(result.params[0].name, "items");
        assert!(result.params[0].view);
        assert_eq!(result.params[0].type_name, "list[T]");
        assert_eq!(result.return_type, "optional[T]");
        assert!(
            result
                .file_path
                .replace('\\', "/")
                .ends_with("stdlib/list.jett")
        );
    }

    #[test]
    fn query_signature_reports_source_owned_list_surface() {
        let functions = [
            "new",
            "append",
            "length",
            "get",
            "is_empty",
            "first",
            "last",
            "insert_at",
            "remove_at",
            "remove",
            "swap",
            "skip",
            "take",
            "reverse",
            "sort",
            "contains",
            "index_of",
            "last_index_of",
            "concat",
            "flatten",
            "unique",
            "zip",
            "filter",
            "map",
            "flat_map",
            "find",
            "sort_by",
            "all",
            "any",
            "count",
            "sum",
            "group_by",
            "reduce",
            "chunk",
            "sort_by_index",
            "is_sorted",
            "all_elements_in",
            "enumerate",
            "from_set",
            "repeat",
        ];
        for function in functions {
            let name = format!("list.{function}");
            let result = query_signature(Path::new("."), &name)
                .expect("list signature query should succeed")
                .unwrap_or_else(|| panic!("{name} signature should be found"));
            assert!(
                result
                    .file_path
                    .replace('\\', "/")
                    .ends_with("/stdlib/list.jett"),
                "{name} should resolve to compiler-shipped source, got {}",
                result.file_path
            );
        }

        assert!(
            query_signature(Path::new("."), "list.range")
                .expect("removed list.range query should succeed")
                .is_none(),
            "global range is canonical; list.range must not remain public"
        );
    }

    #[test]
    fn query_signature_reports_source_owned_bytes_surface() {
        let functions = [
            ("new", None),
            ("length", Some(true)),
            ("slice", Some(true)),
            ("concat", Some(false)),
            ("from_string", Some(false)),
            ("to_string", Some(true)),
            ("get", Some(true)),
            ("to_hex", Some(true)),
            ("from_hex", Some(false)),
        ];

        for (function, first_param_view) in functions {
            let name = format!("bytes.{function}");
            let result = query_signature(Path::new("."), &name)
                .expect("bytes signature query should succeed")
                .unwrap_or_else(|| panic!("{name} signature should be found"));
            assert!(
                result
                    .file_path
                    .replace('\\', "/")
                    .ends_with("/stdlib/bytes.jett"),
                "{name} should resolve to compiler-shipped source, got {}",
                result.file_path
            );
            if let Some(expected_view) = first_param_view {
                assert_eq!(result.params[0].view, expected_view, "{name} view contract");
            } else {
                assert!(result.params.is_empty());
            }
        }
    }

    #[test]
    fn query_signature_reports_source_owned_encoding_surface() {
        let functions = [
            ("base64_encode", true, "string"),
            ("base64_decode", false, "result[bytes, string]"),
            ("hex_encode", true, "string"),
            ("hex_decode", false, "result[bytes, string]"),
            ("url_encode", false, "string"),
            ("url_decode", false, "result[string, string]"),
            ("form_encode", false, "string"),
            ("form_decode", false, "result[string, string]"),
        ];

        for (function, first_param_view, return_type) in functions {
            let name = format!("encoding.{function}");
            let result = query_signature(Path::new("."), &name)
                .expect("encoding signature query should succeed")
                .unwrap_or_else(|| panic!("{name} signature should be found"));
            assert!(
                result
                    .file_path
                    .replace('\\', "/")
                    .ends_with("/stdlib/encoding.jett"),
                "{name} should resolve to compiler-shipped source, got {}",
                result.file_path
            );
            assert_eq!(
                result.params[0].view, first_param_view,
                "{name} view contract"
            );
            assert_eq!(result.return_type, return_type, "{name} return type");
        }
    }

    #[test]
    fn query_signature_reports_source_owned_crypto_surface() {
        for function in ["sha256", "sha512", "md5"] {
            let name = format!("crypto.{function}");
            let result = query_signature(Path::new("."), &name)
                .expect("crypto signature query should succeed")
                .unwrap_or_else(|| panic!("{name} signature should be found"));
            assert!(
                result
                    .file_path
                    .replace('\\', "/")
                    .ends_with("/stdlib/crypto.jett"),
                "{name} should resolve to compiler-shipped source, got {}",
                result.file_path
            );
            assert_eq!(result.params.len(), 1);
            assert_eq!(result.params[0].name, "input");
            assert_eq!(result.params[0].type_name, "string");
            assert_eq!(result.return_type, "string");
        }

        for reserved in ["crypto.hmac_sha256", "crypto.hmac_sha512"] {
            assert!(
                query_signature(Path::new("."), reserved)
                    .expect("reserved crypto query should succeed")
                    .is_none(),
                "{reserved} must remain undiscoverable until implemented"
            );
        }
    }

    #[test]
    fn query_signature_reports_source_owned_csv_surface() {
        let functions = [
            ("parse", "result[list[list[string]], string]"),
            ("stringify", "string"),
            (
                "parse_with_header",
                "result[list[map[string, string]], string]",
            ),
        ];

        for (function, return_type) in functions {
            let name = format!("csv.{function}");
            let result = query_signature(Path::new("."), &name)
                .expect("CSV signature query should succeed")
                .unwrap_or_else(|| panic!("{name} signature should be found"));
            assert!(
                result
                    .file_path
                    .replace('\\', "/")
                    .ends_with("/stdlib/csv.jett"),
                "{name} should resolve to compiler-shipped source, got {}",
                result.file_path
            );
            assert_eq!(result.return_type, return_type, "{name} return type");
        }
    }

    #[test]
    fn query_signature_reports_source_owned_map_and_set_surfaces() {
        let map_functions = [
            "new",
            "length",
            "is_empty",
            "has",
            "get",
            "insert",
            "remove",
            "keys",
            "values",
            "set",
            "get_or",
            "merge",
            "contains_key",
            "from_lists",
            "entries",
            "filter",
            "map_values",
            "for_each",
        ];
        for function in map_functions {
            let name = format!("map.{function}");
            let result = query_signature(Path::new("."), &name)
                .expect("map signature query should succeed")
                .unwrap_or_else(|| panic!("{name} signature should be found"));
            assert!(
                result
                    .file_path
                    .replace('\\', "/")
                    .ends_with("/stdlib/map.jett"),
                "{name} should resolve to compiler-shipped source, got {}",
                result.file_path
            );
        }

        let set_functions = [
            "new",
            "add",
            "remove",
            "contains",
            "length",
            "is_empty",
            "to_list",
            "union",
            "intersection",
            "difference",
        ];
        for function in set_functions {
            let name = format!("set.{function}");
            let result = query_signature(Path::new("."), &name)
                .expect("set signature query should succeed")
                .unwrap_or_else(|| panic!("{name} signature should be found"));
            assert!(
                result
                    .file_path
                    .replace('\\', "/")
                    .ends_with("/stdlib/set.jett"),
                "{name} should resolve to compiler-shipped source, got {}",
                result.file_path
            );
        }
    }

    #[test]
    fn query_signature_reports_source_owned_random_surface() {
        let expected = [
            ("int64", 3, "result[int64, string]", false),
            ("float64", 1, "float64", false),
            ("bool", 1, "bool", false),
            ("choice", 2, "optional[T]", true),
            ("shuffle", 2, "list[T]", true),
        ];
        for (function, param_count, return_type, generic) in expected {
            let name = format!("random.{function}");
            let result = query_signature(Path::new("."), &name)
                .expect("random signature query should succeed")
                .unwrap_or_else(|| panic!("{name} signature should be found"));
            assert_eq!(result.params.len(), param_count);
            assert_eq!(result.params[0].name, "rng");
            assert_eq!(result.params[0].type_name, "Random");
            assert!(result.params[0].view);
            assert_eq!(result.type_params, if generic { vec!["T"] } else { vec![] });
            assert_eq!(result.return_type, return_type);
            assert!(
                result
                    .file_path
                    .replace('\\', "/")
                    .ends_with("/stdlib/random.jett"),
                "{name} should resolve to compiler-shipped source, got {}",
                result.file_path
            );
        }
    }

    #[test]
    fn query_signature_reports_source_owned_environment_surface() {
        for (name, parameter_count, return_type) in [
            ("Environment.get", 2, "result[optional[string], string]"),
            ("Environment.args", 1, "list[string]"),
        ] {
            let result = query_signature(Path::new("."), name)
                .expect("Environment signature query should succeed")
                .unwrap_or_else(|| panic!("{name} signature should be found"));
            assert_eq!(result.params.len(), parameter_count);
            assert_eq!(result.params[0].name, "env");
            assert_eq!(result.params[0].type_name, "Environment");
            assert!(result.params[0].view);
            assert_eq!(result.return_type, return_type);
            assert!(
                result
                    .file_path
                    .replace('\\', "/")
                    .ends_with("/stdlib/environment.jett"),
                "{name} should resolve to compiler-shipped source, got {}",
                result.file_path
            );
        }
    }

    #[test]
    fn query_signature_reports_source_owned_time_and_clock_surfaces() {
        let time_functions = [
            ("from_unix_milliseconds", 1, "time.Timestamp"),
            ("to_unix_milliseconds", 1, "int64"),
            ("to_unix_seconds", 1, "int64"),
            ("duration_milliseconds", 1, "time.Duration"),
            ("duration_seconds", 1, "result[time.Duration, string]"),
            ("duration_to_milliseconds", 1, "int64"),
            ("difference", 2, "result[time.Duration, string]"),
            ("add", 2, "result[time.Timestamp, string]"),
            ("subtract", 2, "result[time.Timestamp, string]"),
            ("before", 2, "bool"),
        ];
        for (function, param_count, return_type) in time_functions {
            let name = format!("time.{function}");
            let result = query_signature(Path::new("."), &name)
                .expect("time signature query should succeed")
                .unwrap_or_else(|| panic!("{name} signature should be found"));
            assert_eq!(result.params.len(), param_count);
            assert_eq!(result.return_type, return_type);
            assert!(
                result
                    .file_path
                    .replace('\\', "/")
                    .ends_with("/stdlib/time.jett")
            );
        }

        let now = query_signature(Path::new("."), "Clock.now")
            .expect("Clock.now signature query should succeed")
            .expect("Clock.now signature should be found");
        assert_eq!(now.params.len(), 1);
        assert_eq!(now.params[0].name, "clock");
        assert_eq!(now.params[0].type_name, "Clock");
        assert!(now.params[0].view);
        assert_eq!(now.return_type, "time.Timestamp");
        assert!(
            now.file_path
                .replace('\\', "/")
                .ends_with("/stdlib/time.jett")
        );
    }

    #[test]
    fn bundle_project_writes_validated_single_file() {
        let root = temp_test_dir("jett_driver_bundle_project");
        fs::create_dir_all(root.join("src")).expect("temp bundle dir should be created");
        fs::write(root.join("jett.proj"), "name: bundle_fixture\n")
            .expect("project marker should be written");
        fs::write(
            root.join("src").join("core.jett"),
            "namespace core\n\nexport function answer() returns int64:\n    return 42\n",
        )
        .expect("bundle source should be written");
        let output = root.join("dist").join("lib.jett");

        let result = bundle_project(&root, &output).expect("bundle should succeed");

        let bundled = fs::read_to_string(&output).expect("bundle output should be readable");
        fs::remove_dir_all(&root).expect("temp bundle dir should be removed");

        assert_eq!(result.files.len(), 1);
        assert_eq!(result.files[0].path.replace('\\', "/"), "src/core.jett");
        assert_eq!(result.files[0].start_line, 5);
        assert_eq!(result.files[0].end_line, 8);
        assert!(bundled.contains("# Generated by jett bundle."));
        assert!(bundled.contains("namespace core"));
        assert!(
            result
                .output_path
                .replace('\\', "/")
                .ends_with("dist/lib.jett")
        );
    }

    #[test]
    fn bundle_project_orders_dependency_before_lexical_consumer() {
        let root = temp_test_dir("jett_driver_bundle_dependency_order");
        fs::create_dir_all(root.join("src")).expect("temp bundle dir should be created");
        fs::write(root.join("jett.proj"), "name: bundle_fixture\n")
            .expect("project marker should be written");
        fs::write(
            root.join("src/a_consumer.jett"),
            "namespace app\n\nfunction main() returns int64:\n    use provider\n    return provider.value()\n",
        )
        .expect("consumer source should be written");
        fs::write(
            root.join("src/z_provider.jett"),
            "namespace provider\n\nexport function value() returns int64:\n    return 42\n",
        )
        .expect("provider source should be written");
        let output = root.join("dist/lib.jett");

        let result = bundle_project(&root, &output).expect("bundle should reorder whole files");
        let bundled = fs::read_to_string(&output).expect("bundle output should be readable");
        fs::remove_dir_all(&root).expect("temp bundle dir should be removed");

        let paths: Vec<_> = result
            .files
            .iter()
            .map(|file| file.path.replace('\\', "/"))
            .collect();
        assert_eq!(paths, ["src/z_provider.jett", "src/a_consumer.jett"]);
        assert!(
            bundled.find("namespace provider").expect("provider source")
                < bundled.find("namespace app").expect("consumer source")
        );
        assert_eq!(
            (result.files[0].start_line, result.files[0].end_line),
            (5, 8)
        );
        assert_eq!(
            (result.files[1].start_line, result.files[1].end_line),
            (11, 15)
        );
    }

    #[test]
    fn bundle_project_uses_lexical_tie_breaking_for_unrelated_files() {
        let root = temp_test_dir("jett_driver_bundle_stable_order");
        fs::create_dir_all(root.join("src")).expect("temp bundle dir should be created");
        fs::write(root.join("jett.proj"), "name: bundle_fixture\n")
            .expect("project marker should be written");
        fs::write(
            root.join("src/a_consumer.jett"),
            "namespace app\n\nfunction main() returns int64:\n    use provider\n    return provider.value()\n",
        )
        .expect("consumer source should be written");
        fs::write(
            root.join("src/b_unrelated.jett"),
            "namespace unrelated\n\nexport function value() returns int64:\n    return 7\n",
        )
        .expect("unrelated source should be written");
        fs::write(
            root.join("src/c_provider.jett"),
            "namespace provider\n\nexport function value() returns int64:\n    return 42\n",
        )
        .expect("provider source should be written");
        let output = root.join("dist/lib.jett");

        let result = bundle_project(&root, &output).expect("bundle should succeed");
        fs::remove_dir_all(&root).expect("temp bundle dir should be removed");

        let paths: Vec<_> = result
            .files
            .iter()
            .map(|file| file.path.replace('\\', "/"))
            .collect();
        assert_eq!(
            paths,
            [
                "src/b_unrelated.jett",
                "src/c_provider.jett",
                "src/a_consumer.jett"
            ]
        );
    }

    #[test]
    fn bundle_project_rejects_root_declarations_after_a_namespace() {
        let root = temp_test_dir("jett_driver_bundle_root_namespace_boundary");
        fs::create_dir_all(root.join("dist")).expect("temp bundle dist dir should be created");
        fs::create_dir_all(root.join("src")).expect("temp bundle src dir should be created");
        fs::write(root.join("jett.proj"), "name: bundle_fixture\n")
            .expect("project marker should be written");
        fs::write(
            root.join("src/provider.jett"),
            "namespace provider\n\nexport function value() returns int64:\n    return 42\n",
        )
        .expect("provider source should be written");
        fs::write(
            root.join("src/main.jett"),
            "function main() returns int64:\n    use provider\n    return provider.value()\n",
        )
        .expect("root source should be written");
        let output = root.join("dist/lib.jett");
        fs::write(&output, "existing bundle\n").expect("existing output should be written");

        let error = match bundle_project_detailed(&root, &output) {
            Ok(_) => panic!("a leaked namespace across a file boundary should fail"),
            Err(error) => error,
        };
        let preserved = fs::read_to_string(&output).expect("existing output should be readable");
        fs::remove_dir_all(&root).expect("temp bundle dir should be removed");

        assert_eq!(error.kind_name(), Some("ordering"));
        let diagnostics = error
            .diagnostic_result()
            .expect("namespace boundary failure should retain structured diagnostics");
        assert_eq!(diagnostics.diagnostics.len(), 1);
        assert!(
            diagnostics.diagnostics[0]
                .message
                .replace('\\', "/")
                .contains(
                    "root-namespace declarations in src/main.jett after namespace `provider`"
                )
        );
        assert_eq!(preserved, "existing bundle\n");
    }

    #[test]
    fn bundle_project_preserves_root_declarations_before_a_namespace() {
        let root = temp_test_dir("jett_driver_bundle_root_before_namespace");
        fs::create_dir_all(root.join("src")).expect("temp bundle src dir should be created");
        fs::write(root.join("jett.proj"), "name: bundle_fixture\n")
            .expect("project marker should be written");
        fs::write(
            root.join("src/a_root.jett"),
            "function main() returns int64:\n    return 42\n",
        )
        .expect("root source should be written");
        fs::write(
            root.join("src/z_namespace.jett"),
            "namespace helper\n\nexport function value() returns int64:\n    return 7\n",
        )
        .expect("namespaced source should be written");
        let output = root.join("dist/lib.jett");

        let result = bundle_project(&root, &output)
            .expect("root declarations before a namespace should bundle successfully");
        let bundled = fs::read_to_string(&output).expect("bundle output should be readable");
        fs::remove_dir_all(&root).expect("temp bundle dir should be removed");

        assert_eq!(
            result
                .files
                .iter()
                .map(|file| file.path.replace('\\', "/"))
                .collect::<Vec<_>>(),
            ["src/a_root.jett", "src/z_namespace.jett"]
        );
        assert!(bundled.find("function main").unwrap() < bundled.find("namespace helper").unwrap());
    }

    #[test]
    fn bundle_project_reports_ordering_cycle_and_preserves_output() {
        let root = temp_test_dir("jett_driver_bundle_ordering_cycle");
        fs::create_dir_all(root.join("dist")).expect("temp bundle dist dir should be created");
        fs::create_dir_all(root.join("src")).expect("temp bundle src dir should be created");
        fs::write(root.join("jett.proj"), "name: bundle_fixture\n")
            .expect("project marker should be written");
        fs::write(
            root.join("src/alpha.jett"),
            "namespace alpha\n\nexport function value() returns int64:\n    use beta\n    return beta.value()\n",
        )
        .expect("alpha source should be written");
        fs::write(
            root.join("src/beta.jett"),
            "namespace beta\n\nexport function value() returns int64:\n    use alpha\n    return alpha.value()\n",
        )
        .expect("beta source should be written");
        fs::write(
            root.join("src/gamma.jett"),
            "namespace gamma\n\nexport function value() returns int64:\n    use alpha\n    return alpha.value()\n",
        )
        .expect("cycle-dependent source should be written");
        let output = root.join("dist/lib.jett");
        fs::write(&output, "existing bundle\n").expect("existing output should be written");

        let error = match bundle_project_detailed(&root, &output) {
            Ok(_) => panic!("cyclic whole-file dependencies should fail"),
            Err(error) => error,
        };
        let preserved = fs::read_to_string(&output).expect("existing output should be readable");
        fs::remove_dir_all(&root).expect("temp bundle dir should be removed");

        assert_eq!(error.kind_name(), Some("ordering"));
        assert!(error.validation_result().is_none());
        let diagnostics = error
            .diagnostic_result()
            .expect("ordering failure should retain structured diagnostics");
        assert_eq!(diagnostics.diagnostics.len(), 1);
        assert!(
            diagnostics.diagnostics[0]
                .message
                .replace('\\', "/")
                .contains("src/alpha.jett, src/beta.jett")
        );
        assert!(!diagnostics.diagnostics[0].message.contains("gamma.jett"));
        assert_eq!(preserved, "existing bundle\n");
    }

    #[test]
    fn bundle_project_reports_parse_errors_before_ordering_cycles() {
        let root = temp_test_dir("jett_driver_bundle_parse_before_ordering");
        fs::create_dir_all(root.join("src")).expect("temp bundle src dir should be created");
        fs::write(root.join("jett.proj"), "name: bundle_fixture\n")
            .expect("project marker should be written");
        fs::write(
            root.join("src/alpha.jett"),
            "namespace alpha\n\nexport function value() returns int64:\n    return beta.value()\n",
        )
        .expect("alpha source should be written");
        fs::write(
            root.join("src/beta.jett"),
            "namespace beta\n\nexport function value() returns int64:\n    return alpha.value()\n",
        )
        .expect("beta source should be written");
        fs::write(
            root.join("src/broken.jett"),
            "namespace broken\nfunction nope(\n",
        )
        .expect("malformed source should be written");

        let error = match bundle_project_detailed(&root, Path::new("dist/lib.jett")) {
            Ok(_) => panic!("malformed bundle should fail validation"),
            Err(error) => error,
        };
        fs::remove_dir_all(&root).expect("temp bundle dir should be removed");

        assert_eq!(error.kind_name(), Some("validation"));
        let diagnostics = error
            .validation_result()
            .expect("parse failure should retain structured validation diagnostics");
        assert!(
            diagnostics
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code.code() == 1000)
        );
    }

    #[test]
    fn bundle_validation_error_rejects_successful_build() {
        let validation = build_source(
            "function answer() returns int64:\n    return 42\n",
            "dist/lib.jett",
        );

        assert!(BundleError::from_validation(validation).is_none());
    }

    #[test]
    fn bundle_project_leaves_output_untouched_when_validation_fails() {
        let root = temp_test_dir("jett_driver_bundle_validation_failure");
        fs::create_dir_all(root.join("dist")).expect("temp bundle dist dir should be created");
        fs::create_dir_all(root.join("src")).expect("temp bundle src dir should be created");
        fs::write(root.join("jett.proj"), "name: bundle_fixture\n")
            .expect("project marker should be written");
        fs::write(
            root.join("src").join("broken.jett"),
            "namespace broken\n\nfunction bad() returns int64:\n    return missing\n",
        )
        .expect("broken bundle source should be written");
        let output = root.join("dist").join("lib.jett");
        fs::write(&output, "existing bundle\n").expect("existing bundle output should be written");

        let error = match bundle_project_detailed(&root, &output) {
            Ok(_) => panic!("bundle validation should fail"),
            Err(error) => error,
        };

        let preserved =
            fs::read_to_string(&output).expect("existing bundle output should remain readable");
        fs::remove_dir_all(&root).expect("temp bundle dir should be removed");

        assert!(
            error
                .to_string()
                .contains("candidate bundle failed validation"),
            "expected validation failure, got {error}"
        );
        assert!(
            error.validation_result().is_some(),
            "expected structured candidate diagnostics"
        );
        assert_eq!(preserved, "existing bundle\n");
    }

    #[test]
    fn bundle_project_excludes_aliased_existing_output() {
        let root = temp_test_dir("jett_driver_bundle_output_alias");
        fs::create_dir_all(root.join("dist")).expect("temp bundle dist dir should be created");
        fs::create_dir_all(root.join("src")).expect("temp bundle src dir should be created");
        fs::write(root.join("jett.proj"), "name: bundle_fixture\n")
            .expect("project marker should be written");
        fs::write(
            root.join("src").join("core.jett"),
            "namespace core\n\nexport function answer() returns int64:\n    return 42\n",
        )
        .expect("bundle source should be written");
        let output = root.join("dist").join("lib.jett");
        fs::write(
            &output,
            "# Generated by jett bundle.\n\nnamespace stale\n\nfunction old() returns int64:\n    return 0\n",
        )
        .expect("existing bundle output should be written");
        let output_alias = root.join("dist").join("nested").join("..").join("lib.jett");

        let result = bundle_project(&root, &output_alias).expect("bundle should succeed");

        let bundled = fs::read_to_string(&output).expect("bundle output should be readable");
        fs::remove_dir_all(&root).expect("temp bundle dir should be removed");

        assert_eq!(result.files.len(), 1);
        assert_eq!(result.files[0].path.replace('\\', "/"), "src/core.jett");
        assert!(bundled.contains("namespace core"));
        assert!(!bundled.contains("namespace stale"));
    }

    #[cfg(any(unix, windows))]
    #[test]
    fn bundle_project_rejects_hard_linked_source_output() {
        let root = temp_test_dir("jett_driver_bundle_output_hard_link_alias");
        fs::create_dir_all(root.join("dist")).expect("temp bundle dist dir should be created");
        fs::create_dir_all(root.join("src")).expect("temp bundle src dir should be created");
        fs::write(root.join("jett.proj"), "name: bundle_fixture\n")
            .expect("project marker should be written");
        let source = root.join("src").join("core.jett");
        let source_text =
            "namespace core\n\nexport function answer() returns int64:\n    return 42\n";
        fs::write(&source, source_text).expect("bundle source should be written");
        let output = root.join("dist").join("lib.jett");
        fs::hard_link(&source, &output).expect("hard-linked output should be created");
        let output_alias = root
            .join("missing")
            .join("..")
            .join("dist")
            .join("lib.jett");

        let error = match bundle_project(&root, &output_alias) {
            Ok(_) => panic!("hard-link alias should be rejected"),
            Err(error) => error,
        };

        let preserved_source =
            fs::read_to_string(&source).expect("bundle source should remain readable");
        let preserved_output =
            fs::read_to_string(&output).expect("bundle output should remain readable");
        fs::remove_dir_all(&root).expect("temp bundle dir should be removed");

        assert!(
            error.contains("is a hard-link alias of source"),
            "expected hard-link alias error, got {error}"
        );
        assert_eq!(preserved_source, source_text);
        assert_eq!(preserved_output, source_text);
    }

    #[cfg(unix)]
    #[test]
    fn bundle_project_preserves_symlink_identity_with_missing_output_parent() {
        use std::os::unix::fs::symlink;

        let root = temp_test_dir("jett_driver_bundle_output_symlink_alias");
        fs::create_dir_all(root.join("src")).expect("temp bundle src dir should be created");
        fs::write(root.join("jett.proj"), "name: bundle_fixture\n")
            .expect("project marker should be written");
        fs::write(
            root.join("src").join("core.jett"),
            "namespace core\n\nexport function answer() returns int64:\n    return 42\n",
        )
        .expect("bundle source should be written");
        let output = root.join("actual.jett");
        fs::write(
            &output,
            "# Generated by jett bundle.\n\nnamespace stale\n\nfunction old() returns int64:\n    return 0\n",
        )
        .expect("existing bundle output should be written");
        let linked_output = root.join("linked.jett");
        symlink(&output, &linked_output).expect("output symlink should be created");
        let output_alias = root.join("missing").join("..").join("linked.jett");

        let result = bundle_project(&root, &output_alias).expect("bundle should succeed");

        let bundled = fs::read_to_string(&output).expect("bundle output should be readable");
        fs::remove_dir_all(&root).expect("temp bundle dir should be removed");

        assert_eq!(result.files.len(), 1);
        assert_eq!(result.files[0].path.replace('\\', "/"), "src/core.jett");
        assert!(bundled.contains("namespace core"));
        assert!(!bundled.contains("namespace stale"));
    }

    #[cfg(unix)]
    #[test]
    fn project_file_collection_rejects_external_source_symlinks() {
        use std::os::unix::fs::symlink;

        let root = temp_test_dir("jett_driver_symlink_file_project");
        let external = temp_test_dir("jett_driver_symlink_file_external");
        fs::create_dir_all(root.join("src")).expect("temp project src dir should be created");
        fs::create_dir_all(&external).expect("external source dir should be created");
        fs::write(root.join("src/main.jett"), "namespace app\n")
            .expect("project source should be written");
        fs::write(external.join("outside.jett"), "namespace outside\n")
            .expect("external source should be written");
        symlink(external.join("outside.jett"), root.join("linked.jett"))
            .expect("source file symlink should be created");

        let mut files = Vec::new();
        let result = collect_jett_files(&root, &mut files);

        fs::remove_dir_all(&root).expect("temp project dir should be removed");
        fs::remove_dir_all(&external).expect("external source dir should be removed");
        let error = result.expect_err("an external source symlink must be rejected");
        assert_eq!(error.kind(), std::io::ErrorKind::InvalidData);
        assert!(error.to_string().contains("resolves outside source root"));
    }

    #[cfg(unix)]
    #[test]
    fn project_file_collection_ignores_symlinked_directories() {
        use std::os::unix::fs::symlink;

        let root = temp_test_dir("jett_driver_symlink_project");
        let external = temp_test_dir("jett_driver_symlink_external");
        fs::create_dir_all(root.join("src")).expect("temp project src dir should be created");
        fs::create_dir_all(&external).expect("external source dir should be created");
        let source = root.join("src").join("main.jett");
        fs::write(&source, "namespace app\n").expect("project source should be written");
        fs::write(external.join("outside.jett"), "namespace outside\n")
            .expect("external source should be written");
        symlink(&external, root.join("linked"))
            .expect("source directory symlink should be created");

        let mut files = Vec::new();
        collect_jett_files(&root, &mut files).expect("project files should be collected");

        fs::remove_dir_all(&root).expect("temp project dir should be removed");
        fs::remove_dir_all(&external).expect("external source dir should be removed");
        assert_eq!(files, vec![source]);
    }

    #[test]
    fn references_at_returns_all_source_use_sites() {
        let source = "namespace app\n\nfunction double(value: int64) returns int64:\n    return value + value\n\nfunction main() returns int64:\n    return double(21)\n";

        let references = references_at(source, 7, 12);

        assert_eq!(references.len(), 1);
        let call_start = source.find("double(21)").expect("call should exist") as u32;
        assert_eq!(references, vec![(call_start, call_start + 6)]);
    }

    #[test]
    fn references_at_accepts_the_source_declaration() {
        let source = "namespace app\n\nfunction double(value: int64) returns int64:\n    return value + value\n\nfunction main() returns int64:\n    return double(21)\n";

        let references = references_at(source, 3, 10);

        let call_start = source.find("double(21)").expect("call should exist") as u32;
        assert_eq!(references, vec![(call_start, call_start + 6)]);
    }
}

// ---------------------------------------------------------------------------
// Helpers — project file discovery for `jett test`
// ---------------------------------------------------------------------------

/// Walk up from `start_dir` to find a directory containing `jett.proj`.
fn find_project_root(start_dir: &Path) -> Result<std::path::PathBuf, String> {
    let start = if start_dir.is_file() {
        start_dir.parent().unwrap_or(start_dir).to_path_buf()
    } else {
        start_dir.to_path_buf()
    };

    let mut current = start.as_path();
    loop {
        if current.join("jett.proj").exists() {
            return Ok(current.to_path_buf());
        }
        match current.parent() {
            Some(parent) => current = parent,
            None => {
                return Err("no jett.proj found in current directory or any parent".to_string());
            }
        }
    }
}

/// Recursively collect all `.jett` files in a directory, skipping hidden dirs,
/// `target/`, and symlinked directories. Source-file symlinks are accepted only
/// when their canonical targets remain inside the source root.
fn collect_jett_files(dir: &Path, out: &mut Vec<std::path::PathBuf>) -> std::io::Result<()> {
    let canonical_root = dir.canonicalize()?;
    collect_jett_files_within(dir, &canonical_root, out)
}

fn collect_jett_files_within(
    dir: &Path,
    canonical_root: &Path,
    out: &mut Vec<std::path::PathBuf>,
) -> std::io::Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        let file_type = entry.file_type()?;
        if file_type.is_symlink() {
            if path.is_file() && path.extension().and_then(|e| e.to_str()) == Some("jett") {
                let canonical_path = path.canonicalize()?;
                if !canonical_path.starts_with(canonical_root) {
                    let logical_path = path.strip_prefix(dir).unwrap_or(&path);
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        format!(
                            "source file '{}' resolves outside source root",
                            logical_path.display()
                        ),
                    ));
                }
                out.push(path);
            }
        } else if file_type.is_dir() {
            let dir_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if !dir_name.starts_with('.') && dir_name != "target" {
                collect_jett_files_within(&path, canonical_root, out)?;
            }
        } else if path.extension().and_then(|e| e.to_str()) == Some("jett") {
            out.push(path);
        }
    }
    Ok(())
}
