use clap::{Parser, Subcommand};
use std::path::Path;
use std::process;

/// The Jett programming language compiler
#[derive(Parser)]
#[command(name = "jett", version, about)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Format a .jett source file
    Format {
        /// Path to the .jett file
        file: String,

        /// Check formatting without modifying the file
        #[arg(long)]
        check: bool,

        /// Emit TOON agent output
        #[arg(long)]
        agent: bool,
    },

    /// Compile a .jett source file
    Build {
        /// Path to the .jett file
        file: String,

        /// Build in release mode (optimized)
        #[arg(long)]
        release: bool,

        /// Emit TOON agent output
        #[arg(long)]
        agent: bool,

        /// Target triple for cross-compilation
        #[arg(long)]
        target: Option<String>,
    },

    /// Interpret a .jett source file
    Run {
        /// Path to the .jett file
        file: String,

        /// Emit TOON agent output
        #[arg(long)]
        agent: bool,
    },

    /// Run all verify and property blocks
    Test {
        /// Path to a .jett file (if omitted, finds jett.proj and tests all files)
        file: Option<String>,

        /// Emit TOON agent output
        #[arg(long)]
        agent: bool,
    },

    /// Bundle a project into one validated .jett file
    Bundle {
        /// Project file or directory to bundle (defaults to current directory)
        start: Option<String>,

        /// Output .jett file
        #[arg(long)]
        output: String,

        /// Emit TOON agent output
        #[arg(long)]
        agent: bool,
    },

    /// Query compiler facts for agent tooling
    Query {
        /// Emit TOON agent output
        #[arg(long)]
        agent: bool,

        /// List public namespaces and discoverable definitions
        #[arg(long)]
        namespaces: bool,

        /// List top-level symbols in a single file
        #[arg(long)]
        symbols: Option<String>,

        /// Return the type at file:line:column
        #[arg(long = "type-at")]
        type_at: Option<String>,

        /// Return the definition target at file:line:column
        #[arg(long = "definition-at")]
        definition_at: Option<String>,

        /// Return references to the symbol at file:line:column
        #[arg(long = "references-at")]
        references_at: Option<String>,

        /// Return completions at file:line:column
        #[arg(long = "complete-at")]
        complete_at: Option<String>,

        /// Return a public function signature
        #[arg(long)]
        signature: Option<String>,
    },

    /// Start the Language Server Protocol server (for editor integration)
    Lsp,
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Command::Format { file, check, agent } => {
            let path = Path::new(&file);

            if check {
                // Check mode: compare formatted output to original
                match jett_driver::format_file(path) {
                    Ok(result) => {
                        if !result.errors.is_empty() {
                            if agent {
                                print!(
                                    "{}",
                                    render_format_agent_output(
                                        &file,
                                        "check",
                                        false,
                                        &result.errors
                                    )
                                );
                            } else {
                                eprintln!("error: {}", result.errors.join("\n"));
                            }
                            process::exit(1);
                        }
                        let original = std::fs::read_to_string(path).unwrap_or_default();
                        if result.output != original {
                            if agent {
                                print!("{}", render_format_agent_output(&file, "check", true, &[]));
                            } else {
                                eprintln!("{file} needs formatting");
                            }
                            process::exit(1);
                        } else if agent {
                            print!("{}", render_format_agent_output(&file, "check", false, &[]));
                        }
                    }
                    Err(e) => {
                        if agent {
                            print!("{}", render_format_agent_error(&file, &e));
                        } else {
                            eprintln!("error: {e}");
                        }
                        process::exit(1);
                    }
                }
            } else {
                // Format in place
                match jett_driver::format_file(path) {
                    Ok(result) => {
                        if !result.errors.is_empty() {
                            if agent {
                                print!(
                                    "{}",
                                    render_format_agent_output(
                                        &file,
                                        "write",
                                        false,
                                        &result.errors
                                    )
                                );
                            } else {
                                eprintln!("error: {}", result.errors.join("\n"));
                            }
                            process::exit(1);
                        }
                        let original = std::fs::read_to_string(path).unwrap_or_default();
                        let changed = result.output != original;
                        match jett_driver::format_file_in_place(path) {
                            Ok(()) => {
                                if agent {
                                    print!(
                                        "{}",
                                        render_format_agent_output(&file, "write", changed, &[])
                                    );
                                } else {
                                    println!("formatted {file}");
                                }
                            }
                            Err(e) => {
                                if agent {
                                    print!("{}", render_format_agent_error(&file, &e));
                                } else {
                                    eprintln!("error: {e}");
                                }
                                process::exit(1);
                            }
                        }
                    }
                    Err(e) => {
                        if agent {
                            print!("{}", render_format_agent_error(&file, &e));
                        } else {
                            eprintln!("error: {e}");
                        }
                        process::exit(1);
                    }
                }
            }
        }
        Command::Build {
            file,
            release: _,
            agent,
            target: _,
        } => {
            let path = Path::new(&file);
            let result = jett_driver::build_file(path);

            if agent {
                // TOON agent output mode
                let toon_output = jett_diagnostics::toon::render_toon(
                    &result.diagnostics,
                    &result.source,
                    &result.file_path,
                );
                print!("{toon_output}");

                if result.has_errors {
                    process::exit(1);
                }
            } else {
                // Human-readable output mode
                for diag in &result.diagnostics {
                    let rendered = jett_diagnostics::render::render_diagnostic(
                        diag,
                        &result.source,
                        &result.file_path,
                    );
                    eprint!("{rendered}");
                }

                if result.has_errors {
                    let error_count = result
                        .diagnostics
                        .iter()
                        .filter(|d| d.severity == jett_diagnostics::Severity::Error)
                        .count();
                    eprintln!("build failed: {error_count} error(s)");
                    process::exit(1);
                } else {
                    println!("build ok: {file} (type checked, no codegen yet)");
                }
            }
        }
        Command::Run { file, agent } => {
            let path = Path::new(&file);
            if agent {
                match jett_driver::run_file_capture_output(path) {
                    Ok(output) => {
                        print!("{}", render_run_agent_output(&file, &output));
                    }
                    Err(e) => {
                        print!("{}", render_run_agent_error(&file, &e));
                        process::exit(1);
                    }
                }
            } else {
                match jett_driver::run_file(path) {
                    Ok(()) => {}
                    Err(e) => {
                        eprintln!("error: {e}");
                        process::exit(1);
                    }
                }
            }
        }
        Command::Bundle {
            start,
            output,
            agent,
        } => {
            let start_path = start.unwrap_or_else(|| ".".to_string());
            match jett_driver::bundle_project_detailed(Path::new(&start_path), Path::new(&output)) {
                Ok(result) => {
                    if agent {
                        print!("{}", render_bundle_agent_output(&result));
                    } else {
                        println!(
                            "bundled {} files into {}",
                            result.files.len(),
                            result.output_path
                        );
                    }
                }
                Err(e) => {
                    if agent {
                        print!("{}", render_bundle_agent_error(&start_path, &output, &e));
                    } else {
                        eprintln!("bundle error: {e}");
                    }
                    process::exit(1);
                }
            }
        }
        Command::Lsp => {
            let rt = tokio::runtime::Runtime::new().expect("failed to create tokio runtime");
            rt.block_on(jett_lsp::run_server());
        }
        Command::Query {
            agent,
            namespaces,
            symbols,
            type_at,
            definition_at,
            references_at,
            complete_at,
            signature,
        } => {
            let query_count = usize::from(namespaces)
                + usize::from(symbols.is_some())
                + usize::from(type_at.is_some())
                + usize::from(definition_at.is_some())
                + usize::from(references_at.is_some())
                + usize::from(complete_at.is_some())
                + usize::from(signature.is_some());
            if query_count != 1 {
                if agent {
                    print!(
                        "{}",
                        render_query_agent_error("query requires exactly one query flag")
                    );
                } else {
                    eprintln!("error: query requires exactly one query flag");
                }
                process::exit(1);
            }

            if namespaces {
                let cwd = std::env::current_dir().unwrap_or_default();
                match jett_driver::query_namespaces(&cwd) {
                    Ok(result) => {
                        if agent {
                            print!("{}", render_query_namespaces_agent_output(&result));
                        } else {
                            print_query_namespaces_human(&result);
                        }
                    }
                    Err(e) => {
                        if agent {
                            print!("{}", render_query_agent_error(&e));
                        } else {
                            eprintln!("error: {e}");
                        }
                        process::exit(1);
                    }
                }
            }

            if let Some(file) = symbols {
                match jett_driver::query_file_symbols_detailed(Path::new(&file)) {
                    Ok(result) => {
                        if agent {
                            print!("{}", render_query_file_symbols_agent_output(&result));
                        } else {
                            print_query_file_symbols_human(&result);
                        }
                    }
                    Err(e) => {
                        if agent {
                            print!("{}", render_file_symbols_query_agent_error(&e));
                        } else {
                            eprintln!("error: {e}");
                        }
                        process::exit(1);
                    }
                }
            }

            if let Some(position) = type_at {
                let position = match parse_source_position(&position) {
                    Ok(position) => position,
                    Err(e) => {
                        if agent {
                            print!("{}", render_query_agent_error(&e));
                        } else {
                            eprintln!("error: {e}");
                        }
                        process::exit(1);
                    }
                };
                match jett_driver::query_type_at(
                    Path::new(&position.file),
                    position.line,
                    position.column,
                ) {
                    Ok(result) => {
                        if agent {
                            print!("{}", render_query_type_at_agent_output(&result));
                        } else {
                            print_query_type_at_human(&result);
                        }
                    }
                    Err(e) => {
                        if agent {
                            print!("{}", render_query_agent_error(&e));
                        } else {
                            eprintln!("error: {e}");
                        }
                        process::exit(1);
                    }
                }
            }

            if let Some(position) = definition_at {
                let position = match parse_source_position(&position) {
                    Ok(position) => position,
                    Err(e) => {
                        if agent {
                            print!("{}", render_query_agent_error(&e));
                        } else {
                            eprintln!("error: {e}");
                        }
                        process::exit(1);
                    }
                };
                match jett_driver::query_definition_at(
                    Path::new(&position.file),
                    position.line,
                    position.column,
                ) {
                    Ok(result) => {
                        if agent {
                            print!("{}", render_query_definition_at_agent_output(&result));
                        } else {
                            print_query_definition_at_human(&result);
                        }
                    }
                    Err(e) => {
                        if agent {
                            print!("{}", render_query_agent_error(&e));
                        } else {
                            eprintln!("error: {e}");
                        }
                        process::exit(1);
                    }
                }
            }

            if let Some(position) = references_at {
                let position = match parse_source_position(&position) {
                    Ok(position) => position,
                    Err(e) => {
                        if agent {
                            print!("{}", render_query_agent_error(&e));
                        } else {
                            eprintln!("error: {e}");
                        }
                        process::exit(1);
                    }
                };
                match jett_driver::query_references_at(
                    Path::new(&position.file),
                    position.line,
                    position.column,
                ) {
                    Ok(result) => {
                        if agent {
                            print!("{}", render_query_references_at_agent_output(&result));
                        } else {
                            print_query_references_at_human(&result);
                        }
                    }
                    Err(e) => {
                        if agent {
                            print!("{}", render_query_agent_error(&e));
                        } else {
                            eprintln!("error: {e}");
                        }
                        process::exit(1);
                    }
                }
            }

            if let Some(position) = complete_at {
                let position = match parse_source_position(&position) {
                    Ok(position) => position,
                    Err(e) => {
                        if agent {
                            print!("{}", render_query_agent_error(&e));
                        } else {
                            eprintln!("error: {e}");
                        }
                        process::exit(1);
                    }
                };
                match jett_driver::query_completions_at(
                    Path::new(&position.file),
                    position.line,
                    position.column,
                ) {
                    Ok(result) => {
                        if agent {
                            print!("{}", render_query_completions_agent_output(&result));
                        } else {
                            print_query_completions_human(&result);
                        }
                    }
                    Err(e) => {
                        if agent {
                            print!("{}", render_query_agent_error(&e));
                        } else {
                            eprintln!("error: {e}");
                        }
                        process::exit(1);
                    }
                }
            }

            if let Some(function_name) = signature {
                let cwd = std::env::current_dir().unwrap_or_default();
                match jett_driver::query_signature(&cwd, &function_name) {
                    Ok(result) => {
                        if agent {
                            print!(
                                "{}",
                                render_query_signature_agent_output(
                                    &function_name,
                                    result.as_ref()
                                )
                            );
                        } else {
                            print_query_signature_human(&function_name, result.as_ref());
                        }
                    }
                    Err(e) => {
                        if agent {
                            print!("{}", render_query_agent_error(&e));
                        } else {
                            eprintln!("error: {e}");
                        }
                        process::exit(1);
                    }
                }
            }
        }
        Command::Test { file, agent } => {
            if let Some(f) = file {
                // Test a single file
                let path = Path::new(&f);
                match jett_driver::test_file(path) {
                    Ok(result) => {
                        if agent {
                            print!("{}", render_test_agent_file_output(&result));
                        } else {
                            print_file_results(&result);
                            println!(
                                "\n{} block(s), {} passed, {} failed",
                                result.total, result.passed, result.failed
                            );
                        }
                        if result.failed > 0 {
                            process::exit(1);
                        }
                    }
                    Err(e) => {
                        if agent {
                            print!("{}", render_test_agent_error(Some(&f), &e));
                        } else {
                            eprintln!("error testing {f}: {e}");
                        }
                        process::exit(1);
                    }
                }
            } else {
                // Discover project and test all files
                let cwd = std::env::current_dir().unwrap_or_default();
                match jett_driver::test_project(&cwd) {
                    Ok(result) => {
                        if agent {
                            print!("{}", render_test_agent_project_output(&result));
                        } else {
                            for file_result in &result.file_results {
                                println!("--- {} ---", file_result.file_path);
                                print_file_results(file_result);
                                println!();
                            }
                            println!(
                                "{} file(s), {} block(s), {} passed, {} failed",
                                result.total_files,
                                result.total_blocks,
                                result.total_passed,
                                result.total_failed
                            );
                        }
                        if result.total_failed > 0 {
                            process::exit(1);
                        }
                    }
                    Err(e) => {
                        if agent {
                            print!("{}", render_test_agent_error(None, &e));
                        } else {
                            eprintln!("error: {e}");
                        }
                        process::exit(1);
                    }
                }
            }
        }
    }
}

