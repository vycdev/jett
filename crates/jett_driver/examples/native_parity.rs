//! Exhaustive, deliberately failing-until-complete native parity release probe.
//! Usage: cargo run -p jett_driver --example native_parity -- LAUNCHER REPORT.json
//! Every inventory row is attempted, irrespective of staged object_emit markers.
use jett_driver::native::{self, NativeLauncherBundle};
use serde_json::{Value, json};
use std::collections::BTreeSet;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode, Output, Stdio};
use std::time::{Duration, Instant};

fn execute(binary: &Path, directory: &Path) -> Result<Output, String> {
    let mut child = Command::new(binary)
        .current_dir(directory)
        .env_clear()
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| e.to_string())?;
    let mut stdout = child.stdout.take().unwrap();
    let mut stderr = child.stderr.take().unwrap();
    let out = std::thread::spawn(move || {
        let mut b = Vec::new();
        stdout.read_to_end(&mut b).map(|_| b)
    });
    let err = std::thread::spawn(move || {
        let mut b = Vec::new();
        stderr.read_to_end(&mut b).map(|_| b)
    });
    let start = Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if start.elapsed() < Duration::from_secs(10) => {
                std::thread::sleep(Duration::from_millis(10))
            }
            outcome => {
                let _ = child.kill();
                let _ = child.wait();
                let _ = out.join();
                let _ = err.join();
                return Err(format!(
                    "native execution failed or exceeded 10s: {outcome:?}"
                ));
            }
        }
    };
    Ok(Output {
        status,
        stdout: out
            .join()
            .map_err(|_| "stdout reader panic")?
            .map_err(|e| e.to_string())?,
        stderr: err
            .join()
            .map_err(|_| "stderr reader panic")?
            .map_err(|e| e.to_string())?,
    })
}

fn behavior(
    source: &Path,
    binary: &Path,
    directory: &Path,
    expected_failure: bool,
) -> Result<Value, String> {
    let actual = execute(binary, directory)?;
    let stdout = String::from_utf8(actual.stdout).map_err(|e| e.to_string())?;
    let stderr = String::from_utf8(actual.stderr).map_err(|e| e.to_string())?;
    let (oracle, failure) = match jett_driver::run_file_capture_outcome(source) {
        Ok(output) => (output, None),
        Err(failure) => (failure.output, Some(failure.message)),
    };
    if failure.is_some() != expected_failure {
        return Err("interpreter outcome contradicts manifest".into());
    }
    let debug = if oracle.debug_output.is_empty() {
        String::new()
    } else {
        format!("{}\n", oracle.debug_output.join("\n"))
    };
    let matched = if let Some(message) = &failure {
        actual.status.code() == Some(71)
            && stdout == oracle.stdout
            && stderr.contains(message)
            && (debug.is_empty() || stderr.contains(&debug))
    } else {
        actual.status.success() && stdout == oracle.stdout && stderr == debug
    };
    Ok(
        json!({"behavior_matches":matched,"exit_code":actual.status.code(),"stdout":stdout,"stderr":stderr,
                "interpreter_stdout":oracle.stdout,"interpreter_debug":oracle.debug_output,"interpreter_failure":failure,
                // Current scalar/string ABI destruction checks its live owning-value
                // registry. Entry-failure 71 requires successful destruction; a leak
                // overrides it with 72. Launcher tests enforce both outcomes. Future
                // resource families also need finalizer/effect oracles before acceptance.
                "failure_cleanup_verified":expected_failure && matched && actual.status.code() == Some(71),
                "cleanup_contract":"native owning-value registry empty at checked context destruction"
        }),
    )
}

