use jett_common::{FileId, STDLIB_FILE_ID_START};
use jett_comptime::value::Value;
use jett_comptime::verify::{
    run_verify_blocks_detailed_with_metadata, run_verify_blocks_with_metadata,
};
use jett_diagnostics::Diagnostic;
use jett_fmt::{FormatResult, format_source};
use jett_parser::ast::{FunctionDef, Item, Module, Param, TypeExpr};
use jett_parser::parse;
use jett_resolve::resolve;
use jett_typecheck::check;
use jett_types::ReflectionMetadata;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::thread;

const RUNTIME_STACK_SIZE: usize = 8 * 1024 * 1024;

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
}

/// Run the full compilation pipeline on in-memory source text.
/// Used by the LSP server to validate documents without touching the filesystem.
pub fn build_source(source: &str, file_path: &str) -> BuildResult {
    let file_id = FileId::new(0);
    let mut all_diagnostics = Vec::new();

    // Phase 1+2: Lex + Parse
    let mut parse_result = parse(source, file_id);
    all_diagnostics.extend(parse_result.errors.clone());

    let has_parse_errors = all_diagnostics
        .iter()
        .any(|d| d.severity == jett_diagnostics::Severity::Error);
    if has_parse_errors {
        return BuildResult {
            has_errors: true,
            diagnostics: all_diagnostics,
            source: source.to_string(),
            file_path: file_path.to_string(),
            reflection_metadata: None,
        };
    }

    // Phase 3: Resolve names
    prepend_support_modules(&mut parse_result.module, discover_stdlib_modules());

    let resolve_result = resolve(&parse_result.module);
    all_diagnostics.extend(resolve_result.diagnostics.clone());

    let has_resolve_errors = all_diagnostics
        .iter()
        .any(|d| d.severity == jett_diagnostics::Severity::Error);
    if has_resolve_errors {
        return BuildResult {
            has_errors: true,
            diagnostics: all_diagnostics,
            source: source.to_string(),
            file_path: file_path.to_string(),
            reflection_metadata: None,
        };
    }

    // Phase 4: Type check
    let check_result = check(&parse_result.module, &resolve_result);
    all_diagnostics.extend(check_result.diagnostics.clone());

    let has_typecheck_errors = all_diagnostics
        .iter()
        .any(|d| d.severity == jett_diagnostics::Severity::Error);
    if has_typecheck_errors {
        return BuildResult {
            has_errors: true,
            diagnostics: all_diagnostics,
            source: source.to_string(),
            file_path: file_path.to_string(),
            reflection_metadata: None,
        };
    }

    // Phase 5: Execute verify blocks at compile time
    let reflection_metadata = check_result.reflection_metadata.clone();
    let verify_diagnostics =
        run_verify_blocks_with_metadata(&parse_result.module, check_result.reflection_metadata);
    all_diagnostics.extend(verify_diagnostics);

    let has_errors = all_diagnostics
        .iter()
        .any(|d| d.severity == jett_diagnostics::Severity::Error);

    BuildResult {
        has_errors,
        diagnostics: all_diagnostics,
        source: source.to_string(),
        file_path: file_path.to_string(),
        reflection_metadata: Some(reflection_metadata),
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
    completions_for_namespace(source, current_namespace.as_deref())
}

fn completions_for_namespace(
    source: &str,
    current_namespace: Option<&str>,
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
    prepend_support_modules(&mut parse_result.module, discover_stdlib_modules());

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

    // Find the reference span that covers `offset`.
    let mut best_def: Option<(u32, jett_resolve::scope::DefId)> = None;
    for (span, def_id) in &resolve_result.resolutions {
        if span.file == file_id && span.start <= offset && offset <= span.end {
            let len = span.end - span.start;
            if best_def.is_none() || len < best_def.unwrap().0 {
                best_def = Some((len, *def_id));
            }
        }
    }

    best_def.and_then(|(_, def_id)| {
        let def_info = resolve_result.scope_table.def(def_id);
        if def_info.span.file == file_id {
            Some((def_info.span.start, def_info.span.end))
        } else {
            None
        }
    })
}

/// Convert a 1-based line+column to a byte offset in `source`.
fn line_col_to_offset(source: &str, line: u32, col: u32) -> Option<u32> {
    if line == 0 || col == 0 {
        return None;
    }
    let mut current_line = 1u32;
    let mut line_start = 0usize;
    for (i, ch) in source.char_indices() {
        if current_line == line {
            // col is 1-based within the line; advance col-1 chars.
            let col_offset = source[line_start..]
                .char_indices()
                .nth((col - 1) as usize)
                .map(|(o, _)| o)
                .unwrap_or(source.len() - line_start);
            return Some((line_start + col_offset) as u32);
        }
        if ch == '\n' {
            current_line += 1;
            line_start = i + 1;
        }
    }
    if current_line == line {
        return Some(line_start as u32);
    }
    None
}

/// Run the full compilation pipeline on a single file: lex → parse → resolve → typecheck.
/// Does not produce executable output yet — just validates the source.
pub fn build_file(path: &Path) -> BuildResult {
    build_file_inner(path, true)
}

fn build_file_inner(path: &Path, include_project: bool) -> BuildResult {
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
            };
        }
    };

    let file_id = FileId::new(0);
    let mut all_diagnostics = Vec::new();

    // Phase 1+2: Lex + Parse (parse internally calls tokenize)
    let mut parse_result = parse(&source, file_id);
    all_diagnostics.extend(parse_result.errors.clone());

    // If there are parse errors, stop here — resolve/typecheck won't produce useful results
    let has_parse_errors = all_diagnostics
        .iter()
        .any(|d| d.severity == jett_diagnostics::Severity::Error);
    if has_parse_errors {
        return BuildResult {
            has_errors: true,
            diagnostics: all_diagnostics,
            source,
            file_path: file_path_str,
            reflection_metadata: None,
        };
    }

    // Multi-file: prepend stdlib and sibling project modules so
    // resolver/typechecker can see cross-file definitions (functions, types,
    // etc.).
    let mut support_modules = discover_stdlib_modules();
    if include_project {
        support_modules.extend(discover_project_modules(path));
    }
    prepend_support_modules(&mut parse_result.module, support_modules);

    // Phase 3: Resolve names
    let resolve_result = resolve(&parse_result.module);
    all_diagnostics.extend(resolve_result.diagnostics.clone());

    let has_resolve_errors = all_diagnostics
        .iter()
        .any(|d| d.severity == jett_diagnostics::Severity::Error);
    if has_resolve_errors {
        return BuildResult {
            has_errors: true,
            diagnostics: all_diagnostics,
            source,
            file_path: file_path_str,
            reflection_metadata: None,
        };
    }

    // Phase 4: Type check
    let check_result = check(&parse_result.module, &resolve_result);
    all_diagnostics.extend(check_result.diagnostics.clone());

    let has_typecheck_errors = all_diagnostics
        .iter()
        .any(|d| d.severity == jett_diagnostics::Severity::Error);
    if has_typecheck_errors {
        return BuildResult {
            has_errors: true,
            diagnostics: all_diagnostics,
            source,
            file_path: file_path_str,
            reflection_metadata: None,
        };
    }

    // Phase 5: Execute verify blocks at compile time
    let reflection_metadata = check_result.reflection_metadata.clone();
    let verify_diagnostics =
        run_verify_blocks_with_metadata(&parse_result.module, check_result.reflection_metadata);
    all_diagnostics.extend(verify_diagnostics);

    let has_errors = all_diagnostics
        .iter()
        .any(|d| d.severity == jett_diagnostics::Severity::Error);

    BuildResult {
        has_errors,
        diagnostics: all_diagnostics,
        source,
        file_path: file_path_str,
        reflection_metadata: Some(reflection_metadata),
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
        Item::TypeAlias(alias) => alias.span.file,
    }
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
    discover_modules_in_dir(&stdlib_root(), None, STDLIB_FILE_ID_START)
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
    let canon = entry_path.canonicalize().ok();
    let project_root = find_project_root(entry_path).ok();
    let Some(root) = project_root else {
        return Vec::new();
    };
    discover_modules_in_dir(&root, canon.as_deref(), 1)
}

