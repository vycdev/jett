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
    },

    /// Run all verify and property blocks
    Test {
        /// Path to a .jett file (if omitted, finds jett.proj and tests all files)
        file: Option<String>,
    },
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
                    let error_count = result.diagnostics.iter()
                        .filter(|d| d.severity == jett_diagnostics::Severity::Error)
                        .count();
                    eprintln!("build failed: {error_count} error(s)");
                    process::exit(1);
                } else {
                    println!("build ok: {file} (type checked, no codegen yet)");
                }
            }
        }
        Command::Run { file } => {
            let path = Path::new(&file);
            match jett_driver::run_file(path) {
                Ok(()) => {}
                Err(e) => {
                    eprintln!("error: {e}");
                    process::exit(1);
                }
            }
        }
        Command::Test { file } => {
            if let Some(f) = file {
                // Test a single file
                let path = Path::new(&f);
                match jett_driver::test_file(path) {
                    Ok(result) => {
                        print_file_results(&result);
                        println!(
                            "\n{} verify block(s), {} passed, {} failed",
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
                            "{} file(s), {} verify block(s), {} passed, {} failed",
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

fn print_file_results(result: &jett_driver::TestResult) {
    for (name, passed, err) in &result.blocks {
        if *passed {
            println!("  verify {name}: ok");
        } else {
            let msg = err.as_deref().unwrap_or("unknown error");
            println!("  verify {name}: FAILED ({msg})");
        }
    }
}
