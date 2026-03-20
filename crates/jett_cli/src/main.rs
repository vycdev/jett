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
            release,
            agent,
            target,
        } => {
            let mode = if release { "release" } else { "debug" };
            print!("building {file} ({mode})");
            if agent {
                print!(" [agent/TOON]");
            }
            if let Some(ref t) = target {
                print!(" for {t}");
            }
            println!();
            eprintln!("error: jett build is not yet implemented");
            process::exit(1);
        }
        Command::Run { file } => {
            println!("running {file}");
            eprintln!("error: jett run is not yet implemented");
            process::exit(1);
        }
    }
}