fn main() -> ExitCode {
    let args = std::env::args_os().skip(1).collect::<Vec<_>>();
    assert_eq!(args.len(), 2, "usage: native_parity LAUNCHER REPORT.json");
    let archive = fs::canonicalize(&args[0]).expect("launcher archive must exist");
    let launcher = match native::host_target().as_str() {
        native::LINUX_GNU_NATIVE_TARGET => NativeLauncherBundle::linux_gnu_v1(archive),
        native::WINDOWS_MSVC_NATIVE_TARGET => NativeLauncherBundle::windows_msvc_static_v1(archive),
        target => panic!("unsupported native execution host: {target}"),
    };
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap();
    let manifest: Value =
        serde_json::from_slice(&fs::read(root.join("tests/native_parity.json")).unwrap()).unwrap();
    let fixtures = manifest["fixtures"].as_array().unwrap();
    let paths = fixtures
        .iter()
        .map(|f| f["path"].as_str().unwrap().to_owned())
        .collect::<BTreeSet<_>>();
    assert_eq!(paths.len(), fixtures.len(), "duplicate manifest rows");
    let discovered = ["run_pass", "runtime_fail"]
        .into_iter()
        .flat_map(|category| {
            fs::read_dir(root.join("tests").join(category))
                .unwrap()
                .map(move |p| (category, p.unwrap().path()))
        })
        .filter(|(_, p)| p.extension().is_some_and(|e| e == "jett"))
        .map(|(c, p)| format!("tests/{c}/{}", p.file_name().unwrap().to_str().unwrap()))
        .collect::<BTreeSet<_>>();
    assert_eq!(paths, discovered, "no inventory omissions permitted");
    let mut rows = Vec::new();
    let (mut lower_pass, mut object_pass, mut main_pass, mut runtime_pass) = (0, 0, 0, 0);
    let (mut lower_total, mut main_total, mut runtime_total) = (0, 0, 0);
    for fixture in fixtures {
        let path = fixture["path"].as_str().unwrap();
        let obligations = fixture["obligations"].as_array().unwrap();
        let lower = obligations.iter().any(|o| o == "lower");
        let main = obligations.iter().any(|o| o == "main_execute");
        let runtime = obligations.iter().any(|o| o == "runtime_contract");
        lower_total += usize::from(lower);
        main_total += usize::from(main);
        runtime_total += usize::from(runtime);
        let source = root.join(path);
        let mut row = json!({"path":path,"obligations":obligations});
        match jett_driver::lower_file_for_backend(&source) {
            Err(error) => row["lower_error"] = json!(error.to_string()),
            Ok(lowered) => {
                row["lowered"] = json!(true);
                lower_pass += usize::from(lower);
                match jett_codegen_cranelift::emit_host_object(&lowered.mir, &lowered.interner) {
                    Ok(object) if !object.bytes.is_empty() && !object.symbols.is_empty() => {
                        row["object_emitted"] = json!(true);
                        row["object_bytes"] = json!(object.bytes.len());
                        row["defined_symbols"] = json!(object.symbols.len());
                        object_pass += usize::from(lower);
                    }
                    Ok(_) => {
                        row["object_error"] = json!("empty object or no reachable code symbols")
                    }
                    Err(error) => row["object_error"] = json!(error.to_string()),
                }
            }
        }
        if main || runtime {
            let directory = tempfile::tempdir().unwrap();
            let output = directory.path().join(if cfg!(windows) {
                "program.exe"
            } else {
                "program"
            });
            match native::build_host_executable(&source, &launcher, &output) {
                Err(error) => row["execution_error"] = json!(error.to_string()),
                Ok(_) => {
                    row["linked"] = json!(true);
                    let failure = fixture["expected_outcome"] == "expected_failure";
                    match behavior(&source, &output, directory.path(), failure) {
                        Err(error) => row["execution_error"] = json!(error),
                        Ok(result) => {
                            let pass = execution_passes(&result, failure);
                            main_pass += usize::from(main && pass);
                            runtime_pass += usize::from(runtime && pass);
                            row["execution"] = result;
                        }
                    }
                }
            }
        }
        eprintln!("{} / {}: {path}", rows.len() + 1, fixtures.len());
        rows.push(row);
        // Persist every batch; a failed or interrupted gate never erases evidence.
        fs::write(
            &args[1],
            serde_json::to_vec_pretty(&json!({"complete":false,"fixtures":rows})).unwrap(),
        )
        .unwrap();
    }
    let counts = json!({"lower":[lower_pass,lower_total],"object":[object_pass,lower_total],
        "main":[main_pass,main_total],"runtime_contract":[runtime_pass,runtime_total]});
    let complete = lower_pass == lower_total
        && object_pass == lower_total
        && main_pass == main_total
        && runtime_pass == runtime_total;
    let report = json!({"complete":complete,"target":native::host_target(),"counts":counts,"fixtures":rows,
        "pending_release_gates":["move-only resource finalizer instrumentation","capability-effect differential oracles","clean Windows MSVC distribution"]});
    fs::write(&args[1], serde_json::to_vec_pretty(&report).unwrap()).unwrap();
    println!("{counts}");
    if complete {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

fn execution_passes(result: &Value, expected_failure: bool) -> bool {
    result["behavior_matches"] == true
        && (!expected_failure || result["failure_cleanup_verified"] == true)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn expected_failures_count_only_with_cleanup_evidence() {
        let verified = json!({"behavior_matches":true, "failure_cleanup_verified":true});
        assert!(execution_passes(&verified, true));
        assert!(!execution_passes(
            &json!({"behavior_matches":true, "failure_cleanup_verified":false}),
            true
        ));
        assert!(!execution_passes(
            &json!({"behavior_matches":false, "failure_cleanup_verified":true}),
            true
        ));
        assert!(execution_passes(&json!({"behavior_matches":true}), false));
    }
}