fn render_run_agent_output(file: &str, output: &jett_driver::RunOutput) -> String {
    let mut out = String::new();
    out.push_str("status: ok\n");
    out.push_str(&format!("file: {}\n", escape_toon_scalar(file)));
    out.push_str(&format!("stdout: {}\n", escape_toon_scalar(&output.stdout)));
    out.push_str(&format!(
        "debug[{}]{{kind,message}}:\n",
        output.debug_output.len()
    ));
    for line in &output.debug_output {
        out.push_str(&format!(
            "  {},{}\n",
            debug_line_kind(line),
            escape_toon_scalar(line)
        ));
    }
    out
}

fn debug_line_kind(line: &str) -> &'static str {
    if line.starts_with("trace ") {
        "trace"
    } else if line.starts_with("breakpoint hit") {
        "breakpoint"
    } else {
        "debug"
    }
}

fn render_format_agent_output(file: &str, mode: &str, changed: bool, errors: &[String]) -> String {
    let status = if !errors.is_empty() || (mode == "check" && changed) {
        "error"
    } else {
        "ok"
    };
    let mut out = String::new();
    out.push_str(&format!("status: {status}\n"));
    out.push_str(&format!("file: {}\n", escape_toon_scalar(file)));
    out.push_str(&format!("mode: {}\n", escape_toon_scalar(mode)));
    out.push_str(&format!(
        "changed: {}\n",
        if changed { "true" } else { "false" }
    ));
    out.push_str(&format!("errors[{}]{{message}}:\n", errors.len()));
    for error in errors {
        out.push_str(&format!("  {}\n", escape_toon_scalar(error)));
    }
    out
}

