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
            let files: Vec<String> = if let Some(f) = file {
                vec![f]
            } else {
                // Look for jett.proj and collect all .jett files
                match find_project_files() {
                    Ok(f) => f,
                    Err(e) => {
                        eprintln!("error: {e}");
                        process::exit(1);
                    }
                }
            };

            let mut total_blocks = 0usize;
            let mut total_passed = 0usize;
            let mut total_failed = 0usize;

            for f in &files {
                let path = Path::new(f);
                match jett_driver::test_file(path) {
                    Ok(result) => {
                        for (name, passed, err) in &result.blocks {
                            if *passed {
                                println!("verify {}: ok", name);
                            } else {
                                let msg = err.as_deref().unwrap_or("unknown error");
                                println!("verify {}: FAILED ({})", name, msg);
                            }
                        }
                        total_blocks += result.total;
                        total_passed += result.passed;
                        total_failed += result.failed;
                    }
                    Err(e) => {
                        eprintln!("error testing {}: {e}", f);
                        process::exit(1);
                    }
                }
            }

            println!(
                "\n{} verify blocks, {} passed, {} failed",
                total_blocks, total_passed, total_failed
            );

            if total_failed > 0 {
                process::exit(1);
            }
        }
    }
}

/// Find all .jett files in the project by looking for jett.proj in the
/// current directory or its ancestors, then collecting all .jett files in
/// the `src/` directory.
fn find_project_files() -> Result<Vec<String>, String> {
    let cwd = std::env::current_dir().map_err(|e| format!("cannot determine cwd: {e}"))?;
    let mut dir = cwd.as_path();

    loop {
        let proj = dir.join("jett.proj");
        if proj.exists() {
            let src_dir = dir.join("src");
            if src_dir.is_dir() {
                let mut files = Vec::new();
                collect_jett_files(&src_dir, &mut files)
                    .map_err(|e| format!("error scanning src/: {e}"))?;
                if files.is_empty() {
                    return Err("no .jett files found in src/".to_string());
                }
                return Ok(files);
            }
            return Err(format!(
                "found jett.proj at {} but no src/ directory",
                dir.display()
            ));
        }
        match dir.parent() {
            Some(p) => dir = p,
            None => {
                return Err(
                    "no jett.proj found in current directory or any parent".to_string(),
                )
            }
        }
    }
}

fn collect_jett_files(dir: &Path, out: &mut Vec<String>) -> std::io::Result<()> {
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            collect_jett_files(&path, out)?;
        } else if path.extension().and_then(|e| e.to_str()) == Some("jett") {
            out.push(path.display().to_string());
        }
    }
    Ok(())
}
