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
            agent: _,
            target: _,
        } => {
            let path = Path::new(&file);
            let result = jett_driver::build_file(path);

            for diag in &result.diagnostics {
                let prefix = match diag.severity {
                    jett_diagnostics::Severity::Error => "error",
                    jett_diagnostics::Severity::Warning => "warning",
                    jett_diagnostics::Severity::Info => "info",
                };
                eprintln!("{prefix}[{}]: {}", diag.code, diag.message);
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
    }
}