fn render_format_agent_error(file: &str, error: &str) -> String {
    format!(
        "status: error\nfile: {}\nerror: {}\n",
        escape_toon_scalar(file),
        escape_toon_scalar(error)
    )
}

fn render_run_agent_error(file: &str, error: &str) -> String {
    format!(
        "status: error\nfile: {}\nerror: {}\n",
        escape_toon_scalar(file),
        escape_toon_scalar(error)
    )
}

fn render_test_agent_file_output(result: &jett_driver::TestResult) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "status: {}\n",
        if result.failed == 0 { "ok" } else { "error" }
    ));
    out.push_str("files: 1\n");
    out.push_str(&format!("total: {}\n", result.total));
    out.push_str(&format!("passed: {}\n", result.passed));
    out.push_str(&format!("failed: {}\n", result.failed));
    append_test_agent_blocks(&mut out, std::slice::from_ref(result));
    out
}

fn render_test_agent_project_output(result: &jett_driver::ProjectTestResult) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "status: {}\n",
        if result.total_failed == 0 {
            "ok"
        } else {
            "error"
        }
    ));
    out.push_str(&format!("files: {}\n", result.total_files));
    out.push_str(&format!("total: {}\n", result.total_blocks));
    out.push_str(&format!("passed: {}\n", result.total_passed));
    out.push_str(&format!("failed: {}\n", result.total_failed));
    append_test_agent_blocks(&mut out, &result.file_results);
    out
}

fn render_test_agent_error(file: Option<&str>, error: &str) -> String {
    let mut out = String::new();
    out.push_str("status: error\n");
    if let Some(file) = file {
        out.push_str(&format!("file: {}\n", escape_toon_scalar(file)));
    }
    out.push_str(&format!("error: {}\n", escape_toon_scalar(error)));
    out
}

fn render_bundle_agent_output(result: &jett_driver::BundleResult) -> String {
    let mut out = String::new();
    out.push_str("status: ok\n");
    out.push_str(&format!(
        "project_root: {}\n",
        escape_toon_scalar(&result.project_root)
    ));
    out.push_str(&format!(
        "output: {}\n",
        escape_toon_scalar(&result.output_path)
    ));
    out.push_str(&format!("files: {}\n", result.files.len()));
    out.push_str(&format!(
        "bundled_files[{}]{{path,start_line,end_line}}:\n",
        result.files.len()
    ));
    for file in &result.files {
        out.push_str(&format!(
            "  {},{},{}\n",
            escape_toon_scalar(&file.path),
            file.start_line,
            file.end_line
        ));
    }
    out
}

fn render_bundle_agent_error(
    start: &str,
    output: &str,
    error: &jett_driver::BundleError,
) -> String {
    let mut out = format!(
        "status: error\nstart: {}\noutput: {}\n",
        escape_toon_scalar(start),
        escape_toon_scalar(output)
    );
    if let Some(result) = error.diagnostic_result() {
        out.push_str(&format!(
            "kind: {}\n",
            error.kind_name().unwrap_or("diagnostic")
        ));
        let diagnostics = jett_diagnostics::toon::render_toon(
            &result.diagnostics,
            &result.source,
            &result.file_path,
        );
        out.push_str(
            diagnostics
                .strip_prefix("status: error\n")
                .unwrap_or(&diagnostics),
        );
    } else {
        out.push_str(&format!(
            "error: {}\n",
            escape_toon_scalar(&error.to_string())
        ));
    }
    out
}

