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
    },

    /// Start the Language Server Protocol server (for editor integration)
    Lsp,
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Command::Format { file, check } => {
            let path = Path::new(&file);

            if check {
                // Check mode: compare formatted output to original
                match jett_driver::format_file(path) {
                    Ok(result) => {
                        if !result.errors.is_empty() {
                            eprintln!("error: {}", result.errors.join("\n"));
                            process::exit(1);
                        }
                        let original = std::fs::read_to_string(path).unwrap_or_default();
                        if result.output != original {
                            eprintln!("{file} needs formatting");
                            process::exit(1);
                        }
                    }
                    Err(e) => {
                        eprintln!("error: {e}");
                        process::exit(1);
                    }
                }
            } else {
                // Format in place
                match jett_driver::format_file_in_place(path) {
                    Ok(()) => {
                        println!("formatted {file}");
                    }
                    Err(e) => {
                        eprintln!("error: {e}");
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
        Command::Lsp => {
            let rt = tokio::runtime::Runtime::new().expect("failed to create tokio runtime");
            rt.block_on(jett_lsp::run_server());
        }
        Command::Test { file } => {
            if let Some(f) = file {
                // Test a single file
                let path = Path::new(&f);
                match jett_driver::test_file(path) {
                    Ok(result) => {
                        print_file_results(&result);
                        println!(
                            "\n{} block(s), {} passed, {} failed",
                            result.total, result.passed, result.failed
                        );
                        if result.failed > 0 {
                            process::exit(1);
                        }
                    }
                    Err(e) => {
                        eprintln!("error testing {f}: {e}");
                        process::exit(1);
                    }
                }
            } else {
                // Discover project and test all files
                let cwd = std::env::current_dir().unwrap_or_default();
                match jett_driver::test_project(&cwd) {
                    Ok(result) => {
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
                        if result.total_failed > 0 {
                            process::exit(1);
                        }
                    }
                    Err(e) => {
                        eprintln!("error: {e}");
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
        "debug[{}]{{message}}:\n",
        output.debug_output.len()
    ));
    for line in &output.debug_output {
        out.push_str(&format!("  {}\n", escape_toon_scalar(line)));
    }
    out
}

fn render_run_agent_error(file: &str, error: &str) -> String {
    format!(
        "status: error\nfile: {}\nerror: {}\n",
        escape_toon_scalar(file),
        escape_toon_scalar(error)
    )
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
            debug_output: vec!["trace total: int64 = 42".to_string()],
        };

        let rendered = render_run_agent_output("app.jett", &output);

        assert_eq!(
            rendered,
            "status: ok\nfile: app.jett\nstdout: hello\\n\ndebug[1]{message}:\n  trace total: int64 = 42\n"
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
}
