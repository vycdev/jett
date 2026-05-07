use jett_common::FileId;
use jett_comptime::value::Value;
use jett_comptime::verify::{run_verify_blocks, run_verify_blocks_detailed};
use jett_diagnostics::Diagnostic;
use jett_fmt::{FormatResult, format_source};
use jett_parser::ast::{FunctionDef, Item, Param, TypeExpr};
use jett_parser::parse;
use jett_resolve::resolve;
use jett_typecheck::check;
use std::fs;
use std::path::Path;

/// Result of compiling a single file.
pub struct BuildResult {
    pub diagnostics: Vec<Diagnostic>,
    pub has_errors: bool,
    /// The source text that was compiled (for diagnostic rendering).
    pub source: String,
    /// The file path that was compiled (for diagnostic rendering).
    pub file_path: String,
}

/// Run the full compilation pipeline on in-memory source text.
/// Used by the LSP server to validate documents without touching the filesystem.
pub fn build_source(source: &str, file_path: &str) -> BuildResult {
    let file_id = FileId::new(0);
    let mut all_diagnostics = Vec::new();

    // Phase 1+2: Lex + Parse
    let parse_result = parse(source, file_id);
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
        };
    }

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
            source: source.to_string(),
            file_path: file_path.to_string(),
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
        };
    }

    // Phase 5: Execute verify blocks at compile time
    let verify_diagnostics = run_verify_blocks(&parse_result.module);
    all_diagnostics.extend(verify_diagnostics);

    let has_errors = all_diagnostics
        .iter()
        .any(|d| d.severity == jett_diagnostics::Severity::Error);

    BuildResult {
        has_errors,
        diagnostics: all_diagnostics,
        source: source.to_string(),
        file_path: file_path.to_string(),
    }
}

/// Return the inferred type name at the given (1-based) line and column in `source`.
/// Returns `None` if the position is outside any typed expression or if the file
/// does not compile cleanly past the parse phase.
pub fn hover_type(source: &str, line: u32, col: u32) -> Option<String> {
    let file_id = FileId::new(0);

    // Convert 1-based (line, col) to a byte offset.
    let offset = line_col_to_offset(source, line, col)?;

    let parse_result = parse(source, file_id);
    if parse_result
        .errors
        .iter()
        .any(|d| d.severity == jett_diagnostics::Severity::Error)
    {
        return None;
    }

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
        if span.start <= offset && offset <= span.end {
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
    let file_id = FileId::new(0);
    let parse_result = parse(source, file_id);
    if parse_result
        .errors
        .iter()
        .any(|d| d.severity == jett_diagnostics::Severity::Error)
    {
        return Vec::new();
    }
    let resolve_result = resolve(&parse_result.module);
    resolve_result
        .scope_table
        .definitions
        .iter()
        .map(|def| (def.name.clone(), def.kind))
        .collect()
}

/// Return the byte span of the definition of the symbol at the given (1-based)
/// line and column in `source`.  Returns `None` if no definition is found.
pub fn goto_definition(source: &str, line: u32, col: u32) -> Option<(u32, u32)> {
    let file_id = FileId::new(0);

    let offset = line_col_to_offset(source, line, col)?;

    let parse_result = parse(source, file_id);
    if parse_result
        .errors
        .iter()
        .any(|d| d.severity == jett_diagnostics::Severity::Error)
    {
        return None;
    }

    let resolve_result = resolve(&parse_result.module);

    // Find the reference span that covers `offset`.
    let mut best_def: Option<(u32, jett_resolve::scope::DefId)> = None;
    for (span, def_id) in &resolve_result.resolutions {
        if span.start <= offset && offset <= span.end {
            let len = span.end - span.start;
            if best_def.is_none() || len < best_def.unwrap().0 {
                best_def = Some((len, *def_id));
            }
        }
    }

    best_def.map(|(_, def_id)| {
        let def_info = resolve_result.scope_table.def(def_id);
        (def_info.span.start, def_info.span.end)
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
        };
    }

    // Multi-file: prepend items from sibling project files so resolver/typechecker
    // can see cross-file definitions (functions, types, etc.).
    if include_project {
        let sibling_modules = discover_project_modules(path);
        if !sibling_modules.is_empty() {
            let mut merged_items = Vec::new();
            for module in sibling_modules {
                merged_items.extend(module.items);
            }
            merged_items.append(&mut parse_result.module.items);
            parse_result.module.items = merged_items;
        }
    }

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
        };
    }

    // Phase 5: Execute verify blocks at compile time
    let verify_diagnostics = run_verify_blocks(&parse_result.module);
    all_diagnostics.extend(verify_diagnostics);

    let has_errors = all_diagnostics
        .iter()
        .any(|d| d.severity == jett_diagnostics::Severity::Error);

    BuildResult {
        has_errors,
        diagnostics: all_diagnostics,
        source,
        file_path: file_path_str,
    }
}

/// Register all items from a parsed module into an interpreter.
fn register_module_items(
    interp: &mut jett_comptime::interpreter::Interpreter,
    module: &jett_parser::ast::Module,
) {
    for item in &module.items {
        match item {
            Item::Function(func) => interp.register_function(func),
            Item::TypeAlias(alias) => interp.register_type_alias(alias),
            Item::Interface(interface) => interp.register_interface(interface),
            Item::Implement(block) => interp.register_implement_block(block),
            Item::Struct(strukt) => interp.register_struct(strukt),
            Item::Enum(enm) => interp.register_enum(enm),
            Item::Bitfield(bitfield) => interp.register_bitfield(bitfield),
            Item::Actor(actor) => interp.register_actor(actor),
            _ => {}
        }
    }
}

/// Discover and parse all sibling .jett files in the project (if a jett.proj exists).
/// Returns parsed modules for files other than the entry file.
fn discover_project_modules(entry_path: &Path) -> Vec<jett_parser::ast::Module> {
    let canon = entry_path.canonicalize().ok();
    let project_root = find_project_root(entry_path).ok();
    let Some(root) = project_root else {
        return Vec::new();
    };
    let mut files = Vec::new();
    if collect_jett_files(&root, &mut files).is_err() {
        return Vec::new();
    }
    files.sort();

    let mut modules = Vec::new();
    for (idx, file_path) in files.iter().enumerate() {
        // Skip the entry file — it will be registered separately.
        let is_entry = canon
            .as_ref()
            .map_or(false, |c| file_path.canonicalize().ok().as_ref() == Some(c));
        if is_entry {
            continue;
        }
        if let Ok(source) = fs::read_to_string(file_path) {
            let file_id = FileId::new((idx + 1) as u32);
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

    // Find main()
    let main_func = parse_result
        .module
        .items
        .iter()
        .find_map(|item| match item {
            Item::Function(func) if func.name.name == "main" => Some(func),
            _ => None,
        });

    let Some(main_func) = main_func else {
        return Err("runtime error: no `main` function found".to_string());
    };

    let main_args = default_runtime_args_for_main(main_func)?;

    use jett_comptime::interpreter::Interpreter;
    let mut interp = Interpreter::new_runtime();

    // Register items from sibling project files first (so they're available to main file).
    let sibling_modules = discover_project_modules(path);
    for module in &sibling_modules {
        register_module_items(&mut interp, module);
    }

    // Register items from the entry file (may override sibling definitions).
    register_module_items(&mut interp, &parse_result.module);

    // Call main()
    match interp.call_function("main", main_args) {
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
    let parse_result = parse(&source, file_id);

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

    let results = run_verify_blocks_detailed(&parse_result.module);

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