fn render_query_namespaces_agent_output(result: &jett_driver::NamespaceQueryResult) -> String {
    let mut out = String::new();
    out.push_str("status: ok\n");
    out.push_str(&format!("total: {}\n", result.definitions.len()));
    out.push_str("definitions");
    out.push_str(&format!(
        "[{}]{{name,kind,namespace,visibility,file,line,column,end_line,end_column}}:\n",
        result.definitions.len()
    ));
    for definition in &result.definitions {
        out.push_str(&format!(
            "  {},{},{},{},{},{},{},{},{}\n",
            escape_toon_scalar(&definition.name),
            jett_driver::query_kind_name(definition.kind),
            escape_toon_scalar(definition.namespace.as_deref().unwrap_or("")),
            jett_driver::query_visibility_name(definition.visibility),
            escape_toon_scalar(&definition.file_path),
            definition.line,
            definition.column,
            definition.end_line,
            definition.end_column
        ));
    }
    out
}

fn render_query_file_symbols_agent_output(result: &jett_driver::FileSymbolsQueryResult) -> String {
    let mut out = String::new();
    out.push_str("status: ok\n");
    out.push_str(&format!(
        "file: {}\n",
        escape_toon_scalar(&result.file_path)
    ));
    out.push_str(&format!("total: {}\n", result.symbols.len()));
    out.push_str(&format!(
        "symbols[{}]{{name,kind,namespace,visibility,signature,line,column,end_line,end_column}}:\n",
        result.symbols.len()
    ));
    for symbol in &result.symbols {
        out.push_str(&format!(
            "  {},{},{},{},{},{},{},{},{}\n",
            escape_toon_scalar(&symbol.name),
            escape_toon_scalar(&symbol.kind),
            escape_toon_scalar(symbol.namespace.as_deref().unwrap_or("")),
            jett_driver::query_visibility_name(symbol.visibility),
            escape_toon_scalar(symbol.signature.as_deref().unwrap_or("")),
            symbol.line,
            symbol.column,
            symbol.end_line,
            symbol.end_column
        ));
    }
    out
}

fn render_query_agent_error(error: &str) -> String {
    format!("status: error\nerror: {}\n", escape_toon_scalar(error))
}

fn render_file_symbols_query_agent_error(error: &jett_driver::FileSymbolsQueryError) -> String {
    if let Some((diagnostics, source, file_path)) = error.diagnostic_context() {
        jett_diagnostics::toon::render_toon(diagnostics, source, file_path)
    } else {
        render_query_agent_error(&error.to_string())
    }
}

fn render_query_type_at_agent_output(result: &jett_driver::TypeAtQueryResult) -> String {
    let mut out = String::new();
    out.push_str("status: ok\n");
    out.push_str(&format!(
        "file: {}\n",
        escape_toon_scalar(&result.file_path)
    ));
    out.push_str(&format!("line: {}\n", result.line));
    out.push_str(&format!("column: {}\n", result.column));
    out.push_str(&format!(
        "found: {}\n",
        if result.type_name.is_some() {
            "true"
        } else {
            "false"
        }
    ));
    out.push_str(&format!(
        "type: {}\n",
        escape_toon_scalar(result.type_name.as_deref().unwrap_or(""))
    ));
    if let (Some(line), Some(column), Some(end_line), Some(end_column)) = (
        result.span_line,
        result.span_column,
        result.span_end_line,
        result.span_end_column,
    ) {
        out.push_str(&format!("span_line: {}\n", line));
        out.push_str(&format!("span_column: {}\n", column));
        out.push_str(&format!("span_end_line: {}\n", end_line));
        out.push_str(&format!("span_end_column: {}\n", end_column));
    }
    out
}

fn render_query_definition_at_agent_output(
    result: &jett_driver::DefinitionAtQueryResult,
) -> String {
    let mut out = String::new();
    out.push_str("status: ok\n");
    out.push_str(&format!(
        "file: {}\n",
        escape_toon_scalar(&result.file_path)
    ));
    out.push_str(&format!("line: {}\n", result.line));
    out.push_str(&format!("column: {}\n", result.column));
    out.push_str(&format!(
        "found: {}\n",
        if result.target.is_some() {
            "true"
        } else {
            "false"
        }
    ));

    let Some(target) = &result.target else {
        return out;
    };

    append_query_definition_target_agent_output(&mut out, target);
    out
}

fn render_query_references_at_agent_output(
    result: &jett_driver::ReferencesAtQueryResult,
) -> String {
    let mut out = String::new();
    out.push_str("status: ok\n");
    out.push_str(&format!(
        "file: {}\n",
        escape_toon_scalar(&result.file_path)
    ));
    out.push_str(&format!("line: {}\n", result.line));
    out.push_str(&format!("column: {}\n", result.column));
    out.push_str(&format!(
        "found: {}\n",
        if result.target.is_some() {
            "true"
        } else {
            "false"
        }
    ));
    if let Some(target) = &result.target {
        append_query_definition_target_agent_output(&mut out, target);
    }
    out.push_str(&format!("total: {}\n", result.references.len()));
    out.push_str(&format!(
        "references[{}]{{file,line,column,end_line,end_column}}:\n",
        result.references.len()
    ));
    for reference in &result.references {
        out.push_str(&format!(
            "  {},{},{},{},{}\n",
            escape_toon_scalar(&reference.file_path),
            reference.line,
            reference.column,
            reference.end_line,
            reference.end_column
        ));
    }
    out
}

fn append_query_definition_target_agent_output(
    out: &mut String,
    target: &jett_driver::DefinitionQueryTarget,
) {
    out.push_str(&format!("target: {}\n", escape_toon_scalar(&target.name)));
    out.push_str(&format!(
        "kind: {}\n",
        jett_driver::query_kind_name(target.kind)
    ));
    out.push_str(&format!(
        "namespace: {}\n",
        escape_toon_scalar(target.namespace.as_deref().unwrap_or(""))
    ));
    out.push_str(&format!(
        "visibility: {}\n",
        jett_driver::query_visibility_name(target.visibility)
    ));
    out.push_str(&format!(
        "target_file: {}\n",
        escape_toon_scalar(&target.file_path)
    ));
    out.push_str(&format!("target_line: {}\n", target.line));
    out.push_str(&format!("target_column: {}\n", target.column));
    out.push_str(&format!("target_end_line: {}\n", target.end_line));
    out.push_str(&format!("target_end_column: {}\n", target.end_column));
}