fn discover_modules_in_dir(
    root: &Path,
    skip_canon: Option<&Path>,
    start_file_id: u32,
) -> Vec<Module> {
    let mut files = Vec::new();
    if collect_jett_files(root, &mut files).is_err() {
        return Vec::new();
    }
    files.sort();

    let mut modules = Vec::new();
    for (idx, file_path) in files.iter().enumerate() {
        // Skip the entry file when parsing project siblings.
        let should_skip = skip_canon
            .map(|skip| file_path.canonicalize().ok().as_deref() == Some(skip))
            .unwrap_or(false);
        if should_skip {
            continue;
        }
        if let Ok(source) = fs::read_to_string(file_path) {
            let file_id = FileId::new(start_file_id + idx as u32);
            let parsed = parse(&source, file_id);
            // Only include files that parse without errors.
            let has_errors = parsed
                .errors
                .iter()
                .any(|d| d.severity == jett_diagnostics::Severity::Error);
            if !has_errors {
                modules.push(parsed.module);
            }
        }
    }
    modules
}

/// Run a .jett file using the tree-walking interpreter.
/// First validates (lex → parse → resolve → typecheck → verify), then executes main().
/// If a jett.proj exists, also loads sibling .jett files so cross-file calls work.
pub fn run_file(path: &Path) -> Result<(), String> {
    let thread_path = path.to_path_buf();
    let fallback_path = thread_path.clone();
    match thread::Builder::new()
        .name("jett-runtime".to_string())
        .stack_size(RUNTIME_STACK_SIZE)
        .spawn(move || run_file_inner(&thread_path))
    {
        Ok(handle) => match handle.join() {
            Ok(result) => result,
            Err(payload) => std::panic::resume_unwind(payload),
        },
        Err(_) => run_file_inner(&fallback_path),
    }
}

fn run_file_inner(path: &Path) -> Result<(), String> {
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
    let mut interp = Interpreter::new_runtime();
    if let Some(metadata) = build.reflection_metadata.clone() {
        interp.set_reflection_metadata(metadata);
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
        Ok(_) => Ok(()),
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

    let mut support_modules = discover_stdlib_modules();
    support_modules.extend(discover_project_modules(path));
    strip_test_items_from_support_modules(&mut support_modules);
    prepend_support_modules(&mut parse_result.module, support_modules);

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

    let results = run_verify_blocks_detailed_with_metadata(
        &parse_result.module,
        Some(check_result.reflection_metadata),
    );

    let total = results.len();
    let passed = results.iter().filter(|r| r.passed).count();
    let failed = total - passed;

    let blocks = results
        .into_iter()
        .map(|r| TestBlockResult {
            name: r.name,
            passed: r.passed,
            error: r.error,
            is_property: r.is_property,
            iterations: r.iterations,
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

/// Recursively collect all `.jett` files in a directory, skipping hidden dirs
/// and `target/`.
fn collect_jett_files(dir: &Path, out: &mut Vec<std::path::PathBuf>) -> std::io::Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            let dir_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if !dir_name.starts_with('.') && dir_name != "target" {
                collect_jett_files(&path, out)?;
            }
        } else if path.extension().and_then(|e| e.to_str()) == Some("jett") {
            out.push(path);
        }
    }
    Ok(())
}