fn render_query_completions_agent_output(result: &jett_driver::CompletionsQueryResult) -> String {
    let mut out = String::new();
    out.push_str("status: ok\n");
    out.push_str(&format!(
        "file: {}\n",
        escape_toon_scalar(&result.file_path)
    ));
    out.push_str(&format!("line: {}\n", result.line));
    out.push_str(&format!("column: {}\n", result.column));
    out.push_str(&format!("prefix: {}\n", escape_toon_scalar(&result.prefix)));
    out.push_str(&format!("total: {}\n", result.candidates.len()));
    out.push_str(&format!(
        "completions[{}]{{rank,match,name,kind,namespace,visibility,file,line,column,end_line,end_column,signature}}:\n",
        result.candidates.len()
    ));
    for candidate in &result.candidates {
        out.push_str(&format!(
            "  {},{},{},{},{},{},{},{},{},{},{},{}\n",
            candidate.rank,
            jett_driver::completion_match_kind_name(candidate.match_kind),
            escape_toon_scalar(&candidate.name),
            jett_driver::query_kind_name(candidate.kind),
            escape_toon_scalar(candidate.namespace.as_deref().unwrap_or("")),
            jett_driver::query_visibility_name(candidate.visibility),
            escape_toon_scalar(&candidate.file_path),
            candidate.line,
            candidate.column,
            candidate.end_line,
            candidate.end_column,
            escape_toon_scalar(candidate.signature.as_deref().unwrap_or(""))
        ));
    }
    out
}

fn render_query_signature_agent_output(
    function_name: &str,
    result: Option<&jett_driver::SignatureQueryResult>,
) -> String {
    let mut out = String::new();
    out.push_str("status: ok\n");
    out.push_str(&format!(
        "function: {}\n",
        escape_toon_scalar(function_name)
    ));
    let Some(result) = result else {
        out.push_str("found: false\n");
        return out;
    };

    out.push_str("found: true\n");
    out.push_str(&format!(
        "file: {}\n",
        escape_toon_scalar(&result.file_path)
    ));
    out.push_str(&format!(
        "returns: {}\n",
        escape_toon_scalar(&result.return_type)
    ));
    out.push_str(&format!(
        "type_params[{}]{{name}}:\n",
        result.type_params.len()
    ));
    for type_param in &result.type_params {
        out.push_str(&format!("  {}\n", escape_toon_scalar(type_param)));
    }
    out.push_str(&format!(
        "params[{}]{{name,type,view,mutable}}:\n",
        result.params.len()
    ));
    for param in &result.params {
        out.push_str(&format!(
            "  {},{},{},{}\n",
            escape_toon_scalar(&param.name),
            escape_toon_scalar(&param.type_name),
            if param.view { "true" } else { "false" },
            if param.mutable { "true" } else { "false" }
        ));
    }
    out
}

fn print_query_namespaces_human(result: &jett_driver::NamespaceQueryResult) {
    for definition in &result.definitions {
        let namespace = definition.namespace.as_deref().unwrap_or("-");
        println!(
            "{}\t{}\t{}\t{}",
            jett_driver::query_kind_name(definition.kind),
            definition.name,
            namespace,
            definition.file_path
        );
    }
}

fn print_query_file_symbols_human(result: &jett_driver::FileSymbolsQueryResult) {
    for symbol in &result.symbols {
        let namespace = symbol.namespace.as_deref().unwrap_or("-");
        println!(
            "{}\t{}\t{}\t{}\t{}:{}",
            symbol.kind,
            symbol.name,
            namespace,
            jett_driver::query_visibility_name(symbol.visibility),
            symbol.line,
            symbol.column
        );
    }
}

fn print_query_signature_human(
    function_name: &str,
    result: Option<&jett_driver::SignatureQueryResult>,
) {
    let Some(result) = result else {
        println!("no signature found for {function_name}");
        return;
    };

    let type_params = if result.type_params.is_empty() {
        String::new()
    } else {
        format!("[{}]", result.type_params.join(", "))
    };
    let params: Vec<String> = result
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
    println!(
        "{}{}({}) returns {}",
        result.name,
        type_params,
        params.join(", "),
        result.return_type
    );
}

fn print_query_completions_human(result: &jett_driver::CompletionsQueryResult) {
    for candidate in &result.candidates {
        println!(
            "{}\t{}\t{}",
            jett_driver::query_kind_name(candidate.kind),
            candidate.name,
            candidate.signature.as_deref().unwrap_or("")
        );
    }
}

fn print_query_type_at_human(result: &jett_driver::TypeAtQueryResult) {
    match result.type_name.as_deref() {
        Some(type_name) => println!("{type_name}"),
        None => println!(
            "no type found at {}:{}:{}",
            result.file_path, result.line, result.column
        ),
    }
}

fn print_query_definition_at_human(result: &jett_driver::DefinitionAtQueryResult) {
    match &result.target {
        Some(target) => println!(
            "{}\t{}\t{}:{}:{}",
            jett_driver::query_kind_name(target.kind),
            target.name,
            target.file_path,
            target.line,
            target.column
        ),
        None => println!(
            "no definition found at {}:{}:{}",
            result.file_path, result.line, result.column
        ),
    }
}

fn print_query_references_at_human(result: &jett_driver::ReferencesAtQueryResult) {
    let Some(target) = &result.target else {
        println!(
            "no references found at {}:{}:{}",
            result.file_path, result.line, result.column
        );
        return;
    };

    println!(
        "{}\t{}\t{} reference(s)",
        jett_driver::query_kind_name(target.kind),
        target.name,
        result.references.len()
    );
    for reference in &result.references {
        println!(
            "{}:{}:{}",
            reference.file_path, reference.line, reference.column
        );
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SourcePosition {
    file: String,
    line: u32,
    column: u32,
}

fn parse_source_position(raw: &str) -> Result<SourcePosition, String> {
    let mut parts = raw.rsplitn(3, ':');
    let column = parts
        .next()
        .ok_or_else(|| "position must be file:line:column".to_string())?;
    let line = parts
        .next()
        .ok_or_else(|| "position must be file:line:column".to_string())?;
    let file = parts
        .next()
        .ok_or_else(|| "position must be file:line:column".to_string())?;

    if file.is_empty() {
        return Err("position file must not be empty".to_string());
    }

    let line = line
        .parse::<u32>()
        .map_err(|_| "position line must be a positive integer".to_string())?;
    let column = column
        .parse::<u32>()
        .map_err(|_| "position column must be a positive integer".to_string())?;
    if line == 0 || column == 0 {
        return Err("position line and column are 1-based".to_string());
    }

    Ok(SourcePosition {
        file: file.to_string(),
        line,
        column,
    })
}

fn append_test_agent_blocks(out: &mut String, file_results: &[jett_driver::TestResult]) {
    let block_count: usize = file_results.iter().map(|result| result.blocks.len()).sum();
    out.push_str(&format!(
        "blocks[{}]{{file,name,kind,status,iterations,line,column,end_line,end_column,error}}:\n",
        block_count
    ));

    for file_result in file_results {
        for block in &file_result.blocks {
            let kind = if block.is_property {
                "property"
            } else {
                "verify"
            };
            let status = if block.passed { "passed" } else { "failed" };
            let iterations = block.iterations.map(|value| value.to_string());
            let error = block.error.as_deref().unwrap_or("");
            out.push_str(&format!(
                "  {},{},{},{},{},{},{},{},{},{}\n",
                escape_toon_scalar(&file_result.file_path),
                escape_toon_scalar(&block.name),
                kind,
                status,
                iterations.as_deref().unwrap_or(""),
                block.line,
                block.column,
                block.end_line,
                block.end_column,
                escape_toon_scalar(error)
            ));
        }
    }
}

fn escape_toon_scalar(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('\r', "\\r")
        .replace('\n', "\\n")
        .replace(',', "\\,")
}

fn print_file_results(result: &jett_driver::TestResult) {
    for block in &result.blocks {
        let name = &block.name;
        if block.is_property {
            if block.passed {
                let iters = block.iterations.unwrap_or(0);
                println!("  property {name}: ok ({iters} iterations)");
            } else {
                let msg = block.error.as_deref().unwrap_or("unknown error");
                println!("  property {name}: FAILED ({msg})");
            }
        } else if block.passed {
            println!("  verify {name}: ok");
        } else {
            let msg = block.error.as_deref().unwrap_or("unknown error");
            println!("  verify {name}: FAILED ({msg})");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn run_agent_output_includes_stdout_and_debug_rows() {
        let output = jett_driver::RunOutput {
            stdout: "hello\n".to_string(),
            debug_output: vec![
                "trace total: int64 = 42".to_string(),
                "breakpoint hit: total: int64 = 42".to_string(),
            ],
        };

        let rendered = render_run_agent_output("app.jett", &output);

        assert_eq!(
            rendered,
            "status: ok\nfile: app.jett\nstdout: hello\\n\ndebug[2]{kind,message}:\n  trace,trace total: int64 = 42\n  breakpoint,breakpoint hit: total: int64 = 42\n"
        );
    }

    #[test]
    fn run_agent_error_escapes_multiline_message() {
        let rendered = render_run_agent_error("app.jett", "runtime error: bad\nline");

        assert_eq!(
            rendered,
            "status: error\nfile: app.jett\nerror: runtime error: bad\\nline\n"
        );
    }

    #[test]
    fn format_agent_check_reports_needed_change() {
        let rendered = render_format_agent_output("app.jett", "check", true, &[]);

        assert_eq!(
            rendered,
            "status: error\nfile: app.jett\nmode: check\nchanged: true\nerrors[0]{message}:\n"
        );
    }

    #[test]
    fn format_agent_output_lists_errors() {
        let errors = vec!["bad token\nline".to_string()];
        let rendered = render_format_agent_output("app.jett", "write", false, &errors);

        assert_eq!(
            rendered,
            "status: error\nfile: app.jett\nmode: write\nchanged: false\nerrors[1]{message}:\n  bad token\\nline\n"
        );
    }

    #[test]
    fn test_agent_file_output_lists_verify_and_property_blocks() {
        let result = jett_driver::TestResult {
            total: 2,
            passed: 1,
            failed: 1,
            file_path: "tests/sample.jett".to_string(),
            blocks: vec![
                jett_driver::TestBlockResult {
                    name: "adds".to_string(),
                    passed: true,
                    error: None,
                    is_property: false,
                    iterations: None,
                    line: 3,
                    column: 8,
                    end_line: 3,
                    end_column: 12,
                },
                jett_driver::TestBlockResult {
                    name: "roundtrip".to_string(),
                    passed: false,
                    error: Some("expected ok, got bad\ncase".to_string()),
                    is_property: true,
                    iterations: Some(42),
                    line: 8,
                    column: 10,
                    end_line: 8,
                    end_column: 19,
                },
            ],
        };

        let rendered = render_test_agent_file_output(&result);

        assert_eq!(
            rendered,
            "status: error\nfiles: 1\ntotal: 2\npassed: 1\nfailed: 1\nblocks[2]{file,name,kind,status,iterations,line,column,end_line,end_column,error}:\n  tests/sample.jett,adds,verify,passed,,3,8,3,12,\n  tests/sample.jett,roundtrip,property,failed,42,8,10,8,19,expected ok\\, got bad\\ncase\n"
        );
    }

    #[test]
    fn test_agent_project_output_summarizes_all_files() {
        let result = jett_driver::ProjectTestResult {
            total_files: 2,
            total_blocks: 1,
            total_passed: 1,
            total_failed: 0,
            file_results: vec![
                jett_driver::TestResult {
                    total: 0,
                    passed: 0,
                    failed: 0,
                    file_path: "tests/empty.jett".to_string(),
                    blocks: Vec::new(),
                },
                jett_driver::TestResult {
                    total: 1,
                    passed: 1,
                    failed: 0,
                    file_path: "tests/checks.jett".to_string(),
                    blocks: vec![jett_driver::TestBlockResult {
                        name: "ok".to_string(),
                        passed: true,
                        error: None,
                        is_property: false,
                        iterations: None,
                        line: 4,
                        column: 8,
                        end_line: 4,
                        end_column: 10,
                    }],
                },
            ],
        };

        let rendered = render_test_agent_project_output(&result);

        assert_eq!(
            rendered,
            "status: ok\nfiles: 2\ntotal: 1\npassed: 1\nfailed: 0\nblocks[1]{file,name,kind,status,iterations,line,column,end_line,end_column,error}:\n  tests/checks.jett,ok,verify,passed,,4,8,4,10,\n"
        );
    }

    #[test]
    fn test_agent_error_escapes_file_and_message() {
        let rendered = render_test_agent_error(Some("bad,path.jett"), "parse\nfailed");

        assert_eq!(
            rendered,
            "status: error\nfile: bad\\,path.jett\nerror: parse\\nfailed\n"
        );
    }

    #[test]
    fn bundle_agent_output_lists_bundled_files() {
        let result = jett_driver::BundleResult {
            project_root: "project".to_string(),
            output_path: "dist/lib.jett".to_string(),
            files: vec![jett_driver::BundleFileResult {
                path: "src/core.jett".to_string(),
                start_line: 4,
                end_line: 12,
            }],
        };

        let rendered = render_bundle_agent_output(&result);

        assert_eq!(
            rendered,
            "status: ok\nproject_root: project\noutput: dist/lib.jett\nfiles: 1\nbundled_files[1]{path,start_line,end_line}:\n  src/core.jett,4,12\n"
        );
    }

    #[test]
    fn bundle_agent_validation_error_lists_structured_diagnostics() {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system time should be after the Unix epoch")
            .as_nanos();
        let root = std::env::temp_dir().join(format!("jett_cli_bundle_agent_error_{nanos}"));
        std::fs::create_dir_all(root.join("src"))
            .expect("temporary bundle project should be created");
        std::fs::write(root.join("jett.proj"), "name: bundle_error\n")
            .expect("project marker should be written");
        std::fs::write(
            root.join("src/broken.jett"),
            "function broken() returns int64:\n    return missing\n",
        )
        .expect("invalid source should be written");
        let error = match jett_driver::bundle_project_detailed(&root, Path::new("dist/lib.jett")) {
            Ok(_) => panic!("invalid bundle should fail validation"),
            Err(error) => error,
        };

        let rendered = render_bundle_agent_error(".", "dist/lib.jett", &error);
        std::fs::remove_dir_all(&root).expect("temporary bundle project should be removed");

        assert!(rendered.starts_with(
            "status: error\nstart: .\noutput: dist/lib.jett\nkind: validation\nfile: "
        ));
        assert!(rendered.contains(
            "diagnostics[1]{code,severity,message,file,line,column,end_line,end_column}:"
        ));
        assert!(rendered.contains("E0200,error,undefined name: `missing`,"));
        assert!(!rendered.contains("error: candidate bundle failed validation"));
    }

    #[test]
    fn bundle_agent_ordering_error_lists_structured_diagnostics() {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system time should be after the Unix epoch")
            .as_nanos();
        let root = std::env::temp_dir().join(format!("jett_cli_bundle_order_error_{nanos}"));
        std::fs::create_dir_all(root.join("src"))
            .expect("temporary bundle project should be created");
        std::fs::write(root.join("jett.proj"), "name: bundle_error\n")
            .expect("project marker should be written");
        std::fs::write(
            root.join("src/alpha.jett"),
            "namespace alpha\n\nexport function value() returns int64:\n    return beta.value()\n",
        )
        .expect("alpha source should be written");
        std::fs::write(
            root.join("src/beta.jett"),
            "namespace beta\n\nexport function value() returns int64:\n    return alpha.value()\n",
        )
        .expect("beta source should be written");
        let error = match jett_driver::bundle_project_detailed(&root, Path::new("dist/lib.jett")) {
            Ok(_) => panic!("cyclic bundle should fail ordering"),
            Err(error) => error,
        };

        let rendered = render_bundle_agent_error(".", "dist/lib.jett", &error);
        std::fs::remove_dir_all(&root).expect("temporary bundle project should be removed");

        assert!(
            rendered.starts_with(
                "status: error\nstart: .\noutput: dist/lib.jett\nkind: ordering\nfile: "
            )
        );
        assert!(rendered.contains(
            "diagnostics[1]{code,severity,message,file,line,column,end_line,end_column}:"
        ));
        assert!(rendered.contains("bundle ordering cycle requires declaration interleaving"));
        assert!(!rendered.contains("error: bundle ordering cycle"));
    }

    #[test]
    fn query_namespaces_agent_output_lists_definition_rows() {
        let result = jett_driver::NamespaceQueryResult {
            definitions: vec![jett_driver::QueryDefinition {
                name: "api.login".to_string(),
                kind: jett_resolve::scope::DefKind::Function,
                namespace: Some("api".to_string()),
                visibility: jett_resolve::scope::DefVisibility::Public,
                file_path: "src/api.jett".to_string(),
                line: 3,
                column: 17,
                end_line: 3,
                end_column: 22,
            }],
        };

        let rendered = render_query_namespaces_agent_output(&result);

        assert_eq!(
            rendered,
            "status: ok\ntotal: 1\ndefinitions[1]{name,kind,namespace,visibility,file,line,column,end_line,end_column}:\n  api.login,function,api,public,src/api.jett,3,17,3,22\n"
        );
    }

    #[test]
    fn query_file_symbols_agent_output_lists_symbol_rows() {
        let result = jett_driver::FileSymbolsQueryResult {
            file_path: "src/api.jett".to_string(),
            symbols: vec![jett_driver::FileSymbolQueryEntry {
                name: "api.login".to_string(),
                kind: "function".to_string(),
                namespace: Some("api".to_string()),
                visibility: jett_resolve::scope::DefVisibility::Public,
                signature: Some("api.login(raw: string) returns int64".to_string()),
                line: 3,
                column: 17,
                end_line: 3,
                end_column: 22,
            }],
        };

        let rendered = render_query_file_symbols_agent_output(&result);

        assert_eq!(
            rendered,
            "status: ok\nfile: src/api.jett\ntotal: 1\nsymbols[1]{name,kind,namespace,visibility,signature,line,column,end_line,end_column}:\n  api.login,function,api,public,api.login(raw: string) returns int64,3,17,3,22\n"
        );
    }

    #[test]
    fn query_agent_error_escapes_message() {
        let rendered = render_query_agent_error("query\nfailed");

        assert_eq!(rendered, "status: error\nerror: query\\nfailed\n");
    }

    #[test]
    fn file_symbols_query_agent_error_lists_structured_parse_diagnostics() {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system time should be after the Unix epoch")
            .as_nanos();
        let root = std::env::temp_dir().join(format!("jett_cli_symbols_agent_error_{nanos}"));
        std::fs::create_dir_all(&root).expect("temporary query directory should be created");
        let file = root.join("broken.jett");
        std::fs::write(&file, "function broken( returns int64:\n    return 1\n")
            .expect("invalid source should be written");
        let error = jett_driver::query_file_symbols_detailed(&file)
            .expect_err("invalid symbols query should fail");

        let rendered = render_file_symbols_query_agent_error(&error);
        std::fs::remove_dir_all(&root).expect("temporary query directory should be removed");

        assert!(rendered.starts_with("status: error\nfile: "));
        assert!(rendered.contains("{code,severity,message,file,line,column,end_line,end_column}:"));
        assert!(rendered.contains("E1000,error,"));
        assert!(!rendered.contains("error: parse errors"));
    }

    #[test]
    fn query_type_at_agent_output_reports_found_type() {
        let result = jett_driver::TypeAtQueryResult {
            file_path: "src/main.jett".to_string(),
            line: 4,
            column: 19,
            type_name: Some("int64".to_string()),
            span_line: Some(4),
            span_column: Some(19),
            span_end_line: Some(4),
            span_end_column: Some(24),
        };

        let rendered = render_query_type_at_agent_output(&result);

        assert_eq!(
            rendered,
            "status: ok\nfile: src/main.jett\nline: 4\ncolumn: 19\nfound: true\ntype: int64\nspan_line: 4\nspan_column: 19\nspan_end_line: 4\nspan_end_column: 24\n"
        );
    }

    #[test]
    fn parse_source_position_splits_from_the_right() {
        assert_eq!(
            parse_source_position(r"C:\project\main.jett:12:34"),
            Ok(SourcePosition {
                file: r"C:\project\main.jett".to_string(),
                line: 12,
                column: 34,
            })
        );
    }

    #[test]
    fn query_definition_at_agent_output_reports_target() {
        let result = jett_driver::DefinitionAtQueryResult {
            file_path: "src/main.jett".to_string(),
            line: 6,
            column: 12,
            target: Some(jett_driver::DefinitionQueryTarget {
                name: "models.User".to_string(),
                kind: jett_resolve::scope::DefKind::Struct,
                namespace: Some("models".to_string()),
                visibility: jett_resolve::scope::DefVisibility::Public,
                file_path: "src/models.jett".to_string(),
                line: 3,
                column: 15,
                end_line: 3,
                end_column: 19,
            }),
        };

        let rendered = render_query_definition_at_agent_output(&result);

        assert_eq!(
            rendered,
            "status: ok\nfile: src/main.jett\nline: 6\ncolumn: 12\nfound: true\ntarget: models.User\nkind: struct\nnamespace: models\nvisibility: public\ntarget_file: src/models.jett\ntarget_line: 3\ntarget_column: 15\ntarget_end_line: 3\ntarget_end_column: 19\n"
        );
    }

    #[test]
    fn query_references_at_agent_output_lists_references() {
        let result = jett_driver::ReferencesAtQueryResult {
            file_path: "src/main.jett".to_string(),
            line: 6,
            column: 12,
            target: Some(jett_driver::DefinitionQueryTarget {
                name: "mathlib.double".to_string(),
                kind: jett_resolve::scope::DefKind::Function,
                namespace: Some("mathlib".to_string()),
                visibility: jett_resolve::scope::DefVisibility::Public,
                file_path: "src/mathlib.jett".to_string(),
                line: 3,
                column: 17,
                end_line: 3,
                end_column: 23,
            }),
            references: vec![
                jett_driver::ReferenceQueryEntry {
                    file_path: "src/main.jett".to_string(),
                    line: 6,
                    column: 12,
                    end_line: 6,
                    end_column: 18,
                },
                jett_driver::ReferenceQueryEntry {
                    file_path: "src/other.jett".to_string(),
                    line: 9,
                    column: 20,
                    end_line: 9,
                    end_column: 26,
                },
            ],
        };

        let rendered = render_query_references_at_agent_output(&result);

        assert_eq!(
            rendered,
            "status: ok\nfile: src/main.jett\nline: 6\ncolumn: 12\nfound: true\ntarget: mathlib.double\nkind: function\nnamespace: mathlib\nvisibility: public\ntarget_file: src/mathlib.jett\ntarget_line: 3\ntarget_column: 17\ntarget_end_line: 3\ntarget_end_column: 23\ntotal: 2\nreferences[2]{file,line,column,end_line,end_column}:\n  src/main.jett,6,12,6,18\n  src/other.jett,9,20,9,26\n"
        );
    }

    #[test]
    fn query_completions_agent_output_lists_candidates() {
        let result = jett_driver::CompletionsQueryResult {
            file_path: "src/main.jett".to_string(),
            line: 4,
            column: 5,
            prefix: "json.pa".to_string(),
            candidates: vec![jett_driver::CompletionQueryEntry {
                name: "json.parse".to_string(),
                kind: jett_resolve::scope::DefKind::Function,
                namespace: Some("json".to_string()),
                visibility: jett_resolve::scope::DefVisibility::Public,
                file_path: "stdlib/json/90_public_api.jett".to_string(),
                line: 12,
                column: 17,
                end_line: 12,
                end_column: 22,
                match_kind: jett_driver::CompletionMatchKind::QualifiedPrefix,
                rank: 10,
                signature: Some("json.parse[T](raw: string) returns result[T, string]".to_string()),
            }],
        };

        let rendered = render_query_completions_agent_output(&result);

        assert_eq!(
            rendered,
            "status: ok\nfile: src/main.jett\nline: 4\ncolumn: 5\nprefix: json.pa\ntotal: 1\ncompletions[1]{rank,match,name,kind,namespace,visibility,file,line,column,end_line,end_column,signature}:\n  10,qualified_prefix,json.parse,function,json,public,stdlib/json/90_public_api.jett,12,17,12,22,json.parse[T](raw: string) returns result[T\\, string]\n"
        );
    }

    #[test]
    fn query_signature_agent_output_lists_params() {
        let result = jett_driver::SignatureQueryResult {
            name: "json.parse".to_string(),
            type_params: vec!["T".to_string()],
            params: vec![jett_driver::SignatureParam {
                name: "raw".to_string(),
                type_name: "string".to_string(),
                view: false,
                mutable: false,
            }],
            return_type: "result[T, string]".to_string(),
            file_path: "stdlib/json/90_public_api.jett".to_string(),
        };

        let rendered = render_query_signature_agent_output("json.parse", Some(&result));

        assert_eq!(
            rendered,
            "status: ok\nfunction: json.parse\nfound: true\nfile: stdlib/json/90_public_api.jett\nreturns: result[T\\, string]\ntype_params[1]{name}:\n  T\nparams[1]{name,type,view,mutable}:\n  raw,string,false,false\n"
        );
    }
}
